//! Fixed-point NLSF decoding, NLSF to LPC conversion and filter stabilisation
//! (RFC 6716 section 4.2.7.5, with the RFC 8251 sections 6 and 7 overflow fixes).
//!
//! The section is written in the reference decoder's fixed-point primitives, and they are
//! named here as it names them. `SMULWB` multiplies by the low sixteen bits of its second
//! operand, so that operand is a sixteen-bit value and the truncation is said where it is
//! made; `SMULWW` and the 64-bit roundings keep the low thirty-two bits of a wider product,
//! which is what the reference decoder's casts do. The rest is 32-bit arithmetic that the RFC
//! defines modulo two to the thirty-second, so it wraps, except where the RFC itself says to
//! saturate. The stabilisation loops are the RFC's: twenty rounds of moving the closest pair
//! apart, and sixteen of bandwidth expansion.

use std::cmp::Ordering;

use std::cell::Cell;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use super::tables::{
  COS_Q12, LSF_ORDER_NBMB, LSF_ORDER_WB, NLSF_CB_NBMB, NLSF_CB_WB, NLSF_MIN_SPACING_NBMB,
  NLSF_MIN_SPACING_WB, NLSF_PRED_NBMB, NLSF_PRED_WB, NLSF_PSEL_NBMB, NLSF_PSEL_WB,
};

const QA: u32 = 24;
const A_LIMIT: i32 = 16_773_022;
const A_MIN: i32 = A_LIMIT.wrapping_neg();
const MIN_INVERSE_GAIN: i32 = 107_374;
const STABILISING_ROUNDS: u32 = 20;
const EXPANSION_ROUNDS: u32 = 16;
const LIMITING_ROUNDS: u32 = 10;
const RECIPROCAL_NUMERATOR: i32 = i32::MAX >> 2;
const ONE_Q29: i32 = 1 << 29;
const ONE_Q30: i32 = 1 << 30;
const ONE_Q16_WIDE: i64 = 1 << 16;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Codebook {
  NarrowMedium,
  Wide,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stability {
  Stable,
  Unstable,
}

pub(crate) fn narrow(value: i32) -> Result<i16, Never> {
  fitted::<i32, i16>(value.wrapping_shl(16).wrapping_shr(16))
}

pub(crate) fn low32(value: i64) -> Result<i32, Never> {
  fitted::<i64, i32>(value.wrapping_shl(32).wrapping_shr(32))
}

pub(crate) fn smulwb(wide: i32, low: i16) -> Result<i32, Never> {
  low32(
    i64::from(wide)
      .wrapping_mul(i64::from(low))
      .wrapping_shr(16),
  )
}

pub(crate) fn smulwb_low(
  crate::quantities::WideWord(wide): crate::quantities::WideWord,
  crate::quantities::LowWord(low): crate::quantities::LowWord,
) -> Result<i32, Never> {
  let Ok(low_2) = narrow(low);

  smulwb(wide, low_2)
}

#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  allow(
    explicit024_no_swappable_params,
    reason = "a product is commutative, so its two factors taken in either order are the same call"
  )
)]
pub(crate) fn smulww(a: i32, b: i32) -> Result<i32, Never> {
  low32(i64::from(a).wrapping_mul(i64::from(b)).wrapping_shr(16))
}

#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  allow(
    explicit024_no_swappable_params,
    reason = "a product is commutative, so its two factors taken in either order are the same call"
  )
)]
pub(crate) fn smmul(a: i32, b: i32) -> Result<i32, Never> {
  low32(i64::from(a).wrapping_mul(i64::from(b)).wrapping_shr(32))
}

pub(crate) fn rshift_round(value: i32, shift: u32) -> Result<i32, Never> {
  Ok(match shift {
    1 => value.wrapping_shr(1).wrapping_add(value & 1),
    _other => value
      .wrapping_shr(shift.wrapping_sub(1))
      .wrapping_add(1)
      .wrapping_shr(1),
  })
}

