use core::cmp::Ordering;

use console_core_number_conversion::{fixed_index, index};

use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};

pub(super) struct PalettePosition {
  pub x: i32,
  pub y: i32,
}

pub(super) fn record_index(position: PalettePosition, stride: i64) -> Rav1dResult<u64> {
  let x = u64::try_from(position.x).map_err(|_negative_column| EINVAL)?;
  let y = u64::try_from(position.y).map_err(|_negative_row| EINVAL)?;
  let stride_width = u64::try_from(stride).map_err(|_negative_stride| EINVAL)?;

  match (x.cmp(&stride_width), stride_width & 1) {
    (Ordering::Less, 0) => {}
    _ => return Err(EINVAL),
  }

  let row = y.wrapping_shr(1).checked_add(x & 1).ok_or(EINVAL)?;
  let record = row
    .checked_mul(stride_width.wrapping_shr(1))
    .and_then(|offset| offset.checked_add(x.wrapping_shr(1)))
    .and_then(|offset| offset.checked_add(y & 1))
    .ok_or(EINVAL)?;
  let Ok(native_position) = index(record);
  let Ok(round_trip) = fixed_index(native_position);

  match round_trip.cmp(&record) {
    Ordering::Equal => Ok(record),
    Ordering::Less | Ordering::Greater => Err(EINVAL),
  }
}

pub(super) struct NeighborColors<'a, T> {
  pub left: &'a [T],
  pub left_count: u8,
  pub above: &'a [T],
  pub above_count: u8,
}

pub(super) fn merge_neighbors<'a, T: Copy + Ord>(
  neighbors: NeighborColors<'_, T>,
  cache: &'a mut [T; 16],
) -> Rav1dResult<&'a [T]> {
  let Ok(left_end) = index(neighbors.left_count);
  let Ok(above_end) = index(neighbors.above_count);
  let left_prefix = neighbors.left.get(..left_end).ok_or(EINVAL)?;
  let above_prefix = neighbors.above.get(..above_end).ok_or(EINVAL)?;

  let mut left = left_prefix.iter().copied().peekable();
  let mut above = above_prefix.iter().copied().peekable();
  let mut slots = cache.iter_mut();
  let mut count = 0u8;
  let mut previous = None;
  let limit = u16::from(neighbors.left_count).wrapping_add(u16::from(neighbors.above_count));

  for _ in 0..limit {
    let next = match (left.peek(), above.peek()) {
      (Some(left_value), Some(above_value)) => match left_value.cmp(above_value) {
        Ordering::Less => left.next(),
        Ordering::Equal | Ordering::Greater => above.next(),
      },
      (None, Some(_)) => above.next(),
      (Some(_), None) => left.next(),
      (None, None) => break,
    };
    let value = next.ok_or(EINVAL)?;

    match previous.cmp(&Some(value)) {
      Ordering::Equal => {}
      Ordering::Less | Ordering::Greater => {
        let slot = slots.next().ok_or(EINVAL)?;
        *slot = value;
        count = count.wrapping_add(1);
        previous = Some(value);
      }
    }
  }

  let Ok(end) = index(count);
  cache.get(..end).ok_or(EINVAL)
}

