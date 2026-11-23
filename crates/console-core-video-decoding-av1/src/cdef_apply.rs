#![forbid(unsafe_code)]

use crate::include::common::bitdepth::BPC;
use crate::include::common::bitdepth::BitDepth;
use crate::include::common::intops::ulog2;
use crate::include::dav1d::headers::{Rav1dFrameHeaderCdef, Rav1dPixelLayout};
use crate::include::dav1d::picture::PicOffset;
use crate::src::align::Align16;
use crate::src::align::AlignedVec64;
use crate::src::cdef::{
  CdefBlockShape, CdefBottom, CdefEdgeFlags, CdefFilter, CdefTop, Rav1dCdefDSPContext,
};
use crate::src::disjoint_mut::DisjointMut;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use crate::src::internal::Rav1dContext;
use crate::src::internal::Rav1dFrameData;
use crate::src::internal::Rav1dTaskContext;
use crate::src::pic_or_buf::PicOrBuf;
use crate::src::strided::Strided as _;
use crate::src::strided::WithStride;
use crate::src::with_offset::WithOffset;
use aligned::Aligned;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fixed_offset, index, wrapped_index, wrapped_offset};
use core::ops::Range;
use std::ffi::c_int;
use std::ffi::c_uint;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BackupPlanes {
  None,
  Luma,
  Chroma,
  Both,
}

impl BackupPlanes {
  fn newly_required(self, previous: Self) -> Result<Self, Never> {
    use BackupPlanes::{Both, Chroma, Luma, None};

    Ok(match (self, previous) {
      (None, _) | (_, Both) | (Luma, Luma) | (Chroma, Chroma) => None,
      (Both, Luma) | (Chroma, None | Luma) => Chroma,
      (Both, Chroma) | (Luma, None | Chroma) => Luma,
      (Both, None) => Both,
    })
  }
}

struct BackupPlane<'picture> {
  destination: u64,
  source: PicOffset<'picture>,
  first_row: i64,
}

fn backup_plane_rows<BD: BitDepth>(
  destination: &DisjointMut<AlignedVec64<u8>>,
  plane: BackupPlane<'_>,
) -> Result<(), Never> {
  let Ok(stride) = fixed_offset(match plane.source.pixel_stride::<BD>() {
    Ok(native_stride) => native_stride,
  });
  let Ok(samples) = index(stride.unsigned_abs());
  let skipped = i64::from(stride.is_negative());
  let source_start = plane.first_row.wrapping_add(skipped);

  for row in 0_i64..2 {
    let delta = row.wrapping_add(skipped).wrapping_mul(stride);
    let Ok(destination_start) = wrapped_index(plane.destination.wrapping_add_signed(delta));
    let source_row = source_start.wrapping_add(row);
    let Ok(source_offset) = wrapped_offset(source_row.wrapping_mul(stride));
    let source = (plane.source + source_offset).slice::<BD>(samples);
    let mut backup = destination.mut_slice_as((destination_start.., ..samples));
    BD::pixel_copy(&mut backup, &source, samples);
  }

  Ok(())
}

