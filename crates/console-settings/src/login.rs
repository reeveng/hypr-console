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
            Event::Custom(LoginPatternEvent::Touched(touch)) => on_touch(drawing, *touch),
            Event::Opened | Event::Changed(_) | Event::Tick(..) | Event::Replied(_) | Event::Chosen(_) | Event::Stopping => {
                Update::none(drawing.clone())
            }
        };

        update
    }
}

fn with_pattern(drawing: &Drawing, pattern: Pattern) -> Result<Update<Drawing, LoginPatternEffect>, Never> {
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

        with_pattern(drawing, pattern)
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

                with_pattern(drawing, undone)
            }
        },
        ButtonPress::Choose => {
            let Ok(clicked) = pattern.click();

            on_click(drawing, clicked)
        }
    }
}

fn on_touch(drawing: &Drawing, touch: Touch) -> Result<Update<Drawing, LoginPatternEffect>, Never> {
    let pattern = &drawing.greeting.pattern;

    match (touch, pattern.finger, pattern.path.is_empty()) {
        (Touch::Up, Some(_), true) => Update::new(drawing.clone(), vec![Effect::Stop(Exit::Success)]),
        (Touch::Up, Some(_), false) | (Touch::Up, None, true | false) | (Touch::Down(_) | Touch::Moved(_), Some(_) | None, true | false) => {
            let Ok(clicked) = pattern.touch(touch);

            on_click(drawing, clicked)
        }
    }
}

fn on_click(drawing: &Drawing, clicked: Clicked) -> Result<Update<Drawing, LoginPatternEffect>, Never> {
    match clicked {
        Clicked::Traced(next) => with_pattern(drawing, next),
        Clicked::Submitted(secret) => submitted(drawing, secret),
    }
}

