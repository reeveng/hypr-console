#![forbid(unsafe_code)]

use crate::include::common::bitdepth::BitDepth;
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::picture::PicOffset;
use crate::src::align::{Align8, AlignedVec64};
use crate::src::disjoint_mut::DisjointMut;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use crate::src::internal::Rav1dContext;
use crate::src::internal::{Rav1dFrameData, RightEdgeRows};
use crate::src::loopfilter::{HV, YUV};
use crate::src::lr_apply::LrRestorePlanes;
use crate::src::mc::{ResizePhase, ResizeSampling};
use crate::src::relaxed_atomic::RelaxedAtomic;
use crate::src::strided::Strided as _;
use crate::src::strided::WithStride;
use crate::src::with_offset::WithOffset;
use console_core_geometry::Size;
use console_core_number_conversion::{checked_index, index};
use core::ops::Range;
use std::cmp;
use std::ffi::c_int;

#[derive(Clone, Copy)]
enum BackupFilter {
  Cdef,
  Restoration,
}

type BackupBuffer<'a> = WithOffset<WithStride<&'a DisjointMut<AlignedVec64<u8>>>>;

struct BackupStripe<'a> {
  destination: BackupBuffer<'a>,
  source: PicOffset<'a>,
  rows: Range<i32>,
  source_size: Size<i32>,
  subsampling: console_core_geometry::Point<i32>,
  filter: BackupFilter,
}

fn backup_row_position(
  base: u64,
  BackupRowPositionArguments { row, stride }: BackupRowPositionArguments,
) -> Rav1dResult<u64> {
  let offset = row.checked_mul(stride).ok_or(EINVAL)?;
  base.checked_add_signed(offset).ok_or(EINVAL)
}

struct BackupRowPositionArguments {
  row: i64,
  stride: i64,
}

struct BackupLineCopy {
  source: u64,
  destination: u64,
  samples: u64,
}

struct BackupHeaderRows {
  base: u64,
  pitch: i64,
  previous: i64,
  samples: u64,
}

fn restore_header_rows<BD: BitDepth>(
  buffer: &DisjointMut<AlignedVec64<u8>>,
  BackupHeaderRows {
    base,
    pitch,
    previous,
    samples,
  }: BackupHeaderRows,
) -> Rav1dResult {
  for line in 0_i64..4 {
    let row = match pitch.cmp(&0) {
      cmp::Ordering::Less => 3_i64.wrapping_sub(line),
      cmp::Ordering::Equal | cmp::Ordering::Greater => line,
    };
    let source_row = BackupRowPositionArguments {
      row: row.wrapping_add(previous),
      stride: pitch,
    };
    let source = backup_row_position(base, source_row)?;
    let destination =
      backup_row_position(base, BackupRowPositionArguments { row, stride: pitch })?;
    backup_line_copy::<BD>(
      buffer,
      BackupLineCopy {
        source,
        destination,
        samples,
      },
    )?;
  }

  Ok(())
}

enum BackupTransfer {
  Resample,
  Copy,
}

struct BackupCopy<'a> {
  source: PicOffset<'a>,
  destination: &'a DisjointMut<AlignedVec64<u8>>,
  source_base: u64,
  destination_base: u64,
  source_pitch: i64,
  destination_pitch: i64,
  samples: u64,
  rows: i64,
}

fn copy_backup_rows<BD: BitDepth>(copy: BackupCopy<'_>) -> Rav1dResult {
  let BackupCopy {
    source,
    destination,
    source_base,
    destination_base,
    source_pitch,
    destination_pitch,
    samples,
    rows,
  } = copy;
  let Ok(width) = index(samples);
  let Ok(source_capacity) =
    console_core_number_conversion::fixed_index(source.data.pixel_len::<BD>());
  let Ok(buffer_bytes) = console_core_number_conversion::fixed_index(destination.len());
  let Ok(pixel_size) =
    console_core_number_conversion::fixed_index(core::mem::size_of::<BD::Pixel>());
  let buffer_capacity = buffer_bytes.checked_div(pixel_size).ok_or(EINVAL)?;

  for row in 0..rows {
    let src_position = backup_row_position(
      source_base,
      BackupRowPositionArguments {
        row,
        stride: source_pitch,
      },
    )?;
    let dst_position = backup_row_position(
      destination_base,
      BackupRowPositionArguments {
        row,
        stride: destination_pitch,
      },
    )?;
    let src_end = src_position.checked_add(samples).ok_or(EINVAL)?;
    let dst_end = dst_position.checked_add(samples).ok_or(EINVAL)?;

    match (src_end <= source_capacity && dst_end <= buffer_capacity).then_some(()) {
      Some(()) => {}
      None => return Err(EINVAL),
    }

    let Ok(src_index) = index(src_position);
    let Ok(dst_range) = console_core_number_conversion::indices(dst_position..dst_end);
    let pixels = PicOffset {
      offset: src_index,
      ..source
    };
    BD::pixel_copy(
      &mut destination.mut_slice_as(dst_range),
      &pixels.slice::<BD>(width),
      width,
    );
  }

  Ok(())
}

