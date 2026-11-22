//! The IVRS describes which AMD-Vi unit owns each PCI requester. This boot
//! path supports a single unit on PCI segment zero. A USB requester must
//! appear directly in its scope. An IVMD reservation needs a platform
//! mapping that survives handoff, so that configuration stays with firmware.

use core::fmt;

#[derive(Clone, Copy)]
pub(super) struct Requester(pub(super) u16);

#[derive(Clone, Copy)]
pub(super) struct Ivrs<'a>(&'a [u8]);

#[derive(Clone, Copy)]
pub(super) struct Unit<'a> {
  pub(super) registers: u64,
  device: u16,
  kind: u8,
  scope: &'a [u8],
}

#[derive(Debug)]
pub(super) enum Error {
  Length,
  MultipleUnits,
  FirmwareMemory,
  Unsupported,
}

impl fmt::Display for Error {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    out.write_str(match self {
      Self::Length => "truncated IVRS block",
      Self::MultipleUnits => "IVRS describes multiple AMD-Vi units",
      Self::FirmwareMemory => "IVRS describes firmware DMA memory reservations",
      Self::Unsupported => "unsupported IVRS segment or requester scope",
    })
  }
}

#[derive(PartialEq, Eq)]
pub(super) enum Coverage {
  Assigned,
  Skipped,
}

struct Block<'a> {
  kind: u8,
  device: u16,
  bytes: &'a [u8],
}

impl<'a> Ivrs<'a> {
  pub(super) fn new(table: &'a [u8]) -> Result<Self, Error> {
    let body = table.get(48..).ok_or(Error::Length);
    let blocks = body?;

    Ok(Self(blocks))
  }

  pub(super) fn unit(self) -> Result<Option<Unit<'a>>, Error> {
    let mut selected: Option<Unit<'a>> = None;
    let mut remaining = self.0;

    for _record in 0..=16_384_u32 {
      let next = take_block(&mut remaining)?;
      let block = match next {
        Some(block) => block,
        None => return Ok(selected),
      };
      let hardware = block.hardware()?;
      let candidate = match hardware {
        Some(candidate) => candidate,
        None => continue,
      };

      selected = match selected {
        None => Some(candidate),
        Some(previous) => match (
          previous.registers == candidate.registers && previous.device == candidate.device,
          previous.kind >= candidate.kind,
        ) {
          (false, _) => return Err(Error::MultipleUnits),
          (true, true) => Some(previous),
          (true, false) => Some(candidate),
        },
      };
    }

    Err(Error::Length)
  }
}

impl<'a> Block<'a> {
  fn hardware(self) -> Result<Option<Unit<'a>>, Error> {
    let header_end = match self.kind {
      0x10 => 24,
      0x11 | 0x40 => 40,
      0x20..=0x22 => return Err(Error::FirmwareMemory),
      _ => return Ok(None),
    };
    let header = self.bytes.first_chunk::<20>().ok_or(Error::Length);
    let fields = header?;
    let [
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      a,
      b,
      c,
      d,
      e,
      f,
      g,
      h,
      lower,
      upper,
      _,
      _,
    ] = *fields;
    let registers = u64::from_le_bytes([a, b, c, d, e, f, g, h]);
    let segment = u16::from_le_bytes([lower, upper]);

    match ::core::primitive::bool::then_some(segment == 0 && registers != 0 && registers.is_multiple_of(16_384), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(Error::Unsupported),
    }

    let (_, scope) = split(self.bytes, header_end)?;

    Ok(Some(Unit {
      registers,
      device: self.device,
      kind: self.kind,
      scope,
    }))
  }
}

#[derive(Clone, Copy)]
enum Range {
  Direct(u16),
  Aliased(u16),
  Closed,
}

impl Range {
  fn close(self, end: u16, requester: Requester) -> Result<Option<Coverage>, Error> {
    let (start, coverage) = match self {
      Self::Direct(start) => (start, Coverage::Assigned),
      Self::Aliased(start) => (start, Coverage::Skipped),
      Self::Closed => return Err(Error::Unsupported),
    };

    Ok(match ::core::primitive::bool::then_some((start..=end).contains(&requester.0), ()) {
      ::core::option::Option::Some(()) => Some(coverage),
      ::core::option::Option::None => None,
    })
  }
}