pub(crate) fn rshift_round64(value: i64, shift: u32) -> Result<i64, Never> {
  Ok(match shift {
    1 => value.wrapping_shr(1).wrapping_add(value & 1),
    _other => value
      .wrapping_shr(shift.wrapping_sub(1))
      .wrapping_add(1)
      .wrapping_shr(1),
  })
}

pub(crate) fn sat16(value: i32) -> Result<i32, Never> {
  Ok(value.clamp(-32768, 32767))
}

fn headroom(value: i32) -> Result<i32, Never> {
  Ok(
    value
      .unsigned_abs()
      .leading_zeros()
      .cast_signed()
      .wrapping_sub(1),
  )
}

fn reciprocal(normalised: i32) -> Result<i32, Never> {
  Ok(
    match RECIPROCAL_NUMERATOR.checked_div(normalised.wrapping_shr(16)) {
      Some(inverse) => inverse,
      None => 0,
    },
  )
}

fn rescaled(
  crate::quantities::Accumulator(result): crate::quantities::Accumulator,
  crate::quantities::LeftShift(lshift): crate::quantities::LeftShift,
) -> Result<i32, Never> {
  match (lshift.cmp(&0), lshift.cmp(&32)) {
    (Ordering::Less | Ordering::Equal, _) => {
      let shift = lshift.wrapping_neg().cast_unsigned();
      let high = i32::MAX.wrapping_shr(shift);
      let low = i32::MIN.wrapping_shr(shift);

      Ok(result.clamp(low, high).wrapping_shl(shift))
    }
    (Ordering::Greater, Ordering::Less) => Ok(result.wrapping_shr(lshift.cast_unsigned())),
    (Ordering::Greater, Ordering::Equal | Ordering::Greater) => Ok(0),
  }
}

pub(crate) fn inverse32_varq(value: i32, q_res: u32) -> Result<i32, Never> {
  let Ok(room) = headroom(value);
  let normalised = value.wrapping_shl(room.cast_unsigned());
  let Ok(inverse) = reciprocal(normalised);
  let Ok(guess) = smulwb_low(
    crate::quantities::WideWord(normalised),
    crate::quantities::LowWord(inverse),
  );
  let error = ONE_Q29.wrapping_sub(guess).wrapping_shl(3);
  let Ok(correction) = low32(
    i64::from(error)
      .wrapping_mul(i64::from(inverse))
      .wrapping_shr(16),
  );
  let result = inverse.wrapping_shl(16).wrapping_add(correction);
  let lshift = 61i32.wrapping_sub(room).wrapping_sub(q_res.cast_signed());

  rescaled(
    crate::quantities::Accumulator(result),
    crate::quantities::LeftShift(lshift),
  )
}

#[derive(Clone, Copy)]
pub(crate) struct Ratio {
  pub(crate) numerator: i32,
  pub(crate) denominator: i32,
}

pub(crate) fn div32_varq(ratio: Ratio, q_res: u32) -> Result<i32, Never> {
  let Ok(numerator_room) = headroom(ratio.numerator);
  let numerator = ratio.numerator.wrapping_shl(numerator_room.cast_unsigned());
  let Ok(denominator_room) = headroom(ratio.denominator);
  let denominator = ratio
    .denominator
    .wrapping_shl(denominator_room.cast_unsigned());
  let Ok(inverse) = reciprocal(denominator);
  let Ok(result) = smulwb_low(
    crate::quantities::WideWord(numerator),
    crate::quantities::LowWord(inverse),
  );
  let Ok(back) = smmul(denominator, result);
  let numerator_2 = numerator.wrapping_sub(back.wrapping_shl(3));
  let Ok(refined) = smulwb_low(
    crate::quantities::WideWord(numerator_2),
    crate::quantities::LowWord(inverse),
  );
  let result_2 = result.wrapping_add(refined);
  let lshift = 29i32
    .wrapping_add(numerator_room)
    .wrapping_sub(denominator_room)
    .wrapping_sub(q_res.cast_signed());

  rescaled(
    crate::quantities::Accumulator(result_2),
    crate::quantities::LeftShift(lshift),
  )
}

