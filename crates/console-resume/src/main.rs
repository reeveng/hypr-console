//! The words this program is started with, and what each of them does.
//!
//! `console-session.service` starts it with no words at all, and no words is the
//! whole of what this desktop asks of it: put back what was open, once for this
//! compositor, and then keep saving what is on the screen. The other modes are
//! here because someone with a broken session needs a way to look at one and to
//! throw one away, and because `save` once is what a check presses.
//!
//! ## No words does not mean `load`
//!
//! It did in the fork, and `load` closes every window before it starts anything.
//! Under `Restart=always` that made a crash indistinguishable from a log-in: the
//! desktop was swept and rebuilt five seconds later, in front of whoever was
//! using it. It is also what an accidental `cargo run` did to this machine.
//!
//! So no words is [`Mode::Default`], which puts back only if
//! `console_resume::already` says this compositor has not had it done yet, and
//! watches either way. `load` is the word for doing it now regardless, and it is
//! a word someone has to type.
//!
//! ## Nothing saved is not a fault, and asking for a session that is not there is
//!
//! `load` typed by a person names a session, and a name no one saved is worth a
//! word and a status: they asked for something that is not there. The desktop's
//! own first start asks for the same thing and means something else -- a machine
//! that has never saved a session has nothing to put back, which is what a first
//! start *is*. Told those apart by the mode rather than by the file, because
//! the file says the same thing to both.
//!
//! It was one sentence and both, which made the ordinary first start of a
//! fresh device exit 1 without ever reaching the watching -- and
//! `console-session` carries `ExecStopPost=console-report-crash`, so it said
//! so on the screen as a program that had fallen over. `Restart=always` then
//! started it again, the mark from the first attempt made the second one a
//! no-op, and it watched. The desktop worked; the notification was true about
//! the status and wrong about the desktop.
//!
//! ## A word this does not know stops it
//!
//! The fork took clap, which is a dependency and a help screen for a program
//! no one types. It is also what made `--help` harmless there and a swept desktop
//! here: with clap gone, an argument nothing recognises left no mode word, and no
//! mode word was `load`. The words are declared now, with `console-core-arguments`
//! reading them, so a word this does not know draws the usage and stops, and
//! that is the only thing an unknown word may do. A value is the word after its
//! flag, `--save-interval 30`, and the fork's `--save-interval=30` is one of the
//! words it does not know.

use std::env;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use console_core_arguments::{Command, CommandLine, Flag, Operands, Presence, Reason, Subcommand, Takes, ValidationError, read_with};
use console_core_never::Never;
use console_core_words::Words;
use console_resume::{SAVE_EVERY, Unresumed};
use console_resume::already::Already;
use console_resume::session::{Duplicates, Restore, Really, Restoring, Sessions};

const UNLESS_NAMED: &str = "default";


const SAVE_INTERVAL: Duration = Duration::from_secs(60);
const ADJUSTING_FOR: Duration = Duration::from_secs(60);

const WHERE: &str = "CONSOLE_RESUME_PATH";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
enum Mode {
    #[words(word = "default", about = "put back what was open, once for this compositor, then keep saving")]
    Default,
    #[words(word = "save", about = "save what is on the screen now, once")]
    Save,
    #[words(word = "watch", about = "keep saving what is on the screen")]
    Watch,
    #[words(word = "list", about = "the sessions that are saved")]
    List,
    #[words(word = "load", about = "close every window and put back the session, now")]
    Load,
    #[words(word = "clear", about = "throw away every session")]
    Clear,
    #[words(word = "delete", about = "throw away the session")]
    Delete,
}

impl Subcommand for Mode {
    fn variants() -> Result<impl Iterator<Item = Self>, Never> {
        Ok(Mode::VARIANTS.iter().copied())
    }

    fn spelling(self) -> Result<&'static str, Never> {
        self.word()
    }

    fn about(self) -> Result<&'static str, Never> {
        Mode::about(self)
    }
}

struct Arguments {
    mode: Mode,
    name: String,
    save_interval: Duration,
    adjusting_for: Duration,
    really: Really,
    restoring: Restoring,
}

const LOAD_TIME: Flag = Flag {
    spelling: "--load-time",
    takes: Takes::Value("SECONDS"),
    about: "how long a program put back is given to open its windows",
};

const SIMULATE: Flag = Flag { spelling: "--simulate", takes: Takes::None, about: "say what it would do, and do none of it" };

