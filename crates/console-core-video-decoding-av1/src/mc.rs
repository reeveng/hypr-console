#![forbid(unsafe_code)]
use crate::include::common::bitdepth::AsPrimitive;
use crate::include::common::bitdepth::BitDepth;
use crate::include::common::intops::iclip;
use crate::include::dav1d::headers::Rav1dFilterMode;
use crate::include::dav1d::headers::Rav1dPixelLayoutSubSampled;
use crate::include::dav1d::picture::PicOffset;
use crate::include::dav1d::picture::Rav1dPictureDataComponent;
use crate::src::align::AlignedVec64;
use crate::src::cpu::CpuFlags;
use crate::src::enum_map::DefaultValue;
use crate::src::enum_map::EnumMap;
use crate::src::enum_map::enum_map_ty;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use crate::src::internal::CompoundPrediction;
use crate::src::internal::EmulatedEdge;
use crate::src::internal::InterIntraPrediction;
use crate::src::internal::MAX_COMPOUND_SAMPLES;
use crate::src::internal::OverlapPrediction;
use crate::src::internal::SegmentMask;
use crate::src::levels::{Filter2d, MaskPolarity};
use crate::src::pic_or_buf::PicOrBuf;
use crate::src::safe_simd::mc::BlendAxis;
use crate::src::strided::Strided as _;
use crate::src::tables::dav1d_mc_subpel_filters;
use crate::src::tables::dav1d_mc_warp_filter;
use crate::src::tables::dav1d_obmc_masks;
use crate::src::tables::dav1d_resize_filter;
use crate::src::with_offset::WithOffset;
use crate::src::wrap_fn_ptr::wrap_fn_ptr;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_pixel_buffers::RowLayout;
use console_core_video_interpolation::{convolve8, round_shift};
use std::cmp;
use std::iter;

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the compile-time intermediate row is a Rust array of exactly 128 samples"
  )
)]
const MID_STRIDE: usize = 128;

fn filter_8tap_mid<const WIDTH: usize>(
  mid: &[[i16; WIDTH]],
  column: u32,
  coefficients: &[i8; 8],
) -> Rav1dResult<i32> {
  let Ok(position) = console_core_number_conversion::index(column);
  let rows = mid.first_chunk::<8>().ok_or(EINVAL)?;

  rows
    .iter()
    .zip(coefficients)
    .try_fold(0_i32, |sum, (row, &coefficient)| {
      let &sample = row.get(position).ok_or(EINVAL)?;
      let weighted = i32::from(sample).wrapping_mul(i32::from(coefficient));
      Ok(sum.wrapping_add(weighted))
    })
}

fn filter_8tap<BD: BitDepth>(
  source: PicOffset,
  column: u32,
  coefficients: &[i8; 8],
  stride: i64,
) -> Result<i32, Never> {
  let Ok(position) = console_core_number_conversion::index(column);
  let source_column = source + position;
  Ok(convolve8(coefficients, |tap| {
    let Ok(ordinal) = console_core_number_conversion::fixed_index(tap);
    let Ok(signed) = console_core_number_conversion::wrapped::<_, i64>(ordinal);
    let Ok(offset) =
      console_core_number_conversion::wrapped_offset(signed.wrapping_sub(3).wrapping_mul(stride));
    (*(source_column + offset).index::<BD>()).as_::<i32>()
  }))
}

pub(super) struct FilterExtent {
  pub(super) phase: u32,
  pub(super) samples: u32,
}

pub(super) fn fractional_filter(
  FilterExtent { phase, samples }: FilterExtent,
  filter_type: Rav1dFilterMode,
) -> Result<Option<&'static [i8; 8]>, Never> {
  use Rav1dFilterMode::{Bilinear, Regular8Tap, Sharp8Tap, Smooth8Tap, Switchable};

  let index = match phase.checked_sub(1) {
    Some(fraction) => fraction,
    None => return Ok(None),
  };
  let [
    regular,
    smooth,
    sharp,
    narrow_regular,
    narrow_smooth,
    _unused,
  ] = &*dav1d_mc_subpel_filters;
  let table = match (samples, filter_type) {
    (0..=4, Regular8Tap | Sharp8Tap | Switchable) | (5.., Bilinear) => narrow_regular,
    (0..=4, Smooth8Tap | Bilinear) | (5.., Switchable) => narrow_smooth,
    (5.., Regular8Tap) => regular,
    (5.., Smooth8Tap) => smooth,
    (5.., Sharp8Tap) => sharp,
  };
  let Ok(position) = console_core_number_conversion::index(index);

  Ok(table.get(position))
}

#[cfg(target_arch = "aarch64")]
#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  expect(
    explicit024_no_swappable_params,
    reason = "the SIMD entry uses its established native phase and block-extent order before building a fixed-width filter extent"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "SIMD kernels retain native slice offsets at this adapter, which immediately converts them to fixed filter coordinates"
  )
)]
pub(super) fn get_filter(
  phase: console_core_api_compatibility::NativeSize,
  samples: console_core_api_compatibility::NativeSize,
  filter_type: Rav1dFilterMode,
) -> Result<Option<&'static [i8; 8]>, Never> {
  let Ok(fixed_phase) = console_core_number_conversion::fitted::<_, u32>(phase);
  let Ok(fixed_samples) = console_core_number_conversion::fitted::<_, u32>(samples);
  fractional_filter(
    FilterExtent {
      phase: fixed_phase,
      samples: fixed_samples,
    },
    filter_type,
  )
}

fn bilinear_sum([first, second]: [i32; 2], fraction: u32) -> Result<i32, Never> {
  let Ok(factor) = console_core_number_conversion::wrapped::<_, i32>(fraction);
  Ok(
    first
      .wrapping_mul(16)
      .wrapping_add(factor.wrapping_mul(second.wrapping_sub(first))),
  )
}

fn filter_bilin_mid(mid: &[[i16; MID_STRIDE]], position: SubpixelPosition) -> Rav1dResult<i32> {
  let Ok(column) = console_core_number_conversion::index(position.index);
  let [first, second] = mid.first_chunk::<2>().ok_or(EINVAL)?;
  let &a = first.get(column).ok_or(EINVAL)?;
  let &b = second.get(column).ok_or(EINVAL)?;
  let Ok(sum) = bilinear_sum([i32::from(a), i32::from(b)], position.phase);

  Ok(sum)
}

fn filter_bilin<BD: BitDepth>(
  source: PicOffset,
  position: SubpixelPosition,
  stride: i64,
) -> Result<i32, Never> {
  let Ok(column) = console_core_number_conversion::index(position.index);
  let Ok(offset) = console_core_number_conversion::wrapped_offset(stride);
  let first_sample = source + column;
  let second_sample = first_sample + offset;
  let first = (*first_sample.index::<BD>()).as_::<i32>();
  let second = (*second_sample.index::<BD>()).as_::<i32>();
  bilinear_sum([first, second], position.phase)
}

#[derive(Clone, Copy)]
pub(crate) struct UnscaledSampling {
  pub size: Size<i32>,
  pub phase: Point<i32>,
}

#[derive(Clone, Copy)]
enum SubpixelKernel {
  Identity,
  Bilinear(u8),
  EightTap(&'static [i8; 8]),
}

impl SubpixelKernel {
  #[must_use]
  fn support(self) -> Result<core::ops::Range<i64>, Never> {
    Ok(match self {
      Self::Identity => 0..1,
      Self::Bilinear(_) => 0..2,
      Self::EightTap(_) => -3..5,
    })
  }

  #[must_use]
  fn sample<BD: BitDepth>(
    self,
    source: PicOffset,
    column: u32,
    stride: i64,
  ) -> Result<i32, Never> {
    match self {
      Self::Identity => {
        let Ok(position) = console_core_number_conversion::index(column);
        Ok((*(source + position).index::<BD>()).as_::<i32>())
      }
      Self::Bilinear(fraction) => filter_bilin::<BD>(
        source,
        SubpixelPosition {
          index: column,
          phase: u32::from(fraction),
        },
        stride,
      ),
      Self::EightTap(taps) => filter_8tap::<BD>(source, column, taps, stride),
    }
  }

  fn intermediate(self, rows: &[[i16; MID_STRIDE]], column: u32) -> Rav1dResult<i32> {
    match self {
      Self::Bilinear(fraction) => filter_bilin_mid(
        rows,
        SubpixelPosition {
          index: column,
          phase: u32::from(fraction),
        },
      ),
      Self::EightTap(taps) => filter_8tap_mid(rows, column, taps),
      Self::Identity => Err(EINVAL),
    }
  }
}

#[derive(Clone, Copy)]
enum SubpixelRounding {
  Copy,
  HorizontalBilinear,
  HorizontalEightTap,
  Vertical(u8),
  Separable(u8),
}

impl SubpixelRounding {
  #[must_use]
  fn pixel<BD: BitDepth>(
    self,
    sample: i32,
    precision: u8,
    depth: BD,
  ) -> Result<BD::Pixel, Never> {
    let shift = u32::from(precision);
    let value = match self {
      Self::Copy => return Ok(sample.as_::<BD::Pixel>()),
      Self::HorizontalBilinear => {
        round_shift(round_shift(sample, 4_u32.wrapping_sub(shift)), shift)
      }
      Self::HorizontalEightTap => sample
        .wrapping_add(32)
        .wrapping_add(1_i32.wrapping_shl(5_u32.wrapping_sub(shift)))
        .wrapping_shr(6),
      Self::Vertical(bits) => round_shift(sample, u32::from(bits)),
      Self::Separable(bits) => round_shift(
        sample,
        ::core::primitive::u32::wrapping_add(u32::from(bits), shift),
      ),
    };
    Ok(depth.iclip_pixel(value))
  }

  #[must_use]
  fn predictor<BD: BitDepth>(self, sample: i32, precision: u8) -> Result<i16, Never> {
    let shift = u32::from(precision);
    let value = match self {
      Self::Copy => sample.wrapping_shl(shift),
      Self::HorizontalBilinear => round_shift(sample, 4_u32.wrapping_sub(shift)),
      Self::HorizontalEightTap => round_shift(sample, 6_u32.wrapping_sub(shift)),
      Self::Vertical(bits) => round_shift(sample, u32::from(bits).wrapping_sub(shift)),
      Self::Separable(bits) => round_shift(sample, u32::from(bits)),
    };
    Ok(BD::sub_prep_bias(value))
  }
}

struct UnscaledBlock {
  size: Size<u32>,
  horizontal: SubpixelKernel,
  vertical: SubpixelKernel,
  rounding: SubpixelRounding,
}

impl UnscaledBlock {
  fn new(filter: Filter2d, sampling: UnscaledSampling) -> Rav1dResult<Self> {
    let width = u32::try_from(sampling.size.width).map_err(|_| EINVAL)?;
    let height = u32::try_from(sampling.size.height).map_err(|_| EINVAL)?;
    let phase_x = u8::try_from(sampling.phase.x).map_err(|_| EINVAL)?;
    let phase_y = u8::try_from(sampling.phase.y).map_err(|_| EINVAL)?;

    match {
      ::core::primitive::bool::then_some(
        !matches!(
          (width, height, phase_x, phase_y),
          (1..=128, 1..=128, 0..=15, 0..=15)
        ),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        return Err(EINVAL);
      }
      ::core::option::Option::None => {}
    }

    let size = Size { width, height };
    let (horizontal, vertical, filter_bits) = match filter {
      Filter2d::Bilinear => {
        let kernel = |phase| match phase {
          0 => SubpixelKernel::Identity,
          _ => SubpixelKernel::Bilinear(phase),
        };
        (kernel(phase_x), kernel(phase_y), 4)
      }
      Filter2d::Regular8Tap
      | Filter2d::RegularSmooth8Tap
      | Filter2d::RegularSharp8Tap
      | Filter2d::SharpRegular8Tap
      | Filter2d::SharpSmooth8Tap
      | Filter2d::Sharp8Tap
      | Filter2d::SmoothRegular8Tap
      | Filter2d::Smooth8Tap
      | Filter2d::SmoothSharp8Tap => {
        let Ok((horizontal, vertical)) = filter.hv();
        let kernel = |phase: u8, extent: u32, mode| {
          let filter_extent = FilterExtent {
            phase: u32::from(phase),
            samples: extent,
          };
          let Ok(coefficients) = fractional_filter(filter_extent, mode);
          coefficients.map_or(SubpixelKernel::Identity, SubpixelKernel::EightTap)
        };
        (
          kernel(phase_x, width, horizontal),
          kernel(phase_y, height, vertical),
          6,
        )
      }
    };
    use SubpixelKernel::{Bilinear, EightTap, Identity};
    let rounding = match (horizontal, vertical) {
      (Identity, Identity) => SubpixelRounding::Copy,
      (Bilinear(_), Identity) => SubpixelRounding::HorizontalBilinear,
      (EightTap(_), Identity) => SubpixelRounding::HorizontalEightTap,
      (Identity, Bilinear(_) | EightTap(_)) => SubpixelRounding::Vertical(filter_bits),
      (Bilinear(_) | EightTap(_), Bilinear(_) | EightTap(_)) => {
        SubpixelRounding::Separable(filter_bits)
      }
    };

    Ok(Self {
      size,
      horizontal,
      vertical,
      rounding,
    })
  }

