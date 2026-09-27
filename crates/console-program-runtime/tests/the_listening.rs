//! A program woken by something only its interpreter can reach.
//!
//! Recovery waits on the pad and on nothing else: no timer and no topic. To
//! the loop that is a program with nothing left to wait for, and before
//! `Interpreter::listen` it was told `Stopping` on its first turn -- which is
//! why recovery wrote the loop again. What is asked here is that a program
//! with no timer and no topic hears what its interpreter was handed, in order,
//! and stops when the interpreter says the thing it was waiting on has gone.

use std::sync::{Arc, Mutex};

use console_program_contract::{Arguments, Effect, Exit, Initial, Program, Update, Event};
use console_program_runtime::{Delivery, Interpreter, Subscribed, Tell, run};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Pad;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Logged {
    Pressed(u32),
}

impl Program for Pad {
    type State = Pad;
    type Event = u32;
    type Effect = Logged;

    fn init(_argv: &Arguments) -> Initial<Pad> {
        let Ok(opening) = Initial::new(Pad);

        opening
    }

    fn update(state: &Pad, event: &Event<u32>) -> Update<Pad, Logged> {
        let Ok(turn) = match event {
            Event::Custom(pressed) => Update::new(Pad, vec![Effect::Custom(Logged::Pressed(*pressed))]),
            Event::Opened
            | Event::Tick(_, _)
            | Event::Changed(_)
            | Event::Replied(_)
            | Event::Chosen(_)
            | Event::Stopping => Update::none(state.clone()),
        };

        turn
    }
}

struct Pressing(Arc<Mutex<Vec<u32>>>);

impl Interpreter for Pressing {
    type Event = u32;
    type Effect = Logged;

    fn interpret(&mut self, acts: &Logged) -> Vec<Event<u32>> {
        match acts {
            Logged::Pressed(pressed) => {
                let mut kept = match self.0.lock() {
                    Ok(kept) => kept,
                    Err(poisoned) => poisoned.into_inner(),
                };

                kept.push(*pressed);
            }
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
fn a_program_with_no_timer_and_no_topic_hears_its_interpreter_until_it_says_stop() {
    let kept = Arc::new(Mutex::new(Vec::new()));
    let Ok(arguments) = Arguments::of(&[]);
    let mut pressing = Pressing(Arc::clone(&kept));
    let Ok(_code) = run::<Pad, Pressing>("pad", &arguments, &mut pressing);

    let held = match kept.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    };

    assert_eq!(*held, vec![1, 2], "the presses the interpreter was handed never reached the program");
}