const ADJUST_CLIENTS_ONLY: Flag = Flag {
    spelling: "--adjust-clients-only",
    takes: Takes::None,
    about: "move what is open to where it was, and start nothing",
};

const COMMAND: Command = Command {
    name: "console-resume",
    about: "put back what was open, and keep saving what is on the screen",
    flags: &[SAVE_EVERY, LOAD_TIME, SIMULATE, ADJUST_CLIENTS_ONLY],
    operands: Operands::Optional("NAME"),
};

fn seconds(line: &CommandLine<Mode>, flag: Flag, unless: Duration) -> Result<Duration, ValidationError> {
    let said = line.parsed::<u64>(flag)?;

    Ok(match said {
        Some(seconds) => Duration::from_secs(seconds),
        None => unless,
    })
}

#[derive(Debug)]
enum Unstarted {
    Homeless,
    Making(PathBuf, std::io::Error),
    NoSession(PathBuf),
    Resuming(Unresumed),
}

impl std::fmt::Display for Unstarted {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unstarted::Homeless => write!(to, "there is no home to keep a session in"),
            Unstarted::Making(at, fault) => {
                write!(to, "{}: making it: {fault}", at.display())
            }
            Unstarted::NoSession(at) => write!(
                to,
                "{} has no session in it, so the screen is left as it is",
                at.display()
            ),
            Unstarted::Resuming(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for Unstarted {}

impl From<Unresumed> for Unstarted {
    fn from(fault: Unresumed) -> Self {
        Unstarted::Resuming(fault)
    }
}

fn parse_arguments(words: &[String]) -> Result<Arguments, ValidationError> {
    let line = read_with::<Mode, String>(&COMMAND, words)?;
    let Ok(said) = line.subcommand();
    let Ok(named) = line.operands();
    let save_interval = seconds(&line, SAVE_EVERY, SAVE_INTERVAL)?;
    let adjusting_for = seconds(&line, LOAD_TIME, ADJUSTING_FOR)?;
    let Ok(simulate) = line.presence(SIMULATE);
    let Ok(adjust) = line.presence(ADJUST_CLIENTS_ONLY);

    match save_interval.is_zero() {
        true => {
            let Ok(refusal) = line.refusal(Reason::InvalidValue { of: SAVE_EVERY.spelling, value: "0".to_string() });

            return Err(refusal);
        },
        false => {},
    }

    Ok(Arguments {
        mode: match said {
            Some(mode) => mode,
            None => Mode::Default,
        },
        name: match named.first() {
            Some(name) => name.clone(),
            None => UNLESS_NAMED.to_string(),
        },
        save_interval,
        adjusting_for,
        really: match simulate {
            Presence::Present => Really::Simulated,
            Presence::Absent => Really::Truly,
        },
        restoring: match adjust {
            Presence::Present => Restoring::MovingWhatIsOpen,
            Presence::Absent => Restoring::StartingItAgain,
        },
    })
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "CONSOLE_RESUME_PATH is where a session is remembered, and the const beside it is the only spelling of the name"
    )
)]
fn where_sessions_live() -> Result<PathBuf, Unstarted> {
    match env::var(WHERE) {
        Ok(said) => return Ok(PathBuf::from(said)),
        Err(_it_was_not_said) => {},
    }

    let Ok(ours) = console_core_places::Base::Share.ours();

    match ours {
        Some(ours) => Ok(ours.join(console_resume::APPLICATION)),
        None => Err(Unstarted::Homeless),
    }
}

fn putting_back(sessions: &Sessions, name: &str) -> Result<(), Unstarted> {
    let Ok(already) = console_resume::already::check();

    match already {
        Already::Restore => {
            println!("this compositor has had its windows put back already");

            return Ok(());
        },
        Already::CannotTell => {
            eprintln!(
                "nothing says which compositor this is, so a restart cannot be told from a \
                 log-in and nothing is put back"
            );

            return Ok(());
        },
        Already::NotYet => {},
    }

    console_resume::already::mark_resumed()?;

    let put_back = sessions.load(name)?;

    match put_back {
        Restore::Windows => {},
        Restore::NothingSaved(at) => {
            println!("{} has nothing saved in it yet, so there is nothing to put back", at.display());
        },
    }

    Ok(())
}