fn backup2lines<BD: BitDepth>(
  dest: &DisjointMut<AlignedVec64<u8>>,
  offsets: [u64; 3],
  src: [PicOffset<'_>; 3],
  layout: Rav1dPixelLayout,
) -> Result<(), Never> {
  let [y, u, v] = src;
  let [y_offset, u_offset, v_offset] = offsets;
  let Ok(()) = backup_plane_rows::<BD>(
    dest,
    BackupPlane {
      destination: y_offset,
      source: y,
      first_row: 6,
    },
  );
  let chroma_row = match layout {
    Rav1dPixelLayout::I400 => return Ok(()),
    Rav1dPixelLayout::I420 => 2,
    Rav1dPixelLayout::I422 | Rav1dPixelLayout::I444 => 6,
  };
  let Ok(()) = backup_plane_rows::<BD>(
    dest,
    BackupPlane {
      destination: u_offset,
      source: u,
      first_row: chroma_row,
    },
  );
  let Ok(()) = backup_plane_rows::<BD>(
    dest,
    BackupPlane {
      destination: v_offset,
      source: v,
      first_row: chroma_row,
    },
  );

  Ok(())
}

#[inline(always)]
fn backup_columns<BD: BitDepth>(
  destination: &mut [[BD::Pixel; 2]; 8],
  source: PicOffset<'_>,
  first_column: c_int,
  row_count: u8,
) -> Result<(), Never> {
  let Ok(column_offset) = wrapped_offset(i64::from(first_column).wrapping_sub(2));
  let edge = source + column_offset;
  let Ok(()) = edge.for_rows_with_context::<BD, _, _>(
    Size {
      width: 2,
      height: u64::from(row_count),
    },
    &mut destination.iter_mut(),
    |_row, pixels, targets| match targets.next() {
      Some(target) => BD::pixel_copy(target, pixels, 2),
      None => {}
    },
  );

  Ok(())
}

#[inline(always)]
fn backup2x8<BD: BitDepth>(
  dst: &mut [[[BD::Pixel; 2]; 8]; 3],
  src: &[PicOffset<'_>; 3],
  column: c_int,
  layout: Rav1dPixelLayout,
  planes: BackupPlanes,
) -> Result<(), Never> {
  let [y_dst, u_dst, v_dst] = dst;
  let [y_src, u_src, v_src] = *src;

  match planes {
    BackupPlanes::Luma | BackupPlanes::Both => {
      let Ok(()) = backup_columns::<BD>(y_dst, y_src, column, 8);
    }
    BackupPlanes::None | BackupPlanes::Chroma => {}
  }

  match planes {
    BackupPlanes::Chroma | BackupPlanes::Both => {}
    BackupPlanes::None | BackupPlanes::Luma => return Ok(()),
  }

  let (chroma_column, chroma_rows) = match layout {
    Rav1dPixelLayout::I400 => return Ok(()),
    Rav1dPixelLayout::I420 => (column.wrapping_shr(1), 4),
    Rav1dPixelLayout::I422 => (column.wrapping_shr(1), 8),
    Rav1dPixelLayout::I444 => (column, 8),
  };
  let Ok(()) = backup_columns::<BD>(u_dst, u_src, chroma_column, chroma_rows);
  let Ok(()) = backup_columns::<BD>(v_dst, v_src, chroma_column, chroma_rows);

  Ok(())
}

fn adjust_strength(strength: u8, variance: c_uint) -> Result<c_int, Never> {
  match variance {
    0 => return Ok(0),
    1.. => {}
  }

  let variance_shift = match variance.wrapping_shr(6) {
    0 => 0,
    scaled @ 1.. => ulog2(scaled).min(12),
  };
  let scaled_strength = i32::from(strength).wrapping_mul(4_i32.wrapping_add(variance_shift));

  Ok(scaled_strength.wrapping_add(8).wrapping_shr(4))
}

#[derive(Clone, Copy)]
struct FilterStrength {
  primary: u8,
  secondary: u8,
}

fn filter_strength(strength: u8, depth_shift: u32) -> Result<FilterStrength, Never> {
  let secondary = strength & 3;
  let expanded = secondary.wrapping_add(u8::from(secondary == 3));

  Ok(FilterStrength {
    primary: strength.wrapping_shr(2).wrapping_shl(depth_shift),
    secondary: expanded.wrapping_shl(depth_shift),
  })
}

#[derive(PartialEq, Eq)]
pub(crate) enum CdefRowSpan {
  AboveBoundary,
  Body,
}

pub(crate) struct CdefRows {
  pub mask_offset: i32,
  pub rows: Range<c_int>,
  pub span: CdefRowSpan,
  pub superblock_row: c_int,
}

#[derive(Clone, Copy)]
enum CdefBoundary {
  PreviousSuperblock,
  FollowingSuperblock,
  PictureRows,
}

struct CdefLineRows<'frame> {
  saved: &'frame DisjointMut<AlignedVec64<u8>>,
  loop_filter: &'frame DisjointMut<AlignedVec64<u8>>,
  boundary: CdefBoundary,
  previous_row: i32,
  following_row: i32,
  saved_offsets: [u64; 3],
  filter_offsets: [u64; 3],
  luma_stride: i64,
  chroma_stride: i64,
  saved_luma_row: i32,
  saved_chroma_row: i32,
}

struct CdefPlaneRows<'frame> {
  source: PicOffset<'frame>,
  saved_offset: u64,
  loop_filter_offset: u64,
  column: i32,
  saved_row: i32,
  height: u8,
  stride: i64,
}