impl Unit<'_> {
  pub(super) fn covers(self, requester: Requester) -> Result<Coverage, Error> {
    let mut entries = self.scope;
    let mut coverage = Coverage::Skipped;
    let mut range = Range::Closed;

    for _record in 0..=16_384_u32 {
      let header = match entries.first_chunk::<4>() {
        Some(header) => header,
        None => {
          return match (entries.is_empty(), range) {
            (true, Range::Closed) => Ok(coverage),
            (true, Range::Direct(_) | Range::Aliased(_)) | (false, _) => Err(Error::Length),
          };
        }
      };
      let [kind, low, high, _settings] = *header;
      let device = u16::from_le_bytes([low, high]);
      let size = entry_length(entries, kind)?;
      let (entry, tail) = split(entries, size)?;
      entries = tail;

      match kind {
        0x01 => coverage = Coverage::Assigned,
        0x02 | 0x46 => match ::core::primitive::bool::then_some(device == requester.0, ()) {
          ::core::option::Option::Some(()) => coverage = Coverage::Assigned,
          ::core::option::Option::None => {}
        },
        0x03 | 0x47 => range = Range::Direct(device),
        0x42 | 0x43 => {
          let alias = entry.first_chunk::<8>().ok_or(Error::Length);
          let fields = alias?;
          let [_, _, _, _, _, a, b, _] = *fields;

          match ::core::primitive::bool::then_some(u16::from_le_bytes([a, b]) == requester.0 || device == requester.0, ()) {
            ::core::option::Option::Some(()) => return Ok(Coverage::Skipped),
            ::core::option::Option::None => {}
          }

          range = match kind {
            0x43 => Range::Aliased(device),
            _ => Range::Closed,
          };
        }
        0x04 => {
          let matched = range.close(device, requester)?;

          match matched {
            Some(Coverage::Assigned) => coverage = Coverage::Assigned,
            Some(Coverage::Skipped) => return Ok(Coverage::Skipped),
            None => {}
          }

          range = Range::Closed;
        }
        _ => {}
      }
    }

    Err(Error::Length)
  }
}

fn entry_length(bytes: &[u8], kind: u8) -> Result<u16, Error> {
  match kind {
    0xf0 => {
      let prefix = bytes.first_chunk::<22>().ok_or(Error::Length);
      let fields = prefix?;
      let [.., length] = *fields;

      Ok(22_u16.saturating_add(u16::from(length)))
    }
    0..=0xbf => Ok(4_u16.wrapping_shl(u32::from(kind.wrapping_shr(6)))),
    _ => Err(Error::Unsupported),
  }
}

fn take_block<'a>(remaining: &mut &'a [u8]) -> Result<Option<Block<'a>>, Error> {
  let prefix = match remaining.first_chunk::<6>() {
    Some(prefix) => prefix,
    None => {
      return match ::core::primitive::bool::then_some(remaining.is_empty(), ()) {
        ::core::option::Option::Some(()) => Ok(None),
        ::core::option::Option::None => Err(Error::Length),
      };
    }
  };
  let [kind, _, low, high, low_id, high_id] = *prefix;
  let length = u16::from_le_bytes([low, high]);

  match ::core::primitive::bool::then_some(length >= 6, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(Error::Length),
  }

  let (bytes, next) = split(remaining, length)?;
  *remaining = next;

  Ok(Some(Block {
    kind,
    device: u16::from_le_bytes([low_id, high_id]),
    bytes,
  }))
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the validated IVRS record length becomes a Rust slice boundary here"
  )
)]
fn split(bytes: &[u8], length: u16) -> Result<(&[u8], &[u8]), Error> {
  bytes
    .split_at_checked(usize::from(length))
    .ok_or(Error::Length)
}
