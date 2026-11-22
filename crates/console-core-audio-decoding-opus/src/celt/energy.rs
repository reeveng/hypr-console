//! Band energy decoding: Laplace-coded coarse energy, fine energy and the final refinement
//! (RFC 6716 section 4.3.2).
//!
//! The Laplace decoder walks outward from zero one magnitude at a time, and each step takes at
//! least four from a cumulative frequency that never reaches two to the fifteenth, so the walk
//! is bounded by that count. Its arithmetic is the section's, unsigned and 32 bits wide, and
//! never leaves fifteen bits.

use std::cmp::Ordering;

use console_core_never::Never;
use console_core_number_conversion::{index, narrow_f32};

use super::Layout;
use super::rate::{MAX_FINE_BITS, of};
use super::tables::{BANDS, BETA_COEF, BETA_INTRA, E_PROB_MODEL, PRED_COEF, SMALL_ENERGY};
use crate::range::{Bit, RangeDecoder, Span};

const LAPLACE_MINP: u32 = 1;
const LAPLACE_NMIN: u32 = 16;
const LAPLACE_BITS: u32 = 15;
const LAPLACE_TOTAL: u32 = 1 << LAPLACE_BITS;
const UNMODELLED: [u8; 42] = [0; 42];

#[derive(Clone, Copy)]
struct Laplace {
  zero: u32,
  decay: u32,
}

fn first_step(model: Laplace) -> Result<u32, Never> {
  let spread = LAPLACE_TOTAL
    .wrapping_sub(LAPLACE_MINP.wrapping_mul(2u32.wrapping_mul(LAPLACE_NMIN)))
    .wrapping_sub(model.zero);

  Ok(
    spread
      .wrapping_mul(16384u32.wrapping_sub(model.decay))
      .wrapping_shr(15),
  )
}

fn laplace_decode(dec: &mut RangeDecoder<'_>, model: Laplace) -> Result<i32, Never> {
  let Ok(found) = dec.decode_bin(LAPLACE_BITS);
  let mut value = 0i32;
  let mut low = 0u32;
  let mut share = model.zero;

  match found.cmp(&share) {
    Ordering::Equal | Ordering::Greater => {
      value = 1;
      low = share;

      let Ok(step) = first_step(model);

      share = step.wrapping_add(LAPLACE_MINP);

      for _step in 0..LAPLACE_TOTAL {
        match (
          share.cmp(&LAPLACE_MINP),
          found.cmp(&low.wrapping_add(share.wrapping_mul(2))),
        ) {
          (Ordering::Greater, Ordering::Equal | Ordering::Greater) => {
            share = share.wrapping_mul(2);
            low = low.wrapping_add(share);
            share = share
              .wrapping_sub(2u32.wrapping_mul(LAPLACE_MINP))
              .wrapping_mul(model.decay)
              .wrapping_shr(15)
              .wrapping_add(LAPLACE_MINP);
            value = value.wrapping_add(1);
          }
          _reached => break,
        }
      }

      match share.cmp(&LAPLACE_MINP) {
        Ordering::Less | Ordering::Equal => {
          let further = found.wrapping_sub(low).wrapping_shr(1);

          value = value.wrapping_add(further.cast_signed());
          low = low.wrapping_add(further.wrapping_mul(2).wrapping_mul(LAPLACE_MINP));
        }
        Ordering::Greater => {}
      }

      match found.cmp(&low.wrapping_add(share)) {
        Ordering::Less => value = value.wrapping_neg(),
        Ordering::Equal | Ordering::Greater => low = low.wrapping_add(share),
      }
    }
    Ordering::Less => {}
  }

  let Ok(()) = dec.update_bin(
    Span {
      low,
      high: low.wrapping_add(share).min(LAPLACE_TOTAL),
    },
    LAPLACE_BITS,
  );

  Ok(value)
}

fn coarse_step(
  dec: &mut RangeDecoder<'_>,
  prob: &[u8; 42],
  band: u32,
  budget: i32,
) -> Result<i32, Never> {
  let Ok(told) = dec.tell();
  let left = budget.wrapping_sub(told);

  match (left.cmp(&15), left.cmp(&2), left.cmp(&1)) {
    (Ordering::Equal | Ordering::Greater, _, _) => {
      let Ok(at) = index(band.min(20).wrapping_mul(2));
      let mut pair = prob.iter().skip(at).map(|value| u32::from(*value));
      let zero = match pair.next() {
        Some(zero) => zero,
        None => 0,
      };
      let decay = match pair.next() {
        Some(decay) => decay,
        None => 0,
      };

      laplace_decode(
        dec,
        Laplace {
          zero: zero.wrapping_shl(7),
          decay: decay.wrapping_shl(6),
        },
      )
    }
    (Ordering::Less, Ordering::Equal | Ordering::Greater, _) => {
      let Ok(symbol) = dec.symbol(&SMALL_ENERGY, 2);
      let symbol_2 = symbol.cast_signed();

      Ok(symbol_2.wrapping_shr(1) ^ (symbol_2 & 1).wrapping_neg())
    }
    (Ordering::Less, Ordering::Less, Ordering::Equal | Ordering::Greater) => {
      let Ok(bit) = dec.bit_logp(1);

      Ok(match bit {
        Bit::One => -1,
        Bit::Zero => 0,
      })
    }
    (Ordering::Less, Ordering::Less, Ordering::Less) => Ok(-1),
  }
}

