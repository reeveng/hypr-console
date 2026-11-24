#![forbid(unsafe_code)]
use crate::include::common::bitdepth::AsPrimitive;
use crate::include::common::bitdepth::BitDepth;
use crate::src::align::AlignedVec64;
use crate::src::disjoint_mut::DisjointMut;
use crate::src::env::{BlockContext, read_context};
use crate::src::error::{Rav1dError, Rav1dResult};
use crate::src::intra_edge::{EdgeAvailability, EdgeFlags};
use crate::src::levels::DC_128_PRED;
use crate::src::levels::DC_PRED;
use crate::src::levels::DIAG_DOWN_LEFT_PRED;
use crate::src::levels::DIAG_DOWN_RIGHT_PRED;
use crate::src::levels::FILTER_PRED;
use crate::src::levels::HOR_DOWN_PRED;
use crate::src::levels::HOR_PRED;
use crate::src::levels::HOR_UP_PRED;
use crate::src::levels::IntraPredMode;
use crate::src::levels::LEFT_DC_PRED;
use crate::src::levels::PAETH_PRED;
use crate::src::levels::SMOOTH_H_PRED;
use crate::src::levels::SMOOTH_PRED;
use crate::src::levels::SMOOTH_V_PRED;
use crate::src::levels::TOP_DC_PRED;
use crate::src::levels::VERT_LEFT_PRED;
use crate::src::levels::VERT_PRED;
use crate::src::levels::VERT_RIGHT_PRED;
use crate::src::levels::Z1_PRED;
use crate::src::levels::Z2_PRED;
use crate::src::levels::Z3_PRED;
use crate::src::owned_recon::ReconSrc;
use crate::src::strided::Strided as _;
use console_core_geometry::{Point, Size};
use console_core_number_conversion::{
  fixed_index, fixed_offset, index, indices, wrapped, wrapped_offset,
};
use core::ops::Range;
use std::ffi::c_int;

#[inline]
fn smooth(m: u8) -> core::result::Result<c_int, console_core_never::Never> {
  Ok({
    match ::core::primitive::bool::then_some(
      m == SMOOTH_PRED || m == SMOOTH_H_PRED || m == SMOOTH_V_PRED,
      (),
    ) {
      ::core::option::Option::Some(()) => 512,
      ::core::option::Option::None => 0,
    }
  })
}

#[inline]
pub(crate) fn sm_flag(b: &BlockContext, position: i32) -> Rav1dResult<c_int> {
  let intra = read_context(&b.intra, position)?;

  match intra {
    0 => Ok(0),
    _ => {
      let mode = read_context(&b.mode, position)?;
      let Ok(flag) = smooth(mode);
      Ok(flag)
    }
  }
}

#[inline]
pub(crate) fn sm_uv_flag(b: &BlockContext, position: i32) -> Rav1dResult<c_int> {
  let mode = read_context(&b.uvmode, position)?;
  let Ok(flag) = smooth(mode);
  Ok(flag)
}

const LEFT_EDGE: u8 = 1;
const TOP_EDGE: u8 = 1 << 1;
const CORNER_EDGE: u8 = 1 << 2;
const TOP_RIGHT_EDGE: u8 = 1 << 3;
const BOTTOM_LEFT_EDGE: u8 = 1 << 4;

fn edge_requirements(mode: IntraPredMode) -> Result<u8, console_core_never::Never> {
  Ok(match mode {
    DC_PRED | SMOOTH_PRED | SMOOTH_H_PRED | SMOOTH_V_PRED => LEFT_EDGE | TOP_EDGE,
    VERT_PRED | TOP_DC_PRED => TOP_EDGE,
    HOR_PRED | LEFT_DC_PRED => LEFT_EDGE,
    Z1_PRED => TOP_EDGE | TOP_RIGHT_EDGE | CORNER_EDGE,
    Z2_PRED | PAETH_PRED | FILTER_PRED => TOP_EDGE | LEFT_EDGE | CORNER_EDGE,
    Z3_PRED => LEFT_EDGE | BOTTOM_LEFT_EDGE | CORNER_EDGE,
    _ => 0,
  })
}

pub(crate) struct IntraEdgePreparation<'buffer> {
  pub(crate) position: Point<i32>,
  pub(crate) frame: Size<i32>,
  pub(crate) transform: Size<i32>,
  pub(crate) has_left: bool,
  pub(crate) above: AboveEdge<'buffer>,
  pub(crate) available: EdgeFlags,
  pub(crate) filter_corner: u8,
}

