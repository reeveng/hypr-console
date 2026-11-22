//! Unfiltered rows turned into RGBA and set down where their pass puts them.
//!
//! A sample is one, two, four, eight or sixteen bits. Under eight it is grey
//! or a palette index, packed from the high bit down, and a grey is stretched
//! to eight bits by repeating its bits, so that white stays white. Sixteen is
//! rounded to the nearest of eight. A palette index past the end of the
//! palette is damage. The transparent grey or colour a `tRNS` chunk names is
//! matched against the sample as it was written, before any of that.
//!
//! Eight bits a channel with no colour keyed out is nearly every PNG there is,
//! so a row of it is set down a pixel at a time without each pixel being
//! asked again what kind it is.
//!
//! Writing goes the other way and keeps no more channels than the picture
//! uses: no alpha when every pixel is opaque, and one channel for grey when
//! every pixel's red, green and blue are the same. A screenshot of a page is
//! often both, and is a third of the bytes before anything is deflated.

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_pixel_buffers::RowLayout;

use crate::chunks::{Color, Key, Read};
use crate::filters::{self, Pass};
use crate::{PngError, Reduction};

const RGBA: u64 = 4;

const OPAQUE: u8 = u8::MAX;

const CLEAR: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stored {
  Grey,
  GreyAlpha,
  Rgb,
  Rgba,
}

impl Stored {
  pub(crate) fn color(self) -> Result<Color, Never> {
    Ok(match self {
      Stored::Grey => Color::Grey,
      Stored::GreyAlpha => Color::GreyAlpha,
      Stored::Rgb => Color::Rgb,
      Stored::Rgba => Color::Rgba,
    })
  }
}

pub(crate) fn fewest(rgba: &[u8]) -> Result<Stored, Never> {
  let (pixels, _) = rgba.as_chunks::<4>();
  let grey = pixels
    .iter()
    .all(|[red, green, blue, _]| red == green && green == blue);
  let opaque = pixels.iter().all(|[.., alpha]| *alpha == OPAQUE);

  Ok(match (grey, opaque) {
    (true, true) => Stored::Grey,
    (true, false) => Stored::GreyAlpha,
    (false, true) => Stored::Rgb,
    (false, false) => Stored::Rgba,
  })
}

pub(crate) fn unpainted(rgba: &[u8], kept: Stored) -> Result<Vec<u8>, Never> {
  let (pixels, _) = rgba.as_chunks::<4>();

  Ok(match kept {
    Stored::Grey => pixels.iter().map(|[grey, ..]| *grey).collect(),
    Stored::GreyAlpha => pixels
      .iter()
      .flat_map(|[grey, _, _, alpha]| [*grey, *alpha])
      .collect(),
    Stored::Rgb => pixels
      .iter()
      .flat_map(|[red, green, blue, _]| [*red, *green, *blue])
      .collect(),
    Stored::Rgba => rgba.to_vec(),
  })
}

pub(crate) fn painted(
  read: &Read,
  mut unpacked: Vec<u8>,
  rgba: &mut [u8],
  reduction: Reduction,
) -> Result<(), PngError> {
  let Ok(passes) = filters::passes(&read.header);
  let mut rest = unpacked.as_mut_slice();

  for pass in passes {
    let Ok(length) = filters::row_length(&read.header, pass.size.width);
    let Ok(taken) = index(u64::from(pass.size.height).saturating_mul(length.saturating_add(1)));

    let (rows, after) = match std::mem::take(&mut rest).split_at_mut_checked(taken) {
      Some(split) => split,
      None => return Err(PngError::Truncated),
    };

    rest = after;

    filters::unfiltered(rows, &read.header, pass.size.width)?;

    drawn(read, rows, rgba, pass, reduction)?;
  }

  Ok(())
}

