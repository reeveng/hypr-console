//! Safe SIMD implementations of intra prediction functions
#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]
//!
//! Replaces hand-written assembly with safe Rust intrinsics.
//!
//! Implemented so far:
//! - DC_128 prediction (constant fill with mid-value)
//! - Vertical prediction (copy top row)
//! - Horizontal prediction (fill from left pixels)

use crate::src::ipred::{
  EdgeFilter, EdgeUpsampling, PredictionEdge, filter_edge as filter_edge_8bpc, upsample_edge,
};
use console_core_geometry::Size;
use core::num::Wrapping;

#[cfg(target_arch = "x86_64")]
use crate::src::error::{Rav1dError, Rav1dResult};

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

#[cfg(target_arch = "x86_64")]
use archmage::X64V4xToken;
use archmage::{Desktop64, Server64, arcane};
use std::ffi::c_int;

#[cfg(target_arch = "x86_64")]
use super::partial_simd;
#[cfg(target_arch = "x86_64")]
use crate::src::safe_simd::pixel_access::{
  loadu_128, loadu_256, loadu_512, storeu_128, storeu_256, storeu_512,
};

use console_core_never::Never;
use console_core_number_conversion::index;
use core::ops::{Range, RangeFrom, RangeTo};
use core::slice::SliceIndex;

trait PredictorSliceIndex<Pixel> {
  type Native: SliceIndex<[Pixel]>;

  fn native(self) -> Result<Self::Native, Never>;
}

impl<Pixel> PredictorSliceIndex<Pixel> for u64 {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "The standard SliceIndex interface requires native slice coordinates"
    )
  )]
  type Native = console_core_api_compatibility::NativeSize;

  fn native(self) -> Result<Self::Native, Never> {
    index(self)
  }
}

impl<Pixel> PredictorSliceIndex<Pixel> for Range<u64> {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "The standard SliceIndex interface requires native slice coordinates"
    )
  )]
  type Native = Range<console_core_api_compatibility::NativeSize>;

  fn native(self) -> Result<Self::Native, Never> {
    console_core_number_conversion::indices(self)
  }
}

impl<Pixel> PredictorSliceIndex<Pixel> for RangeFrom<u64> {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "The standard SliceIndex interface requires native slice coordinates"
    )
  )]
  type Native = RangeFrom<console_core_api_compatibility::NativeSize>;

  fn native(self) -> Result<Self::Native, Never> {
    let Ok(start) = index(self.start);
    Ok(start..)
  }
}

impl<Pixel> PredictorSliceIndex<Pixel> for RangeTo<u64> {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "The standard SliceIndex interface requires native slice coordinates"
    )
  )]
  type Native = RangeTo<console_core_api_compatibility::NativeSize>;

  fn native(self) -> Result<Self::Native, Never> {
    let Ok(end) = index(self.end);
    Ok(..end)
  }
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "This fixed-width adapter preserves the standard slice lookup's Option"
  )
)]
fn predictor_get<Pixel, Coordinate>(
  slice: &[Pixel],
  position: Coordinate,
) -> Option<&<Coordinate::Native as SliceIndex<[Pixel]>>::Output>
where
  Coordinate: PredictorSliceIndex<Pixel>,
{
  let Ok(native) = position.native();
  slice.get(native)
}

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "This fixed-width adapter preserves the standard slice lookup's Option"
  )
)]
fn predictor_get_mut<Pixel, Coordinate>(
  slice: &mut [Pixel],
  position: Coordinate,
) -> Option<&mut <Coordinate::Native as SliceIndex<[Pixel]>>::Output>
where
  Coordinate: PredictorSliceIndex<Pixel>,
{
  let Ok(native) = position.native();
  slice.get_mut(native)
}

fn advance_predictor_position(step: u64, position: &mut u64) -> Result<(), Never> {
  *position = position.wrapping_add(step);
  Ok(())
}

fn rounded_dc(sum: u32, length: u64) -> Rav1dResult<u32> {
  let Ok(count) = console_core_number_conversion::wrapped::<_, u32>(length);
  sum
    .wrapping_add(count.wrapping_shr(1))
    .checked_div(count)
    .ok_or(Rav1dError::EINVAL)
}

struct PredictorOutput {
  base: u64,
  stride: i64,
  size: Size<u64>,
}

impl PredictorOutput {
  fn new(base: u64, stride: i64, size: Size<u64>) -> Result<Self, Never> {
    Ok(Self { base, stride, size })
  }

  fn rows(
    self,
    destination: &mut [u8],
    samples: PredictorStorage,
  ) -> Rav1dResult<impl Iterator<Item = &mut [u8]>> {
    let width = match samples {
      PredictorStorage::Bytes => self.size.width,
      PredictorStorage::WordSamples => {
        self.size.width.checked_mul(2).ok_or(Rav1dError::ERANGE)?
      }
    };
    let extent = Size {
      width,
      height: self.size.height,
    };
    let layout = console_core_pixel_buffers::RowLayout::new(extent, self.base, self.stride)
      .map_err(|_| Rav1dError::EINVAL)?;
    layout.rows_mut(destination).map_err(|_| Rav1dError::EINVAL)
  }
}

enum PredictorStorage {
  Bytes,
  WordSamples,
}

#[archmage::rite]
#[inline(always)]
fn fill_predictor_bytes(
  _token: Desktop64,
  row: &mut [u8],
  color: u8,
) -> Result<(), console_core_never::Never> {
  let packed = _mm256_set1_epi8(color.cast_signed());
  let (blocks, tail) = row.as_chunks_mut::<32>();

  for block in blocks {
    storeu_256!(block, packed);
  }

  let (short_blocks, remainder) = tail.as_chunks_mut::<16>();
  let narrow = _mm256_castsi256_si128(packed);

  for block in short_blocks {
    storeu_128!(block, narrow);
  }

  remainder.fill(color);
  Ok(())
}

#[arcane]
fn fill_predictor_bytes_wide(
  token: Server64,
  row: &mut [u8],
  color: u8,
) -> Result<(), console_core_never::Never> {
  let packed = _mm512_set1_epi8(color.cast_signed());
  let (blocks, remainder) = row.as_chunks_mut::<64>();

  for block in blocks {
    storeu_512!(block, packed);
  }

  fill_predictor_bytes(token.v3(), remainder, color)
}

#[archmage::rite]
#[inline(always)]
fn fill_predictor_words(
  _token: Desktop64,
  row: &mut [u8],
  color: u16,
) -> Result<(), console_core_never::Never> {
  let packed = _mm256_set1_epi16(color.cast_signed());
  let (blocks, tail) = row.as_chunks_mut::<32>();

  for block in blocks {
    storeu_256!(block, packed);
  }

  let (short_blocks, remainder) = tail.as_chunks_mut::<16>();
  let narrow = _mm256_castsi256_si128(packed);

  for block in short_blocks {
    storeu_128!(block, narrow);
  }

  for word in remainder.as_chunks_mut::<2>().0 {
    *word = color.to_le_bytes();
  }

  Ok(())
}

