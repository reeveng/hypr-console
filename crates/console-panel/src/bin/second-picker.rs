//! A picker a test can be the second of.
//!
//! The lock is between processes, so nothing inside one process proves anything
//! about it. This is the other process. It takes the screen and keeps it, or it
//! asks for the screen once and says what it was told.
//!
//! It says "held" on the way in rather than being given a moment, because
//! waiting for a line is waiting for the lock and waiting for a moment is a test
//! that fails on a busy machine.

use std::process::ExitCode;

use console_core_arguments::{Command, Operands, Subcommand, ValidationError, read_with};
use console_core_never::Never;
use console_core_words::Words;
use console_panel::picker::{Again, Alone, alone, mark_drawn, gone};

const KEPT: std::time::Duration = std::time::Duration::from_secs(30);

const DRAWING: std::time::Duration = std::time::Duration::from_millis(200);

const GOING: std::time::Duration = std::time::Duration::from_millis(300);

fn as_word(so: Alone) -> Result<&'static str, Never> {
    Ok(match so {
        Alone::Yes => "yes",
        Alone::No => "no",
    })
}

fn took(name: &str) -> Result<(), Never> {
    let Ok(alone) = alone(name, Again::Closes);

    assert_eq!(alone, Alone::Yes, "something was already holding it");

    Ok(())
}

fn posing(for_: std::time::Duration) -> Result<(), Never> {
    #[cfg_attr(
        dylint_lib = "explicit021_no_sleeping",
        allow(
            explicit021_no_sleeping,
            reason = "this program is a clock and nothing else: it exists so a test has a second process that holds the screen for a known length of time, or draws late, or leaves late, and the waiting is the behavior being posed rather than something being waited for"
        )
    )]
    std::thread::sleep(for_);

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
enum Pose {
    #[words(word = "hold", about = "take the screen and keep it")]
    Hold,
    #[words(word = "coming", about = "take the screen and draw late")]
    Coming,
    #[words(word = "going", about = "take the screen, draw, and leave late")]
    Going,
    #[words(word = "stuck", about = "take the screen and never draw")]
    Stuck,
    #[words(word = "twice", about = "ask for the screen twice and say both answers")]
    Twice,
    #[words(word = "ask", about = "ask for the screen once and say the answer")]
    Ask,
}

impl Subcommand for Pose {
    fn variants() -> Result<impl Iterator<Item = Self>, Never> {
        Ok(Pose::VARIANTS.iter().copied())
    }

    fn spelling(self) -> Result<&'static str, Never> {
        self.word()
    }

    fn about(self) -> Result<&'static str, Never> {
        Pose::about(self)
    }
}

const NAME: [&str; 1] = ["NAME"];

const COMMAND: Command = Command {
    name: "second-picker",
    about: "a picker a test can be the second of, posing one way under NAME",
    flags: &[],
    operands: Operands::Named(&NAME),
};

fn request(words: &[String]) -> Result<(Pose, String), ValidationError> {
    let read = read_with::<Pose, String>(&COMMAND, words);
    let line = read?;
    let required = line.require_subcommand();
    let pose = required?;
    let operands = line.exactly(NAME);
    let [name] = operands?;

    Ok((pose, name.clone()))
}

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let (pose, name) = match request(&words) {
        Ok(request) => request,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };

    match pose {
        Pose::Hold => {
            let Ok(()) = took(&name);
            let Ok(()) = mark_drawn();

            println!("held");
            let Ok(()) = posing(KEPT);
        }
        Pose::Coming => {
            let Ok(()) = took(&name);

            println!("held");
            let Ok(()) = posing(DRAWING);

            let Ok(()) = mark_drawn();

            let Ok(()) = posing(KEPT);
        }
        Pose::Going => {
            let Ok(()) = took(&name);
            let Ok(()) = mark_drawn();
            let Ok(()) = gone();

            println!("held");
            let Ok(()) = posing(GOING);
        }
        Pose::Stuck => {
            let Ok(()) = took(&name);

            println!("held");
            let Ok(()) = posing(KEPT);
        }
        Pose::Twice => {
            let Ok(one) = alone(&name, Again::Closes);
            let Ok(other) = alone(&name, Again::Closes);
            let Ok(one) = as_word(one);
            let Ok(other) = as_word(other);

            println!("{one} {other}");
        }
        Pose::Ask => {
            let Ok(so) = alone(&name, Again::Closes);
            let Ok(so) = as_word(so);

            println!("{so}");
        }
    }

    ExitCode::SUCCESS
}
