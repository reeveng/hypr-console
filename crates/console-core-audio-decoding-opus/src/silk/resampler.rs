//! SILK output resampler (RFC 6716 section 4.2.9: a non-normative filter with a normative delay
//! budget).
//!
//! A windowed-sinc polyphase interpolator whose group delay matches the delay allocation of
//! RFC 6716 Table 54, so that the SILK output lines up with the CELT layer in Hybrid mode. The
//! output is always 48 kHz and SILK's internal rate is always lower, so every resampler here
//! interpolates: each phase's taps are the least-squares fit to an ideal low-pass edge at 0.94
//! of the input band, solved once by Gauss-Jordan elimination with partial pivoting when the
//! resampler is made. A pivot is the last of the largest in its column, which is the order the
//! filters were first designed in, so the taps are the same to the last bit.

use std::cmp::Ordering;

use std::f64::consts::PI;
use std::num::NonZeroU64;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, narrow_f32, toward_zero_i64, whole_i64};

const OUTPUT_RATE: u32 = 48_000;
const PAST: i64 = 24;
const GUARD: i64 = 4;
const PASS_EDGE: f64 = 0.94;
const SHOULDER: f64 = 0.6;
const SHOULDER_WEIGHT: f64 = 0.05;
const RIDGE: f64 = 1e-9;
const TINY: f64 = 1e-12;
const EUCLID_STEPS: u32 = 64;

#[derive(Clone)]
struct Phase {
  base: i64,
  taps: Vec<f32>,
}

#[derive(Clone)]
pub(crate) struct Resampler {
  input_rate: u32,
  phases: Vec<Phase>,
  period: NonZeroU64,
  advance: i64,
  future: i64,
  history: Vec<f32>,
  history_start: i64,
  taken: i64,
  made: u64,
}

fn common_divisor(
  crate::quantities::InputRate(first): crate::quantities::InputRate,
  crate::quantities::OutputRate(second): crate::quantities::OutputRate,
) -> Result<u32, Never> {
  let mut larger = first;
  let mut smaller = second;

  for _step in 0..EUCLID_STEPS {
    match std::num::NonZeroU32::new(smaller) {
      Some(divisor) => {
        let rest = larger % divisor;

        larger = smaller;
        smaller = rest;
      }
      None => break,
    }
  }

  Ok(larger)
}

fn integral(
  crate::quantities::SamplePosition(at): crate::quantities::SamplePosition,
  crate::quantities::RangeStart(from): crate::quantities::RangeStart,
  crate::quantities::RangeEnd(to): crate::quantities::RangeEnd,
) -> Result<f64, Never> {
  Ok(match at.abs().partial_cmp(&TINY) {
    Some(Ordering::Less) => to - from,
    Some(Ordering::Equal | Ordering::Greater) | None => {
      ((to * at).sin() - (from * at).sin()) / at
    }
  })
}

fn response(
  crate::quantities::SamplePosition(at): crate::quantities::SamplePosition,
) -> Result<f64, Never> {
  let pass = PASS_EDGE * PI;
  let Ok(flat) = integral(
    crate::quantities::SamplePosition(at),
    crate::quantities::RangeStart(0.0),
    crate::quantities::RangeEnd(SHOULDER * pass),
  );
  let Ok(shoulder) = integral(
    crate::quantities::SamplePosition(at),
    crate::quantities::RangeStart(SHOULDER * pass),
    crate::quantities::RangeEnd(pass),
  );

  Ok(flat + SHOULDER_WEIGHT * shoulder)
}

fn system(
  offsets: &[f64],
  crate::quantities::SamplePosition(fraction): crate::quantities::SamplePosition,
) -> Result<Vec<Vec<f64>>, Never> {
  let mut rows = Vec::new();

  for (row, at) in (0u32..).zip(offsets.iter()) {
    let mut equation = Vec::new();

    for (column, other) in (0u32..).zip(offsets.iter()) {
      let Ok(value) = response(crate::quantities::SamplePosition(at - other));
      let ridge = match row.cmp(&column) {
        Ordering::Equal => RIDGE,
        Ordering::Less | Ordering::Greater => 0.0,
      };

      equation.push(value + ridge);
    }

    let Ok(wanted) = response(crate::quantities::SamplePosition(at - fraction));

    equation.push(wanted);
    rows.push(equation);
  }

  Ok(rows)
}

