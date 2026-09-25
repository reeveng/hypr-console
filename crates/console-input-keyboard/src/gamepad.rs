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
//! that touches the kernel.  Which button is which is no one's to say here.
//! `console_input_focus` names a press in the profile's own words and
//! `console_input_gamepad::vocabulary` turns that into the word a person uses,
//! which is what this table is written in. The trap that vocabulary exists for
//! is live in this file: the button labelled X on this device is `BTN_NORTH` --
//! `North` -- and the one labelled Y is `BTN_WEST`, and X is the button that
//! puts the keyboard on the screen.

use std::time::{Duration, Instant};

use console_input_gamepad::vocabulary::spoken_for;
use console_input_focus::{InputEvent, Spans, Direction};
use console_core_never::Never;
use console_input_event_devices::AbsoluteAxisCode;

pub const BEFORE_REPEAT: Duration = Duration::from_millis(350);
pub const BETWEEN_REPEATS: Duration = Duration::from_millis(90);

const DEADZONE: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardCommand {
    Up,
    Down,
    Left,
    Right,
    Select,
    Backspace,
    Enter,
    Shift,
    Toggle,
    PreviousLanguage,
    NextLanguage,
}

impl KeyboardCommand {
    pub fn direction(self) -> Result<Option<(i32, i32)>, Never> {
        Ok(match self {
            KeyboardCommand::Up => Some((0, -1)),
            KeyboardCommand::Down => Some((0, 1)),
            KeyboardCommand::Left => Some((-1, 0)),
            KeyboardCommand::Right => Some((1, 0)),
            KeyboardCommand::Select
            | KeyboardCommand::Backspace
            | KeyboardCommand::Enter
            | KeyboardCommand::Shift
            | KeyboardCommand::Toggle
            | KeyboardCommand::PreviousLanguage
            | KeyboardCommand::NextLanguage => None,
        })
    }

    pub fn repeat_mode(self) -> Result<RepeatMode, Never> {
        let Ok(direction) = self.direction();

        Ok(match direction {
            Some(_) => RepeatMode::Repeating,
            None => RepeatMode::Once,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Repeating,
    Once,
}

const BUTTONS: [(&str, KeyboardCommand); 13] = [
    ("a", KeyboardCommand::Select),
    ("b", KeyboardCommand::Backspace),
    ("x", KeyboardCommand::Toggle),
    ("y", KeyboardCommand::Shift),
    ("menu", KeyboardCommand::Enter),
    ("l1", KeyboardCommand::PreviousLanguage),
    ("r1", KeyboardCommand::NextLanguage),
    ("l3", KeyboardCommand::Select),
    ("r3", KeyboardCommand::Select),
    ("dpad-up", KeyboardCommand::Up),
    ("dpad-down", KeyboardCommand::Down),
    ("dpad-left", KeyboardCommand::Left),
    ("dpad-right", KeyboardCommand::Right),
];

pub fn command_for(event: InputEvent, axis: Option<(AbsoluteAxisCode, i32)>, spans: &Spans) -> Result<Option<KeyboardCommand>, Never> {
    match event {
        InputEvent::Pressed { button, direction } => pressed(button, direction),
        InputEvent::None => moved(axis, spans),
        InputEvent::Trigger { trigger: _, direction: _ }
        | InputEvent::Typed { code: _, direction: _ }
        | InputEvent::Unnamed { code: _, direction: _ } => Ok(None),
    }
}

fn pressed(button: &str, direction: Direction) -> Result<Option<KeyboardCommand>, Never> {
    Ok(match direction {
        Direction::Up => None,
        Direction::Down => {
            let Ok(spoken) = spoken_for(button);

            BUTTONS.iter().find(|(named, _)| *named == spoken).map(|(_, command)| *command)
        }
    })
}

fn moved(axis: Option<(AbsoluteAxisCode, i32)>, spans: &Spans) -> Result<Option<KeyboardCommand>, Never> {
    let (axis, value) = match axis {
        Some((axis, value)) => (axis, value),
        None => return Ok(None),
    };

    let (_taken, range) = match spans.iter().find(|(named, _)| *named == axis) {
        Some((_taken, range)) => (_taken, range),
        None => return Ok(None),
    };

    from_stick(axis, value, *range)
}

pub fn from_stick(axis: AbsoluteAxisCode, value: i32, range: (i32, i32)) -> Result<Option<KeyboardCommand>, Never> {
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
            true => Some(KeyboardCommand::Left),
            false => Some(KeyboardCommand::Right),
        },
        AbsoluteAxisCode::ABS_Y | AbsoluteAxisCode::ABS_RY => match pushed < 0.0 {
            true => Some(KeyboardCommand::Up),
            false => Some(KeyboardCommand::Down),
        },
        _ => None,
    })
}

