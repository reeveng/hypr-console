//! One event, as the kernel lays out `input_event` on this machine.
//!
//! Sixteen bytes of time, then a type, a code and a value, little-endian: the
//! width of the time is already a fact about the architecture, so the order is
//! said with it rather than left to whatever the compiler ran on. The time is
//! neither read nor written. Nothing here decides anything by it, and uinput
//! stamps what it is handed with its own clock whatever it said.

use console_core_never::Never;
use console_core_number_conversion::index;

use crate::codes::{EventType, SynchronizationCode};

pub const LONG: u32 = 24;

const TIME: u32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
    pub kind: EventType,
    pub code: u16,
    pub value: i32,
}

impl InputEvent {
    pub const REPORT: InputEvent =
        InputEvent { kind: EventType::SYNCHRONIZATION, code: SynchronizationCode::SYN_REPORT.0, value: 0 };

    pub fn bytes(&self) -> Result<Vec<u8>, Never> {
        let Ok(time) = index(TIME);
        let mut bytes = vec![0; time];

        bytes.extend(self.kind.0.to_le_bytes());
        bytes.extend(self.code.to_le_bytes());
        bytes.extend(self.value.to_le_bytes());

        Ok(bytes)
    }
}

pub fn events(read: &[u8]) -> Result<Vec<InputEvent>, Never> {
    let Ok(long) = index(LONG);

    Ok(read
        .chunks_exact(long)
        .filter_map(|event| {
            let Ok(event) = event_in(event);

            event
        })
        .collect())
}

fn event_in(bytes: &[u8]) -> Result<Option<InputEvent>, Never> {
    let Ok(time) = index(TIME);

    let after = match bytes.split_at_checked(time) {
        Some((_, after)) => after,
        None => return Ok(None),
    };
    let (kind, after) = match after.split_first_chunk::<2>() {
        Some(split) => split,
        None => return Ok(None),
    };
    let (code, after) = match after.split_first_chunk::<2>() {
        Some(split) => split,
        None => return Ok(None),
    };
    let value = match after.first_chunk::<4>() {
        Some(value) => value,
        None => return Ok(None),
    };

    Ok(Some(InputEvent {
        kind: EventType(u16::from_le_bytes(*kind)),
        code: u16::from_le_bytes(*code),
        value: i32::from_le_bytes(*value),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::KeyCode;

    #[test]
    fn an_event_reads_back_as_itself() {
        let pressed = InputEvent { kind: EventType::KEY, code: KeyCode::BTN_SOUTH.0, value: 1 };
        let Ok(bytes) = pressed.bytes();
        let Ok(read) = events(&bytes);

        assert_eq!(bytes.len(), 24);
        assert_eq!(read, vec![pressed]);
    }

    #[test]
    fn a_torn_event_is_not_read() {
        let Ok(whole) = InputEvent::REPORT.bytes();
        let torn = match whole.split_last() {
            Some((_, torn)) => torn,
            None => panic!("an event with nothing in it"),
        };
        let Ok(read) = events(torn);

        assert_eq!(read, Vec::new());
    }
}