pub(super) fn merge_decoded<T: Copy + Ord>(colors: &mut [T], cached: &[T]) -> Rav1dResult<()> {
  let mut from_cache = cached.iter().copied().peekable();
  let Ok(mut fresh) = fixed_index(cached.len());
  let Ok(color_count) = fixed_index(colors.len());

  match fresh.cmp(&color_count) {
    Ordering::Greater => return Err(EINVAL),
    Ordering::Less | Ordering::Equal => {}
  }

  let mut remaining = colors;

  for _ in 0..color_count {
    let (pixel, rest) = remaining.split_first_mut().ok_or(EINVAL)?;
    let decoded = match fresh.checked_sub(1) {
      Some(offset) => {
        let Ok(position) = index(offset);
        rest.get(position).copied()
      }
      None => Some(*pixel),
    };
    let next_cached = from_cache.peek().copied();
    let selected = match decoded.filter(|value| next_cached.is_none_or(|cached_value| cached_value > *value)) {
      Some(value) => value,
      None => {
        let checked_value = fresh.checked_sub(1).ok_or(EINVAL)?;
        fresh = checked_value;
        let cached_value = from_cache.next().ok_or(EINVAL)?;
        cached_value
      }
    };
    *pixel = selected;
    remaining = rest;
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::src::recon::PaletteRead;
  use console_core_geometry::Point;

  #[test]
  fn palette_neighbor_merge_and_decoded_order()
  -> ::core::result::Result<(), crate::src::error::Rav1dError> {
    let mut storage = [0u16; 16];
    let neighborhoods = NeighborColors {
      left: &[2, 5, 5, 8],
      left_count: 4,
      above: &[2, 4, 8, 9],
      above_count: 4,
    };
    let neighbors = merge_neighbors(neighborhoods, &mut storage)?;
    assert_eq!(neighbors, [2, 4, 5, 8, 9]);
    let mut colors = [0, 0, 0, 1, 4, 7];
    assert_eq!(merge_decoded(&mut colors, &[2, 4, 8]), Ok(()));
    assert_eq!(colors, [1, 2, 4, 4, 7, 8]);
    assert_eq!(record_index(PalettePosition { x: 7, y: 5 }, 32), Ok(52));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn palette_rejects_truncated_neighbors_and_impossible_record() {
    let mut storage = [0u8; 16];
    assert_eq!(merge_neighbors(
      NeighborColors { left: &[1], left_count: 2, above: &[], above_count: 0 },
      &mut storage,
    ), Err(EINVAL));
    assert_eq!(merge_neighbors(
      NeighborColors { left: &[], left_count: 0, above: &[2], above_count: 2 },
      &mut storage,
    ), Err(EINVAL));
    let mut colors = [1, 2];
    assert_eq!(merge_decoded(&mut colors, &[1, 2, 3]), Err(EINVAL));
    assert_eq!(colors, [1, 2]);
    assert_eq!(record_index(PalettePosition { x: -1, y: 0 }, 32), Err(EINVAL));
    assert_eq!(record_index(PalettePosition { x: 32, y: 0 }, 32), Err(EINVAL));
    assert_eq!(record_index(PalettePosition { x: 0, y: 0 }, 3), Err(EINVAL));
    assert_eq!(record_index(PalettePosition { x: 0, y: 0 }, 0), Err(EINVAL));
    assert_eq!(
      record_index(
        PalettePosition { x: 0, y: i32::MAX },
        i64::MAX.wrapping_sub(1)
      ),
      Err(EINVAL)
    );
  }

  #[cfg(all(feature = "bitdepth_8", feature = "bitdepth_16"))]
  #[test]
  fn palette_frame_storage_checks_the_pixel_width()
  -> Result<(), std::collections::TryReserveError> {
    use crate::include::common::bitdepth::{BitDepth8, BitDepth16};
    use crate::src::internal::Pal;

    let mut records = Pal::default();

    records.try_resize(1)?;

    assert_eq!(records.checked_index_mut::<BitDepth8>(0).map(|_record| ()), Ok(()));
    assert!(matches!(
      records.checked_index_mut::<BitDepth8>(1),
      Err(EINVAL)
    ));
    assert!(matches!(
      records.checked_index_mut::<BitDepth16>(0),
      Err(EINVAL)
    ));

    records.try_resize(2)?;

    assert_eq!(records.checked_index_mut::<BitDepth16>(0).map(|_record| ()), Ok(()));
    assert!(matches!(
      records.checked_index_mut::<BitDepth16>(1),
      Err(EINVAL)
    ));
    ::core::result::Result::Ok(())
  }

  #[cfg(feature = "bitdepth_8")]
  #[test]
  fn palette_bitstream_and_dispatch_reject_invalid_regions()
  -> ::core::result::Result<(), crate::src::error::Rav1dError> {
    use crate::include::common::bitdepth::BitDepth8;
    use crate::src::c_arc::CArc;
    use crate::src::c_box::CBox;
    use crate::src::cdf::{rav1d_cdf_thread_copy, rav1d_cdf_thread_init_static};
    use crate::src::internal::{
      Rav1dFrameContextBdFn, Rav1dFrameData, Rav1dTaskContext, Rav1dTileStateContext,
    };
    use crate::src::msac::{MsacContext, Rav1dMsacDSPContext};
    let scratch_memory = crate::src::internal::TaskContextScratch::allocate()?;
    let Ok(mut task) = Rav1dTaskContext::new(scratch_memory);
    let mut frame = Rav1dFrameData::default();
    frame.cur.p.bpc = 8;
    frame.b4_stride = 32;
    let Ok(initial) = rav1d_cdf_thread_init_static(0);
    let cdf = rav1d_cdf_thread_copy(&initial)?;
    let Ok(dsp) = Rav1dMsacDSPContext::default();
    let Ok(bytes) = CBox::from_box(vec![0; 64].into_boxed_slice());
    let bytes_2 = CArc::wrap(bytes)?;
    let Ok(msac) = MsacContext::new(bytes_2, crate::src::msac::CdfUpdates::Allowed, &dsp);
    let mut tile = Rav1dTileStateContext { cdf, msac };
    let Ok(dispatch) = Rav1dFrameContextBdFn::new::<BitDepth8>();
    assert_eq!(
      (dispatch.read_pal_y)(&mut task, &frame, &mut tile, PaletteRead { size_context: 0, position: Point { x: 0, y: 0 } }),
      Ok(2)
    );
    let scratch = task.scratch.inter_intra_mut()?;
    let [luma, _, _] = scratch.interintra_edge_pal.pal.buf_mut::<BitDepth8>()?;
    assert_eq!(luma.get(..2), Some([0, 1].as_slice()));

    assert_eq!(
      (dispatch.read_pal_y)(&mut task, &frame, &mut tile, PaletteRead { size_context: 7, position: Point { x: 0, y: 0 } }),
      Err(EINVAL)
    );
    assert_eq!(
      (dispatch.read_pal_uv)(&mut task, &frame, &mut tile, PaletteRead { size_context: 0, position: Point { x: 32, y: 0 } }),
      Err(EINVAL)
    );
    task.frame_thread.pass = 1;
    assert_eq!(
      (dispatch.read_pal_y)(&mut task, &frame, &mut tile, PaletteRead { size_context: 0, position: Point { x: 0, y: 0 } }),
      Err(EINVAL)
    );
    ::core::result::Result::Ok(())
  }

  #[cfg(feature = "bitdepth_8")]
  #[test]
  fn public_decoder_lossless_palette_and_truncated_tile()
  -> crate::src::managed::Result<()> {
    use crate::src::managed::{Decoder, Error, Planes, Settings};

    let packet = [
      0x12, 0x00, 0x0a, 0x06, 0x18, 0x0c, 0xff, 0xd9, 0x00, 0x80, 0x32, 0x2a, 0x44, 0x00, 0x00,
      0x25, 0xb8, 0xb7, 0xe8, 0x0a, 0xb9, 0xf9, 0xbc, 0x7d, 0xa1, 0x2d, 0x96, 0x11, 0xad, 0x6b,
      0x76, 0x14, 0x6c, 0x4a, 0x52, 0xef, 0x8f, 0xd3, 0x72, 0xfc, 0x6a, 0xe6, 0xa6, 0x6d, 0x55,
      0x25, 0xec, 0x62, 0x93, 0x18, 0x89, 0x69, 0xf7, 0x58,
    ];
    let settings = Settings {
      threads: 1,
      max_frame_delay: 1,
      ..Settings::default()
    };
    let mut decoder = Decoder::with_settings(settings.clone())?;
    let picture = decoder.decode(&packet)?;
    let frame = picture.ok_or(Error::InvalidData)?;
    assert_eq!(frame.width(), Ok(16));
    assert_eq!(frame.height(), Ok(16));

    let pixels = frame.planes()?;
    let planes = match pixels {
      Planes::Depth8(planes) => planes,
      Planes::Depth16(_) => return Err(whereat::At::<Error>::from(Error::InvalidData)),
    };

    let Ok(luma) = planes.y();
    let Ok(rows) = luma.rows();

    for (y, row) in rows.enumerate() {
      let Ok(row_number) = fixed_index(y);
      let expected: [u8; 16] = core::array::from_fn(|x| {
        let Ok(column) = fixed_index(x);
        let pattern = column.wrapping_mul(7).wrapping_add(row_number.wrapping_mul(13))
          ^ column.wrapping_mul(row_number).wrapping_shr(1);

        match pattern.wrapping_rem(5).cmp(&2) {
          Ordering::Less => 200,
          Ordering::Equal | Ordering::Greater => 32,
        }
      });
      assert_eq!(row, expected);
    }

    let prefix = packet.get(..24).ok_or(Error::InvalidData)?;
    let mut truncated = prefix.to_vec();
    let length_byte = truncated.get_mut(11).ok_or(Error::InvalidData)?;
    *length_byte = 12;
    let mut decoder_2 = Decoder::with_settings(settings)?;
    let result = decoder_2.decode(&truncated);
    assert!(matches!(result, Err(error) if error.error() == &Error::InvalidData));
    ::core::result::Result::Ok(())
  }
}