fn sqrt_approx(value: i32) -> Result<i32, Never> {
  match value.cmp(&0) {
    Ordering::Less | Ordering::Equal => Ok(0),
    Ordering::Greater => {
      let zeros = value.leading_zeros();
      let fraction = value.rotate_right(24u32.wrapping_sub(zeros)) & 0x7f;
      let start = match zeros & 1 {
        0 => 46214i32,
        _odd => 32768,
      };
      let root = start.wrapping_shr(zeros.wrapping_shr(1));
      let Ok(step) = smulwb_low(
        crate::quantities::WideWord(root),
        crate::quantities::LowWord(fraction.wrapping_mul(213)),
      );

      Ok(root.wrapping_add(step))
    }
  }
}

fn table_at<const N: usize>(table: &[i32; N], place: u32) -> Result<i32, Never> {
  let Ok(at) = index(place);

  Ok(match table.get(at) {
    Some(value) => *value,
    None => 0,
  })
}

pub(crate) fn slice_at(values: &[i32], place: u32) -> Result<i32, Never> {
  let Ok(at) = index(place);

  Ok(match values.get(at) {
    Some(value) => *value,
    None => 0,
  })
}

fn predictor(
  codebook: Codebook,
  crate::quantities::CodebookIndex(first): crate::quantities::CodebookIndex,
  crate::quantities::CoefficientIndex(place): crate::quantities::CoefficientIndex,
) -> Result<i32, Never> {
  let Ok(row) = index(first);
  let Ok(at) = index(place);
  let selected = match codebook {
    Codebook::Wide => NLSF_PSEL_WB
      .get(row)
      .and_then(|row_2| row_2.get(at))
      .copied(),
    Codebook::NarrowMedium => NLSF_PSEL_NBMB
      .get(row)
      .and_then(|row_3| row_3.get(at))
      .copied(),
  };
  let Ok(choice) = index(match selected {
    Some(choice) => choice,
    None => 0,
  });
  let weight = match codebook {
    Codebook::Wide => NLSF_PRED_WB
      .get(choice)
      .and_then(|row_4| row_4.get(at))
      .copied(),
    Codebook::NarrowMedium => NLSF_PRED_NBMB
      .get(choice)
      .and_then(|row_5| row_5.get(at))
      .copied(),
  };

  Ok(match weight {
    Some(weight_2) => weight_2,
    None => 0,
  })
}

fn residuals(indices: &[i32; 17], codebook: Codebook, order: u32) -> Result<[i32; 16], Never> {
  let first = match indices.first() {
    Some(first) => first.cast_unsigned(),
    None => 0,
  };
  let step: i16 = match codebook {
    Codebook::Wide => 9830,
    Codebook::NarrowMedium => 11796,
  };
  let mut residual = [0i32; 16];
  let mut out = 0i32;

  for place in (0..order).rev() {
    let prediction = match place.wrapping_add(1).cmp(&order) {
      Ordering::Less => {
        let Ok(weight) = predictor(
          codebook,
          crate::quantities::CodebookIndex(first),
          crate::quantities::CoefficientIndex(place),
        );

        out.wrapping_mul(weight).wrapping_shr(8)
      }
      Ordering::Equal | Ordering::Greater => 0,
    };
    let Ok(coded) = table_at(indices, place.wrapping_add(1));
    let value = coded.wrapping_shl(10);
    let value_2 = match (value.cmp(&0), value.cmp(&0)) {
      (Ordering::Greater, _) => value.wrapping_sub(102),
      (Ordering::Less | Ordering::Equal, Ordering::Less) => value.wrapping_add(102),
      (Ordering::Less | Ordering::Equal, Ordering::Equal | Ordering::Greater) => value,
    };
    let Ok(scaled) = smulwb(value_2, step);

    out = prediction.wrapping_add(scaled);

    let Ok(at) = index(place);

    match residual.get_mut(at) {
      Some(slot) => *slot = out,
      None => {}
    }
  }

  Ok(residual)
}

