//! Copyright 2020 the Resvg Authors
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{Error, ImageRef, ImageRefMut, PixelPosition};
use console_core_never::Never;
use console_core_number_conversion::{nearest_f32_from_u64, toward_zero_i32};
use console_core_pixel_buffers::Rgba8;
use console_core_scalable_vector_graphics::filter::{ColorChannel, DisplacementMap};

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The two canvas scale factors express output pixels per SVG coordinate"
  )
)]
pub(super) struct CanvasScale {
  pub x: f32,
  pub y: f32,
}

#[doc = "Displaces source samples using the two selected channels from an unpremultiplied map."]
pub(super) fn apply(
  map: &DisplacementMap,
  scale: CanvasScale,
  source: ImageRef<'_>,
  field: ImageRef<'_>,
  destination: ImageRefMut<'_>,
) -> Result<(), Error> {
  let size = source.dimensions()?;
  let field_size = field.dimensions()?;
  let destination_size = destination.dimensions()?;

  match (size == field_size && size == destination_size).then_some(()) {
    Some(()) => {}
    None => return Err(Error::InvalidRegion),
  }

  let Ok(horizontal) = map.x_channel_selector();
  let Ok(vertical) = map.y_channel_selector();
  let Ok(distance) = map.scale();
  let displacement = Displacement {
    horizontal,
    vertical,
    scale,
    distance,
  };
  let mut outputs = destination.data.iter_mut().zip(field.data.iter());

  for y in 0..size.height {
    'over_x_2: for x in 0..size.width {
      let (output, field_sample) = outputs.next().ok_or(Error::InvalidRegion)?;
      let Ok(offset) = displacement.position(PixelPosition { x, y }, *field_sample);
      let position = match offset {
        Some(position) => position,
        None => continue 'over_x_2,
      };
      let Ok(sample) = source.sample(position);

      match sample {
        Some(pixel) => *output = pixel,
        None => {}
      }
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The requested displacement distance is in SVG coordinates"
  )
)]
struct Displacement {
  horizontal: ColorChannel,
  vertical: ColorChannel,
  scale: CanvasScale,
  distance: f32,
}

impl Displacement {
  fn position(
    &self,
    point: PixelPosition,
    [red, green, blue, alpha]: Rgba8,
  ) -> Result<Option<PixelPosition>, Never> {
    let offset = |channel| {
      let byte = match channel {
        ColorChannel::R => red,
        ColorChannel::G => green,
        ColorChannel::B => blue,
        ColorChannel::A => alpha,
      };
      f32::from(byte) / 255.0 - 0.5
    };
    let Ok(x) = nearest_f32_from_u64(u64::from(point.x));
    let Ok(y) = nearest_f32_from_u64(u64::from(point.y));
    let Ok(position_x) = toward_zero_i32(f64::from(
      (x + offset(self.horizontal) * self.scale.x * self.distance).round(),
    ));
    let Ok(position_y) = toward_zero_i32(f64::from(
      (y + offset(self.vertical) * self.scale.y * self.distance).round(),
    ));

    Ok(
      match (u32::try_from(position_x), u32::try_from(position_y)) {
        (Ok(x_2), Ok(y_2)) => Some(PixelPosition { x: x_2, y: y_2 }),
        (Err(_negative_coordinate), _) | (Ok(_), Err(_negative_coordinate)) => None,
      },
    )
  }
}
