//! Choosing the pattern the login window asks for.
//!
//! It is drawn on the greeter's own dots, so the hand that chooses it here is
//! the hand that draws it there, and it is drawn twice: once to choose and
//! once to say it was meant. A pattern kept from a single drawing is one slip
//! away from a desktop nobody can get back into, and the second drawing is the
//! confirmation the way a new password is typed twice.
//!
//! A pattern joins at least four dots, the least Android has ever accepted,
//! because a pattern of one dot is a button rather than a secret. A finger
//! lifted is Next or Save, as it is at login. B takes the last dot back, and
//! with no dots drawn it puts the screen away without keeping anything. A hand
//! with no pad has no B, so a tap that joins no dot is the same leaving: at
//! login there is nowhere else to go, and here there always is.

use console_core_never::Never;
use console_input_event_devices::presses::ButtonPress;
use console_login_greeter::greeting::{Greeting, Status};
use console_login_pattern::{Clicked, Direction, Pattern, Secret, Touch};
use console_program_contract::{Arguments, Effect, Event, Exit, Initial, Program, Update};

pub const FEWEST: u32 = 4;

pub const DRAW: &str = "Draw a new pattern, then Next. Tap away from the dots to cancel.";

pub const AGAIN: &str = "Draw it again to confirm, then Save.";

pub const APART: &str = "The two were not the same. Draw a new pattern.";

pub const SHORT: &str = "Join at least four dots.";

pub const NEXT: &str = "Next";