pub(crate) fn nlsf_decode(indices: &[i32; 17], codebook: Codebook) -> Result<[i32; 16], Never> {
  let order = match codebook {
    Codebook::Wide => 16u32,
    Codebook::NarrowMedium => 10,
  };
  let first = match indices.first() {
    Some(first) => first.cast_unsigned(),
    None => 0,
  };
  let Ok(row) = index(first);
  let base: &[i32] = match codebook {
    Codebook::Wide => NLSF_CB_WB.get(row).map_or(&[], |row_2| row_2.as_slice()),
    Codebook::NarrowMedium => NLSF_CB_NBMB.get(row).map_or(&[], |row_3| row_3.as_slice()),
  };
  let Ok(residual) = residuals(indices, codebook, order);
  let mut nlsf = [0i32; 16];

  for (place, slot) in (0..order).zip(nlsf.iter_mut()) {
    let Ok(centre) = slice_at(base, place);
    let previous = match place {
      0 => 0,
      _later => {
        let Ok(previous) = slice_at(base, place.wrapping_sub(1));

        previous
      }
    };
    let next = match place.wrapping_add(1).cmp(&order) {
      Ordering::Equal => 256,
      Ordering::Less | Ordering::Greater => {
        let Ok(next) = slice_at(base, place.wrapping_add(1));

        next
      }
    };
    let below = match 1024i32.checked_div(centre.wrapping_sub(previous).max(1)) {
      Some(weight) => weight,
      None => 0,
    };
    let above = match 1024i32.checked_div(next.wrapping_sub(centre).max(1)) {
      Some(weight) => weight,
      None => 0,
    };
    let weight = below.wrapping_add(above).min(32767);
    let Ok(root) = sqrt_approx(weight.wrapping_shl(16));
    let Ok(coded) = table_at(&residual, place);
    let shifted = match coded.wrapping_shl(14).checked_div(root.max(1)) {
      Some(shifted) => shifted,
      None => 0,
    };

    *slot = centre.wrapping_shl(7).wrapping_add(shifted).clamp(0, 32767);
  }

  let spacing: &[i32] = match codebook {
    Codebook::Wide => &NLSF_MIN_SPACING_WB,
    Codebook::NarrowMedium => &NLSF_MIN_SPACING_NBMB,
  };
  let Ok(length) = index(order);
  let Ok(()) = nlsf_stabilize(nlsf.get_mut(..length).map_or(&mut [], |used| used), spacing);

  Ok(nlsf)
}

fn closest(nlsf: &[i32], spacing: &[i32]) -> Result<(i32, u32), Never> {
  let Ok(length) = fitted::<_, u32>(nlsf.len());
  let Ok(first) = slice_at(nlsf, 0);
  let Ok(first_spacing) = slice_at(spacing, 0);
  let mut smallest = first.wrapping_sub(first_spacing);
  let mut at = 0u32;

  for ((place, pair), gap) in (1u32..).zip(nlsf.windows(2)).zip(spacing.iter().skip(1)) {
    let difference = match pair {
      [previous, current] => current.wrapping_sub(previous.wrapping_add(*gap)),
      _uneven => 0,
    };

    match difference.cmp(&smallest) {
      Ordering::Less => {
        smallest = difference;
        at = place;
      }
      Ordering::Equal | Ordering::Greater => {}
    }
  }

  let Ok(last) = slice_at(nlsf, length.wrapping_sub(1));
  let Ok(last_spacing) = slice_at(spacing, length);
  let difference = 32768i32.wrapping_sub(last.wrapping_add(last_spacing));

  match difference.cmp(&smallest) {
    Ordering::Less => Ok((difference, length)),
    Ordering::Equal | Ordering::Greater => Ok((smallest, at)),
  }
}

pub(crate) fn set_at(values: &mut [i32], place: u32, value: i32) -> Result<(), Never> {
  let Ok(at) = index(place);

  match values.get_mut(at) {
    Some(slot) => *slot = value,
    None => {}
  }

  Ok(())
}