pub(crate) fn unquant_coarse_energy(
  layout: Layout,
  old_e: &mut [f32; 42],
  intra: Bit,
  dec: &mut RangeDecoder<'_>,
  lm: u32,
) -> Result<(), Never> {
  let Ok(row) = index(lm);
  let (inter_prob, intra_prob) = match E_PROB_MODEL.get(row) {
    Some([inter, intra_2]) => (inter, intra_2),
    None => (&UNMODELLED, &UNMODELLED),
  };
  let (prob, coef, beta) = match intra {
    Bit::One => (intra_prob, 0.0, BETA_INTRA),
    Bit::Zero => (
      inter_prob,
      match PRED_COEF.get(row) {
        Some(coef) => *coef,
        None => 0.0,
      },
      match BETA_COEF.get(row) {
        Some(beta) => *beta,
        None => 0.0,
      },
    ),
  };
  let Ok(storage) = dec.storage();
  let budget = storage.wrapping_mul(8).cast_signed();
  let mut carried = [0f32; 2];
  let Ok(channels) = index(layout.channels);

  for band in layout.start..layout.end {
    for (channel, carry) in (0u32..).zip(carried.iter_mut()).take(channels) {
      let Ok(step) = coarse_step(dec, prob, band, budget);
      let Ok(quantum) = narrow_f32(f64::from(step));
      let Ok(at) = index(band.wrapping_add(channel.wrapping_mul(BANDS)));

      match old_e.get_mut(at) {
        Some(energy) => {
          *energy = energy.max(-9.0);
          *energy = coef * *energy + *carry + quantum;
        }
        None => {}
      }

      *carry = *carry + quantum - beta * quantum;
    }
  }

  Ok(())
}

pub(crate) fn unquant_fine_energy(
  layout: Layout,
  old_e: &mut [f32; 42],
  fine_quant: &[i32; 21],
  dec: &mut RangeDecoder<'_>,
) -> Result<(), Never> {
  for band in layout.start..layout.end {
    let Ok(fine) = of(fine_quant, band);

    match fine.cmp(&0) {
      Ordering::Greater => {}
      Ordering::Less | Ordering::Equal => continue,
    }

    for channel in 0..layout.channels {
      let Ok(bits) = dec.bits(fine.cast_unsigned());
      let Ok(quantum) = narrow_f32(f64::from(bits));
      let Ok(scale) = narrow_f32(f64::from(
        1i32.wrapping_shl(14i32.wrapping_sub(fine).cast_unsigned()),
      ));
      let offset = (quantum + 0.5) * scale / 16384.0 - 0.5;
      let Ok(at) = index(band.wrapping_add(channel.wrapping_mul(BANDS)));

      match old_e.get_mut(at) {
        Some(energy) => *energy += offset,
        None => {}
      }
    }
  }

  Ok(())
}

pub(crate) struct Fine<'a> {
  pub(crate) quant: &'a [i32; 21],
  pub(crate) priority: &'a [i32; 21],
}

pub(crate) fn unquant_energy_finalise(
  layout: Layout,
  old_e: &mut [f32; 42],
  fine: Fine<'_>,
  bits_left: i32,
  dec: &mut RangeDecoder<'_>,
) -> Result<(), Never> {
  let mut left = bits_left;
  let wide = layout.channels.cast_signed();

  for priority in 0..2 {
    'energy_bands: for band in layout.start..layout.end {
      match left.cmp(&wide) {
        Ordering::Equal | Ordering::Greater => {}
        Ordering::Less => break 'energy_bands,
      }

      let Ok(quant) = of(fine.quant, band);
      let Ok(chosen) = of(fine.priority, band);

      match (quant.cmp(&MAX_FINE_BITS), chosen.cmp(&priority)) {
        (Ordering::Less, Ordering::Equal) => {}
        _skip => continue 'energy_bands,
      }

      for channel in 0..layout.channels {
        let Ok(bit) = dec.bits(1);
        let Ok(quantum) = narrow_f32(f64::from(bit));
        let Ok(scale) = narrow_f32(f64::from(
          1i32.wrapping_shl(14i32.wrapping_sub(quant).wrapping_sub(1).cast_unsigned()),
        ));
        let offset = (quantum - 0.5) * scale / 16384.0;
        let Ok(at) = index(band.wrapping_add(channel.wrapping_mul(BANDS)));

        match old_e.get_mut(at) {
          Some(energy) => *energy += offset,
          None => {}
        }

        left = left.wrapping_sub(1);
      }
    }
  }

  Ok(())
}
