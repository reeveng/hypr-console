//! The layer above the devices: one event, in one vocabulary.  What a program
//! wants to know is which button a person pressed. Which device file said so is
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
//! transcript of somebody pressing buttons a test.

use console_input_gamepad::routing::{self, Hat};
use console_input_gamepad::vocabulary::TRIGGER_BUTTONS;
use console_core_never::Never;
use evdev::EventType;

use crate::devices::Which;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Went {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said {
    Pressed { button: &'static str, went: Went },
    Trigger { trigger: &'static str, went: Went },
    Typed { code: u16, went: Went },
    Unnamed { code: u16, went: Went },
    Nothing,
}

pub fn said(which: Which, kind: EventType, code: u16, value: i32) -> Result<Said, Never> {
    Ok(match which {
        Which::Touch => Said::Nothing,
        Which::Typing => match kind {
            EventType::KEY => {
                let Ok(typed) = typed(code, value);

                typed
            }
            _ => Said::Nothing,
        },
        Which::Pad | Which::Keys => match kind {
            EventType::KEY => {
                let Ok(key) = key(which, code, value);

                key
            }
            EventType::ABSOLUTE => {
                let Ok(hat) = hat(code, value);

                hat
            }
            _ => Said::Nothing,
        },
    })
}

fn typed(code: u16, value: i32) -> Result<Said, Never> {
    Ok(match value {
        1 => Said::Typed { code, went: Went::Down },
        0 => Said::Typed { code, went: Went::Up },
        _ => Said::Nothing,
    })
}

fn key(which: Which, code: u16, value: i32) -> Result<Said, Never> {
    let went = match value {
        1 => Went::Down,
        0 => Went::Up,
        _ => return Ok(Said::Nothing),
    };

    let Ok(named) = match which {
        Which::Pad => routing::button_of_pad(code),
        Which::Keys => routing::button_of_key(code),
        Which::Typing | Which::Touch => Ok(None),
    };

    Ok(match named {
        Some(button) => Said::Pressed { button, went },
        None => {
            let Ok(trigger) = trigger(code, went);

            trigger
        }
    })
}

fn trigger(code: u16, went: Went) -> Result<Said, Never> {
    let pulled = TRIGGER_BUTTONS.iter().find(|(_, key)| key.0 == code).map(|(named, _)| *named);

    Ok(match pulled {
        Some(trigger) => Said::Trigger { trigger, went },
        None => Said::Unnamed { code, went },
    })
}

fn hat(code: u16, value: i32) -> Result<Said, Never> {
    let Ok(axis) = routing::is_hat(code);

    Ok(match axis {
        Hat::NotAnAxis => Said::Nothing,
        Hat::Axis => {
            let Ok(named) = routing::button_of_hat(code, value);

            match named {
                Some(button) => Said::Pressed { button, went: Went::Down },
                None => Said::Nothing,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use evdev::{AbsoluteAxisCode, KeyCode};

    fn ok(which: Which, kind: EventType, code: u16, value: i32) -> Said {
        let Ok(said) = said(which, kind, code, value);

        said
    }

    #[test]
    fn a_face_button_is_named_off_the_pad() {
        assert_eq!(
            ok(Which::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 1),
            Said::Pressed { button: "South", went: Went::Down }
        );
        assert_eq!(
            ok(Which::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 0),
            Said::Pressed { button: "South", went: Went::Up }
        );
    }

    #[test]
    fn the_same_code_off_the_other_device_is_a_different_button() {
        assert_eq!(ok(Which::Keys, EventType::KEY, KeyCode::BTN_SOUTH.0, 1), Said::Unnamed {
            code: KeyCode::BTN_SOUTH.0,
            went: Went::Down
        });
        assert_eq!(
            ok(Which::Keys, EventType::KEY, KeyCode::KEY_F22.0, 1),
            Said::Pressed { button: "North", went: Went::Down },
            "X arrives as a key, and the keyboard is the device it arrives on"
        );
        assert_eq!(ok(Which::Pad, EventType::KEY, KeyCode::KEY_F22.0, 1), Said::Unnamed {
            code: KeyCode::KEY_F22.0,
            went: Went::Down
        });
    }

    #[test]
    fn a_key_held_down_is_not_pressed_again() {
        assert_eq!(ok(Which::Keys, EventType::KEY, KeyCode::KEY_F13.0, 2), Said::Nothing);
    }

    #[test]
    fn the_dpad_is_a_hat_and_the_middle_is_nobodys_end_of_it() {
        assert_eq!(
            ok(Which::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, -1),
            Said::Pressed { button: "DPadUp", went: Went::Down }
        );
        assert_eq!(
            ok(Which::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, 0),
            Said::Nothing
        );
    }

    #[test]
    fn a_stick_is_nothing_here_and_is_read_where_a_range_is_known() {
        assert_eq!(
            ok(Which::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RX.0, 20000),
            Said::Nothing
        );
    }

    #[test]
    fn a_trigger_is_held_rather_than_pressed() {
        assert_eq!(
            ok(Which::Pad, EventType::KEY, KeyCode::BTN_TL2.0, 1),
            Said::Trigger { trigger: "LeftTrigger", went: Went::Down },
            "a layer held is not a button with no name"
        );
    }

    #[test]
    fn a_key_off_a_keyboard_somebody_plugged_in_is_a_key_and_not_a_button() {
        assert_eq!(
            ok(Which::Typing, EventType::KEY, KeyCode::KEY_I.0, 1),
            Said::Typed { code: KeyCode::KEY_I.0, went: Went::Down }
        );
        assert_eq!(
            ok(Which::Typing, EventType::KEY, KeyCode::KEY_F22.0, 1),
            Said::Typed { code: KeyCode::KEY_F22.0, went: Went::Down },
            "a paddle's key is a paddle only on the device the paddles arrive on"
        );
    }

    #[test]
    fn a_finger_is_not_a_press() {
        assert_eq!(ok(Which::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 1), Said::Nothing);
        assert_eq!(
            ok(Which::Touch, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_X.0, 300),
            Said::Nothing
        );
    }

    #[test]
    fn a_sync_is_nothing() {
        assert_eq!(ok(Which::Pad, EventType::SYNCHRONIZATION, 0, 0), Said::Nothing);
    }
}
