#![forbid(unsafe_code)]

use crate::include::common::bitdepth::BitDepth;
use crate::include::dav1d::headers::DAV1D_RESTORATION_SGRPROJ;
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::headers::Rav1dRestorationType;
use crate::include::dav1d::picture::PicOffset;
use crate::src::align::Align16;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use crate::src::internal::Rav1dContext;
use crate::src::internal::Rav1dFrameData;
use crate::src::lf_mask::Av1RestorationUnit;
use crate::src::looprestoration::LooprestorationParams;
use crate::src::looprestoration::LooprestorationParamsSgr;
use crate::src::looprestoration::LrEdgeFlags;
use crate::src::looprestoration::{
  Rav1dLoopRestorationDSPContext, RestorationKernel, RestorationPadding, RestorationRequest,
  SgrSelection, loop_restoration_filter,
};
use crate::src::strided::Strided as _;
use crate::src::tables::dav1d_sgr_params;
use aligned::Aligned;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{
  fixed_offset, index, wrapped, wrapped_index, wrapped_offset,
};
use core::cmp::Ordering;
use core::num::NonZeroU32;

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "bitflags generates the restoration plane queries used by the decoder and loop filters"
  )
)]
mod restore_planes {
  use bitflags::bitflags;

  bitflags! {
    #[derive(Clone, Copy, Default)]
    pub struct LrRestorePlanes: u8 {
      const Y = 1 << 0;
      const U = 1 << 1;
      const V = 1 << 2;

      const UV = Self::U.bits() | Self::V.bits();
    }
  }
}

pub(crate) use restore_planes::LrRestorePlanes;

struct StripeFilter {
  function: loop_restoration_filter::Fn,
  kernel: RestorationKernel,
  parameters: LooprestorationParams,
}

fn wiener_coefficients(taps: [i8; 3], center_bias: i16) -> Result<[i16; 8], Never> {
  let [outer, middle, inner] = taps.map(i16::from);
  let side = outer.wrapping_add(middle).wrapping_add(inner);
  let center = center_bias.wrapping_sub(side.wrapping_mul(2));

  Ok([outer, middle, inner, center, inner, middle, outer, 0])
}

fn select_stripe_filter<BD: BitDepth>(
  dsp: &Rav1dLoopRestorationDSPContext,
  unit: Av1RestorationUnit,
) -> Rav1dResult<StripeFilter> {
  match unit.r#type {
    Rav1dRestorationType::Wiener => {
      let horizontal_bias = match BD::BITDEPTH {
        8 => 0,
        _depth => 128,
      };
      let Ok(horizontal) = wiener_coefficients(unit.filter_h, horizontal_bias);
      let Ok(vertical) = wiener_coefficients(unit.filter_v, 128);
      let [outer_horizontal, ..] = horizontal;
      let [outer_vertical, ..] = vertical;
      let [seven_tap, five_tap] = dsp.wiener;
      let function = match (outer_horizontal, outer_vertical) {
        (0, 0) => five_tap,
        (_horizontal, _vertical) => seven_tap,
      };

      Ok(StripeFilter {
        function,
        kernel: RestorationKernel::Wiener,
        parameters: LooprestorationParams {
          filter: [horizontal, vertical],
        },
      })
    }
    sgr @ Rav1dRestorationType::SgrProj(_) => {
      let table_code = sgr.to_repr().wrapping_sub(DAV1D_RESTORATION_SGRPROJ);
      let Ok(table_index) = index(table_code);
      let &[first, second] = dav1d_sgr_params.get(table_index).ok_or(EINVAL)?;
      let [only_first, only_second, combined] = dsp.sgr;
      let (function, selection) = match (first, second) {
        (0, 0) => return Err(EINVAL),
        (1.., 0) => (only_first, SgrSelection::FiveByFive),
        (0, 1..) => (only_second, SgrSelection::ThreeByThree),
        (1.., 1..) => (combined, SgrSelection::Mixed),
      };
      let [w0, supplied_w1] = unit.sgr_weights.map(i16::from);
      let mut parameters = LooprestorationParams::default();
      let sgr_parameters = parameters.sgr_mut()?;
      *sgr_parameters = LooprestorationParamsSgr {
        s0: u32::from(first),
        s1: u32::from(second),
        w0,
        w1: 128_i16.wrapping_sub(w0.wrapping_add(supplied_w1)),
      };

      Ok(StripeFilter {
        function,
        kernel: RestorationKernel::SelfGuided(selection),
        parameters,
      })
    }
    Rav1dRestorationType::None | Rav1dRestorationType::Switchable => Err(EINVAL),
  }
}

struct RestorationStripe {
  origin: Point<i32>,
  plane: i32,
  width: i32,
  row_end: i32,
  unit: Av1RestorationUnit,
  edges: LrEdgeFlags,
}

