use super::{Rav1dMCDSPContext, UnscaledBlock, UnscaledSampling};
use crate::src::cpu::{CpuFlags, flags_for_mask};
use crate::src::enum_map::EnumKey;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use crate::src::levels::Filter2d;
use console_core_geometry::{Point, Size};
use console_core_pixel_buffers::{RowLayout, RowLayoutError};
use console_core_video_av1_digital_signal_processing::{
  EightTap, MotionCompensation, MotionScratch, PlaneMut, Prediction, read_extent,
};
use zerocopy::FromZeros;

const FILTERS: [Filter2d; 9] = [
  Filter2d::Regular8Tap,
  Filter2d::RegularSmooth8Tap,
  Filter2d::RegularSharp8Tap,
  Filter2d::SmoothRegular8Tap,
  Filter2d::Smooth8Tap,
  Filter2d::SmoothSharp8Tap,
  Filter2d::SharpRegular8Tap,
  Filter2d::SharpSmooth8Tap,
  Filter2d::Sharp8Tap,
];

struct Fixture {
  input: [Vec<u8>; 2],
  origin: u64,
  pitch: i64,
  extent: Size<u64>,
  filter: EightTap,
  output_origin: u64,
  output_pitch: i64,
  output_length: usize,
}

impl Fixture {
  fn new(sampling: UnscaledSampling, filter: Filter2d, reverse_rows: bool) -> Rav1dResult<Self> {
    let block = UnscaledBlock::new(filter, sampling)?;
    let Ok(filter) = block.resolved_eight_tap();
    let size = Size {
      width: u64::from(sampling.size.width.cast_unsigned()),
      height: u64::from(sampling.size.height.cast_unsigned()),
    };
    let Ok(axes) = filter.filtered_axes();
    let footprint = read_extent(size, axes).map_err(|_| EINVAL)?;
    let stride = footprint.size.width | 1;
    let stride = stride.wrapping_add(4);
    let output_stride = (size.width | 1).wrapping_add(6);
    let (pitch, output_pitch, first, output_origin) = match reverse_rows {
      false => (stride.cast_signed(), output_stride.cast_signed(), 5, 3),
      true => (
        stride.cast_signed().wrapping_neg(),
        output_stride.cast_signed().wrapping_neg(),
        5 + (footprint.size.height - 1) * stride,
        3 + (size.height - 1) * output_stride,
      ),
    };
    let origin = first
      .wrapping_add_signed(footprint.before.y.cast_signed().wrapping_mul(pitch))
      .wrapping_add(footprint.before.x);
    let Ok(input_length) =
      console_core_number_conversion::index(footprint.size.height * stride + 10);
    let Ok(output_length) =
      console_core_number_conversion::index(size.height * output_stride + 12);
    let mut input = [vec![0x42; input_length], vec![0xa9; input_length]];
    let [first_input, second_input] = &mut input;
    let input_layout = RowLayout::new(footprint.size, first, pitch).map_err(|_| EINVAL)?;
    let first_rows = input_layout.rows_mut(first_input).map_err(|_| EINVAL)?;
    let second_rows = input_layout.rows_mut(second_input).map_err(|_| EINVAL)?;
    let mut seed = 0xea06_c541_u32;

    for (a, b) in first_rows.zip(second_rows) {
      for (a, b) in a.iter_mut().zip(b) {
        seed ^= seed.wrapping_shl(13);
        seed ^= seed.wrapping_shr(17);
        seed ^= seed.wrapping_shl(5);
        let [sample, ..] = seed.to_le_bytes();
        *a = sample;
        *b = sample;
      }
    }

    Ok(Self {
      input,
      origin,
      pitch,
      extent: size,
      filter,
      output_origin,
      output_pitch,
      output_length,
    })
  }

  fn render(
    &self,
    table: MotionCompensation,
    input: &[u8],
    scratch: &mut MotionScratch,
  ) -> Result<(Vec<u8>, Vec<i16>), RowLayoutError> {
    let mut pixels = vec![123; self.output_length];
    let mut predictors = vec![-30_000; self.output_length];
    let output = PlaneMut::new(
      &mut pixels,
      self.extent,
      self.output_origin,
      self.output_pitch,
    )?;
    let request = Prediction::new(output, input, self.origin, self.pitch, self.filter)?;
    (table.put)(request, scratch)?;
    let output = PlaneMut::new(
      &mut predictors,
      self.extent,
      self.output_origin,
      self.output_pitch,
    )?;
    let request = Prediction::new(output, input, self.origin, self.pitch, self.filter)?;
    (table.prep)(request, scratch)?;

    Ok((pixels, predictors))
  }

