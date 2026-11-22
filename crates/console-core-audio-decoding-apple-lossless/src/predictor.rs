//! One channel of an element: the header that says how it was predicted, the
//! rice-coded residuals that follow it, and the adaptive filter that turns the
//! residuals back into samples.
//!
//! Decoding runs on signed 32-bit numbers while the predicted samples are only
//! `pred_bits` wide, so every sample is clipped back to that width after it is
//! rebuilt. All the arithmetic wraps, because that is what an encoder that
//! overflowed would have done and what a release build of the reference
//! decoder does with it.

use std::{cell::Cell, cmp::Ordering};

use console_core_byte_streams::ReadBitsLtr;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::cookie::MagicCookie;
use crate::fault::Fault;

const FIRST_ORDER_MODE: u32 = 15;

const FIRST_ORDER_LPC: u32 = 31;

const LONGEST_PREFIX: u32 = 9;

const SHORTEST_ESCAPE: u32 = 8;

const LONGEST_READ: u32 = 32;

const ZERO_RUN_BITS: u32 = 16;

#[derive(Debug)]
pub(crate) struct Predictor {
  pred_bits: u32,
  kb: u32,
  mb: u32,
  mode: u32,
  shift: u32,
  pb_factor: u32,
  lpc_order: u32,
  lpc_coeffs: [i32; 32],
}

struct Run {
  mb: u32,
  sign_toggle: u32,
  zero_run_end: u32,
}

enum ZeroRunField {
  Read,
  Absent,
}

impl From<bool> for ZeroRunField {
  fn from(present: bool) -> Self {
    match bool::then_some(present, ()) {
      Some(()) => Self::Read,
      None => Self::Absent,
    }
  }
}

struct Rice {
  k: u32,
  escape: u32,
}

struct Moment {
  residual: i32,
  past0: i32,
}

fn signed(value: u32) -> Result<i32, Never> {
  Ok(i32::from_be_bytes(value.to_be_bytes()))
}

fn lg3a(val: u32) -> Result<u32, Never> {
  Ok(31u32.wrapping_sub(val.wrapping_shr(9).wrapping_add(3).leading_zeros()))
}

fn rice_code_to_signed(val: u32) -> Result<i32, Never> {
  let Ok(half) = signed(val.wrapping_shr(1));
  let Ok(sign) = signed(val & 1);

  Ok(half ^ sign.wrapping_neg())
}

fn clip_msbs(val: i32, num: u32) -> Result<i32, Never> {
  Ok(val.wrapping_shl(num).wrapping_shr(num))
}

fn quotient<B: ReadBitsLtr>(bs: &mut B, rice: &Rice, prefix: u32) -> Result<u32, Fault> {
  match rice.k {
    0 => Ok(0),
    1 => Ok(prefix),
    _ => {
      let base = prefix.wrapping_shl(rice.k).wrapping_sub(prefix);
      let suffix = bs.read_bits_leq32(rice.k.wrapping_sub(1))?;

      match suffix {
        0 => Ok(base),
        _ => {
          let low = bs.read_bit()?;

          Ok(
            base
              .wrapping_add(suffix.wrapping_shl(1))
              .wrapping_add(low)
              .wrapping_sub(1),
          )
        }
      }
    }
  }
}

fn read_rice_code<B: ReadBitsLtr>(bs: &mut B, rice: Rice) -> Result<u32, Fault> {
  let prefix = bs.read_unary_ones_capped(LONGEST_PREFIX)?;

  match prefix.cmp(&SHORTEST_ESCAPE) {
    Ordering::Greater => bs
      .read_bits_leq32(rice.escape.min(LONGEST_READ))
      .map_err(Fault::from),
    Ordering::Equal | Ordering::Less => quotient(bs, &rice, prefix),
  }
}

fn accumulate<'a, I: Iterator<Item = &'a mut i32>>(
  samples: I,
  first: i32,
  clip: u32,
) -> Result<(), Never> {
  let mut previous = first;

  for sample in samples {
    let Ok(clipped) = clip_msbs(sample.wrapping_add(previous), clip);

    *sample = clipped;
    previous = clipped;
  }

  Ok(())
}

impl Predictor {
  pub(crate) fn read<B: ReadBitsLtr>(
    bs: &mut B,
    config: &MagicCookie,
    pred_bits: u32,
  ) -> Result<Predictor, Fault> {
    let mode = bs.read_bits_leq32(4)?;
    let shift = bs.read_bits_leq32(4)?;
    let scale = bs.read_bits_leq32(3)?;
    let lpc_order = bs.read_bits_leq32(5)?;
    let pb_factor = scale.wrapping_mul(u32::from(config.pb)).wrapping_shr(2);
    let Ok(taps) = index(lpc_order);
    let mut lpc_coeffs = [0; 32];

    for coeff in lpc_coeffs.iter_mut().take(taps) {
      let value = bs.read_bits_leq32_signed(16)?;

      *coeff = value;
    }

    Ok(Predictor {
      pred_bits,
      kb: u32::from(config.kb),
      mb: u32::from(config.mb),
      mode,
      shift,
      pb_factor,
      lpc_order,
      lpc_coeffs,
    })
  }

  pub(crate) fn read_residuals<B: ReadBitsLtr>(
    &self,
    bs: &mut B,
    out: &mut [i32],
  ) -> Result<(), Fault> {
    let Ok(count): Result<u32, Never> = fitted(out.len());
    let mut run = Run {
      mb: self.mb,
      sign_toggle: 0,
      zero_run_end: 0,
    };

    for (position, sample) in (0..count).zip(out.iter_mut()) {
      match position.cmp(&run.zero_run_end) {
        Ordering::Less => continue,
        Ordering::Equal | Ordering::Greater => {}
      }

      self.residual(bs, &mut run, sample)?;

      match ZeroRunField::from(run.mb < 128 && position.wrapping_add(1) < count) {
        ZeroRunField::Read => self.zero_run(bs, &mut run, position)?,
        ZeroRunField::Absent => {}
      }
    }

    Ok(())
  }

