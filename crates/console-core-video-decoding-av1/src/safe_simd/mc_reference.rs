//! Bounded source reservations for the x86 interpolation kernels.
//!
//! These replace whole-component read guards on reference pictures / private
//! edge-emulation buffers. Unlike reconstruction writes, they may cover row
//! gaps: the complete contiguous slice is registered with DisjointMut, so any
//! concurrent write in a gap is still rejected. This is a narrowing of the
//! existing whole-component exemption in `note_pic_extent`, not permission to
//! widen a reconstruction guard. No mutable reference is created here.

use crate::include::common::bitdepth::BitDepth;
use crate::include::dav1d::picture::{PicOffset, Rav1dPictureDataComponentInner};
use crate::src::disjoint_mut::DisjointImmutGuard;
#[cfg(target_arch = "x86_64")]
use crate::src::levels::Filter2d;
use crate::src::strided::Strided as _;
#[cfg(target_arch = "x86_64")]
use console_core_geometry::Point;

type SourceGuard<'a, BD> =
  DisjointImmutGuard<'a, Rav1dPictureDataComponentInner, [<BD as BitDepth>::Pixel]>;

use crate::src::error::{Rav1dError, Rav1dResult};
#[cfg(target_arch = "x86_64")]
use console_core_geometry::Size;
use console_core_number_conversion::{checked_offset, fixed_index, fixed_offset, index, indices};
use console_core_pixel_buffers::RowLayout;
use console_core_video_av1_digital_signal_processing::FilterFootprint;

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
pub(super) struct Sampling {
  pub size: Size<i32>,
  pub phase: Point<i32>,
}

#[cfg(target_arch = "x86_64")]
impl Sampling {
  pub(super) fn checked_size(self) -> Rav1dResult<Size<u64>> {
    match (
      self.size.width,
      self.size.height,
      self.phase.x,
      self.phase.y,
    ) {
      (1..=128, 1..=128, 0..=15, 0..=15) => Ok(Size {
        width: u64::from(self.size.width.cast_unsigned()),
        height: u64::from(self.size.height.cast_unsigned()),
      }),
      _ => Err(Rav1dError::EINVAL),
    }
  }
}

#[cfg(target_arch = "x86_64")]
struct AxisPadding {
  before: u64,
  after: u64,
}

#[cfg(target_arch = "x86_64")]
const WARP_PADDING: AxisPadding = AxisPadding {
  before: 3,
  after: 4,
};

#[cfg(target_arch = "x86_64")]
#[inline]
#[track_caller]
fn read_guard<BD: BitDepth>(
  src: PicOffset<'_>,
  size: Size<u64>,
  padding: [AxisPadding; 2],
) -> Rav1dResult<(SourceGuard<'_, BD>, u64)> {
  let [horizontal, vertical] = padding;
  footprint_guard::<BD>(
    src,
    FilterFootprint {
      before: Point {
        x: horizontal.before,
        y: vertical.before,
      },
      size: Size {
        width: size
          .width
          .wrapping_add(horizontal.before)
          .wrapping_add(horizontal.after),
        height: size
          .height
          .wrapping_add(vertical.before)
          .wrapping_add(vertical.after),
      },
    },
  )
}