fn backup_line_copy<BD: BitDepth>(
  buffer: &DisjointMut<AlignedVec64<u8>>,
  line: BackupLineCopy,
) -> Rav1dResult {
  let Ok(byte_capacity) = console_core_number_conversion::fixed_index(buffer.len());
  let Ok(pixel_size) =
    console_core_number_conversion::fixed_index(core::mem::size_of::<BD::Pixel>());
  let capacity = byte_capacity.checked_div(pixel_size).ok_or(EINVAL)?;
  let source_end = line.source.checked_add(line.samples).ok_or(EINVAL)?;
  let destination_end = line.destination.checked_add(line.samples).ok_or(EINVAL)?;

  match (source_end <= capacity && destination_end <= capacity).then_some(()) {
    Some(()) => {}
    None => return Err(EINVAL),
  }

  let Ok(source) = console_core_number_conversion::indices(line.source..source_end);
  let Ok(destination) =
    console_core_number_conversion::indices(line.destination..destination_end);
  let Ok(samples) = index(line.samples);
  BD::pixel_copy(
    &mut buffer.mut_slice_as(destination),
    &buffer.slice_as(source),
    samples,
  );
  Ok(())
}

fn backup_lpf<BD: BitDepth>(
  c: &Rav1dContext,
  f: &Rav1dFrameData,
  BackupStripe {
    destination,
    source,
    rows,
    source_size,
    subsampling,
    filter,
  }: BackupStripe<'_>,
) -> Rav1dResult {
  let frame = f.frame_hdr.as_ref().ok_or(EINVAL)?;
  let sequence = f.seq_hdr.as_ref().ok_or(EINVAL)?;
  let vertical = u32::try_from(subsampling.y).map_err(|_invalid_subsampling| EINVAL)?;
  let horizontal = u32::try_from(subsampling.x).map_err(|_invalid_subsampling| EINVAL)?;
  let [original_width, scaled_width] = frame.size.width;
  let transfer = match (filter, original_width.cmp(&scaled_width)) {
    (BackupFilter::Restoration, cmp::Ordering::Less | cmp::Ordering::Greater) => {
      BackupTransfer::Resample
    }
    _ => BackupTransfer::Copy,
  };
  let source_width = u64::try_from(source_size.width).map_err(|_invalid_width| EINVAL)?;
  let destination_width = match frame.size.super_res.enabled.then_some(()) {
    Some(()) => u64::try_from(
      scaled_width
        .wrapping_add(subsampling.x)
        .wrapping_shr(horizontal),
    )
    .map_err(|_invalid_width| EINVAL)?,
    None => source_width,
  };
  let enhancement_shift = match filter {
    BackupFilter::Cdef => u32::from(sequence.sb128),
    BackupFilter::Restoration => 0,
  };
  let first_height = 64_i32
    .wrapping_shl(enhancement_shift)
    .wrapping_sub(8_i32.wrapping_mul(i32::from(rows.start == 0)))
    .wrapping_shr(vertical);
  let stripe_height = 64_i32.wrapping_shr(vertical);
  let Ok(src_offset) = console_core_number_conversion::fixed_index(source.offset);
  let Ok(dst_offset) = console_core_number_conversion::fixed_index(destination.offset);
  let Ok(src_pitch) = source.fixed_pixel_stride::<BD>();
  let Ok(dst_pitch) = destination.fixed_pixel_stride::<BD>();
  let mut src_base = backup_row_position(
    src_offset,
    self::BackupRowPositionArguments {
      row: i64::from(first_height.wrapping_sub(2)),
      stride: src_pitch,
    },
  )?;
  let mut dst_base = dst_offset;
  let buffer = destination.data.buf;

  match (c.tc.len() == 1).then_some(()) {
    Some(()) => {
      match rows.start {
        0 => {}
        _ => {
          let previous = 4_i64.wrapping_shl(u32::from(sequence.sb128));
          let header = BackupHeaderRows {
            base: dst_base,
            pitch: dst_pitch,
            previous,
            samples: destination_width,
          };
          restore_header_rows::<BD>(buffer, header)?;
        }
      }

      let first = backup_row_position(
        dst_base,
        BackupRowPositionArguments {
          row: 4,
          stride: dst_pitch,
        },
      )?;
      dst_base = first;
    }
    None => {}
  }

  let Ok(horizontal_index) = index(horizontal);
  let &step = f.resize_step.get(horizontal_index).ok_or(EINVAL)?;
  let &initial = f.resize_start.get(horizontal_index).ok_or(EINVAL)?;
  let phase = ResizePhase { step, initial };
  let depth = BD::from_c(f.bitdepth_max);
  let mut boundary = rows.start.wrapping_add(first_height);

  for _ in rows.clone() {
    match boundary.cmp(&rows.end) {
      cmp::Ordering::Greater => break,
      cmp::Ordering::Less | cmp::Ordering::Equal => {}
    }

    let available = 4_i64.wrapping_sub(i64::from(boundary.wrapping_add(1) == source_size.height));

    match transfer {
      BackupTransfer::Resample => {
        let src_position = checked_index(src_base).map_err(|_outside_picture| EINVAL)?;
        let dst_position = checked_index(dst_base).map_err(|_outside_buffer| EINVAL)?;
        let Ok(buf) = WithOffset::buf(BackupBuffer {
          offset: dst_position,
          ..destination
        });
        let Ok(height) = console_core_number_conversion::wrapped::<_, u64>(available);
        let sampling = ResizeSampling {
          source_width,
          destination: Size {
            width: destination_width,
            height,
          },
          phase,
        };
        f.dsp.mc.resize.call::<BD>(
          buf,
          PicOffset {
            offset: src_position,
            ..source
          },
          sampling,
          depth,
        )?;
      }
      BackupTransfer::Copy => {
        let copy = BackupCopy {
          source,
          destination: buffer,
          source_base: src_base,
          destination_base: dst_base,
          source_pitch: src_pitch,
          destination_pitch: dst_pitch,
          samples: source_width,
          rows: available,
        };
        copy_backup_rows::<BD>(copy)?;
      }
    }

    match available {
      3 => {
        let previous = backup_row_position(
          dst_base,
          self::BackupRowPositionArguments {
            row: 2,
            stride: dst_pitch,
          },
        )?;
        let current = backup_row_position(
          dst_base,
          self::BackupRowPositionArguments {
            row: 3,
            stride: dst_pitch,
          },
        )?;
        let samples = match transfer {
          BackupTransfer::Resample => destination_width,
          BackupTransfer::Copy => source_width,
        };
        backup_line_copy::<BD>(
          buffer,
          BackupLineCopy {
            source: previous,
            destination: current,
            samples,
          },
        )?;
      }
      _ => {}
    }

    boundary = boundary.wrapping_add(stripe_height);

    match boundary.cmp(&rows.end) {
      cmp::Ordering::Greater => break,
      cmp::Ordering::Less | cmp::Ordering::Equal => {
        let next_source = backup_row_position(
          src_base,
          BackupRowPositionArguments {
            row: i64::from(stripe_height),
            stride: src_pitch,
          },
        )?;
        let next_destination = backup_row_position(
          dst_base,
          BackupRowPositionArguments {
            row: 4,
            stride: dst_pitch,
          },
        )?;
        src_base = next_source;
        dst_base = next_destination;
      }
    }
  }

  Ok(())
}

