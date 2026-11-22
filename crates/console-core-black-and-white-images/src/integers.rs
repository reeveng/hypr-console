//! The integer decoding procedure of A.2 and the symbol identifier decoding of A.3.

use console_core_mq_decoding::{Context, Decoder as Arithmetic};

use crate::arithmetic;
use crate::error::DecodeError;

const LADDER: [(u32, u32); 5] = [(2, 0), (4, 4), (6, 20), (8, 84), (12, 340)];

const CONTEXTS: u32 = 9;

#[derive(Debug, Clone)]
pub(crate) struct Integer {
  contexts: Vec<Context>,
  previous: u32,
}

impl Integer {
  pub(crate) fn new() -> Result<Integer, DecodeError> {
    let contexts = arithmetic::contexts(CONTEXTS)?;

    Ok(Integer {
      contexts,
      previous: 1,
    })
  }

  pub(crate) fn decode(
    &mut self,
    decoder: &mut Arithmetic<'_>,
  ) -> Result<Option<i32>, DecodeError> {
    self.previous = 1;

    let sign = self.bit(decoder)?;
    let mut magnitude = None;

    for (count, base) in LADDER {
      match magnitude {
        Some(_) => break,
        None => {
          let more = self.bit(decoder)?;

          match more {
            0 => {
              let value = self.bits(decoder, count)?;

              magnitude = Some(value.wrapping_add(base));
            }
            _ => {}
          }
        }
      }
    }

    let magnitude_2 = match magnitude {
      Some(found) => found,
      None => {
        let wide = self.bits(decoder, 32)?;

        wide.wrapping_add(4436)
      }
    };
    let value = i32::from_be_bytes(magnitude_2.to_be_bytes());

    Ok(match (sign, magnitude_2) {
      (0, _) => Some(value),
      (_, 0) => None,
      (_, _) => Some(value.wrapping_neg()),
    })
  }

  fn bit(&mut self, decoder: &mut Arithmetic<'_>) -> Result<u32, DecodeError> {
    let bit = arithmetic::bit(decoder, &mut self.contexts, self.previous & 0x1FF)?;
    let shifted = self.previous.wrapping_shl(1) | bit;

    self.previous = match ::core::primitive::bool::then_some(self.previous < 256, ()) {
      ::core::option::Option::Some(()) => shifted,
      ::core::option::Option::None => (shifted & 511) | 256,
    };

    Ok(bit)
  }

  fn bits(&mut self, decoder: &mut Arithmetic<'_>, count: u32) -> Result<u32, DecodeError> {
    let mut value = 0u32;

    for _place in 0..count {
      let bit = self.bit(decoder)?;

      value = value.wrapping_shl(1) | bit;
    }

    Ok(value)
  }
}

#[derive(Debug, Clone)]
pub(crate) struct Identifier {
  contexts: Vec<Context>,
  length: u32,
}

impl Identifier {
  pub(crate) fn new(length: u32) -> Result<Identifier, DecodeError> {
    let contexts = arithmetic::contexts(length)?;

    Ok(Identifier { contexts, length })
  }

  pub(crate) fn decode(&mut self, decoder: &mut Arithmetic<'_>) -> Result<u32, DecodeError> {
    let mut previous = 1u32;
    let mask = 1u32
      .wrapping_shl(self.length.saturating_add(1))
      .saturating_sub(1);

    for _place in 0..self.length {
      let bit = arithmetic::bit(decoder, &mut self.contexts, previous & mask)?;

      previous = previous.wrapping_shl(1) | bit;
    }

    Ok(previous.saturating_sub(1u32.wrapping_shl(self.length)))
  }
}