  fn picture_layout<BD: BitDepth>(
    &self,
    picture: PicOffset,
    support: Point<core::ops::Range<i64>>,
  ) -> Rav1dResult {
    let Ok(pitch) =
      console_core_number_conversion::fixed_offset(match picture.pixel_stride::<BD>() {
        Ok(stride) => stride,
      });
    let Ok(base) = console_core_number_conversion::fixed_index(picture.offset);
    let displacement = support
      .y
      .start
      .checked_mul(pitch)
      .and_then(|rows| rows.checked_add(support.x.start))
      .ok_or(EINVAL)?;
    let first = base.checked_add_signed(displacement).ok_or(EINVAL)?;
    let extra_columns = support.x.end.wrapping_sub(support.x.start).wrapping_sub(1);
    let extra_rows = support.y.end.wrapping_sub(support.y.start).wrapping_sub(1);
    let size = Size {
      width: u64::from(self.size.width).wrapping_add(extra_columns.cast_unsigned()),
      height: u64::from(self.size.height).wrapping_add(extra_rows.cast_unsigned()),
    };
    let layout = RowLayout::new(size, first, pitch).map_err(|_| EINVAL)?;
    layout
      .validate_len(picture.data.pixel_len::<BD>())
      .map_err(|_| EINVAL)
  }

  fn interpolate<BD: BitDepth, Output>(
    &self,
    source: PicOffset,
    precision: u8,
    output: &mut Output,
    mut write_row: impl FnMut(&mut Output, &[i32]) -> Rav1dResult,
  ) -> Rav1dResult {
    let Ok(x) = self.horizontal.support();
    let Ok(y) = self.vertical.support();
    let support = Point { x, y };
    self.picture_layout::<BD>(source, support.clone())?;
    let Ok(pitch) =
      console_core_number_conversion::fixed_offset(match source.pixel_stride::<BD>() {
        Ok(stride) => stride,
      });
    let Ok(source_stride) = console_core_number_conversion::wrapped_offset(pitch);
    let Ok(width) = console_core_number_conversion::index(self.size.width);
    let Ok(height) = console_core_number_conversion::index(self.size.height);
    let mut filtered = [0_i32; MID_STRIDE];
    let pixels = filtered.get_mut(..width).ok_or(EINVAL)?;

    match self.rounding {
      SubpixelRounding::Separable(bits) => {
        let Ok(top) = console_core_number_conversion::wrapped_offset(support.y.start);
        let extra_rows = support
          .y
          .end
          .wrapping_sub(support.y.start)
          .wrapping_sub(1)
          .cast_unsigned();
        let Ok(extra_rows_2) = console_core_number_conversion::index(extra_rows);
        let mut storage = [[0_i16; MID_STRIDE]; 135];
        let intermediate = storage
          .get_mut(..height.wrapping_add(extra_rows_2))
          .ok_or(EINVAL)?;
        let shift = u32::from(bits.wrapping_sub(precision));
        let mut source_row = source + top.wrapping_mul(source_stride);

        for row in intermediate.iter_mut() {
          for (column, pixel) in (0_u32..self.size.width).zip(row.iter_mut()) {
            let Ok(sum) = self.horizontal.sample::<BD>(source_row, column, 1);
            let Ok(value) =
              console_core_number_conversion::wrapped::<_, i16>(round_shift(sum, shift));
            *pixel = value;
          }

          source_row += source_stride;
        }

        for index in 0..height {
          let rows = intermediate.get(index..).ok_or(EINVAL)?;

          for (column, pixel) in (0_u32..self.size.width).zip(pixels.iter_mut()) {
            let checked_value = self.vertical.intermediate(rows, column)?;
            *pixel = checked_value;
          }

          write_row(output, pixels)?;
        }

        return Ok(());
      }
      SubpixelRounding::Copy
      | SubpixelRounding::HorizontalBilinear
      | SubpixelRounding::HorizontalEightTap
      | SubpixelRounding::Vertical(_) => {}
    }

    let mut source_row = source;

    for _ in 0..height {
      match (self.horizontal, self.vertical) {
        (SubpixelKernel::Identity, SubpixelKernel::Identity) => {
          let input = source_row.slice::<BD>(width);

          for (sample, &pixel) in pixels.iter_mut().zip(input.iter()) {
            *sample = pixel.as_::<i32>();
          }
        }
        (SubpixelKernel::Identity, vertical) => {
          for (column, pixel) in (0_u32..self.size.width).zip(pixels.iter_mut()) {
            let Ok(sample) = vertical.sample::<BD>(source_row, column, pitch);
            *pixel = sample;
          }
        }
        (horizontal, SubpixelKernel::Identity) => {
          for (column, pixel) in (0_u32..self.size.width).zip(pixels.iter_mut()) {
            let Ok(sample) = horizontal.sample::<BD>(source_row, column, 1);
            *pixel = sample;
          }
        }
        (
          SubpixelKernel::Bilinear(_) | SubpixelKernel::EightTap(_),
          SubpixelKernel::Bilinear(_) | SubpixelKernel::EightTap(_),
        ) => return Err(EINVAL),
      }

      write_row(output, pixels)?;
      source_row += source_stride;
    }

    Ok(())
  }
}

fn put_unscaled<BD: BitDepth>(
  block: &UnscaledBlock,
  mut output: PicOffset,
  source: PicOffset,
  bit_depth: BD,
) -> Rav1dResult {
  block.picture_layout::<BD>(output, Point { x: 0..1, y: 0..1 })?;
  let Ok(stride) =
    console_core_number_conversion::fixed_offset(match output.pixel_stride::<BD>() {
      Ok(samples) => samples,
    });
  let precision = bit_depth.get_intermediate_bits();
  block.interpolate::<BD, _>(source, precision, &mut output, |destination, samples| {
    for (pixel, &sample) in destination
      .slice_mut::<BD>(samples.len())
      .iter_mut()
      .zip(samples)
    {
      let Ok(output_pixel) = block.rounding.pixel(sample, precision, bit_depth);
      *pixel = output_pixel;
    }

    let Ok(row_step) = console_core_number_conversion::wrapped_offset(stride);
    *destination += row_step;
    Ok(())
  })
}

fn prep_unscaled<BD: BitDepth>(
  block: &UnscaledBlock,
  output: &mut [i16],
  source: PicOffset,
  bit_depth: BD,
) -> Rav1dResult {
  let extent = Size {
    width: u64::from(block.size.width),
    height: u64::from(block.size.height),
  };
  let layout = RowLayout::new(extent, 0, i64::from(block.size.width)).map_err(|_| EINVAL)?;
  let mut rows = layout.rows_mut(output).map_err(|_| EINVAL)?;
  let precision = bit_depth.get_intermediate_bits();
  block.interpolate::<BD, _>(source, precision, &mut rows, |destination, samples| {
    let row = destination.next().ok_or(EINVAL)?;

    for (pixel, &sample) in row.iter_mut().zip(samples) {
      let Ok(prediction) = block.rounding.predictor::<BD>(sample, precision);
      *pixel = prediction;
    }

    Ok(())
  })
}

type PredictorPair<'a> = [&'a CompoundPrediction; 2];

#[derive(Clone, Copy)]
pub(crate) enum PredictorBlend<'a> {
  Average,
  Weighted(i32),
  Mask(&'a [u8]),
}

fn predictor_blend_size<BD: BitDepth>(
  dst: PicOffset,
  extent: Size<i32>,
  blend: PredictorBlend<'_>,
) -> Rav1dResult<Option<Size<u64>>> {
  let width = u64::try_from(extent.width).map_err(|_| EINVAL)?;
  let height = u64::try_from(extent.height).map_err(|_| EINVAL)?;

  match (width, height) {
    (0, _) | (_, 0) => return Ok(None),
    _ => {}
  }

  let size = Size { width, height };
  let area = width.checked_mul(height).ok_or(EINVAL)?;
  let capacity = u64::from(MAX_COMPOUND_SAMPLES);

  let usable = match blend {
    PredictorBlend::Mask(weights) => {
      let Ok(available) = console_core_number_conversion::fixed_index(weights.len());
      capacity.min(available)
    }
    PredictorBlend::Average | PredictorBlend::Weighted(_) => capacity,
  };

  match area.cmp(&usable) {
    cmp::Ordering::Greater => return Err(EINVAL),
    cmp::Ordering::Less | cmp::Ordering::Equal => {}
  }

  let Ok(origin) = console_core_number_conversion::fixed_index(dst.offset);
  let Ok(pitch) = console_core_number_conversion::fixed_offset(match dst.pixel_stride::<BD>() {
    Ok(samples) => samples,
  });
  let layout = RowLayout::new(size, origin, pitch).map_err(|_| EINVAL)?;
  layout
    .validate_len(dst.data.pixel_len::<BD>())
    .map_err(|_| EINVAL)?;
  Ok(Some(size))
}

fn blend_predictors<BD: BitDepth>(
  dst: PicOffset,
  predictions: PredictorPair<'_>,
  extent: Size<i32>,
  blend: PredictorBlend<'_>,
  depth: BD,
) -> Rav1dResult {
  let validated = predictor_blend_size::<BD>(dst, extent, blend)?;
  let size = match validated {
    Some(size) => size,
    None => return Ok(()),
  };

  #[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "wasm32"
  ))]
  let [first, second] = predictions;

  #[cfg(target_arch = "x86_64")]
  {
    let accelerated = match blend {
      PredictorBlend::Average => {
        crate::src::safe_simd::mc::avg_dispatch::<BD>(dst, first, second, extent, depth)
      }
      PredictorBlend::Weighted(weight) => {
        crate::src::safe_simd::mc::w_avg_dispatch::<BD>(dst, first, second, extent, weight, depth)
      }
      PredictorBlend::Mask(mask) => {
        crate::src::safe_simd::mc::mask_dispatch::<BD>(dst, first, second, extent, mask, depth)
      }
    };

    match accelerated {
      Ok(()) => Ok(()),
      Err(_scalar_fallback) => scalar_predictor_blend(dst, predictions, size, blend, depth),
    }
  }

  #[cfg(target_arch = "aarch64")]
  match crate::src::safe_simd::mc_arm::compound_blend_dispatch::<BD>(
    dst,
    [first, second],
    extent,
    blend,
    depth,
  ) {
    Ok(()) => Ok(()),
    Err(_scalar_fallback) => scalar_predictor_blend(dst, predictions, size, blend, depth),
  }

  #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
  {
    #[cfg(target_arch = "wasm32")]
    use crate::src::safe_simd::mc_wasm as simd;
    #[cfg(target_arch = "wasm32")]
    let accelerated = match blend {
      PredictorBlend::Average => {
        simd::avg_dispatch::<BD>(dst, first, second, extent.width, extent.height, depth)
      }
      PredictorBlend::Weighted(weight) => simd::w_avg_dispatch::<BD>(
        dst,
        first,
        second,
        extent.width,
        extent.height,
        weight,
        depth,
      ),
      PredictorBlend::Mask(mask) => {
        simd::mask_dispatch::<BD>(dst, first, second, extent.width, extent.height, mask, depth)
      }
    };
    #[cfg(not(target_arch = "wasm32"))]
    let accelerated = false;

    match ::core::primitive::bool::then_some(accelerated, ()) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => {
        scalar_predictor_blend(dst, predictions, size, blend, depth)
      }
    }
  }
}

fn mc_put_direct<BD: BitDepth>(
  filter: Filter2d,
  dst: PicOffset,
  src: PicOffset,
  sampling: UnscaledSampling,
  bit_depth: BD,
) -> Rav1dResult {
  let block = UnscaledBlock::new(filter, sampling)?;

  #[cfg(target_arch = "x86_64")]
  match crate::src::safe_simd::mc::mc_put_dispatch::<BD>(
    filter,
    dst,
    src,
    sampling.size,
    sampling.phase,
    bit_depth,
  ) {
    Ok(()) => Ok(()),
    Err(_scalar_fallback) => put_unscaled(&block, dst, src, bit_depth),
  }

  #[cfg(not(target_arch = "x86_64"))]
  {
    #[cfg(target_arch = "aarch64")]
    {
      let completed = crate::src::safe_simd::mc_arm::mc_put_dispatch::<BD>(
        filter,
        dst,
        src,
        sampling.size.width,
        sampling.size.height,
        sampling.phase.x,
        sampling.phase.y,
        bit_depth,
      );

      match completed.then_some(()) {
        Some(()) => return Ok(()),
        None => {}
      }
    }

    put_unscaled(&block, dst, src, bit_depth)
  }
}

fn mct_prep_direct<BD: BitDepth>(
  filter: Filter2d,
  tmp: &mut [i16],
  src: PicOffset,
  sampling: UnscaledSampling,
  bit_depth: BD,
) -> Rav1dResult {
  let block = UnscaledBlock::new(filter, sampling)?;
  let Ok(required) =
    console_core_number_conversion::index(block.size.width.wrapping_mul(block.size.height));
  let predictors = tmp.get_mut(..required).ok_or(EINVAL)?;

  #[cfg(target_arch = "x86_64")]
  match crate::src::safe_simd::mc::mct_prep_dispatch::<BD>(
    filter,
    predictors,
    src,
    sampling.size,
    sampling.phase,
    bit_depth,
  ) {
    Ok(()) => Ok(()),
    Err(_scalar_fallback) => prep_unscaled(&block, predictors, src, bit_depth),
  }

  #[cfg(not(target_arch = "x86_64"))]
  {
    #[cfg(target_arch = "aarch64")]
    {
      let completed = crate::src::safe_simd::mc_arm::mct_prep_dispatch::<BD>(
        filter,
        predictors,
        src,
        sampling.size.width,
        sampling.size.height,
        sampling.phase.x,
        sampling.phase.y,
        bit_depth,
      );

      match completed.then_some(()) {
        Some(()) => return Ok(()),
        None => {}
      }
    }

    prep_unscaled(&block, predictors, src, bit_depth)
  }
}

