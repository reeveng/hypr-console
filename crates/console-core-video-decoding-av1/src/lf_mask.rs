#![forbid(unsafe_code)]
use crate::include::common::intops::clip;
use crate::include::common::intops::iclip;
use crate::include::dav1d::headers::Rav1dFrameHeader;
use crate::include::dav1d::headers::Rav1dLoopfilterModeRefDeltas;
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::headers::Rav1dRestorationType;
use crate::src::align::Align16;
use crate::src::align::ArrayDefault;
use crate::src::internal::Bxy;
use crate::src::levels::BlockSize;
use crate::src::levels::SegmentId;
use crate::src::levels::TxfmSize;
use crate::src::relaxed_atomic::RelaxedAtomic;
use crate::src::tables::{TxfmInfo, dav1d_txfm_dimensions};
use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use console_core_number_conversion::{checked_index, index, wrapped};
use console_core_synchronization::nonpoison::RwLock;
use core::num::NonZeroU32;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering::Relaxed;

#[repr(C)]
pub struct Av1FilterLUT {
  pub e: [u8; 64],
  pub i: [u8; 64],
  pub sharp: [u64; 2],
}

impl Default for Av1FilterLUT {
  fn default() -> Self {
    Self {
      e: [0; 64],
      i: [0; 64],
      sharp: Default::default(),
    }
  }
}

impl ArrayDefault for Av1FilterLUT {
  fn default() -> Self {
    Default::default()
  }
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Av1RestorationUnit {
  pub r#type: Rav1dRestorationType,
  pub filter_h: [i8; 3],
  pub filter_v: [i8; 3],
  pub sgr_weights: [i8; 2],
}

#[derive(Default)]
#[repr(C)]
pub struct Av1Filter {
  pub filter_y: [[[[RelaxedAtomic<u16>; 2]; 3]; 32]; 2],
  pub filter_uv: [[[[RelaxedAtomic<u16>; 2]; 2]; 32]; 2],

  pub cdef_idx: [RelaxedAtomic<i8>; 4],

  pub noskip_mask: [[RelaxedAtomic<u16>; 2]; 16],
}

#[derive(Default)]
#[repr(C)]
pub struct Av1Restoration {
  pub lr: [[RwLock<Av1RestorationUnit>; 4]; 3],
}

type TransformMap = [[[[u8; 32]; 32]; 2]; 2];

#[derive(Clone, Copy)]
enum TransformSplitDepth {
  Root,
  Child,
  Leaf,
}

struct DecompTxArguments {
  y_off: u8,
  x_off: u8,
}

fn transform_dimensions(
  size: TxfmSize,
) -> Result<Option<&'static TxfmInfo>, console_core_never::Never> {
  let Ok(ordinal) = size.ordinal();
  let Ok(index) = console_core_number_conversion::index(ordinal);

  Ok(dav1d_txfm_dimensions.get(index))
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The transform split depth advances from root to child to leaf and cannot recurse at a leaf"
  )
)]
fn decomp_tx(
  map: &mut TransformMap,
  size: TxfmSize,
  depth: TransformSplitDepth,
  offsets: DecompTxArguments,
  masks: &[u16; 2],
) -> Result<(), console_core_never::Never> {
  let Ok(info) = transform_dimensions(size);
  let dimensions = match info {
    Some(dimensions) => dimensions,
    None => {
      console_core_logging::debug!("No AV1 filter-mask dimensions for {size:?}");
      return Ok(());
    }
  };
  let &[root, child] = masks;
  let split = match (&depth, size) {
    (TransformSplitDepth::Leaf, _) | (_, TxfmSize::S4x4) => None,
    (TransformSplitDepth::Root, _) => Some((root, TransformSplitDepth::Child)),
    (TransformSplitDepth::Child, _) => Some((child, TransformSplitDepth::Leaf)),
  };
  let bit = u32::from(offsets.y_off)
    .wrapping_mul(4)
    .wrapping_add(u32::from(offsets.x_off));
  let next = match split {
    Some((mask, depth_2)) => match (mask.wrapping_shr(bit) & 1).cmp(&0) {
      core::cmp::Ordering::Greater => depth_2,
      core::cmp::Ordering::Equal | core::cmp::Ordering::Less => {
        return write_transform_leaf(map, offsets, dimensions);
      }
    },
    None => return write_transform_leaf(map, offsets, dimensions),
  };

  let quadrants: &[(u8, u8)] = match dimensions.w.cmp(&dimensions.h) {
    core::cmp::Ordering::Equal => &[(0, 0), (0, 1), (1, 0), (1, 1)],
    core::cmp::Ordering::Greater => &[(0, 0), (0, 1)],
    core::cmp::Ordering::Less => &[(0, 0), (1, 0)],
  };

  for &(y, x) in quadrants {
    let child_offsets = DecompTxArguments {
      y_off: offsets.y_off.wrapping_mul(2).wrapping_add(y),
      x_off: offsets.x_off.wrapping_mul(2).wrapping_add(x),
    };
    let Ok(()) = decomp_tx(map, dimensions.sub, next, child_offsets, masks);
  }

  Ok(())
}