struct LoopFilterPlane<'a> {
  source: PicOffset<'a>,
  resized: &'a crate::include::dav1d::picture::Rav1dPictureDataComponent,
  restoration_line: u64,
  cdef_line: u64,
  restore: LrRestorePlanes,
}

fn loop_filter_planes<'a>(
  sources: [PicOffset<'a>; 3],
  picture: &'a crate::include::dav1d::picture::Rav1dPictureData,
  f: &Rav1dFrameData,
) -> Result<[LoopFilterPlane<'a>; 3], console_core_never::Never> {
  let [luma_resized, cb_resized, cr_resized] = &picture.data;
  let [luma_source, cb_source, cr_source] = sources;
  let [y_restoration, u_restoration, v_restoration] = f.lf.lr_lpf_line;
  let [y_cdef, u_cdef, v_cdef] = f.lf.cdef_lpf_line;
  Ok([
    LoopFilterPlane {
      source: luma_source,
      resized: luma_resized,
      restoration_line: y_restoration,
      cdef_line: y_cdef,
      restore: LrRestorePlanes::Y,
    },
    LoopFilterPlane {
      source: cb_source,
      resized: cb_resized,
      restoration_line: u_restoration,
      cdef_line: u_cdef,
      restore: LrRestorePlanes::U,
    },
    LoopFilterPlane {
      source: cr_source,
      resized: cr_resized,
      restoration_line: v_restoration,
      cdef_line: v_cdef,
      restore: LrRestorePlanes::V,
    },
  ])
}