pub const SAVE: &str = "Save";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Choosing,
    Confirming(Secret),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drawing {
    pub step: Step,
    pub greeting: Greeting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginPatternEvent {
    Pressed(ButtonPress),
    Touched(Touch),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginPatternEffect {
    Save(Secret),
}

pub struct LoginPattern;

impl Drawing {
    pub fn button(&self) -> Result<&'static str, Never> {
        Ok(match self.step {
            Step::Choosing => NEXT,
            Step::Confirming(_) => SAVE,
        })
    }
}

impl Program for LoginPattern {
    type State = Drawing;
    type Event = LoginPatternEvent;
    type Effect = LoginPatternEffect;

    fn init(_arguments: &Arguments) -> Initial<Drawing> {
        let Ok(pattern) = Pattern::new();
        let greeting = Greeting { pattern, status: Status::Message(DRAW.to_string()) };
        let Ok(opening) = Initial::new(Drawing { step: Step::Choosing, greeting });

        opening
    }

    fn update(drawing: &Drawing, event: &Event<LoginPatternEvent>) -> Update<Drawing, LoginPatternEffect> {
        let Ok(update) = match event {
            Event::Custom(LoginPatternEvent::Pressed(press)) => pressed(drawing, *press),
            Event::Custom(LoginPatternEvent::Touched(touch)) => touched(drawing, *touch),
            Event::Opened | Event::Changed(_) | Event::Tick(..) | Event::Replied(_) | Event::Chosen(_) | Event::Stopping => {
                Update::none(drawing.clone())
            }
        };

        update
    }
}

fn drawn(drawing: &Drawing, pattern: Pattern) -> Result<Update<Drawing, LoginPatternEffect>, Never> {
    Update::none(Drawing { step: drawing.step.clone(), greeting: Greeting { pattern, status: drawing.greeting.status.clone() } })
}

fn cleared(drawing: &Drawing, step: Step, message: &str) -> Result<Update<Drawing, LoginPatternEffect>, Never> {
    let Ok(pattern) = drawing.greeting.pattern.cleared();

    Update::none(Drawing { step, greeting: Greeting { pattern, status: Status::Message(message.to_string()) } })
}

fn pressed(drawing: &Drawing, press: ButtonPress) -> Result<Update<Drawing, LoginPatternEffect>, Never> {
    let pattern = &drawing.greeting.pattern;
    let moved = |direction| {
        let Ok(pattern) = pattern.moved(direction);

        drawn(drawing, pattern)
    };

    match press {
        ButtonPress::Up => moved(Direction::Up),
        ButtonPress::Down => moved(Direction::Down),
        ButtonPress::Left => moved(Direction::Left),
        ButtonPress::Right => moved(Direction::Right),
        ButtonPress::Back => match pattern.path.is_empty() {
            true => Update::new(drawing.clone(), vec![Effect::Stop(Exit::Success)]),
            false => {
                let Ok(undone) = pattern.undone();

                drawn(drawing, undone)
            }
        },
        ButtonPress::Choose => {
            let Ok(clicked) = pattern.clicked();

            chosen(drawing, clicked)
        }
    }
}

fn touched(drawing: &Drawing, touch: Touch) -> Result<Update<Drawing, LoginPatternEffect>, Never> {
    let pattern = &drawing.greeting.pattern;

    match (touch, pattern.finger, pattern.path.is_empty()) {
        (Touch::Up, Some(_), true) => Update::new(drawing.clone(), vec![Effect::Stop(Exit::Success)]),
        (Touch::Up, Some(_), false) | (Touch::Up, None, true | false) | (Touch::Down(_) | Touch::Moved(_), Some(_) | None, true | false) => {
            let Ok(clicked) = pattern.touched(touch);

            chosen(drawing, clicked)
        }
    }
}

fn chosen(drawing: &Drawing, clicked: Clicked) -> Result<Update<Drawing, LoginPatternEffect>, Never> {
    match clicked {
        Clicked::Traced(next) => drawn(drawing, next),
        Clicked::Submitted(secret) => submitted(drawing, secret),
    }
}

fn submitted(drawing: &Drawing, secret: Secret) -> Result<Update<Drawing, LoginPatternEffect>, Never> {
    let Ok(letters) = secret.spelled();
    let Ok(fewest) = console_core_number_conversion::index(FEWEST);

    match (letters.chars().count() < fewest, &drawing.step) {
        (true, step) => cleared(drawing, step.clone(), SHORT),
        (false, Step::Choosing) => cleared(drawing, Step::Confirming(secret), AGAIN),
        (false, Step::Confirming(first)) => match *first == secret {
            true => Update::new(drawing.clone(), vec![Effect::Custom(LoginPatternEffect::Save(secret)), Effect::Stop(Exit::Success)]),
            false => cleared(drawing, Step::Choosing, APART),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_geometry::Point;
    use console_login_pattern::Target;
    use console_program_contract::{run, run_from};

    fn presses(presses: &[ButtonPress]) -> Vec<Event<LoginPatternEvent>> {
        presses.iter().map(|press| Event::Custom(LoginPatternEvent::Pressed(*press))).collect()
    }

    #[test]
    fn one_drawing_asks_for_a_second() {
        let drawn = Drawing {
            step: Step::Choosing,
            greeting: Greeting { pattern: Pattern { at: Target::Login, path: vec![0, 1, 2, 3], finger: None }, status: Status::Waiting },
        };
        let Ok(trace) = run_from::<LoginPattern>(&drawn, &presses(&[ButtonPress::Choose]));
        let Ok(effects) = trace.effects();

        assert_eq!(trace.state.step, Step::Confirming(secret(vec![0, 1, 2, 3])));
        assert_eq!(trace.state.greeting.status, Status::Message(AGAIN.to_string()));
        assert!(trace.state.greeting.pattern.path.is_empty());
        assert!(effects.is_empty());
    }

    fn secret(path: Vec<u32>) -> Secret {
        match (Pattern { at: Target::Login, path, finger: None }).clicked() {
            Ok(Clicked::Submitted(secret)) => secret,
            Ok(Clicked::Traced(_)) => panic!("dots drawn and Login pressed are a secret"),
        }
    }

    fn confirming(first: Vec<u32>, again: Vec<u32>) -> Drawing {
        Drawing {
            step: Step::Confirming(secret(first)),
            greeting: Greeting { pattern: Pattern { at: Target::Login, path: again, finger: None }, status: Status::Waiting },
        }
    }

    #[test]
    fn the_same_drawing_twice_is_kept() {
        let Ok(trace) = run_from::<LoginPattern>(&confirming(vec![0, 1, 2, 3], vec![0, 1, 2, 3]), &presses(&[ButtonPress::Choose]));
        let Ok(effects) = trace.effects();

        assert_eq!(effects, vec![Effect::Custom(LoginPatternEffect::Save(secret(vec![0, 1, 2, 3]))), Effect::Stop(Exit::Success)]);
    }

    #[test]
    fn a_second_drawing_that_differs_starts_again() {
        let Ok(trace) = run_from::<LoginPattern>(&confirming(vec![0, 1, 2, 3], vec![3, 2, 1, 0]), &presses(&[ButtonPress::Choose]));
        let Ok(effects) = trace.effects();

        assert_eq!(trace.state.step, Step::Choosing);
        assert_eq!(trace.state.greeting.status, Status::Message(APART.to_string()));
        assert!(effects.is_empty());
    }

    #[test]
    fn fewer_than_four_dots_is_not_a_pattern() {
        let short = Drawing {
            step: Step::Choosing,
            greeting: Greeting { pattern: Pattern { at: Target::Login, path: vec![0, 1, 2], finger: None }, status: Status::Waiting },
        };
        let Ok(trace) = run_from::<LoginPattern>(&short, &presses(&[ButtonPress::Choose]));

        assert_eq!(trace.state.step, Step::Choosing);
        assert_eq!(trace.state.greeting.status, Status::Message(SHORT.to_string()));
        assert!(trace.state.greeting.pattern.path.is_empty());
    }

    #[test]
    fn a_finger_drawn_twice_the_same_way_is_kept() {
        let over = |at: u32| match console_login_pattern::dot(at) {
            Ok(Some(dot)) => dot.centre,
            Ok(None) => panic!("no dot {at}"),
        };
        let stroke = [Touch::Down(over(0)), Touch::Moved(over(4)), Touch::Moved(over(2)), Touch::Moved(over(7)), Touch::Up];
        let events: Vec<Event<LoginPatternEvent>> =
            stroke.iter().chain(stroke.iter()).map(|touch| Event::Custom(LoginPatternEvent::Touched(*touch))).collect();
        let Ok(arguments) = Arguments::of(&[]);
        let Ok(trace) = run::<LoginPattern>(&arguments, &events);
        let Ok(effects) = trace.effects();

        assert_eq!(effects, vec![Effect::Custom(LoginPatternEffect::Save(secret(vec![0, 4, 2, 7]))), Effect::Stop(Exit::Success)]);
    }

    #[test]
    fn a_tap_away_from_the_dots_leaves_without_keeping() {
        let Ok(arguments) = Arguments::of(&[]);
        let away = Point { x: 220, y: 180 };
        let tap = [Touch::Down(away), Touch::Up].map(|touch| Event::Custom(LoginPatternEvent::Touched(touch)));
        let Ok(trace) = run::<LoginPattern>(&arguments, &tap);
        let Ok(effects) = trace.effects();

        assert_eq!(effects, vec![Effect::Stop(Exit::Success)]);
    }

    #[test]
    fn a_tap_away_while_confirming_leaves_too() {
        let Ok(trace) = run_from::<LoginPattern>(&confirming(vec![0, 1, 2, 3], vec![]), &[Touch::Down(Point { x: 220, y: 180 }), Touch::Up].map(|touch| Event::Custom(LoginPatternEvent::Touched(touch))));
        let Ok(effects) = trace.effects();

        assert_eq!(effects, vec![Effect::Stop(Exit::Success)]);
    }

    #[test]
    fn b_with_nothing_drawn_leaves_without_keeping() {
        let Ok(arguments) = Arguments::of(&[]);
        let Ok(trace) = run::<LoginPattern>(&arguments, &presses(&[ButtonPress::Back]));
        let Ok(effects) = trace.effects();

        assert_eq!(effects, vec![Effect::Stop(Exit::Success)]);
    }
}
