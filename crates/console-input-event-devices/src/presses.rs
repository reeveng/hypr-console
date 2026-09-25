//! The pad and the keys, read the way the kernel hands them over.
//!
//! Nothing that maps the pad is running down here -- InputPlumber may or may
//! not be up, and the desktop's own daemon certainly is not -- so every event
//! node is read and a press means the same thing whichever of them it came
//! out of. The d-pad arrives as a hat from the pad InputPlumber makes and as
//! buttons from one it has not claimed, and a keyboard plugged in by somebody
//! who has one arrives as arrows; all three are the same four directions. A
//! is Enter and B is Backspace, for the same reason.

use console_core_never::Never;

use crate::codes::{AbsoluteAxisCode, EventType};
use crate::event::{InputEvent, events};
use crate::keys::KeyCode;

const PRESSED: i32 = 1;
const REPEATED: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonPress {
    Up,
    Down,
    Left,
    Right,
    Choose,
    Back,
}

pub fn pressed(read: &[u8]) -> Result<Vec<ButtonPress>, Never> {
    let Ok(read) = events(read);

    Ok(read
        .iter()
        .filter_map(|event| {
            let Ok(press) = meant(event);

            press
        })
        .collect())
}

fn meant(event: &InputEvent) -> Result<Option<ButtonPress>, Never> {
    Ok(match (event.kind, event.code, event.value) {
        (EventType::KEY, code, PRESSED | REPEATED) => match KeyCode(code) {
            KeyCode::KEY_UP | KeyCode::BTN_DPAD_UP => Some(ButtonPress::Up),
            KeyCode::KEY_DOWN | KeyCode::BTN_DPAD_DOWN => Some(ButtonPress::Down),
            KeyCode::KEY_LEFT | KeyCode::BTN_DPAD_LEFT => Some(ButtonPress::Left),
            KeyCode::KEY_RIGHT | KeyCode::BTN_DPAD_RIGHT => Some(ButtonPress::Right),
            KeyCode::KEY_ENTER | KeyCode::BTN_SOUTH => match event.value {
                PRESSED => Some(ButtonPress::Choose),
                _ => None,
            },
            KeyCode::KEY_BACKSPACE | KeyCode::KEY_ESC | KeyCode::BTN_EAST => match event.value {
                PRESSED => Some(ButtonPress::Back),
                _ => None,
            },
            _ => None,
        },
        (EventType::ABSOLUTE, code, value) => match (AbsoluteAxisCode(code), value) {
            (AbsoluteAxisCode::ABS_HAT0Y, -1) => Some(ButtonPress::Up),
            (AbsoluteAxisCode::ABS_HAT0Y, 1) => Some(ButtonPress::Down),
            (AbsoluteAxisCode::ABS_HAT0X, -1) => Some(ButtonPress::Left),
            (AbsoluteAxisCode::ABS_HAT0X, 1) => Some(ButtonPress::Right),
            _ => None,
        },
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: EventType, code: u16, value: i32) -> Vec<u8> {
        let Ok(bytes) = InputEvent { kind, code, value }.bytes();

        bytes
    }

    #[test]
    fn the_hat_the_pad_buttons_and_the_arrows_are_one_direction() {
        let mut read = event(EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, 1);

        read.extend(event(EventType::KEY, KeyCode::BTN_DPAD_DOWN.0, PRESSED));
        read.extend(event(EventType::KEY, KeyCode::KEY_DOWN.0, PRESSED));
        let Ok(presses) = pressed(&read);

        assert_eq!(presses, vec![ButtonPress::Down, ButtonPress::Down, ButtonPress::Down]);
    }

    #[test]
    fn letting_go_is_not_a_press() {
        let mut read = event(EventType::KEY, KeyCode::BTN_SOUTH.0, 0);

        read.extend(event(EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, 0));
        let Ok(presses) = pressed(&read);

        assert_eq!(presses, Vec::new());
    }

    #[test]
    fn a_held_choice_is_chosen_once() {
        let mut read = event(EventType::KEY, KeyCode::BTN_SOUTH.0, PRESSED);

        read.extend(event(EventType::KEY, KeyCode::BTN_SOUTH.0, REPEATED));
        let Ok(presses) = pressed(&read);

        assert_eq!(presses, vec![ButtonPress::Choose]);
    }

    #[test]
    fn a_torn_event_is_not_read() {
        let whole = event(EventType::KEY, KeyCode::BTN_SOUTH.0, PRESSED);
        let torn = match whole.split_last() {
            Some((_, torn)) => torn,
            None => panic!("an event with nothing in it"),
        };
        let Ok(presses) = pressed(torn);

        assert_eq!(presses, Vec::new());
    }
}