fn move_apart(nlsf: &mut [i32], spacing: &[i32], at: u32) -> Result<(), Never> {
  let Ok(length) = fitted::<_, u32>(nlsf.len());
  let Ok(here) = slice_at(spacing, at);
  let Ok(split) = index(at);

  match (at, at.cmp(&length)) {
    (0, _) => set_at(nlsf, 0, here),
    (_, Ordering::Equal) => set_at(nlsf, length.wrapping_sub(1), 32768i32.wrapping_sub(here)),
    (_, Ordering::Less | Ordering::Greater) => {
      let Ok(count) = index(length.wrapping_sub(at));
      let below: i32 = spacing.iter().take(split).sum();
      let above: i32 = spacing
        .iter()
        .skip(split.saturating_add(1))
        .take(count)
        .sum();
      let min_center = below.wrapping_add(here.wrapping_shr(1));
      let max_center = 32768i32
        .wrapping_sub(above)
        .wrapping_sub(here.wrapping_shr(1));
      let Ok(previous) = slice_at(nlsf, at.wrapping_sub(1));
      let Ok(current) = slice_at(nlsf, at);
      let Ok(centre) = rshift_round(previous.wrapping_add(current), 1);
      let centre_2 = match min_center.cmp(&max_center) {
        Ordering::Greater => centre.clamp(max_center, min_center),
        Ordering::Less | Ordering::Equal => centre.clamp(min_center, max_center),
      };
      let lower = centre_2.wrapping_sub(here.wrapping_shr(1));
      let Ok(()) = set_at(nlsf, at.wrapping_sub(1), lower);

      set_at(nlsf, at, lower.wrapping_add(here))
    }
  }
}

