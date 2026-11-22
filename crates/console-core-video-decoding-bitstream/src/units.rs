//! A stream of NAL units cut into its units, and a unit's payload unescaped.
//!
//! A raw H.264 or HEVC stream puts `00 00 01` before every unit, with a zero
//! in front of that where the encoder liked, and nothing says how long a unit
//! is but where the next one starts. Every start code is found in one pass
//! and a unit runs from the end of one to the start of the next, less the
//! zeros at its end, which belong to the code after it rather than to the
//! unit. A sample in an MP4 or Matroska file says the length of each unit in
//! front of it instead, in as many bytes as the file's decoder configuration
//! said, and is walked from one length to the next.

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::BitError;

const START: [u8; 3] = [0, 0, 1];

const START_LENGTH: u64 = 3;

const EMULATION_PREVENTION: u8 = 3;

const RUN_BEFORE_PREVENTION: u8 = 2;

pub fn unescape_rbsp(escaped: &[u8]) -> Result<Vec<u8>, Never> {
  let mut payload = Vec::with_capacity(escaped.len());
  let mut zeros = 0u8;

  for byte in escaped {
    match (zeros, *byte) {
      (RUN_BEFORE_PREVENTION.., EMULATION_PREVENTION) => zeros = 0,
      (_, 0) => {
        payload.push(0);
        zeros = zeros.saturating_add(1);
      }
      (_, payload_byte) => {
        payload.push(payload_byte);
        zeros = 0;
      }
    }
  }

  Ok(payload)
}

pub fn annexb_nals(stream: &[u8]) -> Result<Vec<&[u8]>, Never> {
  let starts = || {
    stream
      .array_windows::<{ START.len() }>()
      .enumerate()
      .filter_map(|(at, three)| match *three {
        START => {
          let Ok(start) = fitted::<_, u64>(at);

          Some(start)
        }
        _ => None,
      })
  };

  let Ok(length) = fitted::<_, u64>(stream.len());
  let ends = starts().skip(1).chain(std::iter::once(length));

  Ok(
    starts()
      .zip(ends)
      .filter_map(|(start, end)| {
        let Ok(from) = index(start.saturating_add(START_LENGTH));
        let Ok(to) = index(end);

        stream.get(from..to)
      })
      .filter_map(|unit| {
        let Ok(kept) = without_trailing_zeros(unit);

        kept
      })
      .collect(),
  )
}

pub fn length_prefixed_nals(sample: &[u8], length_size: u8) -> Result<Vec<&[u8]>, BitError> {
  match length_size {
    0 => Err(BitError::Unmeasured),
    _measured => {
      let walked = iterate((sample, Vec::new()), |(rest, mut units)| {
        Ok(match rest {
          [] => Step::Halt(Ok(units)),
          _ => match next_unit(rest, length_size) {
            Ok((unit, after)) => {
              units.push(unit);

              Step::Again((after, units))
            }
            Err(fault) => Step::Halt(Err(fault)),
          },
        })
      });

      match walked {
        Ok(units) => units,
        Err(Endless) => Err(BitError::PastTheEnd),
      }
    }
  }
}

fn next_unit(rest: &[u8], length_size: u8) -> Result<(&[u8], &[u8]), BitError> {
  let Ok(said) = index(length_size);

  let (length, after) = match rest.split_at_checked(said) {
    Some(split) => split,
    None => return Err(BitError::PastTheEnd),
  };

  let measured = length
    .iter()
    .fold(0u64, |held, byte| held.wrapping_shl(8) | u64::from(*byte));
  let Ok(long) = index(measured);

  match after.split_at_checked(long) {
    Some(split) => Ok(split),
    None => Err(BitError::PastTheEnd),
  }
}

fn without_trailing_zeros(unit: &[u8]) -> Result<Option<&[u8]>, Never> {
  Ok(match unit.iter().rposition(|byte| *byte != 0) {
    Some(last) => unit.get(..=last),
    None => None,
  })
}
