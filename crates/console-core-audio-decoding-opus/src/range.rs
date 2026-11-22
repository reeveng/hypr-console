//! The range decoder of RFC 6716 section 4.1.
//!
//! Symbols are read from the front of a frame and raw bits from its back, and a read past
//! either end reads zeros, so a damaged frame decodes to something and is noticed by its
//! budget running out or a value out of range rather than by a panic. The section defines the
//! range, the value and the window as unsigned 32-bit words and every step on them as
//! arithmetic modulo two to the thirty-second, so the wrapping here is the decoder's own
//! policy and not a bound left unsaid. A table of probabilities is the distribution as the
//! RFC prints it, each symbol's share of two to the `bits`, and the cumulative frequencies
//! the reference decoder keeps precomputed are summed as a symbol is read.
//!
//! Renormalising shifts a byte in for each eight bits the range has fallen below the top,
//! which is at most four times for any range that is not zero; the loop says that bound.

use std::cmp::Ordering;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const SYMBOL_BITS: u32 = 8;
const CODE_BITS: u32 = 32;
const SYMBOL_MAX: u32 = (1 << SYMBOL_BITS) - 1;
const CODE_TOP: u32 = 1 << (CODE_BITS - 1);
const CODE_MASK: u32 = CODE_TOP - 1;
const CODE_BOTTOM: u32 = CODE_TOP >> SYMBOL_BITS;
const CODE_EXTRA: u32 = (CODE_BITS - 2) % SYMBOL_BITS + 1;
const CARRIED: u32 = SYMBOL_BITS - CODE_EXTRA;
const RANGE_AT_START: u32 = 1 << CODE_EXTRA;
const WINDOW_FULL: u32 = CODE_BITS - SYMBOL_BITS;
const BYTES_IN_A_WORD: u32 = CODE_BITS / SYMBOL_BITS;
const BITS_AT_START: i32 = 9;