#[inline]
#[track_caller]
pub(crate) fn footprint_guard<BD: BitDepth>(
  src: PicOffset<'_>,
  footprint: FilterFootprint,
) -> Rav1dResult<(SourceGuard<'_, BD>, u64)> {
  let Ok(origin) = fixed_index(src.offset);
  let Ok(stride) = fixed_offset(match src.data.pixel_stride::<BD>() {
    Ok(pixel_stride) => pixel_stride,
  });
  let top_left = footprint
    .start(origin, stride)
    .map_err(|_| Rav1dError::EINVAL)?;
  let layout =
    RowLayout::new(footprint.size, top_left, stride).map_err(|_| Rav1dError::EINVAL)?;
  layout
    .validate_len(src.data.pixel_len::<BD>())
    .map_err(|_| Rav1dError::EINVAL)?;
  let span = layout.span();
  let start = span.start;
  let width = footprint.size.width;
  let rows = footprint.size.height;

  let Ok(pixel_size) = fixed_index(core::mem::size_of::<BD::Pixel>());
  let pixel_bytes = i64::try_from(pixel_size).map_err(|_too_wide| Rav1dError::EINVAL)?;
  let byte_stride = stride.checked_mul(pixel_bytes).ok_or(Rav1dError::EINVAL)?;
  let first_byte = top_left.checked_mul(pixel_size).ok_or(Rav1dError::EINVAL)?;
  let visible_bytes = width.checked_mul(pixel_size).ok_or(Rav1dError::EINVAL)?;
  let Ok(range) = indices(span);
  let guard = src.data.dm().slice_as::<_, BD::Pixel>(range);
  guard.probe_declare_rows(
    match index(first_byte) {
      Ok(byte_index) => byte_index,
    },
    match index(visible_bytes) {
      Ok(byte_count) => byte_count,
    },
    match index(rows) {
      Ok(row_count) => row_count,
    },
    match checked_offset(byte_stride) {
      Ok(row_offset) => row_offset,
      Err(_unrepresentable_offset) => return Err(Rav1dError::EINVAL),
    },
  );

  Ok((guard, origin.wrapping_sub(start)))
}

#[cfg(target_arch = "x86_64")]
#[inline]
#[track_caller]
pub(super) fn warp_guard<BD: BitDepth>(
  src: PicOffset<'_>,
) -> Rav1dResult<(SourceGuard<'_, BD>, u64)> {
  read_guard::<BD>(
    src,
    Size {
      width: 8,
      height: 8,
    },
    [WARP_PADDING, WARP_PADDING],
  )
}

#[cfg(target_arch = "x86_64")]
#[inline]
#[track_caller]
pub(super) fn filter_guard<BD: BitDepth>(
  src: PicOffset<'_>,
  filter: Filter2d,
  sampling: Sampling,
) -> Rav1dResult<(SourceGuard<'_, BD>, u64)> {
  let padding = |phase| match (phase, filter) {
    (0, _) => AxisPadding {
      before: 0,
      after: 0,
    },
    (_, Filter2d::Bilinear) => AxisPadding {
      before: 0,
      after: 1,
    },
    _ => WARP_PADDING,
  };
  let size = sampling.checked_size()?;
  read_guard::<BD>(
    src,
    size,
    [padding(sampling.phase.x), padding(sampling.phase.y)],
  )
}

#[cfg(all(test, target_arch = "x86_64", not(feature = "c-ffi")))]
mod tests {
  use super::*;
  use crate::include::common::bitdepth::{BitDepth8, BitDepth16};
  use crate::include::dav1d::picture::Rav1dPictureDataComponent;
  use crate::src::with_offset::WithOffset;
  use std::panic::{AssertUnwindSafe, catch_unwind};
  use zerocopy::IntoBytes;