#[cfg(all(test, target_arch = "aarch64"))]
pub(crate) fn prep_unscaled_reference<BD: BitDepth>(
  filter: Filter2d,
  predictors: &mut [i16],
  source: PicOffset,
  sampling: UnscaledSampling,
  depth: BD,
) -> Rav1dResult {
  let block = UnscaledBlock::new(filter, sampling)?;
  prep_unscaled(&block, predictors, source, depth)
}

#[derive(Clone, Copy)]
pub(crate) struct ScaledSampling {
  pub size: Size<i32>,
  pub phase: Point<i32>,
  pub step: Point<i32>,
}

#[derive(Clone, Copy)]
struct ScaledAxis {
  count: u32,
  initial: u32,
  step: u32,
  last: u32,
}

struct AxisSampling {
  count: i32,
  phase: i32,
  step: i32,
}

impl ScaledAxis {
  fn new(
    AxisSampling {
      count,
      phase: initial,
      step,
    }: AxisSampling,
  ) -> Rav1dResult<Self> {
    let count_u32 = u32::try_from(count).map_err(|_negative_count| EINVAL)?;
    let phase = u32::try_from(initial).map_err(|_negative_phase| EINVAL)?;
    let shift = u32::try_from(step).map_err(|_negative_step| EINVAL)?;

    match (count_u32, phase) {
      (1..=128, 0..=1023) => {}
      _invalid_extent_or_phase => return Err(EINVAL),
    }

    let last = count_u32
      .wrapping_sub(1)
      .checked_mul(shift)
      .and_then(|advance| advance.checked_add(phase))
      .ok_or(EINVAL)?;

    Ok(Self {
      count: count_u32,
      initial: phase,
      step: shift,
      last: ::core::primitive::u32::wrapping_shr(last, 10_u32),
    })
  }

  #[must_use]
  fn positions(self) -> Result<impl Iterator<Item = SubpixelPosition>, Never> {
    Ok((0..self.count).map(move |distance| {
      let coordinate = self.initial.wrapping_add(distance.wrapping_mul(self.step));
      SubpixelPosition {
        index: ::core::primitive::u32::wrapping_shr(coordinate, 10_u32),
        phase: ::core::primitive::u32::wrapping_shr(coordinate & 1023, 6_u32),
      }
    }))
  }
}

#[derive(Clone, Copy)]
struct SubpixelPosition {
  index: u32,
  phase: u32,
}

#[derive(Clone, Copy)]
enum ScaledFilter {
  Bilinear,
  EightTap {
    horizontal: Rav1dFilterMode,
    vertical: Rav1dFilterMode,
  },
}

impl ScaledFilter {
  fn new(filter: Filter2d) -> Result<Self, Never> {
    Ok(match filter {
      Filter2d::Bilinear => Self::Bilinear,
      coefficients @ (Filter2d::Regular8Tap
      | Filter2d::RegularSmooth8Tap
      | Filter2d::RegularSharp8Tap
      | Filter2d::SharpRegular8Tap
      | Filter2d::SharpSmooth8Tap
      | Filter2d::Sharp8Tap
      | Filter2d::SmoothRegular8Tap
      | Filter2d::Smooth8Tap
      | Filter2d::SmoothSharp8Tap) => {
        let Ok((horizontal, vertical)) = coefficients.hv();
        Self::EightTap {
          horizontal,
          vertical,
        }
      }
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "both scaled interpolation paths use a fixed Rust array of 263 intermediate rows"
  )
)]
const SCALED_INTERMEDIATE_ROWS: usize = 263;

struct ScaledBlock {
  horizontal: ScaledAxis,
  vertical: ScaledAxis,
  filter: ScaledFilter,
  row_count: u32,
  top_margin: i64,
}

impl ScaledBlock {
  fn new(sampling: ScaledSampling, filter: ScaledFilter) -> Rav1dResult<Self> {
    let horizontal = ScaledAxis::new(AxisSampling {
      count: sampling.size.width,
      phase: sampling.phase.x,
      step: sampling.step.x,
    })?;
    let vertical = ScaledAxis::new(AxisSampling {
      count: sampling.size.height,
      phase: sampling.phase.y,
      step: sampling.step.y,
    })?;
    let (extra_rows, top_margin, limit) = match filter {
      ScaledFilter::Bilinear => (2, 0, 257),
      ScaledFilter::EightTap { .. } => (8, -3, 263),
    };
    let row_count = vertical.last.checked_add(extra_rows).ok_or(EINVAL)?;

    match row_count.cmp(&limit) {
      cmp::Ordering::Less | cmp::Ordering::Equal => {}
      cmp::Ordering::Greater => return Err(EINVAL),
    }

    Ok(Self {
      horizontal,
      vertical,
      filter,
      row_count,
      top_margin,
    })
  }

  fn horizontal_span(&self) -> Result<core::ops::Range<i64>, Never> {
    Ok(match self.filter {
      ScaledFilter::Bilinear => 0..i64::from(self.horizontal.last).wrapping_add(2),
      ScaledFilter::EightTap { .. } => {
        let mut span = 0..0;

        let Ok(positions) = self.horizontal.positions();

        for position in positions {
          let location = i64::from(position.index);
          let required = match position.phase {
            0 => location..location.wrapping_add(1),
            _filtered => location.wrapping_sub(3)..location.wrapping_add(5),
          };
          span.start = span.start.min(required.start);
          span.end = span.end.max(required.end);
        }

        span
      }
    })
  }

  fn input_layout<BD: BitDepth>(&self, source: PicOffset) -> Rav1dResult {
    let Ok(pitch) =
      console_core_number_conversion::fixed_offset(match source.pixel_stride::<BD>() {
        Ok(stride) => stride,
      });
    let Ok(base) = console_core_number_conversion::fixed_index(source.offset);
    let Ok(columns) = self.horizontal_span();
    let delta = self
      .top_margin
      .checked_mul(pitch)
      .and_then(|row| row.checked_add(columns.start))
      .ok_or(EINVAL)?;
    let first_sample = base.checked_add_signed(delta).ok_or(EINVAL)?;
    let width = columns.end.wrapping_sub(columns.start).cast_unsigned();
    let size = Size {
      width,
      height: u64::from(self.row_count),
    };
    let layout = RowLayout::new(size, first_sample, pitch).map_err(|_invalid_layout| EINVAL)?;

    layout
      .validate_len(source.data.pixel_len::<BD>())
      .map_err(|_short_reference| EINVAL)
  }

  fn output_layout<BD: BitDepth>(&self, output: PicOffset) -> Rav1dResult {
    let Ok(pitch) =
      console_core_number_conversion::fixed_offset(match output.pixel_stride::<BD>() {
        Ok(stride) => stride,
      });
    let Ok(first_sample) = console_core_number_conversion::fixed_index(output.offset);
    let size = Size {
      width: u64::from(self.horizontal.count),
      height: u64::from(self.vertical.count),
    };
    let layout = RowLayout::new(size, first_sample, pitch).map_err(|_invalid_layout| EINVAL)?;

    layout
      .validate_len(output.data.pixel_len::<BD>())
      .map_err(|_short_output| EINVAL)
  }
}

fn fill_scaled_intermediate<BD: BitDepth>(
  block: &ScaledBlock,
  source: PicOffset,
  intermediate_bits: u8,
  mid: &mut [[i16; MID_STRIDE]; SCALED_INTERMEDIATE_ROWS],
) -> Rav1dResult {
  block.input_layout::<BD>(source)?;
  let Ok(pitch) =
    console_core_number_conversion::fixed_offset(match source.pixel_stride::<BD>() {
      Ok(samples) => samples,
    });
  let Ok(row_stride) = console_core_number_conversion::wrapped_offset(pitch);
  let top = console_core_number_conversion::checked_offset(block.top_margin)
    .map_err(|_outside_row_offset| EINVAL)?;
  let mut source_row = source + top.wrapping_mul(row_stride);
  let rounding = match block.filter {
    ScaledFilter::Bilinear => 4_u8.wrapping_sub(intermediate_bits),
    ScaledFilter::EightTap { .. } => 6_u8.wrapping_sub(intermediate_bits),
  };
  let scale = ScaledRounding {
    intermediate_bits,
    shift: rounding,
  };

  for (_row_number, row) in (0..block.row_count).zip(mid) {
    let Ok(positions) = block.horizontal.positions();

    for (pixel, position) in row.iter_mut().zip(positions) {
      let Ok(sample) = scaled_horizontal_sample::<BD>(block, source_row, position, scale);
      *pixel = sample;
    }

    source_row += row_stride;
  }

  Ok(())
}

#[derive(Clone, Copy)]
struct ScaledRounding {
  intermediate_bits: u8,
  shift: u8,
}

fn scaled_horizontal_sample<BD: BitDepth>(
  block: &ScaledBlock,
  source: PicOffset,
  position: SubpixelPosition,
  scale: ScaledRounding,
) -> Result<i16, Never> {
  let Ok(sum) = match block.filter {
    ScaledFilter::Bilinear => filter_bilin::<BD>(source, position, 1),
    ScaledFilter::EightTap { horizontal, .. } => {
      let extent = FilterExtent {
        phase: position.phase,
        samples: block.horizontal.count,
      };
      let Ok(coefficients) = fractional_filter(extent, horizontal);

      match coefficients {
        Some(taps) => filter_8tap::<BD>(source, position.index, taps, 1),
        None => {
          let Ok(column) = console_core_number_conversion::index(position.index);
          let pixel = (*(source + column).index::<BD>()).as_::<i16>();
          return Ok(pixel.wrapping_shl(u32::from(scale.intermediate_bits)));
        }
      }
    }
  };

  console_core_number_conversion::wrapped::<_, i16>(round_shift(sum, u32::from(scale.shift)))
}

fn write_scaled_row<Sample>(
  block: &ScaledBlock,
  mid: &[[i16; MID_STRIDE]],
  position: SubpixelPosition,
  output: &mut [Sample],
  store: impl Fn(&mut Sample, i32, u8),
) -> Rav1dResult {
  let Ok(index) = console_core_number_conversion::index(position.index);
  let rows = mid.get(index..).ok_or(EINVAL)?;

  match block.filter {
    ScaledFilter::Bilinear => {
      let window = rows.get(..2).ok_or(EINVAL)?;

      for (column, pixel) in (0_u32..).zip(output) {
        let sum = filter_bilin_mid(
          window,
          SubpixelPosition {
            index: column,
            phase: position.phase,
          },
        )?;
        store(pixel, sum, 4);
      }
    }
    ScaledFilter::EightTap { vertical, .. } => {
      let filter_extent = FilterExtent {
        phase: position.phase,
        samples: block.vertical.count,
      };
      let Ok(coefficients) = fractional_filter(filter_extent, vertical);

      match coefficients {
        Some(taps) => {
          let window = rows.get(..8).ok_or(EINVAL)?;

          for (column, pixel) in (0_u32..).zip(output) {
            let sum = filter_8tap_mid(window, column, taps)?;
            store(pixel, sum, 6);
          }
        }
        None => {
          let center = rows.get(3).ok_or(EINVAL)?;

          for (pixel, &sample) in output.iter_mut().zip(center) {
            store(pixel, i32::from(sample), 0);
          }
        }
      }
    }
  }

  Ok(())
}

fn put_scaled<BD: BitDepth>(
  filter: Filter2d,
  mut dst: PicOffset,
  src: PicOffset,
  sampling: ScaledSampling,
  bit_depth: BD,
) -> Rav1dResult {
  let Ok(wide_filter) = ScaledFilter::new(filter);
  let block = ScaledBlock::new(sampling, wide_filter)?;
  block.output_layout::<BD>(dst)?;
  let mut mid = [[0; MID_STRIDE]; SCALED_INTERMEDIATE_ROWS];
  let intermediate_bits = bit_depth.get_intermediate_bits();
  fill_scaled_intermediate::<BD>(&block, src, intermediate_bits, &mut mid)?;
  let Ok(pitch) = console_core_number_conversion::fixed_offset(match dst.pixel_stride::<BD>() {
    Ok(samples) => samples,
  });
  let Ok(stride) = console_core_number_conversion::wrapped_offset(pitch);
  let Ok(width) = console_core_number_conversion::index(block.horizontal.count);
  let Ok(positions) = block.vertical.positions();

  for position in positions {
    let mut row = dst.slice_mut::<BD>(width);
    write_scaled_row(&block, &mid, position, &mut row, |pixel, sample, shift| {
      let amount = u32::from(shift.wrapping_add(intermediate_bits));
      let rounded = round_shift(sample, amount);
      *pixel = bit_depth.iclip_pixel(rounded);
    })?;
    drop(row);
    dst += stride;
  }

  Ok(())
}

fn prep_scaled<BD: BitDepth>(
  filter: Filter2d,
  output: &mut [i16],
  src: PicOffset,
  sampling: ScaledSampling,
  bit_depth: BD,
) -> Rav1dResult {
  let Ok(wide_filter) = ScaledFilter::new(filter);
  let block = ScaledBlock::new(sampling, wide_filter)?;
  let extent = Size {
    width: u64::from(block.horizontal.count),
    height: u64::from(block.vertical.count),
  };
  let layout =
    RowLayout::new(extent, 0, i64::from(block.horizontal.count)).map_err(|_| EINVAL)?;
  let rows = layout.rows_mut(output).map_err(|_| EINVAL)?;
  let mut mid = [[0; MID_STRIDE]; SCALED_INTERMEDIATE_ROWS];
  fill_scaled_intermediate::<BD>(&block, src, bit_depth.get_intermediate_bits(), &mut mid)?;
  let Ok(positions) = block.vertical.positions();

  for (position, row) in positions.zip(rows) {
    write_scaled_row(&block, &mid, position, row, |pixel, sample, shift| {
      let rounded = round_shift(sample, u32::from(shift));
      *pixel = BD::sub_prep_bias(rounded);
    })?;
  }

  Ok(())
}

enum BlockBlendInput<'a, Pixel> {
  Pixels {
    source: &'a InterIntraPrediction<Pixel>,
    mask: &'a [u8],
  },
  Overlap {
    source: &'a OverlapPrediction<Pixel>,
    axis: BlendAxis,
  },
}

