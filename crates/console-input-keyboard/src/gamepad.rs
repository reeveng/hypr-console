//! What the controller asked the keyboard to do.  The on-screen keyboard reads
//! the controller itself while it is up, and that is a claim rather than an
//! arrangement: `console_input_focus` takes the pad and the keyboard
//! InputPlumber publishes beside it for exactly as long as the layer surface is
//! on the screen, with `EVIOCGRAB` underneath, so nothing else receives a press
//! meanwhile. Two readers of one device would both act on the right stick,
//! which navigates and scrolls at once and flickers.  It used to be a profile
//! instead -- `keyboard.yaml`, which translated nothing, loaded when the
//! keyboard came up and taken off when it went. That worked and cost more than
//! it bought: a profile load destroys the pad and builds another, which is the
//! flake where X stopped showing the keyboard until the next reboot, and the
//! `After=` on the unit that was put there to survive it. The claim is the same
//! promise made where it can be kept.  Nothing here opens a device, in the same
//! way and for the same reason as `console_input_controller`: what arrives is
//! handed in and what to do about it is handed back, so every decision can be
//! asked of it twice and answered the same way. The binary is the only part
//! that touches the kernel.  Which button is which is nobody's to say here.
//! `console_input_focus` names a press in the profile's own words and
//! `console_input_gamepad::vocabulary` turns that into the word a person uses,
//! which is what this table is written in. The trap that vocabulary exists for
//! is live in this file: the button labelled X on this device is `BTN_NORTH` --
//! `North` -- and the one labelled Y is `BTN_WEST`, and X is the button that
//! puts the keyboard on the screen.

use std::time::{Duration, Instant};

use console_input_gamepad::vocabulary::spoken_for;
use console_input_focus::{Said, Spans, Went};
use console_core_never::Never;
use evdev::AbsoluteAxisCode;

pub const BEFORE_REPEAT: Duration = Duration::from_millis(350);
pub const BETWEEN_REPEATS: Duration = Duration::from_millis(90);

const DEADZONE: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    Up,
    Down,
    Left,
    Right,
    Press,
    Backspace,
    Enter,
    Shift,
    Toggle,
    PreviousLanguage,
    NextLanguage,
}

impl Asked {
    pub fn direction(self) -> Result<Option<(i32, i32)>, Never> {
        Ok(match self {
            Asked::Up => Some((0, -1)),
            Asked::Down => Some((0, 1)),
            Asked::Left => Some((-1, 0)),
            Asked::Right => Some((1, 0)),
            Asked::Press
            | Asked::Backspace
            | Asked::Enter
            | Asked::Shift
            | Asked::Toggle
            | Asked::PreviousLanguage
            | Asked::NextLanguage => None,
        })
    }