fn write_transform_leaf(
  map: &mut TransformMap,
  offsets: DecompTxArguments,
  dimensions: &TxfmInfo,
) -> Result<(), console_core_never::Never> {
  let width = u32::from(dimensions.w);
  let height = u32::from(dimensions.h);
  let x = u32::from(offsets.x_off).saturating_mul(width);
  let y = u32::from(offsets.y_off).saturating_mul(height);
  let x_end = x.saturating_add(width);
  let y_end = y.saturating_add(height);

  match (x_end <= 32 && y_end <= 32).then_some(()) {
    Some(()) => {}
    None => {
      console_core_logging::debug!("AV1 transform leaf falls outside the filter-mask map");
      return Ok(());
    }
  }

  let Ok(first_x) = console_core_number_conversion::index(x);
  let Ok(last_x) = console_core_number_conversion::index(x_end);
  let Ok(first_y) = console_core_number_conversion::index(y);
  let Ok(last_y) = console_core_number_conversion::index(y_end);
  let [[widths, width_edges], [heights, height_edges]] = map;

  let rows = widths
    .iter_mut()
    .zip(heights)
    .zip(width_edges)
    .take(last_y)
    .skip(first_y);

  for ((width_row, height_row), edge_row) in rows {
    match (
      width_row.get_mut(first_x..last_x),
      height_row.get_mut(first_x..last_x),
      edge_row.get_mut(first_x),
    ) {
      (Some(widths_2), Some(heights_2), Some(edge)) => {
        widths_2.fill(dimensions.lw.min(2));
        heights_2.fill(dimensions.lh.min(2));
        *edge = dimensions.w;
      }
      (_, _, _) => {}
    }
  }

  let first_row = height_edges
    .get_mut(first_y)
    .and_then(|row| row.get_mut(first_x..last_x));

  match first_row {
    Some(edge) => edge.fill(dimensions.h),
    None => {}
  }

  Ok(())
}

type DirectionMask<const WIDTHS: usize> = [[[RelaxedAtomic<u16>; 2]; WIDTHS]; 32];
type TransformGrid = [[u8; 32]; 32];

#[derive(Clone, Copy)]
pub(crate) enum EdgeScan {
  Boundary,
  AllTransforms,
}

#[derive(Clone, Copy)]
struct MaskSpan {
  start: u32,
  length: u32,
}

#[derive(Clone, Copy)]
struct MaskRegion {
  x: MaskSpan,
  y: MaskSpan,
}

#[derive(Debug)]
pub(crate) enum FilterMaskError {
  Region,
  Neighbors,
  Width,
  Transform,
  Cache,
  Reference,
}

impl MaskRegion {
  fn new(rect: Rectangle<u32>) -> Result<Self, FilterMaskError> {
    let Rectangle { origin, size } = rect;
    let x = MaskSpan::new(origin.x..origin.x.saturating_add(size.width))?;
    let y = MaskSpan::new(origin.y..origin.y.saturating_add(size.height))?;

    Ok(Self { x, y })
  }
}

impl MaskSpan {
  fn new(span: core::ops::Range<u32>) -> Result<Self, FilterMaskError> {
    match ::core::primitive::bool::then_some(span.end <= 32 && span.start < span.end, ()) {
      ::core::option::Option::Some(()) => {
        let start = span.start;
        let length = span.end.wrapping_sub(start);
        Ok(Self { start, length })
      }
      ::core::option::Option::None => Err(FilterMaskError::Region),
    }
  }
}

pub(crate) struct EdgeNeighbors<'a> {
  pub above: &'a mut [u8],
  pub left: &'a mut [u8],
}

impl EdgeNeighbors<'_> {
  fn for_region(self, region: MaskRegion) -> Result<Self, FilterMaskError> {
    let Ok(width) = index(region.x.length);
    let Ok(height) = index(region.y.length);
    let above = self
      .above
      .get_mut(..width)
      .ok_or(FilterMaskError::Neighbors)?;
    let left = self
      .left
      .get_mut(..height)
      .ok_or(FilterMaskError::Neighbors)?;

    Ok(Self { above, left })
  }
}