fn blend_block_direct<BD: BitDepth>(
  dst: PicOffset,
  extent: Size<i32>,
  blend: BlockBlendInput<'_, BD::Pixel>,
) -> Rav1dResult {
  let (prediction, weights): (&[BD::Pixel], BlockBlendMask<'_>) = match blend {
    BlockBlendInput::Pixels { source, mask } => (source, BlockBlendMask::Pixels(mask)),
    BlockBlendInput::Overlap { source, axis } => (source, BlockBlendMask::Overlap(axis)),
  };
  let prepared = BlockBlend::new::<BD>(dst, extent, weights, prediction)?;

  let plan = match prepared {
    Some(plan) => plan,
    None => return Ok(()),
  };

  #[cfg(target_arch = "x86_64")]
  let accelerated = match blend {
    BlockBlendInput::Pixels { source, mask } => {
      crate::src::safe_simd::mc::blend_dispatch::<BD>(dst, source, extent, mask)
    }
    BlockBlendInput::Overlap { source, axis } => {
      crate::src::safe_simd::mc::blend_dir_dispatch::<BD>(axis, dst, source, extent)
    }
  };

  #[cfg(target_arch = "x86_64")]
  match accelerated {
    Ok(()) => Ok(()),
    Err(_scalar_fallback) => plan.apply::<BD>(dst, prediction),
  }

  #[cfg(not(target_arch = "x86_64"))]
  plan.apply::<BD>(dst, prediction)
}

fn w_mask_direct<BD: BitDepth>(
  layout: Rav1dPixelLayoutSubSampled,
  dst: PicOffset,
  predictions: (&CompoundPrediction, &CompoundPrediction),
  extent: Size<i32>,
  mask: &mut SegmentMask,
  polarity: MaskPolarity,
  bd: BD,
) -> Rav1dResult {
  let plan = compound_mask_plan::<BD>(&dst, extent, layout, polarity)?;

  #[cfg(target_arch = "x86_64")]
  match crate::src::safe_simd::mc::w_mask_dispatch::<BD>(
    layout,
    dst,
    predictions.0,
    predictions.1,
    extent,
    mask,
    i32::from(polarity),
    bd,
  ) {
    Ok(()) => Ok(()),
    Err(_scalar_fallback) => w_mask_rust(dst, predictions, mask, plan, bd),
  }

  #[cfg(not(target_arch = "x86_64"))]
  {
    #[cfg(target_arch = "aarch64")]
    match crate::src::safe_simd::mc_arm::w_mask_dispatch::<BD>(
      layout,
      dst,
      predictions.0,
      predictions.1,
      extent.width,
      extent.height,
      mask,
      i32::from(polarity),
      bd,
    )
    .then_some(())
    {
      Some(()) => return Ok(()),
      None => {}
    }

    w_mask_rust(dst, predictions, mask, plan, bd)
  }
}

fn warp8x8_direct<BD: BitDepth>(
  dst: PicOffset,
  src: PicOffset,
  sampling: WarpSampling,
  depth: BD,
) -> Rav1dResult {
  #[cfg(target_arch = "x86_64")]
  match crate::src::safe_simd::mc::warp8x8_dispatch::<BD>(dst, src, sampling, depth) {
    Ok(()) => Ok(()),
    Err(_scalar_fallback) => warp_affine_8x8_rust(dst, src, sampling, depth),
  }

  #[cfg(not(target_arch = "x86_64"))]
  {
    warp_affine_8x8_rust(dst, src, sampling, depth)
  }
}

fn warp8x8t_direct<BD: BitDepth>(
  output: &mut [i16],
  stride: u64,
  src: PicOffset,
  sampling: WarpSampling,
  depth: BD,
) -> Rav1dResult {
  #[cfg(target_arch = "x86_64")]
  {
    match crate::src::safe_simd::mc::warp8x8t_dispatch::<BD>(output, stride, src, sampling, depth)
    {
      Ok(()) => Ok(()),
      Err(_scalar_fallback) => warp_affine_8x8t_rust(output, stride, src, sampling, depth),
    }
  }

  #[cfg(not(target_arch = "x86_64"))]
  warp_affine_8x8t_rust(output, stride, src, sampling, depth)
}

fn scalar_predictor_blend<BD: BitDepth>(
  dst: PicOffset,
  predictions: PredictorPair<'_>,
  size: Size<u64>,
  kind: PredictorBlend<'_>,
  depth: BD,
) -> Rav1dResult {
  let area = size.width.checked_mul(size.height).ok_or(EINVAL)?;
  let Ok(sample_count) = console_core_number_conversion::index(area);

  match kind {
    PredictorBlend::Average => write_predictor_rows(
      dst,
      predictions,
      size,
      1,
      iter::repeat_n(1, sample_count),
      depth,
    ),
    PredictorBlend::Weighted(weight) => write_predictor_rows(
      dst,
      predictions,
      size,
      4,
      iter::repeat_n(weight, sample_count),
      depth,
    ),
    PredictorBlend::Mask(mask) => write_predictor_rows(
      dst,
      predictions,
      size,
      6,
      mask.iter().copied().map(i32::from),
      depth,
    ),
  }
}

fn write_predictor_rows<BD: BitDepth>(
  mut dst: PicOffset,
  [first, second]: PredictorPair<'_>,
  size: Size<u64>,
  coefficient_bits: u8,
  mut weights: impl Iterator<Item = i32>,
  depth: BD,
) -> Rav1dResult {
  let intermediate_bits = depth.get_intermediate_bits();
  let denominator = 1i32.wrapping_shl(u32::from(coefficient_bits));
  let shift = u32::from(intermediate_bits.wrapping_add(coefficient_bits));
  let adjustment = ::core::primitive::i32::wrapping_shr(denominator, 1_u32)
    .wrapping_shl(u32::from(intermediate_bits))
    .wrapping_add(i32::from(BD::PREP_BIAS).wrapping_mul(denominator));
  let Ok(columns) = console_core_number_conversion::index(size.width);
  let packed_stride = i64::try_from(size.width).map_err(|_| EINVAL)?;
  let packed = RowLayout::new(size, 0, packed_stride).map_err(|_| EINVAL)?;
  let first_rows = packed.rows(first).map_err(|_| EINVAL)?;
  let second_rows = packed.rows(second).map_err(|_| EINVAL)?;
  let Ok(pitch) = console_core_number_conversion::fixed_offset(match dst.pixel_stride::<BD>() {
    Ok(stride) => stride,
  });
  let Ok(stride) = console_core_number_conversion::wrapped_offset(pitch);

  for (_, (left, right)) in (0..size.height).zip(first_rows.zip(second_rows)) {
    let mut out = dst.slice_mut::<BD>(columns);

    for (target, (&primary, &secondary)) in out.iter_mut().zip(left.iter().zip(right)) {
      let weight = weights.next().ok_or(EINVAL)?;
      let first_term = i32::from(primary).wrapping_mul(weight);
      let second_term = i32::from(secondary).wrapping_mul(denominator.wrapping_sub(weight));
      let value = first_term
        .wrapping_add(second_term)
        .wrapping_add(adjustment);
      *target = depth.iclip_pixel(value.wrapping_shr(shift));
    }

    drop(out);
    dst += stride;
  }

  Ok(())
}

enum BlockBlendMask<'a> {
  Pixels(&'a [u8]),
  Overlap(BlendAxis),
}