    pub fn repeats(self) -> Result<Repeats, Never> {
        let Ok(direction) = self.direction();

        Ok(match direction {
            Some(_) => Repeats::Held,
            None => Repeats::Once,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeats {
    Held,
    Once,
}

const BUTTONS: [(&str, Asked); 13] = [
    ("a", Asked::Press),
    ("b", Asked::Backspace),
    ("x", Asked::Toggle),
    ("y", Asked::Shift),
    ("menu", Asked::Enter),
    ("l1", Asked::PreviousLanguage),
    ("r1", Asked::NextLanguage),
    ("l3", Asked::Press),
    ("r3", Asked::Press),
    ("dpad-up", Asked::Up),
    ("dpad-down", Asked::Down),
    ("dpad-left", Asked::Left),
    ("dpad-right", Asked::Right),
];

pub fn wanted(said: Said, axis: Option<(AbsoluteAxisCode, i32)>, spans: &Spans) -> Result<Option<Asked>, Never> {
    match said {
        Said::Pressed { button, went } => pressed(button, went),
        Said::Nothing => moved(axis, spans),
        Said::Trigger { trigger: _, went: _ } | Said::Unnamed { code: _, went: _ } => Ok(None),
    }
}

fn pressed(button: &str, went: Went) -> Result<Option<Asked>, Never> {
    Ok(match went {
        Went::Up => None,
        Went::Down => {
            let Ok(spoken) = spoken_for(button);

            BUTTONS.iter().find(|(named, _)| *named == spoken).map(|(_, asked)| *asked)
        }
    })
}

fn moved(axis: Option<(AbsoluteAxisCode, i32)>, spans: &Spans) -> Result<Option<Asked>, Never> {
    let Some((axis, value)) = axis else { return Ok(None) };

    let Some((_, range)) = spans.iter().find(|(named, _)| *named == axis) else { return Ok(None) };

    from_stick(axis, value, *range)
}

pub fn from_stick(axis: AbsoluteAxisCode, value: i32, range: (i32, i32)) -> Result<Option<Asked>, Never> {
    let (low, high) = range;
    let span = match high > low {
        true => f64::from(high.saturating_sub(low)) / 2.0,
        false => 1.0,
    };
    let pushed = (f64::from(value) - (f64::from(low) + span)) / span;

    match pushed.abs() < DEADZONE {
        true => return Ok(None),
        false => {},
    }

    Ok(match axis {
        AbsoluteAxisCode::ABS_X | AbsoluteAxisCode::ABS_RX => match pushed < 0.0 {
            true => Some(Asked::Left),
            false => Some(Asked::Right),
        },
        AbsoluteAxisCode::ABS_Y | AbsoluteAxisCode::ABS_RY => match pushed < 0.0 {
            true => Some(Asked::Up),
            false => Some(Asked::Down),
        },
        _ => None,
    })
}

#[derive(Debug, Default)]
pub struct Held {
    what: Option<Asked>,
    due: Option<Instant>,
}

impl Held {
    pub fn went(&mut self, asked: Option<Asked>, now: Instant) -> Result<Option<Asked>, Never> {
        Ok(match asked {
            None => {
                self.what = None;
                self.due = None;
                None
            },
            Some(asked) if asked.repeats() == Ok(Repeats::Once) => Some(asked),
            Some(asked) if self.what == Some(asked) => None,
            Some(asked) => {
                self.what = Some(asked);
                self.due = Some(now + BEFORE_REPEAT);
                Some(asked)
            },
        })
    }

    pub fn until(&self, now: Instant) -> Result<Option<Duration>, Never> {
        Ok(self.due.map(|due| due.saturating_duration_since(now)))
    }

    pub fn due(&mut self, now: Instant) -> Result<Option<Asked>, Never> {
        let Some(due) = self.due else { return Ok(None) };

        match now < due {
            true => return Ok(None),
            false => {},
        }

        self.due = Some(now + BETWEEN_REPEATS);
        Ok(self.what)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_input_gamepad::vocabulary::BUTTONS as EVERY;

    fn down(button: &'static str) -> Said {
        Said::Pressed { button, went: Went::Down }
    }

    fn asked(said: Said) -> Option<Asked> {
        wanted(said, None, &Spans::new())
    }

    fn wanted(said: Said, axis: Option<(AbsoluteAxisCode, i32)>, spans: &Spans) -> Option<Asked> {
        let Ok(wanted) = super::wanted(said, axis, spans);

        wanted
    }

    #[test]
    fn the_button_labelled_x_is_the_one_that_raises_the_keyboard() {
        assert_eq!(asked(down("North")), Some(Asked::Toggle));
        assert_eq!(asked(down("West")), Some(Asked::Shift));
        assert_eq!(asked(down("South")), Some(Asked::Press));
        assert_eq!(asked(down("East")), Some(Asked::Backspace));
    }

    #[test]
    fn a_button_asks_once_and_on_the_way_down() {
        assert_eq!(asked(Said::Pressed { button: "South", went: Went::Up }), None);
    }

    #[test]
    fn a_button_this_keyboard_does_nothing_with_asks_for_nothing() {
        assert_eq!(asked(down("Select")), None);
        assert_eq!(asked(Said::Unnamed { code: 999, went: Went::Down }), None);
        assert_eq!(asked(Said::Trigger { trigger: "LeftTrigger", went: Went::Down }), None);
    }

    #[test]
    fn the_dpad_arrives_named_rather_than_as_a_hat_to_be_read_here() {
        assert_eq!(asked(down("DPadUp")), Some(Asked::Up));
        assert_eq!(asked(down("DPadRight")), Some(Asked::Right));
    }

    #[test]
    fn letting_the_dpad_go_asks_for_nothing_at_all() {
        let spans: Spans = vec![(AbsoluteAxisCode::ABS_HAT0X, (-1, 1))];
        assert_eq!(wanted(Said::Nothing, Some((AbsoluteAxisCode::ABS_HAT0X, 0)), &spans), None);
    }

    #[test]
    fn every_button_this_table_names_is_one_the_vocabulary_carries() {
        for (name, _) in BUTTONS {
            assert!(
                EVERY.iter().any(|(spoken, _)| *spoken == name),
                "{name} is not a button `vocabulary` knows"
            );
        }
    }

    #[test]
    fn a_stick_near_the_middle_asks_for_nothing() {
        let spans: Spans =
            vec![(AbsoluteAxisCode::ABS_X, (0, 255)), (AbsoluteAxisCode::ABS_RY, (0, 255))];
        let pushed = |axis, value| wanted(Said::Nothing, Some((axis, value)), &spans);
        assert_eq!(pushed(AbsoluteAxisCode::ABS_X, 128), None);
        assert_eq!(pushed(AbsoluteAxisCode::ABS_X, 140), None);
        assert_eq!(pushed(AbsoluteAxisCode::ABS_X, 255), Some(Asked::Right));
        assert_eq!(pushed(AbsoluteAxisCode::ABS_X, 0), Some(Asked::Left));
        assert_eq!(pushed(AbsoluteAxisCode::ABS_RY, 0), Some(Asked::Up));
    }

    #[test]
    fn a_stick_the_device_said_nothing_about_moves_nothing() {
        let nothing = Spans::new();
        assert_eq!(wanted(Said::Nothing, Some((AbsoluteAxisCode::ABS_X, 255)), &nothing), None);
        assert_eq!(wanted(Said::Nothing, None, &nothing), None);
    }

    #[test]
    fn a_direction_already_held_does_not_ask_again() {
        let now = Instant::now();
        let mut held = Held::default();
        assert_eq!(held.went(Some(Asked::Left), now), Ok(Some(Asked::Left)));
        assert_eq!(held.went(Some(Asked::Left), now), Ok(None));
        assert_eq!(held.went(Some(Asked::Right), now), Ok(Some(Asked::Right)), "a turn is a new ask");
    }

    #[test]
    fn a_held_direction_waits_then_repeats() {
        let start = Instant::now();
        let mut held = Held::default();
        let Ok(first) = held.went(Some(Asked::Down), start);

        assert_eq!(first, Some(Asked::Down), "the press itself");
        assert_eq!(held.due(start), Ok(None), "not yet");
        assert_eq!(held.due(start + BEFORE_REPEAT - Duration::from_millis(1)), Ok(None));
        assert_eq!(held.due(start + BEFORE_REPEAT), Ok(Some(Asked::Down)), "the first repeat");
        let then = start + BEFORE_REPEAT;
        assert_eq!(held.due(then), Ok(None), "and not again immediately");
        assert_eq!(held.due(then + BETWEEN_REPEATS), Ok(Some(Asked::Down)));
    }

    #[test]
    fn letting_go_stops_the_repeat() {
        let now = Instant::now();
        let mut held = Held::default();
        let Ok(first) = held.went(Some(Asked::Up), now);
        let Ok(waking) = held.until(now);

        assert_eq!(first, Some(Asked::Up), "the press itself");
        assert!(waking.is_some(), "something to wake for");
        assert_eq!(held.went(None, now), Ok(None));
        assert_eq!(held.due(now + BEFORE_REPEAT * 4), Ok(None));
        assert_eq!(held.until(now), Ok(None), "and nothing to wake for");
    }

    #[test]
    fn a_press_is_not_a_thing_that_repeats() {
        let now = Instant::now();
        let mut held = Held::default();
        assert_eq!(held.went(Some(Asked::Press), now), Ok(Some(Asked::Press)));
        assert_eq!(held.went(Some(Asked::Press), now), Ok(Some(Asked::Press)), "still not a repeat");
        assert_eq!(held.until(now), Ok(None));
    }
}