pub(crate) enum AboveEdge<'buffer> {
  Absent,
  Reconstructed,
  Prefiltered(PrefilteredTop<'buffer>),
}

impl<'buffer> AboveEdge<'buffer> {
  pub(crate) fn select(
    availability: EdgeAvailability,
    prefiltered: Option<PrefilteredTop<'buffer>>,
  ) -> Result<Self, console_core_never::Never> {
    Ok(match (availability, prefiltered) {
      (EdgeAvailability::Absent, _) => Self::Absent,
      (EdgeAvailability::Present, None) => Self::Reconstructed,
      (EdgeAvailability::Present, Some(edge)) => Self::Prefiltered(edge),
    })
  }
}

pub(crate) struct PrefilteredTop<'buffer> {
  pub(crate) samples: &'buffer DisjointMut<AlignedVec64<u8>>,
  pub(crate) row_start: i64,
}

pub(crate) struct IntraEdgeDestination<'samples, Pixel> {
  pub(crate) samples: &'samples mut [Pixel],
  pub(crate) center: u64,
}

#[cfg(kani)]
#[path = "../proofs/intra_prediction.rs"]
mod intra_prediction_proofs;

fn prepare_intra_mode(
  config: &IntraEdgePreparation<'_>,
  mut mode: IntraPredMode,
  angle: &mut c_int,
) -> Result<IntraPredMode, console_core_never::Never> {
  let has_top = !matches!(&config.above, AboveEdge::Absent);
  let has_left = config.has_left;

  match mode {
    VERT_PRED..=VERT_LEFT_PRED => {
      let base_angle: i32 = match mode {
        VERT_PRED => 90,
        HOR_PRED => 180,
        DIAG_DOWN_LEFT_PRED => 45,
        DIAG_DOWN_RIGHT_PRED => 135,
        VERT_RIGHT_PRED => 113,
        HOR_DOWN_PRED => 157,
        HOR_UP_PRED => 203,
        VERT_LEFT_PRED => 67,
        _ => return Ok(mode),
      };
      *angle = base_angle.wrapping_add(3i32.wrapping_mul(*angle));

      mode = match (*angle, has_top, has_left) {
        (..=89, true, _) => Z1_PRED,
        (..=90, _, _) => VERT_PRED,
        (91..=179, _, _) => Z2_PRED,
        (181.., _, true) => Z3_PRED,
        _ => HOR_PRED,
      };
    }
    DC_PRED | PAETH_PRED => {
      mode = match (has_left, has_top, mode) {
        (false, false, _) => DC_128_PRED,
        (false, true, DC_PRED) => TOP_DC_PRED,
        (false, true, _) => VERT_PRED,
        (true, false, DC_PRED) => LEFT_DC_PRED,
        (true, false, _) => HOR_PRED,
        (true, true, unchanged) => unchanged,
      };
    }
    _ => {}
  }

  Ok(mode)
}

fn edge_range<Pixel>(samples: &mut [Pixel], range: Range<u64>) -> Rav1dResult<&mut [Pixel]> {
  let Ok(native_range) = indices(range);
  samples.get_mut(native_range).ok_or(Rav1dError::EINVAL)
}

fn edge_sample<Pixel: Copy>(samples: &[Pixel]) -> Rav1dResult<Pixel> {
  samples.first().copied().ok_or(Rav1dError::EINVAL)
}

struct EdgeSamples<'source, Pixel> {
  above: &'source [Pixel],
  midpoint: i32,
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "The reconstruction source presents a native stride at this boundary"
  )
)]
fn fixed_pixel_pitch<BD: BitDepth>(
  source: &ReconSrc<'_>,
) -> Result<i64, console_core_never::Never> {
  let Ok(pitch) = source.pixel_stride::<BD>();
  fixed_offset(pitch)
}

fn left_source<BD: BitDepth>(source: &ReconSrc<'_>) -> Rav1dResult<BD::Pixel> {
  let Ok(left) = source.at(-1);
  left.get::<BD>()
}