fn run(asked: Arguments) -> Result<(), Unstarted> {

    let at = where_sessions_live()?;

    create_dir_all(&at).map_err(|fault| Unstarted::Making(at.clone(), fault))?;

    let sessions = Sessions {
        at,
        adjusting_for: asked.adjusting_for,
        really: asked.really,
        restoring: asked.restoring,
        duplicates: Duplicates::OnePerProgram,
    };

    match asked.mode {
        Mode::Clear => sessions.clear()?,
        Mode::Default => putting_back(&sessions, &asked.name)?,
        Mode::Load => {
            let put_back = sessions.load(&asked.name)?;

            match put_back {
                Restore::Windows => {},
                Restore::NothingSaved(at) => return Err(Unstarted::NoSession(at)),
            }
        },
        Mode::Watch => {},
        Mode::Save => sessions.save(&asked.name)?,
        Mode::Delete => sessions.delete(&asked.name)?,
        Mode::List => {
            let Ok(saved) = sessions.list();

            match saved.first() {
                Some(_there_are_some) => {
                    for name in saved {
                        println!(" - {name}");
                    }
                },
                None => println!("(no sessions saved)"),
            }
        },
    }

    match asked.mode {
        Mode::Default | Mode::Watch => sessions
            .watch(&asked.name, asked.save_interval)
            .map_err(Unstarted::Resuming),
        Mode::Save | Mode::List | Mode::Load | Mode::Clear | Mode::Delete => Ok(()),
    }
}

fn main() -> ExitCode {
    let words: Vec<String> = env::args().skip(1).collect();
    let asked = match parse_arguments(&words) {
        Ok(asked) => asked,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        },
    };

    match run(asked) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("console-resume: {why}");

            ExitCode::FAILURE
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(said: &[&str]) -> Result<Vec<String>, Never> {
        Ok(said.iter().map(|word| (*word).to_string()).collect())
    }

    #[test]
    fn no_words_at_all_is_what_the_unit_starts() -> Result<(), ValidationError> {
        let Ok(none) = words(&[]);
        let asked = parse_arguments(&none)?;

        assert_eq!(asked.mode, Mode::Default);
        assert_eq!(asked.name, "default".to_string());

        Ok(())
    }

    #[test]
    fn no_words_is_not_the_mode_that_closes_every_window() -> Result<(), ValidationError> {
        let Ok(none) = words(&[]);
        let asked = parse_arguments(&none)?;

        assert_ne!(asked.mode, Mode::Load, "a bare run of this program must not sweep the desktop");

        Ok(())
    }

    #[test]
    fn a_word_this_does_not_know_is_refused_rather_than_read_as_no_word_at_all() {
        for unknown in ["--help", "--version", "sideways", "--save-duplicate-pids", "--save-interval=30"] {
            let Ok(said) = words(&[unknown]);

            assert!(
                matches!(parse_arguments(&said), Err(_refusal)),
                "{unknown} was read as no word at all, which is the mode that sweeps the desktop"
            );
        }
    }

    #[test]
    fn a_mode_no_one_here_says_names_the_ones_that_are_said() {
        let Ok(said) = words(&["sideways"]);
        let why = match parse_arguments(&said) {
            Err(why) => format!("{why}\n\n{}", why.usage),
            Ok(_no_such_mode) => "sideways was read as a mode".to_string(),
        };

        assert!(why.contains("delete"), "{why}");
    }

    #[test]
    fn the_mode_is_a_word_and_the_name_is_the_word_after_it() -> Result<(), ValidationError> {
        let Ok(said) = words(&["load", "yesterday"]);
        let asked = parse_arguments(&said)?;

        assert_eq!(asked.mode, Mode::Load);
        assert_eq!(asked.name, "yesterday".to_string());

        Ok(())
    }

    #[test]
    fn a_flag_is_read_wherever_it_stands_among_the_words() -> Result<(), ValidationError> {
        let Ok(said) = words(&["save", "--load-time", "5", "nightly", "--simulate"]);
        let asked = parse_arguments(&said)?;

        assert_eq!(asked.mode, Mode::Save);
        assert_eq!(asked.name, "nightly".to_string());
        assert_eq!(asked.adjusting_for, Duration::from_secs(5));
        assert_eq!(asked.really, Really::Simulated);

        Ok(())
    }

    #[test]
    fn saving_every_nought_seconds_is_refused_rather_than_spun_on() {
        let Ok(said) = words(&["--save-interval", "0"]);

        assert_eq!(
            parse_arguments(&said).map(|asked| asked.save_interval).map_err(|refusal| refusal.reason),
            Err(Reason::InvalidValue { of: SAVE_EVERY.spelling, value: "0".to_string() })
        );
    }
}