enum BlendRows<'a> {
  Pixels(&'a [u8]),
  PerRow(&'a [u8]),
  PerColumn(&'a [u8]),
}

struct BlockBlend<'a> {
  width: u64,
  input: RowLayout,
  weights: BlendRows<'a>,
}

impl<'a> BlockBlend<'a> {
  fn new<BD: BitDepth>(
    destination: PicOffset,
    extent: Size<i32>,
    mask: BlockBlendMask<'a>,
    prediction: &[BD::Pixel],
  ) -> Rav1dResult<Option<Self>> {
    let width = u64::try_from(extent.width).map_err(|_| EINVAL)?;
    let height = u64::try_from(extent.height).map_err(|_| EINVAL)?;

    match (width, height) {
      (0, _) | (_, 0) => return Ok(None),
      _ => {}
    }

    let full = Size { width, height };
    let (active, weights) = match mask {
      BlockBlendMask::Pixels(weights) => {
        let area = width.checked_mul(height).ok_or(EINVAL)?;
        let Ok(required) = console_core_number_conversion::indices_to(..area);
        let needed = weights.get(required).ok_or(EINVAL)?;
        (full, BlendRows::Pixels(needed))
      }
      BlockBlendMask::Overlap(axis) => {
        let (length, overlap_weights) = overlap_blend_weights(full, axis)?;

        match axis {
          BlendAxis::Horizontal => (
            Size {
              width,
              height: length,
            },
            BlendRows::PerRow(overlap_weights),
          ),
          BlendAxis::Vertical => (
            Size {
              width: length,
              height,
            },
            BlendRows::PerColumn(overlap_weights),
          ),
        }
      }
    };

    match (active.width, active.height) {
      (0, _) | (_, 0) => return Ok(None),
      _ => {}
    }

    let Ok(first) = console_core_number_conversion::fixed_index(destination.offset);
    let Ok(stride) =
      console_core_number_conversion::fixed_offset(match destination.pixel_stride::<BD>() {
        Ok(samples) => samples,
      });
    let output = RowLayout::new(full, first, stride).map_err(|_| EINVAL)?;
    output
      .validate_len(destination.data.pixel_len::<BD>())
      .map_err(|_| EINVAL)?;
    let input = RowLayout::new(active, 0, i64::from(extent.width)).map_err(|_| EINVAL)?;
    input.validate_len(prediction.len()).map_err(|_| EINVAL)?;

    Ok(Some(Self {
      width: active.width,
      input,
      weights,
    }))
  }

  fn apply<BD: BitDepth>(
    self,
    mut destination: PicOffset,
    prediction: &[BD::Pixel],
  ) -> Rav1dResult {
    let Ok(columns) = console_core_number_conversion::index(self.width);
    let Ok(pitch) =
      console_core_number_conversion::fixed_offset(match destination.pixel_stride::<BD>() {
        Ok(samples) => samples,
      });
    let Ok(stride) = console_core_number_conversion::wrapped_offset(pitch);
    let rows = self.input.rows(prediction).map_err(|_| EINVAL)?;

    for (row, input) in (0_u64..).zip(rows) {
      let mut pixels = destination.slice_mut::<BD>(columns);

      match self.weights {
        BlendRows::Pixels(weights) => {
          let samples = self.input.row(weights, row).map_err(|_| EINVAL)?;
          let Ok(()) = blend_weighted_row::<BD>(&mut pixels, input, samples.iter().copied());
        }
        BlendRows::PerRow(weights) => {
          let Ok(index) = console_core_number_conversion::index(row);
          let &weight = weights.get(index).ok_or(EINVAL)?;
          let Ok(()) =
            blend_weighted_row::<BD>(&mut pixels, input, iter::repeat_n(weight, columns));
        }
        BlendRows::PerColumn(weights) => {
          let Ok(()) = blend_weighted_row::<BD>(&mut pixels, input, weights.iter().copied());
        }
      }

      drop(pixels);
      destination += stride;
    }

    Ok(())
  }
}

fn overlap_blend_weights(
  extent: Size<u64>,
  axis: BlendAxis,
) -> Rav1dResult<(u64, &'static [u8])> {
  let start = match axis {
    BlendAxis::Horizontal => extent.height,
    BlendAxis::Vertical => extent.width,
  };
  let length = start.wrapping_mul(3).wrapping_shr(2);
  let end = start.checked_add(length).ok_or(EINVAL)?;
  let Ok(range) = console_core_number_conversion::indices(start..end);
  let weights = dav1d_obmc_masks.get(range).ok_or(EINVAL)?;

  Ok((length, weights))
}

fn blend_weighted_row<BD: BitDepth>(
  output: &mut [BD::Pixel],
  prediction: &[BD::Pixel],
  weights: impl Iterator<Item = u8>,
) -> Result<(), Never> {
  for ((first, &second), contribution) in output.iter_mut().zip(prediction).zip(weights) {
    let weight = u32::from(contribution);
    let rounded = (*first)
      .as_::<u32>()
      .wrapping_mul(64_u32.wrapping_sub(weight))
      .wrapping_add(second.as_::<u32>().wrapping_mul(weight))
      .wrapping_add(32)
      .wrapping_shr(6);
    *first = rounded.as_::<BD::Pixel>();
  }

  Ok(())
}

enum MaskPairReduction {
  Horizontal,
  Vertical,
  Accumulate,
}

struct CompoundMaskRows {
  chroma: Rav1dPixelLayoutSubSampled,
  width: u64,
  vertical_shift: u32,
  polarity: MaskPolarity,
}

impl CompoundMaskRows {
  fn write<BD: BitDepth>(
    &self,
    row_number: u64,
    dst: &mut [BD::Pixel],
    predictions: (&[i16], &[i16]),
    mask: &mut [u8],
    round: &CompoundMaskRounding<BD>,
  ) -> Rav1dResult {
    let (first, second) = predictions;
    let start = row_number
      .wrapping_shr(self.vertical_shift)
      .wrapping_mul(self.width);
    let end = start.wrapping_add(self.width);
    let Ok(indices) = console_core_number_conversion::indices(start..end);
    let mask_row = mask.get_mut(indices).ok_or(EINVAL)?;

    let reduction = match self.chroma {
      Rav1dPixelLayoutSubSampled::I444 => {
        for (((pixel, weight), &a), &b) in dst.iter_mut().zip(mask_row).zip(first).zip(second) {
          let Ok((blended, contribution)) = round.blend([a, b]);
          *pixel = blended;
          *weight = contribution;
        }

        return Ok(());
      }
      Rav1dPixelLayoutSubSampled::I422 => MaskPairReduction::Horizontal,
      Rav1dPixelLayoutSubSampled::I420 => match row_number & 1 {
        0 => MaskPairReduction::Accumulate,
        _odd_row => MaskPairReduction::Vertical,
      },
    };
    let (pixels, _complete_pixel_pairs) = dst.as_chunks_mut::<2>();
    let (first_predictions, _complete_first_pairs) = first.as_chunks::<2>();
    let (second_predictions, _complete_second_pairs) = second.as_chunks::<2>();
    let sign = u16::from(u8::from(self.polarity));

    for ((([left, right], weight), &[first_left, first_right]), &[second_left, second_right]) in
      pixels
        .iter_mut()
        .zip(mask_row)
        .zip(first_predictions)
        .zip(second_predictions)
    {
      let Ok((blended_left, left_weight)) = round.blend([first_left, second_left]);
      let Ok((blended_right, right_weight)) = round.blend([first_right, second_right]);
      *left = blended_left;
      *right = blended_right;
      let pair = u16::from(left_weight).wrapping_add(u16::from(right_weight));
      let averaged = match reduction {
        MaskPairReduction::Accumulate => pair,
        MaskPairReduction::Vertical => pair
          .wrapping_add(u16::from(*weight))
          .wrapping_add(2)
          .wrapping_sub(sign)
          .wrapping_shr(2),
        MaskPairReduction::Horizontal => pair.wrapping_add(1).wrapping_sub(sign).wrapping_shr(1),
      };
      let Ok(value) = console_core_number_conversion::wrapped::<_, u8>(averaged);
      *weight = value;
    }

    Ok(())
  }
}

struct CompoundMaskRounding<BD> {
  bit_depth: BD,
  pixel_shift: u32,
  pixel_round: i32,
  weight_shift: u32,
  weight_round: u16,
}

impl<BD: BitDepth> CompoundMaskRounding<BD> {
  fn new(bit_depth: BD) -> Result<Self, Never> {
    let intermediate = u32::from(bit_depth.get_intermediate_bits());
    let weight_shift = u32::from(bit_depth.bitdepth())
      .wrapping_add(intermediate)
      .wrapping_sub(4);
    let weight_round = 1_u16.wrapping_shl(weight_shift.wrapping_sub(5));
    let pixel_round = 32_i32
      .wrapping_shl(intermediate)
      .wrapping_add(i32::from(BD::PREP_BIAS).wrapping_mul(64));

    Ok(Self {
      bit_depth,
      pixel_shift: intermediate.wrapping_add(6),
      pixel_round,
      weight_shift,
      weight_round,
    })
  }

  #[inline]
  fn blend(&self, [first, second]: [i16; 2]) -> Result<(BD::Pixel, u8), Never> {
    let difference = first.abs_diff(second).saturating_add(self.weight_round);
    let contribution = 38_u16
      .wrapping_add(difference.wrapping_shr(self.weight_shift))
      .min(64);
    let weight = i32::from(contribution);
    let combined = i32::from(first)
      .wrapping_mul(weight)
      .wrapping_add(i32::from(second).wrapping_mul(64_i32.wrapping_sub(weight)))
      .wrapping_add(self.pixel_round);
    let Ok(mask) = console_core_number_conversion::wrapped::<_, u8>(contribution);
    let pixel = self
      .bit_depth
      .iclip_pixel(combined.wrapping_shr(self.pixel_shift));

    Ok((pixel, mask))
  }
}

struct CompoundMaskPlan {
  extent: Size<u64>,
  rows: CompoundMaskRows,
  input: RowLayout,
  mask_samples: u64,
}

fn compound_mask_plan<BD: BitDepth>(
  destination: &PicOffset,
  dimensions: Size<i32>,
  chroma: Rav1dPixelLayoutSubSampled,
  polarity: MaskPolarity,
) -> Rav1dResult<CompoundMaskPlan> {
  let width = u64::try_from(dimensions.width).map_err(|_outside_width| EINVAL)?;
  let height = u64::try_from(dimensions.height).map_err(|_outside_height| EINVAL)?;
  let (horizontal_shift, vertical_shift) = match chroma {
    Rav1dPixelLayoutSubSampled::I420 => (1, 1),
    Rav1dPixelLayoutSubSampled::I422 => (1, 0),
    Rav1dPixelLayoutSubSampled::I444 => (0, 0),
  };
  let mask_width = width.wrapping_shr(horizontal_shift);
  let mask_height = height.wrapping_shr(vertical_shift);

  match (
    mask_width.wrapping_shl(horizontal_shift).cmp(&width),
    mask_height.wrapping_shl(vertical_shift).cmp(&height),
  ) {
    (cmp::Ordering::Equal, cmp::Ordering::Equal) => {}
    _incomplete_chroma_block => return Err(EINVAL),
  }

  let extent = Size { width, height };
  let input =
    RowLayout::new(extent, 0, i64::from(dimensions.width)).map_err(|_input_layout| EINVAL)?;
  let Ok(capacity) = console_core_number_conversion::index(MAX_COMPOUND_SAMPLES);
  input
    .validate_len(capacity)
    .map_err(|_short_predictions| EINVAL)?;
  let Ok(start) = console_core_number_conversion::fixed_index(destination.offset);
  let Ok(native_stride) =
    ::console_core_number_conversion::fixed_offset(match destination.pixel_stride::<BD>() {
      ::core::result::Result::Ok(native_stride) => native_stride,
    });
  let Ok(stride) = console_core_number_conversion::fixed_offset(
    match ::console_core_number_conversion::wrapped_offset(native_stride) {
      ::core::result::Result::Ok(native_position) => native_position,
    },
  );
  let output = RowLayout::new(extent, start, stride).map_err(|_output_layout| EINVAL)?;
  output
    .validate_len(destination.data.pixel_len::<BD>())
    .map_err(|_short_output| EINVAL)?;
  let mask_samples = mask_width.checked_mul(mask_height).ok_or(EINVAL)?;
  let maximum_mask_samples = u64::from(MAX_COMPOUND_SAMPLES);

  match mask_samples.cmp(&maximum_mask_samples) {
    cmp::Ordering::Less | cmp::Ordering::Equal => {}
    cmp::Ordering::Greater => return Err(EINVAL),
  }

  Ok(CompoundMaskPlan {
    extent,
    rows: CompoundMaskRows {
      chroma,
      width: mask_width,
      vertical_shift,
      polarity,
    },
    input,
    mask_samples,
  })
}

fn w_mask_rust<BD: BitDepth>(
  mut destination: PicOffset,
  predictions: (&CompoundPrediction, &CompoundPrediction),
  mask: &mut SegmentMask,
  CompoundMaskPlan {
    extent,
    rows,
    input,
    mask_samples,
  }: CompoundMaskPlan,
  bit_depth: BD,
) -> Rav1dResult {
  let first_rows = input.rows(predictions.0).map_err(|_first_short| EINVAL)?;
  let second_rows = input.rows(predictions.1).map_err(|_second_short| EINVAL)?;
  let Ok(indices) = console_core_number_conversion::indices_to(..mask_samples);
  let weights = mask.get_mut(indices).ok_or(EINVAL)?;
  let Ok(pitch) =
    console_core_number_conversion::fixed_offset(match destination.pixel_stride::<BD>() {
      Ok(samples) => samples,
    });
  let Ok(native_stride) = console_core_number_conversion::wrapped_offset(pitch);
  let Ok(round) = CompoundMaskRounding::new(bit_depth);
  let Ok(columns) = console_core_number_conversion::index(extent.width);

  for (row_number, pair) in (0..extent.height).zip(first_rows.zip(second_rows)) {
    let mut dst = destination.slice_mut::<BD>(columns);
    rows.write(row_number, &mut dst, pair, weights, &round)?;
    drop(dst);
    destination += native_stride;
  }

  Ok(())
}

#[derive(Clone, Copy)]
pub(crate) struct WarpSampling {
  pub(crate) coefficients: [i16; 4],
  pub(crate) phase: Point<i32>,
}

pub(crate) fn warp_coefficients(phase: i32) -> Rav1dResult<&'static [i8; 8]> {
  let ordinal = 64_i32.wrapping_add(::core::primitive::i32::wrapping_shr(
    phase.wrapping_add(512),
    10_u32,
  ));
  let Ok(index) = console_core_number_conversion::wrapped_index(ordinal);
  dav1d_mc_warp_filter.get(index).ok_or(EINVAL)
}

pub(crate) fn warp_vertical_sums(
  intermediate: &[[i16; 8]; 15],
  sampling: WarpSampling,
) -> Rav1dResult<[[i32; 8]; 8]> {
  let [_, _, gamma, delta] = sampling.coefficients.map(i32::from);
  let mut sums = [[0_i32; 8]; 8];

  for ((row, window), output) in (0_i32..).zip(intermediate.windows(8)).zip(&mut sums) {
    let travel = row.wrapping_mul(delta);
    let mut phase = sampling.phase.y.wrapping_add(travel);

    for (column, sample) in (0_u32..).zip(output) {
      let taps = warp_coefficients(phase)?;
      let Ok(index) = console_core_number_conversion::index(column);
      let filtered = taps
        .iter()
        .zip(window)
        .try_fold(0_i32, |sum, (&tap, row_2)| {
          let pixel = row_2.get(index).ok_or(EINVAL)?;
          Ok::<i32, crate::src::error::Rav1dError>(
            sum.wrapping_add(i32::from(*pixel).wrapping_mul(i32::from(tap))),
          )
        })?;
      *sample = filtered;
      phase = phase.wrapping_add(gamma);
    }
  }

  Ok(sums)
}

fn warp_scalar_sums<BD: BitDepth>(
  src: PicOffset,
  sampling: WarpSampling,
  depth: BD,
) -> Rav1dResult<[[i32; 8]; 8]> {
  let Ok(stride) = console_core_number_conversion::fixed_offset(match src.pixel_stride::<BD>() {
    Ok(samples) => samples,
  });
  let Ok(origin) = console_core_number_conversion::fixed_index(src.offset);
  let margin = stride
    .checked_mul(-3)
    .and_then(|rows| rows.checked_sub(3))
    .ok_or(EINVAL)?;
  let start = origin.checked_add_signed(margin).ok_or(EINVAL)?;
  let layout = RowLayout::new(
    Size {
      width: 15,
      height: 15,
    },
    start,
    stride,
  )
  .map_err(|_| EINVAL)?;
  layout
    .validate_len(src.data.pixel_len::<BD>())
    .map_err(|_| EINVAL)?;
  let Ok(offset) = console_core_number_conversion::index(start);
  let Ok(step) = console_core_number_conversion::wrapped_offset(stride);
  let mut reference = PicOffset {
    data: src.data,
    offset,
  };
  let [alpha, beta, _, _] = sampling.coefficients.map(i32::from);
  let shift = 7_u32.wrapping_sub(u32::from(depth.get_intermediate_bits()));
  let mut intermediate = [[0_i16; 8]; 15];

  for (row_number, output) in (0_i32..).zip(&mut intermediate) {
    let travel = row_number.wrapping_mul(beta);
    let mut phase = sampling.phase.x.wrapping_add(travel);
    let pixels = reference.slice::<BD>(15);

    for (window, sample) in pixels.windows(8).zip(output) {
      let taps = warp_coefficients(phase)?;
      let sum = taps.iter().zip(window).fold(0_i32, |sum, (&tap, &input)| {
        sum.wrapping_add(input.as_::<i32>().wrapping_mul(i32::from(tap)))
      });
      let Ok(rounded) =
        console_core_number_conversion::wrapped::<_, i16>(round_shift(sum, shift));
      *sample = rounded;
      phase = phase.wrapping_add(alpha);
    }

    reference += step;
  }

  warp_vertical_sums(&intermediate, sampling)
}

pub(crate) fn warp_affine_8x8_rust<BD: BitDepth>(
  mut dst: PicOffset,
  src: PicOffset,
  sampling: WarpSampling,
  depth: BD,
) -> Rav1dResult {
  let Ok(stride) = console_core_number_conversion::fixed_offset(match dst.pixel_stride::<BD>() {
    Ok(samples) => samples,
  });
  let Ok(origin) = console_core_number_conversion::fixed_index(dst.offset);
  let layout = RowLayout::new(
    Size {
      width: 8,
      height: 8,
    },
    origin,
    stride,
  )
  .map_err(|_| EINVAL)?;
  layout
    .validate_len(dst.data.pixel_len::<BD>())
    .map_err(|_| EINVAL)?;
  let sums = warp_scalar_sums(src, sampling, depth)?;
  let shift = 7_u32.wrapping_add(u32::from(depth.get_intermediate_bits()));
  let Ok(step) = console_core_number_conversion::wrapped_offset(stride);

  for row in sums {
    for (target, value) in dst.slice_mut::<BD>(8).iter_mut().zip(row) {
      *target = depth.iclip_pixel(round_shift(value, shift));
    }

    dst += step;
  }

  Ok(())
}