#[derive(Clone, Copy)]
struct EdgeAxis {
  cross: MaskSpan,
  along: MaskSpan,
  word_bits: u32,
}

struct EdgeBits {
  offset: u32,
  width: u8,
  bits: u32,
}

impl EdgeAxis {
  fn set<const WIDTHS: usize>(
    self,
    masks: &DirectionMask<WIDTHS>,
    EdgeBits {
      offset,
      width,
      bits,
    }: EdgeBits,
  ) -> Result<(), FilterMaskError> {
    let Ok(edge_index) = index(self.cross.start.wrapping_add(offset));
    let Ok(width_index) = index(width);
    let entry = masks.get(edge_index).and_then(|edge| edge.get(width_index));
    let [lower, upper] = entry.ok_or(FilterMaskError::Width)?;
    let low = bits & 1u32.wrapping_shl(self.word_bits).wrapping_sub(1);
    let high = bits.wrapping_shr(self.word_bits);
    let Ok(lower_bits) = wrapped::<_, u16>(low);
    let Ok(upper_bits) = wrapped::<_, u16>(high);
    let Ok(()) = set_mask_word(lower, lower_bits);
    let Ok(()) = set_mask_word(upper, upper_bits);

    Ok(())
  }

  fn set_uniform<const WIDTHS: usize>(
    self,
    masks: &DirectionMask<WIDTHS>,
    neighbors: &mut [u8],
    transform: UniformTransform,
    scan: EdgeScan,
  ) -> Result<(), FilterMaskError> {
    for (line, previous) in (0..self.along.length).zip(neighbors.iter_mut()) {
      let bit = 1u32.wrapping_shl(self.along.start.wrapping_add(line));
      self.set(
        masks,
        EdgeBits {
          offset: 0,
          width: transform.width.min(*previous),
          bits: bit,
        },
      )?;
      *previous = transform.width;
    }

    let step = match scan {
      EdgeScan::Boundary => return Ok(()),
      EdgeScan::AllTransforms => transform.step,
    };
    let bits = 1u64
      .wrapping_shl(self.along.length)
      .wrapping_sub(1)
      .wrapping_shl(self.along.start);
    let Ok(line_bits) = wrapped::<_, u32>(bits);
    let interior_edges = self.cross.length.saturating_sub(1).wrapping_div(step.get());

    for edge in 1..=interior_edges {
      let offset = edge.wrapping_mul(step.get());
      self.set(
        masks,
        EdgeBits {
          offset,
          width: transform.width,
          bits: line_bits,
        },
      )?;
    }

    Ok(())
  }
}