fn copy_padded_row<Pixel: Copy>(output: &mut [Pixel], input: &[Pixel]) -> Rav1dResult {
  let (visible, padding) = output
    .split_at_mut_checked(input.len())
    .ok_or(Rav1dError::EINVAL)?;
  let fill = input.last().copied().ok_or(Rav1dError::EINVAL)?;

  for (destination, &sample) in visible.iter_mut().zip(input) {
    *destination = sample;
  }

  padding.fill(fill);
  Ok(())
}

struct EdgeColumn {
  height: u64,
  base: u64,
  pitch: i64,
}

fn copy_reverse_column<BD: BitDepth>(
  output: &mut [BD::Pixel],
  source: &[u8],
  column: EdgeColumn,
) -> Rav1dResult {
  let pixels: &[BD::Pixel] =
    zerocopy::FromBytes::ref_from_bytes(source).map_err(|_| Rav1dError::EINVAL)?;
  let Ok(sample_size) = fixed_index(core::mem::size_of::<BD::Pixel>());
  let Ok(output_height) = fixed_index(output.len());
  let padding_rows = output_height
    .checked_sub(column.height)
    .ok_or(Rav1dError::EINVAL)?;
  let Ok(padding_count) = index(padding_rows);
  let (padding, visible) = output
    .split_at_mut_checked(padding_count)
    .ok_or(Rav1dError::EINVAL)?;

  for (destination, row) in visible.iter_mut().rev().zip(0..column.height) {
    let Ok(row_offset) = wrapped::<_, i64>(row);
    let byte = column
      .base
      .wrapping_add_signed(row_offset.wrapping_mul(column.pitch));
    let sample = byte.checked_div(sample_size).ok_or(Rav1dError::EINVAL)?;
    let Ok(sample_index) = index(sample);
    let value = pixels.get(sample_index).ok_or(Rav1dError::EINVAL)?;
    *destination = *value;
  }

  let fill = edge_sample(visible)?;
  padding.fill(fill);
  Ok(())
}

fn read_left_column<BD: BitDepth>(
  source: &ReconSrc<'_>,
  output: &mut [BD::Pixel],
  height: u64,
) -> Rav1dResult {
  let Ok(result) = source.with_block_context::<BD, _, _>(
    Size { width: 1, height },
    output,
    |column, bytes, base, pitch| {
      copy_reverse_column::<BD>(
        column,
        bytes,
        EdgeColumn {
          height,
          base,
          pitch,
        },
      )
    },
  );
  result
}

struct EdgeExtent {
  output: u64,
  available: u64,
}

fn prepare_left<BD: BitDepth>(
  output: &mut IntraEdgeDestination<'_, BD::Pixel>,
  source: &ReconSrc<'_>,
  input: &EdgeSamples<'_, BD::Pixel>,
  config: &IntraEdgePreparation<'_>,
  geometry: EdgeExtent,
  required: u8,
) -> Rav1dResult {
  let length = geometry.output;
  let left_start = output
    .center
    .checked_sub(length)
    .ok_or(Rav1dError::EINVAL)?;
  let left = edge_range(output.samples, left_start..output.center)?;

  match config.has_left.then_some(()) {
    Some(()) => {
      let Ok(column) = source.at(-1);
      read_left_column::<BD>(&column, left, length.min(geometry.available))?;
    }
    None => {
      let fill = match (!matches!(&config.above, AboveEdge::Absent)).then_some(()) {
        Some(()) => {
          let sample = edge_sample(input.above)?;
          sample
        }
        None => input.midpoint.wrapping_add(1).as_::<BD::Pixel>(),
      };
      left.fill(fill);
    }
  }

  match (required & BOTTOM_LEFT_EDGE != 0).then_some(()) {
    Some(()) => {
      let repeat = edge_sample(left)?;
      let start = left_start.checked_sub(length).ok_or(Rav1dError::EINVAL)?;
      let bottom_left = edge_range(output.samples, start..left_start)?;
      extend_left::<BD>(source, bottom_left, config, geometry, repeat)
    }
    None => Ok(()),
  }
}

