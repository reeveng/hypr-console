//! USB HID boot-keyboard descriptors and report changes.
//!
//! The descriptor selection and fixed report decoding follow the nos USB
//! driver. The events use Console's key edges. See `docs/third-party/nos.txt`.

#![forbid(unsafe_code)]

use console_core_never::Never;

use crate::events::KeyEdge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
  pub configuration: u8,
  pub interface: u8,
  pub dci: u8,
  pub max_packet: u16,
  pub interval: u8,
}

impl Endpoint {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "USB descriptor lengths are converted to Rust slice offsets at the bounded lookups"
    )
  )]
  pub fn from_configuration(bytes: &[u8]) -> Result<Option<Self>, Never> {
    let (total, configuration) = match bytes {
      &[9, 2, low, high, _, configuration, ..] => {
        (u16::from_le_bytes([low, high]), configuration)
      }
      _ => return Ok(None),
    };

    match ::core::primitive::bool::then_some(
      usize::from(total) == bytes.len() && configuration != 0,
      (),
    ) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(None),
    }

    let mut remaining = match bytes.get(9..) {
      Some(tail) => tail,
      None => return Ok(None),
    };
    let mut boot_interface = None;

    for _descriptor in 0..total.div_euclid(2) {
      let (length, kind) = match remaining {
        &[length @ 2..=255, kind, ..] => (length, kind),
        _ => return Ok(None),
      };
      let (descriptor, next) = match remaining.split_at_checked(usize::from(length)) {
        Some(parts) => parts,
        None => return Ok(None),
      };
      remaining = next;

      match (kind, descriptor) {
        (4, &[_, _, interface, 0, _, 3, 1, 1, ..]) => boot_interface = Some(interface),
        (4, _) => boot_interface = None,
        (5, &[_, _, address, attributes, low, high, interval, ..]) => {
          let max_packet = u16::from_le_bytes([low, high]);
          let number = address & 0xf;
          let interrupt_in = address & 0xf0 == 0x80 && number != 0 && attributes & 3 == 3;
          let supported = interrupt_in && (8..=64).contains(&max_packet) && interval != 0;

          match (boot_interface, supported) {
            (Some(interface), true) => {
              return Ok(Some(Self {
                configuration,
                interface,
                dci: number.wrapping_shl(1) | 1,
                max_packet,
                interval,
              }));
            }
            (None, _) | (Some(_), false) => {}
          }
        }
        _ => {}
      }
    }

    Ok(None)
  }
}