#[inline(always)]
fn cdef_line_view<'frame>(
  buffer: &'frame DisjointMut<AlignedVec64<u8>>,
  base: u64,
  origin: Point<i32>,
  stride: i64,
) -> Result<CdefTop<'frame>, Never> {
  let delta = i64::from(origin.y)
    .wrapping_mul(stride)
    .wrapping_add(i64::from(origin.x));
  let Ok(offset) = wrapped_index(base.wrapping_add_signed(delta));

  Ok(WithOffset {
    data: buffer,
    offset,
  })
}

#[inline(always)]
fn cdef_neighbour_rows<'frame>(
  lines: &CdefLineRows<'frame>,
  plane: CdefPlaneRows<'frame>,
) -> Result<(CdefTop<'frame>, CdefBottom<'frame>), Never> {
  let Ok(top) = match lines.boundary {
    CdefBoundary::PreviousSuperblock => cdef_line_view(
      lines.loop_filter,
      plane.loop_filter_offset,
      Point {
        x: plane.column,
        y: lines.previous_row,
      },
      plane.stride,
    ),
    CdefBoundary::FollowingSuperblock | CdefBoundary::PictureRows => cdef_line_view(
      lines.saved,
      plane.saved_offset,
      Point {
        x: plane.column,
        y: plane.saved_row,
      },
      plane.stride,
    ),
  };
  let bottom = match lines.boundary {
    CdefBoundary::FollowingSuperblock => {
      let Ok(buf) = cdef_line_view(
        lines.loop_filter,
        plane.loop_filter_offset,
        Point {
          x: plane.column,
          y: lines.following_row,
        },
        plane.stride,
      );
      WithOffset {
        data: PicOrBuf::Buf(WithStride {
          buf: buf.data,
          stride: plane.stride,
        }),
        offset: buf.offset,
      }
    }
    CdefBoundary::PreviousSuperblock | CdefBoundary::PictureRows => {
      let Ok(delta) = wrapped_offset(i64::from(plane.height).wrapping_mul(plane.stride));
      let Ok(pic) = WithOffset::pic(plane.source + delta);
      pic
    }
  };

  Ok((top, bottom))
}

struct CdefLineSources<'frame> {
  header: &'frame Rav1dFrameHeaderCdef,
  filter: &'frame DisjointMut<AlignedVec64<u8>>,
  filter_offsets: [u64; 3],
  saved_banks: [[u64; 3]; 2],
  previous_row: i32,
  following_row: i32,
  luma_stride: i64,
  chroma_stride: i64,
}

fn cdef_line_sources<BD: BitDepth>(
  f: &Rav1dFrameData,
  superblock_row: i32,
) -> Rav1dResult<CdefLineSources<'_>> {
  let frame = f.frame_hdr.as_ref().ok_or(EINVAL)?;
  let sequence = f.seq_hdr.as_ref().ok_or(EINVAL)?;
  let [coded_width, display_width] = frame.size.width;
  let [luma_stride, chroma_stride] = f.cur.stride.map(|bytes| {
    let Ok(samples) = fixed_offset(BD::pxstride(bytes));
    samples
  });
  let (filter, filter_offsets, previous_row, following_row) =
    match (coded_width != display_width).then_some(()) {
      Some(()) => (
        &f.lf.cdef_line_buf,
        f.lf.cdef_lpf_line,
        superblock_row.wrapping_sub(1).wrapping_mul(4),
        superblock_row.wrapping_mul(4).wrapping_add(2),
      ),
      None => {
        let Ok(sb_shift) = console_core_number_conversion::wrapped::<_, u32>(sequence.sb128);
        let row = superblock_row.wrapping_mul(4_i32.wrapping_shl(sb_shift));
        let following = row.wrapping_add(4_i32.wrapping_mul(i32::from(sequence.sb128)));
        (
          &f.lf.lr_line_buf,
          f.lf.lr_lpf_line,
          row.wrapping_sub(4),
          following.wrapping_add(2),
        )
      }
    };

  Ok(CdefLineSources {
    header: &frame.cdef,
    filter,
    filter_offsets,
    saved_banks: f.lf.cdef_line,
    previous_row,
    following_row,
    luma_stride,
    chroma_stride,
  })
}