  #[test]
  fn reference_window_reserves_its_full_hull_including_gaps() -> Rav1dResult {
    for pitch in [64_i64, -64] {
      let inner = Rav1dPictureDataComponentInner::from_slice_copy(&[0; 64 * 32]);
      let Ok(picture) = Rav1dPictureDataComponent::from_parts(inner, pitch);
      let origin = 1040_u64;
      let Ok(offset) = console_core_number_conversion::index(origin);
      let source = WithOffset {
        data: &picture,
        offset,
      };
      let (guard, base) = warp_guard::<BitDepth8>(source)?;
      let start = origin.checked_sub(base).ok_or(Rav1dError::EINVAL)?;
      let Ok(length) = console_core_number_conversion::fixed_index(guard.len());
      let end = start.checked_add(length).ok_or(Rav1dError::EINVAL)?;
      assert_eq!(length, 911);

      for y in -3_i64..12 {
        for x in -3_i64..12 {
          let delta = y.wrapping_mul(pitch).wrapping_add(x);
          let index = origin.checked_add_signed(delta).ok_or(Rav1dError::EINVAL)?;
          assert!(start <= index && index < end);
        }
      }

      #[cfg(not(feature = "unchecked"))]
      for probe in [start, start.wrapping_add(32), end.wrapping_sub(1)] {
        let Ok(index) = console_core_number_conversion::index(probe);
        let overlap = catch_unwind(AssertUnwindSafe(|| picture.index_mut::<BitDepth8>(index)));
        assert!(matches!(overlap, Err(_overlap)));
      }

      let Ok(before) = console_core_number_conversion::index(start.wrapping_sub(1));
      let Ok(after) = console_core_number_conversion::index(end);
      *picture.index_mut::<BitDepth8>(before) = 1;
      *picture.index_mut::<BitDepth8>(after) = 2;
      drop(guard);
      let Ok(first) = console_core_number_conversion::index(start);
      *picture.index_mut::<BitDepth8>(first) = 3;
    }

    Ok(())
  }

  #[test]
  fn warp_reference_windows_cover_both_strides_and_all_depths() -> Rav1dResult {
    use console_core_checksums::{Checksum, md5::Md5};
    let _lock = archmage::testing::lock_token_testing();
    let Ok(avx2) = crate::src::cpu::summon_avx2();

    let token = match avx2 {
      Some(matched_token) => matched_token,
      ::core::option::Option::None => return Ok(()),
    };

    let mut tested = 0_u32;

    for bits in [8_u32, 10, 12] {
      let mut hash = Md5::default();
      let sample_bytes = match bits {
        8 => 1_i64,
        _ => 2,
      };
      let max = 1_u16.wrapping_shl(bits).wrapping_sub(1);
      let pixels: Vec<u16> = (0_u16..2048)
        .map(|index| index.wrapping_mul(73).wrapping_add(index.wrapping_div(7)) & max)
        .collect();
      let bytes: Vec<u8> = match bits {
        8 => pixels
          .into_iter()
          .map(|pixel| {
            let Ok(byte) = console_core_number_conversion::wrapped::<_, u8>(pixel);
            byte
          })
          .collect(),
        _ => pixels.as_bytes().to_vec(),
      };

      for sign in [1_i64, -1] {
        let pitch = sign.wrapping_mul(64).wrapping_mul(sample_bytes);
        let Ok(picture) = Rav1dPictureDataComponent::from_parts(
          Rav1dPictureDataComponentInner::from_slice_copy(&bytes),
          pitch,
        );
        let Ok(origin) = index(1040_u32);
        let source = WithOffset {
          data: &picture,
          offset: origin,
        };

        for coefficients in [[0; 4], [64, -64, 32, -32]] {
          let phases = [-1024, 0, 1024]
            .into_iter()
            .flat_map(|x| [-1024, 0, 1024].into_iter().map(move |y| Point { x, y }));

          for phase in phases {
            let sample = WarpFixture {
              coefficients,
              phase,
              stride: pitch,
            };
            let (pixels_2, predictors) = match bits {
              8 => check_warp(token, source, sample, BitDepth8::new(()))?,
              _ => check_warp(token, source, sample, BitDepth16::new(max))?,
            };
            let Ok(()) = hash.process_buf_bytes(&pixels_2);

            for value in predictors {
              let Ok(()) = hash.process_buf_bytes(&value.to_le_bytes());
            }

            tested = tested.wrapping_add(1);
          }
        }
      }

      let Ok(digest) = hash.md5();
      let expected = match bits {
        8 => [
          200, 0, 157, 32, 249, 150, 123, 29, 99, 31, 58, 18, 109, 201, 211, 197,
        ],
        10 => [
          178, 50, 115, 82, 11, 194, 168, 51, 69, 185, 83, 114, 170, 207, 86, 246,
        ],
        12 => [
          14, 64, 32, 218, 243, 215, 53, 83, 166, 89, 79, 118, 243, 71, 103, 76,
        ],
        _ => return Err(Rav1dError::EINVAL),
      };
      assert_eq!(digest, expected);
    }

    assert_eq!(tested, 108);
    Ok(())
  }