fn set_mask_word(word: &RelaxedAtomic<u16>, bits: u16) -> Result<(), Never> {
  match bits {
    0 => {}
    _ => {
      let Ok(()) = word.update(|previous| previous | bits);
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
struct UniformTransform {
  width: u8,
  step: NonZeroU32,
}

#[derive(Clone, Copy)]
enum EdgeDirection {
  Vertical,
  Horizontal,
}

impl EdgeDirection {
  fn sample(self, grid: &TransformGrid, coordinates: Point<u32>) -> Result<u8, FilterMaskError> {
    let Point { x, y } = match self {
      Self::Vertical => coordinates,
      Self::Horizontal => Point {
        x: coordinates.y,
        y: coordinates.x,
      },
    };
    let Ok(row) = index(y);
    let Ok(column) = index(x);

    grid
      .get(row)
      .and_then(|values| values.get(column))
      .copied()
      .ok_or(FilterMaskError::Transform)
  }
}

struct InterTransforms<'a> {
  widths: &'a TransformGrid,
  steps: &'a TransformGrid,
  direction: EdgeDirection,
}

impl InterTransforms<'_> {
  fn set_edges(
    &self,
    axis: EdgeAxis,
    masks: &DirectionMask<3>,
    neighbors: &mut [u8],
    scan: EdgeScan,
  ) -> Result<(), FilterMaskError> {
    for (line, previous) in (0..axis.along.length).zip(neighbors.iter_mut()) {
      let bit = 1u32.wrapping_shl(axis.along.start.wrapping_add(line));
      let mut cell = Point { x: 0, y: line };
      let initial_width = self.direction.sample(self.widths, cell)?;
      axis.set(
        masks,
        EdgeBits {
          offset: cell.x,
          width: initial_width.min(*previous),
          bits: bit,
        },
      )?;

      match scan {
        EdgeScan::Boundary => {}
        EdgeScan::AllTransforms => {
          self.set_inner_edges(axis, masks, &mut cell, initial_width, bit)?
        }
      }

      cell.x = axis.cross.length.wrapping_sub(1);
      let final_width = self.direction.sample(self.widths, cell)?;
      *previous = final_width;
    }

    Ok(())
  }

  fn set_inner_edges(
    &self,
    axis: EdgeAxis,
    masks: &DirectionMask<3>,
    cell: &mut Point<u32>,
    mut previous_width: u8,
    bit: u32,
  ) -> Result<(), FilterMaskError> {
    for _ in 0..axis.cross.length {
      let step = self.direction.sample(self.steps, *cell)?;

      match step {
        0 => return Err(FilterMaskError::Transform),
        _ => cell.x = cell.x.wrapping_add(u32::from(step)),
      }

      match cell.x.cmp(&axis.cross.length) {
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => return Ok(()),
        core::cmp::Ordering::Less => {}
      }

      let width = self.direction.sample(self.widths, *cell)?;
      axis.set(
        masks,
        EdgeBits {
          offset: cell.x,
          width: width.min(previous_width),
          bits: bit,
        },
      )?;
      previous_width = width;
    }

    Err(FilterMaskError::Transform)
  }
}

fn inter_transform_map(
  size: TxfmSize,
  region: MaskRegion,
  masks: &[u16; 2],
) -> Result<TransformMap, FilterMaskError> {
  let Ok(info) = transform_dimensions(size);
  let dimensions = info.ok_or(FilterMaskError::Transform)?;
  let horizontal_step =
    NonZeroU32::new(u32::from(dimensions.w)).ok_or(FilterMaskError::Transform)?;
  let vertical_step =
    NonZeroU32::new(u32::from(dimensions.h)).ok_or(FilterMaskError::Transform)?;
  let mut map = [[[[0u8; 32]; 32]; 2]; 2];
  let rows = region.y.length.div_ceil(vertical_step.get());
  let columns = region.x.length.div_ceil(horizontal_step.get());

  for row in 0..rows {
    let y_off = u8::try_from(row).map_err(|_| FilterMaskError::Transform)?;

    for column in 0..columns {
      let x_off = u8::try_from(column).map_err(|_| FilterMaskError::Transform)?;
      let Ok(()) = decomp_tx(
        &mut map,
        size,
        TransformSplitDepth::Root,
        DecompTxArguments { y_off, x_off },
        masks,
      );
    }
  }

  Ok(map)
}

fn mask_edges_inter(
  masks: &[DirectionMask<3>; 2],
  rectangle: Rectangle<u32>,
  scan: EdgeScan,
  size: TxfmSize,
  split_masks: &[u16; 2],
  neighbors: EdgeNeighbors<'_>,
) -> Result<(), FilterMaskError> {
  let region = MaskRegion::new(rectangle)?;
  let EdgeNeighbors { above, left } = neighbors.for_region(region)?;
  let [vertical, horizontal] = masks;
  let map = inter_transform_map(size, region, split_masks)?;
  let [[widths, width_steps], [heights, height_steps]] = &map;
  let vertical_axis = EdgeAxis {
    cross: region.x,
    along: region.y,
    word_bits: 16,
  };
  let horizontal_axis = EdgeAxis {
    cross: region.y,
    along: region.x,
    word_bits: 16,
  };
  let column_edges = InterTransforms {
    widths,
    steps: width_steps,
    direction: EdgeDirection::Vertical,
  };
  let row_edges = InterTransforms {
    widths: heights,
    steps: height_steps,
    direction: EdgeDirection::Horizontal,
  };
  column_edges.set_edges(vertical_axis, vertical, left, scan)?;

  row_edges.set_edges(horizontal_axis, horizontal, above, scan)
}

fn mask_edges_uniform<const WIDTHS: usize>(
  masks: &[DirectionMask<WIDTHS>; 2],
  region: MaskRegion,
  size: TxfmSize,
  word_bits: Size<u32>,
  scan: EdgeScan,
  neighbors: EdgeNeighbors<'_>,
) -> Result<(), FilterMaskError> {
  let EdgeNeighbors { above, left } = neighbors.for_region(region)?;
  let Ok(info) = transform_dimensions(size);
  let dimensions = info.ok_or(FilterMaskError::Transform)?;
  let width_limit = match WIDTHS {
    2 => 1,
    _ => 2,
  };
  let horizontal_step =
    NonZeroU32::new(u32::from(dimensions.w)).ok_or(FilterMaskError::Transform)?;
  let vertical_step =
    NonZeroU32::new(u32::from(dimensions.h)).ok_or(FilterMaskError::Transform)?;
  let width = UniformTransform {
    width: dimensions.lw.min(width_limit),
    step: horizontal_step,
  };
  let height = UniformTransform {
    width: dimensions.lh.min(width_limit),
    step: vertical_step,
  };
  let [vertical, horizontal] = masks;
  let vertical_axis = EdgeAxis {
    cross: region.x,
    along: region.y,
    word_bits: word_bits.height,
  };
  let horizontal_axis = EdgeAxis {
    cross: region.y,
    along: region.x,
    word_bits: word_bits.width,
  };
  vertical_axis.set_uniform(vertical, left, width, scan)?;

  horizontal_axis.set_uniform(horizontal, above, height, scan)
}

fn mask_edges_intra(
  masks: &[DirectionMask<3>; 2],
  rectangle: Rectangle<u32>,
  size: TxfmSize,
  neighbors: EdgeNeighbors<'_>,
) -> Result<(), FilterMaskError> {
  let region = MaskRegion::new(rectangle)?;
  let word_bits = Size {
    width: 16,
    height: 16,
  };

  mask_edges_uniform(
    masks,
    region,
    size,
    word_bits,
    EdgeScan::AllTransforms,
    neighbors,
  )
}

fn mask_edges_chroma(
  masks: &[DirectionMask<2>; 2],
  rectangle: Rectangle<u32>,
  scan: EdgeScan,
  size: TxfmSize,
  neighbors: EdgeNeighbors<'_>,
  layout: Rav1dPixelLayout,
) -> Result<(), FilterMaskError> {
  let region = MaskRegion::new(rectangle)?;
  let word_bits = match layout {
    Rav1dPixelLayout::I400 | Rav1dPixelLayout::I444 => Size {
      width: 16,
      height: 16,
    },
    Rav1dPixelLayout::I422 => Size {
      width: 8,
      height: 16,
    },
    Rav1dPixelLayout::I420 => Size {
      width: 8,
      height: 8,
    },
  };

  mask_edges_uniform(masks, region, size, word_bits, scan, neighbors)
}

pub(crate) struct FilterMaskBlock<'a> {
  pub mask: &'a Av1Filter,
  pub level_cache: &'a [AtomicU8],
  pub stride: i64,
  pub levels: &'a Align16<[[[u8; 2]; 8]; 4]>,
  pub origin: Bxy,
  pub frame: Size<i32>,
  pub size: BlockSize,
  pub ytx: TxfmSize,
  pub uvtx: TxfmSize,
  pub layout: Rav1dPixelLayout,
  pub luma: EdgeNeighbors<'a>,
  pub chroma: Option<(&'a mut [u8], &'a mut [u8])>,
}