struct RestorationLine {
  offset: i64,
  stride: i64,
  superblock_row: i32,
  vertical_shift: u32,
}

fn restoration_line<BD: BitDepth>(
  c: &Rav1dContext,
  f: &Rav1dFrameData,
  stripe: &RestorationStripe,
) -> Rav1dResult<RestorationLine> {
  let sequence = f.seq_hdr.as_ref().ok_or(EINVAL)?;
  let Ok(superblock_shift) = wrapped::<_, u32>(sequence.sb128);
  let [luma, chroma] = f.sr_cur.p.stride.map(|native| {
    let Ok(samples) = fixed_offset(BD::pxstride(native));
    samples
  });
  let stride = match stripe.plane {
    0 => luma,
    1 | 2 => chroma,
    _outside_planes => return Err(EINVAL),
  };
  let vertical_shift =
    u32::from(stripe.plane != 0 && f.sr_cur.p.p.layout == Rav1dPixelLayout::I420);
  let margin = match stripe.origin.y {
    0 => 0,
    _later_rows => 8_i32.wrapping_shl(vertical_shift),
  };
  let row_shift = 6_u32
    .wrapping_sub(vertical_shift)
    .wrapping_add(superblock_shift);
  let superblock_row = stripe.origin.y.wrapping_add(margin).wrapping_shr(row_shift);
  let Ok(plane) = wrapped_index(stripe.plane);
  let &origin = f.lf.lr_lpf_line.get(plane).ok_or(EINVAL)?;
  let base = i64::try_from(origin).map_err(|_invalid_line_origin| EINVAL)?;
  let line = superblock_row
    .wrapping_mul(4_i32.wrapping_shl(superblock_shift))
    .wrapping_sub(4);
  let stored_row = i64::from(i32::from(c.tc.len() > 1).wrapping_mul(line));
  let offset = base
    .wrapping_add(stored_row.wrapping_mul(stride))
    .wrapping_add(i64::from(stripe.origin.x));

  Ok(RestorationLine {
    offset,
    stride,
    superblock_row,
    vertical_shift,
  })
}

fn lr_stripe<BD: BitDepth>(
  c: &Rav1dContext,
  f: &Rav1dFrameData,
  mut picture: PicOffset<'_>,
  left: &[[BD::Pixel; 4]; 128 + 8],
  stripe: RestorationStripe,
) -> Rav1dResult {
  let filter = select_stripe_filter::<BD>(&f.dsp.lr, stripe.unit)?;
  let line = restoration_line::<BD>(c, f, &stripe)?;
  let bitdepth = BD::from_c(f.bitdepth_max);
  let start = stripe.origin.y;
  let available_rows = stripe.row_end.checked_sub(start).ok_or(EINVAL)?;
  let limit = u32::try_from(available_rows).map_err(|_negative| EINVAL)?;
  let Ok(row_count) = index(limit);
  let mut left_rows: &[_] = left;

  match (row_count <= left_rows.len()).then_some(()) {
    Some(()) => {}
    None => return Err(EINVAL),
  }

  let Ok(pixel_stride) = fixed_offset(match picture.pixel_stride::<BD>() {
    Ok(native) => native,
  });
  let mut row = start;
  let mut edges = stripe.edges;
  let first_height = 64_i32.wrapping_sub(8_i32.wrapping_mul(i32::from(row == 0)));
  let mut next_height = first_height.wrapping_shr(line.vertical_shift);
  let mut line_offset = line.offset;
  let width = u64::try_from(stripe.width).map_err(|_width| EINVAL)?;

  for _ in 0..limit {
    let height = next_height.min(stripe.row_end.wrapping_sub(row));
    let following = row.wrapping_add(height);
    edges.set(
      LrEdgeFlags::BOTTOM,
      line.superblock_row.wrapping_add(1) != f.sbh || following != stripe.row_end,
    );
    let height_bound = u64::try_from(height).map_err(|_height| EINVAL)?;
    filter.function.call(RestorationRequest {
      kernel: filter.kernel,
      padding: RestorationPadding {
        picture,
        backup: left_rows,
        lines: &f.lf.lr_line_buf,
        offset: line_offset,
        size: Size {
          width,
          height: height_bound,
        },
        edges,
      },
      parameters: &filter.parameters,
      bitdepth,
    })?;

    match following.cmp(&stripe.row_end) {
      Ordering::Equal => return Ok(()),
      Ordering::Greater => return Err(EINVAL),
      Ordering::Less => {}
    }

    let Ok(consumed) = wrapped_index(height);
    let remaining_left = left_rows.get(consumed..).ok_or(EINVAL)?;
    left_rows = remaining_left;
    let Ok(vertical_offset) = wrapped_offset(i64::from(height).wrapping_mul(pixel_stride));
    picture += vertical_offset;
    edges.insert(LrEdgeFlags::TOP);
    next_height = 64_i32.wrapping_shr(line.vertical_shift);
    line_offset = line_offset.wrapping_add(4_i64.wrapping_mul(line.stride));
    row = following;
  }

  Err(EINVAL)
}

