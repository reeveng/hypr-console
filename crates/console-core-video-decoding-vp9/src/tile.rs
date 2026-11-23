//! Tile decoding: partition trees, mode info (6.4), and block reconstruction. One `TileDecoder`
//! decodes all tiles of one tile column (tile columns are independent of each other; tile rows
//! of a column are decoded in order).

use std::ops::Range;

use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};
use console_core_size::Size;

use crate::boolcoder::BoolDecoder;
use crate::frame::{Frame, FrameKind, MiGrid, MiInfo, MiPlace, Mv, Subsampling, filled};
use crate::header::{FrameHeader, MapUpdate, Segmentation, SegmentationStatus};
use crate::places::{counted, length, read, row, row_mut, span, span_mut, write};
use crate::probs::{Counts, FrameContext};
use crate::tables::*;

mod motion;
mod recon;

pub struct RefUse<'a> {
  pub frame: &'a Frame,
  pub x_scale: i32,
  pub y_scale: i32,
  pub x_step: i32,
  pub y_step: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbabilityCounts {
  Collect,
  Ignore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransformSelection {
  Read,
  Infer,
}

impl From<bool> for TransformSelection {
  fn from(read: bool) -> Self {
    match read.then_some(()) {
      Some(()) => Self::Read,
      None => Self::Infer,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReferenceScale {
  Scaled,
  Unscaled,
}

impl From<bool> for ReferenceScale {
  fn from(scaled: bool) -> Self {
    match scaled.then_some(()) {
      Some(()) => Self::Scaled,
      None => Self::Unscaled,
    }
  }
}

pub struct FrameShared<'a> {
  pub h: &'a FrameHeader,
  pub fc: &'a FrameContext,
  pub seg: &'a Segmentation,

  pub prev_seg_ids: &'a [u8],

  pub prev_mi: Option<&'a MiGrid>,

  pub refs: [Option<RefUse<'a>>; 3],

  pub seg_q: [[[i32; 2]; 2]; 8],

  pub counting: ProbabilityCounts,
  pub pools: &'a crate::frame::Pools,
}

pub struct TileBand {
  pub columns: Range<u64>,

  pub mi: Vec<MiInfo>,
  pub seg_ids: Vec<u8>,
  pub planes: [Vec<u16>; 3],
  pub strides: [u64; 3],

  pub x_off: [u64; 3],
  pub counts: Box<Counts>,

  pub compound_blocks: u64,
  pub scaled_blocks: u64,
  pub intra_blocks: u64,
  pub inter_blocks: u64,
}

#[derive(Clone, Copy, Default)]
struct BlockNeighbors {
  above: Option<MiInfo>,
  left: Option<MiInfo>,
}

impl BlockNeighbors {
  fn key_modes(self, at: MiPlace) -> Result<ModeNeighbors, Never> {
    let empty = [DC_PRED; 4];
    let above = self.above.map_or(empty, |mi| mi.sub_modes);
    let left = self.left.map_or(empty, |mi| mi.sub_modes);
    let Ok(above_2) = read(&above, 2u64.wrapping_add(at.col));
    let Ok(left_2) = read(&left, at.row.wrapping_mul(2).wrapping_add(1));
    Ok(ModeNeighbors {
      above: above_2,
      left: left_2,
    })
  }
}

#[must_use]
#[derive(Clone, Copy)]
enum ReferenceNeighbor {
  Absent,
  Intra,
  Single(i8),
  Compound([i8; 2]),
}

impl From<Option<MiInfo>> for ReferenceNeighbor {
  fn from(neighbor: Option<MiInfo>) -> Self {
    match neighbor.map(|mi| mi.ref_frame) {
      None => Self::Absent,
      Some([..=INTRA_FRAME, _]) => Self::Intra,
      Some([reference, ..=NONE]) => Self::Single(reference),
      Some(pair) => Self::Compound(pair),
    }
  }
}

#[derive(Clone, Copy)]
struct ReferenceNeighbors {
  above: ReferenceNeighbor,
  left: ReferenceNeighbor,
}

impl From<BlockNeighbors> for ReferenceNeighbors {
  fn from(neighbors: BlockNeighbors) -> Self {
    Self {
      above: ReferenceNeighbor::from(neighbors.above),
      left: ReferenceNeighbor::from(neighbors.left),
    }
  }
}

enum ReferenceCheck {
  Last,
  Golden,
}

#[derive(Clone, Copy)]
enum CompoundOrder {
  FixedFirst,
  FixedSecond,
}

impl From<u8> for CompoundOrder {
  fn from(sign: u8) -> Self {
    match sign {
      0 => Self::FixedFirst,
      _ => Self::FixedSecond,
    }
  }
}

impl CompoundOrder {
  fn variable(self, [first, second]: [i8; 2]) -> Result<i8, Never> {
    Ok(match self {
      Self::FixedFirst => second,
      Self::FixedSecond => first,
    })
  }
}

#[derive(Clone, Copy)]
struct CompoundReferences {
  fixed: i8,
  options: [i8; 2],
}

impl ReferenceNeighbors {
  fn compound_mode_context(self, fixed: i8) -> Result<u64, Never> {
    use ReferenceNeighbor::{Absent, Compound, Intra, Single};

    Ok(match (self.above, self.left) {
      (Compound(_), Compound(_)) => 4,
      (Compound(_), Intra | Absent) | (Intra | Absent, Compound(_)) => 3,
      (Compound(_), Single(reference)) | (Single(reference), Compound(_)) => {
        2u64.wrapping_add(u64::from(reference == fixed))
      }
      (Single(above), Single(left)) => u64::from((above == fixed) ^ (left == fixed)),
      (Single(reference), Intra | Absent) | (Intra | Absent, Single(reference)) => {
        u64::from(reference == fixed)
      }
      (Absent, Absent) => 1,
      _ => 0,
    })
  }

  fn compound_context(
    self,
    references: CompoundReferences,
    order: CompoundOrder,
  ) -> Result<u64, Never> {
    use ReferenceNeighbor::{Absent, Compound, Intra, Single};

    let [low, high] = references.options;
    let context = match (self.above, self.left) {
      (Single(a), Single(b)) => {
        match (
          a == high && b == high,
          a == b,
          (a == references.fixed && b == low) || (b == references.fixed && a == low),
        ) {
          (true, _, _) => 0,
          (false, _, true) => 4,
          (false, true, false) => 3,
          _ => 1,
        }
      }
      (Compound(a), Compound(b)) => {
        let Ok(a_2) = order.variable(a);
        let Ok(b_2) = order.variable(b);

        match (a_2 == b_2, a_2 == high) {
          (true, true) => 0,
          (true, false) => 4,
          (false, _) => 2,
        }
      }
      (Compound(pair), Single(single)) | (Single(single), Compound(pair)) => {
        let Ok(compound) = order.variable(pair);

        match (compound == high, single == high) {
          (true, true) => 0,
          (true, false) => 1,
          (false, true) => 2,
          (false, false) => 4,
        }
      }
      (Single(reference), Intra) | (Intra, Single(reference)) => {
        1u64.wrapping_add(2u64.wrapping_mul(u64::from(reference != high)))
      }
      (Compound(pair), Intra) | (Intra, Compound(pair)) => {
        let Ok(reference) = order.variable(pair);
        1u64.wrapping_add(2u64.wrapping_mul(u64::from(reference != high)))
      }
      (Single(reference), Absent) | (Absent, Single(reference)) => {
        3u64.wrapping_mul(u64::from(reference != high))
      }
      (Compound(pair), Absent) | (Absent, Compound(pair)) => {
        let Ok(reference) = order.variable(pair);
        4u64.wrapping_mul(u64::from(reference != high))
      }
      _ => 2,
    };

    Ok(context)
  }

  fn last_reference_context(self) -> Result<u64, Never> {
    use ReferenceNeighbor::{Absent, Compound, Intra, Single};

    const LAST: i8 = LAST_FRAME;
    Ok(match (self.above, self.left) {
      (Single(a), Single(b)) => {
        2u64.wrapping_mul(u64::from(a == LAST).wrapping_add(u64::from(b == LAST)))
      }
      (Compound(a), Compound(b)) => {
        1u64.wrapping_add(u64::from(a.contains(&LAST) || b.contains(&LAST)))
      }
      (Single(single), Compound(pair)) | (Compound(pair), Single(single)) => 3u64
        .wrapping_mul(u64::from(single == LAST))
        .wrapping_add(u64::from(pair.contains(&LAST))),
      (Single(single), Intra | Absent) | (Intra | Absent, Single(single)) => {
        4u64.wrapping_mul(u64::from(single == LAST))
      }
      (Compound(pair), Intra | Absent) | (Intra | Absent, Compound(pair)) => {
        1u64.wrapping_add(u64::from(pair.contains(&LAST)))
      }
      _ => 2,
    })
  }

  fn golden_reference_context(self) -> Result<u64, Never> {
    use ReferenceNeighbor::{Absent, Compound, Intra, Single};

    const GOLDEN: i8 = GOLDEN_FRAME;
    const LAST: i8 = LAST_FRAME;
    let context = match (self.above, self.left) {
      (Single(LAST), Single(LAST) | Intra) | (Intra, Single(LAST)) => 3,
      (Single(LAST), Single(other)) | (Single(other), Single(LAST)) => {
        4u64.wrapping_mul(u64::from(other == GOLDEN))
      }
      (Single(a), Single(b)) => {
        2u64.wrapping_mul(u64::from(a == GOLDEN).wrapping_add(u64::from(b == GOLDEN)))
      }
      (Compound(a), Compound(b)) => match (a == b).then_some(()) {
        Some(()) => 3u64.wrapping_mul(u64::from(a.contains(&GOLDEN))),
        None => 2,
      },
      (Single(single), Compound(pair)) | (Compound(pair), Single(single)) => {
        let golden = u64::from(pair.contains(&GOLDEN));

        match single {
          GOLDEN => 3u64.wrapping_add(golden),
          ALTREF_FRAME => golden,
          _ => 1u64.wrapping_add(2u64.wrapping_mul(golden)),
        }
      }
      (Single(LAST), Absent) | (Absent, Single(LAST)) => 2,
      (Single(single), Intra | Absent) | (Intra | Absent, Single(single)) => {
        4u64.wrapping_mul(u64::from(single == GOLDEN))
      }
      (Compound(pair), Intra) | (Intra, Compound(pair)) => {
        1u64.wrapping_add(2u64.wrapping_mul(u64::from(pair.contains(&GOLDEN))))
      }
      (Compound(pair), Absent) | (Absent, Compound(pair)) => {
        3u64.wrapping_mul(u64::from(pair.contains(&GOLDEN)))
      }
      _ => 2,
    };

    Ok(context)
  }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum BlockPrediction {
  #[default]
  Intra,
  Inter,
}

impl From<bool> for BlockPrediction {
  fn from(inter: bool) -> Self {
    match inter.then_some(()) {
      Some(()) => Self::Inter,
      None => Self::Intra,
    }
  }
}

struct ModeNeighbors {
  above: u8,
  left: u8,
}

fn key_mode(bd: &mut BoolDecoder, neighbors: ModeNeighbors) -> Result<u8, Never> {
  let start = u64::from(neighbors.above)
    .wrapping_mul(10)
    .wrapping_add(u64::from(neighbors.left))
    .wrapping_mul(9);
  let Ok(probabilities) = span(&KF_Y_MODE_PROBS, start..start.wrapping_add(9));
  bd.read_tree(&INTRA_MODE_TREE, probabilities)
}

#[derive(Clone, Copy)]
struct SubblockLayout(Size<u64>);

impl SubblockLayout {
  fn for_size(size: u8) -> Result<Self, Never> {
    let Ok(width) = read(&NUM_4X4_WIDE, u64::from(size));
    let Ok(height) = read(&NUM_4X4_HIGH, u64::from(size));
    Ok(Self(Size {
      width: u64::from(width.max(1)),
      height: u64::from(height.max(1)),
    }))
  }

  fn origins(self) -> Result<impl Iterator<Item = MiPlace>, Never> {
    let Ok(rows) = index(self.0.height);
    let Ok(cols) = index(self.0.width);
    Ok((0u64..2).step_by(rows.max(1)).flat_map(move |row| {
      (0u64..2)
        .step_by(cols.max(1))
        .map(move |col| MiPlace { row, col })
    }))
  }

  fn cells(self, at: MiPlace) -> Result<impl Iterator<Item = u64>, Never> {
    Ok(
      (at.row..at.row.wrapping_add(self.0.height)).flat_map(move |row| {
        (at.col..at.col.wrapping_add(self.0.width))
          .map(move |col| row.wrapping_mul(2).wrapping_add(col))
      }),
    )
  }
}

struct ContextChoice {
  context: u64,
  value: u64,
}

fn count_choice<const N: usize>(
  mode: ProbabilityCounts,
  counts: &mut [[u32; N]],
  choice: ContextChoice,
) -> Result<(), Never> {
  match mode {
    ProbabilityCounts::Collect => {
      let Ok(bucket) = row_mut(counts, choice.context);
      counted(bucket, choice.value)
    }
    ProbabilityCounts::Ignore => Ok(()),
  }
}

#[derive(Clone, Copy, Default)]
struct Blk {
  r: u64,
  c: u64,
  size: u8,
  neighbors: BlockNeighbors,
  seg_id: u8,
  skip: bool,
  tx_size: u8,
  prediction: BlockPrediction,
  y_mode: u8,
  sub_modes: [u8; 4],
  uv_mode: u8,
  ref_frame: [i8; 2],
  interp_filter: u8,
  block_mvs: [[Mv; 4]; 2],
  eob_total: u32,
}

pub struct TileDecoder<'a> {
  s: &'a FrameShared<'a>,
  h: &'a FrameHeader,
  fc: &'a FrameContext,
  pub strip: TileBand,
  mi_cols: u64,
  mi_rows: u64,
  subsampling: Subsampling,
  bit_depth: u8,

  above_nonzero: [Vec<u8>; 3],
  left_nonzero: [Vec<u8>; 3],
  above_partition: Vec<u8>,
  left_partition: Vec<u8>,
  above_seg_pred: Vec<u8>,
  left_seg_pred: Vec<u8>,

  coefs: Vec<i32>,
  token_cache: Vec<u8>,
  pred: Vec<u16>,
  pred2: Vec<u16>,
  mc_tmp: Vec<u16>,
  mc_win: Vec<u16>,

  pub error: Option<crate::error::Error>,
}

#[derive(Clone, Copy)]
struct PartitionNode {
  origin: MiPlace,
  size: u8,
}

struct PartitionShape {
  size: u8,
  width: u8,
}

struct LeafPartition {
  start: MiPlace,
  next: MiPlace,
  subsize: u8,
  kind: PartitionKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PartitionKind {
  Whole,
  Horizontal,
  Vertical,
  Split,
}

impl From<u8> for PartitionKind {
  fn from(syntax: u8) -> Self {
    match syntax {
      0 => Self::Whole,
      1 => Self::Horizontal,
      2 => Self::Vertical,
      _ => Self::Split,
    }
  }
}

#[derive(Clone, Copy)]
enum PartitionCoverage {
  Both,
  Rows,
  Columns,
  Neither,
}

impl From<(bool, bool)> for PartitionCoverage {
  fn from((rows, columns): (bool, bool)) -> Self {
    match (rows, columns) {
      (true, true) => Self::Both,
      (true, false) => Self::Rows,
      (false, true) => Self::Columns,
      (false, false) => Self::Neither,
    }
  }
}

impl PartitionCoverage {
  fn read(self, decoder: &mut BoolDecoder, probabilities: &[u8]) -> Result<u8, Never> {
    let (index, unsplit) = match self {
      Self::Both => return decoder.read_tree(&PARTITION_TREE, probabilities),
      Self::Rows => (2, 2),
      Self::Columns => (1, 1),
      Self::Neither => return Ok(3),
    };
    let Ok(probability) = read(probabilities, index);
    let Ok(split) = decoder.read_bool(probability);
    Ok(match split {
      0 => unsplit,
      _ => 3,
    })
  }
}

const SUPERBLOCK_MAX_NODES: u32 = 85;

impl TileBand {
  fn new(
    shared: &FrameShared,
    columns: Range<u64>,
    subsampling: Subsampling,
  ) -> Result<Self, Never> {
    let strip_columns = columns.end.wrapping_sub(columns.start);
    let width = strip_columns.div_ceil(8).wrapping_mul(64);
    let height = u64::from(shared.h.sb64_rows).wrapping_mul(64);
    let x = columns.start.wrapping_mul(8);
    let chroma_width = width.wrapping_shr(subsampling.x);
    let chroma_height = height.wrapping_shr(subsampling.y);
    let chroma_x = x.wrapping_shr(subsampling.x);
    let area_y = width.wrapping_mul(height);
    let area_uv = chroma_width.wrapping_mul(chroma_height);
    let area_mi = strip_columns.wrapping_mul(u64::from(shared.h.mi_rows));
    let Ok(seg_ids) = filled::<_, 4194304>(area_mi, 0);
    let Ok(y) = shared.pools.samples.take(area_y, 0);
    let Ok(u) = shared.pools.samples.take(area_uv, 0);
    let Ok(v) = shared.pools.samples.take(area_uv, 0);
    let Ok(mi) = shared.pools.mi.take(area_mi, MiInfo::BLANK);

    Ok(Self {
      columns,
      mi,
      seg_ids,
      planes: [y, u, v],
      strides: [width, chroma_width, chroma_width],
      x_off: [x, chroma_x, chroma_x],
      counts: Box::default(),
      compound_blocks: 0,
      scaled_blocks: 0,
      intra_blocks: 0,
      inter_blocks: 0,
    })
  }

  fn mi_offset(&self, at: MiPlace) -> Result<u64, Never> {
    let width = self.columns.end.wrapping_sub(self.columns.start);
    Ok(
      at.row
        .wrapping_mul(width)
        .wrapping_add(at.col)
        .wrapping_sub(self.columns.start),
    )
  }
}

impl<'a> TileDecoder<'a> {
  pub fn new(s: &'a FrameShared<'a>, columns: Range<u64>) -> Result<Self, Never> {
    let h = s.h;
    let subsampling = Subsampling {
      x: u32::from(h.color.subsampling.x != 0),
      y: u32::from(h.color.subsampling.y != 0),
    };
    let Ok(strip) = TileBand::new(s, columns, subsampling);
    let row_sb = u64::from(h.sb64_rows);
    let col_sb = u64::from(h.sb64_cols);
    let Ok(above_units) = filled::<_, 4112>(col_sb.wrapping_add(1).wrapping_mul(16), 0);
    let Ok(left_units) = filled::<_, 4112>(row_sb.wrapping_add(1).wrapping_mul(16), 0);
    let Ok(above_blocks) = filled::<_, 2056>(col_sb.wrapping_add(1).wrapping_mul(8), 0);
    let Ok(left_blocks) = filled::<_, 2056>(row_sb.wrapping_add(1).wrapping_mul(8), 0);
    let scratch = match ReferenceScale::from(
      s.refs
        .iter()
        .flatten()
        .any(|r| r.x_scale != 16384 || r.y_scale != 16384),
    ) {
      ReferenceScale::Scaled => vec![0; 21504],
      ReferenceScale::Unscaled => vec![0; 4544],
    };

    Ok(TileDecoder {
      s,
      h,
      fc: s.fc,
      strip,
      mi_cols: u64::from(h.mi_cols),
      mi_rows: u64::from(h.mi_rows),
      subsampling,
      bit_depth: h.color.bit_depth,
      above_nonzero: [above_units.clone(), above_units.clone(), above_units],
      left_nonzero: [left_units.clone(), left_units.clone(), left_units],
      above_partition: above_blocks.clone(),
      left_partition: left_blocks.clone(),
      above_seg_pred: above_blocks,
      left_seg_pred: left_blocks,
      coefs: vec![0; 1024],
      token_cache: vec![0; 1024],
      pred: vec![0; 4096],
      pred2: vec![0; 4096],
      mc_tmp: scratch,
      mc_win: vec![0; 5041],
      error: None,
    })
  }

  #[inline]
  fn mi_at(&self, at: MiPlace) -> Result<&MiInfo, Never> {
    let Ok(offset) = self.strip.mi_offset(at);
    let Ok(native) = index(offset);
    Ok(match self.strip.mi.get(native) {
      Some(found) => found,
      None => &MiInfo::BLANK,
    })
  }

  pub fn decode_tile(&mut self, data: &[u8], rows: Range<u64>) -> Result<(), Never> {
    let Ok(mut bd) = BoolDecoder::new(data);
    let mut partitions = Vec::with_capacity(10);

    for row in rows.step_by(8) {
      let Ok(()) = self.clear_left_context(row);

      for col in self.strip.columns.clone().step_by(8) {
        let Ok(()) = self.decode_superblock(&mut bd, MiPlace { row, col }, &mut partitions);
      }
    }

    Ok(())
  }

  fn clear_left_context(&mut self, row: u64) -> Result<(), Never> {
    let [luma, blue, red] = &mut self.left_nonzero;

    for (shift, context) in [
      (0, luma),
      (self.subsampling.y, blue),
      (self.subsampling.y, red),
    ] {
      let start = row.wrapping_mul(2).wrapping_shr(shift);
      let Ok(length) = length(context);
      let end = start.wrapping_add(16u64.wrapping_shr(shift)).min(length);
      let Ok(span) = span_mut(context, start..end);
      span.fill(0);
    }

    let end = row.wrapping_add(8);
    let Ok(partition) = span_mut(&mut self.left_partition, row..end);
    let Ok(segment) = span_mut(&mut self.left_seg_pred, row..end);
    partition.fill(0);
    segment.fill(0);
    Ok(())
  }

  fn decode_superblock(
    &mut self,
    bd: &mut BoolDecoder,
    origin: MiPlace,
    pending: &mut Vec<PartitionNode>,
  ) -> Result<(), Never> {
    pending.clear();
    pending.push(PartitionNode {
      origin,
      size: BLOCK_64X64,
    });

    for _ in 0..SUPERBLOCK_MAX_NODES {
      let node = {
        let optional_node = pending.pop();

        match optional_node {
          Some(matched_node) => matched_node,
          ::core::option::Option::None => break,
        }
      };

      let Ok(children) = self.decode_partition(bd, node);

      match children {
        Some(split) => pending.extend(split.into_iter().rev()),
        None => {}
      }
    }

    Ok(())
  }

  fn decode_partition(
    &mut self,
    bd: &mut BoolDecoder,
    node: PartitionNode,
  ) -> Result<Option<[PartitionNode; 4]>, Never> {
    let PartitionNode { origin: at, size } = node;

    match (at.row >= self.mi_rows || at.col >= self.mi_cols).then_some(()) {
      Some(()) => return Ok(None),
      None => {}
    }

    let Ok(width) = read(&NUM_8X8_WIDE, u64::from(size));
    let shape = PartitionShape { size, width };
    let half = u64::from(width.wrapping_shr(1));
    let next = MiPlace {
      row: at.row.wrapping_add(half),
      col: at.col.wrapping_add(half),
    };
    let coverage = PartitionCoverage::from((next.row < self.mi_rows, next.col < self.mi_cols));
    let Ok(context) = self.partition_context(at, &shape);
    let Ok(probs) = match self.h.kind {
      FrameKind::Key | FrameKind::IntraOnly => {
        let start = context.wrapping_mul(3);
        span(&KF_PARTITION_PROBS, start..start.wrapping_add(3))
      }
      FrameKind::Inter => row(&self.fc.partition, context),
    };
    let Ok(partition) = coverage.read(bd, probs);
    let choice = ContextChoice {
      context,
      value: u64::from(partition),
    };
    let Ok(()) = count_choice(self.s.counting, &mut self.strip.counts.partition, choice);
    let Ok(sizes) = row(&SUBSIZE_LOOKUP, u64::from(partition));
    let Ok(subsize) = read(sizes, u64::from(size));
    let kind = PartitionKind::from(partition);

    match (kind, subsize) {
      (PartitionKind::Split, BLOCK_8X8..) => {
        let upper_right = MiPlace {
          row: at.row,
          col: next.col,
        };
        let lower_left = MiPlace {
          row: next.row,
          col: at.col,
        };
        Ok(Some([at, upper_right, lower_left, next].map(|origin| {
          PartitionNode {
            origin,
            size: subsize,
          }
        })))
      }
      _ => {
        let Ok(()) = self.decode_partition_blocks(
          bd,
          LeafPartition {
            start: at,
            next,
            subsize,
            kind,
          },
          coverage,
        );
        let Ok(()) = self.record_partition(
          at,
          PartitionShape {
            size: subsize,
            width,
          },
        );
        Ok(None)
      }
    }
  }

  fn partition_context(&self, at: MiPlace, shape: &PartitionShape) -> Result<u64, Never> {
    let Ok(log2) = read(&MI_WIDTH_LOG2, u64::from(shape.size));
    let shift = u32::from(3u8.wrapping_sub(log2));
    let mut above = 0;
    let mut left = 0;

    for offset in 0..u64::from(shape.width) {
      let Ok(above_bits) = read(&self.above_partition, at.col.wrapping_add(offset));
      let Ok(left_bits) = read(&self.left_partition, at.row.wrapping_add(offset));
      above |= above_bits;
      left |= left_bits;
    }

    let column = u64::from(above.wrapping_shr(shift) & 1);
    let row = u64::from(left.wrapping_shr(shift) & 1);
    Ok(
      u64::from(log2)
        .wrapping_mul(4)
        .wrapping_add(row.wrapping_mul(2))
        .wrapping_add(column),
    )
  }

  fn decode_partition_blocks(
    &mut self,
    bd: &mut BoolDecoder,
    partition: LeafPartition,
    coverage: PartitionCoverage,
  ) -> Result<(), Never> {
    let LeafPartition {
      start,
      next,
      subsize,
      kind,
    } = partition;
    let Ok(()) = self.decode_block(bd, start, subsize);

    match (subsize, kind, coverage) {
      (
        BLOCK_8X8..,
        PartitionKind::Horizontal,
        PartitionCoverage::Both | PartitionCoverage::Rows,
      ) => self.decode_block(
        bd,
        MiPlace {
          row: next.row,
          col: start.col,
        },
        subsize,
      ),
      (
        BLOCK_8X8..,
        PartitionKind::Vertical,
        PartitionCoverage::Both | PartitionCoverage::Columns,
      ) => self.decode_block(
        bd,
        MiPlace {
          row: start.row,
          col: next.col,
        },
        subsize,
      ),
      _ => Ok(()),
    }
  }

  fn record_partition(&mut self, at: MiPlace, shape: PartitionShape) -> Result<(), Never> {
    let Ok(block_width) = read(&B_WIDTH_LOG2, u64::from(shape.size));
    let Ok(block_height) = read(&B_HEIGHT_LOG2, u64::from(shape.size));
    let width = u64::from(shape.width);
    let Ok(above) = span_mut(
      &mut self.above_partition,
      at.col..at.col.wrapping_add(width),
    );
    let Ok(left) = span_mut(&mut self.left_partition, at.row..at.row.wrapping_add(width));
    above.fill(15u8.wrapping_shr(u32::from(block_width)));
    left.fill(15u8.wrapping_shr(u32::from(block_height)));
    Ok(())
  }

  fn decode_block(&mut self, bd: &mut BoolDecoder, at: MiPlace, size: u8) -> Result<(), Never> {
    let Ok(chroma_size) = subsampled_size(size, self.subsampling);
    let size_2 = match (size >= BLOCK_8X8 && chroma_size == BLOCK_INVALID).then_some(()) {
      Some(()) => BLOCK_8X8,
      None => size,
    };
    let Ok(neighbors) = self.block_neighbors(at);
    let mut block = Blk {
      r: at.row,
      c: at.col,
      size: size_2,
      neighbors,
      ..Blk::default()
    };

    match self.h.kind {
      FrameKind::Key | FrameKind::IntraOnly => {
        let Ok(()) = self.intra_frame_mode_info(bd, &mut block);
      }
      FrameKind::Inter => {
        let Ok(()) = self.inter_frame_mode_info(bd, &mut block);
      }
    }

    let counter = match (block.prediction == BlockPrediction::Inter).then_some(()) {
      Some(()) => &mut self.strip.inter_blocks,
      None => &mut self.strip.intra_blocks,
    };
    *counter = counter.wrapping_add(1);
    let Ok(()) = self.residual(bd, &mut block);
    block.skip |= block.prediction == BlockPrediction::Inter
      && block.size >= BLOCK_8X8
      && block.eob_total == 0;
    self.store_block(at, block)
  }

  fn block_neighbors(&self, at: MiPlace) -> Result<BlockNeighbors, Never> {
    let above = match at.row {
      0 => None,
      row => {
        let Ok(info) = self.mi_at(MiPlace {
          row: row.wrapping_sub(1),
          col: at.col,
        });
        Some(*info)
      }
    };
    let left = match (at.col > self.strip.columns.start).then_some(()) {
      Some(()) => {
        let Ok(info) = self.mi_at(MiPlace {
          row: at.row,
          col: at.col.wrapping_sub(1),
        });
        Some(*info)
      }
      None => None,
    };

    Ok(BlockNeighbors { above, left })
  }

  fn store_block(&mut self, at: MiPlace, block: Blk) -> Result<(), Never> {
    let info = MiInfo {
      sb_size: block.size,
      skip: block.skip,
      tx_size: block.tx_size,
      y_mode: block.y_mode,
      sub_modes: block.sub_modes,
      seg_id: block.seg_id,
      ref_frame: block.ref_frame,
      interp_filter: block.interp_filter,
      mv: block.block_mvs,
    };
    let Ok(height) = read(&NUM_8X8_HIGH, u64::from(block.size));
    let Ok(width) = read(&NUM_8X8_WIDE, u64::from(block.size));
    let end_row = at.row.wrapping_add(u64::from(height)).min(self.mi_rows);
    let end_col = at
      .col
      .wrapping_add(u64::from(width))
      .min(self.strip.columns.end);

    for row in at.row..end_row {
      for col in at.col..end_col {
        let Ok(offset) = self.strip.mi_offset(MiPlace { row, col });
        let Ok(()) = write(&mut self.strip.mi, offset, info);
        let Ok(()) = write(&mut self.strip.seg_ids, offset, block.seg_id);
      }
    }

    Ok(())
  }

  fn intra_frame_mode_info(
    &mut self,
    bd: &mut BoolDecoder,
    block: &mut Blk,
  ) -> Result<(), Never> {
    let segments = self.s.seg;
    block.seg_id = match (segments.status, segments.map) {
      (SegmentationStatus::Enabled, MapUpdate::Temporal | MapUpdate::Explicit) => {
        let Ok(id) = bd.read_tree(&SEGMENT_TREE, &segments.tree_probs);
        id
      }
      _ => 0,
    };
    let Ok(()) = self.read_skip(bd, block);
    let Ok(()) = self.read_tx_size(bd, block, TransformSelection::Read);
    block.ref_frame = [INTRA_FRAME, NONE];
    block.prediction = BlockPrediction::Intra;

    match block.size {
      ..=2 => {
        let Ok(()) = self.key_subblock_modes(bd, block);
      }
      _ => {
        let Ok(neighbors) = block.neighbors.key_modes(MiPlace { row: 0, col: 0 });
        let Ok(mode) = key_mode(bd, neighbors);
        block.y_mode = mode;
        block.sub_modes = [mode; 4];
      }
    }

    let start = u64::from(block.y_mode).wrapping_mul(9);
    let Ok(probabilities) = span(&KF_UV_MODE_PROBS, start..start.wrapping_add(9));
    let Ok(mode) = bd.read_tree(&INTRA_MODE_TREE, probabilities);
    block.uv_mode = mode;
    Ok(())
  }

  fn key_subblock_modes(&self, bd: &mut BoolDecoder, block: &mut Blk) -> Result<(), Never> {
    let Ok(layout) = SubblockLayout::for_size(block.size);
    let Ok(subblocks) = layout.origins();

    for at in subblocks {
      let Ok(mut neighbors) = block.neighbors.key_modes(at);

      match at.row {
        0 => {}
        _ => {
          let Ok(above) = read(&block.sub_modes, at.col);
          neighbors.above = above;
        }
      }

      match at.col {
        0 => {}
        _ => {
          let Ok(left) = read(&block.sub_modes, at.row.wrapping_mul(2));
          neighbors.left = left;
        }
      }

      let Ok(mode) = key_mode(bd, neighbors);
      let Ok(cells) = layout.cells(at);

      for cell in cells {
        let Ok(()) = write(&mut block.sub_modes, cell, mode);
      }

      block.y_mode = mode;
    }

    Ok(())
  }

  fn read_skip(&mut self, bd: &mut BoolDecoder, block: &mut Blk) -> Result<(), Never> {
    let Ok(feature) = self.s.seg.feature(block.seg_id, SEG_LVL_SKIP);

    match feature {
      Some(_) => block.skip = true,
      None => {
        let above = u64::from(block.neighbors.above.is_some_and(|mi| mi.skip));
        let left = u64::from(block.neighbors.left.is_some_and(|mi| mi.skip));
        let context = above.wrapping_add(left);
        let Ok(probability) = read(&self.fc.skip, context);
        let Ok(skip) = bd.read_bool(probability);
        block.skip = skip != 0;
        let choice = ContextChoice {
          context,
          value: u64::from(skip),
        };
        let Ok(()) = count_choice(self.s.counting, &mut self.strip.counts.skip, choice);
      }
    }

    Ok(())
  }

  fn read_tx_size(
    &mut self,
    bd: &mut BoolDecoder,
    block: &mut Blk,
    selection: TransformSelection,
  ) -> Result<(), Never> {
    let Ok(maximum) = read(&MAX_TXSIZE, u64::from(block.size));
    let Ok(size) = match (selection, self.h.tx_mode, block.size) {
      (TransformSelection::Read, TX_MODE_SELECT, BLOCK_8X8..) => {
        self.signaled_tx_size(bd, block, maximum)
      }
      _ => {
        let Ok(size) = read(&TX_MODE_TO_BIGGEST_TX_SIZE, u64::from(self.h.tx_mode));
        Ok(size.min(maximum))
      }
    };
    block.tx_size = size;
    Ok(())
  }

  fn signaled_tx_size(
    &mut self,
    bd: &mut BoolDecoder,
    block: &Blk,
    maximum: u8,
  ) -> Result<u8, Never> {
    let selected = |mi: MiInfo| match mi.skip.then_some(()) {
      Some(()) => maximum,
      None => mi.tx_size,
    };
    let above = block.neighbors.above.map(selected);
    let left = block.neighbors.left.map(selected);
    let sum = match (above, left) {
      (Some(a), Some(l)) => a.wrapping_add(l),
      (Some(tx), None) | (None, Some(tx)) => tx.wrapping_mul(2),
      (None, None) => maximum.wrapping_mul(2),
    };
    let context = u64::from(sum > maximum);
    let tree: &[i8] = match maximum {
      TX_32X32 => &TX_SIZE_32_TREE,
      TX_16X16 => &TX_SIZE_16_TREE,
      _ => &TX_SIZE_8_TREE,
    };
    let table = u64::from(maximum).wrapping_mul(2).wrapping_add(context);
    let Ok(probabilities) = row(self.fc.tx.as_flattened(), table);
    let Ok(size) = bd.read_tree(tree, probabilities);
    let choice = ContextChoice {
      context: table,
      value: u64::from(size),
    };
    let Ok(()) = count_choice(
      self.s.counting,
      self.strip.counts.tx.as_flattened_mut(),
      choice,
    );
    Ok(size)
  }

  fn inter_frame_mode_info(
    &mut self,
    bd: &mut BoolDecoder,
    block: &mut Blk,
  ) -> Result<(), Never> {
    let Ok(()) = self.inter_segment_id(bd, block);
    let Ok(()) = self.read_skip(bd, block);
    let Ok(prediction) = self.read_prediction(bd, block);
    block.prediction = prediction;
    let selection = TransformSelection::from(!block.skip || prediction == BlockPrediction::Intra);
    let Ok(()) = self.read_tx_size(bd, block, selection);

    match prediction {
      BlockPrediction::Intra => self.intra_block_mode_info(bd, block),
      BlockPrediction::Inter => self.inter_block_mode_info(bd, block),
    }
  }

  fn get_segment_id(&self, block: &Blk) -> Result<u8, Never> {
    let prev = self.s.prev_seg_ids;

    match prev.is_empty().then_some(()) {
      Some(()) => return Ok(0),
      None => {}
    }

    let Ok(width) = read(&NUM_8X8_WIDE, u64::from(block.size));
    let Ok(height) = read(&NUM_8X8_HIGH, u64::from(block.size));
    let last_x = block.c.wrapping_add(u64::from(width)).min(self.mi_cols);
    let last_y = block.r.wrapping_add(u64::from(height)).min(self.mi_rows);
    let mut segment = 7;

    for row in block.r..last_y {
      for col in block.c..last_x {
        let Ok(id) = read(prev, row.wrapping_mul(self.mi_cols).wrapping_add(col));
        segment = segment.min(id);
      }
    }

    Ok(segment)
  }

  fn inter_segment_id(&mut self, bd: &mut BoolDecoder, block: &mut Blk) -> Result<(), Never> {
    let segmentation = self.s.seg;

    match segmentation.status {
      SegmentationStatus::Disabled => {
        block.seg_id = 0;
        return Ok(());
      }
      SegmentationStatus::Enabled => {}
    }

    let Ok(predicted) = self.get_segment_id(block);
    let Ok(segment) = match segmentation.map {
      MapUpdate::Retained => Ok(predicted),
      MapUpdate::Temporal => self.read_temporal_segment(bd, block, predicted),
      MapUpdate::Explicit => bd.read_tree(&SEGMENT_TREE, &segmentation.tree_probs),
    };
    block.seg_id = segment;
    Ok(())
  }

  fn read_temporal_segment(
    &mut self,
    bd: &mut BoolDecoder,
    block: &Blk,
    predicted: u8,
  ) -> Result<u8, Never> {
    let Ok(left) = read(&self.left_seg_pred, block.r);
    let Ok(above) = read(&self.above_seg_pred, block.c);
    let Ok(probability) = read(&self.s.seg.pred_probs, u64::from(left.wrapping_add(above)));
    let Ok(use_prediction) = bd.read_bool(probability);
    let Ok(segment) = match use_prediction {
      0 => bd.read_tree(&SEGMENT_TREE, &self.s.seg.tree_probs),
      _ => Ok(predicted),
    };
    let Ok(width) = read(&NUM_8X8_WIDE, u64::from(block.size));
    let Ok(height) = read(&NUM_8X8_HIGH, u64::from(block.size));
    let Ok(above_row) = span_mut(
      &mut self.above_seg_pred,
      block.c..block.c.wrapping_add(u64::from(width)),
    );
    let Ok(left_row) = span_mut(
      &mut self.left_seg_pred,
      block.r..block.r.wrapping_add(u64::from(height)),
    );
    above_row.fill(u8::from(use_prediction != 0));
    left_row.fill(u8::from(use_prediction != 0));
    Ok(segment)
  }

  fn read_prediction(
    &mut self,
    bd: &mut BoolDecoder,
    block: &Blk,
  ) -> Result<BlockPrediction, Never> {
    let Ok(reference) = self.s.seg.feature(block.seg_id, SEG_LVL_REF_FRAME);

    match reference {
      Some(r) => return Ok(BlockPrediction::from(r != i16::from(INTRA_FRAME))),
      None => {}
    }

    let prediction = |mi: MiInfo| {
      let [reference_2, _] = mi.ref_frame;
      BlockPrediction::from(reference_2 > INTRA_FRAME)
    };
    let above = block.neighbors.above.map(prediction);
    let left = block.neighbors.left.map(prediction);
    use BlockPrediction::{Inter, Intra};
    let context = match (above, left) {
      (Some(Intra), Some(Intra)) => 3,
      (Some(Intra), None) | (None, Some(Intra)) => 2,
      (Some(Intra), Some(Inter)) | (Some(Inter), Some(Intra)) => 1,
      _ => 0,
    };
    let Ok(probability) = read(&self.fc.is_inter, context);
    let Ok(bit) = bd.read_bool(probability);
    let choice = ContextChoice {
      context,
      value: u64::from(bit),
    };
    let Ok(()) = count_choice(self.s.counting, &mut self.strip.counts.is_inter, choice);
    Ok(BlockPrediction::from(bit != 0))
  }

  fn intra_block_mode_info(
    &mut self,
    bd: &mut BoolDecoder,
    block: &mut Blk,
  ) -> Result<(), Never> {
    block.ref_frame = [INTRA_FRAME, NONE];

    match block.size {
      ..=2 => {
        let Ok(layout) = SubblockLayout::for_size(block.size);
        let Ok(subblocks) = layout.origins();

        for at in subblocks {
          let Ok(mode) = self.read_y_mode(bd, 0);
          let Ok(cells) = layout.cells(at);

          for cell in cells {
            let Ok(()) = write(&mut block.sub_modes, cell, mode);
          }

          block.y_mode = mode;
        }
      }
      _ => {
        let Ok(group) = read(&SIZE_GROUP, u64::from(block.size));
        let Ok(mode) = self.read_y_mode(bd, u64::from(group));
        block.y_mode = mode;
        block.sub_modes = [mode; 4];
      }
    }

    let context = u64::from(block.y_mode);
    let Ok(probabilities) = row(&self.fc.uv_mode, context);
    let Ok(mode) = bd.read_tree(&INTRA_MODE_TREE, probabilities);
    block.uv_mode = mode;
    let choice = ContextChoice {
      context,
      value: u64::from(mode),
    };
    count_choice(self.s.counting, &mut self.strip.counts.uv_mode, choice)
  }

  fn read_y_mode(&mut self, bd: &mut BoolDecoder, context: u64) -> Result<u8, Never> {
    let Ok(probabilities) = row(&self.fc.y_mode, context);
    let Ok(mode) = bd.read_tree(&INTRA_MODE_TREE, probabilities);
    let choice = ContextChoice {
      context,
      value: u64::from(mode),
    };
    let Ok(()) = count_choice(self.s.counting, &mut self.strip.counts.intra_mode, choice);
    Ok(mode)
  }

  fn read_ref_frames(&mut self, bd: &mut BoolDecoder, block: &mut Blk) -> Result<(), Never> {
    let Ok(feature) = self.s.seg.feature(block.seg_id, SEG_LVL_REF_FRAME);

    match feature {
      Some(value) => {
        let Ok(reference) = wrapped::<_, i8>(value);
        block.ref_frame = [reference.clamp(LAST_FRAME, ALTREF_FRAME), NONE];
        return Ok(());
      }
      None => {}
    }

    let h = self.h;
    let neighbors = ReferenceNeighbors::from(block.neighbors);
    let Ok(mode) = match h.reference_mode {
      2 => self.read_reference_mode(bd, neighbors),
      mode => Ok(mode),
    };
    let Ok(references) = match mode {
      1 => self.read_compound_reference(bd, neighbors),
      _ => self.read_single_reference(bd, neighbors),
    };
    block.ref_frame = references;
    Ok(())
  }

  fn read_reference_mode(
    &mut self,
    bd: &mut BoolDecoder,
    neighbors: ReferenceNeighbors,
  ) -> Result<u8, Never> {
    let Ok(context) = neighbors.compound_mode_context(self.h.comp_fixed_ref);
    let Ok(probability) = read(&self.fc.comp_mode, context);
    let Ok(bit) = bd.read_bool(probability);
    let choice = ContextChoice {
      context,
      value: u64::from(bit),
    };
    let Ok(()) = count_choice(self.s.counting, &mut self.strip.counts.comp_mode, choice);
    wrapped(bit)
  }

  fn read_compound_reference(
    &mut self,
    bd: &mut BoolDecoder,
    neighbors: ReferenceNeighbors,
  ) -> Result<[i8; 2], Never> {
    let h = self.h;
    let Ok(fixed) = wrapped::<_, u64>(h.comp_fixed_ref);
    let Ok(sign) = read(&h.ref_frame_sign_bias, fixed);
    let pair = CompoundReferences {
      fixed: h.comp_fixed_ref,
      options: h.comp_var_ref,
    };
    let order = CompoundOrder::from(sign);
    let Ok(context) = neighbors.compound_context(pair, order);
    let Ok(probability) = read(&self.fc.comp_ref, context);
    let Ok(bit) = bd.read_bool(probability);
    let choice = ContextChoice {
      context,
      value: u64::from(bit),
    };
    let Ok(()) = count_choice(self.s.counting, &mut self.strip.counts.comp_ref, choice);
    let Ok(variable) = read(&pair.options, u64::from(bit));
    Ok(match order {
      CompoundOrder::FixedFirst => [pair.fixed, variable],
      CompoundOrder::FixedSecond => [variable, pair.fixed],
    })
  }

  fn read_single_reference(
    &mut self,
    bd: &mut BoolDecoder,
    neighbors: ReferenceNeighbors,
  ) -> Result<[i8; 2], Never> {
    let Ok(last_context) = neighbors.last_reference_context();
    let Ok(ref_bit) = self.read_reference_bit(bd, last_context, ReferenceCheck::Last);

    let reference = match ref_bit {
      0 => LAST_FRAME,
      _ => {
        let Ok(context) = neighbors.golden_reference_context();
        let Ok(reference) = self.read_reference_bit(bd, context, ReferenceCheck::Golden);

        match reference {
          0 => GOLDEN_FRAME,
          _ => ALTREF_FRAME,
        }
      }
    };

    Ok([reference, NONE])
  }

  fn read_reference_bit(
    &mut self,
    bd: &mut BoolDecoder,
    context: u64,
    test: ReferenceCheck,
  ) -> Result<u32, Never> {
    let id = match test {
      ReferenceCheck::Last => 0,
      ReferenceCheck::Golden => 1,
    };
    let Ok(probabilities) = row(&self.fc.single_ref, context);
    let Ok(probability) = read(probabilities, id);
    let Ok(bit) = bd.read_bool(probability);
    let context_2 = context.wrapping_mul(2).wrapping_add(id);
    let choice = ContextChoice {
      context: context_2,
      value: u64::from(bit),
    };
    let Ok(()) = count_choice(
      self.s.counting,
      self.strip.counts.single_ref.as_flattened_mut(),
      choice,
    );
    Ok(bit)
  }
}