pub(crate) enum FilterMaskPrediction<'a> {
  Intra,
  Inter {
    reference: i8,
    motion: FilterMaskMotion,
    scan: EdgeScan,
    split_masks: &'a [u16; 2],
  },
}

pub(crate) enum FilterMaskMotion {
  Global,
  Other,
}

impl FilterMaskPrediction<'_> {
  fn levels(&self, levels: &[[[u8; 2]; 8]; 4]) -> Result<[u8; 4], FilterMaskError> {
    match self {
      Self::Intra => {
        let [vertical, horizontal, u, v] = levels;
        let [[left, _], ..] = vertical;
        let [[above, _], ..] = horizontal;
        let [[chroma_u, _], ..] = u;
        let [[chroma_v, _], ..] = v;
        Ok([*left, *above, *chroma_u, *chroma_v])
      }
      Self::Inter {
        reference, motion, ..
      } => {
        let code = u8::try_from(*reference).map_err(|_| FilterMaskError::Reference)?;
        let Ok(slot) = index(code);
        let pick = |plane: &[[u8; 2]; 8]| {
          let [global, other] = plane.get(slot).ok_or(FilterMaskError::Reference)?;
          Ok(match motion {
            FilterMaskMotion::Global => *global,
            FilterMaskMotion::Other => *other,
          })
        };
        let [vertical, horizontal, u, v] = levels;
        let vertical_level = pick(vertical)?;
        let horizontal_level = pick(horizontal)?;
        let u_level = pick(u)?;
        let v_level = pick(v)?;
        Ok([vertical_level, horizontal_level, u_level, v_level])
      }
    }
  }
}

struct FilterMaskGeometry {
  origin: Point<u32>,
  frame: Size<u32>,
  block: Size<u32>,
}

impl FilterMaskGeometry {
  fn new(block: &FilterMaskBlock<'_>) -> Result<Self, FilterMaskError> {
    let x = u32::try_from(block.origin.x).map_err(|_| FilterMaskError::Region)?;
    let y = u32::try_from(block.origin.y).map_err(|_| FilterMaskError::Region)?;
    let frame_width = u32::try_from(block.frame.width).map_err(|_| FilterMaskError::Region)?;
    let frame_height = u32::try_from(block.frame.height).map_err(|_| FilterMaskError::Region)?;
    let Ok(&[width, height, ..]) = block.size.dimensions();

    Ok(Self {
      origin: Point { x, y },
      frame: Size {
        width: frame_width,
        height: frame_height,
      },
      block: Size {
        width: u32::from(width),
        height: u32::from(height),
      },
    })
  }

