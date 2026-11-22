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
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::mpsc::sync_channel;
use std::time::Duration;

use console_core_never::Never;
use console_core_state_machine::{Machine, Queue};
use console_events::serving;
use console_operating_system_linux::program_runtime::{Clock, Interpreter, run};
use console_program_contract::event::Change;
use console_program_contract::{Effect, Event, EventGroup, Exit, Subscription, Timer};
use console_program_lifetime::threads::let_go;
use console_waiting::{Outcome, Ready, Schedule, until};

const BEFORE_LONG: Duration = Duration::from_secs(5);

const GIVING_UP: Timer = Timer {
  name: "giving up",
  interval: Duration::from_secs(20),
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rocker {
  said: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestEffect {
  Logged(String),
}

impl Machine for Rocker {
  type Input = ();
  type State = Rocker;
  type Request = Event<Never>;
  type Effect = Effect<TestEffect>;

  fn initialize(
    _input: &(),
    _previous: Option<Rocker>,
    effects: &mut Queue<Effect<TestEffect>>,
  ) -> Result<Rocker, Never> {
    #[cfg_attr(
      dylint_lib = "explicit043_no_unmatched_listen",
      allow(
        explicit043_no_unmatched_listen,
        reason = "this program exists to hear one change on the event group, and hearing it or giving up stops it, so it listens exactly as long as it runs"
      )
    )]
    let Ok(()) = effects.offer_all([
      Effect::Subscribe(Subscription::EventGroup(EventGroup::Sound)),
      Effect::Subscribe(Subscription::Timer(GIVING_UP)),
    ]);

    Ok(Rocker { said: None })
  }

  fn handle(
    state: Rocker,
    event: Event<Never>,
    effects: &mut Queue<Effect<TestEffect>>,
  ) -> Result<Rocker, Never> {
    match event {
      Event::Modified(changed) => {
        let Ok(()) = effects.offer_all([
          Effect::Custom(TestEffect::Logged(changed.text.clone())),
          Effect::Stop(Exit::Success),
        ]);

        Ok(Rocker {
          said: Some(changed.text),
        })
      }
      Event::Tick(_, _) => {
        let Ok(()) = effects.offer(Effect::Stop(Exit::Failure(
          "nothing was ever told".to_string(),
        )));

        Ok(state)
      }
      Event::Opened
      | Event::Replied(_)
      | Event::Written(_)
      | Event::Chosen(_)
      | Event::Stopping => Ok(state),
      Event::Custom(its) => match its {},
    }
  }
}

struct Keeping(Vec<String>);

impl Interpreter for Keeping {
  type Event = Never;
  type Effect = TestEffect;

  fn interpret(&mut self, acts: &TestEffect, _clock: &Clock) -> Vec<Event<Never>> {
    match acts {
      TestEffect::Logged(said) => self.0.push(said.clone()),
    }

    Vec::new()
  }
}

fn up(at: &Path) -> Result<Outcome, Never> {
  let Ok(patience) = Schedule::of(BEFORE_LONG);

  until(patience, || {
    Ok(match UnixStream::connect(at) {
      Ok(_ready) => Ready::Yes,
      Err(fault) => {
        console_core_logging::trace!("waiting on {}: {fault}", at.display());

        Ready::NotYet
      }
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
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "this binary has one integration test and assigns its private runtime directory before starting any threads"
  )
)]
fn a_program_that_asked_to_be_told_hears_the_pool_rather_than_a_line_on_the_journal()
-> Result<(), Box<dyn Error>> {
  let where_ = console_core_temporary_directories::fresh("runtime-words")?;

  std::fs::create_dir_all(where_.join("console"))?;

  // SAFETY: this binary has one test, and this runs before its server or runtime starts.
  unsafe { std::env::set_var("XDG_RUNTIME_DIR", &where_) };

  let at = where_.join("console").join("events.sock");
  let (handing, handed) = sync_channel(1);

  let serving = at.clone();
  let Ok(()) = let_go(std::thread::spawn(move || {
    let served = serving::serve(&serving, |event_group, say| {
      console_events::sources::handed_to(Some(&handing), &EventGroup::Sound, event_group, say)
    });

    match served {
      Ok(()) => {}
      Err(fault) => eprintln!("the test pool stopped: {fault}"),
    }
  }));

  assert_eq!(
    up(&at),
    Ok(Outcome::Happened),
    "the pool never opened its socket"
  );

  let (done, ended) = sync_channel(1);

  let Ok(()) = let_go(std::thread::spawn(move || {
    let mut carrying = Keeping(Vec::new());
    let Ok(code) = run::<Rocker, Keeping>("rocker", &(), &mut carrying);

    match done.send((code, carrying.0)) {
      Ok(()) => {}
      Err(_test_ended) => eprintln!("the test ended before rocker reported its events"),
    }
  }));

  let saying = handed.recv_timeout(BEFORE_LONG).map_err(|why| {
    format!(
      "the runtime never asked the pool for the event group the program said it wanted: {why}"
    )
  })?;

  saying
    .send(Change {
      event_group: EventGroup::Sound,
      text: "sink 1 at 40%".to_string(),
    })
    .map_err(|why| format!("the pool stopped listening to its own source: {why}"))?;

  let (code, held) = ended
    .recv_timeout(BEFORE_LONG)
    .map_err(|why| format!("the program was never told anything and never stopped: {why}"))?;

  assert_eq!(code, std::process::ExitCode::SUCCESS);
  assert_eq!(held, vec!["sink 1 at 40%".to_string()]);

  std::fs::remove_dir_all(&where_)?;
  Ok(())
}