fn extend_left<BD: BitDepth>(
  source: &ReconSrc<'_>,
  bottom_left: &mut [BD::Pixel],
  config: &IntraEdgePreparation<'_>,
  geometry: EdgeExtent,
  repeat: BD::Pixel,
) -> Rav1dResult {
  let present = config.has_left
    && geometry.available > geometry.output
    && matches!(
      config
        .available
        .availability(EdgeFlags::I444_LEFT_HAS_BOTTOM),
      Ok(EdgeAvailability::Present)
    );

  match present.then_some(()) {
    Some(()) => {
      let Ok(pitch) = fixed_pixel_pitch::<BD>(source);
      let Ok(rows) = wrapped::<_, i64>(geometry.output);
      let Ok(offset) = wrapped_offset(rows.wrapping_mul(pitch).wrapping_sub(1));
      let Ok(column) = source.at(offset);
      let available = geometry.available.wrapping_sub(geometry.output);
      read_left_column::<BD>(&column, bottom_left, geometry.output.min(available))
    }
    None => {
      bottom_left.fill(repeat);
      Ok(())
    }
  }
}

fn prepare_top<BD: BitDepth>(
  output: &mut IntraEdgeDestination<'_, BD::Pixel>,
  source: &ReconSrc<'_>,
  input: &EdgeSamples<'_, BD::Pixel>,
  config: &IntraEdgePreparation<'_>,
  geometry: EdgeExtent,
  required: u8,
) -> Rav1dResult {
  let start = output.center.wrapping_add(1);
  let end = start.wrapping_add(geometry.output);
  let top = edge_range(output.samples, start..end)?;
  let shift = u64::from(u8::from(config.has_left));

  match (!matches!(&config.above, AboveEdge::Absent)).then_some(()) {
    Some(()) => {
      let count = geometry.output.min(geometry.available);
      let Ok(visible) = indices(shift..shift.wrapping_add(count));
      let above = input.above.get(visible).ok_or(Rav1dError::EINVAL)?;
      copy_padded_row(top, above)?;
    }
    None => {
      let fill = match config.has_left.then_some(()) {
        Some(()) => {
          let sample = left_source::<BD>(source)?;
          sample
        }
        None => input.midpoint.wrapping_sub(1).as_::<BD::Pixel>(),
      };
      top.fill(fill);
    }
  }

  match (required & TOP_RIGHT_EDGE != 0).then_some(()) {
    Some(()) => {
      let repeat = top.last().copied().ok_or(Rav1dError::EINVAL)?;
      let top_right = edge_range(output.samples, end..end.wrapping_add(geometry.output))?;
      extend_top(top_right, input, config, geometry, repeat)
    }
    None => Ok(()),
  }
}

fn extend_top<Pixel: Copy>(
  top_right: &mut [Pixel],
  input: &EdgeSamples<'_, Pixel>,
  config: &IntraEdgePreparation<'_>,
  geometry: EdgeExtent,
  repeat: Pixel,
) -> Rav1dResult {
  let present = !matches!(&config.above, AboveEdge::Absent)
    && geometry.available > geometry.output
    && matches!(
      config.available.availability(EdgeFlags::I444_TOP_HAS_RIGHT),
      Ok(EdgeAvailability::Present)
    );

  match present.then_some(()) {
    Some(()) => {
      let available = geometry.available.wrapping_sub(geometry.output);
      let count = geometry.output.min(available);
      let start = geometry
        .output
        .wrapping_add(u64::from(u8::from(config.has_left)));
      let Ok(visible) = indices(start..start.wrapping_add(count));
      let above = input.above.get(visible).ok_or(Rav1dError::EINVAL)?;
      copy_padded_row(top_right, above)
    }
    None => {
      top_right.fill(repeat);
      Ok(())
    }
  }
}

fn prepare_corner<BD: BitDepth>(
  output: &mut IntraEdgeDestination<'_, BD::Pixel>,
  source: &ReconSrc<'_>,
  input: &EdgeSamples<'_, BD::Pixel>,
  config: &IntraEdgePreparation<'_>,
  mode: IntraPredMode,
) -> Rav1dResult {
  let start = output.center.checked_sub(1).ok_or(Rav1dError::EINVAL)?;
  let neighborhood = edge_range(output.samples, start..start.wrapping_add(3))?;
  let corner: &mut [BD::Pixel; 3] = neighborhood.try_into().map_err(|_| Rav1dError::EINVAL)?;
  let [left, center, right] = corner;

  *center = match (!matches!(&config.above, AboveEdge::Absent), config.has_left) {
    (true, _) => {
      let above = edge_sample(input.above)?;
      above
    }
    (false, true) => {
      let adjacent = left_source::<BD>(source)?;
      adjacent
    }
    (false, false) => input.midpoint.as_::<BD::Pixel>(),
  };

  let filter = mode == Z2_PRED
    && config.transform.width.wrapping_add(config.transform.height) >= 6
    && config.filter_corner != 0;

  match filter.then_some(()) {
    Some(()) => {
      let ends = (*left).as_::<c_int>().wrapping_add((*right).as_::<c_int>());
      let sides = ends.wrapping_mul(5);
      let center_weighted = (*center).as_::<c_int>().wrapping_mul(6);
      let filtered = sides
        .wrapping_add(center_weighted)
        .wrapping_add(8)
        .wrapping_shr(4);
      *center = filtered.as_::<BD::Pixel>();
    }
    None => {}
  }

  Ok(())
}