  fn plane(&self, subsampling: Size<u32>) -> Result<Option<Rectangle<u32>>, Never> {
    let Size {
      width: sx,
      height: sy,
    } = subsampling;
    let origin = Point {
      x: self.origin.x.wrapping_shr(sx),
      y: self.origin.y.wrapping_shr(sy),
    };
    let rx = 1u32.wrapping_shl(sx).wrapping_sub(1);
    let ry = 1u32.wrapping_shl(sy).wrapping_sub(1);
    let frame_width = self.frame.width.saturating_add(rx).wrapping_shr(sx);
    let frame_height = self.frame.height.saturating_add(ry).wrapping_shr(sy);
    let block_width = self.block.width.saturating_add(rx).wrapping_shr(sx);
    let block_height = self.block.height.saturating_add(ry).wrapping_shr(sy);
    let width = frame_width.saturating_sub(origin.x).min(block_width);
    let height = frame_height.saturating_sub(origin.y).min(block_height);

    Ok((width != 0 && height != 0).then_some(Rectangle {
      origin,
      size: Size { width, height },
    }))
  }
}

struct LevelCache<'a> {
  cells: &'a [AtomicU8],
  cell_stride: u64,
}

enum CachedLevels {
  Luma { vertical: u8, horizontal: u8 },
  Chroma { u: u8, v: u8 },
}

impl LevelCache<'_> {
  fn fill(&self, rect: Rectangle<u32>, levels: CachedLevels) -> Result<(), FilterMaskError> {
    let Rectangle { origin, size } = rect;
    let row_end = u64::from(origin.x).saturating_add(u64::from(size.width));
    let last_row = u64::from(origin.y).saturating_add(u64::from(size.height.saturating_sub(1)));
    let end = self.row_start(last_row)?;
    let full_end = end
      .checked_add(row_end.wrapping_mul(4))
      .ok_or(FilterMaskError::Cache)?;
    let full_limit = checked_index(full_end).map_err(|_| FilterMaskError::Cache)?;

    match ::core::primitive::bool::then_some(
      row_end <= self.cell_stride && full_limit <= self.cells.len(),
      (),
    ) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(FilterMaskError::Cache),
    }

    let start_column = u64::from(origin.x).wrapping_mul(4);
    let end_column = row_end.wrapping_mul(4);
    let y_end = origin.y.saturating_add(size.height);

    for y in origin.y..y_end {
      let row = self.row_start(u64::from(y))?;
      let start =
        checked_index(row.wrapping_add(start_column)).map_err(|_| FilterMaskError::Cache)?;
      let limit =
        checked_index(row.wrapping_add(end_column)).map_err(|_| FilterMaskError::Cache)?;
      let cells = self.cells.get(start..limit).ok_or(FilterMaskError::Cache)?;
      let (pixels, _) = cells.as_chunks::<4>();

      for [vertical, horizontal, u, v] in pixels {
        match levels {
          CachedLevels::Luma {
            vertical: a,
            horizontal: b,
          } => {
            vertical.store(a, Relaxed);
            horizontal.store(b, Relaxed);
          }
          CachedLevels::Chroma { u: a, v: b } => {
            u.store(a, Relaxed);
            v.store(b, Relaxed);
          }
        }
      }
    }

    Ok(())
  }

  fn row_start(&self, row: u64) -> Result<u64, FilterMaskError> {
    row
      .checked_mul(self.cell_stride)
      .and_then(|start| start.checked_mul(4))
      .ok_or(FilterMaskError::Cache)
  }
}

fn local_rectangle(
  rect: Rectangle<u32>,
  subsampling: Size<u32>,
) -> Result<Rectangle<u32>, Never> {
  let x = rect.origin.x & 31u32.wrapping_shr(subsampling.width);
  let y = rect.origin.y & 31u32.wrapping_shr(subsampling.height);

  Ok(Rectangle {
    origin: Point { x, y },
    size: rect.size,
  })
}

fn populate_luma_mask(
  mask: &[DirectionMask<3>; 2],
  neighbors: EdgeNeighbors<'_>,
  rect: Rectangle<u32>,
  transform: TxfmSize,
  prediction: &FilterMaskPrediction<'_>,
) -> Result<(), FilterMaskError> {
  match prediction {
    FilterMaskPrediction::Intra => mask_edges_intra(mask, rect, transform, neighbors),
    FilterMaskPrediction::Inter {
      scan, split_masks, ..
    } => mask_edges_inter(mask, rect, *scan, transform, split_masks, neighbors),
  }
}