  fn check(
    &self,
    backends: &[(&str, MotionCompensation)],
    scratch: &mut MotionScratch,
    filter: Filter2d,
    sampling: UnscaledSampling,
  ) -> Rav1dResult {
    let [input, with_other_padding] = &self.input;
    let expected = self
      .render(MotionCompensation::default(), input, scratch)
      .map_err(|_| EINVAL)?;
    let mut active = vec![false; self.output_length];
    let layout =
      RowLayout::new(self.extent, self.output_origin, self.output_pitch).map_err(|_| EINVAL)?;
    for row in layout.rows_mut(&mut active).map_err(|_| EINVAL)? {
      row.fill(true);
    }
    assert!(
      active
        .iter()
        .zip(expected.0.iter().zip(&expected.1))
        .all(|(&is_active, (&byte, &word))| { is_active || (byte == 123 && word == -30_000) })
    );

    for &(backend, table) in backends {
      let actual = self
        .render(table, with_other_padding, scratch)
        .map_err(|_| EINVAL)?;
      assert_eq!(
        actual,
        expected,
        "{backend}, filter {}, size {:?}, phase {:?}, pitch {}",
        filter.ordinal(),
        sampling.size,
        sampling.phase,
        self.pitch
      );
    }

    Ok(())
  }
}

#[test]
fn eight_tap_backends_match_scalar_over_phases_filters_and_odd_strides() -> Rav1dResult {
  let mut backends = vec![("scalar", MotionCompensation::default())];
  #[cfg(target_arch = "x86_64")]
  let candidates = [("AVX2", CpuFlags::AVX2), ("AVX-512", CpuFlags::AVX512ICL)];
  #[cfg(target_arch = "aarch64")]
  let candidates = [("NEON", CpuFlags::NEON)];
  #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
  let candidates: [(&str, CpuFlags); 0] = [];
  let scalar = MotionCompensation::default();
  for (name, mask) in candidates {
    let Ok(flags) = flags_for_mask(mask.bits());
    let table = Rav1dMCDSPContext::init(flags);
    match core::ptr::fn_addr_eq(table.put, scalar.put) {
      true => {}
      false => backends.push((name, table)),
    }
  }
  let mut scratch = MotionScratch::new_box_zeroed().map_err(|_| EINVAL)?;
  for filter in FILTERS {
    for size in [
      Size {
        width: 2,
        height: 8,
      },
      Size {
        width: 8,
        height: 4,
      },
    ] {
      for x in 0..16 {
        for y in 0..16 {
          let sampling = UnscaledSampling {
            size,
            phase: Point { x, y },
          };
          let fixture = Fixture::new(sampling, filter, (x + y) % 2 == 0)?;
          fixture.check(&backends, &mut scratch, filter, sampling)?;
        }
      }
    }

    for width in [4, 16, 23, 31, 32, 63, 64, 127, 128] {
      for (x, y) in [(0, 0), (0, 7), (1, 0), (3, 10), (15, 15)] {
        let size = Size {
          width,
          height: width,
        };
        let sampling = UnscaledSampling {
          size,
          phase: Point { x, y },
        };
        for reverse_rows in [false, true] {
          let fixture = Fixture::new(sampling, filter, reverse_rows)?;
          fixture.check(&backends, &mut scratch, filter, sampling)?;
        }
      }
    }
  }

  eprintln!(
    "Checked: {:?}",
    backends.iter().map(|(name, _)| *name).collect::<Vec<_>>()
  );
  Ok(())
}

#[test]
fn eight_tap_rejects_truncated_source_before_writing() -> Result<(), RowLayoutError> {
  let filter = EightTap {
    horizontal: console_core_video_av1_digital_signal_processing::taps(
      console_core_video_av1_digital_signal_processing::TapTable::Sharp,
      4,
    )
    .unwrap(),
    vertical: None,
  };
  let extent = Size {
    width: 4,
    height: 1,
  };
  let mut pixels = [0x27; 4];
  let target = PlaneMut::new(&mut pixels, extent, 0, 4)?;
  assert!(matches!(
    Prediction::new(target, &[0x91; 10], 3, 11, filter),
    Err(RowLayoutError::Bounds)
  ));
  assert_eq!(pixels, [0x27; 4]);
  Ok(())
}
