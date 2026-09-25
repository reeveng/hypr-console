//! What the greeter decides: a press moves the cursor or draws the pattern, a
//! finger draws it across the dots, Login or a lifted finger hands the letters
//! to the login window, and what the window answers is what the screen says
//! next.
//!
//! While a pattern is being checked a press or a touch does nothing, so a
//! second Login cannot race the first. A refusal clears the pattern and keeps the cursor
//! where it was, because the hand that drew it is about to draw it again.

use console_core_never::Never;
use console_input_event_devices::presses::ButtonPress;
use console_login_pattern::{Clicked, Direction, Pattern, Touch};
use console_login_window::protocol::{FromGreeter, ToGreeter};
use console_program_contract::{Arguments, Effect, Event, Exit, Flag, Initial, Program, Update};

pub const FELL: &str = "--fell";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Waiting,
    Fell,
    Checking,
    Failed(String),
    Message(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Greeting {
    pub pattern: Pattern,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GreeterEvent {
    Pressed(ButtonPress),
    Touched(Touch),
    Received(ToGreeter),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GreeterEffect {
    Send(FromGreeter),
}

pub struct Greeter;

impl Program for Greeter {
    type State = Greeting;
    type Event = GreeterEvent;
    type Effect = GreeterEffect;

    fn init(arguments: &Arguments) -> Initial<Greeting> {
        let Ok(pattern) = Pattern::new();
        let Ok(fell) = arguments.given(FELL);
        let status = match fell {
            Flag::Present => Status::Fell,
            Flag::Absent => Status::Waiting,
        };
        let Ok(opening) = Initial::new(Greeting { pattern, status });

        opening
    }

    fn update(greeting: &Greeting, event: &Event<GreeterEvent>) -> Update<Greeting, GreeterEffect> {
        let Ok(update) = match (&greeting.status, event) {
            (_, Event::Custom(GreeterEvent::Received(received))) => answered(greeting, received),
            (Status::Checking, Event::Custom(GreeterEvent::Pressed(_) | GreeterEvent::Touched(_))) => Update::none(greeting.clone()),
            (
                Status::Waiting | Status::Fell | Status::Failed(_) | Status::Message(_),
                Event::Custom(GreeterEvent::Pressed(press)),
            ) => pressed(greeting, *press),
            (
                Status::Waiting | Status::Fell | Status::Failed(_) | Status::Message(_),
                Event::Custom(GreeterEvent::Touched(touch)),
            ) => {
                let Ok(clicked) = greeting.pattern.touched(*touch);

                chosen(greeting, clicked)
            }
            (_, Event::Opened | Event::Changed(_) | Event::Tick(..) | Event::Replied(_) | Event::Chosen(_) | Event::Stopping) => {
                Update::none(greeting.clone())
            }
        };

        update
    }
}

fn pressed(greeting: &Greeting, press: ButtonPress) -> Result<Update<Greeting, GreeterEffect>, Never> {
    let moved = |direction| {
        let Ok(pattern) = greeting.pattern.moved(direction);

        Update::none(Greeting { pattern, status: greeting.status.clone() })
    };

    match press {
        ButtonPress::Up => moved(Direction::Up),
        ButtonPress::Down => moved(Direction::Down),
        ButtonPress::Left => moved(Direction::Left),
        ButtonPress::Right => moved(Direction::Right),
        ButtonPress::Back => {
            let Ok(pattern) = greeting.pattern.undone();

            Update::none(Greeting { pattern, status: greeting.status.clone() })
        }
        ButtonPress::Choose => {
            let Ok(clicked) = greeting.pattern.clicked();

            chosen(greeting, clicked)
        }
    }
}

fn chosen(greeting: &Greeting, clicked: Clicked) -> Result<Update<Greeting, GreeterEffect>, Never> {
    match clicked {
        Clicked::Traced(pattern) => Update::none(Greeting { pattern, status: greeting.status.clone() }),
        Clicked::Submitted(secret) => {
            let Ok(spelled) = secret.spelled();
            let login = FromGreeter::Login(spelled.to_string());
            let Ok(pattern) = greeting.pattern.lifted();

            Update::new(Greeting { pattern, status: Status::Checking }, vec![Effect::Custom(GreeterEffect::Send(login))])
        }
    }
}

fn answered(greeting: &Greeting, received: &ToGreeter) -> Result<Update<Greeting, GreeterEffect>, Never> {
    let Ok(cleared) = greeting.pattern.cleared();

    match received {
        ToGreeter::Welcome => Update::new(greeting.clone(), vec![Effect::Stop(Exit::Success)]),
        ToGreeter::Failed(why) => Update::none(Greeting { pattern: cleared, status: Status::Failed(why.clone()) }),
        ToGreeter::Message(text) => Update::none(Greeting { pattern: greeting.pattern.clone(), status: Status::Message(text.clone()) }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_login_pattern::{DOTS, Target};
    use console_program_contract::run;

    fn arguments(words: &[&str]) -> Arguments {
        let Ok(arguments) = Arguments::of(words);

        arguments
    }

    fn pressed(presses: &[ButtonPress]) -> Vec<Event<GreeterEvent>> {
        presses.iter().map(|press| Event::Custom(GreeterEvent::Pressed(*press))).collect()
    }

    #[test]
    fn a_pattern_ending_at_login_is_handed_to_the_window_as_its_letters() {
        let events = pressed(&[
            ButtonPress::Choose,
            ButtonPress::Right,
            ButtonPress::Choose,
            ButtonPress::Down,
            ButtonPress::Down,
            ButtonPress::Down,
            ButtonPress::Down,
            ButtonPress::Choose,
        ]);
        let Ok(trace) = run::<Greeter>(&arguments(&[]), &events);
        let Ok(effects) = trace.effects();

        assert_eq!(trace.state.status, Status::Checking);
        assert_eq!(effects, vec![Effect::Custom(GreeterEffect::Send(FromGreeter::Login("ab".to_string())))]);
    }

    #[test]
    fn a_finger_drawn_over_the_dots_and_lifted_is_handed_to_the_window() {
        let over = |key: char| match DOTS.iter().find(|dot| dot.key == key) {
            Some(dot) => dot.centre,
            None => panic!("no dot for {key}"),
        };
        let touches = [Touch::Down(over('a')), Touch::Moved(over('e')), Touch::Moved(over('c')), Touch::Up];
        let events: Vec<Event<GreeterEvent>> = touches.iter().map(|touch| Event::Custom(GreeterEvent::Touched(*touch))).collect();
        let Ok(trace) = run::<Greeter>(&arguments(&[]), &events);
        let Ok(effects) = trace.effects();

        assert_eq!(trace.state.status, Status::Checking);
        assert_eq!(trace.state.pattern.finger, None);
        assert_eq!(effects, vec![Effect::Custom(GreeterEffect::Send(FromGreeter::Login("aec".to_string())))]);
    }

    #[test]
    fn a_press_while_checking_does_nothing() {
        let Ok(start) = Pattern::new();
        let checking = Greeting { pattern: start.clone(), status: Status::Checking };
        let Ok(trace) = console_program_contract::run_from::<Greeter>(&checking, &pressed(&[ButtonPress::Up, ButtonPress::Choose]));

        assert_eq!(trace.state, checking);
    }

    #[test]
    fn a_refusal_clears_the_pattern_and_keeps_the_cursor() {
        let drawn = Greeting { pattern: Pattern { at: Target::Login, path: vec![1, 2], finger: None }, status: Status::Checking };
        let events = [Event::Custom(GreeterEvent::Received(ToGreeter::Failed("no".to_string())))];
        let Ok(trace) = console_program_contract::run_from::<Greeter>(&drawn, &events);

        assert_eq!(trace.state.pattern, Pattern { at: Target::Login, path: Vec::new(), finger: None });
        assert_eq!(trace.state.status, Status::Failed("no".to_string()));
    }

    #[test]
    fn a_welcome_stops_the_greeter() {
        let Ok(start) = Pattern::new();
        let checking = Greeting { pattern: start, status: Status::Checking };
        let Ok(trace) = console_program_contract::run_from::<Greeter>(&checking, &[Event::Custom(GreeterEvent::Received(ToGreeter::Welcome))]);
        let Ok(effects) = trace.effects();

        assert_eq!(effects, vec![Effect::Stop(Exit::Success)]);
    }

    #[test]
    fn a_desktop_that_fell_is_said_on_the_first_screen() {
        let Ok(trace) = run::<Greeter>(&arguments(&[FELL]), &[]);

        assert_eq!(trace.state.status, Status::Fell);
    }
}