fn drawn(
  read: &Read,
  rows: &[u8],
  rgba: &mut [u8],
  pass: Pass,
  reduction: Reduction,
) -> Result<(), PngError> {
  let Ok(encoded_width) = filters::row_length(&read.header, pass.size.width);
  let height = u64::from(pass.size.height);
  let encoded_size = Size {
    width: encoded_width,
    height,
  };
  let encoded_stride =
    i64::try_from(encoded_width.saturating_add(1)).map_err(|_| PngError::Corrupt)?;
  let encoded = RowLayout::new(encoded_size, 1, encoded_stride)
    .and_then(|layout| layout.rows(rows))
    .map_err(|_| PngError::Truncated)?;

  let line = u64::from(read.header.size.width).saturating_mul(RGBA);
  let output_size = Size {
    width: line,
    height,
  };
  let output_stride = i64::try_from(line.saturating_mul(u64::from(pass.step.height)))
    .map_err(|_| PngError::Corrupt)?;
  let layout = match RowLayout::new(
    output_size,
    line.saturating_mul(u64::from(pass.from.y)),
    output_stride,
  ) {
    Ok(layout) => layout,
    Err(_) => return Err(PngError::Corrupt),
  };
  let output = layout.rows_mut(rgba).map_err(|_| PngError::Corrupt)?;

  let Ok(left) = index(pass.from.x);
  let Ok(across) = index(pass.step.width);

  for (bytes, line_2) in encoded.zip(output) {
    let pixels = line_2
      .as_chunks_mut::<4>()
      .0
      .iter_mut()
      .skip(left)
      .step_by(across.max(1));
    row(read, bytes, pixels, reduction)?;
  }

  Ok(())
}

fn row<'p>(
  read: &Read,
  bytes: &[u8],
  pixels: impl Iterator<Item = &'p mut [u8; 4]>,
  reduction: Reduction,
) -> Result<(), PngError> {
  match (read.header.color, read.header.depth, read.key) {
    (Color::Rgba, 8, _) => {
      for (into, from) in pixels.zip(bytes.as_chunks::<4>().0) {
        *into = *from;
      }
    }
    (Color::Rgb, 8, None) => {
      for (into, [red, green, blue]) in pixels.zip(bytes.as_chunks::<3>().0) {
        *into = [*red, *green, *blue, OPAQUE];
      }
    }
    (Color::Grey, 8, None) => {
      for (into, grey) in pixels.zip(bytes) {
        *into = [*grey, *grey, *grey, OPAQUE];
      }
    }
    (Color::GreyAlpha, 8, _) => {
      for (into, [grey, alpha]) in pixels.zip(bytes.as_chunks::<2>().0) {
        *into = [*grey, *grey, *grey, *alpha];
      }
    }
    (Color::Indexed, 8, _) => {
      for (into, at) in pixels.zip(bytes) {
        let Ok(at_2) = index(*at);

        *into = match read.palette.get(at_2) {
          Some(entry) => *entry,
          None => return Err(PngError::Corrupt),
        };
      }
    }
    (_, _, _) => any_row(read, bytes, pixels, reduction)?,
  }

  Ok(())
}

fn any_row<'p>(
  read: &Read,
  bytes: &[u8],
  pixels: impl Iterator<Item = &'p mut [u8; 4]>,
  reduction: Reduction,
) -> Result<(), PngError> {
  let header = read.header;
  let Ok(pixel) = filters::pixel_length(&header);
  let Ok(pixel_2) = index(pixel);

  match header.depth {
    1 | 2 | 4 => {
      let Ok(samples) = packed(bytes, header.depth);

      for (into, sample) in pixels.zip(samples) {
        let pixel_3 = colored(read, [sample, 0, 0, 0], reduction)?;

        *into = pixel_3;
      }
    }
    8 => {
      for (into, sample) in pixels.zip(bytes.chunks_exact(pixel_2.max(1))) {
        let mut channels = [0u16; 4];

        for (channel, byte) in channels.iter_mut().zip(sample) {
          *channel = u16::from(*byte);
        }

        let pixel_4 = colored(read, channels, reduction)?;

        *into = pixel_4;
      }
    }
    16 => {
      for (into, sample) in pixels.zip(bytes.chunks_exact(pixel_2.max(1))) {
        let mut channels = [0u16; 4];

        for (channel, pair) in channels.iter_mut().zip(sample.as_chunks::<2>().0) {
          *channel = u16::from_be_bytes(*pair);
        }

        let pixel_5 = colored(read, channels, reduction)?;

        *into = pixel_5;
      }
    }
    _ => return Err(PngError::Corrupt),
  }

  Ok(())
}