#[derive(Debug, Default)]
pub struct PendingRepeat {
    command: Option<KeyboardCommand>,
    due: Option<Instant>,
}

impl PendingRepeat {
    pub fn pressed(&mut self, command: Option<KeyboardCommand>, now: Instant) -> Result<Option<KeyboardCommand>, Never> {
        Ok(match command {
            None => {
                self.command = None;
                self.due = None;
                None
            },
            Some(command) => match command.repeat_mode() == Ok(RepeatMode::Once) {
                true => Some(command),
                false => match self.command == Some(command) {
                    true => None,
                    false => {
                        self.command = Some(command);
                        self.due = Some(now + BEFORE_REPEAT);
                        Some(command)
                    },
                },
            },
        })
    }

    pub fn until(&self, now: Instant) -> Result<Option<Duration>, Never> {
        Ok(self.due.map(|due| due.saturating_duration_since(now)))
    }

    pub fn due(&mut self, now: Instant) -> Result<Option<KeyboardCommand>, Never> {
        let due = match self.due {
            Some(due) => due,
            None => return Ok(None),
        };

        match now < due {
            true => return Ok(None),
            false => {},
        }

        self.due = Some(now + BETWEEN_REPEATS);
        Ok(self.command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_input_gamepad::vocabulary::BUTTONS as EVERY;

    fn button_down(button: &'static str) -> InputEvent {
        InputEvent::Pressed { button, direction: Direction::Down }
    }

    fn command(event: InputEvent) -> Option<KeyboardCommand> {
        command_for(event, None, &Spans::new())
    }

    fn command_for(event: InputEvent, axis: Option<(AbsoluteAxisCode, i32)>, spans: &Spans) -> Option<KeyboardCommand> {
        let Ok(command) = super::command_for(event, axis, spans);

        command
    }

    #[test]
    fn the_button_labelled_x_is_the_one_that_raises_the_keyboard() {
        assert_eq!(command(button_down("North")), Some(KeyboardCommand::Toggle));
        assert_eq!(command(button_down("West")), Some(KeyboardCommand::Shift));
        assert_eq!(command(button_down("South")), Some(KeyboardCommand::Select));
        assert_eq!(command(button_down("East")), Some(KeyboardCommand::Backspace));
    }

    #[test]
    fn a_button_asks_once_and_on_the_way_down() {
        assert_eq!(command(InputEvent::Pressed { button: "South", direction: Direction::Up }), None);
    }

    #[test]
    fn a_button_this_keyboard_does_nothing_with_asks_for_nothing() {
        assert_eq!(command(button_down("Select")), None);
        assert_eq!(command(InputEvent::Unnamed { code: 999, direction: Direction::Down }), None);
        assert_eq!(command(InputEvent::Trigger { trigger: "LeftTrigger", direction: Direction::Down }), None);
    }

    #[test]
    fn the_dpad_arrives_named_rather_than_as_a_hat_to_be_read_here() {
        assert_eq!(command(button_down("DPadUp")), Some(KeyboardCommand::Up));
        assert_eq!(command(button_down("DPadRight")), Some(KeyboardCommand::Right));
    }

    #[test]
    fn letting_the_dpad_go_asks_for_nothing_at_all() {
        let spans: Spans = vec![(AbsoluteAxisCode::ABS_HAT0X, (-1, 1))];
        assert_eq!(command_for(InputEvent::None, Some((AbsoluteAxisCode::ABS_HAT0X, 0)), &spans), None);
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
        let pushed = |axis, value| command_for(InputEvent::None, Some((axis, value)), &spans);
        assert_eq!(pushed(AbsoluteAxisCode::ABS_X, 128), None);
        assert_eq!(pushed(AbsoluteAxisCode::ABS_X, 140), None);
        assert_eq!(pushed(AbsoluteAxisCode::ABS_X, 255), Some(KeyboardCommand::Right));
        assert_eq!(pushed(AbsoluteAxisCode::ABS_X, 0), Some(KeyboardCommand::Left));
        assert_eq!(pushed(AbsoluteAxisCode::ABS_RY, 0), Some(KeyboardCommand::Up));
    }

    #[test]
    fn a_stick_the_device_said_nothing_about_moves_nothing() {
        let nothing = Spans::new();
        assert_eq!(command_for(InputEvent::None, Some((AbsoluteAxisCode::ABS_X, 255)), &nothing), None);
        assert_eq!(command_for(InputEvent::None, None, &nothing), None);
    }

    #[test]
    fn a_direction_already_held_does_not_ask_again() {
        let now = Instant::now();
        let mut held = PendingRepeat::default();
        assert_eq!(held.pressed(Some(KeyboardCommand::Left), now), Ok(Some(KeyboardCommand::Left)));
        assert_eq!(held.pressed(Some(KeyboardCommand::Left), now), Ok(None));
        assert_eq!(held.pressed(Some(KeyboardCommand::Right), now), Ok(Some(KeyboardCommand::Right)), "a turn is a new ask");
    }

    #[test]
    fn a_held_direction_waits_then_repeats() {
        let start = Instant::now();
        let mut held = PendingRepeat::default();
        let Ok(first) = held.pressed(Some(KeyboardCommand::Down), start);

        assert_eq!(first, Some(KeyboardCommand::Down), "the press itself");
        assert_eq!(held.due(start), Ok(None), "not yet");
        assert_eq!(held.due(start + BEFORE_REPEAT - Duration::from_millis(1)), Ok(None));
        assert_eq!(held.due(start + BEFORE_REPEAT), Ok(Some(KeyboardCommand::Down)), "the first repeat");
        let then = start + BEFORE_REPEAT;
        assert_eq!(held.due(then), Ok(None), "and not again immediately");
        assert_eq!(held.due(then + BETWEEN_REPEATS), Ok(Some(KeyboardCommand::Down)));
    }

    #[test]
    fn letting_go_stops_the_repeat() {
        let now = Instant::now();
        let mut held = PendingRepeat::default();
        let Ok(first) = held.pressed(Some(KeyboardCommand::Up), now);
        let Ok(waking) = held.until(now);

        assert_eq!(first, Some(KeyboardCommand::Up), "the press itself");
        assert!(waking.is_some(), "something to wake for");
        assert_eq!(held.pressed(None, now), Ok(None));
        assert_eq!(held.due(now + BEFORE_REPEAT * 4), Ok(None));
        assert_eq!(held.until(now), Ok(None), "and nothing to wake for");
    }

    #[test]
    fn a_press_is_not_a_thing_that_repeats() {
        let now = Instant::now();
        let mut held = PendingRepeat::default();
        assert_eq!(held.pressed(Some(KeyboardCommand::Select), now), Ok(Some(KeyboardCommand::Select)));
        assert_eq!(held.pressed(Some(KeyboardCommand::Select), now), Ok(Some(KeyboardCommand::Select)), "still not a repeat");
        assert_eq!(held.until(now), Ok(None));
    }
}