#[derive(Clone, Copy)]
pub enum Shift {
  Pressed,
  Clear,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Presence {
  Present,
  Clear,
}

#[derive(Clone, Copy)]
pub struct BootReport {
  modifiers: u8,
  keys: [u8; 6],
}

impl BootReport {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "Rust array lengths use usize; the HID report is eight bytes"
    )
  )]
  pub const LENGTH: usize = 8;
  pub const BYTES: u32 = 8;
  pub const EMPTY: Self = Self {
    modifiers: 0,
    keys: [0; 6],
  };

  pub fn read(
    [modifiers, _, one, two, three, four, five, six]: [u8; Self::LENGTH],
  ) -> Result<Option<Self>, Never> {
    let keys = [one, two, three, four, five, six];

    Ok(
      match ::core::primitive::bool::then_some(keys.iter().any(|key| (1..=3).contains(key)), ()) {
        ::core::option::Option::Some(()) => None,
        ::core::option::Option::None => Some(Self { modifiers, keys }),
      },
    )
  }

  pub fn shifted(self) -> Result<Shift, Never> {
    Ok(match self.modifiers & 0x22 {
      0 => Shift::Clear,
      _ => Shift::Pressed,
    })
  }

  fn presence(self, usage: u8) -> Result<Presence, Never> {
    let held = match usage {
      0 => false,
      0xe0..=0xe7 => {
        self.modifiers & 1_u8.wrapping_shl(u32::from(usage.saturating_sub(0xe0))) != 0
      }
      _ => self.keys.contains(&usage),
    };

    Ok(match ::core::primitive::bool::then_some(held, ()) {
      ::core::option::Option::Some(()) => Presence::Present,
      ::core::option::Option::None => Presence::Clear,
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the byte-sized HID usage becomes an index in the array of previously seen usages"
    )
  )]
  pub fn edges(self, next: Self) -> Result<impl Iterator<Item = (u8, KeyEdge)>, Never> {
    let mut earlier = [false; 256];
    Ok(
      self
        .keys
        .into_iter()
        .chain(next.keys)
        .chain(0xe0..=0xe7)
        .filter_map(move |usage| {
          let seen = earlier.get_mut(usize::from(usage))?;

          match ::core::primitive::bool::then_some(core::mem::replace(seen, true), ()) {
            ::core::option::Option::Some(()) => return None,
            ::core::option::Option::None => {}
          }

          let Ok(before) = self.presence(usage);
          let Ok(after) = next.presence(usage);

          match (before, after) {
            (Presence::Clear, Presence::Present) => Some((usage, KeyEdge::Press)),
            (Presence::Present, Presence::Clear) => Some((usage, KeyEdge::Release)),
            (Presence::Clear, Presence::Clear) | (Presence::Present, Presence::Present) => None,
          }
        }),
    )
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the bounded digit number becomes a Rust byte-array index"
  )
)]
pub fn character(usage: u16, shift: Shift) -> Result<Option<char>, Never> {
  Ok(match usage {
    0x04..=0x1d => {
      let base = match shift {
        Shift::Pressed => b'A',
        Shift::Clear => b'a',
      };
      let code = u16::from(base).saturating_add(usage.saturating_sub(0x04));

      char::from_u32(u32::from(code))
    }
    0x1e..=0x27 => {
      let digits = match shift {
        Shift::Pressed => b"!@#$%^&*()",
        Shift::Clear => b"1234567890",
      };

      digits
        .get(usize::from(usage.saturating_sub(0x1e)))
        .copied()
        .map(char::from)
    }
    0x2c => Some(' '),
    _ => None,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Debug)]
  struct MissingReport;

  #[test]
  fn a_held_key_and_a_reordered_report_emit_no_duplicate_edges() -> Result<(), MissingReport> {
    let Ok(first) = BootReport::read([0, 0, 4, 4, 5, 0, 0, 0]);
    let Ok(reordered) = BootReport::read([0, 0, 5, 4, 0, 0, 0, 0]);
    let first_report = first.ok_or(MissingReport)?;
    let reordered_report = reordered.ok_or(MissingReport)?;
    let Ok(pressed) = BootReport::EMPTY.edges(first_report);
    let Ok(mut held) = first_report.edges(reordered_report);
    let Ok(released) = reordered_report.edges(BootReport::EMPTY);

    assert!(pressed.eq([(4, KeyEdge::Press), (5, KeyEdge::Press)]));
    assert!(held.next().is_none());
    assert!(released.eq([(5, KeyEdge::Release), (4, KeyEdge::Release)]));
    let Ok(rollover) = BootReport::read([0, 0, 1, 1, 1, 1, 1, 1]);
    assert!(rollover.is_none());

    Ok(())
  }

  #[test]
  fn an_endpoint_from_another_interface_cannot_supply_the_keyboard() -> Result<(), Never> {
    let configuration = [
      9, 2, 34, 0, 2, 1, 0, 0, 0, 9, 4, 0, 0, 0, 3, 1, 1, 0, 9, 4, 1, 0, 1, 8, 6, 80, 0, 7, 5,
      0x81, 3, 8, 0, 10,
    ];
    let Ok(other_interface) = Endpoint::from_configuration(&configuration);
    assert!(other_interface.is_none());
    let mut keyboard = configuration;
    let [
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      class,
      subclass,
      protocol,
      ..,
    ] = &mut keyboard;
    *class = 3;
    *subclass = 1;
    *protocol = 1;
    let Ok(endpoint) = Endpoint::from_configuration(&keyboard);
    assert_eq!(
      endpoint,
      Some(Endpoint {
        configuration: 1,
        interface: 1,
        dci: 3,
        max_packet: 8,
        interval: 10
      }),
    );

    Ok(())
  }
}