fn packed(bytes: &[u8], depth: u32) -> Result<impl Iterator<Item = u16> + '_, Never> {
  let each = 8u32.wrapping_div(depth.max(1));
  let mask = 1u16.wrapping_shl(depth).wrapping_sub(1);

  Ok(bytes.iter().flat_map(move |byte| {
    (1..=each).map(move |at| {
      u16::from(*byte).wrapping_shr(8u32.wrapping_sub(at.wrapping_mul(depth))) & mask
    })
  }))
}

fn colored(read: &Read, channels: [u16; 4], reduction: Reduction) -> Result<[u8; 4], PngError> {
  let depth = read.header.depth;
  let [first, second, third, fourth] = channels;
  let convert = |value: u16| match (depth, reduction) {
    (16, Reduction::HighByte) => {
      let [high, _low] = value.to_be_bytes();
      Ok(high)
    }
    _ => scaled(value, depth),
  };
  let Ok(grey) = convert(first);

  match read.header.color {
    Color::Grey => {
      let alpha = match read.key {
        Some(Key::Grey(clear)) => match ::core::primitive::bool::then_some(clear == first, ()) {
          ::core::option::Option::Some(()) => CLEAR,
          ::core::option::Option::None => OPAQUE,
        },
        Some(Key::Rgb(_)) | None => OPAQUE,
      };

      Ok([grey, grey, grey, alpha])
    }
    Color::Rgb => {
      let alpha = match read.key {
        Some(Key::Rgb(clear)) => match ::core::primitive::bool::then_some(clear == [first, second, third], ()) {
          ::core::option::Option::Some(()) => CLEAR,
          ::core::option::Option::None => OPAQUE,
        },
        Some(Key::Grey(_)) | None => OPAQUE,
      };

      let Ok(green) = convert(second);
      let Ok(blue) = convert(third);

      Ok([grey, green, blue, alpha])
    }
    Color::Indexed => {
      let Ok(at) = index(first);

      match read.palette.get(at) {
        Some(entry) => Ok(*entry),
        None => Err(PngError::Corrupt),
      }
    }
    Color::GreyAlpha => {
      let Ok(alpha) = convert(second);

      Ok([grey, grey, grey, alpha])
    }
    Color::Rgba => {
      let Ok(green) = convert(second);
      let Ok(blue) = convert(third);
      let Ok(alpha) = convert(fourth);

      Ok([grey, green, blue, alpha])
    }
  }
}

fn scaled(sample: u16, depth: u32) -> Result<u8, Never> {
  let stretched = match depth {
    1 => sample.wrapping_mul(0xFF),
    2 => sample.wrapping_mul(0x55),
    4 => sample.wrapping_mul(0x11),
    16 => {
      let Ok(rounded) =
        fitted::<u32, u16>(u32::from(sample).saturating_add(128).wrapping_div(257));

      rounded
    }
    _ => sample,
  };

  fitted::<u16, u8>(stretched)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_grey_of_any_depth_keeps_black_black_and_white_white() {
    assert_eq!((scaled(0, 1), scaled(1, 1)), (Ok(0), Ok(255)));
    assert_eq!((scaled(0, 2), scaled(3, 2)), (Ok(0), Ok(255)));
    assert_eq!((scaled(0, 4), scaled(15, 4)), (Ok(0), Ok(255)));
    assert_eq!((scaled(0, 16), scaled(u16::MAX, 16)), (Ok(0), Ok(255)));
    assert_eq!(scaled(0x8080, 16), Ok(128));
  }

  #[test]
  fn samples_under_a_byte_come_off_it_from_the_high_bit_down() {
    let Ok(samples) = packed(&[0b1011_0001], 2);

    assert_eq!(samples.collect::<Vec<u16>>(), [2, 3, 0, 1]);
  }
}
