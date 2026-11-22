//! The inverse Modified Discrete Cosine Transform (IMDCT), written in terms of a
//! forward FFT of half its size.
//!
//! The spectrum is packed into complex values and rotated by a twiddle, the
//! FFT is done, and a second rotation unpacks the result into four quarters of
//! the output, two of them written forwards and two backwards. The even and
//! odd positions of a quarter are walked as the two halves of its chunks of
//! two, so no position is computed and no slice is indexed.

use console_core_never::Never;
use console_core_number_conversion::narrow_f32;
use core::f64::consts::PI;

use crate::{BufferLength, Fault, TransformSize};
use crate::complex::Complex;
use crate::fft::Fft;

pub struct Imdct {
  fft: Fft,
  fft_in: Box<[Complex]>,
  fft_out: Box<[Complex]>,
  twiddle: Box<[Complex]>,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "IMDCT normalization is a dimensionless multiplier"
  )
)]
#[derive(Clone, Copy)]
pub struct Scale(pub f64);

enum ScalePhase {
  Ordinary,
  Inverted,
}

impl From<bool> for ScalePhase {
  fn from(negative: bool) -> Self {
    match bool::then_some(negative, ()) {
      Some(()) => Self::Inverted,
      None => Self::Ordinary,
    }
  }
}

fn twiddles(count: u32, scale: Scale) -> Result<Box<[Complex]>, Never> {
  let alpha = 1.0 / 8.0
    + match ScalePhase::from(scale.0.is_sign_negative()) {
      ScalePhase::Ordinary => 0.0,
      ScalePhase::Inverted => f64::from(count),
    };
  let pi_n = PI / f64::from(count.saturating_mul(2));
  let sqrt_scale = scale.0.abs().sqrt();

  Ok(
    (0..count)
      .map(|step| {
        let theta = pi_n * (alpha + f64::from(step));
        let Ok(re) = narrow_f32(sqrt_scale * theta.cos());
        let Ok(im) = narrow_f32(sqrt_scale * theta.sin());

        Complex { re, im }
      })
      .collect(),
  )
}

impl Imdct {
  pub fn new(n: u32) -> Result<Self, Fault> {
    Imdct::new_scaled(n, Scale(1.0))
  }

  pub fn new_scaled(n: u32, scale: Scale) -> Result<Self, Fault> {
    match TransformSize::from(n.is_power_of_two()) {
      TransformSize::Unsupported => Err(Fault::Size),
      TransformSize::Supported => {
        let half = n.wrapping_shr(1);
        let fft = Fft::new(half)?;
        let Ok(twiddle) = twiddles(half, scale);
        let Ok(fft_in) = fft.buffer();
        let Ok(fft_out) = fft.buffer();

        Ok(Imdct {
          fft,
          fft_in,
          fft_out,
          twiddle,
        })
      }
    }
  }

  pub fn imdct(&mut self, spec: &[f32], out: &mut [f32]) -> Result<(), Fault> {
    match (
      BufferLength::from(spec.len() == self.fft_in.len().saturating_mul(2)),
      BufferLength::from(out.len() == self.fft_in.len().saturating_mul(4)),
    ) {
      (BufferLength::Exact, BufferLength::Exact) => {}
      (BufferLength::Mismatched, BufferLength::Exact | BufferLength::Mismatched) => {
        return Err(Fault::Spectrum);
      }
      (BufferLength::Exact, BufferLength::Mismatched) => return Err(Fault::Output),
    }

    let (pairs, _) = spec.as_chunks::<2>();

    for (((w, t), [even, _]), [_, odd]) in self
      .twiddle
      .iter()
      .zip(self.fft_in.iter_mut())
      .zip(pairs)
      .zip(pairs.iter().rev())
    {
      let odd_2 = -odd;

      *t = Complex {
        re: odd_2 * w.im - even * w.re,
        im: odd_2 * w.re + even * w.im,
      };
    }

    self.fft.fft(&self.fft_in, &mut self.fft_out)?;

    let (vec0, rest) = out.split_at_mut(self.fft_in.len());
    let (vec1, rest_2) = rest.split_at_mut(self.fft_in.len());
    let (vec2, vec3) = rest_2.split_at_mut(self.fft_in.len());
    let (chunks0, _) = vec0.as_chunks_mut::<2>();
    let (chunks1, _) = vec1.as_chunks_mut::<2>();
    let (chunks2, _) = vec2.as_chunks_mut::<2>();
    let (chunks3, _) = vec3.as_chunks_mut::<2>();
    let (first_out, second_out) = self.fft_out.split_at(self.fft_in.len().wrapping_shr(1));
    let (first_twiddle, second_twiddle) =
      self.twiddle.split_at(self.fft_in.len().wrapping_shr(1));

    for ((((x, w), [_, back0]), [front1, _]), ([_, back2], [front3, _])) in first_out
      .iter()
      .zip(first_twiddle)
      .zip(chunks0.iter_mut().rev())
      .zip(chunks1.iter_mut())
      .zip(chunks2.iter_mut().rev().zip(chunks3.iter_mut()))
    {
      let val = *w
        * Complex {
          re: x.re,
          im: -x.im,
        };

      *back0 = -val.im;
      *front1 = val.im;
      *back2 = val.re;
      *front3 = val.re;
    }

    for ((((x, w), [front0, _]), [_, back1]), ([front2, _], [_, back3])) in second_out
      .iter()
      .zip(second_twiddle)
      .zip(chunks0.iter_mut())
      .zip(chunks1.iter_mut().rev())
      .zip(chunks2.iter_mut().zip(chunks3.iter_mut().rev()))
    {
      let val = *w
        * Complex {
          re: x.re,
          im: -x.im,
        };

      *front0 = -val.re;
      *back1 = val.re;
      *front2 = val.im;
      *back3 = val.im;
    }

    Ok(())
  }
}
