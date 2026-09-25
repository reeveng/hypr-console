//! The layer above the devices: one event, in one vocabulary.  What a program
//! wants to know is which button a person pressed. DeviceKind device file said so is
//! an accident of how the controller was published this boot, and every program
//! that has had to know it has been wrong about it at least once -- the paddles
//! arrive as function keys, X arrives as a function key, and everything else
//! arrives as itself, so a reader that knew only the pad silently missed a
//! third of the front of the machine.  So a press is `South` here, and a
//! program that wants to say what A does never learns that A is `BTN_SOUTH` on
//! one node and might not be on the next. `console_input_gamepad::routing` is
//! the table that says which is which and `console_input_gamepad::vocabulary`
//! turns a profile's word into a person's; both are shared with the daemon
//! rather than written twice.  Nothing here opens anything or holds any state.
//! The same event named twice is named the same way, which is what makes a
//! transcript of someone pressing buttons a test.

use console_input_gamepad::routing::{self, Hat};
use console_input_gamepad::vocabulary::TRIGGER_BUTTONS;
use console_core_never::Never;
use console_input_event_devices::EventType;

use crate::devices::DeviceKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Pressed { button: &'static str, direction: Direction },
    Trigger { trigger: &'static str, direction: Direction },
    Typed { code: u16, direction: Direction },
    Unnamed { code: u16, direction: Direction },
    None,
}

pub fn said(which: DeviceKind, kind: EventType, code: u16, value: i32) -> Result<InputEvent, Never> {
    Ok(match which {
        DeviceKind::Touch => InputEvent::None,
        DeviceKind::Typing => match kind {
            EventType::KEY => {
                let Ok(typed) = typed(code, value);

                typed
            }
            _ => InputEvent::None,
        },
        DeviceKind::Pad | DeviceKind::Keys => match kind {
            EventType::KEY => {
                let Ok(key) = key(which, code, value);

                key
            }
            EventType::ABSOLUTE => {
                let Ok(hat) = hat(code, value);

                hat
            }
            _ => InputEvent::None,
        },
    })
}

fn typed(code: u16, value: i32) -> Result<InputEvent, Never> {
    Ok(match value {
        1 => InputEvent::Typed { code, direction: Direction::Down },
        0 => InputEvent::Typed { code, direction: Direction::Up },
        _ => InputEvent::None,
    })
}

fn key(which: DeviceKind, code: u16, value: i32) -> Result<InputEvent, Never> {
    let direction = match value {
        1 => Direction::Down,
        0 => Direction::Up,
        _ => return Ok(InputEvent::None),
    };

    let Ok(named) = match which {
        DeviceKind::Pad => routing::button_of_pad(code),
        DeviceKind::Keys => routing::button_of_key(code),
        DeviceKind::Typing | DeviceKind::Touch => Ok(None),
    };

    Ok(match named {
        Some(button) => InputEvent::Pressed { button, direction },
        None => {
            let Ok(trigger) = trigger(code, direction);

            trigger
        }
    })
}

fn trigger(code: u16, direction: Direction) -> Result<InputEvent, Never> {
    let pulled = TRIGGER_BUTTONS.iter().find(|(_, key)| key.0 == code).map(|(named, _)| *named);

    Ok(match pulled {
        Some(trigger) => InputEvent::Trigger { trigger, direction },
        None => InputEvent::Unnamed { code, direction },
    })
}

fn hat(code: u16, value: i32) -> Result<InputEvent, Never> {
    let Ok(axis) = routing::is_hat(code);

    Ok(match axis {
        Hat::NotAnAxis => InputEvent::None,
        Hat::Axis => {
            let Ok(named) = routing::button_of_hat(code, value);

            match named {
                Some(button) => InputEvent::Pressed { button, direction: Direction::Down },
                None => InputEvent::None,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_input_event_devices::{AbsoluteAxisCode, KeyCode};

    fn ok(which: DeviceKind, kind: EventType, code: u16, value: i32) -> InputEvent {
        let Ok(said) = said(which, kind, code, value);

        said
    }

    #[test]
    fn a_face_button_is_named_off_the_pad() {
        assert_eq!(
            ok(DeviceKind::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 1),
            InputEvent::Pressed { button: "South", direction: Direction::Down }
        );
        assert_eq!(
            ok(DeviceKind::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 0),
            InputEvent::Pressed { button: "South", direction: Direction::Up }
        );
    }

    #[test]
    fn the_same_code_off_the_other_device_is_a_different_button() {
        assert_eq!(ok(DeviceKind::Keys, EventType::KEY, KeyCode::BTN_SOUTH.0, 1), InputEvent::Unnamed {
            code: KeyCode::BTN_SOUTH.0,
            direction: Direction::Down
        });
        assert_eq!(
            ok(DeviceKind::Keys, EventType::KEY, KeyCode::KEY_F22.0, 1),
            InputEvent::Pressed { button: "North", direction: Direction::Down },
            "X arrives as a key, and the keyboard is the device it arrives on"
        );
        assert_eq!(ok(DeviceKind::Pad, EventType::KEY, KeyCode::KEY_F22.0, 1), InputEvent::Unnamed {
            code: KeyCode::KEY_F22.0,
            direction: Direction::Down
        });
    }

    #[test]
    fn a_key_held_down_is_not_pressed_again() {
        assert_eq!(ok(DeviceKind::Keys, EventType::KEY, KeyCode::KEY_F13.0, 2), InputEvent::None);
    }

    #[test]
    fn the_dpad_is_a_hat_and_the_middle_is_no_ones_end_of_it() {
        assert_eq!(
            ok(DeviceKind::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, -1),
            InputEvent::Pressed { button: "DPadUp", direction: Direction::Down }
        );
        assert_eq!(
            ok(DeviceKind::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, 0),
            InputEvent::None
        );
    }

    #[test]
    fn a_stick_is_nothing_here_and_is_read_where_a_range_is_known() {
        assert_eq!(
            ok(DeviceKind::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RX.0, 20000),
            InputEvent::None
        );
    }

    #[test]
    fn a_trigger_is_held_rather_than_pressed() {
        assert_eq!(
            ok(DeviceKind::Pad, EventType::KEY, KeyCode::BTN_TL2.0, 1),
            InputEvent::Trigger { trigger: "LeftTrigger", direction: Direction::Down },
            "a layer held is not a button with no name"
        );
    }

    #[test]
    fn a_key_off_a_keyboard_someone_plugged_in_is_a_key_and_not_a_button() {
        assert_eq!(
            ok(DeviceKind::Typing, EventType::KEY, KeyCode::KEY_I.0, 1),
            InputEvent::Typed { code: KeyCode::KEY_I.0, direction: Direction::Down }
        );
        assert_eq!(
            ok(DeviceKind::Typing, EventType::KEY, KeyCode::KEY_F22.0, 1),
            InputEvent::Typed { code: KeyCode::KEY_F22.0, direction: Direction::Down },
            "a paddle's key is a paddle only on the device the paddles arrive on"
        );
    }

    #[test]
    fn a_finger_is_not_a_press() {
        assert_eq!(ok(DeviceKind::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 1), InputEvent::None);
        assert_eq!(
            ok(DeviceKind::Touch, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_X.0, 300),
            InputEvent::None
        );
    }

    #[test]
    fn a_sync_is_nothing() {
        assert_eq!(ok(DeviceKind::Pad, EventType::SYNCHRONIZATION, 0, 0), InputEvent::None);
    }
}
