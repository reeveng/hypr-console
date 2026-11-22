//! Copyright 2020 the Resvg Authors
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{Error, ImageAxis, ImageRefMut};
use console_core_never::Never;
use console_core_number_conversion::{index, nearest_f32_from_u64, toward_zero_u32};
use console_core_pixel_buffers::Rgba8;
use console_core_scalable_vector_graphics::filter::MorphologyOperator;
use console_core_size::Size;

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The horizontal and vertical morphology radii are in output pixels"
  )
)]
pub(super) struct Radii {
  pub x: f32,
  pub y: f32,
}

#[doc = "Applies a rectangular erosion or dilation to premultiplied pixels."]
pub(super) fn apply(
  operator: MorphologyOperator,
  radii: Radii,
  source: ImageRefMut<'_>,
) -> Result<(), Error> {
  let size = source.dimensions()?;

  match (size.width, size.height) {
    (0, _) | (_, 0) => return Ok(()),
    (_, _) => {}
  }

  let mut intermediate = Vec::new();
  intermediate
    .try_reserve_exact(source.data.len())
    .map_err(|_| Error::InvalidRegion)?;
  intermediate.resize(source.data.len(), Rgba8::default());

  let Ok(horizontal) = Neighborhood::new(operator, Radius(radii.x), size.width);
  let Ok(vertical) = Neighborhood::new(operator, Radius(radii.y), size.height);
  let Ok(()) = filter_axis(
    ImageAxis::Rows,
    size,
    horizontal,
    source.data,
    &mut intermediate,
  );
  let Ok(()) = filter_axis(
    ImageAxis::Columns,
    size,
    vertical,
    &intermediate,
    source.data,
  );

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The radius is measured in output pixels"
  )
)]
struct Radius(f32);

struct Neighborhood {
  operator: MorphologyOperator,
  length: u64,
  pivot: u64,
}

impl Neighborhood {
  fn new(operator: MorphologyOperator, Radius(radius): Radius, span: u32) -> Result<Self, Never> {
    let Ok(rounded) = toward_zero_u32(f64::from(radius.ceil()));
    let length = rounded.saturating_mul(2).min(span);
    let Ok(floating_width) = nearest_f32_from_u64(u64::from(length));
    let Ok(pivot) = toward_zero_u32(f64::from((floating_width / 2.0).floor()));

    Ok(Self {
      operator,
      length: u64::from(length),
      pivot: u64::from(pivot),
    })
  }

  fn neutral(&self) -> Result<Rgba8, Never> {
    let byte = match self.operator {
      MorphologyOperator::Erode => 255,
      MorphologyOperator::Dilate => 0,
    };

    Ok([byte; 4])
  }

  fn combine(&self, sample: Rgba8, total: &mut Rgba8) -> Result<(), Never> {
    let combine = match self.operator {
      MorphologyOperator::Erode => u8::min,
      MorphologyOperator::Dilate => u8::max,
    };

    for (channel, byte) in total.iter_mut().zip(sample) {
      *channel = combine(*channel, byte);
    }

    Ok(())
  }
}

fn filter_axis(
  axis: ImageAxis,
  size: Size<u32>,
  neighborhood: Neighborhood,
  input: &[Rgba8],
  output: &mut [Rgba8],
) -> Result<(), Never> {
  let Ok(lines) = axis.lines(size);
  let span = u64::from(lines.length);
  let step = u64::from(lines.pixel_step);

  for line in 0..lines.count {
    let origin =
      ::core::primitive::u64::wrapping_mul(u64::from(line), u64::from(lines.line_step));

    for position in 0..span {
      let start = position.saturating_sub(neighborhood.pivot);
      let end = (position.saturating_add(neighborhood.length))
        .saturating_sub(neighborhood.pivot)
        .min(span);
      let Ok(mut accumulated) = neighborhood.neutral();

      for offset in start..end {
        let Ok(location) = index(origin.saturating_add(offset.saturating_mul(step)));

        match input.get(location) {
          Some(sample) => {
            let Ok(()) = neighborhood.combine(*sample, &mut accumulated);
          }
          None => return Ok(()),
        }
      }

      let Ok(location) = index(origin.saturating_add(position.saturating_mul(step)));

      match output.get_mut(location) {
        Some(target) => *target = accumulated,
        None => return Ok(()),
      }
    }
  }

  Ok(())
}
