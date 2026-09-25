//! A program that asked to be told, told.
//!
//! Everything either half of this can be asked on its own is asked on its own
//! already: `console_program_contract::transcript` presses what a program
//! decides when it hears a word, and `console-events`' own tests press what
//! the pool says to whoever subscribed. What neither can ask is whether the
//! ask reaches the pool at all -- for as long as the runtime answered
//! `Subscription::Words` with a line on the journal, both halves were green and the
//! desktop still polled. So this is the seam: a real pool on a real socket, a
//! real program run by the real loop, and the only thing the test supplies is
//! the source, because the only sound card is the one it is running on.

use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use console_core_never::Never;
use console_events::serving;
use console_events::sources::Subscribed;
use console_program_contract::{
    Arguments, Effect, Exit, Initial, Program, Timer, Topic, Update, Subscription, Event,
};
use console_program_contract::event::Change;
use console_program_runtime::{Interpreter, run};

const BEFORE_LONG: Duration = Duration::from_secs(5);

const GIVING_UP: Timer = Timer { name: "giving up", interval: Duration::from_secs(20) };

static SAYING: std::sync::OnceLock<Sender<Sender<Change>>> = std::sync::OnceLock::new();

fn source(topic: &Topic, say: Sender<Change>) -> Result<Subscribed, Never> {
    console_events::sources::handed_to(SAYING.get(), &Topic::Sound, topic, say)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rocker {
    said: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestEffect {
    Logged(String),
}

impl Program for Rocker {
    type State = Rocker;
    type Event = Never;
    type Effect = TestEffect;

    fn init(_argv: &Arguments) -> Initial<Rocker> {
        let Ok(opening) = Initial::subscribed(
            Rocker { said: None },
            vec![Subscription::Topic(Topic::Sound), Subscription::Timer(GIVING_UP)],
        );

        opening
    }

    fn update(state: &Rocker, event: &Event<Never>) -> Update<Rocker, TestEffect> {
        match event {
            Event::Changed(changed) => {
                let Ok(turn) = Update::new(
                    Rocker { said: Some(changed.text.clone()) },
                    vec![Effect::Custom(TestEffect::Logged(changed.text.clone())), Effect::Stop(Exit::Success)],
                );

                turn
            }
            Event::Tick(_, _) => {
                let Ok(turn) = Update::new(
                    state.clone(),
                    vec![Effect::Stop(Exit::Failure("nothing was ever told".to_string()))],
                );

                turn
            }
            Event::Opened | Event::Replied(_) | Event::Chosen(_) | Event::Stopping => {
                let Ok(turn) = Update::none(state.clone());

                turn
            }
            Event::Custom(its) => match *its {},
        }
    }
}

struct Keeping(Arc<Mutex<Vec<String>>>);

impl Interpreter for Keeping {
    type Event = Never;
    type Effect = TestEffect;

    fn interpret(&mut self, acts: &TestEffect) -> Vec<Event<Never>> {
        match acts {
            TestEffect::Logged(said) => {
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
        let Ok(arguments) = Arguments::of(&[]);
        let mut carrying = Keeping(keeping);
        let _ = run::<Rocker, Keeping>("rocker", &arguments, &mut carrying);
        let _ = done.send(());
    });

    let saying = handed.recv_timeout(BEFORE_LONG).expect(
        "the runtime never asked the pool for the topic the program said it wanted",
    );

    saying
        .send(Change { topic: Topic::Sound, text: "sink 1 at 40%".to_string() })
        .expect("the pool stopped listening to its own source");

    ended.recv_timeout(BEFORE_LONG).expect("the program was never told anything and never stopped");

    let held = match kept.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    };

    assert_eq!(*held, vec!["sink 1 at 40%".to_string()]);

    let _ = std::fs::remove_dir_all(&where_);
}
