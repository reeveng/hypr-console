//! Copyright 2020 the Resvg Authors
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{ChannelValue, Error, ImageRef, ImageRefMut, PixelPosition};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, toward_zero_u8};
use console_core_pixel_buffers::Rgba8;
use console_core_scalable_vector_graphics::PreserveAlphaAnswer;
use console_core_scalable_vector_graphics::filter::{ConvolveMatrix, EdgeMode};
use console_core_size::Size;

#[doc = "Applies a convolution kernel. Input pixels are premultiplied when the kernel changes alpha."]
pub(super) fn apply(matrix: &ConvolveMatrix, source: ImageRefMut<'_>) -> Result<(), Error> {
  let size = source.dimensions()?;

  match (size.width, size.height) {
    (0, _) | (_, 0) => return Ok(()),
    (_, _) => {}
  }

  let kernel = Kernel::from_matrix(matrix)?;
  let Ok(input) = source.borrowed();
  let mut output = Vec::new();
  output
    .try_reserve_exact(source.data.len())
    .map_err(|_| Error::InvalidRegion)?;

  for y in 0..size.height {
    for x in 0..size.width {
      let filtered = kernel.pixel(PixelPosition { x, y }, input, size)?;
      output.push(filtered);
    }
  }

  for (pixel, filtered) in source.data.iter_mut().zip(output) {
    *pixel = filtered;
  }

  Ok(())
}

struct Kernel<'a> {
  dimensions: Size<u32>,
  anchor: PixelPosition,
  weights: &'a [f32],
  alpha: PreserveAlphaAnswer,
  edge: EdgeMode,
  mix: Mix,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "A normalized channel is divided by a dimensionless divisor and shifted by a dimensionless bias"
  )
)]
struct Mix {
  divisor: f32,
  bias: f32,
}

impl<'a> Kernel<'a> {
  fn from_matrix(filter: &'a ConvolveMatrix) -> Result<Self, Error> {
    let Ok(matrix) = filter.matrix();
    let Ok(width) = matrix.columns();
    let Ok(height) = matrix.rows();
    let Ok(x) = matrix.target_x();
    let Ok(y) = matrix.target_y();
    let Ok(weights) = matrix.data();
    let Ok(length) = fixed_index(weights.len());

    match (::core::primitive::u64::wrapping_mul(u64::from(width), u64::from(height)) == length)
      .then_some(())
    {
      Some(()) => {}
      None => return Err(Error::InvalidRegion),
    }

    let Ok(alpha) = filter.preserve_alpha();
    let Ok(edge) = filter.edge_mode();
    let Ok(raw_divisor) = filter.divisor();
    let Ok(divisor) = raw_divisor.get();
    let Ok(bias) = filter.bias();

    Ok(Self {
      dimensions: Size { width, height },
      anchor: PixelPosition { x, y },
      weights,
      alpha,
      edge,
      mix: Mix { divisor, bias },
    })
  }

  fn pixel(
    &self,
    position: PixelPosition,
    source: ImageRef<'_>,
    size: Size<u32>,
  ) -> Result<Rgba8, Error> {
    let mut coefficients = self.weights.iter().rev();
    let mut sums = [0.0; 4];
    let base_x =
      ::core::primitive::i64::wrapping_sub(i64::from(position.x), i64::from(self.anchor.x));
    let base_y =
      ::core::primitive::i64::wrapping_sub(i64::from(position.y), i64::from(self.anchor.y));

    for row in 0..self.dimensions.height {
      let Ok(y) = crop_axis(
        base_y.saturating_add(i64::from(row)),
        size.height,
        self.edge,
      );

      'over_column_2: for column in 0..self.dimensions.width {
        let coefficient = coefficients.next().ok_or(Error::InvalidRegion)?;
        let Ok(x) = crop_axis(
          base_x.saturating_add(i64::from(column)),
          size.width,
          self.edge,
        );
        let sample = match (x, y) {
          (Some(x_2), Some(y_2)) => PixelPosition { x: x_2, y: y_2 },
          (None, _) | (_, None) => continue 'over_column_2,
        };
        let Ok(input) = source.sample(sample);
        let [r, g, b, a] = input.ok_or(Error::InvalidRegion)?;
        let [red, green, blue, alpha] = &mut sums;
        *red += f32::from(r) / 255.0 * coefficient;
        *green += f32::from(g) / 255.0 * coefficient;
        *blue += f32::from(b) / 255.0 * coefficient;

        match self.alpha {
          PreserveAlphaAnswer::Yes => {}
          PreserveAlphaAnswer::No => *alpha += f32::from(a) / 255.0 * coefficient,
        }
      }
    }

    let Ok(center) = source.alpha_at(position);
    let Ok(pixel) = self.finish(sums, center);
    Ok(pixel)
  }

  fn finish(&self, [r, g, b, a]: [f32; 4], center_alpha: i16) -> Result<Rgba8, Never> {
    let Mix { divisor, bias } = self.mix;
    let alpha = match self.alpha {
      PreserveAlphaAnswer::Yes => f32::from(center_alpha) / 255.0,
      PreserveAlphaAnswer::No => a / divisor + bias,
    };
    let Ok(ChannelValue(bounded_alpha)) = ChannelValue(alpha).clamped(ChannelValue(1.0));
    let [r_2, g_2, b_2] = [r, g, b].map(|channel| {
      let offset = channel / divisor + bias * alpha;
      let output = match self.alpha {
        PreserveAlphaAnswer::Yes => {
          let Ok(ChannelValue(color)) = ChannelValue(offset).clamped(ChannelValue(1.0));
          color * bounded_alpha
        }
        PreserveAlphaAnswer::No => {
          let Ok(ChannelValue(color)) = ChannelValue(offset).clamped(ChannelValue(bounded_alpha));
          color
        }
      };
      let Ok(converted) = toward_zero_u8(f64::from(output * 255.0 + 0.5));
      converted
    });
    let Ok(a_2) = toward_zero_u8(f64::from(bounded_alpha * 255.0 + 0.5));

    Ok([r_2, g_2, b_2, a_2])
  }
}

fn crop_axis(value: i64, span: u32, edge: EdgeMode) -> Result<Option<u32>, Never> {
  let extent = i64::from(span);
  let bounded = match (edge, span) {
    (_, 0) => return Ok(None),
    (EdgeMode::Duplicate, _) => value.clamp(0, extent.saturating_sub(1)),
    (EdgeMode::Wrap, _) => value.rem_euclid(extent),
    (EdgeMode::None, _) => match (0..extent).contains(&value).then_some(()) {
      Some(()) => value,
      None => return Ok(None),
    },
  };
  Ok(match u32::try_from(bounded) {
    ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
    ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
  })
}
