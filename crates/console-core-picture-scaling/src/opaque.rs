//! Packed opaque images, mapped between pixel centers with clamped edges.
//!
//! Axis lengths are u32. Area and coordinate setup use wider integers, and
//! the Q8 channel weights sum to 256, so the pixel arithmetic fits its lanes.
//! Quantizing a coordinate folds a weight of 256 into the next pixel with
//! weight zero. Packed weighted sums cannot carry into neighbouring channels.

use alloc::{vec, vec::Vec};
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{checked_index, index, indices, wrapped};
use console_core_simd::Simd;

const ONE: u16 = 256;
const OPAQUE_ALPHA: u8 = 255;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelOrder {
  Rgba,
  Bgra,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpaqueResizeError {
  Empty,
  SourceLength,
  DestinationLength,
  Translucent,
}

#[doc = "Resizes packed straight RGBA with bilinear filtering and clamped edges. Source pixels must be opaque. The two axes use Q8 weights and round only at the final store. Every backend produces the same bytes. Only the indicated image areas are used. An error leaves the destination untouched."]
pub fn resize_opaque(
  source: &[[u8; 4]],
  source_size: Size<u32>,
  destination: &mut [[u8; 4]],
  destination_size: Size<u32>,
  order: ChannelOrder,
) -> Result<(), OpaqueResizeError> {
  let Ok(simd) = Simd::detected();
  resize_with(
    simd,
    source,
    source_size,
    destination,
    destination_size,
    order,
  )
}

fn resize_with(
  simd: Simd,
  source: &[[u8; 4]],
  source_size: Size<u32>,
  destination: &mut [[u8; 4]],
  destination_size: Size<u32>,
  order: ChannelOrder,
) -> Result<(), OpaqueResizeError> {
  match (
    source_size.width,
    source_size.height,
    destination_size.width,
    destination_size.height,
  ) {
    (0, ..) | (_, 0, ..) | (_, _, 0, _) | (.., 0) => return Err(OpaqueResizeError::Empty),
    (1.., 1.., 1.., 1..) => {}
  }

  let Ok(source_area) = source_size.area();
  let source_length = checked_index(source_area).map_err(|_| OpaqueResizeError::SourceLength)?;
  let source_pixels = source
    .get(..source_length)
    .ok_or(OpaqueResizeError::SourceLength)?;
  let Ok(destination_area) = destination_size.area();
  let destination_length =
    checked_index(destination_area).map_err(|_| OpaqueResizeError::DestinationLength)?;
  let destination_pixels = destination
    .get_mut(..destination_length)
    .ok_or(OpaqueResizeError::DestinationLength)?;
  let (blocks, tail) = source_pixels.as_chunks::<16>();

  let opaque = blocks.iter().all(|block| {
    block
      .iter()
      .fold(OPAQUE_ALPHA, |all, &[_, _, _, alpha]| all & alpha)
      == OPAQUE_ALPHA
  }) && tail.iter().all(|&[_, _, _, alpha]| alpha == OPAQUE_ALPHA);

  match opaque.then_some(()) {
    Some(()) => {}
    None => return Err(OpaqueResizeError::Translucent),
  }

  match (source_size == destination_size, order) {
    (true, ChannelOrder::Rgba) => {
      for (from, into) in source_pixels.iter().zip(destination_pixels) {
        *into = *from;
      }

      Ok(())
    }
    (true, ChannelOrder::Bgra) => {
      for (&[r, g, b, a], out) in source_pixels.iter().zip(destination_pixels) {
        *out = [b, g, r, a];
      }

      Ok(())
    }
    (false, ChannelOrder::Rgba) => resize::<false>(
      simd,
      source_pixels,
      source_size,
      destination_pixels,
      destination_size,
    ),
    (false, ChannelOrder::Bgra) => resize::<true>(
      simd,
      source_pixels,
      source_size,
      destination_pixels,
      destination_size,
    ),
  }
}

#[derive(Clone, Copy)]
struct Coordinate {
  offset: u32,
  weight: u16,
}

#[derive(Clone, Copy)]
struct Axis {
  source: u32,
  destination: u32,
}

fn coordinates(
  Axis {
    source,
    destination,
  }: Axis,
) -> Result<impl Iterator<Item = Coordinate>, Never> {
  let half = u128::from(destination);
  let unit = half.wrapping_mul(2).max(1);
  let last = u128::from(source.wrapping_sub(1));
  Ok((0..destination).map(move |at| {
    let center = u128::from(at)
      .wrapping_mul(2)
      .wrapping_add(1)
      .wrapping_mul(u128::from(source))
      .saturating_sub(half)
      .min(last.wrapping_mul(unit));
    let quantized = center
      .wrapping_mul(u128::from(ONE))
      .wrapping_add(half)
      .wrapping_div(unit);
    let Ok(offset) = wrapped(quantized.wrapping_shr(8));
    let Ok(weight) = wrapped(quantized & u128::from(ONE.wrapping_sub(1)));
    Coordinate { offset, weight }
  }))
}

struct Columns {
  left: u32,
  offsets: Vec<u32>,
  weights: Vec<u16>,
}

impl Columns {
  fn new(axis: Axis) -> Result<Self, Never> {
    let mut left = 0_u32;
    let mut offsets = Vec::new();
    let mut weights = Vec::new();
    let Ok(samples) = coordinates(axis);
    let last = axis.source.wrapping_sub(1);

    for Coordinate { offset, weight } in samples {
      match ((offset, weight), offset.cmp(&last)) {
        ((0, 0), _) => left = left.wrapping_add(1),
        (_, core::cmp::Ordering::Less) => {
          offsets.push(offset);
          weights.push(weight);
        }
        (_, core::cmp::Ordering::Equal | core::cmp::Ordering::Greater) => {}
      }
    }

    Ok(Self {
      left,
      offsets,
      weights,
    })
  }
}

fn resize<const SWAP: bool>(
  simd: Simd,
  source: &[[u8; 4]],
  source_size: Size<u32>,
  output: &mut [[u8; 4]],
  destination: Size<u32>,
) -> Result<(), OpaqueResizeError> {
  let Ok(output_width) = index(destination.width);
  let Ok(columns) = Columns::new(Axis {
    source: source_size.width,
    destination: destination.width,
  });
  let Ok(coordinates) = coordinates(Axis {
    source: source_size.height,
    destination: destination.height,
  });
  let first_row = output
    .get(..output_width)
    .ok_or(OpaqueResizeError::DestinationLength)?;
  let mut even = CachedRow {
    source_y: None,
    pixels: vec![[0; 4]; first_row.len()],
  };
  let mut odd = CachedRow {
    source_y: None,
    pixels: vec![[0; 4]; first_row.len()],
  };

  for (target, coordinate) in output
    .chunks_exact_mut(output_width.max(1))
    .zip(coordinates)
  {
    let (first, next) = match coordinate.offset & 1 {
      0 => (&mut even, &mut odd),
      _ => (&mut odd, &mut even),
    };
    let top = first.load::<SWAP>(simd, source, source_size, &columns, coordinate.offset)?;
    let bottom = match coordinate.weight {
      0 => top,
      _ => next.load::<SWAP>(
        simd,
        source,
        source_size,
        &columns,
        coordinate.offset.wrapping_add(1),
      )?,
    };
    let Ok(()) = vertical(simd, top, bottom, coordinate.weight, target);
  }

  Ok(())
}

struct CachedRow {
  source_y: Option<u32>,
  pixels: Vec<[u16; 4]>,
}

impl CachedRow {
  fn load<const SWAP: bool>(
    &mut self,
    simd: Simd,
    input: &[[u8; 4]],
    size: Size<u32>,
    columns: &Columns,
    y: u32,
  ) -> Result<&[[u16; 4]], OpaqueResizeError> {
    match self.source_y {
      Some(cached) => match cached.cmp(&y) {
        core::cmp::Ordering::Equal => return Ok(&self.pixels),
        core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {}
      },
      None => {}
    }

    let width = u64::from(size.width);
    let start = u64::from(y).wrapping_mul(width);
    let Ok(range) = indices(start..start.wrapping_add(width));
    let row = input.get(range).ok_or(OpaqueResizeError::SourceLength)?;
    horizontal::<SWAP>(simd, row, columns, &mut self.pixels)?;
    self.source_y = Some(y);
    Ok(&self.pixels)
  }
}

#[inline]
fn packed<const SWAP: bool>([r, g, b, a]: [u8; 4]) -> Result<u64, Never> {
  let channels = match SWAP.then_some(()) {
    Some(()) => [b, 0, g, 0, r, 0, a, 0],
    None => [r, 0, g, 0, b, 0, a, 0],
  };
  Ok(u64::from_le_bytes(channels))
}

#[inline]
fn unpacked(value: u64) -> Result<[u16; 4], Never> {
  let [a, b, c, d, e, f, g, h] = value.to_le_bytes();
  Ok([
    u16::from_le_bytes([a, b]),
    u16::from_le_bytes([c, d]),
    u16::from_le_bytes([e, f]),
    u16::from_le_bytes([g, h]),
  ])
}

fn horizontal<const SWAP: bool>(
  simd: Simd,
  source: &[[u8; 4]],
  columns: &Columns,
  output: &mut [[u16; 4]],
) -> Result<(), OpaqueResizeError> {
  let Ok(left_count) = index(columns.left);
  let (left, rest) = output
    .split_at_mut_checked(left_count)
    .ok_or(OpaqueResizeError::DestinationLength)?;
  let (middle, right) = rest
    .split_at_mut_checked(columns.offsets.len())
    .ok_or(OpaqueResizeError::DestinationLength)?;
  let first = source
    .first()
    .copied()
    .ok_or(OpaqueResizeError::SourceLength)?;
  let last = source
    .last()
    .copied()
    .ok_or(OpaqueResizeError::SourceLength)?;
  let Ok(start) = packed::<SWAP>(first);
  let Ok(end) = packed::<SWAP>(last);
  let Ok(start_pixel) = unpacked(start.wrapping_shl(8));
  let Ok(end_pixel) = unpacked(end.wrapping_shl(8));
  left.fill(start_pixel);
  right.fill(end_pixel);

  let Ok(done) = simd.weighted_pairs::<SWAP>(source, &columns.offsets, &columns.weights, middle);
  let Ok(skip) = index(done);

  for ((&offset, &weight), target) in columns
    .offsets
    .iter()
    .zip(&columns.weights)
    .zip(middle)
    .skip(skip)
  {
    let Ok(at) = index(offset);
    let &[left_pixel, right_pixel] = source
      .get(at..)
      .and_then(<[[u8; 4]]>::first_chunk::<2>)
      .ok_or(OpaqueResizeError::SourceLength)?;
    let Ok(before) = packed::<SWAP>(left_pixel);
    let Ok(after) = packed::<SWAP>(right_pixel);
    let Ok(blended) = unpacked(
      before
        .wrapping_mul(u64::from(ONE.wrapping_sub(weight)))
        .wrapping_add(after.wrapping_mul(u64::from(weight))),
    );
    *target = blended;
  }

  Ok(())
}

fn vertical(
  simd: Simd,
  top: &[[u16; 4]],
  bottom: &[[u16; 4]],
  weight: u16,
  out: &mut [[u8; 4]],
) -> Result<(), Never> {
  let top_2 = top.as_flattened();
  let bottom_2 = bottom.as_flattened();
  let out_2 = out.as_flattened_mut();

  match weight {
    0 => {
      for (&a, target) in top_2.iter().zip(out_2) {
        let Ok(byte) = wrapped(u32::from(a).wrapping_add(128).wrapping_shr(8));
        *target = byte;
      }
    }
    128 => {
      for ((&a, &b), target) in top_2.iter().zip(bottom_2).zip(out_2) {
        let Ok(byte) = wrapped(
          u32::from(a)
            .wrapping_add(u32::from(b))
            .wrapping_add(256)
            .wrapping_shr(9),
        );
        *target = byte;
      }
    }
    weight_2 => {
      let Ok(done) = simd.blend_rows::<16>(top_2, bottom_2, weight_2, out_2);
      let Ok(skip) = index(done);
      let first = u32::from(ONE.wrapping_sub(weight_2));
      let second = u32::from(weight_2);

      for ((&a, &b), target) in top_2.iter().zip(bottom_2).zip(out_2).skip(skip) {
        let Ok(byte) = wrapped(
          u32::from(a)
            .wrapping_mul(first)
            .wrapping_add(u32::from(b).wrapping_mul(second))
            .wrapping_add(32768)
            .wrapping_shr(16),
        );
        *target = byte;
      }
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn every_available_backend_matches_the_scalar_result() -> Result<(), OpaqueResizeError> {
    for ((width, height), (scaled_width, scaled_height)) in [
      ((13, 7), (23, 12)),
      ((38, 23), (17, 9)),
      ((23, 35), (49, 17)),
      ((1, 5), (37, 11)),
      ((19, 1), (45, 1)),
      ((13, 7), (13, 7)),
    ] {
      let source_size = Size { width, height };
      let destination_size = Size {
        width: scaled_width,
        height: scaled_height,
      };
      let Ok(source_pixels) = source_size.area();
      let Ok(destination_pixels) = destination_size.area();
      let source: Vec<_> = (0..source_pixels)
        .map(|n| {
          let Ok(r) = wrapped(n.wrapping_mul(17));
          let Ok(g) = wrapped(n.wrapping_mul(59).wrapping_add(3));
          let Ok(b) = wrapped(n.wrapping_mul(127).wrapping_add(29));
          [r, g, b, OPAQUE_ALPHA]
        })
        .collect();

      for order in [ChannelOrder::Rgba, ChannelOrder::Bgra] {
        let mut expected: Vec<_> = (0..destination_pixels).map(|_| [0; 4]).collect();
        resize_with(
          Simd::SCALAR,
          &source,
          source_size,
          &mut expected,
          destination_size,
          order,
        )?;

        let Ok(available) = Simd::available();

        for simd in available {
          let mut actual = vec![[0; 4]; expected.len()];
          resize_with(
            simd,
            &source,
            source_size,
            &mut actual,
            destination_size,
            order,
          )?;
          assert_eq!(
            actual, expected,
            "{simd:?} {source_size:?} -> {destination_size:?} {order:?}"
          );
        }
      }
    }

    Ok(())
  }

  #[test]
  fn pixel_centers_use_clamped_edges_and_round_after_both_axes() -> Result<(), OpaqueResizeError>
  {
    let input = [
      [0, 1, 0, 255],
      [255, 0, 0, 255],
      [0, 0, 0, 255],
      [0, 0, 255, 255],
    ];
    let mut result = [[0; 4]; 10];
    resize_opaque(
      &input,
      Size {
        width: 2,
        height: 2,
      },
      &mut result,
      Size {
        width: 3,
        height: 3,
      },
      ChannelOrder::Rgba,
    )?;
    let [top_left, top_right, bottom_left, bottom_right] = input;
    let [first, _, third, _, center, _, seventh, _, ninth, unused] = result;
    assert_eq!(first, top_left);
    assert_eq!(third, top_right);
    assert_eq!(center, [64, 0, 64, 255]);
    assert_eq!(seventh, bottom_left);
    assert_eq!(ninth, bottom_right);
    assert_eq!(unused, [0; 4]);
    Ok(())
  }

  #[test]
  fn invalid_buffers_and_partial_alpha_leave_the_destination_untouched()
  -> Result<(), OpaqueResizeError> {
    let input = [[200, 80, 160, 255]; 17];
    let mut output = [[12; 4]; 9];
    let at = Size {
      width: 3,
      height: 3,
    };
    let resize = |input_2: &[[u8; 4]], original_size, output_2: &mut [[u8; 4]]| {
      resize_opaque(input_2, original_size, output_2, at, ChannelOrder::Bgra)
    };
    assert_eq!(
      resize(
        &input,
        Size {
          width: 0,
          height: 1
        },
        &mut output
      ),
      Err(OpaqueResizeError::Empty)
    );
    assert_eq!(
      resize(
        &input,
        Size {
          width: 2,
          height: 9
        },
        &mut output
      ),
      Err(OpaqueResizeError::SourceLength)
    );
    let original = Size {
      width: 17,
      height: 1,
    };
    let [short @ .., _] = &mut output;
    assert_eq!(
      resize(&input, original, short),
      Err(OpaqueResizeError::DestinationLength)
    );

    for position in [0, 15, 16] {
      let mut translucent = input;
      let pixel = translucent
        .get_mut(position)
        .ok_or(OpaqueResizeError::SourceLength)?;
      let [_, _, _, alpha] = pixel;
      *alpha = 254;
      assert_eq!(
        resize(&translucent, original, &mut output),
        Err(OpaqueResizeError::Translucent)
      );
    }

    assert_eq!(output, [[12; 4]; 9]);
    Ok(())
  }
}
