//! A program that asked to be told, told.
//!
//! Everything either half of this can be asked on its own is asked on its own
//! already: `console_core_state_machine::run` presses what a program
//! decides when it hears a word, and `console-events`' own tests press what
//! the pool says to whoever subscribed. What neither can ask is whether the
//! ask reaches the pool at all -- for as long as the runtime answered
//! `Subscription::Words` with a line on the journal, both halves were green and the
//! desktop still polled. So this is the seam: a real pool on a real socket, a
//! real program run by the real loop, and the only thing the test supplies is
//! the source, because the only sound card is the one it is running on.

use std::error::Error;
use std::path::Path;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use console_core_never::Never;
use console_events::serving;
use console_events::sources::Subscribed;
use console_core_state_machine::{Machine, Queue};
use console_program_contract::{Arguments, Effect, Exit, Timer, Topic, Subscription, Event};
use console_program_contract::event::Change;
use console_program_lifetime::threads::let_go;
use console_program_runtime::{Interpreter, run};
use console_waiting::{Outcome, Ready, Schedule, until};

const BEFORE_LONG: Duration = Duration::from_secs(5);

const GIVING_UP: Timer = Timer { name: "giving up", interval: Duration::from_secs(20) };

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the pool asks its source through a plain function, which can close over nothing, so the one way this test can hand the source the channel it answers on is a value the process holds"
    )
)]
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

impl Machine for Rocker {
    type Input = Arguments;
    type State = Rocker;
    type Request = Event<Never>;
    type Effect = Effect<TestEffect>;

    fn initialize(_arguments: &Arguments, _previous: Option<Rocker>, effects: &mut Queue<Effect<TestEffect>>) -> Result<Rocker, Never> {
        #[cfg_attr(
            dylint_lib = "explicit043_no_unmatched_listen",
            allow(
                explicit043_no_unmatched_listen,
                reason = "this program exists to hear one change on the topic, and hearing it or giving up stops it, so it listens exactly as long as it runs"
            )
        )]
        let Ok(()) = effects.offer_all([
            Effect::Subscribe(Subscription::Topic(Topic::Sound)),
            Effect::Subscribe(Subscription::Timer(GIVING_UP)),
        ]);

        Ok(Rocker { said: None })
    }

    fn handle(state: Rocker, event: Event<Never>, effects: &mut Queue<Effect<TestEffect>>) -> Result<Rocker, Never> {
        match event {
            Event::Changed(changed) => {
                let Ok(()) = effects.offer_all([
                    Effect::Custom(TestEffect::Logged(changed.text.clone())),
                    Effect::Stop(Exit::Success),
                ]);

                Ok(Rocker { said: Some(changed.text) })
            }
            Event::Tick(_, _) => {
                let Ok(()) = effects.offer(Effect::Stop(Exit::Failure("nothing was ever told".to_string())));

                Ok(state)
            }
            Event::Opened | Event::Replied(_) | Event::Chosen(_) | Event::Stopping => Ok(state),
            Event::Custom(its) => match its {},
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

fn up(at: &Path) -> Result<Outcome, Never> {
    let Ok(patience) = Schedule::of(BEFORE_LONG);

    until(patience, || {
        Ok(match at.exists() {
            true => Ready::Yes,
            false => Ready::NotYet,
        })
    })
}

#[test]
#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the runtime finds the pool where the session says its sockets are, and this binary is its own session: the one test in it points that at a folder of its own before anything reads it"
    )
)]
fn a_program_that_asked_to_be_told_hears_the_pool_rather_than_a_line_on_the_journal() -> Result<(), Box<dyn Error>> {
    let where_ = console_core_temporary_directories::fresh("runtime-words")?;

    std::fs::create_dir_all(where_.join("console"))?;

    // SAFETY: nothing else in this test binary reads or writes the environment,
    unsafe { std::env::set_var("XDG_RUNTIME_DIR", &where_) };

    let at = where_.join("console").join("events.sock");
    let (handing, handed) = channel();
    let _ = SAYING.set(handing);

    let serving = at.clone();
    let Ok(()) = let_go(std::thread::spawn(move || {
        let _ = serving::serve(&serving, source);
    }));

    assert_eq!(up(&at), Ok(Outcome::Happened), "the pool never opened its socket");

    let kept = Arc::new(Mutex::new(Vec::new()));
    let keeping = Arc::clone(&kept);
    let (done, ended) = channel();

    let running = std::thread::spawn(move || {
        let Ok(arguments) = Arguments::of(&[]);
        let mut carrying = Keeping(keeping);
        let _ = run::<Rocker, Keeping>("rocker", &arguments, &mut carrying);
        let _ = done.send(());
    });

    let saying = handed
        .recv_timeout(BEFORE_LONG)
        .map_err(|why| format!("the runtime never asked the pool for the topic the program said it wanted: {why}"))?;

    saying
        .send(Change { topic: Topic::Sound, text: "sink 1 at 40%".to_string() })
        .map_err(|why| format!("the pool stopped listening to its own source: {why}"))?;

    ended
        .recv_timeout(BEFORE_LONG)
        .map_err(|why| format!("the program was never told anything and never stopped: {why}"))?;

    running.join().map_err(|_| "the program's thread did not come back")?;

    let held = match kept.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    };

    assert_eq!(*held, vec!["sink 1 at 40%".to_string()]);

    let _ = std::fs::remove_dir_all(&where_);
    Ok(())
}