struct CdefBlock<'frame, 'row, BD: BitDepth> {
  planes: [PicOffset<'frame>; 3],
  backups: &'row [[[BD::Pixel; 2]; 8]; 3],
  lines: &'row CdefLineRows<'frame>,
  column: i32,
  luma: FilterStrength,
  chroma: Option<FilterStrength>,
  sampling: Point<u32>,
  layout: Rav1dPixelLayout,
  damping: u8,
  edges: CdefEdgeFlags,
  bitdepth: BD,
}

#[inline(always)]
fn filter_cdef_block<BD: BitDepth>(
  dsp: &Rav1dCdefDSPContext,
  block: CdefBlock<'_, '_, BD>,
) -> Rav1dResult {
  let [luma, _, _] = block.planes;
  let mut variance = 0;
  let direction = match (
    block.luma.primary,
    block.chroma.as_ref().map(|strength| strength.primary),
  ) {
    (0, None | Some(0)) => 0,
    (_luma, _chroma) => {
      let found = dsp.dir.call::<BD>(luma, &mut variance, block.bitdepth)?;
      found
    }
  };
  let (luma_strength, luma_direction) = match block.luma.primary {
    0 => (0, 0),
    primary @ 1.. => {
      let Ok(adjusted) = adjust_strength(primary, variance);
      (adjusted, direction)
    }
  };

  match (luma_strength, block.luma.secondary) {
    (0, 0) => {}
    (primary, secondary) => {
      let [luma_filter, _, _] = &dsp.fb;
      let [left, _, _] = block.backups;
      let [saved, _, _] = block.lines.saved_offsets;
      let [filter, _, _] = block.lines.filter_offsets;
      let Ok((top, bottom)) = cdef_neighbour_rows(
        block.lines,
        CdefPlaneRows {
          source: luma,
          saved_offset: saved,
          loop_filter_offset: filter,
          column: block.column,
          saved_row: block.lines.saved_luma_row,
          height: 8,
          stride: block.lines.luma_stride,
        },
      );
      luma_filter.call(CdefFilter {
        shape: CdefBlockShape::EightByEight,
        destination: luma,
        left,
        top,
        bottom,
        primary,
        secondary,
        direction: luma_direction,
        damping: block.damping,
        edges: block.edges,
        bitdepth: block.bitdepth,
      })?;
    }
  }

  let chroma = match block.chroma {
    Some(strength) => strength,
    None => return Ok(()),
  };

  filter_cdef_chroma(dsp, block, chroma, direction)
}

#[inline(always)]
fn filter_cdef_chroma<BD: BitDepth>(
  dsp: &Rav1dCdefDSPContext,
  block: CdefBlock<'_, '_, BD>,
  strength: FilterStrength,
  direction: i32,
) -> Rav1dResult {
  let [eight_square, four_wide, four_square] = &dsp.fb;
  let (shape, filter, remap) = match block.layout {
    Rav1dPixelLayout::I444 => (
      CdefBlockShape::EightByEight,
      eight_square,
      &[0_u8, 1, 2, 3, 4, 5, 6, 7],
    ),
    Rav1dPixelLayout::I422 => (
      CdefBlockShape::FourByEight,
      four_wide,
      &[7_u8, 0, 2, 4, 5, 6, 6, 6],
    ),
    Rav1dPixelLayout::I420 => (
      CdefBlockShape::FourByFour,
      four_square,
      &[0_u8, 1, 2, 3, 4, 5, 6, 7],
    ),
    Rav1dPixelLayout::I400 => return Err(EINVAL),
  };
  let chroma_direction = match strength.primary {
    0 => 0,
    1.. => {
      let Ok(direction_index) = wrapped_index(direction);
      let mapped = remap.get(direction_index).ok_or(EINVAL)?;
      i32::from(*mapped)
    }
  };
  let offsets = block
    .lines
    .saved_offsets
    .into_iter()
    .zip(block.lines.filter_offsets);
  let planes = block.planes.into_iter().zip(block.backups).zip(offsets);

  for ((source, left), (saved, next)) in planes.skip(1) {
    let Ok((top, bottom)) = cdef_neighbour_rows(
      block.lines,
      CdefPlaneRows {
        source,
        saved_offset: saved,
        loop_filter_offset: next,
        column: block.column.wrapping_shr(block.sampling.x),
        saved_row: block.lines.saved_chroma_row,
        height: 8_u8.wrapping_shr(block.sampling.y),
        stride: block.lines.chroma_stride,
      },
    );
    filter.call(CdefFilter {
      shape,
      destination: source,
      left,
      top,
      bottom,
      primary: i32::from(strength.primary),
      secondary: strength.secondary,
      direction: chroma_direction,
      damping: block.damping.wrapping_sub(1),
      edges: block.edges,
      bitdepth: block.bitdepth,
    })?;
  }

  Ok(())
}

struct CdefBlockPresets {
  luma: FilterStrength,
  chroma: Option<FilterStrength>,
  planes: BackupPlanes,
}

fn cdef_block_presets(
  cdef: &Rav1dFrameHeaderCdef,
  strength_index: i8,
  depth_shift: u32,
) -> Rav1dResult<Option<CdefBlockPresets>> {
  match strength_index {
    -1 => return Ok(None),
    _preset => {}
  }

  let Ok(table_index) = wrapped_index(strength_index);
  let y = cdef.y_strength.get(table_index).copied().ok_or(EINVAL)?;
  let uv = cdef.uv_strength.get(table_index).copied().ok_or(EINVAL)?;
  let planes = match (y, uv) {
    (0, 0) => return Ok(None),
    (0, 1..) => BackupPlanes::Chroma,
    (1.., 0) => BackupPlanes::Luma,
    (1.., 1..) => BackupPlanes::Both,
  };
  let Ok(luma) = filter_strength(y, depth_shift);
  let chroma = match uv {
    0 => None,
    1.. => {
      let Ok(chroma_strength) = filter_strength(uv, depth_shift);
      Some(chroma_strength)
    }
  };

  Ok(Some(CdefBlockPresets {
    luma,
    chroma,
    planes,
  }))
}

#[inline(always)]
fn cdef_advance_columns(
  pictures: &mut [PicOffset<'_>; 3],
  columns: u8,
  chroma_shift: u32,
) -> Result<(), Never> {
  let Ok(luma) = index(columns);
  let Ok(chroma) = index(columns.wrapping_shr(chroma_shift));
  let [y, u, v] = pictures;
  *y += luma;
  *u += chroma;
  *v += chroma;

  Ok(())
}

#[inline(always)]
fn cdef_advance_rows<BD: BitDepth>(
  pictures: &mut [PicOffset<'_>; 3],
  chroma_shift: u32,
) -> Result<(), Never> {
  for (picture, shift) in pictures.iter_mut().zip([0, chroma_shift, chroma_shift]) {
    let Ok(stride) = fixed_offset(match picture.pixel_stride::<BD>() {
      Ok(native) => native,
    });
    let Ok(step) = wrapped_offset(stride.wrapping_shr(shift).wrapping_mul(8));
    *picture += step;
  }

  Ok(())
}

struct CdefFilterRow<'frame, 'state, BD: BitDepth> {
  frame: &'frame Rav1dFrameData,
  header: &'frame Rav1dFrameHeaderCdef,
  lines: CdefLineRows<'frame>,
  pictures: [PicOffset<'frame>; 3],
  edges: &'state mut CdefEdgeFlags,
  secondary_backup: &'state mut bool,
  block_row: i32,
  mask_offset: i32,
  layout: Rav1dPixelLayout,
  sampling: Point<u32>,
  damping: u8,
  bitdepth: BD,
  depth_shift: u32,
}

fn filter_cdef_row<BD: BitDepth>(row: CdefFilterRow<'_, '_, BD>) -> Rav1dResult {
  let mut superblock_pictures = row.pictures;
  let mut backups: Align16<_> = Aligned([[[[BD::Pixel::from(0_u8); 2]; 8]; 3]; 2]);
  let mut previous_planes = BackupPlanes::None;
  let mut last_skip = true;
  let superblock_columns = row.frame.sb128w.wrapping_shl(1);
  let Ok(mask_row) = wrapped_index((row.block_row & 30).wrapping_shr(1));
  row.edges.remove(CdefEdgeFlags::HAVE_LEFT);
  row.edges.insert(CdefEdgeFlags::HAVE_RIGHT);

  'superblocks: for sbx in 0..superblock_columns {
    let mut pictures = superblock_pictures;
    let Ok(()) = cdef_advance_columns(&mut superblock_pictures, 64, row.sampling.x);
    let Ok(mask_index) = wrapped_index(row.mask_offset.wrapping_add(sbx.wrapping_shr(1)));
    let mask = row.frame.lf.mask.get(mask_index).ok_or(EINVAL)?;
    let within = (row.block_row & 16).wrapping_shr(3).wrapping_add(sbx & 1);
    let Ok(preset_index) = wrapped_index(within);
    let preset_entry = mask.cdef_idx.get(preset_index).ok_or(EINVAL)?;
    let Ok(strength_index) = preset_entry.get();
    let presets = cdef_block_presets(row.header, strength_index, row.depth_shift)?;
    let preset = match presets {
      Some(active) => active,
      None => {
        last_skip = true;
        row.edges.insert(CdefEdgeFlags::HAVE_LEFT);
        continue 'superblocks;
      }
    };
    let [low, high] = mask.noskip_mask.get(mask_row).ok_or(EINVAL)?;
    let Ok(lower) = low.get();
    let Ok(upper) = high.get();
    let noskip_mask = u32::from(upper).wrapping_shl(16) | u32::from(lower);
    let start = sbx.wrapping_mul(16);
    let end = start.wrapping_add(16).min(row.frame.bw);

    'blocks: for bx in (start..end).step_by(2) {
      let current_pictures = pictures;
      let Ok(()) = cdef_advance_columns(&mut pictures, 8, row.sampling.x);

      match (bx.wrapping_add(2) >= row.frame.bw).then_some(()) {
        Some(()) => row.edges.remove(CdefEdgeFlags::HAVE_RIGHT),
        None => {}
      }

      let block_mask = 3_u32.wrapping_shl((bx & 30).cast_unsigned());

      match (noskip_mask & block_mask == 0).then_some(()) {
        Some(()) => {
          last_skip = true;
          row.edges.insert(CdefEdgeFlags::HAVE_LEFT);
          continue 'blocks;
        }
        None => {}
      }

      let Ok(left_planes) = match last_skip.then_some(()) {
        Some(()) => Ok(preset.planes),
        None => preset.planes.newly_required(previous_planes),
      };
      let [first, second] = &mut *backups;
      let (left, right) = match row.secondary_backup.then_some(()) {
        Some(()) => (second, first),
        None => (first, second),
      };

      match (left_planes != BackupPlanes::None && row.edges.contains(CdefEdgeFlags::HAVE_LEFT))
        .then_some(())
      {
        Some(()) => {
          let Ok(()) = backup2x8::<BD>(left, &current_pictures, 0, row.layout, left_planes);
        }
        None => {}
      }

      match row.edges.contains(CdefEdgeFlags::HAVE_RIGHT).then_some(()) {
        Some(()) => {
          let Ok(()) = backup2x8::<BD>(right, &current_pictures, 8, row.layout, preset.planes);
        }
        None => {}
      }

      let block = CdefBlock {
        planes: current_pictures,
        backups: left,
        lines: &row.lines,
        column: bx.wrapping_mul(4),
        luma: preset.luma,
        chroma: preset.chroma,
        sampling: row.sampling,
        layout: row.layout,
        damping: row.damping,
        edges: *row.edges,
        bitdepth: row.bitdepth,
      };
      filter_cdef_block(&row.frame.dsp.cdef, block)?;
      *row.secondary_backup = !*row.secondary_backup;
      previous_planes = preset.planes;
      last_skip = false;
      row.edges.insert(CdefEdgeFlags::HAVE_LEFT);
    }

    row.edges.insert(CdefEdgeFlags::HAVE_LEFT);
  }

  Ok(())
}

pub(crate) fn rav1d_cdef_brow<BD: BitDepth>(
  c: &Rav1dContext,
  tc: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
  mut pictures: [PicOffset<'_>; 3],
  bounds: CdefRows,
) -> Rav1dResult {
  let sources = cdef_line_sources::<BD>(f, bounds.superblock_row)?;
  let bitdepth = BD::from_c(f.bitdepth_max);
  let depth_shift = match BD::BPC {
    BPC::BPC8 => 0,
    BPC::BPC16 => f.cur.p.bpc.wrapping_sub(8),
  };
  let damping = sources.header.damping.wrapping_add(depth_shift);
  let mut edges = match (bounds.rows.start > 0).then_some(()) {
    Some(()) => CdefEdgeFlags::HAVE_BOTTOM | CdefEdgeFlags::HAVE_TOP,
    None => CdefEdgeFlags::HAVE_BOTTOM,
  };
  let layout = f.cur.p.layout;
  let sampling = Point {
    x: u32::from(layout != Rav1dPixelLayout::I444),
    y: u32::from(layout == Rav1dPixelLayout::I420),
  };
  let threaded = c.tc.len() > 1;
  let threaded_row = bounds.superblock_row.wrapping_mul(i32::from(threaded));
  let saved_luma_row = threaded_row.wrapping_mul(4);
  let saved_chroma_row = threaded_row.wrapping_mul(8);
  let mut secondary_backup = false;

  for by in bounds.rows.clone().step_by(2) {
    let [first, second] = sources.saved_banks;
    let (saved_offsets, future_offsets) = match tc.top_pre_cdef_toggle {
      0 => (first, second),
      _current => (second, first),
    };
    let after_row = by.wrapping_add(2);
    let previous =
      (threaded && bounds.span == CdefRowSpan::AboveBoundary && by == bounds.rows.start)
        .then_some(());
    let following =
      (threaded && bounds.span == CdefRowSpan::Body && after_row >= bounds.rows.end)
        .then_some(());
    let boundary = match (previous, following) {
      (Some(()), _) => CdefBoundary::PreviousSuperblock,
      (None, Some(())) => CdefBoundary::FollowingSuperblock,
      (None, None) => CdefBoundary::PictureRows,
    };
    let lines = CdefLineRows {
      saved: &f.lf.cdef_line_buf,
      loop_filter: sources.filter,
      boundary,
      previous_row: sources.previous_row,
      following_row: sources.following_row,
      saved_offsets,
      filter_offsets: sources.filter_offsets,
      luma_stride: sources.luma_stride,
      chroma_stride: sources.chroma_stride,
      saved_luma_row,
      saved_chroma_row,
    };

    match (after_row >= f.bh).then_some(()) {
      Some(()) => edges.remove(CdefEdgeFlags::HAVE_BOTTOM),
      None => {}
    }

    match ((!threaded
      || bounds.span == CdefRowSpan::AboveBoundary
      || after_row < bounds.rows.end)
      && edges.contains(CdefEdgeFlags::HAVE_BOTTOM))
    .then_some(())
    {
      Some(()) => {
        let [luma, u, v] = future_offsets;
        let y_delta = i64::from(saved_luma_row).wrapping_mul(sources.luma_stride);
        let uv_delta = i64::from(saved_chroma_row).wrapping_mul(sources.chroma_stride);
        let offsets = [
          luma.wrapping_add_signed(y_delta),
          u.wrapping_add_signed(uv_delta),
          v.wrapping_add_signed(uv_delta),
        ];
        let Ok(()) = backup2lines::<BD>(&f.lf.cdef_line_buf, offsets, pictures, layout);
      }
      None => {}
    }

    let row = CdefFilterRow {
      frame: f,
      header: sources.header,
      lines,
      pictures,
      edges: &mut edges,
      secondary_backup: &mut secondary_backup,
      block_row: by,
      mask_offset: bounds.mask_offset,
      layout,
      sampling,
      damping,
      bitdepth,
      depth_shift: u32::from(depth_shift),
    };
    filter_cdef_row(row)?;

    let Ok(()) = cdef_advance_rows::<BD>(&mut pictures, sampling.y);

    tc.top_pre_cdef_toggle ^= 1;
    edges.insert(CdefEdgeFlags::HAVE_TOP);
  }

  Ok(())
}
