//! One event, as the kernel lays out `input_event` on this machine.
//!
//! Sixteen bytes of time, then a type, a code and a value, little-endian: the
//! width of the time is already a fact about the architecture, so the order is
//! said with it rather than left to whatever the compiler ran on. The time is
//! neither read nor written. Nothing here decides anything by it, and uinput
//! stamps what it is handed with its own clock whatever it said.

use console_core_never::Never;

use crate::codes::{EventType, SynchronizationCode};

pub const LONG: u32 = 24;

pub(crate) const EMPTY_BATCH: [u8; 64 * 24] = [0; 64 * 24];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
  pub kind: EventType,
  pub code: u16,
  pub value: i32,
}

impl InputEvent {
  pub const REPORT: InputEvent = InputEvent {
    kind: EventType::SYNCHRONIZATION,
    code: SynchronizationCode::SYN_REPORT.0,
    value: 0,
  };

  pub fn bytes(&self) -> Result<Vec<u8>, Never> {
    let mut bytes = Vec::from([0; 16]);

    bytes.extend(self.kind.0.to_le_bytes());
    bytes.extend(self.code.to_le_bytes());
    bytes.extend(self.value.to_le_bytes());

    Ok(bytes)
  }
}

pub fn events(read: &[u8]) -> Result<Vec<InputEvent>, Never> {
  let (complete, _incomplete) = read.as_chunks::<24>();

  Ok(
    complete
      .iter()
      .map(|event| {
        let Ok(event_2) = event_in(event);

        event_2
      })
      .collect(),
  )
}

fn event_in(bytes: &[u8; 24]) -> Result<InputEvent, Never> {
  let [
    ..,
    kind_a,
    kind_b,
    code_a,
    code_b,
    value_a,
    value_b,
    value_c,
    value_d,
  ] = *bytes;

  Ok(InputEvent {
    kind: EventType(u16::from_le_bytes([kind_a, kind_b])),
    code: u16::from_le_bytes([code_a, code_b]),
    value: i32::from_le_bytes([value_a, value_b, value_c, value_d]),
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::keys::KeyCode;
  use std::error::Error;

  #[test]
  fn an_event_reads_back_as_itself() {
    let pressed = InputEvent {
      kind: EventType::KEY,
      code: KeyCode::BTN_SOUTH.0,
      value: 1,
    };
    let Ok(bytes) = pressed.bytes();
    let Ok(read) = events(&bytes);

    assert_eq!(bytes.len(), 24);
    assert_eq!(read, vec![pressed]);
  }

  #[test]
  fn a_torn_event_is_not_read() -> Result<(), Box<dyn Error>> {
    let Ok(whole) = InputEvent::REPORT.bytes();
    let torn = match whole.split_last() {
      Some((_, torn)) => torn,
      None => return Err(Box::from("an event with nothing in it")),
    };
    let Ok(read) = events(torn);

    assert_eq!(read, Vec::new());

    Ok(())
  }
}