fn backup_4xu<BD: BitDepth>(
  destination: &mut [[BD::Pixel; 4]; 128 + 8],
  source: PicOffset<'_>,
  row_count: i32,
) -> Rav1dResult {
  let Ok(end) = wrapped_index(row_count);
  let rows = destination.get_mut(..end).ok_or(EINVAL)?;
  let Ok(stride) = fixed_offset(match source.pixel_stride::<BD>() {
    Ok(native) => native,
  });

  for (row, backup) in (0..row_count).zip(rows) {
    let Ok(offset) = wrapped_offset(i64::from(row).wrapping_mul(stride));
    let original = (source + offset).slice::<BD>(4);
    BD::pixel_copy(backup, &original, 4);
  }

  Ok(())
}

struct RestorationBand {
  plane: i32,
  row_start: i32,
  row_end: i32,
  dimensions: Size<i32>,
  sampling: Point<u32>,
}

#[derive(Clone, Copy)]
struct RestorationMaskPosition {
  block: i32,
  plane: i32,
  unit: i32,
}

fn restoration_unit(
  f: &Rav1dFrameData,
  position: RestorationMaskPosition,
) -> Rav1dResult<Av1RestorationUnit> {
  let Ok(block) = wrapped_index(position.block);
  let Ok(plane) = wrapped_index(position.plane);
  let Ok(unit) = wrapped_index(position.unit);
  let mask = f.lf.lr_mask.get(block).ok_or(EINVAL)?;
  let units = mask.lr.get(plane).ok_or(EINVAL)?;
  let lock = units.get(unit).ok_or(EINVAL)?;
  let stored = lock.try_read().ok_or(EINVAL)?;

  Ok(*stored)
}

struct RestorationUnits {
  first: RestorationMaskPosition,
  width: i32,
  combined_width: i32,
  column_shift: u32,
}

fn restoration_units(
  f: &Rav1dFrameData,
  band: &RestorationBand,
) -> Rav1dResult<RestorationUnits> {
  let frame = f.frame_hdr.as_ref().ok_or(EINVAL)?;
  let [luma, chroma] = frame.restoration.unit_size;
  let log_width = match band.plane {
    0 => luma,
    1 | 2 => chroma,
    _outside_planes => return Err(EINVAL),
  };
  let width = 1_i32.wrapping_shl(u32::from(log_width));
  let half = width.wrapping_shr(1);
  let combined_width = width.wrapping_add(half);
  let margin = 8_i32
    .wrapping_shr(band.sampling.y)
    .wrapping_mul(i32::from(band.row_start != 0));
  let logical_row = band.row_start.wrapping_add(margin);
  let aligned = logical_row & !width.wrapping_sub(1);
  let top =
    match (aligned != 0 && aligned.wrapping_add(half) > band.dimensions.height).then_some(()) {
      Some(()) => aligned.wrapping_sub(width),
      None => aligned,
    };
  let full_row = top.wrapping_shl(band.sampling.y);
  let first = RestorationMaskPosition {
    block: full_row.wrapping_shr(7).wrapping_mul(f.sr_sb128w),
    plane: band.plane,
    unit: (full_row.wrapping_shr(6) & 1).wrapping_shl(1),
  };

  Ok(RestorationUnits {
    first,
    width,
    combined_width,
    column_shift: 7_u32.wrapping_sub(band.sampling.x),
  })
}