fn entry(row: &[f64], column: u32) -> Result<f64, Never> {
  let Ok(at) = index(column);

  Ok(match row.get(at) {
    Some(value) => *value,
    None => 0.0,
  })
}

fn pivot(rows: &mut [Vec<f64>], column: u32) -> Result<(), Never> {
  let Ok(from) = index(column);
  let mut largest = 0u32;
  let mut size = f64::NEG_INFINITY;

  for (offset, row) in (0u32..).zip(rows.iter().skip(from)) {
    let Ok(magnitude) = entry(row, column);
    let magnitude_2 = magnitude.abs();

    match magnitude_2.total_cmp(&size) {
      Ordering::Equal | Ordering::Greater => {
        largest = offset;
        size = magnitude_2;
      }
      Ordering::Less => {}
    }
  }

  let rest = match rows.get_mut(from..) {
    Some(rest) => rest,
    None => &mut [],
  };

  match (largest.checked_sub(1), rest.split_first_mut()) {
    (Some(further), Some((first, others))) => {
      let Ok(at) = index(further);

      match others.get_mut(at) {
        Some(other) => std::mem::swap(first, other),
        None => {}
      }
    }
    (None, _) | (_, None) => {}
  }

  Ok(())
}

fn eliminate(rows: &mut [Vec<f64>], column: u32) -> Result<(), Never> {
  let Ok(from) = index(column);
  let chosen = match rows.get(from) {
    Some(row) => row.clone(),
    None => Vec::new(),
  };
  let Ok(divisor) = entry(&chosen, column);

  for (row, equation) in (0u32..).zip(rows.iter_mut()) {
    match row.cmp(&column) {
      Ordering::Equal => continue,
      Ordering::Less | Ordering::Greater => {}
    }

    let Ok(lead) = entry(equation, column);
    let factor = lead / divisor;

    match factor.classify() {
      std::num::FpCategory::Zero => continue,
      std::num::FpCategory::Normal
      | std::num::FpCategory::Subnormal
      | std::num::FpCategory::Infinite
      | std::num::FpCategory::Nan => {}
    }

    for (slot, held) in equation.iter_mut().skip(from).zip(chosen.iter().skip(from)) {
      *slot -= factor * held;
    }
  }

  Ok(())
}

fn design(future: i64, fraction: crate::quantities::SamplePosition) -> Result<Vec<f32>, Never> {
  let mut offsets = Vec::new();

  for offset in PAST.wrapping_neg().wrapping_add(1)..=future {
    let Ok(held) = fitted_offset(offset);

    offsets.push(held);
  }

  let Ok(mut rows) = system(&offsets, fraction);
  let Ok(count) = fitted::<_, u32>(offsets.len());

  for column in 0..count {
    let Ok(()) = pivot(&mut rows, column);
    let Ok(()) = eliminate(&mut rows, column);
  }

  let mut taps = Vec::new();

  for (column, row) in (0u32..).zip(rows.iter()) {
    let Ok(diagonal) = entry(row, column);
    let solved = match row.last() {
      Some(last) => *last,
      None => 0.0,
    };
    let Ok(tap) = narrow_f32(solved / diagonal);

    taps.push(tap);
  }

  Ok(taps)
}

fn fitted_offset(offset: i64) -> Result<f64, Never> {
  let Ok(small) = fitted::<i64, i32>(offset);

  Ok(f64::from(small))
}

