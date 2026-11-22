//! Copyright 2020 the Resvg Authors
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{BlurDeviation, Error, ImageAxis, ImageRefMut};
use console_core_never::Never;
use console_core_number_conversion::{
  index, nearest_f32_from_f64, nearest_f32_from_u64, toward_zero_u8, toward_zero_u32,
};
use console_core_pixel_buffers::Rgba8;
use console_core_size::Size;

type Radii = [u32; 5];

#[doc = "Applies five horizontal and vertical box passes to premultiplied pixels."]
#[doc = "Samples outside the image are transparent. A zero deviation skips that axis."]
pub(super) fn apply(deviation: BlurDeviation, source: ImageRefMut<'_>) -> Result<(), Error> {
  let size = source.dimensions()?;

  match (size.width, size.height) {
    (0, _) | (_, 0) => return Ok(()),
    (_, _) => {}
  }

  let Ok(horizontal) = box_radii(PixelDeviation(deviation.horizontal));
  let Ok(vertical) = box_radii(PixelDeviation(deviation.vertical));
  let mut back = Vec::new();
  back
    .try_reserve_exact(source.data.len())
    .map_err(|_| Error::InvalidRegion)?;
  back.extend_from_slice(source.data);
  let Ok(mut temporary) = ImageRefMut::new(size, &mut back);
  let mut output = source;

  for (horizontal_radius, vertical_radius) in horizontal.into_iter().zip(vertical) {
    let Ok(()) = blur_axis(
      ImageAxis::Columns,
      size,
      vertical_radius,
      &output,
      &mut temporary,
    );
    let Ok(()) = blur_axis(
      ImageAxis::Rows,
      size,
      horizontal_radius,
      &temporary,
      &mut output,
    );
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The Gaussian deviation is measured in output pixels"
  )
)]
struct PixelDeviation(f64);

fn box_radii(PixelDeviation(deviation): PixelDeviation) -> Result<Radii, Never> {
  let Ok(sigma) = nearest_f32_from_f64(deviation);
  let sigma_variance = 12.0 * sigma * sigma;

  match (sigma > 0.0).then_some(()) {
    Some(()) => {}
    None => return Ok([0; 5]),
  }

  const PASSES: f32 = 5.0;
  let ideal_width = (sigma_variance / PASSES).sqrt() + 1.0;
  let Ok(floor) = toward_zero_u32(f64::from(ideal_width.floor()));
  let lower = match floor & 1 {
    0 => floor.saturating_sub(1).max(1),
    _ => floor,
  };
  let upper = lower.saturating_add(2);
  let Ok(lower_width) = nearest_f32_from_u64(u64::from(lower));
  let ideal_count = (sigma_variance
    - PASSES * lower_width * lower_width
    - 4.0 * PASSES * lower_width
    - 3.0 * PASSES)
    / (-4.0 * lower_width - 4.0);
  let Ok(short_passes) = toward_zero_u8(f64::from(ideal_count.round()));
  let short_radius = ::core::primitive::u32::wrapping_div(lower.saturating_sub(1), 2);
  let long_radius = ::core::primitive::u32::wrapping_div(upper.saturating_sub(1), 2);

  Ok(
    [0_u8, 1, 2, 3, 4].map(|pass| match (pass < short_passes).then_some(()) {
      Some(()) => short_radius,
      None => long_radius,
    }),
  )
}

fn blur_axis(
  axis: ImageAxis,
  size: Size<u32>,
  radius: u32,
  input: &ImageRefMut<'_>,
  output: &mut ImageRefMut<'_>,
) -> Result<(), Never> {
  match radius {
    0 => {
      for (target, original) in output.data.iter_mut().zip(input.data.iter()) {
        *target = *original;
      }

      return Ok(());
    }
    _ => {}
  }

  let Ok(lines) = axis.lines(size);
  let window = ::core::primitive::u64::wrapping_add(
    ::core::primitive::u64::wrapping_mul(u64::from(radius), 2),
    1,
  );
  let Ok(window_width) = nearest_f32_from_u64(window);
  let reciprocal = BoxWeight(1.0 / window_width);

  for line in 0..lines.count {
    let window_2 = LineWindow {
      origin: ::core::primitive::u64::wrapping_mul(u64::from(line), u64::from(lines.line_step)),
      length: u64::from(lines.length),
      step: u64::from(lines.pixel_step),
      radius: u64::from(radius),
      reciprocal,
    };
    let Ok(()) = blur_line(window_2, input.data, output.data);
  }

  Ok(())
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "A box sample has the dimensionless weight one over the window size"
  )
)]
struct BoxWeight(f32);

struct LineWindow {
  origin: u64,
  length: u64,
  step: u64,
  radius: u64,
  reciprocal: BoxWeight,
}

impl LineWindow {
  fn pixel(&self, source: &[Rgba8], offset: u64) -> Result<Option<Rgba8>, Never> {
    match (offset < self.length).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    let Ok(location) = index(self.origin.saturating_add(offset.saturating_mul(self.step)));

    Ok(source.get(location).copied())
  }
}

fn blur_line(window: LineWindow, input: &[Rgba8], output: &mut [Rgba8]) -> Result<(), Never> {
  let mut total = PixelTotal::default();

  for position in 0..window.radius.min(window.length) {
    let Ok(pixel) = window.pixel(input, position);

    match pixel {
      Some(pixel_2) => { let Ok(()) = total.add(pixel_2); }
      None => return Ok(()),
    }
  }

  for position in 0..window.length {
    let Ok(entering) = window.pixel(input, position.saturating_add(window.radius));

    match entering {
      Some(pixel) => {
        let Ok(()) = total.add(pixel);
      }
      None => {}
    }

    let leaving = position.checked_sub(window.radius.saturating_add(1));

    match leaving {
      Some(leaving_2) => {
        let Ok(pixel) = window.pixel(input, leaving_2);

        match pixel {
          Some(pixel_3) => { let Ok(()) = total.remove(pixel_3); }
          None => return Ok(()),
        }
      }
      None => {}
    }

    let Ok(location) = index(
      window
        .origin
        .saturating_add(position.saturating_mul(window.step)),
    );
    let Ok(color) = total.color(window.reciprocal);

    match output.get_mut(location) {
      Some(target) => *target = color,
      None => return Ok(()),
    }
  }

  Ok(())
}

#[derive(Default)]
struct PixelTotal([u64; 4]);

impl PixelTotal {
  fn add(&mut self, pixel: Rgba8) -> Result<(), Never> {
    for (total, channel) in self.0.iter_mut().zip(pixel) {
      *total = total.saturating_add(u64::from(channel));
    }

    Ok(())
  }

  fn remove(&mut self, pixel: Rgba8) -> Result<(), Never> {
    for (total, channel) in self.0.iter_mut().zip(pixel) {
      *total = total.saturating_sub(u64::from(channel));
    }

    Ok(())
  }

  fn color(&self, BoxWeight(weight): BoxWeight) -> Result<Rgba8, Never> {
    let [r, g, b, a] = self.0.map(|channel| {
      let Ok(total) = nearest_f32_from_u64(channel);
      let scaled = total * weight;
      let rounded = (scaled + 12_582_912.0) - 12_582_912.0;
      let Ok(value) = toward_zero_u8(f64::from(rounded));
      value
    });

    Ok([r, g, b, a])
  }
}
