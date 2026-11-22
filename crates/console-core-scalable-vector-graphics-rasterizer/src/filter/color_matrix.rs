//! Copyright 2020 the Resvg Authors
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{ChannelValue, Error, ImageRefMut};
use console_core_never::Never;
use console_core_pixel_buffers::Rgba8;
use console_core_scalable_vector_graphics::console_core_paths::StrictNum;
use console_core_scalable_vector_graphics::filter::ColorMatrixKind as ColorMatrix;

#[doc = "Applies a color matrix to unpremultiplied channels."]
pub(super) fn apply(matrix: &ColorMatrix, source: ImageRefMut<'_>) -> Result<(), Error> {
  let transform = ColorTransform::from_matrix(matrix)?;

  for pixel in source.data {
    let Ok(color) = transform.pixel(*pixel);
    *pixel = color;
  }

  Ok(())
}

enum ColorTransform {
  FourChannels([[f32; 5]; 4]),
  ColorOnly([[f32; 3]; 3]),
  Luminance,
}

impl ColorTransform {
  fn from_matrix(matrix: &ColorMatrix) -> Result<Self, Error> {
    Ok(match matrix {
      ColorMatrix::Matrix(components) => match <&[f32; 20]>::try_from(components.as_slice()) {
        Ok(
          [
            r0,
            r1,
            r2,
            r3,
            r4,
            g0,
            g1,
            g2,
            g3,
            g4,
            b0,
            b1,
            b2,
            b3,
            b4,
            a0,
            a1,
            a2,
            a3,
            a4,
          ],
        ) => Self::FourChannels([
          [*r0, *r1, *r2, *r3, *r4],
          [*g0, *g1, *g2, *g3, *g4],
          [*b0, *b1, *b2, *b3, *b4],
          [*a0, *a1, *a2, *a3, *a4],
        ]),
        Err(_wrong_matrix_size) => return Err(Error::InvalidRegion),
      },
      ColorMatrix::Saturate(value) => {
        let saturation = value.get().max(0.0);
        Self::ColorOnly([
          [
            0.213 + 0.787 * saturation,
            0.715 - 0.715 * saturation,
            0.072 - 0.072 * saturation,
          ],
          [
            0.213 - 0.213 * saturation,
            0.715 + 0.285 * saturation,
            0.072 - 0.072 * saturation,
          ],
          [
            0.213 - 0.213 * saturation,
            0.715 - 0.715 * saturation,
            0.072 + 0.928 * saturation,
          ],
        ])
      }
      ColorMatrix::HueRotate(degrees) => {
        let angle = degrees.to_radians();
        let c = angle.cos();
        let s = angle.sin();
        Self::ColorOnly([
          [
            0.213 + 0.787 * c - 0.213 * s,
            0.715 - 0.715 * c - 0.715 * s,
            0.072 - 0.072 * c + 0.928 * s,
          ],
          [
            0.213 - 0.213 * c + 0.143 * s,
            0.715 + 0.285 * c + 0.140 * s,
            0.072 - 0.072 * c - 0.283 * s,
          ],
          [
            0.213 - 0.213 * c - 0.787 * s,
            0.715 - 0.715 * c + 0.715 * s,
            0.072 + 0.928 * c + 0.072 * s,
          ],
        ])
      }
      ColorMatrix::LuminanceToAlpha => Self::Luminance,
    })
  }

  fn pixel(&self, [red, green, blue, alpha]: Rgba8) -> Result<Rgba8, Never> {
    let Ok(ChannelValue(r)) = ChannelValue::from_byte(red);
    let Ok(ChannelValue(g)) = ChannelValue::from_byte(green);
    let Ok(ChannelValue(b)) = ChannelValue::from_byte(blue);

    Ok(match self {
      Self::FourChannels(rows) => {
        let Ok(ChannelValue(a)) = ChannelValue::from_byte(alpha);
        let [red_2, green_2, blue_2, alpha_2] = rows.map(|[wr, wg, wb, wa, offset]| {
          let Ok(channel) = ChannelValue(r * wr + g * wg + b * wb + a * wa + offset).quantized();
          channel
        });
        [red_2, green_2, blue_2, alpha_2]
      }
      Self::ColorOnly(rows) => {
        let [red_3, green_3, blue_3] = rows.map(|[wr, wg, wb]| {
          let Ok(channel) = ChannelValue(r * wr + g * wg + b * wb).quantized();
          channel
        });
        [red_3, green_3, blue_3, alpha]
      }
      Self::Luminance => {
        let Ok(alpha_3) = ChannelValue(r * 0.2125 + g * 0.7154 + b * 0.0721).quantized();
        [0, 0, 0, alpha_3]
      }
    })
  }
}
