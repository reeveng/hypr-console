//! A program that asked to be told, told.
//!
//! Everything either half of this can be asked on its own is asked on its own
//! already: `console_program_contract::transcript` presses what a program
//! decides when it hears a word, and `console-events`' own tests press what
//! the pool says to whoever subscribed. What neither can ask is whether the
//! ask reaches the pool at all -- for as long as the runtime answered
//! `Wants::Words` with a line on the journal, both halves were green and the
//! desktop still polled. So this is the seam: a real pool on a real socket, a
//! real program run by the real loop, and the only thing the test supplies is
//! the source, because the only sound card is the one it is running on.

use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use console_core_never::Never;
use console_events::serving;
use console_events::sources::Held;
use console_program_contract::{
    Argv, Doing, Ending, Opening, Program, Round, Topic, Turn, Wants, Word,
};
use console_program_contract::word::Changed;
use console_program_runtime::{Carrying, run};

const BEFORE_LONG: Duration = Duration::from_secs(5);

const GIVING_UP: Round = Round { called: "giving up", every: Duration::from_secs(20) };

static SAYING: std::sync::OnceLock<Sender<Sender<Changed>>> = std::sync::OnceLock::new();

fn source(topic: &Topic, say: Sender<Changed>) -> Result<Held, Never> {
    Ok(match topic {
        Topic::Sound => match SAYING.get() {
            Some(handing) => match handing.send(say) {
                Ok(()) => Held::Yes,
                Err(_) => Held::Nothing,
            },
            None => Held::Nothing,
        },
        Topic::Compositor
        | Topic::Network
        | Topic::Notices
        | Topic::Units
        | Topic::Player
        | Topic::Path(_) => Held::Nothing,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rocker {
    said: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Its {
    Told(String),
}

impl Program for Rocker {
    type State = Rocker;
    type Hears = Never;
    type Does = Its;

    fn opening(_argv: &Argv) -> Opening<Rocker> {
        let Ok(opening) = Opening::listening(
            Rocker { said: None },
            vec![Wants::Words(Topic::Sound), Wants::Round(GIVING_UP)],
        );

        opening
    }

    fn heard(state: &Rocker, word: &Word<Never>) -> Turn<Rocker, Its> {
        match word {
            Word::Changed(changed) => {
                let Ok(turn) = Turn::doing(
                    Rocker { said: Some(changed.said.clone()) },
                    vec![Doing::Its(Its::Told(changed.said.clone())), Doing::Stop(Ending::Done)],
                );

                turn
            }
            Word::CameRound(_, _) => {
                let Ok(turn) = Turn::doing(
                    state.clone(),
                    vec![Doing::Stop(Ending::Badly("nothing was ever told".to_string()))],
                );

                turn
            }
            Word::Opened | Word::Answered(_) | Word::Chose(_) | Word::Stopping => {
                let Ok(turn) = Turn::nothing(state.clone());

                turn
            }
            Word::Its(its) => match *its {},
        }
    }
}

struct Keeping(Arc<Mutex<Vec<String>>>);

impl Carrying for Keeping {
    type Hears = Never;
    type Does = Its;

    fn its(&mut self, doing: &Its) -> Vec<Word<Never>> {
        match doing {
            Its::Told(said) => {
                let mut kept = match self.0.lock() {
                    Ok(kept) => kept,
                    Err(poisoned) => poisoned.into_inner(),
                };

                kept.push(said.clone());
            }
        }

        Vec::new()
    }
}

fn runtime_dir() -> PathBuf {
    std::env::temp_dir().join(format!("console-runtime-words-{}", std::process::id()))
}

fn up(at: &std::path::Path) {
    let began = std::time::Instant::now();

    while began.elapsed() < BEFORE_LONG {
        match at.exists() {
            true => return,
            false => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

#[test]
fn a_program_that_asked_to_be_told_hears_the_pool_rather_than_a_line_on_the_journal() {
    let where_ = runtime_dir();
    let _ = std::fs::create_dir_all(where_.join("console"));

    // SAFETY: nothing else in this test binary reads or writes the environment,
    unsafe { std::env::set_var("XDG_RUNTIME_DIR", &where_) };

    let at = where_.join("console").join("events.sock");
    let (handing, handed) = channel();
    let _ = SAYING.set(handing);

    let serving = at.clone();
    let _ = std::thread::spawn(move || serving::serve(&serving, source));

    up(&at);

    let kept = Arc::new(Mutex::new(Vec::new()));
    let keeping = Arc::clone(&kept);
    let (done, ended) = channel();

    let _ = std::thread::spawn(move || {
        let Ok(argv) = Argv::of(&[]);
        let mut carrying = Keeping(keeping);
        let _ = run::<Rocker, Keeping>("rocker", &argv, &mut carrying);
        let _ = done.send(());
    });

    let saying = handed.recv_timeout(BEFORE_LONG).expect(
        "the runtime never asked the pool for the topic the program said it wanted",
    );

    saying
        .send(Changed { about: Topic::Sound, said: "sink 1 at 40%".to_string() })
        .expect("the pool stopped listening to its own source");

    ended.recv_timeout(BEFORE_LONG).expect("the program was never told anything and never stopped");

    let held = match kept.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    };

    assert_eq!(*held, vec!["sink 1 at 40%".to_string()]);

    let _ = std::fs::remove_dir_all(&where_);
}