#[arcane]
fn fill_predictor_words_wide(
  token: Server64,
  row: &mut [u8],
  color: u16,
) -> Result<(), console_core_never::Never> {
  let packed = _mm512_set1_epi16(color.cast_signed());
  let (blocks, remainder) = row.as_chunks_mut::<64>();

  for block in blocks {
    storeu_512!(block, packed);
  }

  fill_predictor_words(token.v3(), remainder, color)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_128_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  IpredDc1288bpcInnerArguments {
    dst_base,
    stride,
    width,
    height,
  }: IpredDc1288bpcInnerArguments,
) -> Rav1dResult {
  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::Bytes)?;

  for row in rows {
    let Ok(()) = fill_predictor_bytes(_token, row, 128);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDc1288bpcInnerArguments {
  dst_base: u64,
  stride: i64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_128_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  IpredDc1288bpcAvx512InnerArguments {
    dst_base,
    stride,
    width,
    height,
  }: IpredDc1288bpcAvx512InnerArguments,
) -> Rav1dResult {
  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::Bytes)?;

  for row in rows {
    let Ok(()) = fill_predictor_bytes_wide(_token, row, 128);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDc1288bpcAvx512InnerArguments {
  dst_base: u64,
  stride: i64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_v_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredV8bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredV8bpcAvx512InnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let top_off = (Wrapping(tl_off) + Wrapping(1)).0;

  match width {
    4 => {
      let top_val = _mm_cvtsi32_si128(i32::from_le_bytes({
        let array_window = (*{
          let checked_value_25 =
            predictor_get(&*topleft_2, top_off..(Wrapping(top_off) + Wrapping(4)).0)
              .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      }));

      for y in 0..height {
        let row_off = (Wrapping(dst_base.cast_signed())
          + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
        .0
        .cast_unsigned();

        {
          let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, row_off..)
            .and_then(<[_]>::first_chunk_mut::<4>)
            .ok_or(Rav1dError::EINVAL)?;
          *explicit_byte_copy_destination = _mm_cvtsi128_si32(top_val).to_le_bytes();
        };
      }
    }
    8 => {
      let Ok(top_val) = partial_simd::mm_loadl_epi64({
        let array_window = (&*{
          let checked_value_27 =
            predictor_get(&*topleft_2, top_off..(Wrapping(top_off) + Wrapping(8)).0)
              .ok_or(Rav1dError::EINVAL)?;
          checked_value_27
        })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      });

      for y in 0..height {
        let row_off = (Wrapping(dst_base.cast_signed())
          + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
        .0
        .cast_unsigned();
        let ::core::result::Result::Ok(()) = partial_simd::mm_storel_epi64(
          {
            let array_window = (&mut *{
              let checked_value_28 =
                predictor_get_mut(&mut *dst_2, row_off..(Wrapping(row_off) + Wrapping(8)).0)
                  .ok_or(Rav1dError::EINVAL)?;
              checked_value_28
            })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
            array_window
          },
          top_val,
        );
      }
    }
    16 => {
      let checked_value =
        predictor_get(&*topleft_2, top_off..(Wrapping(top_off) + Wrapping(16)).0)
          .ok_or(Rav1dError::EINVAL)?;
      let top_val = loadu_128!(checked_value, [u8; 16]);

      for y in 0..height {
        let row_off = (Wrapping(dst_base.cast_signed())
          + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
        .0
        .cast_unsigned();
        let checked_value_2 =
          predictor_get_mut(&mut *dst_2, row_off..(Wrapping(row_off) + Wrapping(16)).0)
            .ok_or(Rav1dError::EINVAL)?;
        storeu_128!(checked_value_2, [u8; 16], top_val);
      }
    }
    32 => {
      let checked_value_3 =
        predictor_get(&*topleft_2, top_off..(Wrapping(top_off) + Wrapping(32)).0)
          .ok_or(Rav1dError::EINVAL)?;
      let top_val = loadu_256!(checked_value_3, [u8; 32]);

      for y in 0..height {
        let row_off = (Wrapping(dst_base.cast_signed())
          + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
        .0
        .cast_unsigned();
        let checked_value_4 =
          predictor_get_mut(&mut *dst_2, row_off..(Wrapping(row_off) + Wrapping(32)).0)
            .ok_or(Rav1dError::EINVAL)?;
        storeu_256!(checked_value_4, [u8; 32], top_val);
      }
    }
    64 => {
      let checked_value_5 =
        predictor_get(&*topleft_2, top_off..(Wrapping(top_off) + Wrapping(64)).0)
          .ok_or(Rav1dError::EINVAL)?;
      let top_val = loadu_512!(checked_value_5, [u8; 64]);

      for y in 0..height {
        let row_off = (Wrapping(dst_base.cast_signed())
          + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
        .0
        .cast_unsigned();
        let checked_value_6 =
          predictor_get_mut(&mut *dst_2, row_off..(Wrapping(row_off) + Wrapping(64)).0)
            .ok_or(Rav1dError::EINVAL)?;
        storeu_512!(checked_value_6, [u8; 64], top_val);
      }
    }
    _ => {
      for y in 0..height {
        let row_off = (Wrapping(dst_base.cast_signed())
          + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
        .0
        .cast_unsigned();

        {
          let checked_copy_destination = predictor_get_mut(
            &mut *dst_2,
            row_off..(Wrapping(row_off) + Wrapping(width)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          let checked_copy_source = predictor_get(
            &*topleft_2,
            top_off..(Wrapping(top_off) + Wrapping(width)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;

          for (destination_element, source_element) in checked_copy_destination
            .iter_mut()
            .zip(checked_copy_source.iter())
          {
            *destination_element = *source_element;
          }
        };
      }
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredV8bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_v_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredV8bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredV8bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;

  let top_off = (Wrapping(tl_off) + Wrapping(1)).0;

  match width {
    4 => {
      let top_val = _mm_cvtsi32_si128(i32::from_le_bytes({
        let array_window = (*{
          let checked_value_25 =
            predictor_get(&*topleft_2, top_off..(Wrapping(top_off) + Wrapping(4)).0)
              .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      }));

      for y in 0..height {
        let row_off = (Wrapping(dst_base.cast_signed())
          + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
        .0
        .cast_unsigned();

        {
          let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, row_off..)
            .and_then(<[_]>::first_chunk_mut::<4>)
            .ok_or(Rav1dError::EINVAL)?;
          *explicit_byte_copy_destination = _mm_cvtsi128_si32(top_val).to_le_bytes();
        };
      }
    }
    8 => {
      let Ok(top_val) = partial_simd::mm_loadl_epi64({
        let array_window = (&*{
          let checked_value_27 =
            predictor_get(&*topleft_2, top_off..(Wrapping(top_off) + Wrapping(8)).0)
              .ok_or(Rav1dError::EINVAL)?;
          checked_value_27
        })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      });

      for y in 0..height {
        let row_off = (Wrapping(dst_base.cast_signed())
          + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
        .0
        .cast_unsigned();
        let ::core::result::Result::Ok(()) = partial_simd::mm_storel_epi64(
          {
            let array_window = (&mut *{
              let checked_value_28 =
                predictor_get_mut(&mut *dst_2, row_off..(Wrapping(row_off) + Wrapping(8)).0)
                  .ok_or(Rav1dError::EINVAL)?;
              checked_value_28
            })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
            array_window
          },
          top_val,
        );
      }
    }
    16 => {
      let checked_value =
        predictor_get(&*topleft_2, top_off..(Wrapping(top_off) + Wrapping(16)).0)
          .ok_or(Rav1dError::EINVAL)?;
      let top_val = loadu_128!(checked_value, [u8; 16]);

      for y in 0..height {
        let row_off = (Wrapping(dst_base.cast_signed())
          + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
        .0
        .cast_unsigned();
        let checked_value_2 =
          predictor_get_mut(&mut *dst_2, row_off..(Wrapping(row_off) + Wrapping(16)).0)
            .ok_or(Rav1dError::EINVAL)?;
        storeu_128!(checked_value_2, [u8; 16], top_val);
      }
    }
    32 => {
      let checked_value_3 =
        predictor_get(&*topleft_2, top_off..(Wrapping(top_off) + Wrapping(32)).0)
          .ok_or(Rav1dError::EINVAL)?;
      let top_val = loadu_256!(checked_value_3, [u8; 32]);

      for y in 0..height {
        let row_off = (Wrapping(dst_base.cast_signed())
          + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
        .0
        .cast_unsigned();
        let checked_value_4 =
          predictor_get_mut(&mut *dst_2, row_off..(Wrapping(row_off) + Wrapping(32)).0)
            .ok_or(Rav1dError::EINVAL)?;
        storeu_256!(checked_value_4, [u8; 32], top_val);
      }
    }
    64 => {
      let checked_value_5 =
        predictor_get(&*topleft_2, top_off..(Wrapping(top_off) + Wrapping(32)).0)
          .ok_or(Rav1dError::EINVAL)?;
      let top_val0 = loadu_256!(checked_value_5, [u8; 32]);
      let checked_value_6 = predictor_get(
        &*topleft_2,
        (Wrapping(top_off) + Wrapping(32)).0..(Wrapping(top_off) + Wrapping(64)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let top_val1 = loadu_256!(checked_value_6, [u8; 32]);

      for y in 0..height {
        let row_off = (Wrapping(dst_base.cast_signed())
          + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
        .0
        .cast_unsigned();
        let checked_value_7 =
          predictor_get_mut(&mut *dst_2, row_off..(Wrapping(row_off) + Wrapping(32)).0)
            .ok_or(Rav1dError::EINVAL)?;
        storeu_256!(checked_value_7, [u8; 32], top_val0);
        let checked_value_8 = predictor_get_mut(
          &mut *dst_2,
          (Wrapping(row_off) + Wrapping(32)).0..(Wrapping(row_off) + Wrapping(64)).0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        storeu_256!(checked_value_8, [u8; 32], top_val1);
      }
    }
    _ => {
      for y in 0..height {
        let row_off = (Wrapping(dst_base.cast_signed())
          + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
        .0
        .cast_unsigned();

        {
          let checked_copy_destination = predictor_get_mut(
            &mut *dst_2,
            row_off..(Wrapping(row_off) + Wrapping(width)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          let checked_copy_source = predictor_get(
            &*topleft_2,
            top_off..(Wrapping(top_off) + Wrapping(width)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;

          for (destination_element, source_element) in checked_copy_destination
            .iter_mut()
            .zip(checked_copy_source.iter())
          {
            *destination_element = *source_element;
          }
        };
      }
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredV8bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_h_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredH8bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredH8bpcInnerArguments,
) -> Rav1dResult {
  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::Bytes)?;
  let start = tl_off.checked_sub(height).ok_or(Rav1dError::EINVAL)?;
  let edge = predictor_get(topleft, start..tl_off).ok_or(Rav1dError::EINVAL)?;

  for (row, &color) in rows.zip(edge.iter().rev()) {
    let Ok(()) = fill_predictor_bytes(_token, row, color);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredH8bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_h_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredH8bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredH8bpcAvx512InnerArguments,
) -> Rav1dResult {
  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::Bytes)?;
  let start = tl_off.checked_sub(height).ok_or(Rav1dError::EINVAL)?;
  let edge = predictor_get(topleft, start..tl_off).ok_or(Rav1dError::EINVAL)?;

  for (row, &color) in rows.zip(edge.iter().rev()) {
    let Ok(()) = fill_predictor_bytes_wide(_token, row, color);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredH8bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredDc8bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredDc8bpcAvx512InnerArguments,
) -> Rav1dResult {
  let topleft_2 = &*topleft;
  let mut sum: u32 = 0;

  for x in 0..width {
    let checked_value_4 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;

    match (
      <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4),
      &mut sum,
    ) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  for y in 0..height {
    let checked_value_5 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) - Wrapping(y)).0) - Wrapping(1)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;

    match (
      <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5),
      &mut sum,
    ) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  let total = (Wrapping(width) + Wrapping(height)).0;
  let average = rounded_dc(sum, total)?;
  let Ok(dc_val) = console_core_number_conversion::wrapped::<_, u8>(average);

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::Bytes)?;

  for row in rows {
    let Ok(()) = fill_predictor_bytes_wide(_token, row, dc_val);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDc8bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_top_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredDcTop8bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredDcTop8bpcAvx512InnerArguments,
) -> Rav1dResult {
  let topleft_2 = &*topleft;
  let mut sum: u32 = 0;

  for x in 0..width {
    let checked_value_4 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;

    match (
      <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4),
      &mut sum,
    ) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  let average = rounded_dc(sum, width)?;
  let Ok(dc_val) = console_core_number_conversion::wrapped::<_, u8>(average);

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::Bytes)?;

  for row in rows {
    let Ok(()) = fill_predictor_bytes_wide(_token, row, dc_val);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDcTop8bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_left_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredDcLeft8bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredDcLeft8bpcAvx512InnerArguments,
) -> Rav1dResult {
  let topleft_2 = &*topleft;
  let mut sum: u32 = 0;

  for y in 0..height {
    let checked_value_4 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) - Wrapping(y)).0) - Wrapping(1)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;

    match (
      <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4),
      &mut sum,
    ) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  let average = rounded_dc(sum, height)?;
  let Ok(dc_val) = console_core_number_conversion::wrapped::<_, u8>(average);

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::Bytes)?;

  for row in rows {
    let Ok(()) = fill_predictor_bytes_wide(_token, row, dc_val);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDcLeft8bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredDc8bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredDc8bpcInnerArguments,
) -> Rav1dResult {
  let topleft_2 = &*topleft;

  let mut sum: u32 = 0;

  for x in 0..width {
    let checked_value_3 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;

    match (
      <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3),
      &mut sum,
    ) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  for y in 0..height {
    let checked_value_4 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) - Wrapping(y)).0) - Wrapping(1)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;

    match (
      <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4),
      &mut sum,
    ) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  let total = (Wrapping(width) + Wrapping(height)).0;
  let average = rounded_dc(sum, total)?;
  let Ok(dc_val) = console_core_number_conversion::wrapped::<_, u8>(average);

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::Bytes)?;

  for row in rows {
    let Ok(()) = fill_predictor_bytes(_token, row, dc_val);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDc8bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_top_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredDcTop8bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredDcTop8bpcInnerArguments,
) -> Rav1dResult {
  let topleft_2 = &*topleft;

  let mut sum: u32 = 0;

  for x in 0..width {
    let checked_value_3 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;

    match (
      <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3),
      &mut sum,
    ) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  let average = rounded_dc(sum, width)?;
  let Ok(dc_val) = console_core_number_conversion::wrapped::<_, u8>(average);

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::Bytes)?;

  for row in rows {
    let Ok(()) = fill_predictor_bytes(_token, row, dc_val);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDcTop8bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_left_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredDcLeft8bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredDcLeft8bpcInnerArguments,
) -> Rav1dResult {
  let topleft_2 = &*topleft;

  let mut sum: u32 = 0;

  for y in 0..height {
    let checked_value_3 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) - Wrapping(y)).0) - Wrapping(1)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;

    match (
      <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3),
      &mut sum,
    ) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  let average = rounded_dc(sum, height)?;
  let Ok(dc_val) = console_core_number_conversion::wrapped::<_, u8>(average);

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::Bytes)?;

  for row in rows {
    let Ok(()) = fill_predictor_bytes(_token, row, dc_val);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDcLeft8bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_paeth_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredPaeth8bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredPaeth8bpcAvx512InnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let checked_value_3 = predictor_get(&*topleft_2, tl_off).ok_or(Rav1dError::EINVAL)?;
  let topleft_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3);
  let topleft_vec = _mm512_set1_epi32(topleft_val);

  for y in 0..height {
    let row_off = (Wrapping(dst_base.cast_signed())
      + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
    .0
    .cast_unsigned();
    let checked_value_4 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) - Wrapping(y)).0) - Wrapping(1)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let left_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4);
    let left_vec = _mm512_set1_epi32(left_val);

    let mut x = 0;

    for _ in 0..width.saturating_sub(x).wrapping_div(16) {
      let checked_value = predictor_get(
        &*topleft_2,
        (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0
          ..(Wrapping((Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0)
            + Wrapping(16))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let top_bytes = loadu_128!(checked_value, [u8; 16]);
      let top = _mm512_cvtepu8_epi32(top_bytes);

      let base = _mm512_sub_epi32(_mm512_add_epi32(left_vec, top), topleft_vec);

      let ldiff = _mm512_abs_epi32(_mm512_sub_epi32(left_vec, base));
      let tdiff = _mm512_abs_epi32(_mm512_sub_epi32(top, base));
      let tldiff = _mm512_abs_epi32(_mm512_sub_epi32(topleft_vec, base));

      let ld_le_td = !_mm512_cmpgt_epi32_mask(ldiff, tdiff);
      let ld_le_tld = !_mm512_cmpgt_epi32_mask(ldiff, tldiff);
      let td_le_tld = !_mm512_cmpgt_epi32_mask(tdiff, tldiff);

      let use_left = ld_le_td & ld_le_tld;

      let use_top = !use_left & td_le_tld;

      let result = _mm512_mask_blend_epi32(
        use_left,
        _mm512_mask_blend_epi32(use_top, topleft_vec, top),
        left_vec,
      );

      let clamped = _mm512_max_epi32(result, _mm512_setzero_si512());
      let result_u8: __m128i = _mm512_cvtusepi32_epi8(clamped);
      let checked_value_2 = predictor_get_mut(
        &mut *dst_2,
        (Wrapping(row_off) + Wrapping(x)).0
          ..(Wrapping((Wrapping(row_off) + Wrapping(x)).0) + Wrapping(16)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      storeu_128!(checked_value_2, [u8; 16], result_u8);

      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
    }

    let row = predictor_get_mut(
      &mut (*{
        let checked_value_25 =
          predictor_get_mut(&mut *dst_2, row_off..).ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      }),
      ..width,
    )
    .ok_or(Rav1dError::EINVAL)?;

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let checked_value_5 = predictor_get(
        &*topleft_2,
        (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let top_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
      let base = i32::wrapping_sub(
        ::core::primitive::i32::wrapping_add(left_val, top_val),
        topleft_val,
      );
      let ldiff = i32::wrapping_sub(left_val, base).abs();
      let tdiff = i32::wrapping_sub(top_val, base).abs();
      let tldiff = i32::wrapping_sub(topleft_val, base).abs();
      let result = match ::core::primitive::bool::then_some(ldiff <= tdiff && ldiff <= tldiff, ())
      {
        ::core::option::Option::Some(()) => left_val,
        ::core::option::Option::None => {
          match ::core::primitive::bool::then_some(tdiff <= tldiff, ()) {
            ::core::option::Option::Some(()) => top_val,
            ::core::option::Option::None => topleft_val,
          }
        }
      };
      *{
        let checked_value_26 = predictor_get_mut(&mut *row, x).ok_or(Rav1dError::EINVAL)?;
        checked_value_26
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredPaeth8bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_smooth_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredSmooth8bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredSmooth8bpcAvx512InnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let weights_hor = predictor_get(
    &(*{
      let checked_value_55 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        width..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_55
    }),
    ..width,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let weights_ver = predictor_get(
    &(*{
      let checked_value_56 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        height..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_56
    }),
    ..height,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let checked_value_4 = predictor_get(&*topleft_2, (Wrapping(tl_off) + Wrapping(width)).0)
    .ok_or(Rav1dError::EINVAL)?;
  let right_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4);
  let checked_value_5 = predictor_get(&*topleft_2, (Wrapping(tl_off) - Wrapping(height)).0)
    .ok_or(Rav1dError::EINVAL)?;
  let bottom_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
  let right_vec = _mm512_set1_epi32(right_val);
  let bottom_vec = _mm512_set1_epi32(bottom_val);
  let rounding = _mm512_set1_epi32(256);
  let c256 = _mm512_set1_epi32(256);
  let zero_512 = _mm512_setzero_si512();

  for y in 0..height {
    let row_off = (Wrapping(dst_base.cast_signed())
      + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
    .0
    .cast_unsigned();
    let checked_value_6 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) - Wrapping(y)).0) - Wrapping(1)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let left_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_6);
    let left_vec = _mm512_set1_epi32(left_val);
    let checked_value_7 = predictor_get(&*weights_ver, y).ok_or(Rav1dError::EINVAL)?;
    let w_v = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_7);
    let w_v_vec = _mm512_set1_epi32(w_v);
    let w_v_inv = _mm512_sub_epi32(c256, w_v_vec);

    let mut x = 0;

    for _ in 0..width.saturating_sub(x).wrapping_div(16) {
      let checked_value = predictor_get(
        &*topleft_2,
        (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0
          ..(Wrapping((Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0)
            + Wrapping(16))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let top_bytes = loadu_128!(checked_value, [u8; 16]);
      let top = _mm512_cvtepu8_epi32(top_bytes);

      let checked_value_2 = predictor_get(&*weights_hor, x..(Wrapping(x) + Wrapping(16)).0)
        .ok_or(Rav1dError::EINVAL)?;
      let wh_bytes = loadu_128!(checked_value_2, [u8; 16]);
      let w_h = _mm512_cvtepu8_epi32(wh_bytes);
      let w_h_inv = _mm512_sub_epi32(c256, w_h);

      let vert = _mm512_add_epi32(
        _mm512_mullo_epi32(w_v_vec, top),
        _mm512_mullo_epi32(w_v_inv, bottom_vec),
      );
      let hor = _mm512_add_epi32(
        _mm512_mullo_epi32(w_h, left_vec),
        _mm512_mullo_epi32(w_h_inv, right_vec),
      );

      let pred = _mm512_add_epi32(vert, hor);
      let result = _mm512_srai_epi32::<9>(_mm512_add_epi32(pred, rounding));

      let clamped = _mm512_max_epi32(result, zero_512);
      let result_u8: __m128i = _mm512_cvtusepi32_epi8(clamped);
      let checked_value_3 = predictor_get_mut(
        &mut *dst_2,
        (Wrapping(row_off) + Wrapping(x)).0
          ..(Wrapping((Wrapping(row_off) + Wrapping(x)).0) + Wrapping(16)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      storeu_128!(checked_value_3, [u8; 16], result_u8);

      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
    }

    let row = predictor_get_mut(
      &mut (*{
        let checked_value_25 =
          predictor_get_mut(&mut *dst_2, row_off..).ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      }),
      ..width,
    )
    .ok_or(Rav1dError::EINVAL)?;

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let checked_value_8 = predictor_get(
        &*topleft_2,
        (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let top_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_8);
      let checked_value_9 = predictor_get(&*weights_hor, x).ok_or(Rav1dError::EINVAL)?;
      let w_h = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_9);
      let pred = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_add(
          ::core::primitive::i32::wrapping_add(
            ::core::primitive::i32::wrapping_mul(w_v, top_val),
            ::core::primitive::i32::wrapping_mul(
              ::core::primitive::i32::wrapping_sub(256, w_v),
              bottom_val,
            ),
          ),
          ::core::primitive::i32::wrapping_mul(w_h, left_val),
        ),
        ::core::primitive::i32::wrapping_mul(
          ::core::primitive::i32::wrapping_sub(256, w_h),
          right_val,
        ),
      );
      *{
        let checked_value_26 = predictor_get_mut(&mut *row, x).ok_or(Rav1dError::EINVAL)?;
        checked_value_26
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        ::core::primitive::i32::wrapping_shr(
          ::core::primitive::i32::wrapping_add(pred, 256),
          9_u32,
        ),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredSmooth8bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_smooth_v_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredSmoothV8bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredSmoothV8bpcAvx512InnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let weights_ver = predictor_get(
    &(*{
      let checked_value_55 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        height..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_55
    }),
    ..height,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let checked_value_3 = predictor_get(&*topleft_2, (Wrapping(tl_off) - Wrapping(height)).0)
    .ok_or(Rav1dError::EINVAL)?;
  let bottom_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3);
  let bottom_vec = _mm512_set1_epi32(bottom_val);
  let rounding = _mm512_set1_epi32(128);
  let c256 = _mm512_set1_epi32(256);
  let zero_512 = _mm512_setzero_si512();

  for y in 0..height {
    let row_off = (Wrapping(dst_base.cast_signed())
      + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
    .0
    .cast_unsigned();
    let checked_value_4 = predictor_get(&*weights_ver, y).ok_or(Rav1dError::EINVAL)?;
    let w_v = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4);
    let w_v_vec = _mm512_set1_epi32(w_v);
    let w_v_inv = _mm512_sub_epi32(c256, w_v_vec);

    let mut x = 0;

    for _ in 0..width.saturating_sub(x).wrapping_div(16) {
      let checked_value = predictor_get(
        &*topleft_2,
        (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0
          ..(Wrapping((Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0)
            + Wrapping(16))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let top_bytes = loadu_128!(checked_value, [u8; 16]);
      let top = _mm512_cvtepu8_epi32(top_bytes);

      let pred = _mm512_add_epi32(
        _mm512_mullo_epi32(w_v_vec, top),
        _mm512_mullo_epi32(w_v_inv, bottom_vec),
      );
      let result = _mm512_srai_epi32::<8>(_mm512_add_epi32(pred, rounding));

      let clamped = _mm512_max_epi32(result, zero_512);
      let result_u8: __m128i = _mm512_cvtusepi32_epi8(clamped);
      let checked_value_2 = predictor_get_mut(
        &mut *dst_2,
        (Wrapping(row_off) + Wrapping(x)).0
          ..(Wrapping((Wrapping(row_off) + Wrapping(x)).0) + Wrapping(16)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      storeu_128!(checked_value_2, [u8; 16], result_u8);

      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
    }

    let row = predictor_get_mut(
      &mut (*{
        let checked_value_25 =
          predictor_get_mut(&mut *dst_2, row_off..).ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      }),
      ..width,
    )
    .ok_or(Rav1dError::EINVAL)?;

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let checked_value_5 = predictor_get(
        &*topleft_2,
        (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let top_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
      let pred = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_mul(w_v, top_val),
        ::core::primitive::i32::wrapping_mul(
          ::core::primitive::i32::wrapping_sub(256, w_v),
          bottom_val,
        ),
      );
      *{
        let checked_value_26 = predictor_get_mut(&mut *row, x).ok_or(Rav1dError::EINVAL)?;
        checked_value_26
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        ::core::primitive::i32::wrapping_shr(
          ::core::primitive::i32::wrapping_add(pred, 128),
          8_u32,
        ),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredSmoothV8bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_smooth_h_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredSmoothH8bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredSmoothH8bpcAvx512InnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let weights_hor = predictor_get(
    &(*{
      let checked_value_55 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        width..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_55
    }),
    ..width,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let checked_value_3 = predictor_get(&*topleft_2, (Wrapping(tl_off) + Wrapping(width)).0)
    .ok_or(Rav1dError::EINVAL)?;
  let right_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3);
  let right_vec = _mm512_set1_epi32(right_val);
  let rounding = _mm512_set1_epi32(128);
  let c256 = _mm512_set1_epi32(256);
  let zero_512 = _mm512_setzero_si512();

  for y in 0..height {
    let row_off = (Wrapping(dst_base.cast_signed())
      + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
    .0
    .cast_unsigned();
    let checked_value_4 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) - Wrapping(y)).0) - Wrapping(1)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let left_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4);
    let left_vec = _mm512_set1_epi32(left_val);

    let mut x = 0;

    for _ in 0..width.saturating_sub(x).wrapping_div(16) {
      let checked_value = predictor_get(&*weights_hor, x..(Wrapping(x) + Wrapping(16)).0)
        .ok_or(Rav1dError::EINVAL)?;
      let wh_bytes = loadu_128!(checked_value, [u8; 16]);
      let w_h = _mm512_cvtepu8_epi32(wh_bytes);
      let w_h_inv = _mm512_sub_epi32(c256, w_h);

      let pred = _mm512_add_epi32(
        _mm512_mullo_epi32(w_h, left_vec),
        _mm512_mullo_epi32(w_h_inv, right_vec),
      );
      let result = _mm512_srai_epi32::<8>(_mm512_add_epi32(pred, rounding));

      let clamped = _mm512_max_epi32(result, zero_512);
      let result_u8: __m128i = _mm512_cvtusepi32_epi8(clamped);
      let checked_value_2 = predictor_get_mut(
        &mut *dst_2,
        (Wrapping(row_off) + Wrapping(x)).0
          ..(Wrapping((Wrapping(row_off) + Wrapping(x)).0) + Wrapping(16)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      storeu_128!(checked_value_2, [u8; 16], result_u8);

      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
    }

    let row = predictor_get_mut(
      &mut (*{
        let checked_value_25 =
          predictor_get_mut(&mut *dst_2, row_off..).ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      }),
      ..width,
    )
    .ok_or(Rav1dError::EINVAL)?;

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let checked_value_5 = predictor_get(&*weights_hor, x).ok_or(Rav1dError::EINVAL)?;
      let w_h = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
      let pred = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_mul(w_h, left_val),
        ::core::primitive::i32::wrapping_mul(
          ::core::primitive::i32::wrapping_sub(256, w_h),
          right_val,
        ),
      );
      *{
        let checked_value_26 = predictor_get_mut(&mut *row, x).ok_or(Rav1dError::EINVAL)?;
        checked_value_26
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        ::core::primitive::i32::wrapping_shr(
          ::core::primitive::i32::wrapping_add(pred, 128),
          8_u32,
        ),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredSmoothH8bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_paeth_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredPaeth8bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredPaeth8bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let checked_value = predictor_get(&*topleft_2, tl_off).ok_or(Rav1dError::EINVAL)?;
  let topleft_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value);
  let topleft_vec = _mm256_set1_epi32(topleft_val);

  for y in 0..height {
    let row_off = (Wrapping(dst_base.cast_signed())
      + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
    .0
    .cast_unsigned();
    let checked_value_2 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) - Wrapping(y)).0) - Wrapping(1)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let left_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_2);
    let left_vec = _mm256_set1_epi32(left_val);

    let mut x = 0;

    for _ in 0..width.saturating_sub(x).wrapping_div(8) {
      let Ok(top_bytes) = partial_simd::mm_loadl_epi64({
        let array_window = (&*{
          let checked_value_25 = predictor_get(
            &*topleft_2,
            (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0
              ..(Wrapping((Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0)
                + Wrapping(8))
              .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      });
      let top_lo = _mm256_cvtepu8_epi32(top_bytes);

      let base = _mm256_sub_epi32(_mm256_add_epi32(left_vec, top_lo), topleft_vec);

      let ldiff = _mm256_abs_epi32(_mm256_sub_epi32(left_vec, base));

      let tdiff = _mm256_abs_epi32(_mm256_sub_epi32(top_lo, base));

      let tldiff = _mm256_abs_epi32(_mm256_sub_epi32(topleft_vec, base));

      let ld_le_td = _mm256_or_si256(
        _mm256_cmpgt_epi32(tdiff, ldiff),
        _mm256_cmpeq_epi32(ldiff, tdiff),
      );

      let ld_le_tld = _mm256_or_si256(
        _mm256_cmpgt_epi32(tldiff, ldiff),
        _mm256_cmpeq_epi32(ldiff, tldiff),
      );

      let td_le_tld = _mm256_or_si256(
        _mm256_cmpgt_epi32(tldiff, tdiff),
        _mm256_cmpeq_epi32(tdiff, tldiff),
      );

      let use_left = _mm256_and_si256(ld_le_td, ld_le_tld);
      let use_top = _mm256_andnot_si256(use_left, td_le_tld);

      let result = _mm256_blendv_epi8(
        _mm256_blendv_epi8(topleft_vec, top_lo, use_top),
        left_vec,
        use_left,
      );

      let packed = _mm256_shuffle_epi8(
        result,
        _mm256_setr_epi8(
          0, 4, 8, 12, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 0, 4, 8, 12, -1, -1, -1,
          -1, -1, -1, -1, -1, -1, -1, -1, -1,
        ),
      );
      let lo = _mm256_castsi256_si128(packed);
      let hi = _mm256_extracti128_si256::<1>(packed);
      let combined = _mm_unpacklo_epi32(lo, hi);
      let ::core::result::Result::Ok(()) = partial_simd::mm_storel_epi64(
        {
          let array_window = (&mut *{
            let checked_value_26 = predictor_get_mut(
              &mut *dst_2,
              (Wrapping(row_off) + Wrapping(x)).0
                ..(Wrapping((Wrapping(row_off) + Wrapping(x)).0) + Wrapping(8)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_26
          })
            .try_into()
            .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        },
        combined,
      );

      let ::core::result::Result::Ok(()) = advance_predictor_position(8, &mut x);
    }

    let row = predictor_get_mut(
      &mut (*{
        let checked_value_27 =
          predictor_get_mut(&mut *dst_2, row_off..).ok_or(Rav1dError::EINVAL)?;
        checked_value_27
      }),
      ..width,
    )
    .ok_or(Rav1dError::EINVAL)?;

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let checked_value_3 = predictor_get(
        &*topleft_2,
        (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let top_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3);
      let base = i32::wrapping_sub(
        ::core::primitive::i32::wrapping_add(left_val, top_val),
        topleft_val,
      );
      let ldiff = i32::wrapping_sub(left_val, base).abs();
      let tdiff = i32::wrapping_sub(top_val, base).abs();
      let tldiff = i32::wrapping_sub(topleft_val, base).abs();

      let result = match ::core::primitive::bool::then_some(ldiff <= tdiff && ldiff <= tldiff, ())
      {
        ::core::option::Option::Some(()) => left_val,
        ::core::option::Option::None => {
          match ::core::primitive::bool::then_some(tdiff <= tldiff, ()) {
            ::core::option::Option::Some(()) => top_val,
            ::core::option::Option::None => topleft_val,
          }
        }
      };
      *{
        let checked_value_28 = predictor_get_mut(&mut *row, x).ok_or(Rav1dError::EINVAL)?;
        checked_value_28
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredPaeth8bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

use crate::src::tables::dav1d_sm_weights;

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_smooth_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredSmooth8bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredSmooth8bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let weights_hor = predictor_get(
    &(*{
      let checked_value_55 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        width..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_55
    }),
    ..width,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let weights_ver = predictor_get(
    &(*{
      let checked_value_56 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        height..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_56
    }),
    ..height,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let checked_value = predictor_get(&*topleft_2, (Wrapping(tl_off) + Wrapping(width)).0)
    .ok_or(Rav1dError::EINVAL)?;
  let right_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value);
  let checked_value_2 = predictor_get(&*topleft_2, (Wrapping(tl_off) - Wrapping(height)).0)
    .ok_or(Rav1dError::EINVAL)?;
  let bottom_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_2);
  let right_vec = _mm256_set1_epi32(right_val);
  let bottom_vec = _mm256_set1_epi32(bottom_val);
  let rounding = _mm256_set1_epi32(256);
  let c256 = _mm256_set1_epi32(256);

  for y in 0..height {
    let row_off = (Wrapping(dst_base.cast_signed())
      + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
    .0
    .cast_unsigned();
    let checked_value_3 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) - Wrapping(y)).0) - Wrapping(1)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let left_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3);
    let left_vec = _mm256_set1_epi32(left_val);
    let checked_value_4 = predictor_get(&*weights_ver, y).ok_or(Rav1dError::EINVAL)?;
    let w_v = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4);
    let w_v_vec = _mm256_set1_epi32(w_v);
    let w_v_inv = _mm256_sub_epi32(c256, w_v_vec);

    let mut x = 0;

    for _ in 0..width.saturating_sub(x).wrapping_div(8) {
      let Ok(top_bytes) = partial_simd::mm_loadl_epi64({
        let array_window = (&*{
          let checked_value_25 = predictor_get(
            &*topleft_2,
            (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0
              ..(Wrapping((Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0)
                + Wrapping(8))
              .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      });
      let top = _mm256_cvtepu8_epi32(top_bytes);

      let Ok(w_h_bytes) = partial_simd::mm_loadl_epi64({
        let array_window = (&*{
          let checked_value_26 = predictor_get(&*weights_hor, x..(Wrapping(x) + Wrapping(8)).0)
            .ok_or(Rav1dError::EINVAL)?;
          checked_value_26
        })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      });
      let w_h = _mm256_cvtepu8_epi32(w_h_bytes);
      let w_h_inv = _mm256_sub_epi32(c256, w_h);

      let vert = _mm256_add_epi32(
        _mm256_mullo_epi32(w_v_vec, top),
        _mm256_mullo_epi32(w_v_inv, bottom_vec),
      );

      let hor = _mm256_add_epi32(
        _mm256_mullo_epi32(w_h, left_vec),
        _mm256_mullo_epi32(w_h_inv, right_vec),
      );

      let pred = _mm256_add_epi32(vert, hor);
      let result = _mm256_srai_epi32::<9>(_mm256_add_epi32(pred, rounding));

      let packed = _mm256_shuffle_epi8(
        result,
        _mm256_setr_epi8(
          0, 4, 8, 12, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 0, 4, 8, 12, -1, -1, -1,
          -1, -1, -1, -1, -1, -1, -1, -1, -1,
        ),
      );
      let lo = _mm256_castsi256_si128(packed);
      let hi = _mm256_extracti128_si256::<1>(packed);
      let combined = _mm_unpacklo_epi32(lo, hi);
      let ::core::result::Result::Ok(()) = partial_simd::mm_storel_epi64(
        {
          let array_window = (&mut *{
            let checked_value_27 = predictor_get_mut(
              &mut *dst_2,
              (Wrapping(row_off) + Wrapping(x)).0
                ..(Wrapping((Wrapping(row_off) + Wrapping(x)).0) + Wrapping(8)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_27
          })
            .try_into()
            .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        },
        combined,
      );

      let ::core::result::Result::Ok(()) = advance_predictor_position(8, &mut x);
    }

    let row = predictor_get_mut(
      &mut (*{
        let checked_value_28 =
          predictor_get_mut(&mut *dst_2, row_off..).ok_or(Rav1dError::EINVAL)?;
        checked_value_28
      }),
      ..width,
    )
    .ok_or(Rav1dError::EINVAL)?;

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let checked_value_5 = predictor_get(
        &*topleft_2,
        (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let top_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
      let checked_value_6 = predictor_get(&*weights_hor, x).ok_or(Rav1dError::EINVAL)?;
      let w_h = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_6);
      let pred = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_add(
          ::core::primitive::i32::wrapping_add(
            ::core::primitive::i32::wrapping_mul(w_v, top_val),
            ::core::primitive::i32::wrapping_mul(
              ::core::primitive::i32::wrapping_sub(256, w_v),
              bottom_val,
            ),
          ),
          ::core::primitive::i32::wrapping_mul(w_h, left_val),
        ),
        ::core::primitive::i32::wrapping_mul(
          ::core::primitive::i32::wrapping_sub(256, w_h),
          right_val,
        ),
      );
      *{
        let checked_value_29 = predictor_get_mut(&mut *row, x).ok_or(Rav1dError::EINVAL)?;
        checked_value_29
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        ::core::primitive::i32::wrapping_shr(
          ::core::primitive::i32::wrapping_add(pred, 256),
          9_u32,
        ),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredSmooth8bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_smooth_v_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredSmoothV8bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredSmoothV8bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let weights_ver = predictor_get(
    &(*{
      let checked_value_55 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        height..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_55
    }),
    ..height,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let checked_value = predictor_get(&*topleft_2, (Wrapping(tl_off) - Wrapping(height)).0)
    .ok_or(Rav1dError::EINVAL)?;
  let bottom_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value);
  let bottom_vec = _mm256_set1_epi32(bottom_val);
  let rounding = _mm256_set1_epi32(128);
  let c256 = _mm256_set1_epi32(256);

  for y in 0..height {
    let row_off = (Wrapping(dst_base.cast_signed())
      + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
    .0
    .cast_unsigned();
    let checked_value_2 = predictor_get(&*weights_ver, y).ok_or(Rav1dError::EINVAL)?;
    let w_v = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_2);
    let w_v_vec = _mm256_set1_epi32(w_v);
    let w_v_inv = _mm256_sub_epi32(c256, w_v_vec);

    let mut x = 0;

    for _ in 0..width.saturating_sub(x).wrapping_div(8) {
      let Ok(top_bytes) = partial_simd::mm_loadl_epi64({
        let array_window = (&*{
          let checked_value_25 = predictor_get(
            &*topleft_2,
            (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0
              ..(Wrapping((Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0)
                + Wrapping(8))
              .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      });
      let top = _mm256_cvtepu8_epi32(top_bytes);

      let pred = _mm256_add_epi32(
        _mm256_mullo_epi32(w_v_vec, top),
        _mm256_mullo_epi32(w_v_inv, bottom_vec),
      );

      let result = _mm256_srai_epi32::<8>(_mm256_add_epi32(pred, rounding));

      let packed = _mm256_shuffle_epi8(
        result,
        _mm256_setr_epi8(
          0, 4, 8, 12, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 0, 4, 8, 12, -1, -1, -1,
          -1, -1, -1, -1, -1, -1, -1, -1, -1,
        ),
      );
      let lo = _mm256_castsi256_si128(packed);
      let hi = _mm256_extracti128_si256::<1>(packed);
      let combined = _mm_unpacklo_epi32(lo, hi);
      let ::core::result::Result::Ok(()) = partial_simd::mm_storel_epi64(
        {
          let array_window = (&mut *{
            let checked_value_26 = predictor_get_mut(
              &mut *dst_2,
              (Wrapping(row_off) + Wrapping(x)).0
                ..(Wrapping((Wrapping(row_off) + Wrapping(x)).0) + Wrapping(8)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_26
          })
            .try_into()
            .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        },
        combined,
      );

      let ::core::result::Result::Ok(()) = advance_predictor_position(8, &mut x);
    }

    let row = predictor_get_mut(
      &mut (*{
        let checked_value_27 =
          predictor_get_mut(&mut *dst_2, row_off..).ok_or(Rav1dError::EINVAL)?;
        checked_value_27
      }),
      ..width,
    )
    .ok_or(Rav1dError::EINVAL)?;

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let checked_value_3 = predictor_get(
        &*topleft_2,
        (Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(x)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let top_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3);
      let pred = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_mul(w_v, top_val),
        ::core::primitive::i32::wrapping_mul(
          ::core::primitive::i32::wrapping_sub(256, w_v),
          bottom_val,
        ),
      );
      *{
        let checked_value_28 = predictor_get_mut(&mut *row, x).ok_or(Rav1dError::EINVAL)?;
        checked_value_28
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        ::core::primitive::i32::wrapping_shr(
          ::core::primitive::i32::wrapping_add(pred, 128),
          8_u32,
        ),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredSmoothV8bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_smooth_h_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredSmoothH8bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredSmoothH8bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let weights_hor = predictor_get(
    &(*{
      let checked_value_55 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        width..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_55
    }),
    ..width,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let checked_value = predictor_get(&*topleft_2, (Wrapping(tl_off) + Wrapping(width)).0)
    .ok_or(Rav1dError::EINVAL)?;
  let right_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value);
  let right_vec = _mm256_set1_epi32(right_val);
  let rounding = _mm256_set1_epi32(128);
  let c256 = _mm256_set1_epi32(256);

  for y in 0..height {
    let row_off = (Wrapping(dst_base.cast_signed())
      + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
    .0
    .cast_unsigned();
    let checked_value_2 = predictor_get(
      &*topleft_2,
      (Wrapping((Wrapping(tl_off) - Wrapping(y)).0) - Wrapping(1)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let left_val = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_2);
    let left_vec = _mm256_set1_epi32(left_val);

    let mut x = 0;

    for _ in 0..width.saturating_sub(x).wrapping_div(8) {
      let Ok(w_h_bytes) = partial_simd::mm_loadl_epi64({
        let array_window = (&*{
          let checked_value_25 = predictor_get(&*weights_hor, x..(Wrapping(x) + Wrapping(8)).0)
            .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      });
      let w_h = _mm256_cvtepu8_epi32(w_h_bytes);
      let w_h_inv = _mm256_sub_epi32(c256, w_h);

      let pred = _mm256_add_epi32(
        _mm256_mullo_epi32(w_h, left_vec),
        _mm256_mullo_epi32(w_h_inv, right_vec),
      );

      let result = _mm256_srai_epi32::<8>(_mm256_add_epi32(pred, rounding));

      let packed = _mm256_shuffle_epi8(
        result,
        _mm256_setr_epi8(
          0, 4, 8, 12, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 0, 4, 8, 12, -1, -1, -1,
          -1, -1, -1, -1, -1, -1, -1, -1, -1,
        ),
      );
      let lo = _mm256_castsi256_si128(packed);
      let hi = _mm256_extracti128_si256::<1>(packed);
      let combined = _mm_unpacklo_epi32(lo, hi);
      let ::core::result::Result::Ok(()) = partial_simd::mm_storel_epi64(
        {
          let array_window = (&mut *{
            let checked_value_26 = predictor_get_mut(
              &mut *dst_2,
              (Wrapping(row_off) + Wrapping(x)).0
                ..(Wrapping((Wrapping(row_off) + Wrapping(x)).0) + Wrapping(8)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_26
          })
            .try_into()
            .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        },
        combined,
      );

      let ::core::result::Result::Ok(()) = advance_predictor_position(8, &mut x);
    }

    let row = predictor_get_mut(
      &mut (*{
        let checked_value_27 =
          predictor_get_mut(&mut *dst_2, row_off..).ok_or(Rav1dError::EINVAL)?;
        checked_value_27
      }),
      ..width,
    )
    .ok_or(Rav1dError::EINVAL)?;

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let checked_value_3 = predictor_get(&*weights_hor, x).ok_or(Rav1dError::EINVAL)?;
      let w_h = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3);
      let pred = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_mul(w_h, left_val),
        ::core::primitive::i32::wrapping_mul(
          ::core::primitive::i32::wrapping_sub(256, w_h),
          right_val,
        ),
      );
      *{
        let checked_value_28 = predictor_get_mut(&mut *row, x).ok_or(Rav1dError::EINVAL)?;
        checked_value_28
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        ::core::primitive::i32::wrapping_shr(
          ::core::primitive::i32::wrapping_add(pred, 128),
          8_u32,
        ),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredSmoothH8bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

use crate::src::tables::{
  dav1d_dr_intra_derivative, dav1d_filter_intra_taps, filter_predictions,
};

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_filter_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredFilter8bpcInnerArguments {
    tl_off,
    width,
    height,
    filt_idx,
    topleft_off,
  }: IpredFilter8bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let width_2 = width.wrapping_div(4).wrapping_mul(4);
  let filt_idx_2 = ({
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(filt_idx);
    explicit_position
  }) & 511;

  let filter = &*{
    let checked_value_48 =
      predictor_get(&dav1d_filter_intra_taps, filt_idx_2).ok_or(Rav1dError::EINVAL)?;
    checked_value_48
  };

  for y in (0..height).step_by(2) {
    let ::core::result::Result::Ok(cur_tl_off) =
      ::console_core_number_conversion::fitted::<_, u64>((Wrapping(topleft_off) - Wrapping(y)).0);
    let checked_value = predictor_get(
      &*topleft_2,
      tl_off.wrapping_add(
        match ::console_core_number_conversion::fitted::<_, u64>(cur_tl_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ),
    )
    .ok_or(Rav1dError::EINVAL)?;
    let mut tl_pixel =
      <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value);

    let ::core::result::Result::Ok(row0_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping((Wrapping(::core::primitive::u64::cast_signed(y)) * Wrapping(stride)).0))
      .0
      .cast_unsigned(),
    );
    let ::core::result::Result::Ok(row1_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping(
          (Wrapping(::core::primitive::u64::cast_signed(
            (Wrapping(y) + Wrapping(1)).0,
          )) * Wrapping(stride))
          .0,
        ))
      .0
      .cast_unsigned(),
    );

    for x in (0..width_2).step_by(4) {
      let (p1, p2, p3, p4) = match ::core::primitive::bool::then_some(y == 0, ()) {
        ::core::option::Option::Some(()) => {
          let ::core::result::Result::Ok(top_base) =
            ::console_core_number_conversion::fitted::<_, u64>(
              tl_off.wrapping_add(
                (Wrapping((Wrapping(topleft_off) + Wrapping(1)).0) + Wrapping(x)).0,
              ),
            );
          let checked_value_2 = predictor_get(
            &*topleft_2,
            match ::console_core_number_conversion::fitted::<_, u64>(top_base) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          )
          .ok_or(Rav1dError::EINVAL)?;
          (
            <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_2),
            <i32 as ::core::convert::From<::core::primitive::u8>>::from(*{
              let checked_value_25 = predictor_get(
                &*topleft_2,
                (Wrapping(
                  match ::console_core_number_conversion::fitted::<_, u64>(top_base) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  },
                ) + Wrapping(1))
                .0,
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_25
            }),
            <i32 as ::core::convert::From<::core::primitive::u8>>::from(*{
              let checked_value_26 = predictor_get(
                &*topleft_2,
                (Wrapping(
                  match ::console_core_number_conversion::fitted::<_, u64>(top_base) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  },
                ) + Wrapping(2))
                .0,
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_26
            }),
            <i32 as ::core::convert::From<::core::primitive::u8>>::from(*{
              let checked_value_27 = predictor_get(
                &*topleft_2,
                (Wrapping(
                  match ::console_core_number_conversion::fitted::<_, u64>(top_base) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  },
                ) + Wrapping(3))
                .0,
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_27
            }),
          )
        }
        ::core::option::Option::None => {
          let ::core::result::Result::Ok(top_row) =
            ::console_core_number_conversion::fitted::<_, u64>(
              (Wrapping(dst_base.cast_signed())
                + Wrapping(
                  (Wrapping((Wrapping(::core::primitive::u64::cast_signed(y)) - Wrapping(1)).0)
                    * Wrapping(stride))
                  .0,
                ))
              .0
              .cast_unsigned(),
            );
          let checked_value_3 = predictor_get(
            &*dst_2,
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(top_row) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(x))
            .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          (
            <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3),
            <i32 as ::core::convert::From<::core::primitive::u8>>::from(*{
              let checked_value_28 = predictor_get(
                &*dst_2,
                (Wrapping(
                  (Wrapping(
                    match ::console_core_number_conversion::fitted::<_, u64>(top_row) {
                      ::core::result::Result::Ok(native_position) => native_position,
                    },
                  ) + Wrapping(x))
                  .0,
                ) + Wrapping(1))
                .0,
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_28
            }),
            <i32 as ::core::convert::From<::core::primitive::u8>>::from(*{
              let checked_value_29 = predictor_get(
                &*dst_2,
                (Wrapping(
                  (Wrapping(
                    match ::console_core_number_conversion::fitted::<_, u64>(top_row) {
                      ::core::result::Result::Ok(native_position) => native_position,
                    },
                  ) + Wrapping(x))
                  .0,
                ) + Wrapping(2))
                .0,
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_29
            }),
            <i32 as ::core::convert::From<::core::primitive::u8>>::from(*{
              let checked_value_30 = predictor_get(
                &*dst_2,
                (Wrapping(
                  (Wrapping(
                    match ::console_core_number_conversion::fitted::<_, u64>(top_row) {
                      ::core::result::Result::Ok(native_position) => native_position,
                    },
                  ) + Wrapping(x))
                  .0,
                ) + Wrapping(3))
                .0,
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_30
            }),
          )
        }
      };

      let (p5, p6) = match ::core::primitive::bool::then_some(x == 0, ()) {
        ::core::option::Option::Some(()) => {
          let ::core::result::Result::Ok(left_base) =
            ::console_core_number_conversion::fitted::<_, u64>(
              tl_off.wrapping_add(
                (match ::console_core_number_conversion::fitted::<_, u64>(cur_tl_off) {
                  ::core::result::Result::Ok(native_position) => native_position,
                })
                .wrapping_sub(1),
              ),
            );
          let checked_value_4 = predictor_get(
            &*topleft_2,
            match ::console_core_number_conversion::fitted::<_, u64>(left_base) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          )
          .ok_or(Rav1dError::EINVAL)?;
          (
            <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4),
            <i32 as ::core::convert::From<::core::primitive::u8>>::from(*{
              let checked_value_31 = predictor_get(
                &*topleft_2,
                (match ::console_core_number_conversion::fitted::<_, u64>(left_base) {
                  ::core::result::Result::Ok(native_position) => native_position,
                })
                .wrapping_sub(1),
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_31
            }),
          )
        }
        ::core::option::Option::None => (
          <i32 as ::core::convert::From<::core::primitive::u8>>::from(*{
            let checked_value_32 = predictor_get(
              &*dst_2,
              (Wrapping(
                (Wrapping(
                  match ::console_core_number_conversion::fitted::<_, u64>(row0_off) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  },
                ) + Wrapping(x))
                .0,
              ) - Wrapping(1))
              .0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_32
          }),
          <i32 as ::core::convert::From<::core::primitive::u8>>::from(*{
            let checked_value_33 = predictor_get(
              &*dst_2,
              (Wrapping(
                (Wrapping(
                  match ::console_core_number_conversion::fitted::<_, u64>(row1_off) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  },
                ) + Wrapping(x))
                .0,
              ) - Wrapping(1))
              .0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_33
          }),
        ),
      };

      let p0 = tl_pixel;
      let p = [p0, p1, p2, p3, p4, p5, p6];

      let Ok([first_row, second_row]) = filter_predictions(filter, p);

      for (xx, acc) in (0..4).zip(first_row) {
        let first_filtered_pixel =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
            ::core::primitive::i32::wrapping_shr(i32::wrapping_add(acc, 8), 4_u32).clamp(0, 255),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };
        *{
          let checked_value_35 = predictor_get_mut(
            &mut *dst_2,
            (Wrapping(
              (Wrapping(
                match ::console_core_number_conversion::fitted::<_, u64>(row0_off) {
                  ::core::result::Result::Ok(native_position) => native_position,
                },
              ) + Wrapping(x))
              .0,
            ) + Wrapping(xx))
            .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_35
        } = first_filtered_pixel;
      }

      for (xx, acc) in (0..4).zip(second_row) {
        let second_filtered_pixel =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
            ::core::primitive::i32::wrapping_shr(i32::wrapping_add(acc, 8), 4_u32).clamp(0, 255),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };
        *{
          let checked_value_37 = predictor_get_mut(
            &mut *dst_2,
            (Wrapping(
              (Wrapping(
                match ::console_core_number_conversion::fitted::<_, u64>(row1_off) {
                  ::core::result::Result::Ok(native_position) => native_position,
                },
              ) + Wrapping(x))
              .0,
            ) + Wrapping(xx))
            .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_37
        } = second_filtered_pixel;
      }

      tl_pixel = p4;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredFilter8bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
  filt_idx: i32,
  topleft_off: u64,
}

struct DirectionalPrediction {
  destination: PredictorOutput,
  edge_center: u64,
  angle: i32,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_z1_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  topleft: &[u8],
  prediction: DirectionalPrediction,
) -> Rav1dResult {
  let DirectionalPrediction {
    destination,
    edge_center: tl_off,
    angle,
  } = prediction;
  let PredictorOutput {
    base: dst_base,
    stride,
    size: Size { width, height },
  } = destination;
  let dst_2 = &mut *dst;
  let width_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(width) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  let height_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(height) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

  let is_sm = ::core::primitive::i32::wrapping_shr(angle, 9_u32) & 1 != 0;
  let enable_intra_edge_filter = ::core::primitive::i32::wrapping_shr(angle, 10_u32) != 0;
  let angle_2 = angle & 511;

  let checked_value_7 = predictor_get(&dav1d_dr_intra_derivative, {
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
        ::core::primitive::i32::wrapping_shr(angle_2, 1_u32),
      );
    explicit_position
  })
  .ok_or(Rav1dError::EINVAL)?;
  let mut dx = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_7);

  let upsample_above = enable_intra_edge_filter
    && i32::wrapping_sub(90, angle_2) < 40
    && ({
      let ::core::result::Result::Ok(explicit_position) =
        ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
          i32::wrapping_add(width_i, height_i),
        );
      explicit_position
    }) <= ::core::primitive::u64::wrapping_shr(
      16,
      <::core::primitive::u32 as ::core::convert::From<::core::primitive::bool>>::from(
        <u64 as ::core::convert::From<::core::primitive::bool>>::from(is_sm) != 0,
      ),
    );

  let mut top_out = [0u8; 64 + 64];
  let (top, max_base_x, base_inc);

  match ::core::primitive::bool::then_some(upsample_above, ()) {
    ::core::option::Option::Some(()) => {
      upsample_edge_8bpc(
        &mut top_out,
        i32::wrapping_add(width_i, height_i),
        PredictionEdge {
          pixels: topleft,
          origin: (Wrapping(tl_off) + Wrapping(1)).0,
          window: -1..i32::wrapping_add(width_i, std::cmp::min(width_i, height_i)),
        },
      )?;
      dx = ::core::primitive::i32::wrapping_shl(dx, 1_u32);
      top = top_out.as_slice();
      max_base_x = {
        let ::core::result::Result::Ok(explicit_position) =
          ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
            i32::wrapping_sub(
              i32::wrapping_mul(2, i32::wrapping_add(width_i, height_i)),
              2,
            ),
          );
        explicit_position
      };
      base_inc = 2u64;
    }
    ::core::option::Option::None => {
      let Ok(filter_strength) =
        match ::core::primitive::bool::then_some(enable_intra_edge_filter, ()) {
          ::core::option::Option::Some(()) => get_filter_strength(EdgeFilterSelection {
            length: i32::wrapping_add(width_i, height_i),
            angle_delta: i32::wrapping_sub(90, angle_2),
            smoothing: is_sm,
          }),
          ::core::option::Option::None => Ok(0),
        };

      match ::core::primitive::bool::then_some(filter_strength != 0, ()) {
        ::core::option::Option::Some(()) => {
          filter_edge_8bpc(
            &mut top_out,
            PredictionEdge {
              pixels: topleft,
              origin: (Wrapping(tl_off) + Wrapping(1)).0,
              window: -1..i32::wrapping_add(width_i, std::cmp::min(width_i, height_i)),
            },
            EdgeFilter {
              count: i32::wrapping_add(width_i, height_i),
              filtered: 0..i32::wrapping_add(width_i, height_i),
              strength: filter_strength,
            },
          )?;
          top = top_out.as_slice();
          max_base_x = {
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i32::wrapping_sub(i32::wrapping_add(width_i, height_i), 1),
              );
            explicit_position
          };
        }
        ::core::option::Option::None => {
          top = &*{
            let checked_value_25 = predictor_get(&*topleft, (Wrapping(tl_off) + Wrapping(1)).0..)
              .ok_or(Rav1dError::EINVAL)?;
            checked_value_25
          };
          max_base_x = (Wrapping((Wrapping(width) + Wrapping(std::cmp::min(width, height))).0)
            - Wrapping(1))
          .0;
        }
      }

      base_inc = 1;
    }
  };

  let top_2 = &*top;

  let rounding = _mm256_set1_epi16(32);

  for y in 0..height_i {
    let xpos = i32::wrapping_mul(::core::primitive::i32::wrapping_add(y, 1), dx);
    let frac =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(xpos & 0x3e) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    let inv_frac = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
      ::core::primitive::i16::wrapping_sub(64, frac),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

    let frac_vec = _mm256_set1_epi16(frac);
    let inv_frac_vec = _mm256_set1_epi16(inv_frac);

    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      dst_base
        .cast_signed()
        .wrapping_add(
          ({
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(y);
            explicit_position
          })
          .wrapping_mul(stride),
        )
        .cast_unsigned(),
    );
    let ::core::result::Result::Ok(base0) = ::console_core_number_conversion::fitted::<_, u64>({
      let ::core::result::Result::Ok(explicit_position) =
        ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
          ::core::primitive::i32::wrapping_shr(xpos, 6_u32),
        );
      explicit_position
    });

    let mut x = 0u64;

    match ::core::primitive::bool::then_some(base_inc == 1, ()) {
      ::core::option::Option::Some(()) => {
        let source_remaining = max_base_x
          .saturating_sub(base0.wrapping_add(x))
          .saturating_sub(1);
        let group_count = width
          .saturating_sub(x)
          .min(source_remaining)
          .wrapping_div(16);

        for _ in 0..group_count {
          let base = (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(base0) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping(x))
          .0;

          let checked_value = predictor_get(&*top_2, base..(Wrapping(base) + Wrapping(16)).0)
            .ok_or(Rav1dError::EINVAL)?;
          let t0 = loadu_128!(checked_value, [u8; 16]);
          let checked_value_2 = predictor_get(
            &*top_2,
            (Wrapping(base) + Wrapping(1)).0..(Wrapping(base) + Wrapping(17)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          let t1 = loadu_128!(checked_value_2, [u8; 16]);

          let t0_w = _mm256_cvtepu8_epi16(t0);
          let t1_w = _mm256_cvtepu8_epi16(t1);

          let prod0 = _mm256_mullo_epi16(t0_w, inv_frac_vec);
          let prod1 = _mm256_mullo_epi16(t1_w, frac_vec);
          let sum = _mm256_add_epi16(_mm256_add_epi16(prod0, prod1), rounding);
          let result = _mm256_srai_epi16::<6>(sum);

          let packed = _mm256_packus_epi16(result, result);
          let lo = _mm256_castsi256_si128(packed);
          let hi = _mm256_extracti128_si256::<1>(packed);
          let combined = _mm_unpacklo_epi64(lo, hi);
          let checked_value_3 = predictor_get_mut(
            &mut *dst_2,
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(x))
            .0
              ..(Wrapping(
                (Wrapping(
                  match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  },
                ) + Wrapping(x))
                .0,
              ) + Wrapping(16))
              .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          storeu_128!(checked_value_3, [u8; 16], combined);

          let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
        }
      }
      ::core::option::Option::None => {}
    }

    'inner_for_2: for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let ::core::result::Result::Ok(base) = ::console_core_number_conversion::fitted::<_, u64>(
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(base0) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping((Wrapping(base_inc) * Wrapping(x)).0))
        .0,
      );

      match ::core::primitive::bool::then_some(
        (match ::console_core_number_conversion::fitted::<_, u64>(base) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) < max_base_x,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let checked_value_4 = predictor_get(
            &*top_2,
            match ::console_core_number_conversion::fitted::<_, u64>(base) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          )
          .ok_or(Rav1dError::EINVAL)?;
          let t0 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4);
          let checked_value_5 = predictor_get(
            &*top_2,
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(base) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(1))
            .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          let t1 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
          let v = ::core::primitive::i32::wrapping_add(
            ::core::primitive::i32::wrapping_mul(
              t0,
              <i32 as ::core::convert::From<::core::primitive::i16>>::from(inv_frac),
            ),
            ::core::primitive::i32::wrapping_mul(
              t1,
              <i32 as ::core::convert::From<::core::primitive::i16>>::from(frac),
            ),
          );
          *{
            let checked_value_26 = predictor_get_mut(
              &mut *dst_2,
              (Wrapping(
                match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
                  ::core::result::Result::Ok(native_position) => native_position,
                },
              ) + Wrapping(x))
              .0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_26
          } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
            ::core::primitive::i32::wrapping_shr(
              ::core::primitive::i32::wrapping_add(v, 32),
              6_u32,
            ),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };
        }
        ::core::option::Option::None => {
          let checked_value_6 = predictor_get(&*top_2, max_base_x).ok_or(Rav1dError::EINVAL)?;
          let fill_val = *checked_value_6;

          for xx in x..width {
            *{
              let checked_value_27 = predictor_get_mut(
                &mut *dst_2,
                (Wrapping(
                  match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  },
                ) + Wrapping(xx))
                .0,
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_27
            } = fill_val;
          }

          break 'inner_for_2;
        }
      }

      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_z1_8bpc_v4x_inner(
  _token: X64V4xToken,
  dst: &mut [u8],
  topleft: &[u8],
  prediction: DirectionalPrediction,
) -> Rav1dResult {
  let DirectionalPrediction {
    destination,
    edge_center: tl_off,
    angle,
  } = prediction;
  let PredictorOutput {
    base: dst_base,
    stride,
    size: Size { width, height },
  } = destination;
  let dst_2 = &mut *dst;
  let width_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(width) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  let height_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(height) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

  let is_sm = ::core::primitive::i32::wrapping_shr(angle, 9_u32) & 1 != 0;
  let enable_intra_edge_filter = ::core::primitive::i32::wrapping_shr(angle, 10_u32) != 0;
  let angle_2 = angle & 511;

  let checked_value_6 = predictor_get(&dav1d_dr_intra_derivative, {
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
        ::core::primitive::i32::wrapping_shr(angle_2, 1_u32),
      );
    explicit_position
  })
  .ok_or(Rav1dError::EINVAL)?;
  let mut dx = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_6);

  let upsample_above = enable_intra_edge_filter
    && i32::wrapping_sub(90, angle_2) < 40
    && ({
      let ::core::result::Result::Ok(explicit_position) =
        ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
          i32::wrapping_add(width_i, height_i),
        );
      explicit_position
    }) <= ::core::primitive::u64::wrapping_shr(
      16,
      <::core::primitive::u32 as ::core::convert::From<::core::primitive::bool>>::from(
        <u64 as ::core::convert::From<::core::primitive::bool>>::from(is_sm) != 0,
      ),
    );

  let mut top_out = [0u8; 64 + 64];
  let (top, max_base_x, base_inc);

  match ::core::primitive::bool::then_some(upsample_above, ()) {
    ::core::option::Option::Some(()) => {
      upsample_edge_8bpc(
        &mut top_out,
        i32::wrapping_add(width_i, height_i),
        PredictionEdge {
          pixels: topleft,
          origin: (Wrapping(tl_off) + Wrapping(1)).0,
          window: -1..i32::wrapping_add(width_i, std::cmp::min(width_i, height_i)),
        },
      )?;
      dx = ::core::primitive::i32::wrapping_shl(dx, 1_u32);
      top = top_out.as_slice();
      max_base_x = {
        let ::core::result::Result::Ok(explicit_position) =
          ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
            i32::wrapping_sub(
              i32::wrapping_mul(2, i32::wrapping_add(width_i, height_i)),
              2,
            ),
          );
        explicit_position
      };
      base_inc = 2u64;
    }
    ::core::option::Option::None => {
      let Ok(filter_strength) =
        match ::core::primitive::bool::then_some(enable_intra_edge_filter, ()) {
          ::core::option::Option::Some(()) => get_filter_strength(EdgeFilterSelection {
            length: i32::wrapping_add(width_i, height_i),
            angle_delta: i32::wrapping_sub(90, angle_2),
            smoothing: is_sm,
          }),
          ::core::option::Option::None => Ok(0),
        };

      match ::core::primitive::bool::then_some(filter_strength != 0, ()) {
        ::core::option::Option::Some(()) => {
          filter_edge_8bpc(
            &mut top_out,
            PredictionEdge {
              pixels: topleft,
              origin: (Wrapping(tl_off) + Wrapping(1)).0,
              window: -1..i32::wrapping_add(width_i, std::cmp::min(width_i, height_i)),
            },
            EdgeFilter {
              count: i32::wrapping_add(width_i, height_i),
              filtered: 0..i32::wrapping_add(width_i, height_i),
              strength: filter_strength,
            },
          )?;
          top = top_out.as_slice();
          max_base_x = {
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i32::wrapping_sub(i32::wrapping_add(width_i, height_i), 1),
              );
            explicit_position
          };
        }
        ::core::option::Option::None => {
          top = &*{
            let checked_value_25 = predictor_get(&*topleft, (Wrapping(tl_off) + Wrapping(1)).0..)
              .ok_or(Rav1dError::EINVAL)?;
            checked_value_25
          };
          max_base_x = (Wrapping((Wrapping(width) + Wrapping(std::cmp::min(width, height))).0)
            - Wrapping(1))
          .0;
        }
      }

      base_inc = 1;
    }
  };

  let edge_len = (Wrapping(max_base_x) + Wrapping(1)).0.min(128);
  let mut ebuf = [0u8; 128];
  let top_f = &*top;

  for i in 0..edge_len {
    let checked_value = predictor_get(&*top_f, i).ok_or(Rav1dError::EINVAL)?;
    *{
      let checked_value_26 = predictor_get_mut(&mut ebuf, i).ok_or(Rav1dError::EINVAL)?;
      checked_value_26
    } = *checked_value;
  }

  let checked_value_2 = predictor_get(&*top_f, max_base_x.min(127)).ok_or(Rav1dError::EINVAL)?;
  let fill_val = *checked_value_2;

  let Ok(present) = index(edge_len);

  for b in ebuf.iter_mut().skip(present) {
    *b = fill_val;
  }

  let checked_value_27 = predictor_get(&ebuf, 0..64).ok_or(Rav1dError::EINVAL)?;
  let edge_lo = loadu_512!(checked_value_27, [u8; 64]);
  let checked_value_28 = predictor_get(&ebuf, 64..128).ok_or(Rav1dError::EINVAL)?;
  let edge_hi = loadu_512!(checked_value_28, [u8; 64]);

  let max_idx8 = _mm512_set1_epi8(
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(
      max_base_x.min(127),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    },
  );
  let rounding = _mm512_set1_epi16(32);

  let lane_off: [u8; 64] = core::array::from_fn(|i| {
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(i) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    }
  });
  let lane_off_v = loadu_512!(&lane_off, [u8; 64]);
  let one8 = _mm512_set1_epi8(1);

  for y in 0..height_i {
    let xpos = i32::wrapping_mul(::core::primitive::i32::wrapping_add(y, 1), dx);
    let frac =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(xpos & 0x3e) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    let inv_frac = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
      ::core::primitive::i16::wrapping_sub(64, frac),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

    let frac_vec = _mm512_set1_epi16(frac);
    let inv_frac_vec = _mm512_set1_epi16(inv_frac);

    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      dst_base
        .cast_signed()
        .wrapping_add(
          ({
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(y);
            explicit_position
          })
          .wrapping_mul(stride),
        )
        .cast_unsigned(),
    );
    let base0 = {
      let ::core::result::Result::Ok(explicit_position) =
        ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
          ::core::primitive::i32::wrapping_shr(xpos, 6_u32),
        );
      explicit_position
    };

    match ::core::primitive::bool::then_some(base_inc == 1, ()) {
      ::core::option::Option::Some(()) => {
        let base0_v = _mm512_set1_epi8(
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(
            base0.min(127),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          },
        );
        let mut x = 0u64;

        for _ in 0..width.saturating_sub(x).div_ceil(32) {
          let xbase = _mm512_set1_epi8(
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(
              x.min(127),
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            },
          );
          let idx0 = _mm512_adds_epu8(_mm512_adds_epu8(base0_v, xbase), lane_off_v);
          let idx0_2 = _mm512_min_epu8(idx0, max_idx8);
          let idx1 = _mm512_min_epu8(_mm512_adds_epu8(idx0_2, one8), max_idx8);

          let t0 = _mm512_permutex2var_epi8(edge_lo, idx0_2, edge_hi);
          let t1 = _mm512_permutex2var_epi8(edge_lo, idx1, edge_hi);

          let t0_lo = _mm512_cvtepu8_epi16(_mm512_castsi512_si256(t0));
          let t1_lo = _mm512_cvtepu8_epi16(_mm512_castsi512_si256(t1));
          let p0 = _mm512_mullo_epi16(t0_lo, inv_frac_vec);
          let p1 = _mm512_mullo_epi16(t1_lo, frac_vec);
          let sblend = _mm512_add_epi16(_mm512_add_epi16(p0, p1), rounding);
          let r = _mm512_srai_epi16::<6>(sblend);

          let out32 = _mm512_cvtusepi16_epi8(r);

          let n = (Wrapping(width) - Wrapping(x)).0.min(32);
          let mut tmp = [0u8; 32];
          storeu_256!(&mut tmp, [u8; 32], out32);
          let copy_start = row_off.wrapping_add(x);
          let predicted_row = predictor_get_mut(dst_2, copy_start..copy_start.wrapping_add(n))
            .ok_or(Rav1dError::EINVAL)?;
          let source = predictor_get(&tmp, ..n).ok_or(Rav1dError::EINVAL)?;

          for (output, &sample) in predicted_row.iter_mut().zip(source) {
            *output = sample;
          }

          let ::core::result::Result::Ok(()) = advance_predictor_position(32, &mut x);
        }
      }
      ::core::option::Option::None => {
        let mut x = 0u64;

        'inner_for_2: for _ in (::core::ops::Range {
          start: x,
          end: width,
        }) {
          let ::core::result::Result::Ok(base) = ::console_core_number_conversion::fitted::<_, u64>(
            (Wrapping(base0) + Wrapping((Wrapping(base_inc) * Wrapping(x)).0)).0,
          );

          match ::core::primitive::bool::then_some(
            (match ::console_core_number_conversion::fitted::<_, u64>(base) {
              ::core::result::Result::Ok(native_position) => native_position,
            }) < max_base_x,
            (),
          ) {
            ::core::option::Option::Some(()) => {
              let checked_value_3 = predictor_get(
                &*top_f,
                match ::console_core_number_conversion::fitted::<_, u64>(base) {
                  ::core::result::Result::Ok(native_position) => native_position,
                },
              )
              .ok_or(Rav1dError::EINVAL)?;
              let t0 =
                <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3);
              let checked_value_4 = predictor_get(
                &*top_f,
                (Wrapping(
                  match ::console_core_number_conversion::fitted::<_, u64>(base) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  },
                ) + Wrapping(1))
                .0,
              )
              .ok_or(Rav1dError::EINVAL)?;
              let t1 =
                <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4);
              let v = ::core::primitive::i32::wrapping_add(
                ::core::primitive::i32::wrapping_mul(
                  t0,
                  <i32 as ::core::convert::From<::core::primitive::i16>>::from(inv_frac),
                ),
                ::core::primitive::i32::wrapping_mul(
                  t1,
                  <i32 as ::core::convert::From<::core::primitive::i16>>::from(frac),
                ),
              );
              *{
                let checked_value_31 = predictor_get_mut(
                  &mut *dst_2,
                  (Wrapping(
                    match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
                      ::core::result::Result::Ok(native_position) => native_position,
                    },
                  ) + Wrapping(x))
                  .0,
                )
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_31
              } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
                ::core::primitive::i32::wrapping_shr(
                  ::core::primitive::i32::wrapping_add(v, 32),
                  6_u32,
                ),
              ) {
                ::core::result::Result::Ok(converted_position) => converted_position,
              };
            }
            ::core::option::Option::None => {
              let checked_value_5 =
                predictor_get(&*top_f, max_base_x).ok_or(Rav1dError::EINVAL)?;
              let fv = *checked_value_5;

              for xx in x..width {
                *{
                  let checked_value_32 = predictor_get_mut(
                    &mut *dst_2,
                    (Wrapping(
                      match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
                        ::core::result::Result::Ok(native_position) => native_position,
                      },
                    ) + Wrapping(xx))
                    .0,
                  )
                  .ok_or(Rav1dError::EINVAL)?;
                  checked_value_32
                } = fv;
              }

              break 'inner_for_2;
            }
          }

          x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
        }
      }
    }
  }

  Ok(())
}

fn upsample_edge_8bpc(out: &mut [u8], count: i32, input: PredictionEdge<'_, u8>) -> Rav1dResult {
  upsample_edge(
    out,
    input,
    EdgeUpsampling {
      count,
      ceiling: 255,
    },
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_z2_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  topleft: &[u8],
  prediction: DirectionalPrediction,
  reach: Size<i32>,
) -> Rav1dResult {
  let DirectionalPrediction {
    destination,
    edge_center: tl_off,
    angle,
  } = prediction;
  let PredictorOutput {
    base: dst_base,
    stride,
    size: Size { width, height },
  } = destination;
  let Size {
    width: max_width,
    height: max_height,
  } = reach;
  let dst_2 = &mut *dst;
  let width_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(width) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  let height_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(height) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

  let is_sm = ::core::primitive::i32::wrapping_shr(angle, 9_u32) & 1 != 0;
  let enable_intra_edge_filter = ::core::primitive::i32::wrapping_shr(angle, 10_u32) != 0;
  let angle_2 = angle & 511;

  let checked_value_9 = predictor_get(&dav1d_dr_intra_derivative, {
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
        ::core::primitive::i32::wrapping_shr(i32::wrapping_sub(angle_2, 90), 1_u32),
      );
    explicit_position
  })
  .ok_or(Rav1dError::EINVAL)?;
  let mut dy = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_9);
  let checked_value_10 = predictor_get(&dav1d_dr_intra_derivative, {
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
        ::core::primitive::i32::wrapping_shr(i32::wrapping_sub(180, angle_2), 1_u32),
      );
    explicit_position
  })
  .ok_or(Rav1dError::EINVAL)?;
  let mut dx = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_10);

  let upsample_left = enable_intra_edge_filter
    && i32::wrapping_sub(180, angle_2) < 40
    && i32::wrapping_add(width_i, height_i)
      <= i32::wrapping_shr(
        16,
        i32::cast_unsigned(<i32 as ::core::convert::From<::core::primitive::bool>>::from(is_sm)),
      );
  let upsample_above = enable_intra_edge_filter
    && i32::wrapping_sub(angle_2, 90) < 40
    && i32::wrapping_add(width_i, height_i)
      <= i32::wrapping_shr(
        16,
        i32::cast_unsigned(<i32 as ::core::convert::From<::core::primitive::bool>>::from(is_sm)),
      );

  let mut edge = [0u8; 64 + 64 + 1];
  let edge_tl = 64u64;

  match ::core::primitive::bool::then_some(upsample_above, ()) {
    ::core::option::Option::Some(()) => {
      upsample_edge_8bpc(
        &mut *{
          let checked_value_25 =
            predictor_get_mut(&mut edge, edge_tl..).ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        },
        i32::wrapping_add(width_i, 1),
        PredictionEdge {
          pixels: topleft,
          origin: tl_off,
          window: 0..i32::wrapping_add(width_i, 1),
        },
      )?;
      dx = ::core::primitive::i32::wrapping_shl(dx, 1_u32);
    }
    ::core::option::Option::None => {
      let Ok(filter_strength) =
        match ::core::primitive::bool::then_some(enable_intra_edge_filter, ()) {
          ::core::option::Option::Some(()) => get_filter_strength(EdgeFilterSelection {
            length: i32::wrapping_add(width_i, height_i),
            angle_delta: i32::wrapping_sub(angle_2, 90),
            smoothing: is_sm,
          }),
          ::core::option::Option::None => Ok(0),
        };

      match ::core::primitive::bool::then_some(filter_strength != 0, ()) {
        ::core::option::Option::Some(()) => {
          filter_edge_8bpc(
            &mut *{
              let checked_value_26 = predictor_get_mut(&mut edge, edge_tl.wrapping_add(1)..)
                .ok_or(Rav1dError::EINVAL)?;
              checked_value_26
            },
            PredictionEdge {
              pixels: topleft,
              origin: (Wrapping(tl_off) + Wrapping(1)).0,
              window: -1..width_i,
            },
            EdgeFilter {
              count: width_i,
              filtered: 0..max_width,
              strength: filter_strength,
            },
          )?;
        }
        ::core::option::Option::None => {
          {
            let checked_copy_destination = predictor_get_mut(
              &mut edge,
              edge_tl.wrapping_add(1)..(Wrapping(edge_tl.wrapping_add(1)) + Wrapping(width)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            let checked_copy_source = predictor_get(
              &*topleft,
              (Wrapping(tl_off) + Wrapping(1)).0
                ..(Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(width)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;

            for (destination_element, source_element) in checked_copy_destination
              .iter_mut()
              .zip(checked_copy_source.iter())
            {
              *destination_element = *source_element;
            }
          };
        }
      }
    }
  }

  match ::core::primitive::bool::then_some(upsample_left, ()) {
    ::core::option::Option::Some(()) => {
      upsample_edge_8bpc(
        &mut *{
          let checked_value_29 = predictor_get_mut(
            &mut edge,
            (Wrapping(edge_tl) - Wrapping((Wrapping(height) * Wrapping(2)).0)).0..,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_29
        },
        i32::wrapping_add(height_i, 1),
        PredictionEdge {
          pixels: topleft,
          origin: tl_off.wrapping_sub(height),
          window: 0..i32::wrapping_add(height_i, 1),
        },
      )?;
      dy = ::core::primitive::i32::wrapping_shl(dy, 1_u32);
    }
    ::core::option::Option::None => {
      let Ok(filter_strength) =
        match ::core::primitive::bool::then_some(enable_intra_edge_filter, ()) {
          ::core::option::Option::Some(()) => get_filter_strength(EdgeFilterSelection {
            length: i32::wrapping_add(width_i, height_i),
            angle_delta: i32::wrapping_sub(180, angle_2),
            smoothing: is_sm,
          }),
          ::core::option::Option::None => Ok(0),
        };

      match ::core::primitive::bool::then_some(filter_strength != 0, ()) {
        ::core::option::Option::Some(()) => {
          filter_edge_8bpc(
            &mut *{
              let checked_value_30 =
                predictor_get_mut(&mut edge, (Wrapping(edge_tl) - Wrapping(height)).0..)
                  .ok_or(Rav1dError::EINVAL)?;
              checked_value_30
            },
            PredictionEdge {
              pixels: topleft,
              origin: tl_off.wrapping_sub(height),
              window: 0..i32::wrapping_add(height_i, 1),
            },
            EdgeFilter {
              count: height_i,
              filtered: i32::wrapping_sub(height_i, max_height)..height_i,
              strength: filter_strength,
            },
          )?;
        }
        ::core::option::Option::None => {
          {
            let checked_copy_destination =
              predictor_get_mut(&mut edge, (Wrapping(edge_tl) - Wrapping(height)).0..edge_tl)
                .ok_or(Rav1dError::EINVAL)?;
            let checked_copy_source =
              predictor_get(&*topleft, (Wrapping(tl_off) - Wrapping(height)).0..tl_off)
                .ok_or(Rav1dError::EINVAL)?;

            for (destination_element, source_element) in checked_copy_destination
              .iter_mut()
              .zip(checked_copy_source.iter())
            {
              *destination_element = *source_element;
            }
          };
        }
      }
    }
  }

  let checked_value_4 = predictor_get(&*topleft, tl_off).ok_or(Rav1dError::EINVAL)?;
  *{
    let checked_value_33 = predictor_get_mut(&mut edge, edge_tl).ok_or(Rav1dError::EINVAL)?;
    checked_value_33
  } = *checked_value_4;

  let edge_2 = edge.as_slice();

  let base_inc_x = (Wrapping(1)
    + Wrapping(<u64 as ::core::convert::From<::core::primitive::bool>>::from(upsample_above)))
  .0;
  let left = (Wrapping(edge_tl)
    - Wrapping(
      (Wrapping(1)
        + Wrapping(<u64 as ::core::convert::From<::core::primitive::bool>>::from(upsample_left)))
      .0,
    ))
  .0;

  let rounding = _mm256_set1_epi16(32);

  for y in 0..height_i {
    let xpos = i32::wrapping_sub(
      ::core::primitive::i32::wrapping_shl(
        ::core::primitive::i32::wrapping_add(
          1,
          <i32 as ::core::convert::From<::core::primitive::bool>>::from(upsample_above),
        ),
        6_u32,
      ),
      i32::wrapping_mul(dx, ::core::primitive::i32::wrapping_add(y, 1)),
    );
    let base_x0 = ::core::primitive::i32::wrapping_shr(xpos, 6_u32);
    let frac_x =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(xpos & 0x3e) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    let inv_frac_x = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
      ::core::primitive::i16::wrapping_sub(64, frac_x),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      dst_base
        .cast_signed()
        .wrapping_add(
          ({
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(y);
            explicit_position
          })
          .wrapping_mul(stride),
        )
        .cast_unsigned(),
    );

    let left_count = match ::core::primitive::bool::then_some(base_x0 >= 0, ()) {
      ::core::option::Option::Some(()) => 0u64,
      ::core::option::Option::None => {
        let ::core::result::Result::Ok(needed) =
          ::console_core_number_conversion::fitted::<_, u64>({
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i32::wrapping_neg(base_x0),
              );
            explicit_position
          });
        (match ::console_core_number_conversion::fitted::<_, u64>(needed) {
          ::core::result::Result::Ok(native_position) => native_position,
        })
        .div_ceil(base_inc_x)
        .min(width)
      }
    };

    let mut x = 0u64;

    for _ in (::core::ops::Range {
      start: x,
      end: left_count,
    }) {
      let ypos = i32::wrapping_sub(
        i32::wrapping_shl(
          y,
          i32::cast_unsigned(::core::primitive::i32::wrapping_add(
            6,
            <i32 as ::core::convert::From<::core::primitive::bool>>::from(upsample_left),
          )),
        ),
        i32::wrapping_mul(
          dy,
          i32::wrapping_add(
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(x) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            },
            1,
          ),
        ),
      );
      let base_y = ::core::primitive::i32::wrapping_shr(ypos, 6_u32);
      let frac_y = ypos & 0x3e;
      let inv_frac_y = ::core::primitive::i32::wrapping_sub(64, frac_y);

      let ::core::result::Result::Ok(l0_idx) =
        ::console_core_number_conversion::fitted::<_, u64>(left.wrapping_add_signed({
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
              i32::wrapping_neg(base_y),
            );
          explicit_position
        }));
      let ::core::result::Result::Ok(l1_idx) =
        ::console_core_number_conversion::fitted::<_, u64>(left.wrapping_add_signed({
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
              i32::wrapping_neg(i32::wrapping_add(base_y, 1)),
            );
          explicit_position
        }));
      let checked_value_5 = predictor_get(
        &*edge_2,
        match ::console_core_number_conversion::fitted::<_, u64>(l0_idx) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      )
      .ok_or(Rav1dError::EINVAL)?;
      let l0 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
      let checked_value_6 = predictor_get(
        &*edge_2,
        match ::console_core_number_conversion::fitted::<_, u64>(l1_idx) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      )
      .ok_or(Rav1dError::EINVAL)?;
      let l1 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_6);
      let v = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_mul(l0, inv_frac_y),
        ::core::primitive::i32::wrapping_mul(l1, frac_y),
      );
      *{
        let checked_value_34 = predictor_get_mut(
          &mut *dst_2,
          (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping(x))
          .0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_34
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_add(v, 32), 6_u32),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }

    match ::core::primitive::bool::then_some(base_inc_x == 1, ()) {
      ::core::option::Option::Some(()) => {
        'inner_while_2: for _ in 0..width.saturating_sub(x).wrapping_div(16) {
          let ::core::result::Result::Ok(base_x) =
            ::console_core_number_conversion::fitted::<_, u64>({
              let ::core::result::Result::Ok(explicit_position) =
                ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                  i32::wrapping_add(
                    base_x0,
                    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
                      x,
                    ) {
                      ::core::result::Result::Ok(converted_position) => converted_position,
                    },
                  ),
                );
              explicit_position
            });
          let idx = (Wrapping(edge_tl)
            + Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(base_x) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ))
          .0;

          match ::core::primitive::bool::then_some(
            (Wrapping(idx) + Wrapping(17)).0 > {
              let Ok(length) = console_core_number_conversion::fixed_index(edge_2.len());
              length
            },
            (),
          ) {
            ::core::option::Option::Some(()) => {
              break 'inner_while_2;
            }
            ::core::option::Option::None => {}
          }

          let checked_value = predictor_get(&*edge_2, idx..(Wrapping(idx) + Wrapping(16)).0)
            .ok_or(Rav1dError::EINVAL)?;
          let t0 = loadu_128!(checked_value, [u8; 16]);
          let checked_value_2 = predictor_get(
            &*edge_2,
            (Wrapping(idx) + Wrapping(1)).0..(Wrapping(idx) + Wrapping(17)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          let t1 = loadu_128!(checked_value_2, [u8; 16]);

          let t0_w = _mm256_cvtepu8_epi16(t0);
          let t1_w = _mm256_cvtepu8_epi16(t1);

          let frac_vec = _mm256_set1_epi16(frac_x);
          let inv_frac_vec = _mm256_set1_epi16(inv_frac_x);

          let prod0 = _mm256_mullo_epi16(t0_w, inv_frac_vec);
          let prod1 = _mm256_mullo_epi16(t1_w, frac_vec);
          let sum = _mm256_add_epi16(_mm256_add_epi16(prod0, prod1), rounding);
          let result = _mm256_srai_epi16::<6>(sum);

          let packed = _mm256_packus_epi16(result, result);
          let lo = _mm256_castsi256_si128(packed);
          let hi = _mm256_extracti128_si256::<1>(packed);
          let combined = _mm_unpacklo_epi64(lo, hi);
          let checked_value_3 = predictor_get_mut(
            &mut *dst_2,
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(x))
            .0
              ..(Wrapping(
                (Wrapping(
                  match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  },
                ) + Wrapping(x))
                .0,
              ) + Wrapping(16))
              .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          storeu_128!(checked_value_3, [u8; 16], combined);

          let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
        }
      }
      ::core::option::Option::None => {}
    }

    'inner_for_2: for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let ::core::result::Result::Ok(base_x) =
        ::console_core_number_conversion::fitted::<_, u64>({
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
              i32::wrapping_add(
                base_x0,
                match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
                  (Wrapping(base_inc_x) * Wrapping(x)).0,
                ) {
                  ::core::result::Result::Ok(converted_position) => converted_position,
                },
              ),
            );
          explicit_position
        });
      let ::core::result::Result::Ok(idx) = ::console_core_number_conversion::fitted::<_, u64>(
        (Wrapping(edge_tl)
          + Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(base_x) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ))
        .0,
      );

      match ::core::primitive::bool::then_some(
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(idx) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(2))
        .0 > {
          let Ok(length) = console_core_number_conversion::fixed_index(edge_2.len());
          length
        },
        (),
      ) {
        ::core::option::Option::Some(()) => {
          break 'inner_for_2;
        }
        ::core::option::Option::None => {}
      }

      let checked_value_7 = predictor_get(
        &*edge_2,
        match ::console_core_number_conversion::fitted::<_, u64>(idx) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      )
      .ok_or(Rav1dError::EINVAL)?;
      let t0 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_7);
      let checked_value_8 = predictor_get(
        &*edge_2,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(idx) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(1))
        .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let t1 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_8);
      let v = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_mul(
          t0,
          <i32 as ::core::convert::From<::core::primitive::i16>>::from(inv_frac_x),
        ),
        ::core::primitive::i32::wrapping_mul(
          t1,
          <i32 as ::core::convert::From<::core::primitive::i16>>::from(frac_x),
        ),
      );
      *{
        let checked_value_35 = predictor_get_mut(
          &mut *dst_2,
          (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping(x))
          .0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_35
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_add(v, 32), 6_u32),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_z2_8bpc_v4x_inner(
  _token: X64V4xToken,
  dst: &mut [u8],
  topleft: &[u8],
  prediction: DirectionalPrediction,
  reach: Size<i32>,
) -> Rav1dResult {
  let DirectionalPrediction {
    destination,
    edge_center: tl_off,
    angle,
  } = prediction;
  let PredictorOutput {
    base: dst_base,
    stride,
    size: Size { width, height },
  } = destination;
  let Size {
    width: max_width,
    height: max_height,
  } = reach;
  let dst_2 = &mut *dst;
  let width_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(width) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  let height_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(height) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

  let is_sm = ::core::primitive::i32::wrapping_shr(angle, 9_u32) & 1 != 0;
  let enable_intra_edge_filter = ::core::primitive::i32::wrapping_shr(angle, 10_u32) != 0;
  let angle_2 = angle & 511;

  let checked_value_7 = predictor_get(&dav1d_dr_intra_derivative, {
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
        ::core::primitive::i32::wrapping_shr(i32::wrapping_sub(angle_2, 90), 1_u32),
      );
    explicit_position
  })
  .ok_or(Rav1dError::EINVAL)?;
  let mut dy = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_7);
  let checked_value_8 = predictor_get(&dav1d_dr_intra_derivative, {
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
        ::core::primitive::i32::wrapping_shr(i32::wrapping_sub(180, angle_2), 1_u32),
      );
    explicit_position
  })
  .ok_or(Rav1dError::EINVAL)?;
  let mut dx = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_8);

  let upsample_left = enable_intra_edge_filter
    && i32::wrapping_sub(180, angle_2) < 40
    && i32::wrapping_add(width_i, height_i)
      <= i32::wrapping_shr(
        16,
        i32::cast_unsigned(<i32 as ::core::convert::From<::core::primitive::bool>>::from(is_sm)),
      );
  let upsample_above = enable_intra_edge_filter
    && i32::wrapping_sub(angle_2, 90) < 40
    && i32::wrapping_add(width_i, height_i)
      <= i32::wrapping_shr(
        16,
        i32::cast_unsigned(<i32 as ::core::convert::From<::core::primitive::bool>>::from(is_sm)),
      );

  let mut edge = [0u8; 64 + 64 + 1];
  let edge_tl = 64u64;

  match ::core::primitive::bool::then_some(upsample_above, ()) {
    ::core::option::Option::Some(()) => {
      upsample_edge_8bpc(
        &mut *{
          let checked_value_25 =
            predictor_get_mut(&mut edge, edge_tl..).ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        },
        i32::wrapping_add(width_i, 1),
        PredictionEdge {
          pixels: topleft,
          origin: tl_off,
          window: 0..i32::wrapping_add(width_i, 1),
        },
      )?;
      dx = ::core::primitive::i32::wrapping_shl(dx, 1_u32);
    }
    ::core::option::Option::None => {
      let Ok(filter_strength) =
        match ::core::primitive::bool::then_some(enable_intra_edge_filter, ()) {
          ::core::option::Option::Some(()) => get_filter_strength(EdgeFilterSelection {
            length: i32::wrapping_add(width_i, height_i),
            angle_delta: i32::wrapping_sub(angle_2, 90),
            smoothing: is_sm,
          }),
          ::core::option::Option::None => Ok(0),
        };

      match ::core::primitive::bool::then_some(filter_strength != 0, ()) {
        ::core::option::Option::Some(()) => {
          filter_edge_8bpc(
            &mut *{
              let checked_value_26 = predictor_get_mut(&mut edge, edge_tl.wrapping_add(1)..)
                .ok_or(Rav1dError::EINVAL)?;
              checked_value_26
            },
            PredictionEdge {
              pixels: topleft,
              origin: (Wrapping(tl_off) + Wrapping(1)).0,
              window: -1..width_i,
            },
            EdgeFilter {
              count: width_i,
              filtered: 0..max_width,
              strength: filter_strength,
            },
          )?;
        }
        ::core::option::Option::None => {
          {
            let checked_copy_destination = predictor_get_mut(
              &mut edge,
              edge_tl.wrapping_add(1)..(Wrapping(edge_tl.wrapping_add(1)) + Wrapping(width)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            let checked_copy_source = predictor_get(
              &*topleft,
              (Wrapping(tl_off) + Wrapping(1)).0
                ..(Wrapping((Wrapping(tl_off) + Wrapping(1)).0) + Wrapping(width)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;

            for (destination_element, source_element) in checked_copy_destination
              .iter_mut()
              .zip(checked_copy_source.iter())
            {
              *destination_element = *source_element;
            }
          };
        }
      }
    }
  }

  match ::core::primitive::bool::then_some(upsample_left, ()) {
    ::core::option::Option::Some(()) => {
      upsample_edge_8bpc(
        &mut *{
          let checked_value_29 = predictor_get_mut(
            &mut edge,
            (Wrapping(edge_tl) - Wrapping((Wrapping(height) * Wrapping(2)).0)).0..,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_29
        },
        i32::wrapping_add(height_i, 1),
        PredictionEdge {
          pixels: topleft,
          origin: tl_off.wrapping_sub(height),
          window: 0..i32::wrapping_add(height_i, 1),
        },
      )?;
      dy = ::core::primitive::i32::wrapping_shl(dy, 1_u32);
    }
    ::core::option::Option::None => {
      let Ok(filter_strength) =
        match ::core::primitive::bool::then_some(enable_intra_edge_filter, ()) {
          ::core::option::Option::Some(()) => get_filter_strength(EdgeFilterSelection {
            length: i32::wrapping_add(width_i, height_i),
            angle_delta: i32::wrapping_sub(180, angle_2),
            smoothing: is_sm,
          }),
          ::core::option::Option::None => Ok(0),
        };

      match ::core::primitive::bool::then_some(filter_strength != 0, ()) {
        ::core::option::Option::Some(()) => {
          filter_edge_8bpc(
            &mut *{
              let checked_value_30 =
                predictor_get_mut(&mut edge, (Wrapping(edge_tl) - Wrapping(height)).0..)
                  .ok_or(Rav1dError::EINVAL)?;
              checked_value_30
            },
            PredictionEdge {
              pixels: topleft,
              origin: tl_off.wrapping_sub(height),
              window: 0..i32::wrapping_add(height_i, 1),
            },
            EdgeFilter {
              count: height_i,
              filtered: i32::wrapping_sub(height_i, max_height)..height_i,
              strength: filter_strength,
            },
          )?;
        }
        ::core::option::Option::None => {
          {
            let checked_copy_destination =
              predictor_get_mut(&mut edge, (Wrapping(edge_tl) - Wrapping(height)).0..edge_tl)
                .ok_or(Rav1dError::EINVAL)?;
            let checked_copy_source =
              predictor_get(&*topleft, (Wrapping(tl_off) - Wrapping(height)).0..tl_off)
                .ok_or(Rav1dError::EINVAL)?;

            for (destination_element, source_element) in checked_copy_destination
              .iter_mut()
              .zip(checked_copy_source.iter())
            {
              *destination_element = *source_element;
            }
          };
        }
      }
    }
  }

  let checked_value = predictor_get(&*topleft, tl_off).ok_or(Rav1dError::EINVAL)?;
  *{
    let checked_value_33 = predictor_get_mut(&mut edge, edge_tl).ok_or(Rav1dError::EINVAL)?;
    checked_value_33
  } = *checked_value;

  let Ok(edge_len) = console_core_number_conversion::fixed_index(edge.len());
  let edge_2 = edge.as_slice();

  let base_inc_x = (Wrapping(1)
    + Wrapping(<u64 as ::core::convert::From<::core::primitive::bool>>::from(upsample_above)))
  .0;
  let left = (Wrapping(edge_tl)
    - Wrapping(
      (Wrapping(1)
        + Wrapping(<u64 as ::core::convert::From<::core::primitive::bool>>::from(upsample_left)))
      .0,
    ))
  .0;

  let top_k_max = (Wrapping((Wrapping(edge_len) - Wrapping(1)).0) - Wrapping(edge_tl)).0;
  let mut tbuf = [0u8; 128];

  for k in 0..=top_k_max {
    let checked_value_2 =
      predictor_get(&*edge_2, (Wrapping(edge_tl) + Wrapping(k)).0).ok_or(Rav1dError::EINVAL)?;
    *{
      let checked_value_34 = predictor_get_mut(&mut tbuf, k).ok_or(Rav1dError::EINVAL)?;
      checked_value_34
    } = *checked_value_2;
  }

  let checked_value_35 = predictor_get(&tbuf, 0..64).ok_or(Rav1dError::EINVAL)?;
  let top_lo = loadu_512!(checked_value_35, [u8; 64]);
  let checked_value_36 = predictor_get(&tbuf, 64..128).ok_or(Rav1dError::EINVAL)?;
  let top_hi = loadu_512!(checked_value_36, [u8; 64]);
  let lane_off: [u8; 64] = core::array::from_fn(|i| {
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(i) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    }
  });
  let lane_off_v = loadu_512!(&lane_off, [u8; 64]);
  let one8 = _mm512_set1_epi8(1);
  let rounding512 = _mm512_set1_epi16(32);

  for y in 0..height_i {
    let xpos = i32::wrapping_sub(
      ::core::primitive::i32::wrapping_shl(
        ::core::primitive::i32::wrapping_add(
          1,
          <i32 as ::core::convert::From<::core::primitive::bool>>::from(upsample_above),
        ),
        6_u32,
      ),
      i32::wrapping_mul(dx, ::core::primitive::i32::wrapping_add(y, 1)),
    );
    let base_x0 = ::core::primitive::i32::wrapping_shr(xpos, 6_u32);
    let frac_x =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(xpos & 0x3e) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    let inv_frac_x = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
      ::core::primitive::i16::wrapping_sub(64, frac_x),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      dst_base
        .cast_signed()
        .wrapping_add(
          ({
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(y);
            explicit_position
          })
          .wrapping_mul(stride),
        )
        .cast_unsigned(),
    );

    let left_count = match ::core::primitive::bool::then_some(base_x0 >= 0, ()) {
      ::core::option::Option::Some(()) => 0u64,
      ::core::option::Option::None => {
        let ::core::result::Result::Ok(needed) =
          ::console_core_number_conversion::fitted::<_, u64>({
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i32::wrapping_neg(base_x0),
              );
            explicit_position
          });
        (match ::console_core_number_conversion::fitted::<_, u64>(needed) {
          ::core::result::Result::Ok(native_position) => native_position,
        })
        .div_ceil(base_inc_x)
        .min(width)
      }
    };

    let mut x = 0u64;

    for _ in (::core::ops::Range {
      start: x,
      end: left_count,
    }) {
      let ypos = i32::wrapping_sub(
        i32::wrapping_shl(
          y,
          i32::cast_unsigned(::core::primitive::i32::wrapping_add(
            6,
            <i32 as ::core::convert::From<::core::primitive::bool>>::from(upsample_left),
          )),
        ),
        i32::wrapping_mul(
          dy,
          i32::wrapping_add(
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(x) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            },
            1,
          ),
        ),
      );
      let base_y = ::core::primitive::i32::wrapping_shr(ypos, 6_u32);
      let frac_y = ypos & 0x3e;
      let inv_frac_y = ::core::primitive::i32::wrapping_sub(64, frac_y);

      let ::core::result::Result::Ok(l0_idx) =
        ::console_core_number_conversion::fitted::<_, u64>(left.wrapping_add_signed({
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
              i32::wrapping_neg(base_y),
            );
          explicit_position
        }));
      let ::core::result::Result::Ok(l1_idx) =
        ::console_core_number_conversion::fitted::<_, u64>(left.wrapping_add_signed({
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
              i32::wrapping_neg(i32::wrapping_add(base_y, 1)),
            );
          explicit_position
        }));
      let checked_value_3 = predictor_get(
        &*edge_2,
        match ::console_core_number_conversion::fitted::<_, u64>(l0_idx) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      )
      .ok_or(Rav1dError::EINVAL)?;
      let l0 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3);
      let checked_value_4 = predictor_get(
        &*edge_2,
        match ::console_core_number_conversion::fitted::<_, u64>(l1_idx) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      )
      .ok_or(Rav1dError::EINVAL)?;
      let l1 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4);
      let v = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_mul(l0, inv_frac_y),
        ::core::primitive::i32::wrapping_mul(l1, frac_y),
      );
      *{
        let checked_value_37 = predictor_get_mut(
          &mut *dst_2,
          (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping(x))
          .0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_add(v, 32), 6_u32),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }

    match ::core::primitive::bool::then_some(base_inc_x == 1, ()) {
      ::core::option::Option::Some(()) => {
        let frac_vec = _mm512_set1_epi16(frac_x);
        let inv_frac_vec = _mm512_set1_epi16(inv_frac_x);

        'inner_while_2: for _ in 0..width.saturating_sub(x).wrapping_div(32) {
          let base_x = i32::wrapping_add(
            base_x0,
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(x) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            },
          );

          match ::core::primitive::bool::then_some(
            ({
              let ::core::result::Result::Ok(explicit_position) =
                ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(base_x);
              explicit_position
            })
            .wrapping_add(31)
            .wrapping_add(1)
              > top_k_max,
            (),
          ) {
            ::core::option::Option::Some(()) => {
              break 'inner_while_2;
            }
            ::core::option::Option::None => {}
          }

          let k0 = _mm512_set1_epi8(
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(
              ({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                    base_x,
                  );
                explicit_position
              })
              .min(127),
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            },
          );
          let idx0 = _mm512_adds_epu8(k0, lane_off_v);
          let idx1 = _mm512_adds_epu8(idx0, one8);

          let t0 = _mm512_permutex2var_epi8(top_lo, idx0, top_hi);
          let t1 = _mm512_permutex2var_epi8(top_lo, idx1, top_hi);

          let t0_lo = _mm512_cvtepu8_epi16(_mm512_castsi512_si256(t0));
          let t1_lo = _mm512_cvtepu8_epi16(_mm512_castsi512_si256(t1));
          let p0 = _mm512_mullo_epi16(t0_lo, inv_frac_vec);
          let p1 = _mm512_mullo_epi16(t1_lo, frac_vec);
          let sblend = _mm512_add_epi16(_mm512_add_epi16(p0, p1), rounding512);
          let r = _mm512_srai_epi16::<6>(sblend);
          let out32 = _mm512_cvtusepi16_epi8(r);

          let mut tmp = [0u8; 32];
          storeu_256!(&mut tmp, [u8; 32], out32);
          let copy_start = row_off.wrapping_add(x);
          let output_window = predictor_get_mut(dst_2, copy_start..copy_start.wrapping_add(32))
            .ok_or(Rav1dError::EINVAL)?;
          let predicted_row: &mut [u8; 32] =
            output_window.try_into().map_err(|_| Rav1dError::EINVAL)?;
          *predicted_row = tmp;
          let ::core::result::Result::Ok(()) = advance_predictor_position(32, &mut x);
        }
      }
      ::core::option::Option::None => {}
    }

    'inner_for_2: for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let ::core::result::Result::Ok(base_x) =
        ::console_core_number_conversion::fitted::<_, u64>({
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
              i32::wrapping_add(
                base_x0,
                match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
                  (Wrapping(base_inc_x) * Wrapping(x)).0,
                ) {
                  ::core::result::Result::Ok(converted_position) => converted_position,
                },
              ),
            );
          explicit_position
        });
      let ::core::result::Result::Ok(idx) = ::console_core_number_conversion::fitted::<_, u64>(
        (Wrapping(edge_tl)
          + Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(base_x) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ))
        .0,
      );

      match ::core::primitive::bool::then_some(
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(idx) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(2))
        .0 > edge_len,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          break 'inner_for_2;
        }
        ::core::option::Option::None => {}
      }

      let checked_value_5 = predictor_get(
        &*edge_2,
        match ::console_core_number_conversion::fitted::<_, u64>(idx) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      )
      .ok_or(Rav1dError::EINVAL)?;
      let t0 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
      let checked_value_6 = predictor_get(
        &*edge_2,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(idx) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(1))
        .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let t1 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_6);
      let v = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_mul(
          t0,
          <i32 as ::core::convert::From<::core::primitive::i16>>::from(inv_frac_x),
        ),
        ::core::primitive::i32::wrapping_mul(
          t1,
          <i32 as ::core::convert::From<::core::primitive::i16>>::from(frac_x),
        ),
      );
      *{
        let checked_value_39 = predictor_get_mut(
          &mut *dst_2,
          (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping(x))
          .0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_39
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_add(v, 32), 6_u32),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_z3_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  topleft: &[u8],
  prediction: DirectionalPrediction,
) -> Rav1dResult {
  let DirectionalPrediction {
    destination,
    edge_center: tl_off,
    angle,
  } = prediction;
  let PredictorOutput {
    base: dst_base,
    stride,
    size: Size { width, height },
  } = destination;
  let dst_2 = &mut *dst;
  let width_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(width) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  let height_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(height) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

  let is_sm = ::core::primitive::i32::wrapping_shr(angle, 9_u32) & 1 != 0;
  let enable_intra_edge_filter = ::core::primitive::i32::wrapping_shr(angle, 10_u32) != 0;
  let angle_2 = angle & 511;

  let checked_value_4 = predictor_get(&dav1d_dr_intra_derivative, {
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
        ::core::primitive::i32::wrapping_shr(i32::wrapping_sub(270, angle_2), 1_u32),
      );
    explicit_position
  })
  .ok_or(Rav1dError::EINVAL)?;
  let mut dy = <u64 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_4);

  let upsample_left = enable_intra_edge_filter
    && i32::wrapping_sub(angle_2, 180) < 40
    && i32::wrapping_add(width_i, height_i)
      <= i32::wrapping_shr(
        16,
        i32::cast_unsigned(<i32 as ::core::convert::From<::core::primitive::bool>>::from(is_sm)),
      );

  let mut left_out = [0u8; 64 + 64];
  let (left, left_off, max_base_y, base_inc);

  match ::core::primitive::bool::then_some(upsample_left, ()) {
    ::core::option::Option::Some(()) => {
      upsample_edge_8bpc(
        &mut left_out,
        i32::wrapping_add(width_i, height_i),
        PredictionEdge {
          pixels: topleft,
          origin: (Wrapping(tl_off) - Wrapping((Wrapping(width) + Wrapping(height)).0)).0,
          window: std::cmp::max(i32::wrapping_sub(width_i, height_i), 0)
            ..i32::wrapping_add(i32::wrapping_add(width_i, height_i), 1),
        },
      )?;
      left_off = {
        let ::core::result::Result::Ok(explicit_position) =
          ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
            i32::wrapping_sub(
              i32::wrapping_mul(2, i32::wrapping_add(width_i, height_i)),
              2,
            ),
          );
        explicit_position
      };
      max_base_y = left_off;
      dy = ::core::primitive::u64::wrapping_shl(dy, 1_u32);
      base_inc = 2u64;
      left = left_out.as_slice();
    }
    ::core::option::Option::None => {
      let Ok(filter_strength) =
        match ::core::primitive::bool::then_some(enable_intra_edge_filter, ()) {
          ::core::option::Option::Some(()) => get_filter_strength(EdgeFilterSelection {
            length: i32::wrapping_add(width_i, height_i),
            angle_delta: i32::wrapping_sub(angle_2, 180),
            smoothing: is_sm,
          }),
          ::core::option::Option::None => Ok(0),
        };

      match ::core::primitive::bool::then_some(filter_strength != 0, ()) {
        ::core::option::Option::Some(()) => {
          filter_edge_8bpc(
            &mut left_out,
            PredictionEdge {
              pixels: topleft,
              origin: (Wrapping(tl_off) - Wrapping((Wrapping(width) + Wrapping(height)).0)).0,
              window: std::cmp::max(i32::wrapping_sub(width_i, height_i), 0)
                ..i32::wrapping_add(i32::wrapping_add(width_i, height_i), 1),
            },
            EdgeFilter {
              count: i32::wrapping_add(width_i, height_i),
              filtered: 0..i32::wrapping_add(width_i, height_i),
              strength: filter_strength,
            },
          )?;
          left_off = {
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i32::wrapping_sub(i32::wrapping_add(width_i, height_i), 1),
              );
            explicit_position
          };
          max_base_y = left_off;
          left = left_out.as_slice();
        }
        ::core::option::Option::None => {
          left = topleft;
          left_off = (Wrapping(tl_off) - Wrapping(1)).0;
          max_base_y = (Wrapping((Wrapping(height) + Wrapping(std::cmp::min(width, height))).0)
            - Wrapping(1))
          .0;
        }
      }

      base_inc = 1;
    }
  };

  let left_2 = &*left;

  for x in 0..width {
    let ::core::result::Result::Ok(ypos) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dy) * Wrapping((Wrapping(x) + Wrapping(1)).0)).0,
    );
    let frac = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
      (match ::console_core_number_conversion::fitted::<_, u64>(ypos) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) & 0x3e,
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    let inv_frac = ::core::primitive::i32::wrapping_sub(64, frac);

    'over_y_2: for y in 0..height_i {
      let ::core::result::Result::Ok(base) = ::console_core_number_conversion::fitted::<_, u64>(
        (match ::console_core_number_conversion::fitted::<_, u64>(ypos) {
          ::core::result::Result::Ok(native_position) => native_position,
        })
        .wrapping_shr(6_u32)
        .wrapping_add(
          (::core::num::Wrapping(base_inc)
            * ::core::num::Wrapping({
              let ::core::result::Result::Ok(explicit_position) =
                ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(y);
              explicit_position
            }))
          .0,
        ),
      );

      match ::core::primitive::bool::then_some(
        (match ::console_core_number_conversion::fitted::<_, u64>(base) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) < max_base_y,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let checked_value = predictor_get(
            &*left_2,
            (Wrapping(left_off)
              - Wrapping(
                match ::console_core_number_conversion::fitted::<_, u64>(base) {
                  ::core::result::Result::Ok(native_position) => native_position,
                },
              ))
            .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          let l0 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value);
          let checked_value_2 = predictor_get(
            &*left_2,
            (Wrapping(
              (Wrapping(left_off)
                - Wrapping(
                  match ::console_core_number_conversion::fitted::<_, u64>(base) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  },
                ))
              .0,
            ) - Wrapping(1))
            .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          let l1 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_2);
          let v = ::core::primitive::i32::wrapping_add(
            ::core::primitive::i32::wrapping_mul(l0, inv_frac),
            ::core::primitive::i32::wrapping_mul(l1, frac),
          );
          let ::core::result::Result::Ok(pixel_off) =
            ::console_core_number_conversion::fitted::<_, u64>(
              dst_base
                .cast_signed()
                .wrapping_add(
                  ({
                    let ::core::result::Result::Ok(explicit_position) =
                      ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(y);
                    explicit_position
                  })
                  .wrapping_mul(stride),
                )
                .cast_unsigned()
                .wrapping_add(x),
            );
          *{
            let checked_value_25 = predictor_get_mut(
              &mut *dst_2,
              match ::console_core_number_conversion::fitted::<_, u64>(pixel_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_25
          } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
            ::core::primitive::i32::wrapping_shr(
              ::core::primitive::i32::wrapping_add(v, 32),
              6_u32,
            ),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };
        }
        ::core::option::Option::None => {
          let checked_value_3 =
            predictor_get(&*left_2, (Wrapping(left_off) - Wrapping(max_base_y)).0)
              .ok_or(Rav1dError::EINVAL)?;
          let fill_val = *checked_value_3;

          for yy in y..height_i {
            let ::core::result::Result::Ok(pixel_off) =
              ::console_core_number_conversion::fitted::<_, u64>(
                dst_base
                  .cast_signed()
                  .wrapping_add(
                    ({
                      let ::core::result::Result::Ok(explicit_position) =
                        ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                          yy,
                        );
                      explicit_position
                    })
                    .wrapping_mul(stride),
                  )
                  .cast_unsigned()
                  .wrapping_add(x),
              );
            *{
              let checked_value_26 = predictor_get_mut(
                &mut *dst_2,
                match ::console_core_number_conversion::fitted::<_, u64>(pixel_off) {
                  ::core::result::Result::Ok(native_position) => native_position,
                },
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_26
            } = fill_val;
          }

          break 'over_y_2;
        }
      }
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_z3_8bpc_v4x_inner(
  _token: X64V4xToken,
  dst: &mut [u8],
  topleft: &[u8],
  prediction: DirectionalPrediction,
) -> Rav1dResult {
  let DirectionalPrediction {
    destination,
    edge_center: tl_off,
    angle,
  } = prediction;
  let PredictorOutput {
    base: dst_base,
    stride,
    size: Size { width, height },
  } = destination;
  let dst_2 = &mut *dst;
  let width_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(width) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  let height_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(height) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

  let is_sm = ::core::primitive::i32::wrapping_shr(angle, 9_u32) & 1 != 0;
  let enable_intra_edge_filter = ::core::primitive::i32::wrapping_shr(angle, 10_u32) != 0;
  let angle_2 = angle & 511;

  let checked_value_5 = predictor_get(&dav1d_dr_intra_derivative, {
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
        ::core::primitive::i32::wrapping_shr(i32::wrapping_sub(270, angle_2), 1_u32),
      );
    explicit_position
  })
  .ok_or(Rav1dError::EINVAL)?;
  let mut dy = <u64 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);

  let upsample_left = enable_intra_edge_filter
    && i32::wrapping_sub(angle_2, 180) < 40
    && i32::wrapping_add(width_i, height_i)
      <= i32::wrapping_shr(
        16,
        i32::cast_unsigned(<i32 as ::core::convert::From<::core::primitive::bool>>::from(is_sm)),
      );

  let mut left_out = [0u8; 64 + 64];
  let (left, left_off, max_base_y, base_inc);

  match ::core::primitive::bool::then_some(upsample_left, ()) {
    ::core::option::Option::Some(()) => {
      upsample_edge_8bpc(
        &mut left_out,
        i32::wrapping_add(width_i, height_i),
        PredictionEdge {
          pixels: topleft,
          origin: (Wrapping(tl_off) - Wrapping((Wrapping(width) + Wrapping(height)).0)).0,
          window: std::cmp::max(i32::wrapping_sub(width_i, height_i), 0)
            ..i32::wrapping_add(i32::wrapping_add(width_i, height_i), 1),
        },
      )?;
      left_off = {
        let ::core::result::Result::Ok(explicit_position) =
          ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
            i32::wrapping_sub(
              i32::wrapping_mul(2, i32::wrapping_add(width_i, height_i)),
              2,
            ),
          );
        explicit_position
      };
      max_base_y = left_off;
      dy = ::core::primitive::u64::wrapping_shl(dy, 1_u32);
      base_inc = 2u64;
      left = left_out.as_slice();
    }
    ::core::option::Option::None => {
      let Ok(filter_strength) =
        match ::core::primitive::bool::then_some(enable_intra_edge_filter, ()) {
          ::core::option::Option::Some(()) => get_filter_strength(EdgeFilterSelection {
            length: i32::wrapping_add(width_i, height_i),
            angle_delta: i32::wrapping_sub(angle_2, 180),
            smoothing: is_sm,
          }),
          ::core::option::Option::None => Ok(0),
        };

      match ::core::primitive::bool::then_some(filter_strength != 0, ()) {
        ::core::option::Option::Some(()) => {
          filter_edge_8bpc(
            &mut left_out,
            PredictionEdge {
              pixels: topleft,
              origin: (Wrapping(tl_off) - Wrapping((Wrapping(width) + Wrapping(height)).0)).0,
              window: std::cmp::max(i32::wrapping_sub(width_i, height_i), 0)
                ..i32::wrapping_add(i32::wrapping_add(width_i, height_i), 1),
            },
            EdgeFilter {
              count: i32::wrapping_add(width_i, height_i),
              filtered: 0..i32::wrapping_add(width_i, height_i),
              strength: filter_strength,
            },
          )?;
          left_off = {
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i32::wrapping_sub(i32::wrapping_add(width_i, height_i), 1),
              );
            explicit_position
          };
          max_base_y = left_off;
          left = left_out.as_slice();
        }
        ::core::option::Option::None => {
          left = topleft;
          left_off = (Wrapping(tl_off) - Wrapping(1)).0;
          max_base_y = (Wrapping((Wrapping(height) + Wrapping(std::cmp::min(width, height))).0)
            - Wrapping(1))
          .0;
        }
      }

      base_inc = 1;
    }
  };

  let left_f = &*left;

  let last = max_base_y.min(127);
  let mut lbuf = [0u8; 128];

  for k in 0..=last {
    let checked_value =
      predictor_get(&*left_f, (Wrapping(left_off) - Wrapping(k)).0).ok_or(Rav1dError::EINVAL)?;
    *{
      let checked_value_25 = predictor_get_mut(&mut lbuf, k).ok_or(Rav1dError::EINVAL)?;
      checked_value_25
    } = *checked_value;
  }

  let checked_value_26 = predictor_get(&lbuf, last).ok_or(Rav1dError::EINVAL)?;
  let fill_val = *checked_value_26;

  for b in lbuf.iter_mut().skip({
    let Ok(start) = index(last.wrapping_add(1));
    start
  }) {
    *b = fill_val;
  }

  let checked_value_27 = predictor_get(&lbuf, 0..64).ok_or(Rav1dError::EINVAL)?;
  let edge_lo = loadu_512!(checked_value_27, [u8; 64]);
  let checked_value_28 = predictor_get(&lbuf, 64..128).ok_or(Rav1dError::EINVAL)?;
  let edge_hi = loadu_512!(checked_value_28, [u8; 64]);

  let max_idx8 = _mm512_set1_epi8(
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(last) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    },
  );
  let rounding = _mm512_set1_epi16(32);
  let lane_off: [u8; 64] = core::array::from_fn(|i| {
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(i) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    }
  });
  let lane_off_v = loadu_512!(&lane_off, [u8; 64]);
  let one8 = _mm512_set1_epi8(1);

  match ::core::primitive::bool::then_some(base_inc == 1, ()) {
    ::core::option::Option::Some(()) => {
      for x in 0..width {
        let ::core::result::Result::Ok(ypos) = ::console_core_number_conversion::fitted::<_, u64>(
          (Wrapping(dy) * Wrapping((Wrapping(x) + Wrapping(1)).0)).0,
        );
        let frac = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
          (match ::console_core_number_conversion::fitted::<_, u64>(ypos) {
            ::core::result::Result::Ok(native_position) => native_position,
          }) & 0x3e,
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
        let inv_frac = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
          ::core::primitive::i16::wrapping_sub(64, frac),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
        let frac_vec = _mm512_set1_epi16(frac);
        let inv_frac_vec = _mm512_set1_epi16(inv_frac);
        let base0 = (match ::console_core_number_conversion::fitted::<_, u64>(ypos) {
          ::core::result::Result::Ok(native_position) => native_position,
        })
        .wrapping_shr(6_u32);
        let base0_v = _mm512_set1_epi8(
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(
            base0.min(127),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          },
        );

        let mut y = 0u64;

        for _ in 0..height.saturating_sub(y).div_ceil(32) {
          let ybase = _mm512_set1_epi8(
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(
              y.min(127),
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            },
          );
          let idx0 = _mm512_adds_epu8(_mm512_adds_epu8(base0_v, ybase), lane_off_v);
          let idx0_2 = _mm512_min_epu8(idx0, max_idx8);
          let idx1 = _mm512_min_epu8(_mm512_adds_epu8(idx0_2, one8), max_idx8);

          let l0 = _mm512_permutex2var_epi8(edge_lo, idx0_2, edge_hi);
          let l1 = _mm512_permutex2var_epi8(edge_lo, idx1, edge_hi);

          let l0_lo = _mm512_cvtepu8_epi16(_mm512_castsi512_si256(l0));
          let l1_lo = _mm512_cvtepu8_epi16(_mm512_castsi512_si256(l1));
          let p0 = _mm512_mullo_epi16(l0_lo, inv_frac_vec);
          let p1 = _mm512_mullo_epi16(l1_lo, frac_vec);
          let sblend = _mm512_add_epi16(_mm512_add_epi16(p0, p1), rounding);
          let r = _mm512_srai_epi16::<6>(sblend);
          let out32 = _mm512_cvtusepi16_epi8(r);

          let n = (Wrapping(height) - Wrapping(y)).0.min(32);
          let mut tmp = [0u8; 32];
          storeu_256!(&mut tmp, [u8; 32], out32);

          for k in 0..n {
            let ::core::result::Result::Ok(off) =
              ::console_core_number_conversion::fitted::<_, u64>(
                (Wrapping(
                  (Wrapping(dst_base.cast_signed())
                    + Wrapping(
                      (Wrapping(::core::primitive::u64::cast_signed(
                        (Wrapping(y) + Wrapping(k)).0,
                      )) * Wrapping(stride))
                      .0,
                    ))
                  .0
                  .cast_unsigned(),
                ) + Wrapping(x))
                .0,
              );
            let checked_value_29 = predictor_get(&tmp, k).ok_or(Rav1dError::EINVAL)?;
            *{
              let checked_value_30 = predictor_get_mut(
                &mut *dst_2,
                match ::console_core_number_conversion::fitted::<_, u64>(off) {
                  ::core::result::Result::Ok(native_position) => native_position,
                },
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_30
            } = *checked_value_29;
          }

          let ::core::result::Result::Ok(()) = advance_predictor_position(32, &mut y);
        }
      }
    }
    ::core::option::Option::None => {
      for x in 0..width {
        let ::core::result::Result::Ok(ypos) = ::console_core_number_conversion::fitted::<_, u64>(
          (Wrapping(dy) * Wrapping((Wrapping(x) + Wrapping(1)).0)).0,
        );
        let frac = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
          (match ::console_core_number_conversion::fitted::<_, u64>(ypos) {
            ::core::result::Result::Ok(native_position) => native_position,
          }) & 0x3e,
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
        let inv_frac = ::core::primitive::i32::wrapping_sub(64, frac);

        'over_y_2: for y in 0..height_i {
          let ::core::result::Result::Ok(base) = ::console_core_number_conversion::fitted::<_, u64>(
            (match ::console_core_number_conversion::fitted::<_, u64>(ypos) {
              ::core::result::Result::Ok(native_position) => native_position,
            })
            .wrapping_shr(6_u32)
            .wrapping_add(
              (::core::num::Wrapping(base_inc)
                * ::core::num::Wrapping({
                  let ::core::result::Result::Ok(explicit_position) =
                    ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(y);
                  explicit_position
                }))
              .0,
            ),
          );

          match ::core::primitive::bool::then_some(
            (match ::console_core_number_conversion::fitted::<_, u64>(base) {
              ::core::result::Result::Ok(native_position) => native_position,
            }) < max_base_y,
            (),
          ) {
            ::core::option::Option::Some(()) => {
              let checked_value_2 = predictor_get(
                &*left_f,
                (Wrapping(left_off)
                  - Wrapping(
                    match ::console_core_number_conversion::fitted::<_, u64>(base) {
                      ::core::result::Result::Ok(native_position) => native_position,
                    },
                  ))
                .0,
              )
              .ok_or(Rav1dError::EINVAL)?;
              let l0 =
                <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_2);
              let checked_value_3 = predictor_get(
                &*left_f,
                (Wrapping(
                  (Wrapping(left_off)
                    - Wrapping(
                      match ::console_core_number_conversion::fitted::<_, u64>(base) {
                        ::core::result::Result::Ok(native_position) => native_position,
                      },
                    ))
                  .0,
                ) - Wrapping(1))
                .0,
              )
              .ok_or(Rav1dError::EINVAL)?;
              let l1 =
                <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3);
              let v = ::core::primitive::i32::wrapping_add(
                ::core::primitive::i32::wrapping_mul(l0, inv_frac),
                ::core::primitive::i32::wrapping_mul(l1, frac),
              );
              let ::core::result::Result::Ok(pixel_off) =
                ::console_core_number_conversion::fitted::<_, u64>(
                  dst_base
                    .cast_signed()
                    .wrapping_add(
                      ({
                        let ::core::result::Result::Ok(explicit_position) =
                          ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(y);
                        explicit_position
                      })
                      .wrapping_mul(stride),
                    )
                    .cast_unsigned()
                    .wrapping_add(x),
                );
              *{
                let checked_value_31 = predictor_get_mut(
                  &mut *dst_2,
                  match ::console_core_number_conversion::fitted::<_, u64>(pixel_off) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  },
                )
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_31
              } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
                ::core::primitive::i32::wrapping_shr(
                  ::core::primitive::i32::wrapping_add(v, 32),
                  6_u32,
                ),
              ) {
                ::core::result::Result::Ok(converted_position) => converted_position,
              };
            }
            ::core::option::Option::None => {
              let checked_value_4 =
                predictor_get(&*left_f, (Wrapping(left_off) - Wrapping(max_base_y)).0)
                  .ok_or(Rav1dError::EINVAL)?;
              let fv = *checked_value_4;

              for yy in y..height_i {
                let ::core::result::Result::Ok(pixel_off) =
                  ::console_core_number_conversion::fitted::<_, u64>(
                    dst_base
                      .cast_signed()
                      .wrapping_add(
                        ({
                          let ::core::result::Result::Ok(explicit_position) =
                            ::console_core_number_conversion::wrapped::<
                              ::core::primitive::i32,
                              i64,
                            >(yy);
                          explicit_position
                        })
                        .wrapping_mul(stride),
                      )
                      .cast_unsigned()
                      .wrapping_add(x),
                  );
                *{
                  let checked_value_32 = predictor_get_mut(
                    &mut *dst_2,
                    match ::console_core_number_conversion::fitted::<_, u64>(pixel_off) {
                      ::core::result::Result::Ok(native_position) => native_position,
                    },
                  )
                  .ok_or(Rav1dError::EINVAL)?;
                  checked_value_32
                } = fv;
              }

              break 'over_y_2;
            }
          }
        }
      }
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_128_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  IpredDc12816bpcInnerArguments {
    dst_base,
    stride,
    width,
    height,
  }: IpredDc12816bpcInnerArguments,
  bitdepth_max: i32,
) -> Rav1dResult {
  let mid_val = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
    ::core::primitive::i32::wrapping_div(i32::wrapping_add(bitdepth_max, 1), 2),
  ) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::WordSamples)?;

  for row in rows {
    let Ok(()) = fill_predictor_words(_token, row, mid_val);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDc12816bpcInnerArguments {
  dst_base: u64,
  stride: i64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_v_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredV16bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredV16bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;

  let top_off = (Wrapping(tl_off) + Wrapping(2)).0;

  for y in 0..height {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
      .0
      .cast_unsigned(),
    );
    let mut x = 0u64;

    for _ in 0..width.saturating_sub(x).wrapping_div(16) {
      let load_off = (Wrapping(top_off) + Wrapping((Wrapping(x) * Wrapping(2)).0)).0;
      let checked_value =
        predictor_get(&*topleft_2, load_off..(Wrapping(load_off) + Wrapping(32)).0)
          .ok_or(Rav1dError::EINVAL)?;
      let top_vals = loadu_256!(checked_value, [u8; 32]);
      let store_off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;
      let checked_value_2 = predictor_get_mut(
        &mut *dst_2,
        store_off..(Wrapping(store_off) + Wrapping(32)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      storeu_256!(checked_value_2, [u8; 32], top_vals);
      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
    }

    for _ in 0..width.saturating_sub(x).wrapping_div(8) {
      let load_off = (Wrapping(top_off) + Wrapping((Wrapping(x) * Wrapping(2)).0)).0;
      let checked_value_3 =
        predictor_get(&*topleft_2, load_off..(Wrapping(load_off) + Wrapping(16)).0)
          .ok_or(Rav1dError::EINVAL)?;
      let top_vals = loadu_128!(checked_value_3, [u8; 16]);
      let store_off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;
      let checked_value_4 = predictor_get_mut(
        &mut *dst_2,
        store_off..(Wrapping(store_off) + Wrapping(16)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      storeu_128!(checked_value_4, [u8; 16], top_vals);
      let ::core::result::Result::Ok(()) = advance_predictor_position(8, &mut x);
    }

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let load_off = (Wrapping(top_off) + Wrapping((Wrapping(x) * Wrapping(2)).0)).0;
      let store_off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;

      {
        let checked_copy_destination = predictor_get_mut(&mut *dst_2, store_off..)
          .and_then(<[_]>::first_chunk_mut::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        let checked_copy_source = predictor_get(&*topleft_2, load_off..)
          .and_then(<[_]>::first_chunk::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        *checked_copy_destination = *checked_copy_source;
      };

      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredV16bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_h_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredH16bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredH16bpcInnerArguments,
) -> Rav1dResult {
  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::WordSamples)?;
  let edge_length = height.checked_mul(2).ok_or(Rav1dError::ERANGE)?;
  let start = tl_off.checked_sub(edge_length).ok_or(Rav1dError::EINVAL)?;
  let edge = predictor_get(topleft, start..tl_off).ok_or(Rav1dError::EINVAL)?;
  let words = edge.as_chunks::<2>().0;

  for (row, &color) in rows.zip(words.iter().rev()) {
    let Ok(()) = fill_predictor_words(_token, row, u16::from_le_bytes(color));
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredH16bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_128_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  IpredDc12816bpcAvx512InnerArguments {
    dst_base,
    stride,
    width,
    height,
  }: IpredDc12816bpcAvx512InnerArguments,
  bitdepth_max: i32,
) -> Rav1dResult {
  let mid_val = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
    ::core::primitive::i32::wrapping_div(i32::wrapping_add(bitdepth_max, 1), 2),
  ) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::WordSamples)?;

  for row in rows {
    let Ok(()) = fill_predictor_words_wide(_token, row, mid_val);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDc12816bpcAvx512InnerArguments {
  dst_base: u64,
  stride: i64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_v_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredV16bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredV16bpcAvx512InnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let top_off = (Wrapping(tl_off) + Wrapping(2)).0;

  for y in 0..height {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
      .0
      .cast_unsigned(),
    );
    let mut x = 0u64;

    for _ in 0..width.saturating_sub(x).wrapping_div(32) {
      let load_off = (Wrapping(top_off) + Wrapping((Wrapping(x) * Wrapping(2)).0)).0;
      let checked_value =
        predictor_get(&*topleft_2, load_off..(Wrapping(load_off) + Wrapping(64)).0)
          .ok_or(Rav1dError::EINVAL)?;
      let top_vals = loadu_512!(checked_value, [u8; 64]);
      let store_off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;
      let checked_value_2 = predictor_get_mut(
        &mut *dst_2,
        store_off..(Wrapping(store_off) + Wrapping(64)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      storeu_512!(checked_value_2, [u8; 64], top_vals);
      let ::core::result::Result::Ok(()) = advance_predictor_position(32, &mut x);
    }

    for _ in 0..width.saturating_sub(x).wrapping_div(16) {
      let load_off = (Wrapping(top_off) + Wrapping((Wrapping(x) * Wrapping(2)).0)).0;
      let checked_value_3 =
        predictor_get(&*topleft_2, load_off..(Wrapping(load_off) + Wrapping(32)).0)
          .ok_or(Rav1dError::EINVAL)?;
      let top_vals = loadu_256!(checked_value_3, [u8; 32]);
      let store_off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;
      let checked_value_4 = predictor_get_mut(
        &mut *dst_2,
        store_off..(Wrapping(store_off) + Wrapping(32)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      storeu_256!(checked_value_4, [u8; 32], top_vals);
      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
    }

    for _ in 0..width.saturating_sub(x).wrapping_div(8) {
      let load_off = (Wrapping(top_off) + Wrapping((Wrapping(x) * Wrapping(2)).0)).0;
      let checked_value_5 =
        predictor_get(&*topleft_2, load_off..(Wrapping(load_off) + Wrapping(16)).0)
          .ok_or(Rav1dError::EINVAL)?;
      let top_vals = loadu_128!(checked_value_5, [u8; 16]);
      let store_off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;
      let checked_value_6 = predictor_get_mut(
        &mut *dst_2,
        store_off..(Wrapping(store_off) + Wrapping(16)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      storeu_128!(checked_value_6, [u8; 16], top_vals);
      let ::core::result::Result::Ok(()) = advance_predictor_position(8, &mut x);
    }

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let load_off = (Wrapping(top_off) + Wrapping((Wrapping(x) * Wrapping(2)).0)).0;
      let store_off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;

      {
        let checked_copy_destination = predictor_get_mut(&mut *dst_2, store_off..)
          .and_then(<[_]>::first_chunk_mut::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        let checked_copy_source = predictor_get(&*topleft_2, load_off..)
          .and_then(<[_]>::first_chunk::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        *checked_copy_destination = *checked_copy_source;
      };

      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredV16bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_h_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredH16bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredH16bpcAvx512InnerArguments,
) -> Rav1dResult {
  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::WordSamples)?;
  let edge_length = height.checked_mul(2).ok_or(Rav1dError::ERANGE)?;
  let start = tl_off.checked_sub(edge_length).ok_or(Rav1dError::EINVAL)?;
  let edge = predictor_get(topleft, start..tl_off).ok_or(Rav1dError::EINVAL)?;
  let words = edge.as_chunks::<2>().0;

  for (row, &color) in rows.zip(words.iter().rev()) {
    let Ok(()) = fill_predictor_words_wide(_token, row, u16::from_le_bytes(color));
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredH16bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredDc16bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredDc16bpcAvx512InnerArguments,
) -> Rav1dResult {
  let topleft_2 = &*topleft;
  let mut sum = 0u32;

  for i in 1..=width {
    let off = (Wrapping(tl_off) + Wrapping((Wrapping(i) * Wrapping(2)).0)).0;
    let bytes = predictor_get(topleft_2, off..off.wrapping_add(2)).ok_or(Rav1dError::EINVAL)?;
    let pair = bytes
      .try_into()
      .map_err(|_wrong_length| Rav1dError::EINVAL)?;
    sum = sum.wrapping_add(u32::from(u16::from_le_bytes(pair)));
  }

  for i in 1..=height {
    let off = (Wrapping(tl_off) - Wrapping((Wrapping(i) * Wrapping(2)).0)).0;
    let bytes = predictor_get(topleft_2, off..off.wrapping_add(2)).ok_or(Rav1dError::EINVAL)?;
    let pair = bytes
      .try_into()
      .map_err(|_wrong_length| Rav1dError::EINVAL)?;
    sum = sum.wrapping_add(u32::from(u16::from_le_bytes(pair)));
  }

  let count = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u32>(
    (Wrapping(width) + Wrapping(height)).0,
  ) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  let average = rounded_dc(sum, u64::from(count))?;
  let Ok(avg) = console_core_number_conversion::wrapped::<_, u16>(average);

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::WordSamples)?;

  for row in rows {
    let Ok(()) = fill_predictor_words_wide(_token, row, avg);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDc16bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_top_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredDcTop16bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredDcTop16bpcAvx512InnerArguments,
) -> Rav1dResult {
  let topleft_2 = &*topleft;
  let mut sum = 0u32;

  for i in 1..=width {
    let off = (Wrapping(tl_off) + Wrapping((Wrapping(i) * Wrapping(2)).0)).0;

    match (
      <u32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
        let array_window = (*{
          let checked_value_25 = predictor_get(&*topleft_2, off..(Wrapping(off) + Wrapping(2)).0)
            .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      })),
      &mut sum,
    ) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  let average = rounded_dc(sum, width)?;
  let Ok(avg) = console_core_number_conversion::wrapped::<_, u16>(average);

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::WordSamples)?;

  for row in rows {
    let Ok(()) = fill_predictor_words_wide(_token, row, avg);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDcTop16bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_left_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredDcLeft16bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredDcLeft16bpcAvx512InnerArguments,
) -> Rav1dResult {
  let topleft_2 = &*topleft;
  let mut sum = 0u32;

  for i in 1..=height {
    let off = (Wrapping(tl_off) - Wrapping((Wrapping(i) * Wrapping(2)).0)).0;

    match (
      <u32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
        let array_window = (*{
          let checked_value_25 = predictor_get(&*topleft_2, off..(Wrapping(off) + Wrapping(2)).0)
            .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      })),
      &mut sum,
    ) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  let average = rounded_dc(sum, height)?;
  let Ok(avg) = console_core_number_conversion::wrapped::<_, u16>(average);

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::WordSamples)?;

  for row in rows {
    let Ok(()) = fill_predictor_words_wide(_token, row, avg);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDcLeft16bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredDc16bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredDc16bpcInnerArguments,
) -> Rav1dResult {
  let topleft_2 = &*topleft;

  let mut sum = 0u32;

  for i in 1..=width {
    let off = (Wrapping(tl_off) + Wrapping((Wrapping(i) * Wrapping(2)).0)).0;
    let bytes = predictor_get(topleft_2, off..off.wrapping_add(2)).ok_or(Rav1dError::EINVAL)?;
    let pair = bytes
      .try_into()
      .map_err(|_wrong_length| Rav1dError::EINVAL)?;
    sum = sum.wrapping_add(u32::from(u16::from_le_bytes(pair)));
  }

  for i in 1..=height {
    let off = (Wrapping(tl_off) - Wrapping((Wrapping(i) * Wrapping(2)).0)).0;
    let bytes = predictor_get(topleft_2, off..off.wrapping_add(2)).ok_or(Rav1dError::EINVAL)?;
    let pair = bytes
      .try_into()
      .map_err(|_wrong_length| Rav1dError::EINVAL)?;
    sum = sum.wrapping_add(u32::from(u16::from_le_bytes(pair)));
  }

  let count = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u32>(
    (Wrapping(width) + Wrapping(height)).0,
  ) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  let average = rounded_dc(sum, u64::from(count))?;
  let Ok(avg) = console_core_number_conversion::wrapped::<_, u16>(average);

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::WordSamples)?;

  for row in rows {
    let Ok(()) = fill_predictor_words(_token, row, avg);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDc16bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_top_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredDcTop16bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredDcTop16bpcInnerArguments,
) -> Rav1dResult {
  let topleft_2 = &*topleft;

  let mut sum = 0u32;

  for i in 1..=width {
    let off = (Wrapping(tl_off) + Wrapping((Wrapping(i) * Wrapping(2)).0)).0;

    match (
      <u32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
        let array_window = (*{
          let checked_value_25 = predictor_get(&*topleft_2, off..(Wrapping(off) + Wrapping(2)).0)
            .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      })),
      &mut sum,
    ) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  let average = rounded_dc(sum, width)?;
  let Ok(avg) = console_core_number_conversion::wrapped::<_, u16>(average);

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::WordSamples)?;

  for row in rows {
    let Ok(()) = fill_predictor_words(_token, row, avg);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDcTop16bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_dc_left_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredDcLeft16bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredDcLeft16bpcInnerArguments,
) -> Rav1dResult {
  let topleft_2 = &*topleft;

  let mut sum = 0u32;

  for i in 1..=height {
    let off = (Wrapping(tl_off) - Wrapping((Wrapping(i) * Wrapping(2)).0)).0;

    match (
      <u32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
        let array_window = (*{
          let checked_value_25 = predictor_get(&*topleft_2, off..(Wrapping(off) + Wrapping(2)).0)
            .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      })),
      &mut sum,
    ) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  let average = rounded_dc(sum, height)?;
  let Ok(avg) = console_core_number_conversion::wrapped::<_, u16>(average);

  let dimensions = Size { width, height };
  let Ok(target) = PredictorOutput::new(dst_base, stride, dimensions);
  let rows = target.rows(dst, PredictorStorage::WordSamples)?;

  for row in rows {
    let Ok(()) = fill_predictor_words(_token, row, avg);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredDcLeft16bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_paeth_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredPaeth16bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredPaeth16bpcAvx512InnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let topleft_val =
    <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
      let array_window = (*{
        let checked_value_25 =
          predictor_get(&*topleft_2, tl_off..(Wrapping(tl_off) + Wrapping(2)).0)
            .ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      })
      .try_into()
      .map_err(|_| Rav1dError::EINVAL)?;
      array_window
    }));
  let topleft_vec = _mm512_set1_epi32(topleft_val);

  for y in 0..height {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
      .0
      .cast_unsigned(),
    );
    let left_byte_off =
      (Wrapping(tl_off) - Wrapping((Wrapping((Wrapping(y) + Wrapping(1)).0) * Wrapping(2)).0)).0;
    let left_val =
      <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
        let array_window = (*{
          let checked_value_26 = predictor_get(
            &*topleft_2,
            left_byte_off..(Wrapping(left_byte_off) + Wrapping(2)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_26
        })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      }));
    let left_vec = _mm512_set1_epi32(left_val);

    let mut x = 0;

    for _ in 0..width.saturating_sub(x).wrapping_div(16) {
      let top_byte_off = (Wrapping(tl_off)
        + Wrapping((Wrapping((Wrapping(x) + Wrapping(1)).0) * Wrapping(2)).0))
      .0;
      let checked_value = predictor_get(
        &*topleft_2,
        top_byte_off..(Wrapping(top_byte_off) + Wrapping(32)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let top_u16 = loadu_256!(checked_value, [u8; 32]);
      let top = _mm512_cvtepu16_epi32(top_u16);

      let base = _mm512_sub_epi32(_mm512_add_epi32(left_vec, top), topleft_vec);
      let ldiff = _mm512_abs_epi32(_mm512_sub_epi32(left_vec, base));
      let tdiff = _mm512_abs_epi32(_mm512_sub_epi32(top, base));
      let tldiff = _mm512_abs_epi32(_mm512_sub_epi32(topleft_vec, base));

      let ld_le_td = !_mm512_cmpgt_epi32_mask(ldiff, tdiff);
      let ld_le_tld = !_mm512_cmpgt_epi32_mask(ldiff, tldiff);
      let td_le_tld = !_mm512_cmpgt_epi32_mask(tdiff, tldiff);

      let use_left = ld_le_td & ld_le_tld;
      let use_top = !use_left & td_le_tld;

      let result = _mm512_mask_blend_epi32(
        use_left,
        _mm512_mask_blend_epi32(use_top, topleft_vec, top),
        left_vec,
      );

      let clamped = _mm512_max_epi32(result, _mm512_setzero_si512());
      let result_u16: __m256i = _mm512_cvtusepi32_epi16(clamped);
      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;
      let checked_value_2 = predictor_get_mut(&mut *dst_2, off..(Wrapping(off) + Wrapping(32)).0)
        .ok_or(Rav1dError::EINVAL)?;
      storeu_256!(checked_value_2, [u8; 32], result_u16);

      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
    }

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let top_byte_off = (Wrapping(tl_off)
        + Wrapping((Wrapping((Wrapping(x) + Wrapping(1)).0) * Wrapping(2)).0))
      .0;
      let top_val =
        <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
          let array_window = (*{
            let checked_value_27 = predictor_get(
              &*topleft_2,
              top_byte_off..(Wrapping(top_byte_off) + Wrapping(2)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_27
          })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        }));
      let base = i32::wrapping_sub(
        ::core::primitive::i32::wrapping_add(left_val, top_val),
        topleft_val,
      );
      let l_diff = i32::wrapping_sub(left_val, base).abs();
      let t_diff = i32::wrapping_sub(top_val, base).abs();
      let tl_diff = i32::wrapping_sub(topleft_val, base).abs();
      let pred =
        match ::core::primitive::bool::then_some(l_diff <= t_diff && l_diff <= tl_diff, ()) {
          ::core::option::Option::Some(()) => left_val,
          ::core::option::Option::None => {
            match ::core::primitive::bool::then_some(t_diff <= tl_diff, ()) {
              ::core::option::Option::Some(()) => top_val,
              ::core::option::Option::None => topleft_val,
            }
          }
        };
      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;

      {
        let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
          .and_then(<[_]>::first_chunk_mut::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        *explicit_byte_copy_destination =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(pred) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          }
          .to_le_bytes();
      };

      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredPaeth16bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_smooth_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredSmooth16bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredSmooth16bpcAvx512InnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let weights_hor = predictor_get(
    &(*{
      let checked_value_55 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        width..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_55
    }),
    ..width,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let weights_ver = predictor_get(
    &(*{
      let checked_value_56 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        height..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_56
    }),
    ..height,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let right_off = (Wrapping(tl_off) + Wrapping((Wrapping(width) * Wrapping(2)).0)).0;
  let right_val =
    <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
      let array_window = (*{
        let checked_value_25 = predictor_get(
          &*topleft_2,
          right_off..(Wrapping(right_off) + Wrapping(2)).0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      })
      .try_into()
      .map_err(|_| Rav1dError::EINVAL)?;
      array_window
    }));
  let bottom_off = (Wrapping(tl_off) - Wrapping((Wrapping(height) * Wrapping(2)).0)).0;
  let bottom_val =
    <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
      let array_window = (*{
        let checked_value_26 = predictor_get(
          &*topleft_2,
          bottom_off..(Wrapping(bottom_off) + Wrapping(2)).0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_26
      })
      .try_into()
      .map_err(|_| Rav1dError::EINVAL)?;
      array_window
    }));
  let right_vec = _mm512_set1_epi32(right_val);
  let bottom_vec = _mm512_set1_epi32(bottom_val);
  let rounding = _mm512_set1_epi32(256);
  let c256 = _mm512_set1_epi32(256);
  let zero_512 = _mm512_setzero_si512();

  for y in 0..height {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
      .0
      .cast_unsigned(),
    );
    let left_byte_off =
      (Wrapping(tl_off) - Wrapping((Wrapping((Wrapping(y) + Wrapping(1)).0) * Wrapping(2)).0)).0;
    let left_val =
      <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
        let array_window = (*{
          let checked_value_27 = predictor_get(
            &*topleft_2,
            left_byte_off..(Wrapping(left_byte_off) + Wrapping(2)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_27
        })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      }));
    let left_vec = _mm512_set1_epi32(left_val);
    let checked_value_4 = predictor_get(&*weights_ver, y).ok_or(Rav1dError::EINVAL)?;
    let w_v = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4);
    let w_v_vec = _mm512_set1_epi32(w_v);
    let w_v_inv = _mm512_sub_epi32(c256, w_v_vec);

    let mut x = 0;

    for _ in 0..width.saturating_sub(x).wrapping_div(16) {
      let top_byte_off = (Wrapping(tl_off)
        + Wrapping((Wrapping((Wrapping(x) + Wrapping(1)).0) * Wrapping(2)).0))
      .0;
      let checked_value = predictor_get(
        &*topleft_2,
        top_byte_off..(Wrapping(top_byte_off) + Wrapping(32)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let top_u16 = loadu_256!(checked_value, [u8; 32]);
      let top = _mm512_cvtepu16_epi32(top_u16);

      let checked_value_2 = predictor_get(&*weights_hor, x..(Wrapping(x) + Wrapping(16)).0)
        .ok_or(Rav1dError::EINVAL)?;
      let wh_bytes = loadu_128!(checked_value_2, [u8; 16]);
      let w_h = _mm512_cvtepu8_epi32(wh_bytes);
      let w_h_inv = _mm512_sub_epi32(c256, w_h);

      let vert = _mm512_add_epi32(
        _mm512_mullo_epi32(w_v_vec, top),
        _mm512_mullo_epi32(w_v_inv, bottom_vec),
      );
      let hor = _mm512_add_epi32(
        _mm512_mullo_epi32(w_h, left_vec),
        _mm512_mullo_epi32(w_h_inv, right_vec),
      );

      let pred = _mm512_add_epi32(vert, hor);
      let result = _mm512_srai_epi32::<9>(_mm512_add_epi32(pred, rounding));

      let clamped = _mm512_max_epi32(result, zero_512);
      let result_u16: __m256i = _mm512_cvtusepi32_epi16(clamped);
      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;
      let checked_value_3 = predictor_get_mut(&mut *dst_2, off..(Wrapping(off) + Wrapping(32)).0)
        .ok_or(Rav1dError::EINVAL)?;
      storeu_256!(checked_value_3, [u8; 32], result_u16);

      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
    }

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let top_byte_off = (Wrapping(tl_off)
        + Wrapping((Wrapping((Wrapping(1) + Wrapping(x)).0) * Wrapping(2)).0))
      .0;
      let top_val =
        <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
          let array_window = (*{
            let checked_value_28 = predictor_get(
              &*topleft_2,
              top_byte_off..(Wrapping(top_byte_off) + Wrapping(2)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_28
          })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        }));
      let checked_value_5 = predictor_get(&*weights_hor, x).ok_or(Rav1dError::EINVAL)?;
      let w_h = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
      let pred = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_add(
          ::core::primitive::i32::wrapping_add(
            ::core::primitive::i32::wrapping_mul(w_v, top_val),
            ::core::primitive::i32::wrapping_mul(
              ::core::primitive::i32::wrapping_sub(256, w_v),
              bottom_val,
            ),
          ),
          ::core::primitive::i32::wrapping_mul(w_h, left_val),
        ),
        ::core::primitive::i32::wrapping_mul(
          ::core::primitive::i32::wrapping_sub(256, w_h),
          right_val,
        ),
      );
      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;

      {
        let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
          .and_then(<[_]>::first_chunk_mut::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        *explicit_byte_copy_destination = match ::console_core_number_conversion::wrapped::<
          _,
          ::core::primitive::u16,
        >(::core::primitive::i32::wrapping_shr(
          ::core::primitive::i32::wrapping_add(pred, 256),
          9_u32,
        )) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        }
        .to_le_bytes();
      };

      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredSmooth16bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_smooth_v_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredSmoothV16bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredSmoothV16bpcAvx512InnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let weights_ver = predictor_get(
    &(*{
      let checked_value_55 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        height..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_55
    }),
    ..height,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let bottom_off = (Wrapping(tl_off) - Wrapping((Wrapping(height) * Wrapping(2)).0)).0;
  let bottom_val =
    <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
      let array_window = (*{
        let checked_value_25 = predictor_get(
          &*topleft_2,
          bottom_off..(Wrapping(bottom_off) + Wrapping(2)).0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      })
      .try_into()
      .map_err(|_| Rav1dError::EINVAL)?;
      array_window
    }));
  let bottom_vec = _mm512_set1_epi32(bottom_val);
  let rounding = _mm512_set1_epi32(128);
  let c256 = _mm512_set1_epi32(256);
  let zero_512 = _mm512_setzero_si512();

  for y in 0..height {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
      .0
      .cast_unsigned(),
    );
    let checked_value_3 = predictor_get(&*weights_ver, y).ok_or(Rav1dError::EINVAL)?;
    let w_v = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3);
    let w_v_vec = _mm512_set1_epi32(w_v);
    let w_v_inv = _mm512_sub_epi32(c256, w_v_vec);

    let mut x = 0;

    for _ in 0..width.saturating_sub(x).wrapping_div(16) {
      let top_byte_off = (Wrapping(tl_off)
        + Wrapping((Wrapping((Wrapping(x) + Wrapping(1)).0) * Wrapping(2)).0))
      .0;
      let checked_value = predictor_get(
        &*topleft_2,
        top_byte_off..(Wrapping(top_byte_off) + Wrapping(32)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let top_u16 = loadu_256!(checked_value, [u8; 32]);
      let top = _mm512_cvtepu16_epi32(top_u16);

      let pred = _mm512_add_epi32(
        _mm512_mullo_epi32(w_v_vec, top),
        _mm512_mullo_epi32(w_v_inv, bottom_vec),
      );
      let result = _mm512_srai_epi32::<8>(_mm512_add_epi32(pred, rounding));

      let clamped = _mm512_max_epi32(result, zero_512);
      let result_u16: __m256i = _mm512_cvtusepi32_epi16(clamped);
      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;
      let checked_value_2 = predictor_get_mut(&mut *dst_2, off..(Wrapping(off) + Wrapping(32)).0)
        .ok_or(Rav1dError::EINVAL)?;
      storeu_256!(checked_value_2, [u8; 32], result_u16);

      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
    }

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let top_byte_off = (Wrapping(tl_off)
        + Wrapping((Wrapping((Wrapping(1) + Wrapping(x)).0) * Wrapping(2)).0))
      .0;
      let top_val =
        <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
          let array_window = (*{
            let checked_value_26 = predictor_get(
              &*topleft_2,
              top_byte_off..(Wrapping(top_byte_off) + Wrapping(2)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_26
          })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        }));
      let pred = ::core::primitive::i32::wrapping_shr(
        ::core::primitive::i32::wrapping_add(
          ::core::primitive::i32::wrapping_add(
            ::core::primitive::i32::wrapping_mul(w_v, top_val),
            ::core::primitive::i32::wrapping_mul(
              ::core::primitive::i32::wrapping_sub(256, w_v),
              bottom_val,
            ),
          ),
          128,
        ),
        8_u32,
      );
      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;

      {
        let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
          .and_then(<[_]>::first_chunk_mut::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        *explicit_byte_copy_destination =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(pred) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          }
          .to_le_bytes();
      };

      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredSmoothV16bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_smooth_h_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredSmoothH16bpcAvx512InnerArguments {
    tl_off,
    width,
    height,
  }: IpredSmoothH16bpcAvx512InnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let weights_hor = predictor_get(
    &(*{
      let checked_value_55 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        width..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_55
    }),
    ..width,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let right_off = (Wrapping(tl_off) + Wrapping((Wrapping(width) * Wrapping(2)).0)).0;
  let right_val =
    <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
      let array_window = (*{
        let checked_value_25 = predictor_get(
          &*topleft_2,
          right_off..(Wrapping(right_off) + Wrapping(2)).0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      })
      .try_into()
      .map_err(|_| Rav1dError::EINVAL)?;
      array_window
    }));
  let right_vec = _mm512_set1_epi32(right_val);
  let rounding = _mm512_set1_epi32(128);
  let c256 = _mm512_set1_epi32(256);
  let zero_512 = _mm512_setzero_si512();

  for y in 0..height {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
      .0
      .cast_unsigned(),
    );
    let left_byte_off =
      (Wrapping(tl_off) - Wrapping((Wrapping((Wrapping(y) + Wrapping(1)).0) * Wrapping(2)).0)).0;
    let left_val =
      <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
        let array_window = (*{
          let checked_value_26 = predictor_get(
            &*topleft_2,
            left_byte_off..(Wrapping(left_byte_off) + Wrapping(2)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_26
        })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      }));
    let left_vec = _mm512_set1_epi32(left_val);

    let mut x = 0;

    for _ in 0..width.saturating_sub(x).wrapping_div(16) {
      let checked_value = predictor_get(&*weights_hor, x..(Wrapping(x) + Wrapping(16)).0)
        .ok_or(Rav1dError::EINVAL)?;
      let wh_bytes = loadu_128!(checked_value, [u8; 16]);
      let w_h = _mm512_cvtepu8_epi32(wh_bytes);
      let w_h_inv = _mm512_sub_epi32(c256, w_h);

      let pred = _mm512_add_epi32(
        _mm512_mullo_epi32(w_h, left_vec),
        _mm512_mullo_epi32(w_h_inv, right_vec),
      );
      let result = _mm512_srai_epi32::<8>(_mm512_add_epi32(pred, rounding));

      let clamped = _mm512_max_epi32(result, zero_512);
      let result_u16: __m256i = _mm512_cvtusepi32_epi16(clamped);
      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;
      let checked_value_2 = predictor_get_mut(&mut *dst_2, off..(Wrapping(off) + Wrapping(32)).0)
        .ok_or(Rav1dError::EINVAL)?;
      storeu_256!(checked_value_2, [u8; 32], result_u16);

      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
    }

    for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let checked_value_3 = predictor_get(&*weights_hor, x).ok_or(Rav1dError::EINVAL)?;
      let w_h = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_3);
      let pred = ::core::primitive::i32::wrapping_shr(
        ::core::primitive::i32::wrapping_add(
          ::core::primitive::i32::wrapping_add(
            ::core::primitive::i32::wrapping_mul(w_h, left_val),
            ::core::primitive::i32::wrapping_mul(
              ::core::primitive::i32::wrapping_sub(256, w_h),
              right_val,
            ),
          ),
          128,
        ),
        8_u32,
      );
      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;

      {
        let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
          .and_then(<[_]>::first_chunk_mut::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        *explicit_byte_copy_destination =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(pred) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          }
          .to_le_bytes();
      };

      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredSmoothH16bpcAvx512InnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_paeth_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredPaeth16bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredPaeth16bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let topleft_val =
    <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
      let array_window = (*{
        let checked_value_25 =
          predictor_get(&*topleft_2, tl_off..(Wrapping(tl_off) + Wrapping(2)).0)
            .ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      })
      .try_into()
      .map_err(|_| Rav1dError::EINVAL)?;
      array_window
    }));

  for y in 0..height {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
      .0
      .cast_unsigned(),
    );
    let left_byte_off =
      (Wrapping(tl_off) - Wrapping((Wrapping((Wrapping(y) + Wrapping(1)).0) * Wrapping(2)).0)).0;
    let left_val =
      <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
        let array_window = (*{
          let checked_value_26 = predictor_get(
            &*topleft_2,
            left_byte_off..(Wrapping(left_byte_off) + Wrapping(2)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_26
        })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      }));

    for x in 0..width {
      let top_byte_off = (Wrapping(tl_off)
        + Wrapping((Wrapping((Wrapping(x) + Wrapping(1)).0) * Wrapping(2)).0))
      .0;
      let top_val =
        <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
          let array_window = (*{
            let checked_value_27 = predictor_get(
              &*topleft_2,
              top_byte_off..(Wrapping(top_byte_off) + Wrapping(2)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_27
          })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        }));

      let base = i32::wrapping_sub(
        ::core::primitive::i32::wrapping_add(left_val, top_val),
        topleft_val,
      );
      let l_diff = i32::wrapping_sub(left_val, base).abs();
      let t_diff = i32::wrapping_sub(top_val, base).abs();
      let tl_diff = i32::wrapping_sub(topleft_val, base).abs();

      let pred =
        match ::core::primitive::bool::then_some(l_diff <= t_diff && l_diff <= tl_diff, ()) {
          ::core::option::Option::Some(()) => left_val,
          ::core::option::Option::None => {
            match ::core::primitive::bool::then_some(t_diff <= tl_diff, ()) {
              ::core::option::Option::Some(()) => top_val,
              ::core::option::Option::None => topleft_val,
            }
          }
        };

      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;

      {
        let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
          .and_then(<[_]>::first_chunk_mut::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        *explicit_byte_copy_destination =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(pred) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          }
          .to_le_bytes();
      };
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredPaeth16bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_smooth_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredSmooth16bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredSmooth16bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let weights_hor = predictor_get(
    &(*{
      let checked_value_55 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        width..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_55
    }),
    ..width,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let weights_ver = predictor_get(
    &(*{
      let checked_value_56 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        height..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_56
    }),
    ..height,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let right_off = (Wrapping(tl_off) + Wrapping((Wrapping(width) * Wrapping(2)).0)).0;
  let right_val =
    <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
      let array_window = (*{
        let checked_value_25 = predictor_get(
          &*topleft_2,
          right_off..(Wrapping(right_off) + Wrapping(2)).0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      })
      .try_into()
      .map_err(|_| Rav1dError::EINVAL)?;
      array_window
    }));
  let bottom_off = (Wrapping(tl_off) - Wrapping((Wrapping(height) * Wrapping(2)).0)).0;
  let bottom_val =
    <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
      let array_window = (*{
        let checked_value_26 = predictor_get(
          &*topleft_2,
          bottom_off..(Wrapping(bottom_off) + Wrapping(2)).0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_26
      })
      .try_into()
      .map_err(|_| Rav1dError::EINVAL)?;
      array_window
    }));

  for y in 0..height {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
      .0
      .cast_unsigned(),
    );
    let left_byte_off =
      (Wrapping(tl_off) - Wrapping((Wrapping((Wrapping(y) + Wrapping(1)).0) * Wrapping(2)).0)).0;
    let left_val =
      <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
        let array_window = (*{
          let checked_value_27 = predictor_get(
            &*topleft_2,
            left_byte_off..(Wrapping(left_byte_off) + Wrapping(2)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_27
        })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      }));
    let checked_value = predictor_get(&*weights_ver, y).ok_or(Rav1dError::EINVAL)?;
    let w_v = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value);

    for x in 0..width {
      let top_byte_off = (Wrapping(tl_off)
        + Wrapping((Wrapping((Wrapping(1) + Wrapping(x)).0) * Wrapping(2)).0))
      .0;
      let top_val =
        <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
          let array_window = (*{
            let checked_value_28 = predictor_get(
              &*topleft_2,
              top_byte_off..(Wrapping(top_byte_off) + Wrapping(2)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_28
          })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        }));
      let checked_value_2 = predictor_get(&*weights_hor, x).ok_or(Rav1dError::EINVAL)?;
      let w_h = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_2);

      let vert = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_mul(w_v, top_val),
        ::core::primitive::i32::wrapping_mul(
          ::core::primitive::i32::wrapping_sub(256, w_v),
          bottom_val,
        ),
      );

      let horz = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_mul(w_h, left_val),
        ::core::primitive::i32::wrapping_mul(
          ::core::primitive::i32::wrapping_sub(256, w_h),
          right_val,
        ),
      );

      let pred = ::core::primitive::i32::wrapping_shr(
        ::core::primitive::i32::wrapping_add(
          ::core::primitive::i32::wrapping_add(vert, horz),
          256,
        ),
        9_u32,
      );
      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;

      {
        let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
          .and_then(<[_]>::first_chunk_mut::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        *explicit_byte_copy_destination =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(pred) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          }
          .to_le_bytes();
      };
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredSmooth16bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_smooth_v_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredSmoothV16bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredSmoothV16bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let weights_ver = predictor_get(
    &(*{
      let checked_value_55 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        height..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_55
    }),
    ..height,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let bottom_off = (Wrapping(tl_off) - Wrapping((Wrapping(height) * Wrapping(2)).0)).0;
  let bottom_val =
    <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
      let array_window = (*{
        let checked_value_25 = predictor_get(
          &*topleft_2,
          bottom_off..(Wrapping(bottom_off) + Wrapping(2)).0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      })
      .try_into()
      .map_err(|_| Rav1dError::EINVAL)?;
      array_window
    }));

  for y in 0..height {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
      .0
      .cast_unsigned(),
    );
    let checked_value = predictor_get(&*weights_ver, y).ok_or(Rav1dError::EINVAL)?;
    let w_v = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value);

    for x in 0..width {
      let top_byte_off = (Wrapping(tl_off)
        + Wrapping((Wrapping((Wrapping(1) + Wrapping(x)).0) * Wrapping(2)).0))
      .0;
      let top_val =
        <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
          let array_window = (*{
            let checked_value_26 = predictor_get(
              &*topleft_2,
              top_byte_off..(Wrapping(top_byte_off) + Wrapping(2)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_26
          })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        }));
      let pred = ::core::primitive::i32::wrapping_shr(
        ::core::primitive::i32::wrapping_add(
          ::core::primitive::i32::wrapping_add(
            ::core::primitive::i32::wrapping_mul(w_v, top_val),
            ::core::primitive::i32::wrapping_mul(
              ::core::primitive::i32::wrapping_sub(256, w_v),
              bottom_val,
            ),
          ),
          128,
        ),
        8_u32,
      );
      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;

      {
        let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
          .and_then(<[_]>::first_chunk_mut::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        *explicit_byte_copy_destination =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(pred) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          }
          .to_le_bytes();
      };
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredSmoothV16bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_smooth_h_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredSmoothH16bpcInnerArguments {
    tl_off,
    width,
    height,
  }: IpredSmoothH16bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let weights_hor = predictor_get(
    &(*{
      let checked_value_55 = predictor_get(
        &*crate::src::align::aligned_inner(&dav1d_sm_weights),
        width..,
      )
      .ok_or(Rav1dError::EINVAL)?;
      checked_value_55
    }),
    ..width,
  )
  .ok_or(Rav1dError::EINVAL)?;
  let right_off = (Wrapping(tl_off) + Wrapping((Wrapping(width) * Wrapping(2)).0)).0;
  let right_val =
    <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
      let array_window = (*{
        let checked_value_25 = predictor_get(
          &*topleft_2,
          right_off..(Wrapping(right_off) + Wrapping(2)).0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      })
      .try_into()
      .map_err(|_| Rav1dError::EINVAL)?;
      array_window
    }));

  for y in 0..height {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
      .0
      .cast_unsigned(),
    );
    let left_byte_off =
      (Wrapping(tl_off) - Wrapping((Wrapping((Wrapping(y) + Wrapping(1)).0) * Wrapping(2)).0)).0;
    let left_val =
      <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
        let array_window = (*{
          let checked_value_26 = predictor_get(
            &*topleft_2,
            left_byte_off..(Wrapping(left_byte_off) + Wrapping(2)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_26
        })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      }));

    for x in 0..width {
      let checked_value = predictor_get(&*weights_hor, x).ok_or(Rav1dError::EINVAL)?;
      let w_h = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value);
      let pred = ::core::primitive::i32::wrapping_shr(
        ::core::primitive::i32::wrapping_add(
          ::core::primitive::i32::wrapping_add(
            ::core::primitive::i32::wrapping_mul(w_h, left_val),
            ::core::primitive::i32::wrapping_mul(
              ::core::primitive::i32::wrapping_sub(256, w_h),
              right_val,
            ),
          ),
          128,
        ),
        8_u32,
      );
      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;

      {
        let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
          .and_then(<[_]>::first_chunk_mut::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        *explicit_byte_copy_destination =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(pred) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          }
          .to_le_bytes();
      };
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredSmoothH16bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_z1_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredZ116bpcInnerArguments {
    tl_off,
    width,
    height,
    angle,
    bitdepth_max,
  }: IpredZ116bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let width_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(width) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  let height_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(height) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

  let is_sm = ::core::primitive::i32::wrapping_shr(angle, 9_u32) & 1 != 0;
  let enable_intra_edge_filter = ::core::primitive::i32::wrapping_shr(angle, 10_u32) != 0;
  let angle_2 = angle & 511;

  let checked_value_9 = predictor_get(&dav1d_dr_intra_derivative, {
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
        ::core::primitive::i32::wrapping_shr(angle_2, 1_u32),
      );
    explicit_position
  })
  .ok_or(Rav1dError::EINVAL)?;
  let mut dx = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_9);

  let tl_pix = tl_off.wrapping_div(2);

  let rd = |off: u64| -> Rav1dResult<u16> {
    let byte_offset = off.checked_mul(2).ok_or(Rav1dError::EINVAL)?;
    let bytes = predictor_get(topleft, byte_offset..)
      .and_then(<[_]>::first_chunk::<2>)
      .ok_or(Rav1dError::EINVAL)?;
    Ok(u16::from_le_bytes(*bytes))
  };

  let upsample_above = enable_intra_edge_filter
    && i32::wrapping_sub(90, angle_2) < 40
    && i32::wrapping_add(width_i, height_i)
      <= i32::wrapping_shr(
        16,
        i32::cast_unsigned(<i32 as ::core::convert::From<::core::primitive::bool>>::from(is_sm)),
      );

  let mut top_px = [0u16; 64 + 64];
  let (max_base_x, base_inc);

  match ::core::primitive::bool::then_some(upsample_above, ()) {
    ::core::option::Option::Some(()) => {
      let kernel: [i8; 4] = [-1, 9, 9, -1];
      let hsz = i32::wrapping_add(width_i, height_i);
      let ::core::result::Result::Ok(in_off) =
        ::console_core_number_conversion::fitted::<_, u64>((Wrapping(tl_pix) + Wrapping(1)).0);
      let from = -1i32;
      let to = i32::wrapping_add(width_i, std::cmp::min(width_i, height_i));

      for i in 0..i32::wrapping_sub(hsz, 1) {
        let checked_value_4 = rd(
          (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          })
          .wrapping_add_signed({
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                i.clamp(from, i32::wrapping_sub(to, 1)),
              );
            explicit_position
          }),
        )?;
        *{
          let checked_value_25 = predictor_get_mut(&mut top_px, {
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i32::wrapping_mul(i, 2),
              );
            explicit_position
          })
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        } = checked_value_4;
        let mut s = 0i32;

        for j in 0..4i32 {
          let checked_value_26 = rd(
            (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            })
            .wrapping_add_signed({
              let ::core::result::Result::Ok(explicit_position) =
                ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                  i32::wrapping_sub(i32::wrapping_add(i, j), 1)
                    .clamp(from, i32::wrapping_sub(to, 1)),
                );
              explicit_position
            }),
          )?;

          match (
            ::core::primitive::i32::wrapping_mul(
              <i32 as ::core::convert::From<::core::primitive::u16>>::from(checked_value_26),
              <i32 as ::core::convert::From<::core::primitive::i8>>::from(*{
                let checked_value_27 = predictor_get(&kernel, {
                  let ::core::result::Result::Ok(explicit_position) =
                    ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(j);
                  explicit_position
                })
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_27
              }),
            ),
            &mut s,
          ) {
            (explicit_arithmetic_amount, explicit_arithmetic_place) => {
              *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
                *explicit_arithmetic_place,
                explicit_arithmetic_amount,
              )
            }
          };
        }

        *{
          let checked_value_28 = predictor_get_mut(&mut top_px, {
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i32::wrapping_add(i32::wrapping_mul(i, 2), 1),
              );
            explicit_position
          })
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_28
        } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
          ::core::primitive::i32::wrapping_shr(i32::wrapping_add(s, 8), 4_u32)
            .clamp(0, bitdepth_max),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
      }

      let i = i32::wrapping_sub(hsz, 1);
      let checked_value_5 = rd(
        (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        })
        .wrapping_add_signed({
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
              i.clamp(from, i32::wrapping_sub(to, 1)),
            );
          explicit_position
        }),
      )?;
      *{
        let checked_value_29 = predictor_get_mut(&mut top_px, {
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
              i32::wrapping_mul(i, 2),
            );
          explicit_position
        })
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_29
      } = checked_value_5;
      dx = ::core::primitive::i32::wrapping_shl(dx, 1_u32);
      max_base_x = {
        let ::core::result::Result::Ok(explicit_position) =
          ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
            i32::wrapping_sub(
              i32::wrapping_mul(2, i32::wrapping_add(width_i, height_i)),
              2,
            ),
          );
        explicit_position
      };
      base_inc = 2u64;
    }
    ::core::option::Option::None => {
      let Ok(filter_strength) =
        match ::core::primitive::bool::then_some(enable_intra_edge_filter, ()) {
          ::core::option::Option::Some(()) => get_filter_strength(EdgeFilterSelection {
            length: i32::wrapping_add(width_i, height_i),
            angle_delta: i32::wrapping_sub(90, angle_2),
            smoothing: is_sm,
          }),
          ::core::option::Option::None => Ok(0),
        };

      match ::core::primitive::bool::then_some(filter_strength != 0, ()) {
        ::core::option::Option::Some(()) => {
          static KERNEL: [[u8; 5]; 3] = [[0, 4, 8, 4, 0], [0, 5, 6, 5, 0], [2, 4, 4, 4, 2]];
          let ::core::result::Result::Ok(in_off) = ::console_core_number_conversion::fitted::<
            _,
            u64,
          >((Wrapping(tl_pix) + Wrapping(1)).0);
          let from = -1i32;
          let to = i32::wrapping_add(width_i, std::cmp::min(width_i, height_i));
          let lim_from = 0i32;
          let lim_to = i32::wrapping_add(width_i, height_i);
          let mut i = 0i32;

          for _ in (::core::ops::Range {
            start: i,
            end: std::cmp::min(i32::wrapping_add(width_i, height_i), lim_from),
          }) {
            let checked_value_6 = rd(
              (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              })
              .wrapping_add_signed({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                    i.clamp(from, i32::wrapping_sub(to, 1)),
                  );
                explicit_position
              }),
            )?;
            *{
              let checked_value_30 = predictor_get_mut(&mut top_px, {
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(i);
                explicit_position
              })
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_30
            } = checked_value_6;
            i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
          }

          for _ in (::core::ops::Range {
            start: i,
            end: std::cmp::min(lim_to, i32::wrapping_add(width_i, height_i)),
          }) {
            let mut s = 0i32;

            for j in 0..5i32 {
              let checked_value_31 = rd(
                (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
                  ::core::result::Result::Ok(native_position) => native_position,
                })
                .wrapping_add_signed({
                  let ::core::result::Result::Ok(explicit_position) =
                    ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                      i32::wrapping_add(i32::wrapping_sub(i, 2), j)
                        .clamp(from, i32::wrapping_sub(to, 1)),
                    );
                  explicit_position
                }),
              )?;

              match (
                ::core::primitive::i32::wrapping_mul(
                  <i32 as ::core::convert::From<::core::primitive::u16>>::from(checked_value_31),
                  <i32 as ::core::convert::From<::core::primitive::u8>>::from(*{
                    let checked_value_52 =
                      predictor_get(
                        &(*{
                          let checked_value_51 = predictor_get(&KERNEL, {
                            let ::core::result::Result::Ok(explicit_position) =
                              ::console_core_number_conversion::wrapped::<
                                ::core::primitive::i32,
                                u64,
                              >(i32::wrapping_sub(
                                filter_strength,
                                1,
                              ));
                            explicit_position
                          })
                          .ok_or(Rav1dError::EINVAL)?;
                          checked_value_51
                        }),
                        {
                          let ::core::result::Result::Ok(explicit_position) =
                            ::console_core_number_conversion::wrapped::<
                              ::core::primitive::i32,
                              u64,
                            >(j);
                          explicit_position
                        },
                      )
                      .ok_or(Rav1dError::EINVAL)?;
                    checked_value_52
                  }),
                ),
                &mut s,
              ) {
                (explicit_arithmetic_amount, explicit_arithmetic_place) => {
                  *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
                    *explicit_arithmetic_place,
                    explicit_arithmetic_amount,
                  )
                }
              };
            }

            *{
              let checked_value_32 = predictor_get_mut(&mut top_px, {
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(i);
                explicit_position
              })
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_32
            } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
              ::core::primitive::i32::wrapping_shr(i32::wrapping_add(s, 8), 4_u32),
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            };
            i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
          }

          for _ in (::core::ops::Range {
            start: i,
            end: i32::wrapping_add(width_i, height_i),
          }) {
            let checked_value_7 = rd(
              (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              })
              .wrapping_add_signed({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                    i.clamp(from, i32::wrapping_sub(to, 1)),
                  );
                explicit_position
              }),
            )?;
            *{
              let checked_value_33 = predictor_get_mut(&mut top_px, {
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(i);
                explicit_position
              })
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_33
            } = checked_value_7;
            i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
          }

          max_base_x = {
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i32::wrapping_sub(i32::wrapping_add(width_i, height_i), 1),
              );
            explicit_position
          };
        }
        ::core::option::Option::None => {
          for i in 0..(Wrapping(width) + Wrapping(std::cmp::min(width, height))).0 {
            let checked_value_8 =
              rd((Wrapping((Wrapping(tl_pix) + Wrapping(1)).0) + Wrapping(i)).0)?;
            *{
              let checked_value_34 =
                predictor_get_mut(&mut top_px, i).ok_or(Rav1dError::EINVAL)?;
              checked_value_34
            } = checked_value_8;
          }

          max_base_x = (Wrapping((Wrapping(width) + Wrapping(std::cmp::min(width, height))).0)
            - Wrapping(1))
          .0;
        }
      }

      base_inc = 1;
    }
  };

  let top_bytes: &[u8] = zerocopy::IntoBytes::as_bytes({
    let [_explicit_part @ ..] = &top_px;
    let _explicit_slice: &[_] = _explicit_part;
    _explicit_slice
  });
  let top_bytes_2 = &*top_bytes;

  let rounding = _mm256_set1_epi32(32);

  for y in 0..height_i {
    let xpos = i32::wrapping_mul(::core::primitive::i32::wrapping_add(y, 1), dx);
    let frac =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(xpos & 0x3e) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    let inv_frac = ::core::primitive::i32::wrapping_sub(64, frac);

    let frac_vec = _mm256_set1_epi32(frac);
    let inv_frac_vec = _mm256_set1_epi32(inv_frac);

    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      dst_base
        .cast_signed()
        .wrapping_add(
          ({
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(y);
            explicit_position
          })
          .wrapping_mul(stride),
        )
        .cast_unsigned(),
    );
    let ::core::result::Result::Ok(base0) = ::console_core_number_conversion::fitted::<_, u64>({
      let ::core::result::Result::Ok(explicit_position) =
        ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
          ::core::primitive::i32::wrapping_shr(xpos, 6_u32),
        );
      explicit_position
    });

    let mut x = 0u64;

    match ::core::primitive::bool::then_some(base_inc == 1, ()) {
      ::core::option::Option::Some(()) => {
        let source_remaining = max_base_x
          .saturating_sub(base0.wrapping_add(x))
          .saturating_sub(1);
        let group_count = width
          .saturating_sub(x)
          .min(source_remaining)
          .wrapping_div(8);

        for _ in 0..group_count {
          let ::core::result::Result::Ok(base) = ::console_core_number_conversion::fitted::<_, u64>(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(base0) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(x))
            .0,
          );

          let load0 = (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(base) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) * Wrapping(2))
          .0;
          let load1 = (Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(base) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(1))
            .0,
          ) * Wrapping(2))
          .0;
          let checked_value =
            predictor_get(&*top_bytes_2, load0..(Wrapping(load0) + Wrapping(16)).0)
              .ok_or(Rav1dError::EINVAL)?;
          let t0 = loadu_128!(checked_value, [u8; 16]);
          let checked_value_2 =
            predictor_get(&*top_bytes_2, load1..(Wrapping(load1) + Wrapping(16)).0)
              .ok_or(Rav1dError::EINVAL)?;
          let t1 = loadu_128!(checked_value_2, [u8; 16]);

          let t0_w = _mm256_cvtepu16_epi32(t0);
          let t1_w = _mm256_cvtepu16_epi32(t1);

          let prod0 = _mm256_mullo_epi32(t0_w, inv_frac_vec);
          let prod1 = _mm256_mullo_epi32(t1_w, frac_vec);
          let sum = _mm256_add_epi32(_mm256_add_epi32(prod0, prod1), rounding);
          let result = _mm256_srai_epi32::<6>(sum);

          let packed = _mm256_packus_epi32(result, result);
          let lo = _mm256_castsi256_si128(packed);
          let hi = _mm256_extracti128_si256::<1>(packed);
          let combined = _mm_unpacklo_epi64(lo, hi);
          let store_off = (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
          .0;
          let checked_value_3 = predictor_get_mut(
            &mut *dst_2,
            store_off..(Wrapping(store_off) + Wrapping(16)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          storeu_128!(checked_value_3, [u8; 16], combined);

          let ::core::result::Result::Ok(()) = advance_predictor_position(8, &mut x);
        }
      }
      ::core::option::Option::None => {}
    }

    'inner_for_2: for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let ::core::result::Result::Ok(base) = ::console_core_number_conversion::fitted::<_, u64>(
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(base0) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping((Wrapping(base_inc) * Wrapping(x)).0))
        .0,
      );

      match ::core::primitive::bool::then_some(
        (match ::console_core_number_conversion::fitted::<_, u64>(base) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) < max_base_x,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let checked_value_35 = predictor_get(
            &top_px,
            match ::console_core_number_conversion::fitted::<_, u64>(base) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          )
          .ok_or(Rav1dError::EINVAL)?;
          let t0 =
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_35);
          let checked_value_36 = predictor_get(
            &top_px,
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(base) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(1))
            .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          let t1 =
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_36);
          let v = ::core::primitive::i32::wrapping_add(
            ::core::primitive::i32::wrapping_mul(t0, inv_frac),
            ::core::primitive::i32::wrapping_mul(t1, frac),
          );
          let off = (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
          .0;

          {
            let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
              .and_then(<[_]>::first_chunk_mut::<2>)
              .ok_or(Rav1dError::EINVAL)?;
            *explicit_byte_copy_destination =
              match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
                ::core::primitive::i32::wrapping_shr(
                  ::core::primitive::i32::wrapping_add(v, 32),
                  6_u32,
                ),
              ) {
                ::core::result::Result::Ok(converted_position) => converted_position,
              }
              .to_le_bytes();
          };
        }
        ::core::option::Option::None => {
          let checked_value_38 = predictor_get(&top_px, max_base_x).ok_or(Rav1dError::EINVAL)?;
          let fill_val = *checked_value_38;

          for xx in x..width {
            let off = (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping((Wrapping(xx) * Wrapping(2)).0))
            .0;

            {
              let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
                .and_then(<[_]>::first_chunk_mut::<2>)
                .ok_or(Rav1dError::EINVAL)?;
              *explicit_byte_copy_destination = fill_val.to_le_bytes();
            };
          }

          break 'inner_for_2;
        }
      }

      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredZ116bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
  angle: i32,
  bitdepth_max: i32,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_z2_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredZ216bpcInnerArguments {
    tl_off,
    width,
    height,
    angle,
    max_width,
    max_height,
    bitdepth_max,
  }: IpredZ216bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let width_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(width) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  let height_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(height) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

  let is_sm = ::core::primitive::i32::wrapping_shr(angle, 9_u32) & 1 != 0;
  let enable_intra_edge_filter = ::core::primitive::i32::wrapping_shr(angle, 10_u32) != 0;
  let angle_2 = angle & 511;

  let checked_value_15 = predictor_get(&dav1d_dr_intra_derivative, {
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
        ::core::primitive::i32::wrapping_shr(i32::wrapping_sub(angle_2, 90), 1_u32),
      );
    explicit_position
  })
  .ok_or(Rav1dError::EINVAL)?;
  let mut dy = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_15);
  let checked_value_16 = predictor_get(&dav1d_dr_intra_derivative, {
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
        ::core::primitive::i32::wrapping_shr(i32::wrapping_sub(180, angle_2), 1_u32),
      );
    explicit_position
  })
  .ok_or(Rav1dError::EINVAL)?;
  let mut dx = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_16);

  let upsample_left = enable_intra_edge_filter
    && i32::wrapping_sub(180, angle_2) < 40
    && i32::wrapping_add(width_i, height_i)
      <= i32::wrapping_shr(
        16,
        i32::cast_unsigned(<i32 as ::core::convert::From<::core::primitive::bool>>::from(is_sm)),
      );
  let upsample_above = enable_intra_edge_filter
    && i32::wrapping_sub(angle_2, 90) < 40
    && i32::wrapping_add(width_i, height_i)
      <= i32::wrapping_shr(
        16,
        i32::cast_unsigned(<i32 as ::core::convert::From<::core::primitive::bool>>::from(is_sm)),
      );

  let mut edge_px = [0u16; 64 + 64 + 1];
  let edge_tl = 64u64;

  let rd = |off: u64| -> Rav1dResult<u16> {
    let byte_offset = off.checked_mul(2).ok_or(Rav1dError::EINVAL)?;
    let bytes = predictor_get(topleft, byte_offset..)
      .and_then(<[_]>::first_chunk::<2>)
      .ok_or(Rav1dError::EINVAL)?;
    Ok(u16::from_le_bytes(*bytes))
  };

  let tl_pix = tl_off.wrapping_div(2);

  match ::core::primitive::bool::then_some(upsample_above, ()) {
    ::core::option::Option::Some(()) => {
      let kernel: [i8; 4] = [-1, 9, 9, -1];
      let hsz = i32::wrapping_add(width_i, 1);
      let ::core::result::Result::Ok(in_off) =
        ::console_core_number_conversion::fitted::<_, u64>(tl_pix);

      for i in 0..i32::wrapping_sub(hsz, 1) {
        let checked_value_4 = rd(
          (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          })
          .wrapping_add({
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i.clamp(0, i32::wrapping_sub(hsz, 1)),
              );
            explicit_position
          }),
        )?;
        *{
          let checked_value_25 = predictor_get_mut(
            &mut edge_px,
            (::core::num::Wrapping(edge_tl)
              + ::core::num::Wrapping({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                    i32::wrapping_mul(i, 2),
                  );
                explicit_position
              }))
            .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        } = checked_value_4;
        let mut s = 0i32;

        for j in 0..4i32 {
          let checked_value_26 = rd(
            (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            })
            .wrapping_add({
              let ::core::result::Result::Ok(explicit_position) =
                ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                  i32::wrapping_sub(i32::wrapping_add(i, j), 1)
                    .clamp(0, i32::wrapping_sub(hsz, 1)),
                );
              explicit_position
            }),
          )?;

          match (
            ::core::primitive::i32::wrapping_mul(
              <i32 as ::core::convert::From<::core::primitive::u16>>::from(checked_value_26),
              <i32 as ::core::convert::From<::core::primitive::i8>>::from(*{
                let checked_value_27 = predictor_get(&kernel, {
                  let ::core::result::Result::Ok(explicit_position) =
                    ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(j);
                  explicit_position
                })
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_27
              }),
            ),
            &mut s,
          ) {
            (explicit_arithmetic_amount, explicit_arithmetic_place) => {
              *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
                *explicit_arithmetic_place,
                explicit_arithmetic_amount,
              )
            }
          };
        }

        *{
          let checked_value_28 = predictor_get_mut(
            &mut edge_px,
            (::core::num::Wrapping(edge_tl)
              + ::core::num::Wrapping({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                    i32::wrapping_add(i32::wrapping_mul(i, 2), 1),
                  );
                explicit_position
              }))
            .0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_28
        } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
          ::core::primitive::i32::wrapping_shr(i32::wrapping_add(s, 8), 4_u32)
            .clamp(0, bitdepth_max),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
      }

      let i = i32::wrapping_sub(hsz, 1);
      let checked_value_5 = rd(
        (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        })
        .wrapping_add({
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
              i.clamp(0, i32::wrapping_sub(hsz, 1)),
            );
          explicit_position
        }),
      )?;
      *{
        let checked_value_29 = predictor_get_mut(
          &mut edge_px,
          (::core::num::Wrapping(edge_tl)
            + ::core::num::Wrapping({
              let ::core::result::Result::Ok(explicit_position) =
                ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                  i32::wrapping_mul(i, 2),
                );
              explicit_position
            }))
          .0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_29
      } = checked_value_5;
      dx = ::core::primitive::i32::wrapping_shl(dx, 1_u32);
    }
    ::core::option::Option::None => {
      let Ok(filter_strength) =
        match ::core::primitive::bool::then_some(enable_intra_edge_filter, ()) {
          ::core::option::Option::Some(()) => get_filter_strength(EdgeFilterSelection {
            length: i32::wrapping_add(width_i, height_i),
            angle_delta: i32::wrapping_sub(angle_2, 90),
            smoothing: is_sm,
          }),
          ::core::option::Option::None => Ok(0),
        };

      match ::core::primitive::bool::then_some(filter_strength != 0, ()) {
        ::core::option::Option::Some(()) => {
          static KERNEL: [[u8; 5]; 3] = [[0, 4, 8, 4, 0], [0, 5, 6, 5, 0], [2, 4, 4, 4, 2]];
          let ::core::result::Result::Ok(in_off) = ::console_core_number_conversion::fitted::<
            _,
            u64,
          >((Wrapping(tl_pix) + Wrapping(1)).0);
          let from = -1i32;
          let to = width_i;
          let lim_from = 0i32;
          let lim_to = max_width;
          let mut i = 0i32;

          for _ in (::core::ops::Range {
            start: i,
            end: std::cmp::min(width_i, lim_from),
          }) {
            let checked_value_6 = rd(
              (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              })
              .wrapping_add_signed({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                    i.clamp(from, i32::wrapping_sub(to, 1)),
                  );
                explicit_position
              }),
            )?;
            *{
              let checked_value_30 = predictor_get_mut(
                &mut edge_px,
                edge_tl.wrapping_add(1).wrapping_add({
                  let ::core::result::Result::Ok(explicit_position) =
                    ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(i);
                  explicit_position
                }),
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_30
            } = checked_value_6;
            i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
          }

          for _ in (::core::ops::Range {
            start: i,
            end: std::cmp::min(lim_to, width_i),
          }) {
            let mut s = 0i32;

            for j in 0..5i32 {
              let checked_value_31 = rd(
                (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
                  ::core::result::Result::Ok(native_position) => native_position,
                })
                .wrapping_add_signed({
                  let ::core::result::Result::Ok(explicit_position) =
                    ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                      i32::wrapping_add(i32::wrapping_sub(i, 2), j)
                        .clamp(from, i32::wrapping_sub(to, 1)),
                    );
                  explicit_position
                }),
              )?;

              match (
                ::core::primitive::i32::wrapping_mul(
                  <i32 as ::core::convert::From<::core::primitive::u16>>::from(checked_value_31),
                  <i32 as ::core::convert::From<::core::primitive::u8>>::from(*{
                    let checked_value_52 =
                      predictor_get(
                        &(*{
                          let checked_value_51 = predictor_get(&KERNEL, {
                            let ::core::result::Result::Ok(explicit_position) =
                              ::console_core_number_conversion::wrapped::<
                                ::core::primitive::i32,
                                u64,
                              >(i32::wrapping_sub(
                                filter_strength,
                                1,
                              ));
                            explicit_position
                          })
                          .ok_or(Rav1dError::EINVAL)?;
                          checked_value_51
                        }),
                        {
                          let ::core::result::Result::Ok(explicit_position) =
                            ::console_core_number_conversion::wrapped::<
                              ::core::primitive::i32,
                              u64,
                            >(j);
                          explicit_position
                        },
                      )
                      .ok_or(Rav1dError::EINVAL)?;
                    checked_value_52
                  }),
                ),
                &mut s,
              ) {
                (explicit_arithmetic_amount, explicit_arithmetic_place) => {
                  *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
                    *explicit_arithmetic_place,
                    explicit_arithmetic_amount,
                  )
                }
              };
            }

            *{
              let checked_value_32 = predictor_get_mut(
                &mut edge_px,
                edge_tl.wrapping_add(1).wrapping_add({
                  let ::core::result::Result::Ok(explicit_position) =
                    ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(i);
                  explicit_position
                }),
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_32
            } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
              ::core::primitive::i32::wrapping_shr(i32::wrapping_add(s, 8), 4_u32),
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            };
            i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
          }

          for _ in (::core::ops::Range {
            start: i,
            end: width_i,
          }) {
            let checked_value_7 = rd(
              (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              })
              .wrapping_add_signed({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                    i.clamp(from, i32::wrapping_sub(to, 1)),
                  );
                explicit_position
              }),
            )?;
            *{
              let checked_value_33 = predictor_get_mut(
                &mut edge_px,
                edge_tl.wrapping_add(1).wrapping_add({
                  let ::core::result::Result::Ok(explicit_position) =
                    ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(i);
                  explicit_position
                }),
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_33
            } = checked_value_7;
            i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
          }
        }
        ::core::option::Option::None => {
          for i in 0..width {
            let checked_value_8 =
              rd((Wrapping((Wrapping(tl_pix) + Wrapping(1)).0) + Wrapping(i)).0)?;
            *{
              let checked_value_34 = predictor_get_mut(
                &mut edge_px,
                (Wrapping(edge_tl.wrapping_add(1)) + Wrapping(i)).0,
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_34
            } = checked_value_8;
          }
        }
      }
    }
  }

  match ::core::primitive::bool::then_some(upsample_left, ()) {
    ::core::option::Option::Some(()) => {
      let kernel: [i8; 4] = [-1, 9, 9, -1];
      let hsz = i32::wrapping_add(height_i, 1);
      let ::core::result::Result::Ok(in_off) = ::console_core_number_conversion::fitted::<_, u64>(
        (Wrapping(tl_pix) - Wrapping(height)).0,
      );

      for i in 0..i32::wrapping_sub(hsz, 1) {
        let checked_value_9 = rd(
          (::core::num::Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + ::core::num::Wrapping({
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i.clamp(0, i32::wrapping_sub(hsz, 1)),
              );
            explicit_position
          }))
          .0,
        )?;
        *{
          let checked_value_35 = predictor_get_mut(
            &mut edge_px,
            (::core::num::Wrapping(edge_tl) - ::core::num::Wrapping(height.wrapping_mul(2)))
              .0
              .wrapping_add({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                    i32::wrapping_mul(i, 2),
                  );
                explicit_position
              }),
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_35
        } = checked_value_9;
        let mut s = 0i32;

        for j in 0..4i32 {
          let checked_value_36 = rd(
            (::core::num::Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + ::core::num::Wrapping({
              let ::core::result::Result::Ok(explicit_position) =
                ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                  i32::wrapping_sub(i32::wrapping_add(i, j), 1)
                    .clamp(0, i32::wrapping_sub(hsz, 1)),
                );
              explicit_position
            }))
            .0,
          )?;

          match (
            ::core::primitive::i32::wrapping_mul(
              <i32 as ::core::convert::From<::core::primitive::u16>>::from(checked_value_36),
              <i32 as ::core::convert::From<::core::primitive::i8>>::from(*{
                let checked_value_37 = predictor_get(&kernel, {
                  let ::core::result::Result::Ok(explicit_position) =
                    ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(j);
                  explicit_position
                })
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_37
              }),
            ),
            &mut s,
          ) {
            (explicit_arithmetic_amount, explicit_arithmetic_place) => {
              *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
                *explicit_arithmetic_place,
                explicit_arithmetic_amount,
              )
            }
          };
        }

        *{
          let checked_value_38 = predictor_get_mut(
            &mut edge_px,
            (::core::num::Wrapping(edge_tl) - ::core::num::Wrapping(height.wrapping_mul(2)))
              .0
              .wrapping_add({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                    i32::wrapping_add(i32::wrapping_mul(i, 2), 1),
                  );
                explicit_position
              }),
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_38
        } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
          ::core::primitive::i32::wrapping_shr(i32::wrapping_add(s, 8), 4_u32)
            .clamp(0, bitdepth_max),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
      }

      let i = i32::wrapping_sub(hsz, 1);
      let checked_value_10 = rd(
        (::core::num::Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + ::core::num::Wrapping({
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
              i.clamp(0, i32::wrapping_sub(hsz, 1)),
            );
          explicit_position
        }))
        .0,
      )?;
      *{
        let checked_value_39 = predictor_get_mut(
          &mut edge_px,
          (::core::num::Wrapping(edge_tl) - ::core::num::Wrapping(height.wrapping_mul(2)))
            .0
            .wrapping_add({
              let ::core::result::Result::Ok(explicit_position) =
                ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                  i32::wrapping_mul(i, 2),
                );
              explicit_position
            }),
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_39
      } = checked_value_10;
      dy = ::core::primitive::i32::wrapping_shl(dy, 1_u32);
    }
    ::core::option::Option::None => {
      let Ok(filter_strength) =
        match ::core::primitive::bool::then_some(enable_intra_edge_filter, ()) {
          ::core::option::Option::Some(()) => get_filter_strength(EdgeFilterSelection {
            length: i32::wrapping_add(width_i, height_i),
            angle_delta: i32::wrapping_sub(180, angle_2),
            smoothing: is_sm,
          }),
          ::core::option::Option::None => Ok(0),
        };

      match ::core::primitive::bool::then_some(filter_strength != 0, ()) {
        ::core::option::Option::Some(()) => {
          static KERNEL: [[u8; 5]; 3] = [[0, 4, 8, 4, 0], [0, 5, 6, 5, 0], [2, 4, 4, 4, 2]];
          let ::core::result::Result::Ok(in_off) =
            ::console_core_number_conversion::fitted::<_, u64>(
              (Wrapping(tl_pix) - Wrapping(height)).0,
            );
          let from = 0i32;
          let to = i32::wrapping_add(height_i, 1);
          let lim_from = i32::wrapping_sub(height_i, max_height);
          let lim_to = height_i;
          let mut i = 0i32;

          for _ in (::core::ops::Range {
            start: i,
            end: std::cmp::min(height_i, lim_from),
          }) {
            let checked_value_11 = rd(
              (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              })
              .wrapping_add_signed({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                    i.clamp(from, i32::wrapping_sub(to, 1)),
                  );
                explicit_position
              }),
            )?;
            *{
              let checked_value_40 = predictor_get_mut(
                &mut edge_px,
                (::core::num::Wrapping(edge_tl) - ::core::num::Wrapping(height))
                  .0
                  .wrapping_add({
                    let ::core::result::Result::Ok(explicit_position) =
                      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(i);
                    explicit_position
                  }),
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_40
            } = checked_value_11;
            i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
          }

          for _ in (::core::ops::Range {
            start: i,
            end: std::cmp::min(lim_to, height_i),
          }) {
            let mut s = 0i32;

            for j in 0..5i32 {
              let checked_value_41 = rd(
                (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
                  ::core::result::Result::Ok(native_position) => native_position,
                })
                .wrapping_add_signed({
                  let ::core::result::Result::Ok(explicit_position) =
                    ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                      i32::wrapping_add(i32::wrapping_sub(i, 2), j)
                        .clamp(from, i32::wrapping_sub(to, 1)),
                    );
                  explicit_position
                }),
              )?;

              match (
                ::core::primitive::i32::wrapping_mul(
                  <i32 as ::core::convert::From<::core::primitive::u16>>::from(checked_value_41),
                  <i32 as ::core::convert::From<::core::primitive::u8>>::from(*{
                    let checked_value_54 =
                      predictor_get(
                        &(*{
                          let checked_value_53 = predictor_get(&KERNEL, {
                            let ::core::result::Result::Ok(explicit_position) =
                              ::console_core_number_conversion::wrapped::<
                                ::core::primitive::i32,
                                u64,
                              >(i32::wrapping_sub(
                                filter_strength,
                                1,
                              ));
                            explicit_position
                          })
                          .ok_or(Rav1dError::EINVAL)?;
                          checked_value_53
                        }),
                        {
                          let ::core::result::Result::Ok(explicit_position) =
                            ::console_core_number_conversion::wrapped::<
                              ::core::primitive::i32,
                              u64,
                            >(j);
                          explicit_position
                        },
                      )
                      .ok_or(Rav1dError::EINVAL)?;
                    checked_value_54
                  }),
                ),
                &mut s,
              ) {
                (explicit_arithmetic_amount, explicit_arithmetic_place) => {
                  *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
                    *explicit_arithmetic_place,
                    explicit_arithmetic_amount,
                  )
                }
              };
            }

            *{
              let checked_value_42 = predictor_get_mut(
                &mut edge_px,
                (::core::num::Wrapping(edge_tl) - ::core::num::Wrapping(height))
                  .0
                  .wrapping_add({
                    let ::core::result::Result::Ok(explicit_position) =
                      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(i);
                    explicit_position
                  }),
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_42
            } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
              ::core::primitive::i32::wrapping_shr(i32::wrapping_add(s, 8), 4_u32),
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            };
            i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
          }

          for _ in (::core::ops::Range {
            start: i,
            end: height_i,
          }) {
            let checked_value_12 = rd(
              (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              })
              .wrapping_add_signed({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                    i.clamp(from, i32::wrapping_sub(to, 1)),
                  );
                explicit_position
              }),
            )?;
            *{
              let checked_value_43 = predictor_get_mut(
                &mut edge_px,
                (::core::num::Wrapping(edge_tl) - ::core::num::Wrapping(height))
                  .0
                  .wrapping_add({
                    let ::core::result::Result::Ok(explicit_position) =
                      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(i);
                    explicit_position
                  }),
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_43
            } = checked_value_12;
            i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
          }
        }
        ::core::option::Option::None => {
          for i in 0..height {
            let checked_value_13 =
              rd((Wrapping((Wrapping(tl_pix) - Wrapping(height)).0) + Wrapping(i)).0)?;
            *{
              let checked_value_44 = predictor_get_mut(
                &mut edge_px,
                (Wrapping((Wrapping(edge_tl) - Wrapping(height)).0) + Wrapping(i)).0,
              )
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_44
            } = checked_value_13;
          }
        }
      }
    }
  }

  let checked_value_14 = rd(tl_pix)?;
  *{
    let checked_value_45 = predictor_get_mut(&mut edge_px, edge_tl).ok_or(Rav1dError::EINVAL)?;
    checked_value_45
  } = checked_value_14;

  let edge_bytes: &[u8] = zerocopy::IntoBytes::as_bytes(edge_px.as_slice());
  let edge = &*edge_bytes;

  let base_inc_x = (Wrapping(1)
    + Wrapping(<u64 as ::core::convert::From<::core::primitive::bool>>::from(upsample_above)))
  .0;
  let left = (Wrapping(edge_tl)
    - Wrapping(
      (Wrapping(1)
        + Wrapping(<u64 as ::core::convert::From<::core::primitive::bool>>::from(upsample_left)))
      .0,
    ))
  .0;

  let rounding = _mm256_set1_epi32(32);

  for y in 0..height_i {
    let xpos = i32::wrapping_sub(
      ::core::primitive::i32::wrapping_shl(
        ::core::primitive::i32::wrapping_add(
          1,
          <i32 as ::core::convert::From<::core::primitive::bool>>::from(upsample_above),
        ),
        6_u32,
      ),
      i32::wrapping_mul(dx, ::core::primitive::i32::wrapping_add(y, 1)),
    );
    let base_x0 = ::core::primitive::i32::wrapping_shr(xpos, 6_u32);
    let frac_x =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(xpos & 0x3e) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    let inv_frac_x = ::core::primitive::i32::wrapping_sub(64, frac_x);

    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      dst_base
        .cast_signed()
        .wrapping_add(
          ({
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(y);
            explicit_position
          })
          .wrapping_mul(stride),
        )
        .cast_unsigned(),
    );

    let left_count = match ::core::primitive::bool::then_some(base_x0 >= 0, ()) {
      ::core::option::Option::Some(()) => 0u64,
      ::core::option::Option::None => {
        let ::core::result::Result::Ok(needed) =
          ::console_core_number_conversion::fitted::<_, u64>({
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i32::wrapping_neg(base_x0),
              );
            explicit_position
          });
        (match ::console_core_number_conversion::fitted::<_, u64>(needed) {
          ::core::result::Result::Ok(native_position) => native_position,
        })
        .div_ceil(base_inc_x)
        .min(width)
      }
    };

    let mut x = 0u64;

    for _ in (::core::ops::Range {
      start: x,
      end: left_count,
    }) {
      let ypos = i32::wrapping_sub(
        i32::wrapping_shl(
          y,
          i32::cast_unsigned(::core::primitive::i32::wrapping_add(
            6,
            <i32 as ::core::convert::From<::core::primitive::bool>>::from(upsample_left),
          )),
        ),
        i32::wrapping_mul(
          dy,
          i32::wrapping_add(
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(x) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            },
            1,
          ),
        ),
      );
      let base_y = ::core::primitive::i32::wrapping_shr(ypos, 6_u32);
      let frac_y = ypos & 0x3e;
      let inv_frac_y = ::core::primitive::i32::wrapping_sub(64, frac_y);

      let ::core::result::Result::Ok(l0_pix) =
        ::console_core_number_conversion::fitted::<_, u64>(left.wrapping_add_signed({
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
              i32::wrapping_neg(base_y),
            );
          explicit_position
        }));
      let ::core::result::Result::Ok(l1_pix) =
        ::console_core_number_conversion::fitted::<_, u64>(left.wrapping_add_signed({
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
              i32::wrapping_neg(i32::wrapping_add(base_y, 1)),
            );
          explicit_position
        }));
      let l0_off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(l0_pix) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) * Wrapping(2))
      .0;
      let l1_off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(l1_pix) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) * Wrapping(2))
      .0;
      let l0 =
        <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
          let array_window = (*{
            let checked_value_46 =
              predictor_get(&*edge, l0_off..(Wrapping(l0_off) + Wrapping(2)).0)
                .ok_or(Rav1dError::EINVAL)?;
            checked_value_46
          })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        }));
      let l1 =
        <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
          let array_window = (*{
            let checked_value_47 =
              predictor_get(&*edge, l1_off..(Wrapping(l1_off) + Wrapping(2)).0)
                .ok_or(Rav1dError::EINVAL)?;
            checked_value_47
          })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        }));
      let v = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_mul(l0, inv_frac_y),
        ::core::primitive::i32::wrapping_mul(l1, frac_y),
      );
      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;

      {
        let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
          .and_then(<[_]>::first_chunk_mut::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        *explicit_byte_copy_destination = match ::console_core_number_conversion::wrapped::<
          _,
          ::core::primitive::u16,
        >(::core::primitive::i32::wrapping_shr(
          ::core::primitive::i32::wrapping_add(v, 32),
          6_u32,
        )) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        }
        .to_le_bytes();
      };

      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }

    match ::core::primitive::bool::then_some(base_inc_x == 1, ()) {
      ::core::option::Option::Some(()) => {
        'inner_while_2: for _ in 0..width.saturating_sub(x).wrapping_div(8) {
          let ::core::result::Result::Ok(base_x) =
            ::console_core_number_conversion::fitted::<_, u64>({
              let ::core::result::Result::Ok(explicit_position) =
                ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                  i32::wrapping_add(
                    base_x0,
                    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
                      x,
                    ) {
                      ::core::result::Result::Ok(converted_position) => converted_position,
                    },
                  ),
                );
              explicit_position
            });
          let load0 = (Wrapping(
            (Wrapping(edge_tl)
              + Wrapping(
                match ::console_core_number_conversion::fitted::<_, u64>(base_x) {
                  ::core::result::Result::Ok(native_position) => native_position,
                },
              ))
            .0,
          ) * Wrapping(2))
          .0;
          let load1 = (Wrapping(
            (Wrapping(
              (Wrapping(edge_tl)
                + Wrapping(
                  match ::console_core_number_conversion::fitted::<_, u64>(base_x) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  },
                ))
              .0,
            ) + Wrapping(1))
            .0,
          ) * Wrapping(2))
          .0;

          match ::core::primitive::bool::then_some(
            (Wrapping(load1) + Wrapping(16)).0 > {
              let Ok(length) = console_core_number_conversion::fixed_index(edge.len());
              length
            },
            (),
          ) {
            ::core::option::Option::Some(()) => {
              break 'inner_while_2;
            }
            ::core::option::Option::None => {}
          }

          let checked_value = predictor_get(&*edge, load0..(Wrapping(load0) + Wrapping(16)).0)
            .ok_or(Rav1dError::EINVAL)?;
          let t0 = loadu_128!(checked_value, [u8; 16]);
          let checked_value_2 = predictor_get(&*edge, load1..(Wrapping(load1) + Wrapping(16)).0)
            .ok_or(Rav1dError::EINVAL)?;
          let t1 = loadu_128!(checked_value_2, [u8; 16]);

          let t0_w = _mm256_cvtepu16_epi32(t0);
          let t1_w = _mm256_cvtepu16_epi32(t1);

          let frac_vec = _mm256_set1_epi32(frac_x);
          let inv_frac_vec = _mm256_set1_epi32(inv_frac_x);

          let prod0 = _mm256_mullo_epi32(t0_w, inv_frac_vec);
          let prod1 = _mm256_mullo_epi32(t1_w, frac_vec);
          let sum = _mm256_add_epi32(_mm256_add_epi32(prod0, prod1), rounding);
          let result = _mm256_srai_epi32::<6>(sum);

          let packed = _mm256_packus_epi32(result, result);
          let lo = _mm256_castsi256_si128(packed);
          let hi = _mm256_extracti128_si256::<1>(packed);
          let combined = _mm_unpacklo_epi64(lo, hi);
          let store_off = (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
          .0;
          let checked_value_3 = predictor_get_mut(
            &mut *dst_2,
            store_off..(Wrapping(store_off) + Wrapping(16)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          storeu_128!(checked_value_3, [u8; 16], combined);

          let ::core::result::Result::Ok(()) = advance_predictor_position(8, &mut x);
        }
      }
      ::core::option::Option::None => {}
    }

    'inner_for_2: for _ in (::core::ops::Range {
      start: x,
      end: width,
    }) {
      let ::core::result::Result::Ok(base_x) =
        ::console_core_number_conversion::fitted::<_, u64>({
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
              i32::wrapping_add(
                base_x0,
                match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
                  (Wrapping(base_inc_x) * Wrapping(x)).0,
                ) {
                  ::core::result::Result::Ok(converted_position) => converted_position,
                },
              ),
            );
          explicit_position
        });
      let t0_off = (Wrapping(
        (Wrapping(edge_tl)
          + Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(base_x) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ))
        .0,
      ) * Wrapping(2))
      .0;
      let t1_off = (Wrapping(
        (Wrapping(
          (Wrapping(edge_tl)
            + Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(base_x) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ))
          .0,
        ) + Wrapping(1))
        .0,
      ) * Wrapping(2))
      .0;

      match ::core::primitive::bool::then_some(
        (Wrapping(t1_off) + Wrapping(2)).0 > {
          let Ok(length) = console_core_number_conversion::fixed_index(edge.len());
          length
        },
        (),
      ) {
        ::core::option::Option::Some(()) => {
          break 'inner_for_2;
        }
        ::core::option::Option::None => {}
      }

      let t0 =
        <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
          let array_window = (*{
            let checked_value_49 =
              predictor_get(&*edge, t0_off..(Wrapping(t0_off) + Wrapping(2)).0)
                .ok_or(Rav1dError::EINVAL)?;
            checked_value_49
          })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        }));
      let t1 =
        <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
          let array_window = (*{
            let checked_value_50 =
              predictor_get(&*edge, t1_off..(Wrapping(t1_off) + Wrapping(2)).0)
                .ok_or(Rav1dError::EINVAL)?;
            checked_value_50
          })
          .try_into()
          .map_err(|_| Rav1dError::EINVAL)?;
          array_window
        }));
      let v = ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_mul(t0, inv_frac_x),
        ::core::primitive::i32::wrapping_mul(t1, frac_x),
      );
      let off = (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
      .0;

      {
        let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
          .and_then(<[_]>::first_chunk_mut::<2>)
          .ok_or(Rav1dError::EINVAL)?;
        *explicit_byte_copy_destination = match ::console_core_number_conversion::wrapped::<
          _,
          ::core::primitive::u16,
        >(::core::primitive::i32::wrapping_shr(
          ::core::primitive::i32::wrapping_add(v, 32),
          6_u32,
        )) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        }
        .to_le_bytes();
      };

      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredZ216bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
  angle: i32,
  max_width: i32,
  max_height: i32,
  bitdepth_max: i32,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_z3_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredZ316bpcInnerArguments {
    tl_off,
    width,
    height,
    angle,
    bitdepth_max,
  }: IpredZ316bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let width_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(width) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  let height_i =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(height) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

  let is_sm = ::core::primitive::i32::wrapping_shr(angle, 9_u32) & 1 != 0;
  let enable_intra_edge_filter = ::core::primitive::i32::wrapping_shr(angle, 10_u32) != 0;
  let angle_2 = angle & 511;

  let checked_value_5 = predictor_get(&dav1d_dr_intra_derivative, {
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
        ::core::primitive::i32::wrapping_shr(i32::wrapping_sub(270, angle_2), 1_u32),
      );
    explicit_position
  })
  .ok_or(Rav1dError::EINVAL)?;
  let mut dy = <u64 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);

  let tl_pix = tl_off.wrapping_div(2);

  let rd = |off: u64| -> Rav1dResult<u16> {
    let byte_offset = off.checked_mul(2).ok_or(Rav1dError::EINVAL)?;
    let bytes = predictor_get(topleft, byte_offset..)
      .and_then(<[_]>::first_chunk::<2>)
      .ok_or(Rav1dError::EINVAL)?;
    Ok(u16::from_le_bytes(*bytes))
  };

  let upsample_left = enable_intra_edge_filter
    && i32::wrapping_sub(angle_2, 180) < 40
    && i32::wrapping_add(width_i, height_i)
      <= i32::wrapping_shr(
        16,
        i32::cast_unsigned(<i32 as ::core::convert::From<::core::primitive::bool>>::from(is_sm)),
      );

  let mut left_px = [0u16; 64 + 64];
  let (left_off, max_base_y, base_inc);
  let use_left_px;

  match ::core::primitive::bool::then_some(upsample_left, ()) {
    ::core::option::Option::Some(()) => {
      let kernel: [i8; 4] = [-1, 9, 9, -1];
      let hsz = i32::wrapping_add(width_i, height_i);
      let ::core::result::Result::Ok(in_off) = ::console_core_number_conversion::fitted::<_, u64>(
        (Wrapping(tl_pix) - Wrapping((Wrapping(width) + Wrapping(height)).0)).0,
      );
      let from = std::cmp::max(i32::wrapping_sub(width_i, height_i), 0);
      let to = i32::wrapping_add(i32::wrapping_add(width_i, height_i), 1);

      for i in 0..i32::wrapping_sub(hsz, 1) {
        let checked_value = rd(
          (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          })
          .wrapping_add_signed({
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                i.clamp(from, i32::wrapping_sub(to, 1)),
              );
            explicit_position
          }),
        )?;
        *{
          let checked_value_25 = predictor_get_mut(&mut left_px, {
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i32::wrapping_mul(i, 2),
              );
            explicit_position
          })
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        } = checked_value;
        let mut s = 0i32;

        for j in 0..4i32 {
          let checked_value_26 = rd(
            (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            })
            .wrapping_add_signed({
              let ::core::result::Result::Ok(explicit_position) =
                ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                  i32::wrapping_sub(i32::wrapping_add(i, j), 1)
                    .clamp(from, i32::wrapping_sub(to, 1)),
                );
              explicit_position
            }),
          )?;

          match (
            ::core::primitive::i32::wrapping_mul(
              <i32 as ::core::convert::From<::core::primitive::u16>>::from(checked_value_26),
              <i32 as ::core::convert::From<::core::primitive::i8>>::from(*{
                let checked_value_27 = predictor_get(&kernel, {
                  let ::core::result::Result::Ok(explicit_position) =
                    ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(j);
                  explicit_position
                })
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_27
              }),
            ),
            &mut s,
          ) {
            (explicit_arithmetic_amount, explicit_arithmetic_place) => {
              *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
                *explicit_arithmetic_place,
                explicit_arithmetic_amount,
              )
            }
          };
        }

        *{
          let checked_value_28 = predictor_get_mut(&mut left_px, {
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i32::wrapping_add(i32::wrapping_mul(i, 2), 1),
              );
            explicit_position
          })
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_28
        } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
          ::core::primitive::i32::wrapping_shr(i32::wrapping_add(s, 8), 4_u32)
            .clamp(0, bitdepth_max),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
      }

      let i = i32::wrapping_sub(hsz, 1);
      let checked_value_2 = rd(
        (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        })
        .wrapping_add_signed({
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
              i.clamp(from, i32::wrapping_sub(to, 1)),
            );
          explicit_position
        }),
      )?;
      *{
        let checked_value_29 = predictor_get_mut(&mut left_px, {
          let ::core::result::Result::Ok(explicit_position) =
            ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
              i32::wrapping_mul(i, 2),
            );
          explicit_position
        })
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_29
      } = checked_value_2;
      left_off = {
        let ::core::result::Result::Ok(explicit_position) =
          ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
            i32::wrapping_sub(
              i32::wrapping_mul(2, i32::wrapping_add(width_i, height_i)),
              2,
            ),
          );
        explicit_position
      };
      max_base_y = left_off;
      dy = ::core::primitive::u64::wrapping_shl(dy, 1_u32);
      base_inc = 2u64;
      use_left_px = true;
    }
    ::core::option::Option::None => {
      let Ok(filter_strength) =
        match ::core::primitive::bool::then_some(enable_intra_edge_filter, ()) {
          ::core::option::Option::Some(()) => get_filter_strength(EdgeFilterSelection {
            length: i32::wrapping_add(width_i, height_i),
            angle_delta: i32::wrapping_sub(angle_2, 180),
            smoothing: is_sm,
          }),
          ::core::option::Option::None => Ok(0),
        };

      match ::core::primitive::bool::then_some(filter_strength != 0, ()) {
        ::core::option::Option::Some(()) => {
          static KERNEL: [[u8; 5]; 3] = [[0, 4, 8, 4, 0], [0, 5, 6, 5, 0], [2, 4, 4, 4, 2]];
          let ::core::result::Result::Ok(in_off) =
            ::console_core_number_conversion::fitted::<_, u64>(
              (Wrapping(tl_pix) - Wrapping((Wrapping(width) + Wrapping(height)).0)).0,
            );
          let from = std::cmp::max(i32::wrapping_sub(width_i, height_i), 0);
          let to = i32::wrapping_add(i32::wrapping_add(width_i, height_i), 1);
          let lim_from = 0i32;
          let lim_to = i32::wrapping_add(width_i, height_i);
          let mut i = 0i32;

          for _ in (::core::ops::Range {
            start: i,
            end: std::cmp::min(i32::wrapping_add(width_i, height_i), lim_from),
          }) {
            let checked_value_3 = rd(
              (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              })
              .wrapping_add_signed({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                    i.clamp(from, i32::wrapping_sub(to, 1)),
                  );
                explicit_position
              }),
            )?;
            *{
              let checked_value_30 = predictor_get_mut(&mut left_px, {
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(i);
                explicit_position
              })
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_30
            } = checked_value_3;
            i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
          }

          for _ in (::core::ops::Range {
            start: i,
            end: std::cmp::min(lim_to, i32::wrapping_add(width_i, height_i)),
          }) {
            let mut s = 0i32;

            for j in 0..5i32 {
              let checked_value_31 = rd(
                (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
                  ::core::result::Result::Ok(native_position) => native_position,
                })
                .wrapping_add_signed({
                  let ::core::result::Result::Ok(explicit_position) =
                    ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                      i32::wrapping_add(i32::wrapping_sub(i, 2), j)
                        .clamp(from, i32::wrapping_sub(to, 1)),
                    );
                  explicit_position
                }),
              )?;

              match (
                ::core::primitive::i32::wrapping_mul(
                  <i32 as ::core::convert::From<::core::primitive::u16>>::from(checked_value_31),
                  <i32 as ::core::convert::From<::core::primitive::u8>>::from(*{
                    let checked_value_52 =
                      predictor_get(
                        &(*{
                          let checked_value_51 = predictor_get(&KERNEL, {
                            let ::core::result::Result::Ok(explicit_position) =
                              ::console_core_number_conversion::wrapped::<
                                ::core::primitive::i32,
                                u64,
                              >(i32::wrapping_sub(
                                filter_strength,
                                1,
                              ));
                            explicit_position
                          })
                          .ok_or(Rav1dError::EINVAL)?;
                          checked_value_51
                        }),
                        {
                          let ::core::result::Result::Ok(explicit_position) =
                            ::console_core_number_conversion::wrapped::<
                              ::core::primitive::i32,
                              u64,
                            >(j);
                          explicit_position
                        },
                      )
                      .ok_or(Rav1dError::EINVAL)?;
                    checked_value_52
                  }),
                ),
                &mut s,
              ) {
                (explicit_arithmetic_amount, explicit_arithmetic_place) => {
                  *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
                    *explicit_arithmetic_place,
                    explicit_arithmetic_amount,
                  )
                }
              };
            }

            *{
              let checked_value_32 = predictor_get_mut(&mut left_px, {
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(i);
                explicit_position
              })
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_32
            } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
              ::core::primitive::i32::wrapping_shr(i32::wrapping_add(s, 8), 4_u32),
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            };
            i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
          }

          for _ in (::core::ops::Range {
            start: i,
            end: i32::wrapping_add(width_i, height_i),
          }) {
            let checked_value_4 = rd(
              (match ::console_core_number_conversion::fitted::<_, u64>(in_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              })
              .wrapping_add_signed({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(
                    i.clamp(from, i32::wrapping_sub(to, 1)),
                  );
                explicit_position
              }),
            )?;
            *{
              let checked_value_33 = predictor_get_mut(&mut left_px, {
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(i);
                explicit_position
              })
              .ok_or(Rav1dError::EINVAL)?;
              checked_value_33
            } = checked_value_4;
            i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
          }

          left_off = {
            let ::core::result::Result::Ok(explicit_position) =
              ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(
                i32::wrapping_sub(i32::wrapping_add(width_i, height_i), 1),
              );
            explicit_position
          };
          max_base_y = left_off;
          use_left_px = true;
        }
        ::core::option::Option::None => {
          left_off = 0;
          max_base_y = (Wrapping((Wrapping(height) + Wrapping(std::cmp::min(width, height))).0)
            - Wrapping(1))
          .0;
          use_left_px = false;
        }
      }

      base_inc = 1;
    }
  };

  for x in 0..width {
    let ::core::result::Result::Ok(ypos) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dy) * Wrapping((Wrapping(x) + Wrapping(1)).0)).0,
    );
    let frac = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
      (match ::console_core_number_conversion::fitted::<_, u64>(ypos) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) & 0x3e,
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    let inv_frac = ::core::primitive::i32::wrapping_sub(64, frac);

    'over_y_2: for y in 0..height_i {
      let ::core::result::Result::Ok(base) = ::console_core_number_conversion::fitted::<_, u64>(
        (match ::console_core_number_conversion::fitted::<_, u64>(ypos) {
          ::core::result::Result::Ok(native_position) => native_position,
        })
        .wrapping_shr(6_u32)
        .wrapping_add(
          (::core::num::Wrapping(base_inc)
            * ::core::num::Wrapping({
              let ::core::result::Result::Ok(explicit_position) =
                ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(y);
              explicit_position
            }))
          .0,
        ),
      );

      match ::core::primitive::bool::then_some(
        (match ::console_core_number_conversion::fitted::<_, u64>(base) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) < max_base_y,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let (l0, l1) = match ::core::primitive::bool::then_some(use_left_px, ()) {
            ::core::option::Option::Some(()) => (
              <i32 as ::core::convert::From<::core::primitive::u16>>::from(*{
                let checked_value_34 = predictor_get(
                  &left_px,
                  (Wrapping(left_off)
                    - Wrapping(
                      match ::console_core_number_conversion::fitted::<_, u64>(base) {
                        ::core::result::Result::Ok(native_position) => native_position,
                      },
                    ))
                  .0,
                )
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_34
              }),
              <i32 as ::core::convert::From<::core::primitive::u16>>::from(*{
                let checked_value_35 = predictor_get(
                  &left_px,
                  (Wrapping(
                    (Wrapping(left_off)
                      - Wrapping(
                        match ::console_core_number_conversion::fitted::<_, u64>(base) {
                          ::core::result::Result::Ok(native_position) => native_position,
                        },
                      ))
                    .0,
                  ) - Wrapping(1))
                  .0,
                )
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_35
              }),
            ),
            ::core::option::Option::None => {
              let first = rd(
                (Wrapping(
                  (Wrapping(tl_pix)
                    - Wrapping(
                      match ::console_core_number_conversion::fitted::<_, u64>(base) {
                        ::core::result::Result::Ok(native_position) => native_position,
                      },
                    ))
                  .0,
                ) - Wrapping(1))
                .0,
              )?;
              let second = rd(
                (Wrapping(
                  (Wrapping(tl_pix)
                    - Wrapping(
                      match ::console_core_number_conversion::fitted::<_, u64>(base) {
                        ::core::result::Result::Ok(native_position) => native_position,
                      },
                    ))
                  .0,
                ) - Wrapping(2))
                .0,
              )?;
              (i32::from(first), i32::from(second))
            }
          };
          let v = i32::wrapping_add(i32::wrapping_mul(l0, inv_frac), i32::wrapping_mul(l1, frac));
          let pixel_off = dst_base
            .cast_signed()
            .wrapping_add(
              ({
                let ::core::result::Result::Ok(explicit_position) =
                  ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(y);
                explicit_position
              })
              .wrapping_mul(stride),
            )
            .cast_unsigned()
            .wrapping_add(x.wrapping_mul(2));

          {
            let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, pixel_off..)
              .and_then(<[_]>::first_chunk_mut::<2>)
              .ok_or(Rav1dError::EINVAL)?;
            *explicit_byte_copy_destination =
              match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
                ::core::primitive::i32::wrapping_shr(i32::wrapping_add(v, 32), 6_u32),
              ) {
                ::core::result::Result::Ok(converted_position) => converted_position,
              }
              .to_le_bytes();
          };
        }
        ::core::option::Option::None => {
          let fill_val = match ::core::primitive::bool::then_some(use_left_px, ()) {
            ::core::option::Option::Some(()) => {
              let checked_value_37 =
                predictor_get(&left_px, (Wrapping(left_off) - Wrapping(max_base_y)).0)
                  .ok_or(Rav1dError::EINVAL)?;
              *checked_value_37
            }
            ::core::option::Option::None => {
              rd((Wrapping((Wrapping(tl_pix) - Wrapping(max_base_y)).0) - Wrapping(1)).0)?
            }
          };

          for yy in y..height_i {
            let pixel_off = dst_base
              .cast_signed()
              .wrapping_add(
                ({
                  let ::core::result::Result::Ok(explicit_position) =
                    ::console_core_number_conversion::wrapped::<::core::primitive::i32, i64>(yy);
                  explicit_position
                })
                .wrapping_mul(stride),
              )
              .cast_unsigned()
              .wrapping_add(x.wrapping_mul(2));

            {
              let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, pixel_off..)
                .and_then(<[_]>::first_chunk_mut::<2>)
                .ok_or(Rav1dError::EINVAL)?;
              *explicit_byte_copy_destination = fill_val.to_le_bytes();
            };
          }

          break 'over_y_2;
        }
      }
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredZ316bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
  angle: i32,
  bitdepth_max: i32,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn ipred_filter_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  topleft: &[u8],
  IpredFilter16bpcInnerArguments {
    tl_off,
    width,
    height,
    filt_idx,
    bitdepth_max,
    topleft_off,
  }: IpredFilter16bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let topleft_2 = &*topleft;
  let width_2 = match ::console_core_number_conversion::wrapped::<_, u64>(width) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  }
  .wrapping_div(4)
  .wrapping_mul(4);
  let filt_idx_2 = ({
    let ::core::result::Result::Ok(explicit_position) =
      ::console_core_number_conversion::wrapped::<::core::primitive::i32, u64>(filt_idx);
    explicit_position
  }) & 511;

  let filter = &*{
    let checked_value_48 =
      predictor_get(&dav1d_filter_intra_taps, filt_idx_2).ok_or(Rav1dError::EINVAL)?;
    checked_value_48
  };

  for y in (0..height).step_by(2) {
    let ::core::result::Result::Ok(cur_tl_off) =
      ::console_core_number_conversion::fitted::<_, u64>((Wrapping(topleft_off) - Wrapping(y)).0);

    let tl_pixel_off = tl_off.wrapping_add(
      (Wrapping(
        match ::console_core_number_conversion::fitted::<_, u64>(cur_tl_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) * Wrapping(2))
      .0,
    );
    let mut tl_pixel =
      <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
        let array_window = (*{
          let checked_value_25 = predictor_get(
            &*topleft_2,
            tl_pixel_off..(Wrapping(tl_pixel_off) + Wrapping(2)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_25
        })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;
        array_window
      }));

    let ::core::result::Result::Ok(row0_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping((Wrapping(::core::primitive::u64::cast_signed(y)) * Wrapping(stride)).0))
      .0
      .cast_unsigned(),
    );
    let ::core::result::Result::Ok(row1_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(dst_base.cast_signed())
        + Wrapping(
          (Wrapping(::core::primitive::u64::cast_signed(
            (Wrapping(y) + Wrapping(1)).0,
          )) * Wrapping(stride))
          .0,
        ))
      .0
      .cast_unsigned(),
    );

    for x in (0..width_2).step_by(4) {
      let (p1, p2, p3, p4) = match ::core::primitive::bool::then_some(y == 0, ()) {
        ::core::option::Option::Some(()) => {
          let top_base = tl_off.wrapping_add(
            (Wrapping((Wrapping((Wrapping(topleft_off) + Wrapping(1)).0) + Wrapping(x)).0)
              * Wrapping(2))
            .0,
          );
          (
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
              let array_window = (*{
                let checked_value_26 =
                  predictor_get(&*topleft_2, top_base..(Wrapping(top_base) + Wrapping(2)).0)
                    .ok_or(Rav1dError::EINVAL)?;
                checked_value_26
              })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
              array_window
            })),
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
              let array_window = (*{
                let checked_value_27 = predictor_get(
                  &*topleft_2,
                  (Wrapping(top_base) + Wrapping(2)).0..(Wrapping(top_base) + Wrapping(4)).0,
                )
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_27
              })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
              array_window
            })),
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
              let array_window = (*{
                let checked_value_28 = predictor_get(
                  &*topleft_2,
                  (Wrapping(top_base) + Wrapping(4)).0..(Wrapping(top_base) + Wrapping(6)).0,
                )
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_28
              })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
              array_window
            })),
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
              let array_window = (*{
                let checked_value_29 = predictor_get(
                  &*topleft_2,
                  (Wrapping(top_base) + Wrapping(6)).0..(Wrapping(top_base) + Wrapping(8)).0,
                )
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_29
              })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
              array_window
            })),
          )
        }
        ::core::option::Option::None => {
          let ::core::result::Result::Ok(top_row) =
            ::console_core_number_conversion::fitted::<_, u64>(
              (Wrapping(dst_base.cast_signed())
                + Wrapping(
                  (Wrapping((Wrapping(::core::primitive::u64::cast_signed(y)) - Wrapping(1)).0)
                    * Wrapping(stride))
                  .0,
                ))
              .0
              .cast_unsigned(),
            );
          let tb = (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(top_row) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping((Wrapping(x) * Wrapping(2)).0))
          .0;
          (
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
              let array_window = (*{
                let checked_value_30 = predictor_get(&*dst_2, tb..(Wrapping(tb) + Wrapping(2)).0)
                  .ok_or(Rav1dError::EINVAL)?;
                checked_value_30
              })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
              array_window
            })),
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
              let array_window = (*{
                let checked_value_31 = predictor_get(
                  &*dst_2,
                  (Wrapping(tb) + Wrapping(2)).0..(Wrapping(tb) + Wrapping(4)).0,
                )
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_31
              })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
              array_window
            })),
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
              let array_window = (*{
                let checked_value_32 = predictor_get(
                  &*dst_2,
                  (Wrapping(tb) + Wrapping(4)).0..(Wrapping(tb) + Wrapping(6)).0,
                )
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_32
              })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
              array_window
            })),
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
              let array_window = (*{
                let checked_value_33 = predictor_get(
                  &*dst_2,
                  (Wrapping(tb) + Wrapping(6)).0..(Wrapping(tb) + Wrapping(8)).0,
                )
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_33
              })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
              array_window
            })),
          )
        }
      };

      let (p5, p6) = match ::core::primitive::bool::then_some(x == 0, ()) {
        ::core::option::Option::Some(()) => {
          let left_base = tl_off.wrapping_add(
            (Wrapping(
              (match ::console_core_number_conversion::fitted::<_, u64>(cur_tl_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              })
              .wrapping_sub(1),
            ) * Wrapping(2))
            .0,
          );
          let left_base2 = tl_off.wrapping_add(
            (Wrapping(
              (match ::console_core_number_conversion::fitted::<_, u64>(cur_tl_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              })
              .wrapping_sub(2),
            ) * Wrapping(2))
            .0,
          );
          (
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
              let array_window = (*{
                let checked_value_34 = predictor_get(
                  &*topleft_2,
                  left_base..(Wrapping(left_base) + Wrapping(2)).0,
                )
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_34
              })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
              array_window
            })),
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
              let array_window = (*{
                let checked_value_35 = predictor_get(
                  &*topleft_2,
                  left_base2..(Wrapping(left_base2) + Wrapping(2)).0,
                )
                .ok_or(Rav1dError::EINVAL)?;
                checked_value_35
              })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
              array_window
            })),
          )
        }
        ::core::option::Option::None => {
          let p5_off = (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(row0_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping((Wrapping((Wrapping(x) - Wrapping(1)).0) * Wrapping(2)).0))
          .0;
          let p6_off = (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(row1_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping((Wrapping((Wrapping(x) - Wrapping(1)).0) * Wrapping(2)).0))
          .0;
          (
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
              let array_window = (*{
                let checked_value_36 =
                  predictor_get(&*dst_2, p5_off..(Wrapping(p5_off) + Wrapping(2)).0)
                    .ok_or(Rav1dError::EINVAL)?;
                checked_value_36
              })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
              array_window
            })),
            <i32 as ::core::convert::From<::core::primitive::u16>>::from(u16::from_le_bytes({
              let array_window = (*{
                let checked_value_37 =
                  predictor_get(&*dst_2, p6_off..(Wrapping(p6_off) + Wrapping(2)).0)
                    .ok_or(Rav1dError::EINVAL)?;
                checked_value_37
              })
              .try_into()
              .map_err(|_| Rav1dError::EINVAL)?;
              array_window
            })),
          )
        }
      };

      let p0 = tl_pixel;
      let p = [p0, p1, p2, p3, p4, p5, p6];

      let Ok([first_row, second_row]) = filter_predictions(filter, p);

      for (xx, acc) in (0..4).zip(first_row) {
        let first_filtered_pixel =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
            ::core::primitive::i32::wrapping_shr(i32::wrapping_add(acc, 8), 4_u32).clamp(
              0,
              match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
                bitdepth_max,
              ) {
                ::core::result::Result::Ok(converted_position) => converted_position,
              },
            ),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };
        let off = (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(row0_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping((Wrapping((Wrapping(x) + Wrapping(xx)).0) * Wrapping(2)).0))
        .0;

        {
          let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
            .and_then(<[_]>::first_chunk_mut::<2>)
            .ok_or(Rav1dError::EINVAL)?;
          *explicit_byte_copy_destination = first_filtered_pixel.to_le_bytes();
        };
      }

      for (xx, acc) in (0..4).zip(second_row) {
        let second_filtered_pixel =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
            ::core::primitive::i32::wrapping_shr(i32::wrapping_add(acc, 8), 4_u32).clamp(
              0,
              match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
                bitdepth_max,
              ) {
                ::core::result::Result::Ok(converted_position) => converted_position,
              },
            ),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };
        let off = (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(row1_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping((Wrapping((Wrapping(x) + Wrapping(xx)).0) * Wrapping(2)).0))
        .0;

        {
          let explicit_byte_copy_destination = predictor_get_mut(&mut *dst_2, off..)
            .and_then(<[_]>::first_chunk_mut::<2>)
            .ok_or(Rav1dError::EINVAL)?;
          *explicit_byte_copy_destination = second_filtered_pixel.to_le_bytes();
        };
      }

      tl_pixel = p4;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct IpredFilter16bpcInnerArguments {
  tl_off: u64,
  width: u64,
  height: u64,
  filt_idx: i32,
  bitdepth_max: i32,
  topleft_off: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn cfl_pred_8bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  ac: &[i16],
  CflPred8bpcInnerArguments {
    width,
    height,
    dc,
    alpha,
  }: CflPred8bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let alpha_v = _mm256_set1_epi32(alpha);
  let dc_v = _mm256_set1_epi32(dc);
  let c32 = _mm256_set1_epi32(32);

  for y in 0..height {
    let row_off = (Wrapping(dst_base.cast_signed())
      + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
    .0
    .cast_unsigned();
    let ac_off = (Wrapping(y) * Wrapping(width)).0;

    match ::core::primitive::bool::then_some(width >= 8, ()) {
      ::core::option::Option::Some(()) => {
        let mut x = 0;

        for _ in 0..width.saturating_sub(x).wrapping_div(8) {
          let checked_value = predictor_get(
            &*ac,
            (Wrapping(ac_off) + Wrapping(x)).0
              ..(Wrapping((Wrapping(ac_off) + Wrapping(x)).0) + Wrapping(8)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          let ac128 = loadu_128!(checked_value, [i16; 8]);
          let ac32 = _mm256_cvtepi16_epi32(ac128);

          let diff = _mm256_mullo_epi32(ac32, alpha_v);
          let abs_diff = _mm256_abs_epi32(diff);
          let plus32 = _mm256_add_epi32(abs_diff, c32);
          let shifted = _mm256_srli_epi32::<6>(plus32);
          let signed = _mm256_sign_epi32(shifted, diff);
          let result = _mm256_add_epi32(dc_v, signed);

          let p16 = _mm256_packus_epi32(result, result);

          let p16_ordered = _mm256_permute4x64_epi64::<0b_00_00_10_00>(p16);
          let p16_lo = _mm256_castsi256_si128(p16_ordered);
          let p8 = _mm_packus_epi16(p16_lo, p16_lo);

          let dst_chunk: &mut [u8; 8] = (&mut *{
            let checked_value_25 = predictor_get_mut(
              &mut *dst_2,
              (Wrapping(row_off) + Wrapping(x)).0
                ..(Wrapping((Wrapping(row_off) + Wrapping(x)).0) + Wrapping(8)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_25
          })
            .try_into()
            .map_err(|_| Rav1dError::EINVAL)?;
          archmage::intrinsics::x86_64::_mm_storeu_si64(dst_chunk, p8);
          let ::core::result::Result::Ok(()) = advance_predictor_position(8, &mut x);
        }
      }
      ::core::option::Option::None => match ::core::primitive::bool::then_some(width == 4, ()) {
        ::core::option::Option::Some(()) => {
          let mut buf = [0i16; 8];

          {
            let checked_copy_destination =
              <[_]>::first_chunk_mut::<4>(&mut buf).ok_or(Rav1dError::EINVAL)?;
            let checked_copy_source = predictor_get(&*ac, ac_off..)
              .and_then(<[_]>::first_chunk::<4>)
              .ok_or(Rav1dError::EINVAL)?;
            *checked_copy_destination = *checked_copy_source;
          };

          let ac128 = loadu_128!(&buf);
          let ac32 = _mm256_cvtepi16_epi32(ac128);

          let diff = _mm256_mullo_epi32(ac32, alpha_v);
          let abs_diff = _mm256_abs_epi32(diff);
          let plus32 = _mm256_add_epi32(abs_diff, c32);
          let shifted = _mm256_srli_epi32::<6>(plus32);
          let signed = _mm256_sign_epi32(shifted, diff);
          let result = _mm256_add_epi32(dc_v, signed);

          let p16 = _mm256_packus_epi32(result, result);
          let p16_lo = _mm256_castsi256_si128(p16);
          let p8 = _mm_packus_epi16(p16_lo, p16_lo);
          let dst_chunk: &mut [u8; 4] = (&mut *{
            let checked_value_28 =
              predictor_get_mut(&mut *dst_2, row_off..(Wrapping(row_off) + Wrapping(4)).0)
                .ok_or(Rav1dError::EINVAL)?;
            checked_value_28
          })
            .try_into()
            .map_err(|_| Rav1dError::EINVAL)?;
          archmage::intrinsics::x86_64::_mm_storeu_si32(dst_chunk, p8);
        }
        ::core::option::Option::None => {}
      },
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct CflPred8bpcInnerArguments {
  width: u64,
  height: u64,
  dc: i32,
  alpha: i32,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn cfl_pred_16bpc_inner(
  _token: Desktop64,
  dst: &mut [u8],
  dst_base: u64,
  stride: i64,
  ac: &[i16],
  CflPred16bpcInnerArguments {
    width,
    height,
    dc,
    alpha,
    bitdepth_max,
  }: CflPred16bpcInnerArguments,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let alpha_v = _mm256_set1_epi32(alpha);
  let dc_v = _mm256_set1_epi32(dc);
  let c32 = _mm256_set1_epi32(32);
  let max_v = _mm256_set1_epi32(bitdepth_max);
  let zero_v = _mm256_setzero_si256();

  for y in 0..height {
    let row_off = (Wrapping(dst_base.cast_signed())
      + Wrapping((Wrapping(y.cast_signed()) * Wrapping(stride)).0))
    .0
    .cast_unsigned();
    let ac_off = (Wrapping(y) * Wrapping(width)).0;

    match ::core::primitive::bool::then_some(width >= 8, ()) {
      ::core::option::Option::Some(()) => {
        let mut x = 0;

        for _ in 0..width.saturating_sub(x).wrapping_div(8) {
          let checked_value = predictor_get(
            &*ac,
            (Wrapping(ac_off) + Wrapping(x)).0
              ..(Wrapping((Wrapping(ac_off) + Wrapping(x)).0) + Wrapping(8)).0,
          )
          .ok_or(Rav1dError::EINVAL)?;
          let ac128 = loadu_128!(checked_value, [i16; 8]);
          let ac32 = _mm256_cvtepi16_epi32(ac128);

          let diff = _mm256_mullo_epi32(ac32, alpha_v);
          let abs_diff = _mm256_abs_epi32(diff);
          let plus32 = _mm256_add_epi32(abs_diff, c32);
          let shifted = _mm256_srli_epi32::<6>(plus32);
          let signed = _mm256_sign_epi32(shifted, diff);
          let mut result = _mm256_add_epi32(dc_v, signed);

          result = _mm256_max_epi32(result, zero_v);
          result = _mm256_min_epi32(result, max_v);

          let p16 = _mm256_packus_epi32(result, result);
          let p16_ordered = _mm256_permute4x64_epi64::<0b_00_00_10_00>(p16);
          let p16_lo = _mm256_castsi256_si128(p16_ordered);

          let ::core::result::Result::Ok(byte_x) =
            ::console_core_number_conversion::fitted::<_, u64>((Wrapping(x) * Wrapping(2)).0);
          let dst_chunk: &mut [u8; 16] = (&mut *{
            let checked_value_25 = predictor_get_mut(
              &mut *dst_2,
              (Wrapping(row_off)
                + Wrapping(
                  match ::console_core_number_conversion::fitted::<_, u64>(byte_x) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  },
                ))
              .0
                ..(Wrapping(
                  (Wrapping(row_off)
                    + Wrapping(
                      match ::console_core_number_conversion::fitted::<_, u64>(byte_x) {
                        ::core::result::Result::Ok(native_position) => native_position,
                      },
                    ))
                  .0,
                ) + Wrapping(16))
                .0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_25
          })
            .try_into()
            .map_err(|_| Rav1dError::EINVAL)?;
          storeu_128!(dst_chunk, p16_lo);
          let ::core::result::Result::Ok(()) = advance_predictor_position(8, &mut x);
        }
      }
      ::core::option::Option::None => match ::core::primitive::bool::then_some(width == 4, ()) {
        ::core::option::Option::Some(()) => {
          let mut buf = [0i16; 8];

          {
            let checked_copy_destination =
              <[_]>::first_chunk_mut::<4>(&mut buf).ok_or(Rav1dError::EINVAL)?;
            let checked_copy_source = predictor_get(&*ac, ac_off..)
              .and_then(<[_]>::first_chunk::<4>)
              .ok_or(Rav1dError::EINVAL)?;
            *checked_copy_destination = *checked_copy_source;
          };

          let ac128 = loadu_128!(&buf);
          let ac32 = _mm256_cvtepi16_epi32(ac128);

          let diff = _mm256_mullo_epi32(ac32, alpha_v);
          let abs_diff = _mm256_abs_epi32(diff);
          let plus32 = _mm256_add_epi32(abs_diff, c32);
          let shifted = _mm256_srli_epi32::<6>(plus32);
          let signed = _mm256_sign_epi32(shifted, diff);
          let mut result = _mm256_add_epi32(dc_v, signed);
          result = _mm256_max_epi32(result, zero_v);
          result = _mm256_min_epi32(result, max_v);

          let p16 = _mm256_packus_epi32(result, result);
          let p16_lo = _mm256_castsi256_si128(p16);

          let dst_chunk: &mut [u8; 8] = (&mut *{
            let checked_value_28 =
              predictor_get_mut(&mut *dst_2, row_off..(Wrapping(row_off) + Wrapping(8)).0)
                .ok_or(Rav1dError::EINVAL)?;
            checked_value_28
          })
            .try_into()
            .map_err(|_| Rav1dError::EINVAL)?;
          archmage::intrinsics::x86_64::_mm_storeu_si64(dst_chunk, p16_lo);
        }
        ::core::option::Option::None => {}
      },
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct CflPred16bpcInnerArguments {
  width: u64,
  height: u64,
  dc: i32,
  alpha: i32,
  bitdepth_max: i32,
}

use crate::include::common::bitdepth::BitDepth;
use crate::include::dav1d::headers::Rav1dPixelLayoutSubSampled;
use crate::src::internal::IntraEdgeSamples;
use crate::src::ipred::{
  CflAcExtraction, CflProjection, EdgeFilterSelection, IntraPrediction, PredictionDispatch,
  get_filter_strength,
};

#[cfg(target_arch = "x86_64")]
pub fn intra_pred_dispatch<BD: BitDepth>(
  dst: &mut crate::src::owned_recon::ReconDst<'_>,
  topleft: &IntraEdgeSamples<BD::Pixel>,
  prediction: IntraPrediction,
  bd: BD,
) -> Rav1dResult<PredictionDispatch> {
  use crate::include::common::bitdepth::BPC;
  use zerocopy::IntoBytes;

  let Ok(summon_avx2) = crate::src::cpu::summon_avx2();
  let token = match summon_avx2 {
    Some(token) => token,
    None => {
      return Ok(PredictionDispatch::ScalarFallback);
    }
  };

  #[cfg(target_arch = "x86_64")]
  let Ok(avx512_token) = crate::src::cpu::summon_avx512();
  #[cfg(not(target_arch = "x86_64"))]
  let avx512_token: Option<Server64> = None;

  #[cfg(target_arch = "x86_64")]
  let Ok(avx512x_token) = crate::src::cpu::summon_avx512x();

  let IntraPrediction {
    mode,
    edge_center: topleft_off,
    size: Size { width, height },
    reach: Size {
      width: max_width,
      height: max_height,
    },
    angle,
  } = prediction;
  let Ok(w) = console_core_number_conversion::wrapped::<_, u64>(width);
  let Ok(h) = console_core_number_conversion::wrapped::<_, u64>(height);
  let bd_c = bd.into_c();

  let tl_bytes: &[u8] = topleft.as_bytes();

  let with_block_mut = dst.with_block_mut::<BD, _, _>(
    Size {
      width: w,
      height: h,
    },
    tl_bytes,
    |dst_bytes, dst_base_bytes, byte_stride, edge_bytes| match (BD::BPC, mode) {
      (BPC::BPC8, 0) => match avx512_token {
        Some(t512) => ipred_dc_8bpc_avx512_inner(
          t512,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredDc8bpcAvx512InnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
        None => ipred_dc_8bpc_inner(
          token,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredDc8bpcInnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
      },
      (BPC::BPC8, 1) => match avx512_token {
        Some(t512) => ipred_v_8bpc_avx512_inner(
          t512,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredV8bpcAvx512InnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
        None => ipred_v_8bpc_inner(
          token,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredV8bpcInnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
      },
      (BPC::BPC8, 2) => match avx512_token {
        Some(t512) => ipred_h_8bpc_avx512_inner(
          t512,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredH8bpcAvx512InnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
        None => ipred_h_8bpc_inner(
          token,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredH8bpcInnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
      },
      (BPC::BPC8, 3) => match avx512_token {
        Some(t512) => ipred_dc_left_8bpc_avx512_inner(
          t512,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredDcLeft8bpcAvx512InnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
        None => ipred_dc_left_8bpc_inner(
          token,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredDcLeft8bpcInnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
      },
      (BPC::BPC8, 4) => match avx512_token {
        Some(t512) => ipred_dc_top_8bpc_avx512_inner(
          t512,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredDcTop8bpcAvx512InnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
        None => ipred_dc_top_8bpc_inner(
          token,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredDcTop8bpcInnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
      },
      (BPC::BPC8, 5) => match avx512_token {
        Some(t512) => ipred_dc_128_8bpc_avx512_inner(
          t512,
          dst_bytes,
          self::IpredDc1288bpcAvx512InnerArguments {
            dst_base: dst_base_bytes,
            stride: byte_stride,
            width: w,
            height: h,
          },
        ),
        None => ipred_dc_128_8bpc_inner(
          token,
          dst_bytes,
          self::IpredDc1288bpcInnerArguments {
            dst_base: dst_base_bytes,
            stride: byte_stride,
            width: w,
            height: h,
          },
        ),
      },
      (BPC::BPC8, 6) => match avx512x_token {
        Some(t512x) => ipred_z1_8bpc_v4x_inner(
          t512x,
          dst_bytes,
          edge_bytes,
          DirectionalPrediction {
            destination: PredictorOutput {
              base: dst_base_bytes,
              stride: byte_stride,
              size: Size {
                width: w,
                height: h,
              },
            },
            edge_center: topleft_off,
            angle: {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(angle);
              numeric_value
            },
          },
        ),
        None => ipred_z1_8bpc_inner(
          token,
          dst_bytes,
          edge_bytes,
          DirectionalPrediction {
            destination: PredictorOutput {
              base: dst_base_bytes,
              stride: byte_stride,
              size: Size {
                width: w,
                height: h,
              },
            },
            edge_center: topleft_off,
            angle: {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(angle);
              numeric_value
            },
          },
        ),
      },
      (BPC::BPC8, 7) => match avx512x_token {
        Some(t512x) => ipred_z2_8bpc_v4x_inner(
          t512x,
          dst_bytes,
          edge_bytes,
          DirectionalPrediction {
            destination: PredictorOutput {
              base: dst_base_bytes,
              stride: byte_stride,
              size: Size {
                width: w,
                height: h,
              },
            },
            edge_center: topleft_off,
            angle: {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(angle);
              numeric_value
            },
          },
          Size {
            width: max_width,
            height: max_height,
          },
        ),
        None => ipred_z2_8bpc_inner(
          token,
          dst_bytes,
          edge_bytes,
          DirectionalPrediction {
            destination: PredictorOutput {
              base: dst_base_bytes,
              stride: byte_stride,
              size: Size {
                width: w,
                height: h,
              },
            },
            edge_center: topleft_off,
            angle: {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(angle);
              numeric_value
            },
          },
          Size {
            width: max_width,
            height: max_height,
          },
        ),
      },
      (BPC::BPC8, 8) => match avx512x_token {
        Some(t512x) => ipred_z3_8bpc_v4x_inner(
          t512x,
          dst_bytes,
          edge_bytes,
          DirectionalPrediction {
            destination: PredictorOutput {
              base: dst_base_bytes,
              stride: byte_stride,
              size: Size {
                width: w,
                height: h,
              },
            },
            edge_center: topleft_off,
            angle: {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(angle);
              numeric_value
            },
          },
        ),
        None => ipred_z3_8bpc_inner(
          token,
          dst_bytes,
          edge_bytes,
          DirectionalPrediction {
            destination: PredictorOutput {
              base: dst_base_bytes,
              stride: byte_stride,
              size: Size {
                width: w,
                height: h,
              },
            },
            edge_center: topleft_off,
            angle: {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(angle);
              numeric_value
            },
          },
        ),
      },
      (BPC::BPC8, 9) => match avx512_token {
        Some(t512) => ipred_smooth_8bpc_avx512_inner(
          t512,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredSmooth8bpcAvx512InnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
        None => ipred_smooth_8bpc_inner(
          token,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredSmooth8bpcInnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
      },
      (BPC::BPC8, 10) => match avx512_token {
        Some(t512) => ipred_smooth_v_8bpc_avx512_inner(
          t512,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredSmoothV8bpcAvx512InnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
        None => ipred_smooth_v_8bpc_inner(
          token,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredSmoothV8bpcInnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
      },
      (BPC::BPC8, 11) => match avx512_token {
        Some(t512) => ipred_smooth_h_8bpc_avx512_inner(
          t512,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredSmoothH8bpcAvx512InnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
        None => ipred_smooth_h_8bpc_inner(
          token,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredSmoothH8bpcInnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
      },
      (BPC::BPC8, 12) => match avx512_token {
        Some(t512) => ipred_paeth_8bpc_avx512_inner(
          t512,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredPaeth8bpcAvx512InnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
        None => ipred_paeth_8bpc_inner(
          token,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredPaeth8bpcInnerArguments {
            tl_off: topleft_off,
            width: w,
            height: h,
          },
        ),
      },
      (BPC::BPC8, 13) => ipred_filter_8bpc_inner(
        token,
        dst_bytes,
        dst_base_bytes,
        byte_stride,
        edge_bytes,
        self::IpredFilter8bpcInnerArguments {
          tl_off: 0,
          width: w,
          height: h,
          filt_idx: {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(angle);
            numeric_value
          },
          topleft_off,
        },
      ),
      (BPC::BPC16, 0) => {
        let ::core::result::Result::Ok(tl_off_bytes) =
          ::console_core_number_conversion::fitted::<_, u64>(topleft_off.wrapping_mul(2));

        match avx512_token {
          Some(t512) => ipred_dc_16bpc_avx512_inner(
            t512,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredDc16bpcAvx512InnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
          None => ipred_dc_16bpc_inner(
            token,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredDc16bpcInnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
        }
      }
      (BPC::BPC16, 1) => {
        let ::core::result::Result::Ok(tl_off_bytes) =
          ::console_core_number_conversion::fitted::<_, u64>(topleft_off.wrapping_mul(2));

        match avx512_token {
          Some(t512) => ipred_v_16bpc_avx512_inner(
            t512,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredV16bpcAvx512InnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
          None => ipred_v_16bpc_inner(
            token,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredV16bpcInnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
        }
      }
      (BPC::BPC16, 2) => {
        let ::core::result::Result::Ok(tl_off_bytes) =
          ::console_core_number_conversion::fitted::<_, u64>(topleft_off.wrapping_mul(2));

        match avx512_token {
          Some(t512) => ipred_h_16bpc_avx512_inner(
            t512,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredH16bpcAvx512InnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
          None => ipred_h_16bpc_inner(
            token,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredH16bpcInnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
        }
      }
      (BPC::BPC16, 3) => {
        let ::core::result::Result::Ok(tl_off_bytes) =
          ::console_core_number_conversion::fitted::<_, u64>(topleft_off.wrapping_mul(2));

        match avx512_token {
          Some(t512) => ipred_dc_left_16bpc_avx512_inner(
            t512,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredDcLeft16bpcAvx512InnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
          None => ipred_dc_left_16bpc_inner(
            token,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredDcLeft16bpcInnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
        }
      }
      (BPC::BPC16, 4) => {
        let ::core::result::Result::Ok(tl_off_bytes) =
          ::console_core_number_conversion::fitted::<_, u64>(topleft_off.wrapping_mul(2));

        match avx512_token {
          Some(t512) => ipred_dc_top_16bpc_avx512_inner(
            t512,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredDcTop16bpcAvx512InnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
          None => ipred_dc_top_16bpc_inner(
            token,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredDcTop16bpcInnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
        }
      }
      (BPC::BPC16, 5) => match avx512_token {
        Some(t512) => ipred_dc_128_16bpc_avx512_inner(
          t512,
          dst_bytes,
          self::IpredDc12816bpcAvx512InnerArguments {
            dst_base: dst_base_bytes,
            stride: byte_stride,
            width: w,
            height: h,
          },
          {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(bd_c);
            numeric_value
          },
        ),
        None => ipred_dc_128_16bpc_inner(
          token,
          dst_bytes,
          self::IpredDc12816bpcInnerArguments {
            dst_base: dst_base_bytes,
            stride: byte_stride,
            width: w,
            height: h,
          },
          {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(bd_c);
            numeric_value
          },
        ),
      },
      (BPC::BPC16, 6) => {
        let ::core::result::Result::Ok(tl_off_bytes) =
          ::console_core_number_conversion::fitted::<_, u64>(topleft_off.wrapping_mul(2));
        ipred_z1_16bpc_inner(
          token,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredZ116bpcInnerArguments {
            tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
            width: w,
            height: h,
            angle: {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(angle);
              numeric_value
            },
            bitdepth_max: bd_c,
          },
        )
      }
      (BPC::BPC16, 7) => {
        let ::core::result::Result::Ok(tl_off_bytes) =
          ::console_core_number_conversion::fitted::<_, u64>(topleft_off.wrapping_mul(2));
        ipred_z2_16bpc_inner(
          token,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredZ216bpcInnerArguments {
            tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
            width: w,
            height: h,
            angle: {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(angle);
              numeric_value
            },
            max_width,
            max_height,
            bitdepth_max: bd_c,
          },
        )
      }
      (BPC::BPC16, 8) => {
        let ::core::result::Result::Ok(tl_off_bytes) =
          ::console_core_number_conversion::fitted::<_, u64>(topleft_off.wrapping_mul(2));
        ipred_z3_16bpc_inner(
          token,
          dst_bytes,
          dst_base_bytes,
          byte_stride,
          edge_bytes,
          self::IpredZ316bpcInnerArguments {
            tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
            width: w,
            height: h,
            angle: {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(angle);
              numeric_value
            },
            bitdepth_max: bd_c,
          },
        )
      }
      (BPC::BPC16, 9) => {
        let ::core::result::Result::Ok(tl_off_bytes) =
          ::console_core_number_conversion::fitted::<_, u64>(topleft_off.wrapping_mul(2));

        match avx512_token {
          Some(t512) => ipred_smooth_16bpc_avx512_inner(
            t512,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredSmooth16bpcAvx512InnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
          None => ipred_smooth_16bpc_inner(
            token,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredSmooth16bpcInnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
        }
      }
      (BPC::BPC16, 10) => {
        let ::core::result::Result::Ok(tl_off_bytes) =
          ::console_core_number_conversion::fitted::<_, u64>(topleft_off.wrapping_mul(2));

        match avx512_token {
          Some(t512) => ipred_smooth_v_16bpc_avx512_inner(
            t512,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredSmoothV16bpcAvx512InnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
          None => ipred_smooth_v_16bpc_inner(
            token,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredSmoothV16bpcInnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
        }
      }
      (BPC::BPC16, 11) => {
        let ::core::result::Result::Ok(tl_off_bytes) =
          ::console_core_number_conversion::fitted::<_, u64>(topleft_off.wrapping_mul(2));

        match avx512_token {
          Some(t512) => ipred_smooth_h_16bpc_avx512_inner(
            t512,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredSmoothH16bpcAvx512InnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
          None => ipred_smooth_h_16bpc_inner(
            token,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredSmoothH16bpcInnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
        }
      }
      (BPC::BPC16, 12) => {
        let ::core::result::Result::Ok(tl_off_bytes) =
          ::console_core_number_conversion::fitted::<_, u64>(topleft_off.wrapping_mul(2));

        match avx512_token {
          Some(t512) => ipred_paeth_16bpc_avx512_inner(
            t512,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredPaeth16bpcAvx512InnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
          None => ipred_paeth_16bpc_inner(
            token,
            dst_bytes,
            dst_base_bytes,
            byte_stride,
            edge_bytes,
            self::IpredPaeth16bpcInnerArguments {
              tl_off: match ::console_core_number_conversion::fitted::<_, u64>(tl_off_bytes) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
              width: w,
              height: h,
            },
          ),
        }
      }
      (BPC::BPC16, 13) => ipred_filter_16bpc_inner(
        token,
        dst_bytes,
        dst_base_bytes,
        byte_stride,
        edge_bytes,
        self::IpredFilter16bpcInnerArguments {
          tl_off: 0,
          width: w,
          height: h,
          filt_idx: {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(angle);
            numeric_value
          },
          bitdepth_max: {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(bd_c);
            numeric_value
          },
          topleft_off,
        },
      ),
      _ => Err(Rav1dError::EINVAL),
    },
  )?;

  with_block_mut.map(|()| PredictionDispatch::Applied)
}

#[cfg(target_arch = "x86_64")]
pub fn cfl_pred_dispatch<BD: BitDepth>(
  dst: &mut crate::src::owned_recon::ReconDst<'_>,
  ac: &[i16],
  projection: CflProjection,
  bd: BD,
) -> Rav1dResult<PredictionDispatch> {
  use crate::include::common::bitdepth::AsPrimitive;
  use crate::include::common::bitdepth::BPC;

  let Ok(summon_avx2) = crate::src::cpu::summon_avx2();
  let token = match summon_avx2 {
    Some(token) => token,
    None => {
      return Ok(PredictionDispatch::ScalarFallback);
    }
  };

  let CflProjection {
    size: Size { width, height },
    dc,
    alpha,
  } = projection;
  let Ok(w) = console_core_number_conversion::wrapped::<_, u64>(width);
  let Ok(h) = console_core_number_conversion::wrapped::<_, u64>(height);

  match ::core::primitive::bool::then_some(
    !(w == 4 || w == 8 || w == 16 || w == 32 || w == 64),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(PredictionDispatch::ScalarFallback);
    }
    ::core::option::Option::None => {}
  }

  let ac_slice = match w
    .checked_mul(h)
    .and_then(|count| predictor_get(ac, ..count))
  {
    Some(block) => block,
    None => return Ok(PredictionDispatch::ScalarFallback),
  };

  let predicted = dst.with_block_mut::<BD, _, _>(
    Size {
      width: w,
      height: h,
    },
    ac_slice,
    |dst_bytes, dst_base_bytes, byte_stride, ac_slice_2| match BD::BPC {
      BPC::BPC8 => cfl_pred_8bpc_inner(
        token,
        dst_bytes,
        dst_base_bytes,
        byte_stride,
        ac_slice_2,
        self::CflPred8bpcInnerArguments {
          width: w,
          height: h,
          dc,
          alpha,
        },
      ),
      BPC::BPC16 => cfl_pred_16bpc_inner(
        token,
        dst_bytes,
        dst_base_bytes,
        byte_stride,
        ac_slice_2,
        self::CflPred16bpcInnerArguments {
          width: w,
          height: h,
          dc,
          alpha,
          bitdepth_max: bd.bitdepth_max().as_::<i32>(),
        },
      ),
    },
  )?;
  predicted.map(|()| PredictionDispatch::Applied)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn cfl_ac_420_8bpc_inner(
  _token: Desktop64,
  ac: &mut [i16],
  CflAc4208bpcInnerArguments {
    width,
    height,
    active_w,
    active_h,
  }: CflAc4208bpcInnerArguments,
  src_bytes: &[u8],
  src_base: u64,
  src_stride: i64,
) -> Rav1dResult {
  let ones = _mm256_set1_epi8(1);

  for y in 0..active_h {
    let ::core::result::Result::Ok(aci) =
      ::console_core_number_conversion::fitted::<_, u64>((Wrapping(y) * Wrapping(width)).0);
    let ::core::result::Result::Ok(row1_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(src_base.cast_signed())
        + Wrapping(
          (Wrapping((Wrapping(2) * Wrapping(y)).0.cast_signed()) * Wrapping(src_stride)).0,
        ))
      .0
      .cast_unsigned(),
    );
    let ::core::result::Result::Ok(row2_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(src_base.cast_signed())
        + Wrapping(
          (Wrapping(
            (Wrapping((Wrapping(2) * Wrapping(y)).0) + Wrapping(1))
              .0
              .cast_signed(),
          ) * Wrapping(src_stride))
          .0,
        ))
      .0
      .cast_unsigned(),
    );

    let mut x = 0;

    for _ in 0..active_w.saturating_sub(x).wrapping_div(16) {
      let ::core::result::Result::Ok(lx) =
        ::console_core_number_conversion::fitted::<_, u64>((Wrapping(2) * Wrapping(x)).0);
      let checked_value = predictor_get(
        &*src_bytes,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(row1_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(lx) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ))
        .0
          ..(Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(row1_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(lx) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ))
            .0,
          ) + Wrapping(32))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let checked_value_16 = <&[u8; 32]>::try_from(checked_value)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      let r1 = loadu_256!(checked_value_16);
      let checked_value_2 = predictor_get(
        &*src_bytes,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(row2_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(lx) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ))
        .0
          ..(Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(row2_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(lx) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ))
            .0,
          ) + Wrapping(32))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let checked_value_17 = <&[u8; 32]>::try_from(checked_value_2)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      let r2 = loadu_256!(checked_value_17);
      let s1 = _mm256_maddubs_epi16(r1, ones);
      let s2 = _mm256_maddubs_epi16(r2, ones);
      let sum = _mm256_add_epi16(s1, s2);
      let shifted = _mm256_slli_epi16::<1>(sum);

      let checked_value_3 = predictor_get_mut(
        &mut *ac,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(aci) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(x))
        .0
          ..(Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(aci) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(x))
            .0,
          ) + Wrapping(16))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let checked_value_18 = <&mut [i16; 16]>::try_from(checked_value_3)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      storeu_256!(checked_value_18, shifted);
      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
    }

    for _ in 0..active_w.saturating_sub(x).wrapping_div(8) {
      let ::core::result::Result::Ok(lx) =
        ::console_core_number_conversion::fitted::<_, u64>((Wrapping(2) * Wrapping(x)).0);
      let checked_value_4 = predictor_get(
        &*src_bytes,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(row1_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(lx) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ))
        .0
          ..(Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(row1_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(lx) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ))
            .0,
          ) + Wrapping(16))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let checked_value_19 = <&[u8; 16]>::try_from(checked_value_4)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      let r1 = loadu_128!(checked_value_19);
      let checked_value_5 = predictor_get(
        &*src_bytes,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(row2_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(lx) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ))
        .0
          ..(Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(row2_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(lx) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ))
            .0,
          ) + Wrapping(16))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let checked_value_20 = <&[u8; 16]>::try_from(checked_value_5)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      let r2 = loadu_128!(checked_value_20);
      let ones128 = _mm_set1_epi8(1);
      let s1 = _mm_maddubs_epi16(r1, ones128);
      let s2 = _mm_maddubs_epi16(r2, ones128);
      let sum = _mm_add_epi16(s1, s2);
      let shifted = _mm_slli_epi16::<1>(sum);
      let checked_value_6 = predictor_get_mut(
        &mut *ac,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(aci) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(x))
        .0
          ..(Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(aci) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(x))
            .0,
          ) + Wrapping(8))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let checked_value_21 = <&mut [i16; 8]>::try_from(checked_value_6)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      storeu_128!(checked_value_21, shifted);
      let ::core::result::Result::Ok(()) = advance_predictor_position(8, &mut x);
    }

    for _ in (::core::ops::Range {
      start: x,
      end: active_w,
    }) {
      let ::core::result::Result::Ok(lx) =
        ::console_core_number_conversion::fitted::<_, u64>((Wrapping(2) * Wrapping(x)).0);
      let checked_value_7 = predictor_get(
        &*src_bytes,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(row1_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(lx) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ))
        .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let a = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_7);
      let checked_value_8 = predictor_get(
        &*src_bytes,
        (Wrapping(
          (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(row1_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(lx) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ))
          .0,
        ) + Wrapping(1))
        .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let b = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_8);
      let checked_value_9 = predictor_get(
        &*src_bytes,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(row2_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(lx) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ))
        .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let c = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_9);
      let checked_value_10 = predictor_get(
        &*src_bytes,
        (Wrapping(
          (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(row2_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(lx) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ))
          .0,
        ) + Wrapping(1))
        .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_10);
      *{
        let checked_value_25 = predictor_get_mut(
          &mut *ac,
          (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(aci) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping(x))
          .0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
        ::core::primitive::i32::wrapping_shl(
          ::core::primitive::i32::wrapping_add(
            ::core::primitive::i32::wrapping_add(::core::primitive::i32::wrapping_add(a, b), c),
            d,
          ),
          1_u32,
        ),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }

    match ::core::primitive::bool::then_some(active_w < width, ()) {
      ::core::option::Option::Some(()) => {
        let checked_value_11 = predictor_get(
          &*ac,
          (Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(aci) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(active_w))
            .0,
          ) - Wrapping(1))
          .0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        let pad = *checked_value_11;

        for x_2 in active_w..width {
          *{
            let checked_value_26 = predictor_get_mut(
              &mut *ac,
              (Wrapping(
                match ::console_core_number_conversion::fitted::<_, u64>(aci) {
                  ::core::result::Result::Ok(native_position) => native_position,
                },
              ) + Wrapping(x_2))
              .0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_26
          } = pad;
        }
      }
      ::core::option::Option::None => {}
    }
  }

  match ::core::primitive::bool::then_some(active_h < height, ()) {
    ::core::option::Option::Some(()) => {
      let src_row_start = (Wrapping((Wrapping(active_h) - Wrapping(1)).0) * Wrapping(width)).0;

      for y in active_h..height {
        let dst_off = (Wrapping(y) * Wrapping(width)).0;

        let Ok(row) = console_core_number_conversion::indices(
          src_row_start..src_row_start.wrapping_add(width),
        );
        let Ok(destination) = index(dst_off);
        ac.copy_within(row, destination);
      }
    }
    ::core::option::Option::None => {}
  }

  let n = (Wrapping(width) * Wrapping(height)).0;
  let log2sz = ::core::primitive::u32::cast_signed(u32::wrapping_add(
    width.trailing_zeros(),
    height.trailing_zeros(),
  ));
  let mut sum_i32 = ::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1i32, i32::cast_unsigned(log2sz)),
    1_u32,
  );

  {
    let mut acc = _mm256_setzero_si256();
    let mut i = 0;

    for _ in 0..n.saturating_sub(i).wrapping_div(16) {
      let checked_value_12 =
        predictor_get(&*ac, i..(Wrapping(i) + Wrapping(16)).0).ok_or(Rav1dError::EINVAL)?;
      let checked_value_22 = <&[i16; 16]>::try_from(checked_value_12)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      let v = loadu_256!(checked_value_22);

      let lo = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(v));
      let hi = _mm256_cvtepi16_epi32(_mm256_extracti128_si256::<1>(v));
      acc = _mm256_add_epi32(acc, lo);
      acc = _mm256_add_epi32(acc, hi);
      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut i);
    }

    let acc_lo = _mm256_castsi256_si128(acc);
    let acc_hi = _mm256_extracti128_si256::<1>(acc);
    let s128 = _mm_add_epi32(acc_lo, acc_hi);
    let s64 = _mm_add_epi32(s128, _mm_shuffle_epi32::<0b_01_00_11_10>(s128));
    let s32 = _mm_add_epi32(s64, _mm_shuffle_epi32::<0b_00_00_00_01>(s64));
    sum_i32 = sum_i32.wrapping_add(_mm_cvtsi128_si32(s32));

    for _ in (::core::ops::Range { start: i, end: n }) {
      sum_i32 = sum_i32.wrapping_add(
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(*{
          let checked_value_27 = predictor_get(&*ac, i).ok_or(Rav1dError::EINVAL)?;
          checked_value_27
        }),
      );
      i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
    }
  }

  let mean = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
    i32::wrapping_shr(sum_i32, i32::cast_unsigned(log2sz)),
  ) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };

  {
    let mean_v = _mm256_set1_epi16(mean);
    let mut i = 0;

    for _ in 0..n.saturating_sub(i).wrapping_div(16) {
      let checked_value_13 =
        predictor_get(&*ac, i..(Wrapping(i) + Wrapping(16)).0).ok_or(Rav1dError::EINVAL)?;
      let checked_value_23 = <&[i16; 16]>::try_from(checked_value_13)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      let v = loadu_256!(checked_value_23);
      let r = _mm256_sub_epi16(v, mean_v);
      let checked_value_14 = predictor_get_mut(&mut *ac, i..(Wrapping(i) + Wrapping(16)).0)
        .ok_or(Rav1dError::EINVAL)?;
      let checked_value_24 = <&mut [i16; 16]>::try_from(checked_value_14)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      storeu_256!(checked_value_24, r);
      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut i);
    }

    for _ in (::core::ops::Range { start: i, end: n }) {
      let checked_value_15 = predictor_get(&*ac, i).ok_or(Rav1dError::EINVAL)?;
      *{
        let checked_value_28 = predictor_get_mut(&mut *ac, i).ok_or(Rav1dError::EINVAL)?;
        checked_value_28
      } = (*checked_value_15).wrapping_sub(mean);
      i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct CflAc4208bpcInnerArguments {
  width: u64,
  height: u64,
  active_w: u64,
  active_h: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn cfl_ac_422_8bpc_inner(
  _token: Desktop64,
  ac: &mut [i16],
  CflAc4228bpcInnerArguments {
    width,
    height,
    active_w,
    active_h,
  }: CflAc4228bpcInnerArguments,
  src_bytes: &[u8],
  src_base: u64,
  src_stride: i64,
) -> Rav1dResult {
  let ones = _mm256_set1_epi8(1);

  for y in 0..active_h {
    let ::core::result::Result::Ok(aci) =
      ::console_core_number_conversion::fitted::<_, u64>((Wrapping(y) * Wrapping(width)).0);
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(src_base.cast_signed())
        + Wrapping((Wrapping(y.cast_signed()) * Wrapping(src_stride)).0))
      .0
      .cast_unsigned(),
    );

    let mut x = 0;

    for _ in 0..active_w.saturating_sub(x).wrapping_div(16) {
      let ::core::result::Result::Ok(lx) =
        ::console_core_number_conversion::fitted::<_, u64>((Wrapping(2) * Wrapping(x)).0);
      let checked_value = predictor_get(
        &*src_bytes,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(lx) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ))
        .0
          ..(Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(lx) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ))
            .0,
          ) + Wrapping(32))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let checked_value_12 = <&[u8; 32]>::try_from(checked_value)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      let r1 = loadu_256!(checked_value_12);
      let s1 = _mm256_maddubs_epi16(r1, ones);

      let shifted = _mm256_slli_epi16::<2>(s1);
      let checked_value_2 = predictor_get_mut(
        &mut *ac,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(aci) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(x))
        .0
          ..(Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(aci) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(x))
            .0,
          ) + Wrapping(16))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let checked_value_13 = <&mut [i16; 16]>::try_from(checked_value_2)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      storeu_256!(checked_value_13, shifted);
      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
    }

    for _ in 0..active_w.saturating_sub(x).wrapping_div(8) {
      let ::core::result::Result::Ok(lx) =
        ::console_core_number_conversion::fitted::<_, u64>((Wrapping(2) * Wrapping(x)).0);
      let checked_value_3 = predictor_get(
        &*src_bytes,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(lx) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ))
        .0
          ..(Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(lx) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ))
            .0,
          ) + Wrapping(16))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let checked_value_14 = <&[u8; 16]>::try_from(checked_value_3)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      let r1 = loadu_128!(checked_value_14);
      let ones128 = _mm_set1_epi8(1);
      let s1 = _mm_maddubs_epi16(r1, ones128);
      let shifted = _mm_slli_epi16::<2>(s1);
      let checked_value_4 = predictor_get_mut(
        &mut *ac,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(aci) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(x))
        .0
          ..(Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(aci) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(x))
            .0,
          ) + Wrapping(8))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let checked_value_15 = <&mut [i16; 8]>::try_from(checked_value_4)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      storeu_128!(checked_value_15, shifted);
      let ::core::result::Result::Ok(()) = advance_predictor_position(8, &mut x);
    }

    for _ in (::core::ops::Range {
      start: x,
      end: active_w,
    }) {
      let ::core::result::Result::Ok(lx) =
        ::console_core_number_conversion::fitted::<_, u64>((Wrapping(2) * Wrapping(x)).0);
      let checked_value_5 = predictor_get(
        &*src_bytes,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(lx) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ))
        .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let a = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
      let checked_value_6 = predictor_get(
        &*src_bytes,
        (Wrapping(
          (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(lx) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ))
          .0,
        ) + Wrapping(1))
        .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let b = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_6);
      *{
        let checked_value_25 = predictor_get_mut(
          &mut *ac,
          (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(aci) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping(x))
          .0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
        ::core::primitive::i32::wrapping_shl(::core::primitive::i32::wrapping_add(a, b), 2_u32),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }

    match ::core::primitive::bool::then_some(active_w < width, ()) {
      ::core::option::Option::Some(()) => {
        let checked_value_7 = predictor_get(
          &*ac,
          (Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(aci) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(active_w))
            .0,
          ) - Wrapping(1))
          .0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        let pad = *checked_value_7;

        for x_2 in active_w..width {
          *{
            let checked_value_26 = predictor_get_mut(
              &mut *ac,
              (Wrapping(
                match ::console_core_number_conversion::fitted::<_, u64>(aci) {
                  ::core::result::Result::Ok(native_position) => native_position,
                },
              ) + Wrapping(x_2))
              .0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_26
          } = pad;
        }
      }
      ::core::option::Option::None => {}
    }
  }

  match ::core::primitive::bool::then_some(active_h < height, ()) {
    ::core::option::Option::Some(()) => {
      let src_row_start = (Wrapping((Wrapping(active_h) - Wrapping(1)).0) * Wrapping(width)).0;

      for y in active_h..height {
        let dst_off = (Wrapping(y) * Wrapping(width)).0;
        let Ok(row) = console_core_number_conversion::indices(
          src_row_start..src_row_start.wrapping_add(width),
        );
        let Ok(destination) = index(dst_off);
        ac.copy_within(row, destination);
      }
    }
    ::core::option::Option::None => {}
  }

  let n = (Wrapping(width) * Wrapping(height)).0;
  let log2sz = ::core::primitive::u32::cast_signed(u32::wrapping_add(
    width.trailing_zeros(),
    height.trailing_zeros(),
  ));
  let mut sum_i32 = ::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1i32, i32::cast_unsigned(log2sz)),
    1_u32,
  );

  {
    let mut acc = _mm256_setzero_si256();
    let mut i = 0;

    for _ in 0..n.saturating_sub(i).wrapping_div(16) {
      let checked_value_8 =
        predictor_get(&*ac, i..(Wrapping(i) + Wrapping(16)).0).ok_or(Rav1dError::EINVAL)?;
      let checked_value_16 = <&[i16; 16]>::try_from(checked_value_8)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      let v = loadu_256!(checked_value_16);
      let lo = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(v));
      let hi = _mm256_cvtepi16_epi32(_mm256_extracti128_si256::<1>(v));
      acc = _mm256_add_epi32(acc, lo);
      acc = _mm256_add_epi32(acc, hi);
      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut i);
    }

    let acc_lo = _mm256_castsi256_si128(acc);
    let acc_hi = _mm256_extracti128_si256::<1>(acc);
    let s128 = _mm_add_epi32(acc_lo, acc_hi);
    let s64 = _mm_add_epi32(s128, _mm_shuffle_epi32::<0b_01_00_11_10>(s128));
    let s32 = _mm_add_epi32(s64, _mm_shuffle_epi32::<0b_00_00_00_01>(s64));
    sum_i32 = sum_i32.wrapping_add(_mm_cvtsi128_si32(s32));

    for _ in (::core::ops::Range { start: i, end: n }) {
      sum_i32 = sum_i32.wrapping_add(
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(*{
          let checked_value_27 = predictor_get(&*ac, i).ok_or(Rav1dError::EINVAL)?;
          checked_value_27
        }),
      );
      i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
    }
  }

  let mean = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
    i32::wrapping_shr(sum_i32, i32::cast_unsigned(log2sz)),
  ) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };

  {
    let mean_v = _mm256_set1_epi16(mean);
    let mut i = 0;

    for _ in 0..n.saturating_sub(i).wrapping_div(16) {
      let checked_value_9 =
        predictor_get(&*ac, i..(Wrapping(i) + Wrapping(16)).0).ok_or(Rav1dError::EINVAL)?;
      let checked_value_17 = <&[i16; 16]>::try_from(checked_value_9)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      let v = loadu_256!(checked_value_17);
      let r = _mm256_sub_epi16(v, mean_v);
      let checked_value_10 = predictor_get_mut(&mut *ac, i..(Wrapping(i) + Wrapping(16)).0)
        .ok_or(Rav1dError::EINVAL)?;
      let checked_value_18 = <&mut [i16; 16]>::try_from(checked_value_10)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      storeu_256!(checked_value_18, r);
      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut i);
    }

    for _ in (::core::ops::Range { start: i, end: n }) {
      let checked_value_11 = predictor_get(&*ac, i).ok_or(Rav1dError::EINVAL)?;
      *{
        let checked_value_28 = predictor_get_mut(&mut *ac, i).ok_or(Rav1dError::EINVAL)?;
        checked_value_28
      } = (*checked_value_11).wrapping_sub(mean);
      i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct CflAc4228bpcInnerArguments {
  width: u64,
  height: u64,
  active_w: u64,
  active_h: u64,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn cfl_ac_444_8bpc_inner(
  _token: Desktop64,
  ac: &mut [i16],
  CflAc4448bpcInnerArguments {
    width,
    height,
    active_w,
    active_h,
  }: CflAc4448bpcInnerArguments,
  src_bytes: &[u8],
  src_base: u64,
  src_stride: i64,
) -> Rav1dResult {
  for y in 0..active_h {
    let ::core::result::Result::Ok(aci) =
      ::console_core_number_conversion::fitted::<_, u64>((Wrapping(y) * Wrapping(width)).0);
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fitted::<_, u64>(
      (Wrapping(src_base.cast_signed())
        + Wrapping((Wrapping(y.cast_signed()) * Wrapping(src_stride)).0))
      .0
      .cast_unsigned(),
    );

    let mut x = 0;

    for _ in 0..active_w.saturating_sub(x).wrapping_div(16) {
      let checked_value = predictor_get(
        &*src_bytes,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(x))
        .0
          ..(Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(x))
            .0,
          ) + Wrapping(16))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let checked_value_10 = <&[u8; 16]>::try_from(checked_value)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      let r1 = loadu_128!(checked_value_10);

      let widened = _mm256_cvtepu8_epi16(r1);
      let shifted = _mm256_slli_epi16::<3>(widened);
      let checked_value_2 = predictor_get_mut(
        &mut *ac,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(aci) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(x))
        .0
          ..(Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(aci) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(x))
            .0,
          ) + Wrapping(16))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let checked_value_11 = <&mut [i16; 16]>::try_from(checked_value_2)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      storeu_256!(checked_value_11, shifted);
      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut x);
    }

    for _ in 0..active_w.saturating_sub(x).wrapping_div(8) {
      let arr: &[u8; 8] = (&*{
        let checked_value_25 = predictor_get(
          &*src_bytes,
          (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping(x))
          .0
            ..(Wrapping(
              (Wrapping(
                match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
                  ::core::result::Result::Ok(native_position) => native_position,
                },
              ) + Wrapping(x))
              .0,
            ) + Wrapping(8))
            .0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_25
      })
        .try_into()
        .map_err(|_| Rav1dError::EINVAL)?;

      let mut buf = [0u8; 16];
      let (lo, _) = buf.split_first_chunk_mut::<8>().ok_or(Rav1dError::EINVAL)?;
      *lo = *arr;
      let r1 = loadu_128!(&buf);
      let widened = _mm_cvtepu8_epi16(r1);
      let shifted = _mm_slli_epi16::<3>(widened);
      let checked_value_3 = predictor_get_mut(
        &mut *ac,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(aci) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(x))
        .0
          ..(Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(aci) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(x))
            .0,
          ) + Wrapping(8))
          .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let checked_value_12 = <&mut [i16; 8]>::try_from(checked_value_3)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      storeu_128!(checked_value_12, shifted);
      let ::core::result::Result::Ok(()) = advance_predictor_position(8, &mut x);
    }

    for _ in (::core::ops::Range {
      start: x,
      end: active_w,
    }) {
      let checked_value_4 = predictor_get(
        &*src_bytes,
        (Wrapping(
          match ::console_core_number_conversion::fitted::<_, u64>(row_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) + Wrapping(x))
        .0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      *{
        let checked_value_27 = predictor_get_mut(
          &mut *ac,
          (Wrapping(
            match ::console_core_number_conversion::fitted::<_, u64>(aci) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) + Wrapping(x))
          .0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        checked_value_27
      } = ::core::primitive::i16::wrapping_shl(
        <i16 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4),
        3_u32,
      );
      x = (::core::num::Wrapping(x) + ::core::num::Wrapping(1)).0;
    }

    match ::core::primitive::bool::then_some(active_w < width, ()) {
      ::core::option::Option::Some(()) => {
        let checked_value_5 = predictor_get(
          &*ac,
          (Wrapping(
            (Wrapping(
              match ::console_core_number_conversion::fitted::<_, u64>(aci) {
                ::core::result::Result::Ok(native_position) => native_position,
              },
            ) + Wrapping(active_w))
            .0,
          ) - Wrapping(1))
          .0,
        )
        .ok_or(Rav1dError::EINVAL)?;
        let pad = *checked_value_5;

        for x_2 in active_w..width {
          *{
            let checked_value_28 = predictor_get_mut(
              &mut *ac,
              (Wrapping(
                match ::console_core_number_conversion::fitted::<_, u64>(aci) {
                  ::core::result::Result::Ok(native_position) => native_position,
                },
              ) + Wrapping(x_2))
              .0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_28
          } = pad;
        }
      }
      ::core::option::Option::None => {}
    }
  }

  match ::core::primitive::bool::then_some(active_h < height, ()) {
    ::core::option::Option::Some(()) => {
      let src_row_start = (Wrapping((Wrapping(active_h) - Wrapping(1)).0) * Wrapping(width)).0;

      for y in active_h..height {
        let dst_off = (Wrapping(y) * Wrapping(width)).0;
        let Ok(row) = console_core_number_conversion::indices(
          src_row_start..src_row_start.wrapping_add(width),
        );
        let Ok(destination) = index(dst_off);
        ac.copy_within(row, destination);
      }
    }
    ::core::option::Option::None => {}
  }

  let n = (Wrapping(width) * Wrapping(height)).0;
  let log2sz = ::core::primitive::u32::cast_signed(u32::wrapping_add(
    width.trailing_zeros(),
    height.trailing_zeros(),
  ));
  let mut sum_i32 = ::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1i32, i32::cast_unsigned(log2sz)),
    1_u32,
  );

  {
    let mut acc = _mm256_setzero_si256();
    let mut i = 0;

    for _ in 0..n.saturating_sub(i).wrapping_div(16) {
      let checked_value_6 =
        predictor_get(&*ac, i..(Wrapping(i) + Wrapping(16)).0).ok_or(Rav1dError::EINVAL)?;
      let checked_value_13 = <&[i16; 16]>::try_from(checked_value_6)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      let v = loadu_256!(checked_value_13);
      let lo = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(v));
      let hi = _mm256_cvtepi16_epi32(_mm256_extracti128_si256::<1>(v));
      acc = _mm256_add_epi32(acc, lo);
      acc = _mm256_add_epi32(acc, hi);
      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut i);
    }

    let acc_lo = _mm256_castsi256_si128(acc);
    let acc_hi = _mm256_extracti128_si256::<1>(acc);
    let s128 = _mm_add_epi32(acc_lo, acc_hi);
    let s64 = _mm_add_epi32(s128, _mm_shuffle_epi32::<0b_01_00_11_10>(s128));
    let s32 = _mm_add_epi32(s64, _mm_shuffle_epi32::<0b_00_00_00_01>(s64));
    sum_i32 = sum_i32.wrapping_add(_mm_cvtsi128_si32(s32));

    for _ in (::core::ops::Range { start: i, end: n }) {
      sum_i32 = sum_i32.wrapping_add(
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(*{
          let checked_value_29 = predictor_get(&*ac, i).ok_or(Rav1dError::EINVAL)?;
          checked_value_29
        }),
      );
      i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
    }
  }

  let mean = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
    i32::wrapping_shr(sum_i32, i32::cast_unsigned(log2sz)),
  ) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };

  {
    let mean_v = _mm256_set1_epi16(mean);
    let mut i = 0;

    for _ in 0..n.saturating_sub(i).wrapping_div(16) {
      let checked_value_7 =
        predictor_get(&*ac, i..(Wrapping(i) + Wrapping(16)).0).ok_or(Rav1dError::EINVAL)?;
      let checked_value_14 = <&[i16; 16]>::try_from(checked_value_7)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      let v = loadu_256!(checked_value_14);
      let r = _mm256_sub_epi16(v, mean_v);
      let checked_value_8 = predictor_get_mut(&mut *ac, i..(Wrapping(i) + Wrapping(16)).0)
        .ok_or(Rav1dError::EINVAL)?;
      let checked_value_15 = <&mut [i16; 16]>::try_from(checked_value_8)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)?;
      storeu_256!(checked_value_15, r);
      let ::core::result::Result::Ok(()) = advance_predictor_position(16, &mut i);
    }

    for _ in (::core::ops::Range { start: i, end: n }) {
      let checked_value_9 = predictor_get(&*ac, i).ok_or(Rav1dError::EINVAL)?;
      *{
        let checked_value_30 = predictor_get_mut(&mut *ac, i).ok_or(Rav1dError::EINVAL)?;
        checked_value_30
      } = (*checked_value_9).wrapping_sub(mean);
      i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct CflAc4448bpcInnerArguments {
  width: u64,
  height: u64,
  active_w: u64,
  active_h: u64,
}

#[cfg(target_arch = "x86_64")]
pub fn cfl_ac_dispatch<BD: BitDepth>(
  ac: &mut [i16],
  y_src: &crate::src::owned_recon::ReconSrc<'_>,
  extraction: CflAcExtraction,
) -> Rav1dResult<PredictionDispatch> {
  use crate::include::common::bitdepth::BPC;

  match ::core::primitive::bool::then_some(BD::BPC != BPC::BPC8, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(PredictionDispatch::ScalarFallback);
    }
    ::core::option::Option::None => {}
  }

  let Ok(summon_avx2) = crate::src::cpu::summon_avx2();
  let token = match summon_avx2 {
    Some(token) => token,
    None => {
      return Ok(PredictionDispatch::ScalarFallback);
    }
  };

  let CflAcExtraction {
    size,
    padding,
    layout,
  } = extraction;
  let Ok(width) = console_core_number_conversion::wrapped::<_, u64>(size.width);
  let Ok(height) = console_core_number_conversion::wrapped::<_, u64>(size.height);
  let Size {
    width: w_pad,
    height: h_pad,
  } = padding;
  let (is_ss_hor, is_ss_ver) = match layout {
    Rav1dPixelLayoutSubSampled::I420 => (true, true),
    Rav1dPixelLayoutSubSampled::I422 => (true, false),
    Rav1dPixelLayoutSubSampled::I444 => (false, false),
  };
  let (column_units, row_units) = match (u32::try_from(w_pad), u32::try_from(h_pad)) {
    (Ok(columns), Ok(rows)) => (columns, rows),
    (_horizontal, _vertical) => return Ok(PredictionDispatch::ScalarFallback),
  };
  let Ok(padded_columns) =
    console_core_number_conversion::fitted::<_, u64>(u64::from(column_units).wrapping_mul(4));
  let Ok(padded_rows) =
    console_core_number_conversion::fitted::<_, u64>(u64::from(row_units).wrapping_mul(4));

  match (padded_columns < width && padded_rows < height).then_some(()) {
    Some(()) => {}
    None => return Ok(PredictionDispatch::ScalarFallback),
  }

  let active_w = width.wrapping_sub(padded_columns);
  let active_h = height.wrapping_sub(padded_rows);
  let ::core::result::Result::Ok(ss_hor) = ::console_core_number_conversion::fitted::<
    ::core::primitive::u8,
    u64,
  >(::core::primitive::u8::from(is_ss_hor));
  let ::core::result::Result::Ok(ss_ver) = ::console_core_number_conversion::fitted::<
    ::core::primitive::u8,
    u64,
  >(::core::primitive::u8::from(is_ss_ver));
  let src_w = active_w.wrapping_shl({
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
    numeric_value
  });
  let src_h = active_h.wrapping_shl({
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
    numeric_value
  });

  let count = match width.checked_mul(height) {
    Some(area) => area,
    None => return Ok(PredictionDispatch::ScalarFallback),
  };
  let ac_block = match predictor_get_mut(ac, ..count) {
    Some(block) => block,
    None => return Ok(PredictionDispatch::ScalarFallback),
  };

  let Ok(predicted) = y_src.with_block_context::<BD, _, _>(
    Size {
      width: src_w,
      height: src_h,
    },
    ac_block,
    |output, src_bytes, src_base, src_stride_i| match ::core::primitive::bool::then_some(
      is_ss_hor && is_ss_ver,
      (),
    ) {
      ::core::option::Option::Some(()) => cfl_ac_420_8bpc_inner(
        token,
        output,
        self::CflAc4208bpcInnerArguments {
          width,
          height,
          active_w,
          active_h,
        },
        src_bytes,
        src_base,
        src_stride_i,
      ),
      ::core::option::Option::None => {
        match ::core::primitive::bool::then_some(is_ss_hor && !is_ss_ver, ()) {
          ::core::option::Option::Some(()) => cfl_ac_422_8bpc_inner(
            token,
            output,
            self::CflAc4228bpcInnerArguments {
              width,
              height,
              active_w,
              active_h,
            },
            src_bytes,
            src_base,
            src_stride_i,
          ),
          ::core::option::Option::None => cfl_ac_444_8bpc_inner(
            token,
            output,
            self::CflAc4448bpcInnerArguments {
              width,
              height,
              active_w,
              active_h,
            },
            src_bytes,
            src_base,
            src_stride_i,
          ),
        }
      }
    },
  );
  predicted.map(|()| PredictionDispatch::Applied)
}

#[cfg(all(test, target_arch = "x86_64"))]
mod v4x_dir_tests {
  use super::*;

  fn make_topleft() -> core::result::Result<(Vec<u8>, u64), console_core_never::Never> {
    Ok({
      let tl_off = 200u64;
      let mut buf = vec![0u8; 512];
      let mut st: u32 = 0x1234_5678;

      for b in buf.iter_mut() {
        st ^= u32::wrapping_shl(st, 13_u32);
        st ^= u32::wrapping_shr(st, 17_u32);
        st ^= u32::wrapping_shl(st, 5_u32);
        *b = {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, u8>(u32::wrapping_shr(st, 3_u32));
          numeric_value
        };
      }

      (buf, tl_off)
    })
  }

  fn run_z1(
    RunZ1Arguments { w, h }: RunZ1Arguments,
    angle: i32,
  ) -> Rav1dResult<(Vec<u8>, Vec<u8>)> {
    Ok({
      let Ok((tl, tl_off)) = make_topleft();
      let stride = 64i64;
      let mut dst_a = vec![7u8; 4096];
      let mut dst_b = vec![7u8; 4096];
      let Ok(summon_avx2) = crate::src::cpu::summon_avx2();
      let t3 = summon_avx2.ok_or(Rav1dError::EINVAL)?;
      let Ok(summon_avx512x) = crate::src::cpu::summon_avx512x();
      let t4x = summon_avx512x.ok_or(Rav1dError::EINVAL)?;
      assert_eq!(
        ipred_z1_8bpc_inner(
          t3,
          &mut dst_a,
          &tl,
          DirectionalPrediction {
            destination: PredictorOutput {
              base: 0,
              stride: stride,
              size: Size {
                width: w,
                height: h
              },
            },
            edge_center: tl_off,
            angle: angle,
          },
        ),
        Ok(())
      );
      assert_eq!(
        ipred_z1_8bpc_v4x_inner(
          t4x,
          &mut dst_b,
          &tl,
          DirectionalPrediction {
            destination: PredictorOutput {
              base: 0,
              stride: stride,
              size: Size {
                width: w,
                height: h
              },
            },
            edge_center: tl_off,
            angle: angle,
          },
        ),
        Ok(())
      );
      (dst_a, dst_b)
    })
  }

  struct RunZ1Arguments {
    w: u64,
    h: u64,
  }

  fn run_z3(
    RunZ3Arguments { w, h }: RunZ3Arguments,
    angle: i32,
  ) -> Rav1dResult<(Vec<u8>, Vec<u8>)> {
    Ok({
      let Ok((tl, tl_off)) = make_topleft();
      let stride = 64i64;
      let mut dst_a = vec![7u8; 4096];
      let mut dst_b = vec![7u8; 4096];
      let Ok(summon_avx2) = crate::src::cpu::summon_avx2();
      let t3 = summon_avx2.ok_or(Rav1dError::EINVAL)?;
      let Ok(summon_avx512x) = crate::src::cpu::summon_avx512x();
      let t4x = summon_avx512x.ok_or(Rav1dError::EINVAL)?;
      assert_eq!(
        ipred_z3_8bpc_inner(
          t3,
          &mut dst_a,
          &tl,
          DirectionalPrediction {
            destination: PredictorOutput {
              base: 0,
              stride: stride,
              size: Size {
                width: w,
                height: h
              },
            },
            edge_center: tl_off,
            angle: angle,
          },
        ),
        Ok(())
      );
      assert_eq!(
        ipred_z3_8bpc_v4x_inner(
          t4x,
          &mut dst_b,
          &tl,
          DirectionalPrediction {
            destination: PredictorOutput {
              base: 0,
              stride: stride,
              size: Size {
                width: w,
                height: h
              },
            },
            edge_center: tl_off,
            angle: angle,
          },
        ),
        Ok(())
      );
      (dst_a, dst_b)
    })
  }

  struct RunZ3Arguments {
    w: u64,
    h: u64,
  }

  fn run_z2(
    RunZ2Arguments {
      w,
      h,
      angle,
      mw,
      mh,
    }: RunZ2Arguments,
  ) -> Rav1dResult<Option<(Vec<u8>, Vec<u8>)>> {
    Ok({
      let Ok((tl, tl_off)) = make_topleft();
      let stride = 64i64;
      let mut dst_a = vec![7u8; 4096];
      let mut dst_b = vec![7u8; 4096];
      let Ok(summon_avx2) = crate::src::cpu::summon_avx2();
      let t3 = summon_avx2.ok_or(Rav1dError::EINVAL)?;
      let Ok(summon_avx512x) = crate::src::cpu::summon_avx512x();
      let t4x = summon_avx512x.ok_or(Rav1dError::EINVAL)?;
      let baseline = ipred_z2_8bpc_inner(
        t3,
        &mut dst_a,
        &tl,
        DirectionalPrediction {
          destination: PredictorOutput {
            base: 0,
            stride: stride,
            size: Size {
              width: w,
              height: h,
            },
          },
          edge_center: tl_off,
          angle: angle,
        },
        Size {
          width: mw,
          height: mh,
        },
      );

      match baseline {
        Ok(()) => {}
        Err(Rav1dError::EINVAL) => return Ok(None),
        Err(error) => return Err(error),
      }

      assert_eq!(
        ipred_z2_8bpc_v4x_inner(
          t4x,
          &mut dst_b,
          &tl,
          DirectionalPrediction {
            destination: PredictorOutput {
              base: 0,
              stride: stride,
              size: Size {
                width: w,
                height: h
              },
            },
            edge_center: tl_off,
            angle: angle,
          },
          Size {
            width: mw,
            height: mh
          },
        ),
        Ok(())
      );
      Some((dst_a, dst_b))
    })
  }

  struct RunZ2Arguments {
    w: u64,
    h: u64,
    angle: i32,
    mw: i32,
    mh: i32,
  }

  fn assert_block_eq(
    a: &[u8],
    b: &[u8],
    AssertBlockEqArguments { w, h, stride }: AssertBlockEqArguments,
    label: &str,
  ) -> Rav1dResult {
    {
      for y in 0..h {
        for x in 0..w {
          let ::core::result::Result::Ok(off) = ::console_core_number_conversion::fitted::<_, u64>(
            y.wrapping_mul(stride).wrapping_add(x),
          );

          let actual = predictor_get(a, off).ok_or(Rav1dError::EINVAL)?;
          let expected = predictor_get(b, off).ok_or(Rav1dError::EINVAL)?;

          assert_eq!(actual, expected, "{label}: mismatch at ({x},{y})");
        }
      }
    };

    Ok(())
  }

  struct AssertBlockEqArguments {
    w: u64,
    h: u64,
    stride: u64,
  }

  const DIRECTIONS: [i32; 3] = [
    0i32,
    i32::wrapping_shl(1, 10_u32),
    i32::wrapping_shl(1, 10_u32) | i32::wrapping_shl(1, 9_u32),
  ];
  const SHAPES: [(u64, u64); 15] = [
    (4, 4),
    (4, 8),
    (8, 4),
    (8, 8),
    (8, 16),
    (16, 8),
    (16, 16),
    (16, 32),
    (32, 16),
    (32, 32),
    (32, 64),
    (64, 32),
    (64, 64),
    (4, 16),
    (16, 4),
  ];

  #[test]
  fn z1_v4x_matches_avx2() -> Rav1dResult {
    let _tok_lock = archmage::testing::lock_token_testing();

    let Ok(summon_avx512x) = crate::src::cpu::summon_avx512x();

    match summon_avx512x {
      ::core::option::Option::None => {
        eprintln!("z1_v4x_matches_avx2: X64V4xToken unavailable, skipping (AVX2 path used)");
        return Ok(());
      }
      ::core::option::Option::Some(_) => {}
    }

    let base_angles = [3, 6, 14, 22, 30, 36, 44, 52, 60, 66, 74, 82, 86];

    for &(w, h) in &SHAPES {
      for &ba in &base_angles {
        for &fl in &DIRECTIONS {
          let angle = ba | fl;
          let (a, b) = run_z1(self::RunZ1Arguments { w, h }, angle)?;
          assert_block_eq(
            &a,
            &b,
            self::AssertBlockEqArguments { w, h, stride: 64 },
            &format!("z1 w={w} h={h} angle={angle}"),
          )?;
        }
      }
    }

    Ok(())
  }

  #[test]
  fn z3_v4x_matches_avx2() -> Rav1dResult {
    let _tok_lock = archmage::testing::lock_token_testing();

    let Ok(summon_avx512x) = crate::src::cpu::summon_avx512x();

    match summon_avx512x {
      ::core::option::Option::None => {
        eprintln!("z3_v4x_matches_avx2: X64V4xToken unavailable, skipping (AVX2 path used)");
        return Ok(());
      }
      ::core::option::Option::Some(_) => {}
    }

    let base_angles = [184, 190, 198, 206, 214, 222, 230, 238, 246, 254, 262, 266];

    for &(w, h) in &SHAPES {
      for &ba in &base_angles {
        for &fl in &DIRECTIONS {
          let angle = ba | fl;
          let (a, b) = run_z3(self::RunZ3Arguments { w, h }, angle)?;
          assert_block_eq(
            &a,
            &b,
            self::AssertBlockEqArguments { w, h, stride: 64 },
            &format!("z3 w={w} h={h} angle={angle}"),
          )?;
        }
      }
    }

    Ok(())
  }

  #[test]
  fn z2_v4x_matches_avx2() -> Rav1dResult {
    let _tok_lock = archmage::testing::lock_token_testing();

    let Ok(summon_avx512x) = crate::src::cpu::summon_avx512x();

    match summon_avx512x {
      ::core::option::Option::None => {
        eprintln!("z2_v4x_matches_avx2: X64V4xToken unavailable, skipping (AVX2 path used)");
        return Ok(());
      }
      ::core::option::Option::Some(_) => {}
    }

    let base_angles = [94, 100, 108, 116, 124, 132, 140, 148, 156, 164, 172, 176];
    let mut compared = 0u64;

    for &(w, h) in &SHAPES {
      for &ba in &base_angles {
        'inner_for_3: for &fl in &DIRECTIONS {
          let angle = ba | fl;

          let Ok(mw) = console_core_number_conversion::wrapped::<_, i32>(w);
          let Ok(mh) = console_core_number_conversion::wrapped::<_, i32>(h);
          let case = run_z2(RunZ2Arguments {
            w,
            h,
            angle,
            mw,
            mh,
          })?;
          let (a, b) = match case {
            Some(samples) => samples,
            None => continue 'inner_for_3,
          };
          assert_block_eq(
            &a,
            &b,
            self::AssertBlockEqArguments { w, h, stride: 64 },
            &format!("z2 w={w} h={h} angle={angle} mw={mw} mh={mh}"),
          )?;

          let ::core::result::Result::Ok(()) = advance_predictor_position(1, &mut compared);
        }
      }
    }

    eprintln!("z2_v4x compared {compared} configs");
    assert!(
      compared >= 100,
      "z2 test compared too few configs: {compared}"
    );
    Ok(())
  }
}