pub(crate) const BITRES: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bit {
  Zero,
  One,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Span {
  pub(crate) low: u32,
  pub(crate) high: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct RangeDecoder<'a> {
  buffer: &'a [u8],
  storage: u32,
  front: u32,
  back: u32,
  window: u32,
  window_bits: u32,
  bits_total: i32,
  range: u32,
  value: u32,
  remainder: u32,
}

pub(crate) fn significant_bits(value: u32) -> Result<u32, Never> {
  Ok(u32::BITS.wrapping_sub(value.leading_zeros()))
}

fn signed(value: u32) -> Result<i32, Never> {
  fitted::<u32, i32>(value)
}

impl<'a> RangeDecoder<'a> {
  pub(crate) fn new(buffer: &'a [u8]) -> Result<RangeDecoder<'a>, Never> {
    let Ok(storage) = fitted::<_, u32>(buffer.len());

    let mut made = RangeDecoder {
      buffer,
      storage,
      front: 0,
      back: 0,
      window: 0,
      window_bits: 0,
      bits_total: BITS_AT_START,
      range: RANGE_AT_START,
      value: 0,
      remainder: 0,
    };
    let Ok(first) = made.front_byte();

    made.remainder = first;
    made.value = made
      .range
      .wrapping_sub(1)
      .wrapping_sub(first.wrapping_shr(CARRIED));

    let Ok(()) = made.normalize();

    Ok(made)
  }

  fn front_byte(&mut self) -> Result<u32, Never> {
    let Ok(at) = index(self.front);

    Ok(match (self.front.cmp(&self.storage), self.buffer.get(at)) {
      (Ordering::Less, Some(byte)) => {
        self.front = self.front.wrapping_add(1);

        u32::from(*byte)
      }
      (Ordering::Less, None) | (Ordering::Equal | Ordering::Greater, _) => 0,
    })
  }

  fn back_byte(&mut self) -> Result<u32, Never> {
    match self.back.cmp(&self.storage) {
      Ordering::Less => {
        self.back = self.back.wrapping_add(1);

        let Ok(at) = index(self.storage.wrapping_sub(self.back));

        Ok(match self.buffer.get(at) {
          Some(byte) => u32::from(*byte),
          None => 0,
        })
      }
      Ordering::Equal | Ordering::Greater => Ok(0),
    }
  }

  fn normalize(&mut self) -> Result<(), Never> {
    for _byte in 0..BYTES_IN_A_WORD {
      match self.range.cmp(&CODE_BOTTOM) {
        Ordering::Less | Ordering::Equal => {
          self.bits_total = self.bits_total.wrapping_add(8);
          self.range = self.range.wrapping_shl(SYMBOL_BITS);

          let carried = self.remainder;
          let Ok(next) = self.front_byte();

          self.remainder = next;

          let symbol = (carried.wrapping_shl(SYMBOL_BITS) | next).wrapping_shr(CARRIED);

          self.value = self
            .value
            .wrapping_shl(SYMBOL_BITS)
            .wrapping_add(SYMBOL_MAX & !symbol)
            & CODE_MASK;
        }
        Ordering::Greater => break,
      }
    }

    Ok(())
  }

  fn slot(&self, extent: u32) -> Result<u32, Never> {
    Ok(match self.value.checked_div(extent) {
      Some(slot) => slot,
      None => u32::MAX,
    })
  }

  pub(crate) fn decode(&mut self, total: u32) -> Result<u32, Never> {
    let extent = match self.range.checked_div(total) {
      Some(extent) => extent,
      None => 0,
    };
    let Ok(slot) = self.slot(extent);

    Ok(total.wrapping_sub(slot.wrapping_add(1).min(total)))
  }

  pub(crate) fn decode_bin(&mut self, bits: u32) -> Result<u32, Never> {
    let extent = self.range.wrapping_shr(bits);
    let Ok(slot) = self.slot(extent);
    let total = 1u32.wrapping_shl(bits);

    Ok(total.wrapping_sub(slot.wrapping_add(1).min(total)))
  }

  fn narrow_interval(
    &mut self,
    span: Span,
    crate::quantities::Extent(extent): crate::quantities::Extent,
    crate::quantities::Total(total): crate::quantities::Total,
  ) -> Result<(), Never> {
    let below = extent.wrapping_mul(total.wrapping_sub(span.high));

    self.value = self.value.wrapping_sub(below);
    self.range = match span.low.cmp(&0) {
      Ordering::Greater => extent.wrapping_mul(span.high.wrapping_sub(span.low)),
      Ordering::Less | Ordering::Equal => self.range.wrapping_sub(below),
    };

    self.normalize()
  }

  pub(crate) fn update(&mut self, span: Span, total: u32) -> Result<(), Never> {
    let extent = match self.range.checked_div(total) {
      Some(extent) => extent,
      None => 0,
    };

    self.narrow_interval(
      span,
      crate::quantities::Extent(extent),
      crate::quantities::Total(total),
    )
  }

  pub(crate) fn update_bin(&mut self, span: Span, bits: u32) -> Result<(), Never> {
    let extent = self.range.wrapping_shr(bits);

    self.narrow_interval(
      span,
      crate::quantities::Extent(extent),
      crate::quantities::Total(1u32.wrapping_shl(bits)),
    )
  }

  pub(crate) fn bit_logp(&mut self, logp: u32) -> Result<Bit, Never> {
    let whole = self.range;
    let step = whole.wrapping_shr(logp);

    let said = match self.value.cmp(&step) {
      Ordering::Less => {
        self.range = step;

        Bit::One
      }
      Ordering::Equal | Ordering::Greater => {
        self.value = self.value.wrapping_sub(step);
        self.range = whole.wrapping_sub(step);

        Bit::Zero
      }
    };
    let Ok(()) = self.normalize();

    Ok(said)
  }

  pub(crate) fn symbol(&mut self, shares: &[u8], bits: u32) -> Result<u32, Never> {
    let unit = self.range.wrapping_shr(bits);
    let mut remaining = 1u32.wrapping_shl(bits);
    let mut above = self.range;
    let mut below = self.range;
    let mut found = 0u32;

    for (place, share) in (0u32..).zip(shares) {
      above = below;
      remaining = remaining.wrapping_sub(u32::from(*share));
      below = unit.wrapping_mul(remaining);
      found = place;

      match self.value.cmp(&below) {
        Ordering::Equal | Ordering::Greater => break,
        Ordering::Less => {}
      }
    }

    let below_2 = match self.value.cmp(&below) {
      Ordering::Less => 0,
      Ordering::Equal | Ordering::Greater => below,
    };

    self.value = self.value.wrapping_sub(below_2);
    self.range = above.wrapping_sub(below_2);

    let Ok(()) = self.normalize();

    Ok(found)
  }

  pub(crate) fn uint(&mut self, total: u32) -> Result<u32, Never> {
    let last = total.wrapping_sub(1);
    let Ok(width) = significant_bits(last);

    match width.cmp(&8) {
      Ordering::Greater => {
        let extra = width.wrapping_sub(8);
        let top = last.wrapping_shr(extra).wrapping_add(1);
        let Ok(slot) = self.decode(top);
        let Ok(()) = self.update(
          Span {
            low: slot,
            high: slot.wrapping_add(1),
          },
          top,
        );
        let Ok(low) = self.bits(extra);
        let joined = slot.wrapping_shl(extra) | low;

        Ok(joined.min(last))
      }
      Ordering::Less | Ordering::Equal => {
        let Ok(slot) = self.decode(total);
        let Ok(()) = self.update(
          Span {
            low: slot,
            high: slot.wrapping_add(1),
          },
          total,
        );

        Ok(slot)
      }
    }
  }

  pub(crate) fn bits(&mut self, count: u32) -> Result<u32, Never> {
    match count {
      0 => return Ok(0),
      _some => {}
    }

    let mut window = self.window;
    let mut available = self.window_bits;

    match available.cmp(&count) {
      Ordering::Less => {
        for _byte in 0..BYTES_IN_A_WORD {
          let Ok(byte) = self.back_byte();

          window |= byte.wrapping_shl(available);
          available = available.wrapping_add(SYMBOL_BITS);

          match available.cmp(&WINDOW_FULL) {
            Ordering::Greater => break,
            Ordering::Less | Ordering::Equal => {}
          }
        }
      }
      Ordering::Equal | Ordering::Greater => {}
    }

    let taken = window & 1u32.wrapping_shl(count).wrapping_sub(1);
    let Ok(spent) = signed(count);

    self.window = window.wrapping_shr(count);
    self.window_bits = available.wrapping_sub(count);
    self.bits_total = self.bits_total.wrapping_add(spent);

    Ok(taken)
  }

  pub(crate) fn skip_to_end(&mut self, total: i32) -> Result<(), Never> {
    let Ok(told) = self.tell();

    self.bits_total = self.bits_total.wrapping_add(total.wrapping_sub(told));

    Ok(())
  }

  pub(crate) fn tell(&self) -> Result<i32, Never> {
    let Ok(width) = significant_bits(self.range);
    let Ok(width_2) = signed(width);

    Ok(self.bits_total.wrapping_sub(width_2))
  }

  pub(crate) fn tell_frac(&self) -> Result<u32, Never> {
    let bits = self.bits_total.cast_unsigned().wrapping_shl(BITRES);
    let Ok(mut length) = significant_bits(self.range);
    let mut scaled = self.range.wrapping_shr(length.wrapping_sub(16));

    for _step in 0..BITRES {
      scaled = scaled.wrapping_mul(scaled).wrapping_shr(15);

      let carry = scaled.wrapping_shr(16);

      length = length.wrapping_shl(1) | carry;
      scaled = scaled.wrapping_shr(carry);
    }

    Ok(bits.wrapping_sub(length))
  }

  pub(crate) fn storage(&self) -> Result<u32, Never> {
    Ok(self.storage)
  }

  pub(crate) fn shrink_storage(&mut self, by: u32) -> Result<(), Never> {
    self.storage = self.storage.saturating_sub(by);

    Ok(())
  }

  pub(crate) fn range(&self) -> Result<u32, Never> {
    Ok(self.range)
  }
}