pub(crate) fn rav1d_prepare_intra_edges<BD: BitDepth>(
  config: IntraEdgePreparation<'_>,
  source: &ReconSrc<'_>,
  mode: IntraPredMode,
  angle: &mut c_int,
  mut output: IntraEdgeDestination<'_, BD::Pixel>,
  bd: BD,
) -> Rav1dResult<IntraPredMode> {
  let Point { x, y } = config.position;

  match (x < config.frame.width && y < config.frame.height).then_some(()) {
    Some(()) => {}
    None => return Err(Rav1dError::EINVAL),
  }

  let Ok(refined_mode) = prepare_intra_mode(&config, mode, angle);
  let Ok(required) = edge_requirements(refined_mode);
  let span_x = config.frame.width.wrapping_sub(x).wrapping_mul(4);
  let span_y = config.frame.height.wrapping_sub(y).wrapping_mul(4);
  let Ok(available_x) = wrapped::<_, u64>(span_x);
  let Ok(available_y) = wrapped::<_, u64>(span_y);
  let Ok(width) = wrapped::<_, u64>(config.transform.width.wrapping_mul(4));
  let Ok(height) = wrapped::<_, u64>(config.transform.height.wrapping_mul(4));
  let need_top = !matches!(&config.above, AboveEdge::Absent)
    && (required & (TOP_EDGE | CORNER_EDGE) != 0
      || required & LEFT_EDGE != 0 && !config.has_left);
  let reconstructed_samples;
  let dst_top: &[BD::Pixel] = match need_top.then_some(()) {
    Some(()) => {
      let shift = u64::from(u8::from(config.has_left));
      let Ok(count) = index(width.wrapping_mul(2).min(available_x).wrapping_add(shift));

      match &config.above {
        AboveEdge::Prefiltered(top) => {
          let Ok(column) = wrapped::<_, u64>(x.wrapping_mul(4));
          let offset = column
            .wrapping_sub(shift)
            .wrapping_add_signed(top.row_start);
          let Ok(start) = index(offset);
          &*top.samples.slice_as((start.., ..count))
        }
        AboveEdge::Reconstructed => {
          let Ok(stride) = fixed_pixel_pitch::<BD>(source);
          let Ok(corner_offset) = wrapped::<_, i64>(shift);
          let displacement = stride.wrapping_neg().wrapping_sub(corner_offset);
          let Ok(pixel_offset) = wrapped_offset(displacement);
          let Ok(near) = source.at(pixel_offset);
          let samples = near.slice::<BD>(count)?;
          reconstructed_samples = samples;
          &reconstructed_samples
        }
        AboveEdge::Absent => return Err(Rav1dError::EINVAL),
      }
    }
    None => &[],
  };
  let Ok(bits) = wrapped::<_, u32>(bd.bitdepth());
  let input = EdgeSamples {
    above: dst_top,
    midpoint: 1i32.wrapping_shl(bits).wrapping_shr(1),
  };

  match (required & LEFT_EDGE != 0).then_some(()) {
    Some(()) => prepare_left::<BD>(
      &mut output,
      source,
      &input,
      &config,
      EdgeExtent {
        output: height,
        available: available_y,
      },
      required,
    )?,
    None => {}
  }

  match (required & TOP_EDGE != 0).then_some(()) {
    Some(()) => prepare_top::<BD>(
      &mut output,
      source,
      &input,
      &config,
      EdgeExtent {
        output: width,
        available: available_x,
      },
      required,
    )?,
    None => {}
  }

  match (required & CORNER_EDGE != 0).then_some(()) {
    Some(()) => prepare_corner::<BD>(&mut output, source, &input, &config, refined_mode)?,
    None => {}
  }

  Ok(refined_mode)
}