#[inline(never)]
pub(crate) fn create_lf_mask(
  block: FilterMaskBlock<'_>,
  prediction: FilterMaskPrediction<'_>,
) -> Result<(), FilterMaskError> {
  let geometry = FilterMaskGeometry::new(&block)?;
  let [vertical, horizontal, u, v] = prediction.levels(block.levels)?;
  let cell_stride = u64::try_from(block.stride).map_err(|_| FilterMaskError::Cache)?;
  let cache = LevelCache {
    cells: block.level_cache,
    cell_stride,
  };
  let luma_sampling = Size {
    width: 0,
    height: 0,
  };
  let Ok(luma) = geometry.plane(luma_sampling);

  match luma {
    Some(rect) => {
      cache.fill(
        rect,
        CachedLevels::Luma {
          vertical,
          horizontal,
        },
      )?;
      let Ok(local) = local_rectangle(rect, luma_sampling);
      populate_luma_mask(
        &block.mask.filter_y,
        block.luma,
        local,
        block.ytx,
        &prediction,
      )?;
    }
    None => {}
  }

  let (above, left) = match block.chroma {
    Some(neighbors) => neighbors,
    None => return Ok(()),
  };
  let subsampling = Size {
    width: u32::from(block.layout != Rav1dPixelLayout::I444),
    height: u32::from(block.layout == Rav1dPixelLayout::I420),
  };
  let Ok(chroma) = geometry.plane(subsampling);
  let rect = match chroma {
    Some(rect) => rect,
    None => return Ok(()),
  };
  let scan = match prediction {
    FilterMaskPrediction::Intra => EdgeScan::AllTransforms,
    FilterMaskPrediction::Inter { scan, .. } => scan,
  };
  cache.fill(rect, CachedLevels::Chroma { u, v })?;
  let Ok(local) = local_rectangle(rect, subsampling);

  mask_edges_chroma(
    &block.mask.filter_uv,
    local,
    scan,
    block.uvtx,
    EdgeNeighbors { above, left },
    block.layout,
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn filter_mask_chroma_splits_words_at_the_subsampled_boundary() {
    let mask = Av1Filter::default();
    let rectangle = Rectangle {
      origin: Point { x: 7, y: 7 },
      size: Size {
        width: 2,
        height: 2,
      },
    };
    let mut above = [1, 1];
    let mut left = [1, 1];
    let result = mask_edges_chroma(
      &mask.filter_uv,
      rectangle,
      EdgeScan::AllTransforms,
      TxfmSize::S4x4,
      EdgeNeighbors {
        above: &mut above,
        left: &mut left,
      },
      Rav1dPixelLayout::I420,
    );

    assert!(matches!(result, Ok(())));
    assert_eq!(above, [0, 0]);
    assert_eq!(left, [0, 0]);

    for axis in &mask.filter_uv {
      for (position, [first, _]) in axis.iter().enumerate() {
        let [lower, upper] = first;
        let Ok(low) = lower.get();
        let Ok(high) = upper.get();
        let expected = match position {
          7 | 8 => (128, 1),
          _ => (0, 0),
        };
        assert_eq!((low, high), expected);
      }
    }
  }

  #[test]
  fn filter_mask_cache_rejects_a_truncated_region_before_storing_levels() {
    let cells: [AtomicU8; 7] = core::array::from_fn(|_| AtomicU8::new(19));
    let cache = LevelCache {
      cells: &cells,
      cell_stride: 1,
    };
    let rectangle = Rectangle {
      origin: Point { x: 0, y: 0 },
      size: Size {
        width: 1,
        height: 2,
      },
    };
    let result = cache.fill(
      rectangle,
      CachedLevels::Luma {
        vertical: 3,
        horizontal: 7,
      },
    );

    assert!(matches!(result, Err(FilterMaskError::Cache)));
    assert!(cells.iter().all(|cell| cell.load(Relaxed) == 19));
  }
}

pub fn rav1d_calc_eih(lut: &mut Av1FilterLUT, sharpness: u8) -> Result<(), Never> {
  let shift = u32::from(sharpness.wrapping_add(3).wrapping_shr(2));
  let max_limit = 9u8.wrapping_sub(sharpness);

  for ((interior, edge), level) in lut.i.iter_mut().zip(&mut lut.e).zip(0u8..64) {
    let limit = match sharpness {
      0 => level,
      _ => level.wrapping_shr(shift).min(max_limit),
    }
    .max(1);
    *interior = limit;
    *edge = 2u8.wrapping_mul(level.wrapping_add(2)).wrapping_add(limit);
  }

  let sharp = u64::from(sharpness);
  let limit = match sharpness {
    0 => 0xff,
    _ => 9u64.wrapping_sub(sharp),
  };
  lut.sharp = [sharp.wrapping_add(3).wrapping_shr(2), limit];

  Ok(())
}

struct LoopFilterAdjustment {
  block: i8,
  segment: i8,
}

#[derive(Clone, Copy)]
enum BaseFilterLevel {
  Luma(u8),
  Chroma(u8),
}

fn calc_lf_value(
  value: BaseFilterLevel,
  adjustment: LoopFilterAdjustment,
  mode_ref: Option<&Rav1dLoopfilterModeRefDeltas>,
) -> Result<[[u8; 2]; 8], Never> {
  let base_level = match value {
    BaseFilterLevel::Chroma(0) => return Ok([[0; 2]; 8]),
    BaseFilterLevel::Luma(base) | BaseFilterLevel::Chroma(base) => i32::from(base),
  };
  let with_block = iclip(base_level.wrapping_add(i32::from(adjustment.block)), 0, 63);
  let base = iclip(
    with_block.wrapping_add(i32::from(adjustment.segment)),
    0,
    63,
  );
  let mode_ref_deltas = match mode_ref {
    Some(deltas) => deltas,
    None => {
      let Ok(level) = wrapped::<_, u8>(base);
      return Ok([[level; 2]; 8]);
    }
  };
  let shift = u32::from(base >= 32);
  let scale = 1i32.wrapping_shl(shift);
  let apply = |delta: i8| {
    clip(
      base.wrapping_add(i32::from(delta).wrapping_mul(scale)),
      0,
      63,
    )
  };
  let mut values = [[0; 2]; 8];
  let [intra, inter @ ..] = &mut values;
  let [intra_ref, inter_refs @ ..] = mode_ref_deltas.ref_delta;

  *intra = [apply(intra_ref); 2];
  for (output, reference) in inter.iter_mut().zip(inter_refs) {
    *output = mode_ref_deltas
      .mode_delta
      .map(|motion| apply(motion.wrapping_add(reference)));
  }

  Ok(values)
}

pub(crate) fn rav1d_calc_lf_values(
  levels: &mut [Align16<[[[u8; 2]; 8]; 4]>; {
         match console_core_number_conversion::const_u8_index(SegmentId::COUNT) {
           Ok(extent) => extent,
         }
       }],
  header: &Rav1dFrameHeader,
  block_deltas: &[i8; 4],
) -> Result<(), Never> {
  let enabled = header.segmentation.enabled != 0;
  let [vertical, horizontal] = header.loopfilter.level_y;
  let luma_disabled = vertical == 0 && horizontal == 0;
  let base = [
    BaseFilterLevel::Luma(vertical),
    BaseFilterLevel::Luma(horizontal),
    BaseFilterLevel::Chroma(header.loopfilter.level_u),
    BaseFilterLevel::Chroma(header.loopfilter.level_v),
  ];
  let mode_ref =
    (header.loopfilter.mode_ref_delta_enabled != 0).then_some(&header.loopfilter.mode_ref_deltas);
  let &[common, ..] = block_deltas;
  let deltas = match header.delta.lf.multi {
    0 => [common; 4],
    _ => *block_deltas,
  };
  let segments = levels
    .iter_mut()
    .zip(&header.segmentation.seg_data.d)
    .zip(0u8..);

  for ((output, segment), ordinal) in segments {
    match (enabled, ordinal) {
      (false, 1..) => break,
      (true, _) | (false, 0) => {}
    }

    match ::core::primitive::bool::then_some(luma_disabled, ()) {
      ::core::option::Option::Some(()) => {
        **output = Default::default();
        continue;
      }
      ::core::option::Option::None => {}
    }

    let segment_deltas = match ::core::primitive::bool::then_some(enabled, ()) {
      ::core::option::Option::Some(()) => [
        segment.delta_lf_y_v,
        segment.delta_lf_y_h,
        segment.delta_lf_u,
        segment.delta_lf_v,
      ],
      ::core::option::Option::None => [0; 4],
    };

    for ((plane, base_level), (block, segment_delta)) in output
      .iter_mut()
      .zip(base)
      .zip(deltas.into_iter().zip(segment_deltas))
    {
      let Ok(value) = calc_lf_value(
        base_level,
        LoopFilterAdjustment {
          block,
          segment: segment_delta,
        },
        mode_ref,
      );
      *plane = value;
    }
  }

  Ok(())
}