fn submitted(drawing: &Drawing, secret: Secret) -> Result<Update<Drawing, LoginPatternEffect>, Never> {
    let Ok(letters) = secret.as_str();
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

    fn presses(presses: &[ButtonPress]) -> Result<Vec<Event<LoginPatternEvent>>, Never> {
        Ok(presses.iter().map(|press| Event::Custom(LoginPatternEvent::Pressed(*press))).collect())
    }

    const TWICE: [u32; 4] = [0, 1, 2, 3];

    const NOT_A_SECRET: &str = "dots drawn and Login pressed are a secret";

    #[test]
    fn one_drawing_asks_for_a_second() -> Result<(), &'static str> {
        let drawn = Drawing {
            step: Step::Choosing,
            greeting: Greeting { pattern: Pattern { at: Target::Login, path: vec![0, 1, 2, 3], finger: None }, status: Status::Waiting },
        };
        let Ok(chosen) = presses(&[ButtonPress::Choose]);
        let Ok(trace) = run_from::<LoginPattern>(&drawn, &chosen);
        let Ok(effects) = trace.effects();
        let Ok(kept) = secret(TWICE.to_vec());
        let kept = kept.ok_or(NOT_A_SECRET)?;

        assert_eq!(trace.state.step, Step::Confirming(kept));
        assert_eq!(trace.state.greeting.status, Status::Message(AGAIN.to_string()));
        assert!(trace.state.greeting.pattern.path.is_empty());
        assert!(effects.is_empty());

        Ok(())
    }

    fn secret(path: Vec<u32>) -> Result<Option<Secret>, Never> {
        Ok(match (Pattern { at: Target::Login, path, finger: None }).click() {
            Ok(Clicked::Submitted(secret)) => Some(secret),
            Ok(Clicked::Traced(_)) => None,
        })
    }

    fn confirming(first: Vec<u32>, again: Vec<u32>) -> Result<Option<Drawing>, Never> {
        let Ok(first) = secret(first);

        Ok(first.map(|first| Drawing {
            step: Step::Confirming(first),
            greeting: Greeting { pattern: Pattern { at: Target::Login, path: again, finger: None }, status: Status::Waiting },
        }))
    }

    #[test]
    fn the_same_drawing_twice_is_kept() -> Result<(), &'static str> {
        let Ok(drawn) = confirming(TWICE.to_vec(), TWICE.to_vec());
        let drawn = drawn.ok_or(NOT_A_SECRET)?;
        let Ok(chosen) = presses(&[ButtonPress::Choose]);
        let Ok(trace) = run_from::<LoginPattern>(&drawn, &chosen);
        let Ok(effects) = trace.effects();
        let Ok(kept) = secret(TWICE.to_vec());
        let kept = kept.ok_or(NOT_A_SECRET)?;

        assert_eq!(effects, vec![Effect::Custom(LoginPatternEffect::Save(kept)), Effect::Stop(Exit::Success)]);

        Ok(())
    }

    #[test]
    fn a_second_drawing_that_differs_starts_again() -> Result<(), &'static str> {
        let Ok(drawn) = confirming(TWICE.to_vec(), vec![3, 2, 1, 0]);
        let drawn = drawn.ok_or(NOT_A_SECRET)?;
        let Ok(chosen) = presses(&[ButtonPress::Choose]);
        let Ok(trace) = run_from::<LoginPattern>(&drawn, &chosen);
        let Ok(effects) = trace.effects();

        assert_eq!(trace.state.step, Step::Choosing);
        assert_eq!(trace.state.greeting.status, Status::Message(APART.to_string()));
        assert!(effects.is_empty());

        Ok(())
    }

    #[test]
    fn fewer_than_four_dots_is_not_a_pattern() {
        let short = Drawing {
            step: Step::Choosing,
            greeting: Greeting { pattern: Pattern { at: Target::Login, path: vec![0, 1, 2], finger: None }, status: Status::Waiting },
        };
        let Ok(chosen) = presses(&[ButtonPress::Choose]);
        let Ok(trace) = run_from::<LoginPattern>(&short, &chosen);

        assert_eq!(trace.state.step, Step::Choosing);
        assert_eq!(trace.state.greeting.status, Status::Message(SHORT.to_string()));
        assert!(trace.state.greeting.pattern.path.is_empty());
    }

    #[test]
    fn a_finger_drawn_twice_the_same_way_is_kept() -> Result<(), &'static str> {
        let over = |at: u32| match console_login_pattern::dot(at) {
            Ok(Some(dot)) => Some(dot.centre),
            Ok(None) => None,
        };
        let (first, second, third, fourth) = match (over(0), over(4), over(2), over(7)) {
            (Some(first), Some(second), Some(third), Some(fourth)) => (first, second, third, fourth),
            (None, _, _, _) | (_, None, _, _) | (_, _, None, _) | (_, _, _, None) => return Err("a dot the pattern does not have"),
        };
        let stroke = [Touch::Down(first), Touch::Moved(second), Touch::Moved(third), Touch::Moved(fourth), Touch::Up];
        let events: Vec<Event<LoginPatternEvent>> =
            stroke.iter().chain(stroke.iter()).map(|touch| Event::Custom(LoginPatternEvent::Touched(*touch))).collect();
        let Ok(arguments) = Arguments::of(&[]);
        let Ok(trace) = run::<LoginPattern>(&arguments, &events);
        let Ok(effects) = trace.effects();
        let Ok(kept) = secret(vec![0, 4, 2, 7]);
        let kept = kept.ok_or(NOT_A_SECRET)?;

        assert_eq!(effects, vec![Effect::Custom(LoginPatternEffect::Save(kept)), Effect::Stop(Exit::Success)]);

        Ok(())
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
    fn a_tap_away_while_confirming_leaves_too() -> Result<(), &'static str> {
        let Ok(drawn) = confirming(TWICE.to_vec(), vec![]);
        let drawn = drawn.ok_or(NOT_A_SECRET)?;
        let tap = [Touch::Down(Point { x: 220, y: 180 }), Touch::Up].map(|touch| Event::Custom(LoginPatternEvent::Touched(touch)));
        let Ok(trace) = run_from::<LoginPattern>(&drawn, &tap);
        let Ok(effects) = trace.effects();

        assert_eq!(effects, vec![Effect::Stop(Exit::Success)]);

        Ok(())
    }

    #[test]
    fn b_with_nothing_drawn_leaves_without_keeping() {
        let Ok(arguments) = Arguments::of(&[]);
        let Ok(back) = presses(&[ButtonPress::Back]);
        let Ok(trace) = run::<LoginPattern>(&arguments, &back);
        let Ok(effects) = trace.effects();

        assert_eq!(effects, vec![Effect::Stop(Exit::Success)]);
    }
}
