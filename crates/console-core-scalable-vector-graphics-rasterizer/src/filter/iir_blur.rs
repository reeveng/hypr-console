//! Copyright 2020 the Resvg Authors
//! SPDX-License-Identifier: Apache-2.0 OR MIT

//! An IIR blur.
//!
//! Based on http://www.getreuer.info/home/gaussianiir
//!
//! Licensed under 'Simplified BSD License'.
//!
//!
//! Implements the fast Gaussian convolution algorithm of Alvarez and Mazorra,
//! where the Gaussian is approximated by a cascade of first-order infinite
//! impulsive response (IIR) filters.  Boundaries are handled with half-sample
//! symmetric extension.
//!
//! Gaussian convolution is approached as approximating the heat equation and
//! each timestep is performed with an efficient recursive computation.  Using
//! more steps yields a more accurate approximation of the Gaussian.  A
//! reasonable default value for `numsteps` is 4.
//!
//! Reference:
//! Alvarez, Mazorra, "Signal and Image Restoration using Shock Filters and
//! Anisotropic Diffusion," SIAM J. on Numerical Analysis, vol. 31, no. 2,
//! pp. 590-605, 1994.

use super::{BlurDeviation, Error, ImageAxis, ImageRefMut};
use console_core_never::Never;
use console_core_number_conversion::{index, toward_zero_u8};
use console_core_pixel_buffers::Rgba8;
use console_core_size::Size;

const PASSES: u8 = 4;

#[doc = "Applies a small Gaussian blur to each premultiplied channel."]
pub(super) fn apply(deviation: BlurDeviation, source: ImageRefMut<'_>) -> Result<(), Error> {
  let size = source.dimensions()?;

  match (size.width, size.height) {
    (0, _) | (_, 0) => return Ok(()),
    (_, _) => {}
  }

  let Ok(horizontal) = coefficients(PixelDeviation(deviation.horizontal));
  let Ok(vertical) = coefficients(PixelDeviation(deviation.vertical));
  let mut buffer = Vec::new();
  buffer
    .try_reserve_exact(source.data.len())
    .map_err(|_| Error::InvalidRegion)?;
  buffer.resize(source.data.len(), 0.0);

  for channel in [Channel::Red, Channel::Green, Channel::Blue, Channel::Alpha] {
    let Ok(()) = gaussian_channel(
      source.data,
      channel,
      size,
      horizontal,
      vertical,
      &mut buffer,
    );
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum Channel {
  Red,
  Green,
  Blue,
  Alpha,
}

impl Channel {
  fn at(self, [red, green, blue, alpha]: &mut Rgba8) -> Result<&mut u8, Never> {
    Ok(match self {
      Self::Red => red,
      Self::Green => green,
      Self::Blue => blue,
      Self::Alpha => alpha,
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The Gaussian deviation is measured in output pixels"
  )
)]
struct PixelDeviation(f64);

#[derive(Clone, Copy)]
enum IirPass {
  Unchanged,
  Gaussian { lambda: f64, feedback: f64 },
}

fn coefficients(PixelDeviation(sigma): PixelDeviation) -> Result<IirPass, Never> {
  match (sigma > 0.0).then_some(()) {
    Some(()) => {}
    None => return Ok(IirPass::Unchanged),
  }

  let lambda = (sigma * sigma) / (2.0 * f64::from(PASSES));
  let feedback = (1.0 + 2.0 * lambda - (1.0 + 4.0 * lambda).sqrt()) / (2.0 * lambda);

  Ok(IirPass::Gaussian { lambda, feedback })
}

fn gaussian_channel(
  data: &mut [Rgba8],
  channel: Channel,
  size: Size<u32>,
  horizontal: IirPass,
  vertical: IirPass,
  buffer: &mut [f64],
) -> Result<(), Never> {
  for (pixel, sample) in data.iter_mut().zip(buffer.iter_mut()) {
    let Ok(value) = channel.at(pixel);
    *sample = f64::from(*value) / 255.0;
  }

  let Ok(()) = filter_axis(ImageAxis::Rows, horizontal, size, buffer);
  let Ok(()) = filter_axis(ImageAxis::Columns, vertical, size, buffer);
  let (lambda_x, feedback_x) = match horizontal {
    IirPass::Unchanged => (1.0, 1.0),
    IirPass::Gaussian { lambda, feedback } => (lambda, feedback),
  };
  let (lambda_y, feedback_y) = match vertical {
    IirPass::Unchanged => (1.0, 1.0),
    IirPass::Gaussian { lambda, feedback } => (lambda, feedback),
  };
  let gain = ((feedback_x * feedback_y).sqrt() / (lambda_x * lambda_y).sqrt())
    .powi(::core::primitive::i32::wrapping_mul(2, i32::from(PASSES)));

  for (pixel, sample) in data.iter_mut().zip(buffer.iter()) {
    let Ok(value) = channel.at(pixel);
    let normalized = *sample * gain;
    let Ok(scaled) = toward_zero_u8(normalized * 255.0);
    *value = scaled;
  }

  Ok(())
}

fn filter_axis(
  axis: ImageAxis,
  pass: IirPass,
  size: Size<u32>,
  buffer: &mut [f64],
) -> Result<(), Never> {
  let feedback = match pass {
    IirPass::Unchanged => return Ok(()),
    IirPass::Gaussian { feedback, .. } => Feedback(feedback),
  };
  let Ok(lines) = axis.lines(size);
  let span = u64::from(lines.length);
  let step = u64::from(lines.pixel_step);

  for line in 0..lines.count {
    let origin =
      ::core::primitive::u64::wrapping_mul(u64::from(line), u64::from(lines.line_step));

    for _ in 0..PASSES {
      let mut view = FilterLine {
        origin,
        span,
        step,
        feedback,
        buffer,
      };
      let Ok(()) = view.forward();
      let Ok(()) = view.reverse();
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The IIR feedback is a dimensionless coefficient for the adjacent sample"
  )
)]
struct Feedback(f64);

struct FilterLine<'a> {
  origin: u64,
  span: u64,
  step: u64,
  feedback: Feedback,
  buffer: &'a mut [f64],
}

impl FilterLine<'_> {
  fn sample(&mut self, offset: u64) -> Result<Option<&mut f64>, Never> {
    let Ok(location) = index(self.origin.saturating_add(offset.saturating_mul(self.step)));

    Ok(self.buffer.get_mut(location))
  }

  fn forward(&mut self) -> Result<(), Never> {
    let Ok(first) = self.sample(0);
    let mut previous = match first {
      Some(first_2) => *first_2,
      None => return Ok(()),
    };
    let Feedback(feedback) = self.feedback;

    for offset in 1..self.span {
      let Ok(target) = self.sample(offset);
      let sample = match target {
        Some(sample) => sample,
        None => return Ok(()),
      };
      *sample += feedback * previous;
      previous = *sample;
    }

    Ok(())
  }

  fn reverse(&mut self) -> Result<(), Never> {
    let last = self.span.saturating_sub(1);
    let Ok(final_sample) = self.sample(last);
    let mut previous = match final_sample {
      Some(final_sample_2) => *final_sample_2,
      None => return Ok(()),
    };
    let Feedback(feedback) = self.feedback;

    for offset in (0..last).rev() {
      let Ok(target) = self.sample(offset);
      let sample = match target {
        Some(sample) => sample,
        None => return Ok(()),
      };
      *sample += feedback * previous;
      previous = *sample;
    }

    Ok(())
  }
}