pub(crate) fn rav1d_copy_lpf<BD: BitDepth>(
  c: &Rav1dContext,
  f: &Rav1dFrameData,
  sources: [PicOffset; 3],
  sby: c_int,
) -> Rav1dResult {
  let sequence = f.seq_hdr.as_ref().ok_or(EINVAL)?;
  let frame = f.frame_hdr.as_ref().ok_or(EINVAL)?;
  let picture = f.sr_cur.p.data.as_ref().ok_or(EINVAL)?;
  let [original_width, scaled_width] = frame.size.width;
  let resizes = original_width != scaled_width;
  let threaded = c.tc.len() > 1;
  let main_layout = f.cur.p.layout;
  let chroma_layout = f.sr_cur.p.p.layout;
  let block_shift = 6_u32.wrapping_add(u32::from(sequence.sb128));
  let row_backup = 8_i32.wrapping_mul(i32::from(sby != 0));
  let first_backup_row = 4_i64.wrapping_mul(i64::from(sby));
  let restoration_row = first_backup_row
    .wrapping_shl(u32::from(sequence.sb128))
    .wrapping_mul(i64::from(threaded));
  let [_, source_u, _] = sources;
  let Ok(chroma_pitch) = source_u.fixed_pixel_stride::<BD>();
  let Ok(planes) = loop_filter_planes(sources, picture, f);

  for plane in planes {
    let luma = plane.restore.contains(LrRestorePlanes::Y);
    let restores = f.lf.restore_planes.contains(plane.restore);

    match ((luma || main_layout != Rav1dPixelLayout::I400) && (sequence.cdef != 0 || restores))
      .then_some(())
    {
      None => continue,
      Some(()) => {}
    }

    let horizontal = u32::from(!luma && chroma_layout != Rav1dPixelLayout::I444);
    let vertical = u32::from(!luma && chroma_layout == Rav1dPixelLayout::I420);
    let subsampling = console_core_geometry::Point {
      x: i32::from(!luma && chroma_layout != Rav1dPixelLayout::I444),
      y: i32::from(!luma && chroma_layout == Rav1dPixelLayout::I420),
    };
    let height = f.cur.p.h.wrapping_add(subsampling.y).wrapping_shr(vertical);
    let source_size = Size {
      width: f.bw.wrapping_shl(2_u32.wrapping_sub(horizontal)),
      height,
    };
    let plane_shift = block_shift.wrapping_sub(vertical);
    let previous_rows = row_backup.wrapping_shr(vertical);
    let start = sby.wrapping_shl(plane_shift).wrapping_sub(previous_rows);
    let end = sby
      .wrapping_add(1)
      .wrapping_shl(plane_shift)
      .min(height.wrapping_sub(1));
    let Ok(byte_stride) = plane.source.fixed_stride();
    let pitch = BD::pxstride(byte_stride);
    let Ok(source_base) = console_core_number_conversion::fixed_index(plane.source.offset);
    let source_start = backup_row_position(
      source_base,
      self::BackupRowPositionArguments {
        row: i64::from(previous_rows).wrapping_neg(),
        stride: pitch,
      },
    )?;
    let Ok(source_offset) = index(source_start);
    let source = PicOffset {
      offset: source_offset,
      ..plane.source
    };

    match (restores || !resizes).then_some(()) {
      Some(()) => {
        let Ok(resized_stride) = plane.resized.fixed_stride();
        let base = backup_row_position(
          plane.restoration_line,
          self::BackupRowPositionArguments {
            row: restoration_row,
            stride: BD::pxstride(resized_stride),
          },
        )?;
        let offset = checked_index(base).map_err(|_outside_buffer| EINVAL)?;
        let destination = BackupBuffer {
          data: WithStride {
            buf: &f.lf.lr_line_buf,
            stride: resized_stride,
          },
          offset,
        };
        backup_lpf::<BD>(
          c,
          f,
          BackupStripe {
            destination,
            source,
            rows: start..end,
            source_size,
            subsampling,
            filter: BackupFilter::Restoration,
          },
        )?;
      }
      None => {}
    }

    match (threaded && resizes).then_some(()) {
      Some(()) => {
        const LUMA: u8 = LrRestorePlanes::Y.bits();
        let cdef_pitch = match plane.restore.bits() {
          LUMA => pitch,
          _ => chroma_pitch,
        };
        let base = backup_row_position(
          plane.cdef_line,
          self::BackupRowPositionArguments {
            row: first_backup_row,
            stride: cdef_pitch,
          },
        )?;
        let offset = checked_index(base).map_err(|_outside_buffer| EINVAL)?;
        let destination = BackupBuffer {
          data: WithStride {
            buf: &f.lf.cdef_line_buf,
            stride: byte_stride,
          },
          offset,
        };
        backup_lpf::<BD>(
          c,
          f,
          BackupStripe {
            destination,
            source,
            rows: start..end,
            source_size,
            subsampling,
            filter: BackupFilter::Cdef,
          },
        )?;
      }
      None => {}
    }
  }

  Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Neighbor {
  Present,
  Absent,
}

#[derive(Clone, Copy)]
enum FilterSweep {
  Columns,
  Rows { level_stride: i64 },
}

#[derive(Clone, Copy)]
enum FilterPictures<'a> {
  Luma(PicOffset<'a>),
  Chroma([PicOffset<'a>; 2]),
}

#[derive(Clone, Copy)]
struct FilterRegion {
  width: u32,
  row: FilterMaskRow,
  half_width: u32,
  neighbor: Neighbor,
}

struct FilterPlane<'a, const LEVELS: usize> {
  pictures: FilterPictures<'a>,
  levels: i64,
  masks: &'a [[[RelaxedAtomic<u16>; 2]; LEVELS]; 32],
  region: FilterRegion,
}

#[derive(Clone, Copy)]
enum MaskHalves {
  Lower,
  Upper,
  Packed { half_width: u32 },
}

impl MaskHalves {
  fn get(
    self,
    [lower, upper]: &[RelaxedAtomic<u16>; 2],
  ) -> Result<u32, console_core_never::Never> {
    let value = match self {
      Self::Lower => {
        let Ok(value) = lower.get();
        u32::from(value)
      }
      Self::Upper => {
        let Ok(value) = upper.get();
        u32::from(value)
      }
      Self::Packed { half_width } => {
        let Ok(lower_value) = lower.get();
        let Ok(upper_value) = upper.get();
        u32::from(lower_value) | (u32::from(upper_value).wrapping_shl(half_width))
      }
    };
    Ok(value)
  }
}

fn filter_pixel_offset<BD: BitDepth>(
  picture: PicOffset<'_>,
  indices: FilterSweep,
  displacement: i64,
) -> Rav1dResult<PicOffset<'_>> {
  let delta = match indices {
    FilterSweep::Rows { .. } => {
      let Ok(byte_stride) = console_core_number_conversion::fixed_offset(picture.stride());
      displacement
        .checked_mul(BD::pxstride(byte_stride))
        .ok_or(EINVAL)?
    }
    FilterSweep::Columns => displacement,
  };
  let Ok(base) = console_core_number_conversion::fixed_index(picture.offset);
  let position = base.checked_add_signed(delta).ok_or(EINVAL)?;
  let Ok(length) = console_core_number_conversion::fixed_index(picture.data.pixel_len::<BD>());

  match position.cmp(&length) {
    cmp::Ordering::Greater => Err(EINVAL),
    cmp::Ordering::Less | cmp::Ordering::Equal => {
      let Ok(offset) = index(position);
      Ok(PicOffset { offset, ..picture })
    }
  }
}

fn filter_plane<BD: BitDepth, const LEVELS: usize>(
  f: &Rav1dFrameData,
  sweep: FilterSweep,
  FilterPlane {
    pictures,
    levels,
    masks,
    region,
  }: FilterPlane<'_, LEVELS>,
) -> Rav1dResult {
  let FilterRegion {
    width,
    row,
    half_width,
    neighbor,
  } = region;
  let (coordinates, length, halves, direction) = match sweep {
    FilterSweep::Columns => {
      let halves = match (row.start, row.end > half_width) {
        (0, true) => MaskHalves::Packed { half_width },
        (0, false) => MaskHalves::Lower,
        (1.., _) => MaskHalves::Upper,
      };
      (
        0..width,
        {
          let checked_value_9 = row.end.checked_sub(row.start).ok_or(EINVAL)?;
          checked_value_9
        },
        halves,
        HV::H,
      )
    }
    FilterSweep::Rows { .. } => (
      row.start..row.end,
      width,
      MaskHalves::Packed { half_width },
      HV::V,
    ),
  };

  let displacement = |coordinate| match sweep {
    FilterSweep::Columns => i64::from(coordinate).wrapping_mul(4),
    FilterSweep::Rows { .. } => {
      i64::from(u32::wrapping_sub(coordinate, row.start)).wrapping_mul(4)
    }
  };

  let Ok(dsp_length) = index(length);
  let filter = &f.dsp.lf.loop_filter_sb;
  let level_values = f.lf.level.as_slice();

  for coordinate in coordinates {
    match {
      ::core::primitive::bool::then_some(coordinate == 0 && neighbor == Neighbor::Absent, ())
    } {
      ::core::option::Option::Some(()) => {
        continue;
      }
      ::core::option::Option::None => {}
    }

    let Ok(mask_index) = index(coordinate);
    let edge = masks.get(mask_index).ok_or(EINVAL)?;
    let mut packed = [0; 3];

    for (value, pair) in packed.iter_mut().zip(edge) {
      let Ok(mask) = halves.get(pair);
      *value = mask;
    }

    let lane = displacement(coordinate);
    let level_delta = match sweep {
      FilterSweep::Columns => lane,
      FilterSweep::Rows { level_stride } => lane.checked_mul(level_stride).ok_or(EINVAL)?,
    };
    let level = levels.checked_add(level_delta).ok_or(EINVAL)?;

    match pictures {
      FilterPictures::Luma(picture) => {
        let (dsp, channel_offset) = match sweep {
          FilterSweep::Columns => (&filter.y.h, 0),
          FilterSweep::Rows { .. } => (&filter.y.v, 1),
        };
        let channel = level.checked_add(channel_offset).ok_or(EINVAL)?;
        let offset = checked_index(channel).map_err(|_outside_levels| EINVAL)?;
        let destination = filter_pixel_offset::<BD>(picture, sweep, lane)?;
        let Ok(()) = dsp.call::<BD>(
          f,
          destination,
          &packed,
          WithOffset {
            data: level_values,
            offset,
          },
          dsp_length,
          YUV::Y,
          direction,
        );
      }
      FilterPictures::Chroma([u, v]) => {
        let dsp = match sweep {
          FilterSweep::Columns => &filter.uv.h,
          FilterSweep::Rows { .. } => &filter.uv.v,
        };

        for (picture, channel_offset) in [(u, 2), (v, 3)] {
          let channel = level.checked_add(channel_offset).ok_or(EINVAL)?;
          let offset = checked_index(channel).map_err(|_outside_levels| EINVAL)?;
          let destination = filter_pixel_offset::<BD>(picture, sweep, lane)?;
          let Ok(()) = dsp.call::<BD>(
            f,
            destination,
            &packed,
            WithOffset {
              data: level_values,
              offset,
            },
            dsp_length,
            YUV::UV,
            direction,
          );
        }
      }
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum EdgeHalf {
  Lower,
  Upper,
}

impl EdgeHalf {
  fn select(
    self,
    [lower, upper]: &[RelaxedAtomic<u16>; 2],
  ) -> Result<&RelaxedAtomic<u16>, console_core_never::Never> {
    Ok(match self {
      Self::Lower => lower,
      Self::Upper => upper,
    })
  }
}

fn limit_edge_strength<const LEVELS: usize>(
  lengths: &[[RelaxedAtomic<u16>; 2]; LEVELS],
  LimitEdgeStrengthArguments {
    coordinate,
    half_width,
  }: LimitEdgeStrengthArguments,
  neighbor_strength: u8,
) -> Rav1dResult {
  let (half, lane) = match coordinate.checked_sub(half_width) {
    Some(upper_lane) => (EdgeHalf::Upper, upper_lane),
    None => (EdgeHalf::Lower, coordinate),
  };
  let bit = 1_u16.checked_shl(lane).ok_or(EINVAL)?;

  let (medium, high) = match lengths.as_slice() {
    [_, medium, high @ ..] => (medium, high),
    [] | [_] => return Err(EINVAL),
  };

  let high_strength = match high {
    [pair] => {
      let Ok(selected) = half.select(pair);
      let Ok(mask) = selected.get();
      ::core::primitive::u8::wrapping_mul(2, u8::from(mask & bit != 0))
    }
    [] => 0,
    _ => return Err(EINVAL),
  };
  let Ok(medium_lane) = half.select(medium);
  let Ok(medium_mask) = medium_lane.get();
  let strength = high_strength.wrapping_add(u8::from(medium_mask & bit != 0));
  let Ok(level_index) = index(strength.min(neighbor_strength));
  let selected = lengths.get(level_index).ok_or(EINVAL)?;

  for pair in lengths.iter().rev() {
    let Ok(lane_2) = half.select(pair);
    let Ok(()) = lane_2.update(|mask| mask & !bit);
  }

  let Ok(selected_lane) = half.select(selected);
  let Ok(()) = selected_lane.update(|mask| mask | bit);
  Ok(())
}

struct LimitEdgeStrengthArguments {
  coordinate: u32,
  half_width: u32,
}

fn limit_side_strengths<const LEVELS: usize>(
  lengths: &[[RelaxedAtomic<u16>; 2]; LEVELS],
  samples: Range<u32>,
  half_width: u32,
  strengths: &[u8],
) -> Rav1dResult {
  for (coordinate, &strength) in samples.zip(strengths) {
    limit_edge_strength(
      lengths,
      self::LimitEdgeStrengthArguments {
        coordinate,
        half_width,
      },
      strength,
    )?;
  }

  Ok(())
}

struct FilterBoundaryColumns {
  width: u32,
  half_width: u32,
}

fn limit_above_strengths<const LEVELS: usize>(
  lengths: &[[RelaxedAtomic<u16>; 2]; LEVELS],
  strengths: &DisjointMut<Align8<[u8; 32]>>,
  FilterBoundaryColumns { width, half_width }: FilterBoundaryColumns,
) -> Rav1dResult {
  for coordinate in 0..width {
    let Ok(lane) = index(coordinate);
    let strength = *strengths.index(lane);
    let args = LimitEdgeStrengthArguments {
      coordinate,
      half_width,
    };
    limit_edge_strength(lengths, args, strength)?;
  }

  Ok(())
}

#[derive(Clone, Copy)]
struct FilterMaskRow {
  start: u32,
  end: u32,
}

impl FilterMaskRow {
  fn subsampled(self, shift: u32) -> Result<Self, console_core_never::Never> {
    let Self { start, end } = self;
    Ok(Self {
      start: start.wrapping_shr(shift),
      end: end.wrapping_add(shift).wrapping_shr(shift),
    })
  }
}

struct DeblockRow<'a> {
  masks: &'a [crate::src::lf_mask::Av1Filter],
  luma: FilterMaskRow,
  chroma: FilterMaskRow,
  horizontal_shift: u32,
  vertical_shift: u32,
  level_stride: i64,
  base: u32,
  width: u32,
}

#[derive(Clone, Copy)]
struct DeblockPass {
  sweep: FilterSweep,
  superblock_row: u32,
  byte_stride: i64,
}

impl DeblockPass {
  fn direction(self) -> Result<HV, console_core_never::Never> {
    Ok(match self.sweep {
      FilterSweep::Columns => HV::H,
      FilterSweep::Rows { .. } => HV::V,
    })
  }

  fn neighbor(self, column: u32) -> Result<Neighbor, console_core_never::Never> {
    let Ok(direction) = self.direction();
    Ok(match (direction, column, self.superblock_row) {
      (HV::H, 0, _) | (HV::V, _, 0) => Neighbor::Absent,
      (HV::H, 1.., _) | (HV::V, _, 1..) => Neighbor::Present,
    })
  }
}

impl DeblockRow<'_> {
  fn dispatch_luma<BD: BitDepth>(
    &self,
    f: &Rav1dFrameData,
    py: PicOffset<'_>,
    pass: DeblockPass,
  ) -> Rav1dResult {
    let Ok(direction) = pass.direction();
    let luma_base = pass.byte_stride.checked_mul(i64::from(self.base)).ok_or(EINVAL)?;

    for (column, mask) in (0_u32..).zip(self.masks) {
      let checked_value = self.width.checked_sub(column.wrapping_mul(32)).ok_or(EINVAL)?;
      let columns = checked_value.min(32);
      let Ok(pixel_offset) = index(column.wrapping_mul(128));
      let [columns_mask, rows_mask] = &mask.filter_y;
      let selected = match direction {
        HV::H => columns_mask,
        HV::V => rows_mask,
      };
      filter_plane::<BD, 3>(
        f,
        pass.sweep,
        FilterPlane {
          pictures: FilterPictures::Luma(py + pixel_offset),
          levels: {
            let checked_value_11 = luma_base
              .checked_add(i64::from(column).wrapping_mul(128))
              .ok_or(EINVAL)?;
            checked_value_11
          },
          masks: selected,
          region: FilterRegion {
            width: columns,
            row: self.luma,
            half_width: 16,
            neighbor: {
              let Ok(neighbor) = pass.neighbor(column);
              neighbor
            },
          },
        },
      )?;
    }

    Ok(())
  }


  fn dispatch<BD: BitDepth>(
    &self,
    f: &Rav1dFrameData,
    [py, pu, pv]: [PicOffset; 3],
    sweep: FilterSweep,
    sby: u32,
  ) -> Rav1dResult {
    let Self {
      masks,
      chroma,
      horizontal_shift,
      vertical_shift,
      level_stride,
      base,
      width,
      ..
    } = *self;
    let frame_hdr = f.frame_hdr.as_ref().ok_or(EINVAL)?;
    let byte_stride = level_stride.checked_mul(4).ok_or(EINVAL)?;
    let pass = DeblockPass { sweep, superblock_row: sby, byte_stride };
    let Ok(horizontal) = pass.direction();
    self.dispatch_luma::<BD>(f, py, pass)?;

    match {
      ::core::primitive::bool::then_some(
        frame_hdr.loopfilter.level_u == 0 && frame_hdr.loopfilter.level_v == 0,
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let chroma_base = byte_stride
      .checked_mul(i64::from(base.wrapping_shr(vertical_shift)))
      .ok_or(EINVAL)?;
    let chroma_step = 128_u32.wrapping_shr(horizontal_shift);
    let half_width = match horizontal {
      HV::H => 16_u32.wrapping_shr(vertical_shift),
      HV::V => 16_u32.wrapping_shr(horizontal_shift),
    };

    for (column, mask) in (0_u32..).zip(masks) {
      let checked_value_2 = width.checked_sub(column.wrapping_mul(32)).ok_or(EINVAL)?;
      let luma_width = checked_value_2.min(32);
      let columns = luma_width
        .wrapping_add(horizontal_shift)
        .wrapping_shr(horizontal_shift);
      let Ok(pixel_offset) = index(column.wrapping_mul(chroma_step));
      let [columns_mask, rows_mask] = &mask.filter_uv;
      let selected = match horizontal {
        HV::H => columns_mask,
        HV::V => rows_mask,
      };
      filter_plane::<BD, 2>(
        f,
        sweep,
        FilterPlane {
          pictures: FilterPictures::Chroma([pu + pixel_offset, pv + pixel_offset]),
          levels: {
            let checked_value_12 = chroma_base
              .checked_add(i64::from(column).wrapping_mul(i64::from(chroma_step)))
              .ok_or(EINVAL)?;
            checked_value_12
          },
          masks: selected,
          region: FilterRegion {
            width: columns,
            row: chroma,
            half_width,
            neighbor: {
              let Ok(neighbor) = pass.neighbor(column);
              neighbor
            },
          },
        },
      )?;
    }

    Ok(())
  }
}

fn deblock_row(f: &Rav1dFrameData, sby: u32) -> Rav1dResult<DeblockRow<'_>> {
  let seq_hdr = f.seq_hdr.as_ref().ok_or(EINVAL)?;
  let small_superblock = u32::from(seq_hdr.sb128 == 0);
  let block_count = u32::try_from(f.sb128w).map_err(|_invalid_width| EINVAL)?;
  let row = ::core::primitive::u32::wrapping_shr(sby, small_superblock);
  let start = row.checked_mul(block_count).ok_or(EINVAL)?;
  let end = start.checked_add(block_count).ok_or(EINVAL)?;
  let Ok(first_column) = index(start);
  let Ok(last_column) = index(end);
  let masks = f.lf.mask.get(first_column..last_column).ok_or(EINVAL)?;
  let layout = f.cur.p.layout;
  let vertical_shift = u32::from(layout == Rav1dPixelLayout::I420);
  let horizontal_shift = u32::from(layout != Rav1dPixelLayout::I444);
  let side = ::core::primitive::u32::wrapping_shr(32, small_superblock);
  let base = sby.checked_mul(side).ok_or(EINVAL)?;
  let height = u32::try_from(f.h4).map_err(|_invalid_height| EINVAL)?;
  let width = u32::try_from(f.w4).map_err(|_invalid_width| EINVAL)?;
  let local_row = (sby & small_superblock).wrapping_mul(16);
  let remaining = height.checked_sub(base).ok_or(EINVAL)?;
  let luma = FilterMaskRow {
    start: local_row,
    end: local_row.wrapping_add(remaining.min(side)),
  };
  let Ok(chroma) = luma.subsampled(vertical_shift);
  let level_stride = i64::from(f.b4_stride);
  Ok(DeblockRow {
    masks,
    luma,
    chroma,
    horizontal_shift,
    vertical_shift,
    level_stride,
    base,
    width,
  })
}

pub(crate) fn rav1d_loopfilter_sbrow_cols<BD: BitDepth>(
  f: &Rav1dFrameData,
  [py, pu, pv]: [PicOffset; 3],
  sby: u32,
  start_of_tile_row: u8,
) -> Rav1dResult {
  let frame_hdr = f.frame_hdr.as_ref().ok_or(EINVAL)?;
  let geometry = deblock_row(f, sby)?;
  let DeblockRow {
    masks,
    luma: luma_row,
    chroma: chroma_row,
    horizontal_shift: ss_hor,
    vertical_shift: ss_ver,
    base: row_base,
    width: frame_width,
    ..
  } = geometry;
  let layout = f.cur.p.layout;
  let seq_hdr = f.seq_hdr.as_ref().ok_or(EINVAL)?;
  let small_superblock = u32::from(seq_hdr.sb128 == 0);
  let side = ::core::primitive::u32::wrapping_shr(32, small_superblock);
  let checked_value_2 = u32::try_from(f.bh).map_err(|_invalid_height| EINVAL)?;
  let padded_height = checked_value_2.wrapping_add(31) & !31;
  let chroma_base = row_base.wrapping_shr(ss_ver);
  let frame_blocks = u32::try_from(f.bw).map_err(|_invalid_width| EINVAL)?;

  for (boundary, &tile_column) in (0_u32..).zip(frame_hdr.tiling.col_start_sb.iter().skip(1)) {
    let column = u32::from(tile_column);

    match { ::core::primitive::bool::then_some(column.wrapping_mul(side) >= frame_blocks, ()) } {
      ::core::option::Option::Some(()) => {
        break;
      }
      ::core::option::Option::None => {}
    }

    let bx4 = (column & small_superblock).wrapping_mul(16);
    let Ok(y_index) = index(bx4);
    let Ok(uv_index) = index(bx4.wrapping_shr(ss_hor));
    let Ok(mask_index) = index(::core::primitive::u32::wrapping_shr(
      column,
      small_superblock,
    ));
    let mask = masks.get(mask_index).ok_or(EINVAL)?;
    let [y_columns, _] = &mask.filter_y;
    let [uv_columns, _] = &mask.filter_uv;
    let y_column = y_columns.get(y_index).ok_or(EINVAL)?;
    let uv_column = uv_columns.get(uv_index).ok_or(EINVAL)?;
    let luma_boundary = u64::from(boundary).wrapping_mul(u64::from(padded_height));
    let chroma_boundary =
      u64::from(boundary).wrapping_mul(u64::from(padded_height.wrapping_shr(ss_ver)));
    let base_y = u64::from(row_base).wrapping_add(luma_boundary);
    let base_uv = u64::from(chroma_base).wrapping_add(chroma_boundary);
    let luma_length = luma_row.end.wrapping_sub(luma_row.start);
    let chroma_length = chroma_row.end.wrapping_sub(chroma_row.start);
    let y_range = base_y..base_y.wrapping_add(u64::from(luma_length));
    let uv_range = base_uv..base_uv.wrapping_add(u64::from(chroma_length));
    let (y_edge, uv_edge) = f.lf.tx_lpf_right_edge.get(RightEdgeRows {
      luma: y_range,
      chroma: uv_range,
    })?;
    limit_side_strengths(y_column, luma_row.start..luma_row.end, 16, &y_edge)?;

    match layout {
      Rav1dPixelLayout::I400 => {}
      Rav1dPixelLayout::I420 | Rav1dPixelLayout::I422 | Rav1dPixelLayout::I444 => {
        limit_side_strengths(
          uv_column,
          chroma_row.start..chroma_row.end,
          16_u32.wrapping_shr(ss_ver),
          &uv_edge,
        )?;
      }
    }
  }

  match start_of_tile_row {
    0 => {}
    _ => {
      let row = u64::from(start_of_tile_row.wrapping_sub(1));
      let columns = u64::try_from(f.sb128w).map_err(|_invalid_width| EINVAL)?;
      let top_row = row.wrapping_mul(columns);
      let top_end = top_row.checked_add(columns).ok_or(EINVAL)?;
      let Ok(above_contexts) = console_core_number_conversion::indices(top_row..top_end);
      let top = f.a.get(above_contexts).ok_or(EINVAL)?;
      let Ok(luma_index) = index(luma_row.start);
      let Ok(chroma_index) = index(chroma_row.start);

      for ((column, mask), above) in (0_u32..).zip(masks).zip(top) {
        let checked_value_10 = frame_width
          .checked_sub(column.wrapping_mul(32))
          .ok_or(EINVAL)?;
        let luma_width = checked_value_10.min(32);
        let [_, y_rows] = &mask.filter_y;
        let y_row = y_rows.get(luma_index).ok_or(EINVAL)?;

        let luma = FilterBoundaryColumns {
          width: luma_width,
          half_width: 16,
        };
        limit_above_strengths(y_row, &above.tx_lpf_y, luma)?;

        match layout {
          Rav1dPixelLayout::I400 => {}
          Rav1dPixelLayout::I420 | Rav1dPixelLayout::I422 | Rav1dPixelLayout::I444 => {
            let chroma_width = luma_width.wrapping_add(ss_hor).wrapping_shr(ss_hor);
            let [_, uv_rows] = &mask.filter_uv;
            let uv_row = uv_rows.get(chroma_index).ok_or(EINVAL)?;

            let chroma = FilterBoundaryColumns {
              width: chroma_width,
              half_width: 16_u32.wrapping_shr(ss_hor),
            };
            limit_above_strengths(uv_row, &above.tx_lpf_uv, chroma)?;
          }
        }
      }
    }
  }

  geometry.dispatch::<BD>(f, [py, pu, pv], FilterSweep::Columns, sby)
}

pub(crate) fn rav1d_loopfilter_sbrow_rows<BD: BitDepth>(
  f: &Rav1dFrameData,
  pictures: [PicOffset; 3],
  sby: u32,
) -> Rav1dResult {
  let geometry = deblock_row(f, sby)?;
  geometry.dispatch::<BD>(
    f,
    pictures,
    FilterSweep::Rows {
      level_stride: geometry.level_stride,
    },
    sby,
  )
}