pub(crate) fn warp_affine_8x8t_rust<BD: BitDepth>(
  output: &mut [i16],
  stride: u64,
  src: PicOffset,
  sampling: WarpSampling,
  depth: BD,
) -> Rav1dResult {
  write_warp_predictor::<BD>(output, stride, || warp_scalar_sums(src, sampling, depth))
}

pub(crate) fn write_warp_predictor<BD: BitDepth>(
  output: &mut [i16],
  stride: u64,
  evaluate: impl FnOnce() -> Rav1dResult<[[i32; 8]; 8]>,
) -> Rav1dResult {
  let pitch = i64::try_from(stride).map_err(|_| EINVAL)?;
  let layout = RowLayout::new(
    Size {
      width: 8,
      height: 8,
    },
    0,
    pitch,
  )
  .map_err(|_| EINVAL)?;
  let rows = layout.rows_mut(output).map_err(|_| EINVAL)?;
  let sums = evaluate()?;

  for (row, values) in rows.zip(sums) {
    for (target, value) in row.iter_mut().zip(values) {
      *target = BD::sub_prep_bias(round_shift(value, 7));
    }
  }

  Ok(())
}

pub(crate) struct EdgeWindow {
  pub block: Size<i32>,
  pub reference: Size<i32>,
  pub origin: Point<i32>,
}

fn emu_edge_rust<BD: BitDepth>(
  window: EdgeWindow,
  dst: &mut EmulatedEdge<BD::Pixel>,
  dst_stride: u32,
  reference: &Rav1dPictureDataComponent,
) -> Result<(), console_core_pixel_buffers::RowLayoutError> {
  use console_core_number_conversion::{fixed_index, fixed_offset, index, indices};
  use console_core_pixel_buffers::{RowLayoutError, copy_clamped_window};

  let width =
    u64::try_from(window.reference.width).map_err(|_negative_width| RowLayoutError::Bounds)?;
  let height =
    u64::try_from(window.reference.height).map_err(|_negative_height| RowLayoutError::Bounds)?;
  let out_width =
    u64::try_from(window.block.width).map_err(|_negative_width| RowLayoutError::Bounds)?;
  let out_height =
    u64::try_from(window.block.height).map_err(|_negative_height| RowLayoutError::Bounds)?;
  let destination = RowLayout::new(
    Size {
      width: out_width,
      height: out_height,
    },
    0,
    i64::from(dst_stride),
  )?;
  let Ok(row_stride) =
    ::console_core_number_conversion::fixed_offset(match reference.pixel_stride::<BD>() {
      ::core::result::Result::Ok(row_stride) => row_stride,
    });
  let Ok(stride) = fixed_offset(
    match ::console_core_number_conversion::wrapped_offset(row_stride) {
      ::core::result::Result::Ok(native_position) => native_position,
    },
  );
  let Ok(first_sample) = fixed_index(reference.with_offset::<BD>().offset);
  let reference_plane = RowLayout::new(Size { width, height }, first_sample, stride)?;
  reference_plane.validate_len(reference.pixel_len::<BD>())?;
  let Ok(ref_width) = index(width);
  let Ok(ref_height) = index(height);
  let origin = Point {
    x: i64::from(window.origin.x),
    y: i64::from(window.origin.y),
  };

  copy_clamped_window(
    Size {
      width: ref_width,
      height: ref_height,
    },
    origin,
    (&destination, dst),
    |row, columns| {
      let row_number = match i64::try_from(row) {
        Ok(signed_row) => signed_row,
        Err(_outside_coordinate_range) => return None,
      };
      let displacement = row_number.checked_mul(stride)?;
      let Ok(column) = fixed_index(columns.start);
      let Ok(length) = fixed_index(columns.len());
      let checked_value = first_sample.checked_add_signed(displacement)?;
      let start = checked_value.checked_add(column)?;
      let end = start.checked_add(length)?;
      let Ok(source) = indices(start..end);

      Some(reference.slice::<BD, _>(source))
    },
    |source, into| {
      for (out, &sample) in into.iter_mut().zip(source) {
        *out = sample;
      }
    },
  )
}

#[derive(Clone, Copy)]
pub(crate) struct ResizePhase {
  pub step: i32,
  pub initial: i32,
}

pub(crate) struct ResizeSampling {
  pub source_width: u64,
  pub destination: Size<u64>,
  pub phase: ResizePhase,
}

fn resize_row<BD: BitDepth>(
  output: &mut [BD::Pixel],
  input: &[BD::Pixel],
  phase: &ResizePhase,
  depth: BD,
) -> Rav1dResult {
  let Ok(input_length) = console_core_number_conversion::fixed_index(input.len());
  let last = input_length.checked_sub(1).ok_or(EINVAL)?;
  let max_column = i32::try_from(last).map_err(|_| EINVAL)?;
  let mut subpixel = phase.initial;
  let mut input_x = -4i32;

  for output_sample in output {
    let slot =
      u32::try_from(::core::primitive::i32::wrapping_shr(subpixel, 8_u32)).map_err(|_| EINVAL)?;
    let Ok(index) = console_core_number_conversion::index(slot);
    let filter = dav1d_resize_filter.get(index).ok_or(EINVAL)?;
    let mut sum = 0i32;

    for (tap, coefficient) in (0i32..).zip(filter) {
      let source_x = iclip(input_x.wrapping_add(tap), 0, max_column);
      let Ok(position) = console_core_number_conversion::index(source_x);
      let pixel = input.get(position).ok_or(EINVAL)?;
      let term = (*pixel).as_::<i32>().wrapping_mul(i32::from(*coefficient));
      sum = sum.wrapping_add(term);
    }

    *output_sample = depth.iclip_pixel(round_shift(sum.wrapping_neg(), 7));
    let next = subpixel.wrapping_add(phase.step);
    input_x = input_x.wrapping_add(::core::primitive::i32::wrapping_shr(next, 14_u32));
    subpixel = next & 0x3fff;
  }

  Ok(())
}

fn resize_rust<BD: BitDepth>(
  mut destination: WithOffset<PicOrBuf<AlignedVec64<u8>>>,
  mut source: PicOffset,
  sampling: ResizeSampling,
  depth: BD,
) -> Rav1dResult {
  let ResizeSampling {
    source_width,
    destination: size,
    phase,
  } = sampling;

  match (size.width, size.height) {
    (0, _) | (_, 0) => return Ok(()),
    _ => {}
  }

  match phase.initial {
    0..=0x3fff => {}
    _ => return Err(EINVAL),
  }

  let Ok(source_pitch) =
    console_core_number_conversion::fixed_offset(match source.pixel_stride::<BD>() {
      Ok(stride) => stride,
    });
  let Ok(destination_pitch) =
    console_core_number_conversion::fixed_offset(match destination.pixel_stride::<BD>() {
      Ok(stride) => stride,
    });
  let Ok(source_origin) = console_core_number_conversion::fixed_index(source.offset);
  let Ok(destination_origin) = console_core_number_conversion::fixed_index(destination.offset);
  let input = RowLayout::new(
    Size {
      width: source_width,
      height: size.height,
    },
    source_origin,
    source_pitch,
  )
  .map_err(|_| EINVAL)?;
  input
    .validate_len(source.data.pixel_len::<BD>())
    .map_err(|_| EINVAL)?;
  let layout = RowLayout::new(size, destination_origin, destination_pitch).map_err(|_| EINVAL)?;
  let Ok(pixel_bytes) =
    console_core_number_conversion::fixed_index(core::mem::size_of::<BD::Pixel>());
  let output_length = match destination.data {
    PicOrBuf::Pic(picture) => {
      let Ok(samples) = console_core_number_conversion::fixed_index(picture.pixel_len::<BD>());
      samples
    }
    PicOrBuf::Buf(buffer) => {
      let Ok(bytes) = console_core_number_conversion::fixed_index(buffer.len());
      bytes.checked_div(pixel_bytes).ok_or(EINVAL)?
    }
  };
  let Ok(output_samples) = console_core_number_conversion::index(output_length);
  layout.validate_len(output_samples).map_err(|_| EINVAL)?;
  let Ok(input_columns) = console_core_number_conversion::index(source_width);
  let Ok(output_columns) = console_core_number_conversion::index(size.width);
  let Ok(source_stride) = console_core_number_conversion::wrapped_offset(source_pitch);
  let Ok(destination_stride) = console_core_number_conversion::wrapped_offset(destination_pitch);

  for _ in 0..size.height {
    let input_row = source.slice::<BD>(input_columns);

    match destination.data {
      PicOrBuf::Pic(picture) => {
        let mut row = picture.slice_mut::<BD, _>((destination.offset.., ..output_columns));
        resize_row::<BD>(&mut row, &input_row, &phase, depth)?;
      }
      PicOrBuf::Buf(buffer) => {
        let mut row = buffer.mut_slice_as((destination.offset.., ..output_columns));
        resize_row::<BD>(&mut row, &input_row, &phase, depth)?;
      }
    }

    drop(input_row);
    destination += destination_stride;
    source += source_stride;
  }

  Ok(())
}

wrap_fn_ptr!(pub mc);

impl mc::Fn {
  pub fn call<BD: BitDepth>(
    &self,
    filter: Filter2d,
    dst: PicOffset,
    src: PicOffset,
    sampling: UnscaledSampling,
    bd: BD,
  ) -> Rav1dResult {
    mc_put_direct::<BD>(filter, dst, src, sampling, bd)
  }
}

wrap_fn_ptr!(pub mc_scaled);

impl mc_scaled::Fn {
  pub fn call<BD: BitDepth>(
    &self,
    filter: Filter2d,
    dst: PicOffset,
    src: PicOffset,
    sampling: ScaledSampling,
    bd: BD,
  ) -> Rav1dResult {
    put_scaled(filter, dst, src, sampling, bd)
  }
}

wrap_fn_ptr!(pub warp8x8);

impl warp8x8::Fn {
  pub fn call<BD: BitDepth>(
    &self,
    dst: PicOffset,
    src: PicOffset,
    sampling: WarpSampling,
    bd: BD,
  ) -> Rav1dResult {
    warp8x8_direct::<BD>(dst, src, sampling, bd)
  }
}

wrap_fn_ptr!(pub mct);

impl mct::Fn {
  pub fn call<BD: BitDepth>(
    &self,
    filter: Filter2d,
    tmp: &mut [i16],
    src: PicOffset,
    sampling: UnscaledSampling,
    bd: BD,
  ) -> Rav1dResult {
    mct_prep_direct::<BD>(filter, tmp, src, sampling, bd)
  }
}

wrap_fn_ptr!(pub mct_scaled);

impl mct_scaled::Fn {
  pub fn call<BD: BitDepth>(
    &self,
    filter: Filter2d,
    tmp: &mut [i16],
    src: PicOffset,
    sampling: ScaledSampling,
    bd: BD,
  ) -> Rav1dResult {
    prep_scaled(filter, tmp, src, sampling, bd)
  }
}

wrap_fn_ptr!(pub warp8x8t);

impl warp8x8t::Fn {
  pub fn call<BD: BitDepth>(
    &self,
    output: &mut [i16],
    stride: u64,
    src: PicOffset,
    sampling: WarpSampling,
    bd: BD,
  ) -> Rav1dResult {
    warp8x8t_direct::<BD>(output, stride, src, sampling, bd)
  }
}

wrap_fn_ptr!(pub avg);

impl avg::Fn {
  pub(crate) fn call<BD: BitDepth>(
    &self,
    dst: PicOffset,
    predictions: PredictorPair<'_>,
    extent: Size<i32>,
    depth: BD,
  ) -> Rav1dResult {
    blend_predictors(dst, predictions, extent, PredictorBlend::Average, depth)
  }
}

wrap_fn_ptr!(pub w_avg);

impl w_avg::Fn {
  pub(crate) fn call<BD: BitDepth>(
    &self,
    dst: PicOffset,
    predictions: PredictorPair<'_>,
    extent: Size<i32>,
    weight: i32,
    depth: BD,
  ) -> Rav1dResult {
    blend_predictors(
      dst,
      predictions,
      extent,
      PredictorBlend::Weighted(weight),
      depth,
    )
  }
}

wrap_fn_ptr!(pub mask);

impl mask::Fn {
  pub(crate) fn call<BD: BitDepth>(
    &self,
    dst: PicOffset,
    predictions: PredictorPair<'_>,
    extent: Size<i32>,
    mask: &[u8],
    depth: BD,
  ) -> Rav1dResult {
    blend_predictors(dst, predictions, extent, PredictorBlend::Mask(mask), depth)
  }
}

wrap_fn_ptr!(pub w_mask);

impl w_mask::Fn {
  pub fn call<BD: BitDepth>(
    &self,
    layout: Rav1dPixelLayoutSubSampled,
    dst: PicOffset,
    predictions: (&CompoundPrediction, &CompoundPrediction),
    extent: Size<i32>,
    mask: &mut SegmentMask,
    polarity: MaskPolarity,
    bd: BD,
  ) -> Rav1dResult {
    w_mask_direct::<BD>(layout, dst, predictions, extent, mask, polarity, bd)
  }
}

