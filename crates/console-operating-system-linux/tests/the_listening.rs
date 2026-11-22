//! A program woken by something only its interpreter can reach.
//!
//! Recovery waits on the pad and on nothing else: no timer and no event group.
//! To the loop that is a program with nothing left to wait for, and before
//! `Interpreter::listen` it was told `Stopping` on its first turn -- which is
//! why recovery wrote the loop again. What is asked here is that a program with
//! no timer and no event group hears what its interpreter was handed, in order,
//! and stops when the interpreter says the thing it was waiting on has gone.

use console_core_state_machine::{Machine, Never, Queue};
use console_operating_system_linux::program_runtime::{
  Clock, Delivery, Interpreter, Subscribed, Tell, run,
};
use console_program_contract::{Effect, Event, Exit};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Pad;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Logged {
  Pressed(u32),
}

impl Machine for Pad {
  type Input = ();
  type State = Pad;
  type Request = Event<u32>;
  type Effect = Effect<Logged>;

  fn initialize(
    _input: &(),
    _previous: Option<Pad>,
    _effects: &mut Queue<Effect<Logged>>,
  ) -> Result<Pad, Never> {
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

  fn interpret(&mut self, acts: &Logged, _clock: &Clock) -> Vec<Event<u32>> {
    match acts {
      Logged::Pressed(pressed) => self.0.push(*pressed),
    }

    Vec::new()
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
  let Ok(_code) = run::<Pad, Pressing>("pad", &(), &mut pressing);

  assert_eq!(
    pressing.0,
    vec![1, 2],
    "the presses the interpreter was handed never reached the program"
  );
}
