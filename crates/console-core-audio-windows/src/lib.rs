//! Window coefficients used to join neighboring blocks of decoded audio.
//!
//! Sine and Kaiser–Bessel windows are used by AAC. The Vorbis half-window is
//! also used for the low-overlap synthesis in Opus CELT. Decoders choose their
//! window sizes and retain their own block transitions and overlap state.
//!
//! Both generators are made once, when a decoder starts, and write into a slice
//! the caller owns. A slice shorter than the window is filled as far as it
//! goes rather than refused, and one longer keeps its tail untouched.

#![forbid(unsafe_code)]

mod window;

pub use window::{Extent, Gain, KaiserAlpha, WindowType, generate_window};

use console_core_never::Never;
use console_core_number_conversion::{Float, narrow_f32};
use core::f64::consts::FRAC_PI_2;

pub fn vorbis_half(dst: &mut [f32]) -> Result<(), Never> {
  let Ok(denominator) = dst.len().float();

  for (index, value) in dst.iter_mut().enumerate() {
    let Ok(position) = index.float();
    let angle = FRAC_PI_2 * ((position + 0.5) / denominator);
    let Ok(coefficient) = narrow_f32((FRAC_PI_2 * angle.sin().powi(2)).sin());

    *value = coefficient;
  }

  Ok(())
}