impl Resampler {
  pub(crate) fn new(input_rate: u32) -> Result<Resampler, Never> {
    let delay = match input_rate {
      8000 => 0.538,
      12_000 => 0.692,
      _wideband => 0.706,
    };
    let delayed = delay * f64::from(input_rate) / 1000.0;
    let Ok(whole) = toward_zero_i64(delayed.floor());
    let future = whole.clamp(1, 11);
    let Ok(delay_steps) = whole_i64(delayed * f64::from(OUTPUT_RATE));
    let Ok(divisor) = common_divisor(
      crate::quantities::InputRate(input_rate),
      crate::quantities::OutputRate(OUTPUT_RATE),
    );
    let cycle = match OUTPUT_RATE.checked_div(divisor) {
      Some(cycle) => cycle,
      None => 1,
    };
    let period = match NonZeroU64::new(u64::from(cycle)) {
      Some(period) => period,
      None => NonZeroU64::MIN,
    };
    let mut phases = Vec::new();

    for step in 0..i64::from(cycle) {
      let place = step
        .wrapping_mul(i64::from(input_rate))
        .wrapping_sub(delay_steps);
      let base = place.div_euclid(i64::from(OUTPUT_RATE));
      let Ok(remainder) = fitted_offset(place.rem_euclid(i64::from(OUTPUT_RATE)));
      let Ok(taps) = design(
        future,
        crate::quantities::SamplePosition(remainder / f64::from(OUTPUT_RATE)),
      );

      phases.push(Phase { base, taps });
    }

    let kept = PAST.wrapping_add(future).wrapping_add(GUARD);
    let Ok(length) = index(kept);
    let scaled = i64::from(input_rate).wrapping_mul(i64::from(cycle));
    let advance = match scaled.checked_div(i64::from(OUTPUT_RATE)) {
      Some(advance) => advance,
      None => 0,
    };

    Ok(Resampler {
      input_rate,
      phases,
      period,
      advance,
      future,
      history: vec![0.0; length.min(const { 24 + 11 + 4 })],
      history_start: kept.wrapping_neg(),
      taken: 0,
      made: 0,
    })
  }

  pub(crate) fn input_rate(&self) -> Result<u32, Never> {
    Ok(self.input_rate)
  }

  pub(crate) fn process(&mut self, input: &[i16], out: &mut Vec<f32>) -> Result<(), Never> {
    self
      .history
      .extend(input.iter().map(|sample| f32::from(*sample)));

    let Ok(arrived) = fitted::<_, i64>(input.len());

    self.taken = self.taken.wrapping_add(arrived);

    let rate = match NonZeroU64::new(u64::from(self.input_rate)) {
      Some(rate) => rate,
      None => NonZeroU64::MIN,
    };
    let total = self
      .taken
      .cast_unsigned()
      .wrapping_mul(u64::from(OUTPUT_RATE))
      / rate;

    for moment in self.made..total {
      let Ok(cycle) = fitted::<u64, i64>(moment / self.period);
      let Ok(slot) = index(moment % self.period);
      let (base, taps) = match self.phases.get(slot) {
        Some(phase) => (phase.base, phase.taps.as_slice()),
        None => (0, [].as_slice()),
      };
      let first = base
        .wrapping_add(cycle.wrapping_mul(self.advance))
        .wrapping_sub(PAST)
        .wrapping_add(1);
      let offset = first.wrapping_sub(self.history_start);
      let Ok(skipped_taps) = index(offset.min(0).wrapping_neg());
      let Ok(skipped_samples) = index(offset.max(0));
      let mut sum = 0f32;

      for (sample, tap) in self
        .history
        .iter()
        .skip(skipped_samples)
        .zip(taps.iter().skip(skipped_taps))
      {
        sum += sample * tap;
      }

      out.push(sum);
    }

    self.made = self.made.max(total);

    let keep_from = self
      .taken
      .wrapping_sub(PAST.wrapping_add(self.future).wrapping_add(GUARD));

    match keep_from.cmp(&self.history_start) {
      Ordering::Greater => {
        let Ok(dropped) = index(keep_from.wrapping_sub(self.history_start));

        self.history.drain(..dropped.min(self.history.len()));
        self.history_start = keep_from;
      }
      Ordering::Less | Ordering::Equal => {}
    }

    Ok(())
  }
}
