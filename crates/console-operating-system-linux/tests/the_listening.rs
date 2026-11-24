//! A program woken by something only its interpreter can reach.
//!
//! Recovery waits on the pad and on nothing else: no timer and no event group.
//! To the loop that is a program with nothing left to wait for, and before
//! `Interpreter::listen` it was told `Stopping` on its first turn -- which is
//! why recovery wrote the loop again. What is asked here is that a program with
//! no timer and no event group hears what its interpreter was handed, in order,
//! and stops when the interpreter says the thing it was waiting on has gone.

use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};
use console_operating_system_linux::program_runtime::{
  Clock, Delivery, Interpreter, Subscribed, Tell, execute, run,
};
use console_program_contract::{Effect, EffectKind, Event, Exit};
use console_program_runtime::ExecutionError;

use std::io;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Pad;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Logged {
  Pressed(u32),
}

impl EffectDependencies for Logged {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Pressed(_) => &[],
    })
  }
}

impl Machine for Pad {
  type Input = Vec<u32>;
  type State = Pad;
  type Request = Event<u32>;
  type Effect = Effect<Logged>;

  fn initialize(
    input: &Vec<u32>,
    _previous: Option<Pad>,
    effects: &mut Queue<Effect<Logged>>,
  ) -> Result<Pad, Never> {
    let Ok(()) = effects.offer_all(
      input
        .iter()
        .map(|press| Effect::Custom(Logged::Pressed(*press))),
    );

    Ok(Pad)
  }

  fn handle(
    state: Pad,
    event: Event<u32>,
    effects: &mut Queue<Effect<Logged>>,
  ) -> Result<Pad, Never> {
    match event {
      Event::Custom(pressed) => {
        let Ok(()) = effects.offer(Effect::Custom(Logged::Pressed(pressed)));

        Ok(Pad)
      }
      Event::Opened
      | Event::Tick(_, _)
      | Event::Modified(_)
      | Event::Replied(_)
      | Event::Written(_)
      | Event::Chosen(_)
      | Event::Stopping => Ok(state),
    }
  }
}

struct Pressing(Vec<u32>);

impl Interpreter for Pressing {
  type Event = u32;
  type Effect = Logged;
  type Error = Never;

  fn interpret(&mut self, acts: &Logged, _clock: &Clock) -> Result<Vec<Event<u32>>, Never> {
    match acts {
      Logged::Pressed(pressed) => self.0.push(*pressed),
    }

    Ok(Vec::new())
  }

  fn listen(&mut self, tell: Tell<u32>) -> Result<Subscribed, console_core_never::Never> {
    let Ok(()) = console_program_lifetime::threads::let_go(std::thread::spawn(move || {
      assert_eq!(tell.tell(1), Ok(Delivery::Yes));
      assert_eq!(tell.tell(2), Ok(Delivery::Yes));
      assert_eq!(tell.end(Exit::Success), Ok(Delivery::Yes));
    }));

    Ok(Subscribed::Yes)
  }
}

#[test]
fn a_program_with_no_timer_and_no_event_group_hears_its_interpreter_until_it_says_stop() {
  let mut pressing = Pressing(Vec::new());
  let Ok(_code) = run::<Pad, Pressing>("pad", &Vec::new(), &mut pressing);

  assert_eq!(
    pressing.0,
    vec![1, 2],
    "the presses the interpreter was handed never reached the program"
  );
}

#[derive(Clone, Copy)]
enum FailAt {
  Start,
  Effect,
}

struct Failing {
  at: FailAt,
  effects: Vec<Logged>,
  cleanups: u32,
}

impl Interpreter for Failing {
  type Event = u32;
  type Effect = Logged;
  type Error = io::Error;

  fn listen(&mut self, _tell: Tell<u32>) -> Result<Subscribed, io::Error> {
    match self.at {
      FailAt::Start => Err(io::Error::other("listener failed")),
      FailAt::Effect => Ok(Subscribed::No),
    }
  }

  fn interpret(&mut self, effect: &Logged, _clock: &Clock) -> Result<Vec<Event<u32>>, io::Error> {
    self.effects.push(effect.clone());

    Err(io::Error::other("effect failed"))
  }

  fn finish(&mut self) -> Result<(), io::Error> {
    self.cleanups = self.cleanups.saturating_add(1);

    Err(io::Error::other("cleanup failed"))
  }
}

#[test]
fn the_linux_runtime_keeps_interpreter_errors_and_finalizes_once() -> io::Result<()> {
  for at in [FailAt::Start, FailAt::Effect] {
    let mut interpreter = Failing {
      at,
      effects: Vec::new(),
      cleanups: 0,
    };
    let result = execute::<Pad, _>("pad", &vec![1, 2], &mut interpreter);
    let (error, failure) = match result {
      Err(ExecutionError::Cleanup {
        error,
        outcome: Err(failure),
      }) => (error, failure),
      Ok(output) => {
        return Err(io::Error::other(format!(
          "expected two failures, received success {output:?}"
        )));
      }
      Err(fault) => {
        return Err(io::Error::other(format!(
          "expected two failures, received {fault:?}"
        )));
      }
    };
    assert_eq!(interpreter.cleanups, 1);
    assert_eq!(error.to_string(), "cleanup failed");

    match at {
      FailAt::Start => {
        assert!(interpreter.effects.is_empty());
        assert!(
          matches!(failure.as_ref(), ExecutionError::Start(start) if start.to_string() == "listener failed")
        );
      }
      FailAt::Effect => {
        assert_eq!(interpreter.effects, [Logged::Pressed(1)]);
        assert!(
          matches!(failure.as_ref(), ExecutionError::Perform { effect: EffectKind::Custom, error: perform } if perform.to_string() == "effect failed")
        );
      }
    }
  }

  Ok(())
}