  struct WarpFixture {
    coefficients: [i16; 4],
    phase: Point<i32>,
    stride: i64,
  }

  fn check_warp<BD: BitDepth>(
    token: archmage::Desktop64,
    source: PicOffset,
    sample: WarpFixture,
    depth: BD,
  ) -> Rav1dResult<([u8; 128], [i16; 64])> {
    use super::super as kernels;
    let (tight, base) = warp_guard::<BD>(source)?;
    let (full, origin) = match source.full_guard::<BD>() {
      (guard, first) => {
        let Ok(position) = fixed_index(first);
        (guard, position)
      }
    };
    let mut put = [[0_u8; 128]; 2];
    let mut prep = [[0_i16; 64]; 2];
    let Ok(pixel_size) = fixed_index(core::mem::size_of::<BD::Pixel>());
    let sampling = kernels::WarpSampling {
      coefficients: sample.coefficients,
      phase: sample.phase,
    };
    let output_stride =
      i64::try_from(pixel_size.wrapping_mul(8)).map_err(|_| Rav1dError::EINVAL)?;

    for ((bytes, first), (pixels, predictors)) in
      [(tight.as_bytes(), base), (full.as_bytes(), origin)]
        .into_iter()
        .zip(put.iter_mut().zip(&mut prep))
    {
      let reference = kernels::PixelSource {
        samples: bytes,
        origin: first.wrapping_mul(pixel_size),
        stride_bytes: sample.stride,
      };
      let output = kernels::PixelDestination {
        bytes: pixels,
        origin: 0,
        stride: output_stride,
      };
      kernels::warp_put(token, reference, sampling, output, depth)?;
      kernels::warp_prep(token, reference, sampling, predictors, 8, depth)?;
    }

    let [tight_pixels, full_pixels] = put;
    let [tight_predictors, full_predictors] = prep;
    assert_eq!(tight_pixels, full_pixels);
    assert_eq!(tight_predictors, full_predictors);

    let mut scalar_pixels = [BD::Pixel::from(0_u8); 64];
    let scalar_plane = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut scalar_pixels, 8);
    let scalar_picture = WithOffset {
      data: &scalar_plane,
      offset: 0,
    };
    let mut scalar_predictors = [0_i16; 64];
    crate::src::mc::warp_affine_8x8_rust(scalar_picture, source, sampling, depth)?;
    crate::src::mc::warp_affine_8x8t_rust(&mut scalar_predictors, 8, source, sampling, depth)?;
    let decoded = scalar_picture.slice::<BD>(64);
    let bytes = decoded.as_bytes();
    let expected = tight_pixels.get(..bytes.len()).ok_or(Rav1dError::EINVAL)?;
    assert_eq!(bytes, expected);
    assert_eq!(scalar_predictors, tight_predictors);
    Ok((tight_pixels, tight_predictors))
  }

  #[test]
  fn a_late_invalid_warp_phase_leaves_both_outputs_untouched() {
    use super::super as kernels;
    let _lock = archmage::testing::lock_token_testing();
    let Ok(avx2) = crate::src::cpu::summon_avx2();

    let token = match avx2 {
      Some(matched_token) => matched_token,
      ::core::option::Option::None => return,
    };

    let storage = [17_u8; 240];
    let reference = kernels::PixelSource {
      samples: &storage,
      origin: 51,
      stride_bytes: 16,
    };
    let sampling = kernels::WarpSampling {
      coefficients: [0, 0, 0, 4096],
      phase: Point { x: 0, y: 128_000 },
    };
    let mut pixels = [31_u8; 64];
    let mut predictors = [42_i16; 64];
    let output = kernels::PixelDestination {
      bytes: &mut pixels,
      origin: 0,
      stride: 8,
    };
    let depth = BitDepth8::new(());
    assert_eq!(
      kernels::warp_put(token, reference, sampling, output, depth),
      Err(Rav1dError::EINVAL)
    );
    assert_eq!(
      kernels::warp_prep(token, reference, sampling, &mut predictors, 8, depth),
      Err(Rav1dError::EINVAL)
    );
    assert_eq!(pixels, [31; 64]);
    assert_eq!(predictors, [42; 64]);
  }

  #[test]
  fn mc_reference_windows_cover_all_filters_phases_widths_and_depths() {
    let _lock = archmage::testing::lock_token_testing();
    let Ok(avx2) = crate::src::cpu::summon_avx2();

    let _available = match avx2 {
      Some(matched_available) => matched_available,
      ::core::option::Option::None => return,
    };

    let tested = std::cell::Cell::new([false, false]);
    let report = archmage::testing::for_each_token_permutation(
      archmage::testing::CompileTimePolicy::WarnStderr,
      |_| {
        let Ok(avx2_2) = crate::src::cpu::summon_avx2();

        let token = match avx2_2 {
          Some(matched_token) => matched_token,
          ::core::option::Option::None => return,
        };

        let Ok(available) = crate::src::cpu::summon_avx512();
        let [avx2_tested, avx512_tested] = tested.get();

        match (available.is_some(), avx2_tested, avx512_tested) {
          (true, _, true) | (false, true, _) => return,
          (true, baseline, false) => tested.set([baseline, true]),
          (false, false, wide) => tested.set([true, wide]),
        }

        assert_eq!(check_filter_cases(token, BitDepth8::new(())), Ok(17_920));
        assert_eq!(check_filter_cases(token, BitDepth16::new(1023)), Ok(17_920));
        assert_eq!(check_filter_cases(token, BitDepth16::new(4095)), Ok(17_920));
      },
    );
    let [avx2_tested, avx512_tested] = tested.get();
    assert!(report.permutations_run > 0 && (avx2_tested || avx512_tested));
  }

  const FIXTURE_PIXEL_STRIDE: u32 = 192;

  struct FilterFixtureOutputs {
    pixels: [crate::src::align::Align64<[u8; 32_768]>; 2],
    predictors: [[i16; 16_384]; 2],
  }

  fn check_filter_cases<BD: BitDepth>(token: archmage::Desktop64, depth: BD) -> Rav1dResult<u32> {
    use crate::include::common::bitdepth::AsPrimitive;
    use aligned::Aligned;
    let max = depth.bitdepth_max().as_::<u32>();
    let mut pixels: Vec<BD::Pixel> = (0_u32..27_648)
      .map(|index| {
        let pattern = index.wrapping_mul(73).wrapping_add(index.wrapping_div(7));
        (pattern & max).as_::<BD::Pixel>()
      })
      .collect();
    let Ok(stride) = index(FIXTURE_PIXEL_STRIDE);
    let picture = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut pixels, stride);
    let Ok(offset) = index(776_u32);
    let source = WithOffset {
      data: &picture,
      offset,
    };
    let mut buffers = FilterFixtureOutputs {
      pixels: [Aligned([0; 32_768]), Aligned([0; 32_768])],
      predictors: [[0; 16_384]; 2],
    };
    let mut tested = 0_u32;

    for (width, height) in [
      (2, 2),
      (4, 4),
      (8, 16),
      (16, 8),
      (32, 32),
      (64, 4),
      (128, 128),
    ] {
      let size = Size { width, height };

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
        let phases = (0..16).flat_map(|x| (0..16).map(move |y| Point { x, y }));

        for phase in phases {
          check_filter_case(
            token,
            filter,
            Sampling { size, phase },
            source,
            depth,
            &mut buffers,
          )?;
          tested = tested.wrapping_add(1);
        }
      }
    }

    Ok(tested)
  }

  fn check_filter_case<BD: BitDepth>(
    token: archmage::Desktop64,
    filter: Filter2d,
    sampling: Sampling,
    source: PicOffset,
    depth: BD,
    buffers: &mut FilterFixtureOutputs,
  ) -> Rav1dResult {
    use crate::include::common::bitdepth::BPC;
    use crate::src::enum_map::EnumKey;
    let width = u32::try_from(sampling.size.width).map_err(|_| Rav1dError::EINVAL)?;
    let height = u32::try_from(sampling.size.height).map_err(|_| Rav1dError::EINVAL)?;
    let area = width.checked_mul(height).ok_or(Rav1dError::EINVAL)?;
    let bytes = match BD::BPC {
      BPC::BPC8 => area,
      BPC::BPC16 => area.wrapping_mul(2),
    };
    let Ok(byte_len) = index(bytes);
    let Ok(pixel_len) = index(area);
    let [tight_put, full_put] = &mut buffers.pixels;
    let [tight_prep, full_prep] = &mut buffers.predictors;
    let tight_output = tight_put.get_mut(..byte_len).ok_or(Rav1dError::EINVAL)?;
    let full_output = full_put.get_mut(..byte_len).ok_or(Rav1dError::EINVAL)?;
    let tight_predictors = tight_prep.get_mut(..pixel_len).ok_or(Rav1dError::EINVAL)?;
    let full_predictors = full_prep.get_mut(..pixel_len).ok_or(Rav1dError::EINVAL)?;
    tight_output.fill(0);
    full_output.fill(0);
    tight_predictors.fill(0);
    full_predictors.fill(0);
    let (tight, base) = filter_guard::<BD>(source, filter, sampling)?;
    let view = PixelWindow {
      bytes: tight.as_bytes(),
      base,
    };
    run_filter_case(
      token,
      filter,
      sampling,
      view,
      tight_output,
      tight_predictors,
      depth,
    )?;
    let (full, origin) = match source.full_guard::<BD>() {
      (guard, position) => {
        let Ok(relative) = fixed_index(position);
        (guard, relative)
      }
    };
    let comparison = PixelWindow {
      bytes: full.as_bytes(),
      base: origin,
    };
    run_filter_case(
      token,
      filter,
      sampling,
      comparison,
      full_output,
      full_predictors,
      depth,
    )?;
    let filter_code = filter.ordinal();
    assert_eq!(
      tight_output, full_output,
      "put {filter_code} {:?} {:?}",
      sampling.size, sampling.phase
    );
    assert_eq!(
      tight_predictors, full_predictors,
      "prep {filter_code} {:?} {:?}",
      sampling.size, sampling.phase
    );
    Ok(())
  }

  struct PixelWindow<'a> {
    bytes: &'a [u8],
    base: u64,
  }

  fn run_filter_case<BD: BitDepth>(
    token: archmage::Desktop64,
    filter: Filter2d,
    sampling: Sampling,
    source: PixelWindow<'_>,
    put: &mut [u8],
    prep: &mut [i16],
    depth: BD,
  ) -> Rav1dResult {
    use super::super as kernels;
    use crate::include::common::bitdepth::BPC;
    let Sampling { size, phase } = sampling;
    let dst_pitch = i64::from(size.width);
    let Ok((horizontal, vertical)) = filter.hv();

    match (BD::BPC, filter) {
      (BPC::BPC8, Filter2d::Bilinear) => {
        let input = kernels::PixelSource {
          samples: source.bytes,
          origin: source.base,
          stride_bytes: i64::from(FIXTURE_PIXEL_STRIDE),
        };
        let stride = i64::from(size.width);
        kernels::put_bilin_8bpc_dispatch_inner(
          token,
          kernels::BilinearRequest {
            destination: put,
            stride,
            source: input,
            sampling,
          },
        )?;
        kernels::prep_bilin_8bpc_dispatch_inner(
          token,
          kernels::BilinearRequest {
            destination: prep,
            stride,
            source: input,
            sampling,
          },
        )
      }
      (BPC::BPC8, _) => {
        use crate::src::mc::{Rav1dMCDSPContext, UnscaledBlock, UnscaledSampling};
        use console_core_video_av1_digital_signal_processing::{
          MotionScratch, PlaneMut, Prediction,
        };
        use zerocopy::FromZeros;

        let Ok(flags) = crate::src::cpu::rav1d_get_cpu_flags();
        let table = Rav1dMCDSPContext::init(flags);
        let block = UnscaledBlock::new(filter, UnscaledSampling { size, phase })?;
        let Ok(taps) = block.resolved_eight_tap();
        let mut scratch = MotionScratch::new_zeroed();
        let extent = sampling.checked_size()?;
        let pitch = i64::from(FIXTURE_PIXEL_STRIDE);
        let put = PlaneMut::new(put, extent, 0, dst_pitch).map_err(|_| Rav1dError::EINVAL)?;
        let put = Prediction::new(put, source.bytes, source.base, pitch, taps)
          .map_err(|_| Rav1dError::EINVAL)?;
        (table.put)(put, &mut scratch).map_err(|_| Rav1dError::EINVAL)?;
        let prep = PlaneMut::new(prep, extent, 0, dst_pitch).map_err(|_| Rav1dError::EINVAL)?;
        let prep = Prediction::new(prep, source.bytes, source.base, pitch, taps)
          .map_err(|_| Rav1dError::EINVAL)?;
        (table.prep)(prep, &mut scratch).map_err(|_| Rav1dError::EINVAL)
      }
      (BPC::BPC16, filter_2) => {
        let pixel_input = <[u16] as zerocopy::FromBytes>::ref_from_bytes(source.bytes)
          .map_err(|_| Rav1dError::EINVAL)?;
        let pixels =
          <[u16] as zerocopy::FromBytes>::mut_from_bytes(put).map_err(|_| Rav1dError::EINVAL)?;
        let max = depth.into_c();

        match filter_2 {
          Filter2d::Bilinear => {
            let input_plane = kernels::PixelSource {
              samples: pixel_input,
              origin: source.base,
              stride_bytes: i64::from(FIXTURE_PIXEL_STRIDE).wrapping_mul(2),
            };
            let stride = i64::from(size.width);
            let picture = kernels::BilinearRequest {
              destination: pixels,
              stride,
              source: input_plane,
              sampling,
            };
            kernels::put_bilin_16bpc_dispatch_inner(token, picture, max)?;
            let prediction = kernels::BilinearRequest {
              destination: prep,
              stride,
              source: input_plane,
              sampling,
            };
            kernels::prep_word_bilinear_dispatch(token, prediction)
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
            let dst_bytes = i64::from(size.width).wrapping_mul(2);
            kernels::put_8tap_16bpc_dispatch_inner(
              token,
              pixels,
              dst_bytes,
              kernels::PixelSource {
                samples: pixel_input,
                origin: source.base,
                stride_bytes: i64::from(FIXTURE_PIXEL_STRIDE).wrapping_mul(2),
              },
              size,
              phase,
              max,
              Point {
                x: horizontal,
                y: vertical,
              },
            )?;
            kernels::prep_8tap_16bpc_dispatch_inner(
              token,
              prep,
              kernels::PixelSource {
                samples: pixel_input,
                origin: source.base,
                stride_bytes: i64::from(FIXTURE_PIXEL_STRIDE).wrapping_mul(2),
              },
              size,
              phase,
              max,
              Point {
                x: horizontal,
                y: vertical,
              },
            )
          }
        }
      }
    }
  }
}