pub(crate) fn nlsf_stabilize(nlsf: &mut [i32], spacing: &[i32]) -> Result<(), Never> {
  for _round in 0..STABILISING_ROUNDS {
    let Ok((smallest, at)) = closest(nlsf, spacing);

    match smallest.cmp(&0) {
      Ordering::Equal | Ordering::Greater => return Ok(()),
      Ordering::Less => {}
    }

    let Ok(()) = move_apart(nlsf, spacing, at);
  }

  nlsf.sort_unstable();

  let Ok(length) = fitted::<_, u32>(nlsf.len());
  let Ok(first_spacing) = slice_at(spacing, 0);

  match nlsf.first_mut() {
    Some(first) => *first = (*first).max(first_spacing),
    None => {}
  }

  let cells = Cell::from_mut(nlsf).as_slice_of_cells();

  for (pair, gap) in cells.windows(2).zip(spacing.iter().skip(1)) {
    match pair {
      [previous, current] => {
        let Ok(floor) = sat16(previous.get().wrapping_add(*gap));

        current.set(current.get().max(floor));
      }
      _uneven => {}
    }
  }

  let Ok(last_spacing) = slice_at(spacing, length);

  match cells.last() {
    Some(last) => last.set(last.get().min(32768i32.wrapping_sub(last_spacing))),
    None => {}
  }

  for (pair, gap) in cells.windows(2).zip(spacing.iter().skip(1)).rev() {
    match pair {
      [previous, current] => previous.set(previous.get().min(current.get().wrapping_sub(*gap))),
      _uneven => {}
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
pub(crate) enum Width {
  Sixteen,
  ThirtyTwo,
}

pub(crate) fn bwexpander(ar: &mut [i32], chirp: i32, width: Width) -> Result<(), Never> {
  let mut chirp_2 = chirp;
  let less_one = chirp_2.wrapping_sub(65536);

  for value in ar.iter_mut() {
    let Ok(scaled) = match width {
      Width::Sixteen => rshift_round(chirp_2.wrapping_mul(*value), 16),
      Width::ThirtyTwo => smulww(chirp_2, *value),
    };
    let Ok(step) = rshift_round(chirp_2.wrapping_mul(less_one), 16);

    *value = scaled;
    chirp_2 = chirp_2.wrapping_add(step);
  }

  Ok(())
}

fn reflection_gain(
  crate::quantities::InverseGain(inverse_gain): crate::quantities::InverseGain,
  crate::quantities::ReflectionCoefficient(coefficient): crate::quantities::ReflectionCoefficient,
) -> Result<(i32, i32, i32), Never> {
  let rc_q31 = coefficient
    .wrapping_shl(31u32.wrapping_sub(QA))
    .wrapping_neg();
  let Ok(squared) = smmul(rc_q31, rc_q31);
  let rc_mult1 = ONE_Q30.wrapping_sub(squared);
  let Ok(scaled) = smmul(inverse_gain, rc_mult1);

  Ok((rc_q31, rc_mult1, scaled.wrapping_shl(2)))
}

fn mul32_frac_q31(
  crate::quantities::SampleWord(x): crate::quantities::SampleWord,
  crate::quantities::Q31Coefficient(rc_q31): crate::quantities::Q31Coefficient,
) -> Result<i32, Never> {
  let Ok(rounded) = rshift_round64(i64::from(x).wrapping_mul(i64::from(rc_q31)), 31);

  low32(rounded)
}

enum Reflected {
  Stable(i32),
  Unstable,
}

fn reflected(
  crate::quantities::OwnCoefficient(own): crate::quantities::OwnCoefficient,
  crate::quantities::OtherCoefficient(other): crate::quantities::OtherCoefficient,
  reflection: (i32, i32, u32),
) -> Result<Reflected, Never> {
  let (rc_q31, rc_mult2, mult2q) = reflection;
  let Ok(taken) = mul32_frac_q31(
    crate::quantities::SampleWord(other),
    crate::quantities::Q31Coefficient(rc_q31),
  );
  let Ok(value) = rshift_round64(
    i64::from(own.saturating_sub(taken)).wrapping_mul(i64::from(rc_mult2)),
    mult2q,
  );

  Ok(match i32::try_from(value) {
    Ok(coefficient) => Reflected::Stable(coefficient),
    Err(_unstable_coefficient) => Reflected::Unstable,
  })
}

fn step_down(a: &mut [i32; 16], order: u32, inverse_gain: i32) -> Result<Option<i32>, Never> {
  let mut gain = inverse_gain;

  for k in (1..order).rev() {
    let Ok(coefficient) = table_at(a, k);

    match coefficient {
      A_MIN..=A_LIMIT => {}
      _outside => return Ok(None),
    }

    let Ok((rc_q31, rc_mult1, scaled)) = reflection_gain(
      crate::quantities::InverseGain(gain),
      crate::quantities::ReflectionCoefficient(coefficient),
    );

    gain = scaled;

    match gain.cmp(&MIN_INVERSE_GAIN) {
      Ordering::Less => return Ok(None),
      Ordering::Equal | Ordering::Greater => {}
    }

    let mult2q = 32u32.wrapping_sub(rc_mult1.unsigned_abs().leading_zeros());
    let Ok(rc_mult2) = inverse32_varq(rc_mult1, mult2q.wrapping_add(30));

    for n in 0..k.wrapping_add(1).wrapping_shr(1) {
      let mirror = k.wrapping_sub(n).wrapping_sub(1);
      let Ok(t1) = table_at(a, n);
      let Ok(t2) = table_at(a, mirror);
      let Ok(first) = reflected(
        crate::quantities::OwnCoefficient(t1),
        crate::quantities::OtherCoefficient(t2),
        (rc_q31, rc_mult2, mult2q),
      );
      let Ok(second) = reflected(
        crate::quantities::OwnCoefficient(t2),
        crate::quantities::OtherCoefficient(t1),
        (rc_q31, rc_mult2, mult2q),
      );

      match (first, second) {
        (Reflected::Stable(first_2), Reflected::Stable(second_2)) => {
          let Ok(()) = set_at(a, n, first_2);
          let Ok(()) = set_at(a, mirror, second_2);
        }
        (Reflected::Unstable, _) | (_, Reflected::Unstable) => return Ok(None),
      }
    }
  }

  Ok(Some(gain))
}

fn lpc_inverse_pred_gain(a_q12: &[i32]) -> Result<Stability, Never> {
  let Ok(order) = fitted::<_, u32>(a_q12.len());
  let mut a = [0i32; 16];
  let mut dc = 0i32;

  for (slot, value) in a.iter_mut().zip(a_q12.iter()) {
    dc = dc.wrapping_add(*value);
    *slot = value.wrapping_shl(QA.wrapping_sub(12));
  }

  match dc.cmp(&4096) {
    Ordering::Equal | Ordering::Greater => return Ok(Stability::Unstable),
    Ordering::Less => {}
  }

  let Ok(stepped) = step_down(&mut a, order, ONE_Q30);
  let [first, ..] = a;

  match (stepped, first) {
    (Some(gain), A_MIN..=A_LIMIT) => {
      let Ok((_rc, _mult, gain_2)) = reflection_gain(
        crate::quantities::InverseGain(gain),
        crate::quantities::ReflectionCoefficient(first),
      );

      Ok(match gain_2.cmp(&MIN_INVERSE_GAIN) {
        Ordering::Less => Stability::Unstable,
        Ordering::Equal | Ordering::Greater => Stability::Stable,
      })
    }
    _unstable => Ok(Stability::Unstable),
  }
}

fn find_poly(
  c_q17: &[i32; 16],
  crate::quantities::Parity(parity): crate::quantities::Parity,
  crate::quantities::HalfOrder(half): crate::quantities::HalfOrder,
) -> Result<[i64; 10], Never> {
  let Ok(first) = table_at(c_q17, parity);
  let mut out = [
    ONE_Q16_WIDE,
    i64::from(first).wrapping_neg(),
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
  ];
  let cells = Cell::from_mut(out.as_mut_slice()).as_slice_of_cells();

  for k in 1..half {
    let Ok(coefficient) = table_at(c_q17, k.wrapping_mul(2).wrapping_add(parity));
    let ftmp = i64::from(coefficient);
    let Ok(at) = index(k);
    let earlier = match cells.get(at.wrapping_sub(1)) {
      Some(cell) => cell.get(),
      None => 0,
    };
    let here = match cells.get(at) {
      Some(cell) => cell.get(),
      None => 0,
    };

    match cells.get(at.saturating_add(1)) {
      Some(cell) => cell.set(
        earlier
          .wrapping_mul(2)
          .wrapping_sub(ftmp.wrapping_mul(here).wrapping_add(32768).wrapping_shr(16)),
      ),
      None => {}
    }

    let used = match cells.get(..=at) {
      Some(used) => used,
      None => &[],
    };

    for window in used.windows(3).rev() {
      match window {
        [two_before, one_before, current] => {
          current.set(
            current.get().wrapping_add(
              two_before.get().wrapping_sub(
                ftmp
                  .wrapping_mul(one_before.get())
                  .wrapping_add(32768)
                  .wrapping_shr(16),
              ),
            ),
          );
        }
        _uneven => {}
      }
    }

    match cells.get(1) {
      Some(cell) => cell.set(cell.get().wrapping_sub(ftmp)),
      None => {}
    }
  }

  Ok(out)
}

fn cosines(nlsf: &[i32]) -> Result<[i32; 16], Never> {
  let ordering: &[u32] = match nlsf.len() {
    16 => &LSF_ORDER_WB,
    _narrower => &LSF_ORDER_NBMB,
  };
  let mut c_q17 = [0i32; 16];

  for (value, target) in nlsf.iter().zip(ordering.iter()) {
    let step = value.wrapping_shr(8).cast_unsigned();
    let fraction = value & 255;
    let Ok(c0) = table_at(&COS_Q12, step);
    let Ok(c1) = table_at(&COS_Q12, step.wrapping_add(1).min(128));
    let Ok(()) = set_at(
      &mut c_q17,
      *target,
      c0.wrapping_mul(256)
        .wrapping_add(c1.wrapping_sub(c0).wrapping_mul(fraction))
        .wrapping_add(4)
        .wrapping_shr(3),
    );
  }

  Ok(c_q17)
}

fn limited(a32: &mut [i32]) -> Result<Stability, Never> {
  for _round in 0..LIMITING_ROUNDS {
    let mut largest = 0i32;
    let mut at = 0i32;

    for (place, value) in (0i32..).zip(a32.iter()) {
      let magnitude = value.saturating_abs();

      match magnitude.cmp(&largest) {
        Ordering::Greater => {
          largest = magnitude;
          at = place;
        }
        Ordering::Less | Ordering::Equal => {}
      }
    }

    let Ok(largest_2) = rshift_round(largest, 5);

    match largest_2.cmp(&32767) {
      Ordering::Greater => {
        let largest_3 = largest_2.min(163_838);
        let denominator = largest_3.wrapping_mul(at.wrapping_add(1)).wrapping_shr(2);
        let reduction = match largest_3
          .wrapping_sub(32767)
          .wrapping_shl(14)
          .checked_div(denominator)
        {
          Some(reduction) => reduction,
          None => 0,
        };
        let Ok(()) = bwexpander(a32, 65470i32.wrapping_sub(reduction), Width::ThirtyTwo);
      }
      Ordering::Less | Ordering::Equal => return Ok(Stability::Stable),
    }
  }

  Ok(Stability::Unstable)
}

pub(crate) fn nlsf2a(nlsf: &[i32]) -> Result<[i32; 16], Never> {
  let Ok(order) = fitted::<_, u32>(nlsf.len());
  let half = order.wrapping_shr(1);
  let Ok(c_q17) = cosines(nlsf);
  let Ok(p) = find_poly(
    &c_q17,
    crate::quantities::Parity(0),
    crate::quantities::HalfOrder(half),
  );
  let Ok(q) = find_poly(
    &c_q17,
    crate::quantities::Parity(1),
    crate::quantities::HalfOrder(half),
  );
  let mut a32 = [0i32; 16];

  for ((k, (p0, p1)), (q0, q1)) in (0..half)
    .zip(p.iter().zip(p.iter().skip(1)))
    .zip(q.iter().zip(q.iter().skip(1)))
  {
    let ptmp = p1.wrapping_add(*p0);
    let qtmp = q1.wrapping_sub(*q0);
    let Ok(front) = low32(qtmp.wrapping_neg().wrapping_sub(ptmp));
    let Ok(back) = low32(qtmp.wrapping_sub(ptmp));
    let Ok(()) = set_at(&mut a32, k, front);
    let Ok(()) = set_at(&mut a32, order.wrapping_sub(k).wrapping_sub(1), back);
  }

  let Ok(length) = index(order);
  let a32_2 = match a32.get_mut(..length) {
    Some(used) => used,
    None => &mut [],
  };
  let mut a_q12 = [0i32; 16];
  let Ok(stability) = limited(a32_2);

  for (slot, value) in a_q12.iter_mut().zip(a32_2.iter_mut()) {
    let Ok(rounded) = rshift_round(*value, 5);

    match stability {
      Stability::Stable => *slot = rounded,
      Stability::Unstable => {
        let Ok(clipped) = sat16(rounded);

        *slot = clipped;
        *value = clipped.wrapping_shl(5);
      }
    }
  }

  for round in 0..EXPANSION_ROUNDS {
    let Ok(stability_2) = lpc_inverse_pred_gain(a_q12.get(..length).map_or(&[], |used| used));

    match stability_2 {
      Stability::Stable => break,
      Stability::Unstable => {
        let Ok(()) = bwexpander(
          a32_2,
          65536i32.wrapping_sub(2i32.wrapping_shl(round)),
          Width::ThirtyTwo,
        );

        for (slot, value) in a_q12.iter_mut().zip(a32_2.iter()) {
          let Ok(rounded) = rshift_round(*value, 5);

          *slot = rounded;
        }
      }
    }
  }

  Ok(a_q12)
}