fn lr_sbrow<BD: BitDepth>(
  c: &Rav1dContext,
  f: &Rav1dFrameData,
  mut picture: PicOffset<'_>,
  band: RestorationBand,
) -> Rav1dResult {
  let units = restoration_units(f, &band)?;
  let mut active = restoration_unit(f, units.first)?;
  let width = u32::try_from(units.width).map_err(|_negative| EINVAL)?;
  let step_width = NonZeroU32::new(width).ok_or(EINVAL)?;
  let Ok(step) = index(width);
  let end = band
    .dimensions
    .width
    .wrapping_sub(units.combined_width)
    .wrapping_add(1);
  let columns = end.max(0).cast_unsigned().div_ceil(step_width.get());
  let pixel = BD::Pixel::from(0_u8);
  let mut saved: Align16<_> = Aligned([[[pixel; 4]; 128 + 8]; 2]);
  let mut alternate = false;
  let mut x = 0;
  let mut edges = match band.row_start {
    1.. => LrEdgeFlags::TOP | LrEdgeFlags::RIGHT,
    _origin => LrEdgeFlags::RIGHT,
  };

  for unit_column in 0..columns {
    let column = unit_column.cast_signed().wrapping_mul(units.width);
    let next_column = column.wrapping_add(units.width);
    let offset = next_column.wrapping_shr(units.column_shift);
    let within = next_column.wrapping_shr(units.column_shift.wrapping_sub(1)) & 1;
    let next_position = RestorationMaskPosition {
      block: units.first.block.wrapping_add(offset),
      plane: band.plane,
      unit: units.first.unit.wrapping_add(within),
    };
    let next = restoration_unit(f, next_position)?;
    let [first, second] = &mut *saved;
    let (left, right) = match alternate.then_some(()) {
      Some(()) => (first, second),
      None => (second, first),
    };

    match next.r#type {
      Rav1dRestorationType::None => {}
      Rav1dRestorationType::Switchable
      | Rav1dRestorationType::Wiener
      | Rav1dRestorationType::SgrProj(_) => {
        let before_boundary = picture + step.wrapping_sub(4);
        let rows = band.row_end.wrapping_sub(band.row_start);
        backup_4xu::<BD>(right, before_boundary, rows)?;
      }
    }

    match active.r#type {
      Rav1dRestorationType::None => {}
      Rav1dRestorationType::Switchable
      | Rav1dRestorationType::Wiener
      | Rav1dRestorationType::SgrProj(_) => {
        let stripe = RestorationStripe {
          origin: Point {
            x: column,
            y: band.row_start,
          },
          plane: band.plane,
          width: units.width,
          row_end: band.row_end,
          unit: active,
          edges,
        };
        lr_stripe::<BD>(c, f, picture, left, stripe)?;
      }
    }

    x = next_column;
    picture += step;
    active = next;
    edges.insert(LrEdgeFlags::LEFT);
    alternate = !alternate;
  }

  match active.r#type {
    Rav1dRestorationType::None => Ok(()),
    Rav1dRestorationType::Switchable
    | Rav1dRestorationType::Wiener
    | Rav1dRestorationType::SgrProj(_) => {
      let [first, second] = &*saved;
      let left = match alternate.then_some(()) {
        Some(()) => first,
        None => second,
      };
      edges.remove(LrEdgeFlags::RIGHT);
      let stripe = RestorationStripe {
        origin: Point {
          x,
          y: band.row_start,
        },
        plane: band.plane,
        width: band.dimensions.width.wrapping_sub(x),
        row_end: band.row_end,
        unit: active,
        edges,
      };
      lr_stripe::<BD>(c, f, picture, left, stripe)
    }
  }
}

pub(crate) fn rav1d_lr_sbrow<BD: BitDepth>(
  c: &Rav1dContext,
  f: &Rav1dFrameData,
  dst: [PicOffset<'_>; 3],
  superblock_row: i32,
) -> Rav1dResult {
  let sequence = f.seq_hdr.as_ref().ok_or(EINVAL)?;
  let shift = u32::from(6_u8.wrapping_add(sequence.sb128));
  let next_superblock = superblock_row.wrapping_add(1);
  let above = i32::from(superblock_row != 0);
  let below = i32::from(next_superblock < f.sbh);
  let layout = f.sr_cur.p.p.layout;
  let planes = [LrRestorePlanes::Y, LrRestorePlanes::U, LrRestorePlanes::V];

  'planes: for ((plane, flag), picture) in (0_i32..).zip(planes).zip(dst) {
    match f.lf.restore_planes.contains(flag).then_some(()) {
      Some(()) => {}
      None => continue 'planes,
    }

    let sampling = Point {
      x: u32::from(plane != 0 && layout != Rav1dPixelLayout::I444),
      y: u32::from(plane != 0 && layout == Rav1dPixelLayout::I420),
    };
    let frame = &f.sr_cur.p.p;
    let dimensions = Size {
      width: frame
        .w
        .wrapping_add(sampling.x.cast_signed())
        .wrapping_shr(sampling.x),
      height: frame
        .h
        .wrapping_add(sampling.y.cast_signed())
        .wrapping_shr(sampling.y),
    };
    let row_shift = shift.wrapping_sub(sampling.y);
    let margin = 8_i32.wrapping_shr(sampling.y);
    let lead = above.wrapping_mul(margin);
    let band = RestorationBand {
      plane,
      row_start: superblock_row.wrapping_shl(row_shift).wrapping_sub(lead),
      row_end: next_superblock
        .wrapping_shl(row_shift)
        .wrapping_sub(below.wrapping_mul(margin))
        .min(dimensions.height),
      dimensions,
      sampling,
    };
    let Ok(stride) = fixed_offset(match picture.pixel_stride::<BD>() {
      Ok(native) => native,
    });
    let Ok(offset) = wrapped_offset(i64::from(lead).wrapping_mul(stride));
    let source = picture - offset;
    lr_sbrow::<BD>(c, f, source, band)?;
  }

  Ok(())
}