wrap_fn_ptr!(pub blend);

impl blend::Fn {
  pub fn call<BD: BitDepth>(
    &self,
    dst: PicOffset,
    tmp: &InterIntraPrediction<BD::Pixel>,
    extent: Size<i32>,
    mask: &[u8],
  ) -> Rav1dResult {
    blend_block_direct::<BD>(dst, extent, BlockBlendInput::Pixels { source: tmp, mask })
  }
}

wrap_fn_ptr!(pub blend_dir);

impl blend_dir::Fn {
  pub fn call<BD: BitDepth>(
    &self,
    axis: BlendAxis,
    dst: PicOffset,
    tmp: &OverlapPrediction<BD::Pixel>,
    extent: Size<i32>,
  ) -> Rav1dResult {
    blend_block_direct::<BD>(dst, extent, BlockBlendInput::Overlap { source: tmp, axis })
  }
}

wrap_fn_ptr!(pub emu_edge);

impl emu_edge::Fn {
  pub fn call<BD: BitDepth>(
    &self,
    window: EdgeWindow,
    dst: &mut EmulatedEdge<BD::Pixel>,
    dst_stride: u32,
    src: &Rav1dPictureDataComponent,
  ) -> Rav1dResult {
    emu_edge_rust::<BD>(window, dst, dst_stride, src).map_err(|_invalid_edge_window| EINVAL)
  }
}

wrap_fn_ptr!(pub resize);

impl resize::Fn {
  pub(crate) fn call<BD: BitDepth>(
    &self,
    dst: WithOffset<PicOrBuf<AlignedVec64<u8>>>,
    src: PicOffset,
    sampling: ResizeSampling,
    depth: BD,
  ) -> Rav1dResult {
    resize_rust::<BD>(dst, src, sampling, depth)
  }
}

pub struct Rav1dMCDSPContext {
  pub mc: enum_map_ty!(Filter2d, mc::Fn),
  pub mc_scaled: enum_map_ty!(Filter2d, mc_scaled::Fn),
  pub mct: enum_map_ty!(Filter2d, mct::Fn),
  pub mct_scaled: enum_map_ty!(Filter2d, mct_scaled::Fn),
  pub avg: avg::Fn,
  pub w_avg: w_avg::Fn,
  pub mask: mask::Fn,
  pub w_mask: enum_map_ty!(Rav1dPixelLayoutSubSampled, w_mask::Fn),
  pub blend: blend::Fn,
  pub blend_v: blend_dir::Fn,
  pub blend_h: blend_dir::Fn,
  pub warp8x8: warp8x8::Fn,
  pub warp8x8t: warp8x8t::Fn,
  pub emu_edge: emu_edge::Fn,
  pub resize: resize::Fn,
}

impl Rav1dMCDSPContext {
  pub const fn default<BD: BitDepth>() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(mc) = EnumMap::default();
    let Ok(mct) = EnumMap::default();
    let Ok(mc_scaled) = EnumMap::default();
    let Ok(mct_scaled) = EnumMap::default();
    let Ok(w_mask) = EnumMap::default();

    Ok({
      Self {
        mc,
        mct,
        mc_scaled,
        mct_scaled,
        avg: avg::Fn::DEFAULT,
        w_avg: w_avg::Fn::DEFAULT,
        mask: mask::Fn::DEFAULT,
        w_mask,
        blend: blend::Fn::DEFAULT,
        blend_v: blend_dir::Fn::DEFAULT,
        blend_h: blend_dir::Fn::DEFAULT,
        warp8x8: warp8x8::Fn::DEFAULT,
        warp8x8t: warp8x8t::Fn::DEFAULT,
        emu_edge: emu_edge::Fn::DEFAULT,
        resize: resize::Fn::DEFAULT,
      }
    })
  }

  #[inline(always)]
  const fn init<BD: BitDepth>(
    self,
    flags: CpuFlags,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      {
        let _ = flags;
        self
      }
    })
  }

  pub const fn new<BD: BitDepth>(
    flags: CpuFlags,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let Ok(rav1d_mcdsp_context) = Self::default::<BD>();
      let Ok(init) = rav1d_mcdsp_context.init::<BD>(flags);

      init
    })
  }
}

#[cfg(test)]
mod compound_mask_tests {
  use super::*;
  use crate::include::common::bitdepth::{BitDepth8, BitDepth16};
  use crate::src::align::Align64;
  use aligned::Aligned;

  const FIRST: [i16; 16] = [
    -8192, -8001, -7511, -7000, -5523, -4000, -2011, -1000, 0, 900, 1906, 3000, 4000, 5012, 6108,
    16000,
  ];
  const SECOND: [i16; 16] = [
    -8192, -8192, -7900, -7010, -5600, -2710, -2000, -900, 612, -1100, 3100, 2900, 1900, 6012,
    2000, 26000,
  ];
  const MASKS_8: [[&[u8]; 2]; 3] = [
    [&[39, 38, 43, 50], &[39, 38, 43, 49]],
    [
      &[38, 39, 41, 38, 43, 40, 44, 59],
      &[38, 38, 40, 38, 42, 40, 43, 59],
    ],
    [
      &[
        38, 38, 39, 38, 38, 43, 38, 38, 40, 45, 42, 38, 46, 41, 54, 64,
      ],
      &[
        38, 38, 39, 38, 38, 43, 38, 38, 40, 45, 42, 38, 46, 41, 54, 64,
      ],
    ],
  ];
  const MASKS_HIGH: [[&[u8]; 2]; 3] = [
    [&[38, 38, 39, 42], &[38, 38, 39, 41]],
    [
      &[38, 38, 39, 38, 39, 39, 40, 45],
      &[38, 38, 38, 38, 38, 38, 39, 44],
    ],
    [
      &[
        38, 38, 38, 38, 38, 39, 38, 38, 38, 39, 39, 38, 40, 39, 42, 47,
      ],
      &[
        38, 38, 38, 38, 38, 39, 38, 38, 38, 39, 39, 38, 40, 39, 42, 47,
      ],
    ],
  ];

  fn check_mask<BD: BitDepth>(
    bit_depth: BD,
    expected_pixels: [u16; 16],
    weights_by_layout: [[&[u8]; 2]; 3],
  ) -> Rav1dResult {
    use Rav1dPixelLayoutSubSampled::{I420, I422, I444};

    let mut first = [0; 128 * 128];
    let mut second = [0; 128 * 128];
    let first_pixels = first.get_mut(..FIRST.len()).ok_or(EINVAL)?;
    let second_pixels = second.get_mut(..SECOND.len()).ok_or(EINVAL)?;
    first_pixels
      .iter_mut()
      .zip(FIRST)
      .for_each(|(dst, value)| *dst = value);
    second_pixels
      .iter_mut()
      .zip(SECOND)
      .for_each(|(dst, value)| *dst = value);
    let dimensions = Size {
      width: 4,
      height: 4,
    };

    for (layout, expected) in [I420, I422, I444].into_iter().zip(weights_by_layout) {
      for (polarity, weights) in [MaskPolarity::Normal, MaskPolarity::Inverted]
        .into_iter()
        .zip(expected)
      {
        let mut plane: Align64<[BD::Pixel; 64]> = Aligned([bit_depth.iclip_pixel(0); 64]);
        let component = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut *plane, 4);
        let offset = PicOffset {
          data: &component,
          offset: 0,
        };
        let untouched = 255_u8;
        let mut mask = [untouched; 128 * 128];
        let plan = compound_mask_plan::<BD>(&offset, dimensions, layout, polarity)?;
        w_mask_rust(offset, (&first, &second), &mut mask, plan, bit_depth)?;
        let pixels = offset.slice::<BD>(expected_pixels.len());
        assert!(
          pixels
            .iter()
            .copied()
            .map(<BD::Pixel as Into<u16>>::into)
            .eq(expected_pixels)
        );
        let (written, padding) = mask.split_at_checked(weights.len()).ok_or(EINVAL)?;
        assert_eq!(written, weights);
        assert!(padding.iter().all(|&entry| entry == untouched));
      }
    }

    Ok(())
  }

  #[test]
  fn scalar_compound_masks_preserve_both_rounding_polarities() -> Rav1dResult {
    check_mask(
      BitDepth8::new(()),
      [0, 0, 0, 0, 0, 0, 0, 0, 14, 19, 145, 185, 213, 255, 255, 255],
      MASKS_8,
    )?;
    check_mask(
      BitDepth16::new(1023),
      [
        0, 7, 33, 74, 165, 293, 387, 452, 528, 519, 660, 697, 713, 850, 805, 1023,
      ],
      MASKS_HIGH,
    )?;
    check_mask(
      BitDepth16::new(4095),
      [
        0, 28, 131, 297, 659, 1174, 1546, 1808, 2110, 2078, 2641, 2788, 2851, 3399, 3222, 4095,
      ],
      MASKS_HIGH,
    )
  }

  #[test]
  fn edge_window_repeats_boundary_pixels_and_keeps_padding() -> Rav1dResult {
    let mut image: Align64<[u8; 64]> = Aligned([99; 64]);

    for (row, samples) in
      image
        .chunks_exact_mut(8)
        .zip([[1, 2, 3, 4], [11, 12, 13, 14], [21, 22, 23, 24]])
    {
      row
        .iter_mut()
        .zip(samples)
        .for_each(|(dst, sample)| *dst = sample);
    }

    let source = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut *image, 8);
    let mut output = [77; 320 * (256 + 7)];
    let extent = Size {
      width: 8,
      height: 5,
    };
    let origin = Point { x: -2, y: -1 };
    let invalid = EdgeWindow {
      block: extent,
      reference: Size {
        width: 9,
        height: 3,
      },
      origin,
    };
    let refused = emu_edge::Fn::DEFAULT.call::<BitDepth8>(invalid, &mut output, 10, &source);
    assert_eq!(refused, Err(EINVAL));
    assert!(output.iter().all(|&value| value == 77));

    let window = EdgeWindow {
      block: extent,
      reference: Size {
        width: 4,
        height: 3,
      },
      origin,
    };
    emu_edge::Fn::DEFAULT.call::<BitDepth8>(window, &mut output, 10, &source)?;
    let (pixels, padding) = output.split_at_checked(50).ok_or(EINVAL)?;
    let first = [1, 1, 1, 2, 3, 4, 4, 4, 77, 77];
    let middle = [11, 11, 11, 12, 13, 14, 14, 14, 77, 77];
    let last = [21, 21, 21, 22, 23, 24, 24, 24, 77, 77];
    assert!(
      pixels.chunks_exact(10).eq(
        [first, first, middle, last, last]
          .iter()
          .map(<[u8; 10]>::as_slice)
      )
    );
    assert!(padding.iter().all(|&value| value == 77));

    Ok(())
  }

  #[cfg(feature = "bitdepth_8")]
  #[test]
  fn checked_resize_rejects_short_output_and_invalid_phase_before_writing() {
    let mut input: Align64<[u8; 128]> = Aligned([11; 128]);
    let mut output: Align64<[u8; 64]> = Aligned([77; 64]);
    let source = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut *input, 32);
    let target = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut *output, 32);
    let src = PicOffset {
      data: &source,
      offset: 0,
    };
    let out = PicOffset {
      data: &target,
      offset: 0,
    };
    let Ok(destination) = WithOffset::pic(out);
    let depth = BitDepth8::new(());

    let short = ResizeSampling {
      source_width: 16,
      destination: Size {
        width: 24,
        height: 4,
      },
      phase: ResizePhase {
        step: 10923,
        initial: 0,
      },
    };
    let bad_phase = ResizeSampling {
      source_width: 16,
      destination: Size {
        width: 24,
        height: 2,
      },
      phase: ResizePhase {
        step: 10923,
        initial: 0x4000,
      },
    };

    for invalid in [short, bad_phase] {
      assert_eq!(
        resize::Fn::DEFAULT.call::<BitDepth8>(destination, src, invalid, depth),
        Err(EINVAL)
      );
    }

    assert!(
      out
        .slice::<BitDepth8>(64)
        .iter()
        .all(|&sample| sample == 77)
    );
  }

  #[cfg(feature = "bitdepth_8")]
  #[test]
  fn checked_resize_preserves_negative_stride_and_row_padding() -> Rav1dResult {
    let mut input: Align64<[u8; 128]> = Aligned([0; 128]);
    let mut output: Align64<[u8; 128]> = Aligned([77; 128]);

    for (row, stamp) in input.chunks_exact_mut(32).zip([13, 67, 131]) {
      row.fill(stamp);
    }

    let ::core::result::Result::Ok(bottom_up) =
      ::console_core_number_conversion::fixed_index((-32isize).cast_unsigned());
    let source = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(
      &mut *input,
      match ::console_core_number_conversion::index(bottom_up) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    );
    let target = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(
      &mut *output,
      match ::console_core_number_conversion::index(bottom_up) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    );
    let src = PicOffset {
      data: &source,
      offset: 64,
    };
    let out = PicOffset {
      data: &target,
      offset: 64,
    };
    let Ok(destination) = WithOffset::pic(out);
    let sampling = ResizeSampling {
      source_width: 16,
      destination: Size {
        width: 24,
        height: 3,
      },
      phase: ResizePhase {
        step: 10923,
        initial: 719,
      },
    };
    resize::Fn::DEFAULT.call::<BitDepth8>(destination, src, sampling, BitDepth8::new(()))?;
    let stored = target.slice::<BitDepth8, _>(..);

    for (row, stamp) in stored.chunks_exact(32).zip([13, 67, 131, 77]) {
      assert!(row.iter().take(24).all(|&sample| sample == stamp));
      assert!(row.iter().skip(24).all(|&sample| sample == 77));
    }

    Ok(())
  }

  #[cfg(feature = "bitdepth_8")]
  #[test]
  fn checked_predictor_blend_refuses_incomplete_mask_before_writing() {
    let mut storage: Align64<[u8; 64]> = Aligned([77; 64]);
    let picture = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut *storage, 16);
    let dst = PicOffset {
      data: &picture,
      offset: 0,
    };
    let first = [320i16; 128 * 128];
    let second = [640i16; 128 * 128];
    let first_row_mask = [32u8; 12];
    let applied = mask::Fn::DEFAULT.call::<BitDepth8>(
      dst,
      [&first, &second],
      Size {
        width: 12,
        height: 2,
      },
      &first_row_mask,
      BitDepth8::new(()),
    );
    assert_eq!(applied, Err(EINVAL));
    assert!(
      picture
        .slice::<BitDepth8, _>(..)
        .iter()
        .all(|&sample| sample == 77)
    );
  }

  #[cfg(feature = "bitdepth_8")]
  #[test]
  fn checked_block_blend_refuses_short_masks_and_destinations() {
    let mut storage: Align64<[u8; 64]> = Aligned([77; 64]);
    let picture = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut *storage, 8);
    let prediction = [192_u8; 64 * 64];
    let extent = Size {
      width: 4,
      height: 4,
    };
    let top = PicOffset {
      data: &picture,
      offset: 0,
    };
    let bottom = PicOffset {
      data: &picture,
      offset: 56,
    };

    assert_eq!(
      blend::Fn::DEFAULT.call::<BitDepth8>(top, &prediction, extent, &[32; 4]),
      Err(EINVAL)
    );
    assert_eq!(
      blend::Fn::DEFAULT.call::<BitDepth8>(bottom, &prediction, extent, &[32; 16]),
      Err(EINVAL)
    );
    assert!(
      picture
        .slice::<BitDepth8, _>(..)
        .iter()
        .all(|&sample| sample == 77)
    );
  }

  #[cfg(feature = "bitdepth_8")]
  #[test]
  fn checked_block_blend_preserves_overlap_direction_and_signed_stride() -> Rav1dResult {
    let cases = [
      (BlendAxis::Horizontal, [[64; 4], [74; 4], [92; 4], [114; 4]]),
      (BlendAxis::Vertical, [[114, 92, 74, 64]; 4]),
    ];
    let extent = Size {
      width: 4,
      height: 4,
    };
    let prediction = [192_u8; 16];

    for (axis, expected) in cases {
      let mut storage: Align64<[u8; 64]> = Aligned([77; 64]);

      for row in storage.chunks_exact_mut(8).take(4) {
        row.iter_mut().take(4).for_each(|pixel| *pixel = 64);
      }

      let Ok(bottom_up) = console_core_number_conversion::wrapped_index(-8_i64);
      let picture = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut *storage, bottom_up);
      let dst = PicOffset {
        data: &picture,
        offset: 24,
      };
      let plan =
        BlockBlend::new::<BitDepth8>(dst, extent, BlockBlendMask::Overlap(axis), &prediction)?;
      let plan_2 = plan.ok_or(EINVAL)?;
      plan_2.apply::<BitDepth8>(dst, &prediction)?;
      let samples = picture.slice::<BitDepth8, _>(..);

      for (row, pixels) in samples.chunks_exact(8).zip(expected) {
        assert!(row.iter().copied().take(4).eq(pixels));
        assert!(row.iter().skip(4).all(|&sample| sample == 77));
      }

      assert!(samples.iter().skip(32).all(|&sample| sample == 77));
    }

    Ok(())
  }
}