  fn residual<B: ReadBitsLtr>(
    &self,
    bs: &mut B,
    run: &mut Run,
    sample: &mut i32,
  ) -> Result<(), Fault> {
    let Ok(k) = lg3a(run.mb);
    let code = read_rice_code(
      bs,
      Rice {
        k: k.min(self.kb),
        escape: self.pred_bits,
      },
    )?;
    let val = code.wrapping_add(run.sign_toggle);
    let Ok(value) = rice_code_to_signed(val);

    *sample = value;
    run.mb = match val {
      0x1_0000.. => 0xffff,
      0..=0xffff => {
        let decayed = run
          .mb
          .wrapping_sub(self.pb_factor.wrapping_mul(run.mb).wrapping_shr(9));

        decayed.wrapping_add(self.pb_factor.wrapping_mul(val))
      }
    };
    run.sign_toggle = 0;

    Ok(())
  }

  fn zero_run<B: ReadBitsLtr>(
    &self,
    bs: &mut B,
    run: &mut Run,
    position: u32,
  ) -> Result<(), Fault> {
    let k = run
      .mb
      .leading_zeros()
      .wrapping_sub(24)
      .wrapping_add(run.mb.wrapping_add(16).wrapping_shr(6));
    let zeros = read_rice_code(
      bs,
      Rice {
        k: k.min(self.kb),
        escape: ZERO_RUN_BITS,
      },
    )?;

    run.sign_toggle = match zeros {
      0..=0xfffe => 1,
      0xffff.. => 0,
    };
    run.mb = 0;
    run.zero_run_end = position.saturating_add(1).saturating_add(zeros);

    Ok(())
  }

  pub(crate) fn predict(&mut self, out: &mut [i32]) -> Result<(), Fault> {
    match self.mode {
      1..=14 => return Err(Fault::Mode),
      _ => {}
    }

    match self.lpc_order {
      0 => return Ok(()),
      _ => {}
    }

    let clip = 32u32.wrapping_sub(self.pred_bits);
    let (first, rest) = match out.split_first_mut() {
      Some(parts) => parts,
      None => return Err(Fault::FrameLength),
    };

    match (self.lpc_order, self.mode) {
      (FIRST_ORDER_LPC, _) | (_, FIRST_ORDER_MODE) => {
        let Ok(()) = accumulate(rest.iter_mut(), *first, clip);
      }
      (_lpc_order, _mode) => {}
    }

    let Ok(order) = index(self.lpc_order);

    match rest.len().cmp(&order) {
      Ordering::Equal | Ordering::Greater => {}
      Ordering::Less => return Err(Fault::FrameLength),
    }

    let Ok(()) = accumulate(rest.iter_mut().take(order), *first, clip);

    self.filter(out, clip)
  }

  fn filter(&mut self, out: &mut [i32], clip: u32) -> Result<(), Fault> {
    let Ok(order) = index(self.lpc_order);
    let cells = Cell::from_mut(out).as_slice_of_cells();

    for window in cells.windows(order.saturating_add(2)) {
      let parts = window.split_first().and_then(|(past, rest)| {
        rest
          .split_last()
          .map(|(sample, recent)| (past, sample, recent))
      });

      match parts {
        Some((past, sample, recent)) => {
          let Ok(()) = self.step(sample, past.get(), recent, clip);
        }
        None => return Err(Fault::FrameLength),
      }
    }

    Ok(())
  }

  fn step(
    &mut self,
    sample: &Cell<i32>,
    past0: i32,
    recent: &[Cell<i32>],
    clip: u32,
  ) -> Result<(), Never> {
    let residual = sample.get();
    let sum = self
      .lpc_coeffs
      .iter()
      .take(recent.len())
      .rev()
      .zip(recent)
      .map(|(&coeff, past)| coeff.wrapping_mul(past.get().wrapping_sub(past0)))
      .fold(0i32, i32::wrapping_add);
    let rounding = 1i32.wrapping_shl(self.shift).wrapping_shr(1);
    let predicted = sum.wrapping_add(rounding).wrapping_shr(self.shift);
    let Ok(rebuilt) = clip_msbs(residual.wrapping_add(past0).wrapping_add(predicted), clip);

    sample.set(rebuilt);

    match residual {
      0 => Ok(()),
      _ => self.adapt(Moment { residual, past0 }, recent),
    }
  }

  fn adapt(&mut self, moment: Moment, recent: &[Cell<i32>]) -> Result<(), Never> {
    let direction = moment.residual.signum();
    let coefficients = self
      .lpc_coeffs
      .iter_mut()
      .take(recent.len())
      .rev()
      .zip(recent);
    let mut remaining = moment.residual;

    for (weight, (coeff, past)) in (1i32..=32).zip(coefficients) {
      let difference = moment.past0.wrapping_sub(past.get());
      let sign = difference.signum();
      let toward = direction.wrapping_mul(sign);

      *coeff = coeff.wrapping_sub(toward);
      remaining = remaining.wrapping_sub(
        weight.wrapping_mul(toward.wrapping_mul(difference).wrapping_shr(self.shift)),
      );

      match (direction, remaining) {
        (1.., ..=0) | (..=0, 0..) => break,
        (1.., 1..) | (..=0, ..=-1) => {}
      }
    }

    Ok(())
  }
}
