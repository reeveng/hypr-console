//! The AAC windows: a sine, and a Kaiser-Bessel derived one that takes its
//! weights from the running sum of a Bessel function.
//!
//! The Kaiser-Bessel window is two passes over the same weights, one for the
//! total they normalise by and one for the running sum, rather than a list of
//! partial sums held to be divided later, so no window is made through the
//! allocator.

use console_core_never::Never;
use console_core_number_conversion::narrow_f32;
use core::f32::consts::PI;

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "sine gain is a dimensionless sample amplitude multiplier"
  )
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gain(pub f32);

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Kaiser alpha is a dimensionless window shape parameter"
  )
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KaiserAlpha(pub f32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowType {
  Sine,
  KaiserBessel(KaiserAlpha),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extent {
  Half,
  Whole,
}

fn exact(count: u32) -> Result<f32, Never> {
  narrow_f32(f64::from(count))
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the Bessel series takes a dimensionless squared half-argument"
  )
)]
fn bessel_i0(argument: f64) -> Result<f64, Never> {
  Ok((1..64u32).rev().fold(1.0, |value, term| {
    value * (argument / (f64::from(term) * f64::from(term))) + 1.0
  }))
}

fn sine(gain: Gain, size: u32, extent: Extent, dst: &mut [f32]) -> Result<(), Never> {
  let Ok(points) = narrow_f32(match extent {
    Extent::Half => f64::from(size) * 2.0,
    Extent::Whole => f64::from(size),
  });
  let step = PI / points;

  for (value, position) in dst.iter_mut().zip(0..size) {
    let Ok(at) = exact(position);

    *value = ((at + 0.5) * step).sin() * gain.0;
  }

  Ok(())
}

fn kaiser_bessel(
  alpha: KaiserAlpha,
  size: u32,
  extent: Extent,
  dst: &mut [f32],
) -> Result<(), Never> {
  let Ok(whole) = exact(size);
  let length = match extent {
    Extent::Half => whole,
    Extent::Whole => whole * 0.5,
  };
  let ratio = alpha.0 * PI / length;
  let alpha_squared = f64::from(ratio * ratio);
  let weight = |position: u32| {
    bessel_i0((f64::from(position) * f64::from(size.saturating_sub(position))) * alpha_squared)
  };
  let mut total = 0.0;

  for position in 0..size {
    let Ok(next) = weight(position);

    total += next;
  }

  total += 1.0;

  let mut running = 0.0;

  for (value, position) in dst.iter_mut().zip(0..size) {
    let Ok(next) = weight(position);

    running += next;

    let Ok(coefficient) = narrow_f32((running / total).sqrt());

    *value = coefficient;
  }

  Ok(())
}

pub fn generate_window(
  mode: WindowType,
  gain: Gain,
  size: u32,
  extent: Extent,
  dst: &mut [f32],
) -> Result<(), Never> {
  match mode {
    WindowType::Sine => sine(gain, size, extent, dst),
    WindowType::KaiserBessel(alpha) => kaiser_bessel(alpha, size, extent, dst),
  }
}