#[cfg(test)]
mod scaled_sampling_tests {
  use super::*;
  use crate::include::common::bitdepth::{BitDepth8, BitDepth16};
  use crate::src::align::Align64;
  use aligned::Aligned;
  use console_core_checksums::{Checksum, md5::Md5};

  #[test]
  fn intermediate_filters_reject_an_incomplete_window_or_column() {
    let rows = [[7_i16; MID_STRIDE]; 8];
    let incomplete = [[7_i16; MID_STRIDE]; 1];
    let taps = [0, 0, 0, 64, 0, 0, 0, 0];
    let present = SubpixelPosition {
      index: 127,
      phase: 7,
    };
    let outside = SubpixelPosition {
      index: 128,
      phase: 7,
    };

    assert_eq!(filter_8tap_mid(&rows, present.index, &taps), Ok(448));
    assert_eq!(filter_bilin_mid(&rows, present), Ok(112));
    assert_eq!(filter_8tap_mid(&rows, outside.index, &taps), Err(EINVAL));
    assert_eq!(filter_bilin_mid(&rows, outside), Err(EINVAL));
    assert_eq!(
      filter_8tap_mid(&incomplete, present.index, &taps),
      Err(EINVAL)
    );
    assert_eq!(filter_bilin_mid(&incomplete, present), Err(EINVAL));
  }

  fn check_sampling<BD: BitDepth>(bit_depth: BD, expected: [u8; 16]) -> Rav1dResult {
    let max = 1_i32
      .wrapping_shl(u32::from(bit_depth.bitdepth()))
      .wrapping_sub(1);
    let mut reference: Align64<[BD::Pixel; 4096]> = Aligned(core::array::from_fn(|index| {
      let Ok(sample) = console_core_number_conversion::wrapped::<_, i32>(index);
      let mixed = sample.wrapping_mul(477) ^ sample.wrapping_mul(9511).wrapping_shr(7);
      bit_depth.iclip_pixel(mixed & max)
    }));
    let plane = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut *reference, 64);
    let source = PicOffset {
      data: &plane,
      offset: 520,
    };
    let mut checksum = Md5::default();

    for filter in [
      Filter2d::Regular8Tap,
      Filter2d::SmoothSharp8Tap,
      Filter2d::Bilinear,
    ] {
      for (phase, step) in [
        (Point { x: 0, y: 0 }, Point { x: 1024, y: 1024 }),
        (Point { x: 777, y: 333 }, Point { x: 1733, y: 1597 }),
        (Point { x: 1023, y: 63 }, Point { x: 736, y: 1029 }),
      ] {
        let sampling = ScaledSampling {
          size: Size {
            width: 16,
            height: 12,
          },
          phase,
          step,
        };
        let mut output: Align64<[BD::Pixel; 512]> = Aligned([bit_depth.iclip_pixel(31); 512]);
        let output_plane = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut *output, 32);
        let dst = PicOffset {
          data: &output_plane,
          offset: 0,
        };
        let mut intermediate = [41_i16; 256];
        mc_scaled::Fn::DEFAULT.call(filter, dst, source, sampling, bit_depth)?;
        mct_scaled::Fn::DEFAULT.call(filter, &mut intermediate, source, sampling, bit_depth)?;

        for pixel in dst.slice::<BD>(512).iter().copied() {
          let value = <BD::Pixel as Into<u16>>::into(pixel);
          let Ok(()) = checksum.process_buf_bytes(&value.to_le_bytes());
        }

        for pixel in intermediate {
          let Ok(()) = checksum.process_buf_bytes(&pixel.to_le_bytes());
        }
      }
    }

    let Ok(result) = checksum.md5();
    assert_eq!(result, expected);

    let sampling = ScaledSampling {
      size: Size {
        width: 16,
        height: 12,
      },
      phase: Point { x: 777, y: 333 },
      step: Point { x: 1733, y: 1597 },
    };
    let mut short = [41_i16; 16];
    assert_eq!(
      mct_scaled::Fn::DEFAULT.call(Filter2d::Bilinear, &mut short, source, sampling, bit_depth),
      Err(EINVAL)
    );
    assert_eq!(short, [41_i16; 16]);

    Ok(())
  }

  #[test]
  fn checked_scaled_sampling_matches_legacy_picture_and_prediction_bytes() -> Rav1dResult {
    check_sampling(
      BitDepth8::new(()),
      [
        243, 66, 12, 52, 41, 227, 112, 90, 25, 200, 56, 85, 154, 172, 97, 214,
      ],
    )?;
    check_sampling(
      BitDepth16::new(1023),
      [
        120, 88, 120, 242, 228, 120, 240, 91, 134, 62, 231, 164, 243, 128, 111, 62,
      ],
    )?;
    check_sampling(
      BitDepth16::new(4095),
      [
        136, 241, 205, 108, 255, 197, 165, 212, 39, 186, 70, 255, 206, 111, 184, 208,
      ],
    )
  }
}

#[cfg(test)]
mod unscaled_sampling_tests {
  use super::*;
  use crate::include::common::bitdepth::{BitDepth8, BitDepth16};
  use crate::src::align::Align64;
  use aligned::Aligned;
  use console_core_checksums::{Checksum, md5::Md5};

  fn unscaled_digest<BD: BitDepth>(bit_depth: BD) -> Rav1dResult<[u8; 16]> {
    let max = 1_i32
      .wrapping_shl(u32::from(bit_depth.bitdepth()))
      .wrapping_sub(1);
    let mut reference: Align64<[BD::Pixel; 4096]> = Aligned(core::array::from_fn(|index| {
      let Ok(sample) = console_core_number_conversion::wrapped::<_, i32>(index);
      let mixed = sample.wrapping_mul(477) ^ sample.wrapping_mul(9511).wrapping_shr(7);
      bit_depth.iclip_pixel(mixed & max)
    }));
    let plane = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut *reference, 64);
    let source = PicOffset {
      data: &plane,
      offset: 520,
    };
    let mut checksum = Md5::default();

    for filter in [
      Filter2d::Regular8Tap,
      Filter2d::RegularSmooth8Tap,
      Filter2d::RegularSharp8Tap,
      Filter2d::SharpRegular8Tap,
      Filter2d::SharpSmooth8Tap,
      Filter2d::Sharp8Tap,
      Filter2d::SmoothRegular8Tap,
      Filter2d::Smooth8Tap,
      Filter2d::SmoothSharp8Tap,
      Filter2d::Bilinear,
    ] {
      for (width, height) in [(4, 4), (16, 12), (32, 4)] {
        for (mx, my) in [(0, 0), (13, 0), (0, 11), (15, 1), (7, 12)] {
          let mut output: Align64<[BD::Pixel; 768]> = Aligned([bit_depth.iclip_pixel(31); 768]);
          let output_plane = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut *output, 64);
          let dst = PicOffset {
            data: &output_plane,
            offset: 0,
          };
          let mut intermediate = [41_i16; 256];
          let sampling = UnscaledSampling {
            size: Size { width, height },
            phase: Point { x: mx, y: my },
          };
          let block = UnscaledBlock::new(filter, sampling)?;
          put_unscaled(&block, dst, source, bit_depth)?;
          prep_unscaled(&block, &mut intermediate, source, bit_depth)?;

          for pixel in dst.slice::<BD>(768).iter().copied() {
            let value = <BD::Pixel as Into<u16>>::into(pixel);
            let Ok(()) = checksum.process_buf_bytes(&value.to_le_bytes());
          }

          for pixel in intermediate {
            let Ok(()) = checksum.process_buf_bytes(&pixel.to_le_bytes());
          }
        }
      }
    }

    let Ok(result) = checksum.md5();
    Ok(result)
  }

  #[test]
  fn checked_unscaled_sampling_matches_legacy_picture_and_prediction_bytes() -> Rav1dResult {
    let byte_hash = unscaled_digest(BitDepth8::new(()))?;
    assert_eq!(
      byte_hash,
      [
        0, 82, 0, 10, 41, 206, 175, 86, 164, 44, 76, 17, 95, 38, 225, 125
      ]
    );
    let ten_bit_hash = unscaled_digest(BitDepth16::new(1023))?;
    assert_eq!(
      ten_bit_hash,
      [
        162, 60, 138, 249, 76, 195, 2, 126, 39, 187, 27, 254, 159, 157, 84, 254
      ]
    );
    let twelve_bit_hash = unscaled_digest(BitDepth16::new(4095))?;
    assert_eq!(
      twelve_bit_hash,
      [
        235, 72, 167, 78, 177, 159, 60, 180, 64, 42, 87, 77, 238, 193, 33, 6
      ]
    );
    Ok(())
  }

  #[test]
  fn missing_taps_and_short_predictor_leave_destinations_unchanged() -> Rav1dResult {
    let mut reference: Align64<[u8; 4096]> = Aligned([17; 4096]);
    let mut pixels: Align64<[u8; 512]> = Aligned([39; 512]);
    let source_plane = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut *reference, 64);
    let output_plane = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut *pixels, 64);
    let source = PicOffset {
      data: &source_plane,
      offset: 0,
    };
    let output = PicOffset {
      data: &output_plane,
      offset: 0,
    };
    let sampling = UnscaledSampling {
      size: Size {
        width: 4,
        height: 4,
      },
      phase: Point { x: 9, y: 7 },
    };
    let block = UnscaledBlock::new(Filter2d::Regular8Tap, sampling)?;
    let depth = BitDepth8::new(());
    assert_eq!(put_unscaled(&block, output, source, depth), Err(EINVAL));
    assert_eq!(&*output.slice::<BitDepth8>(512), &[39_u8; 512]);

    let mut short = [42_i16; 8];
    let padded_source = PicOffset {
      data: &source_plane,
      offset: 520,
    };
    assert_eq!(
      prep_unscaled(&block, &mut short, padded_source, depth),
      Err(EINVAL)
    );
    assert_eq!(short, [42_i16; 8]);
    Ok(())
  }
}
