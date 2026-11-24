//! Slice segment data decoding (7.3.8) with reconstruction: CTU / coding quadtree / coding unit /
//! prediction unit / transform tree parsing (CABAC, 9.3) interleaved with intra prediction, motion
//! compensation, scaling and inverse transforms. In-loop filters and row publication live in
//! `filter.rs`.

use crate::cabac::{Cabac, ContextTable, Contexts, Initialization};
use crate::decoder::{ParameterSnapshot, ScalingMatrices};
use crate::error::{Result, ensure, invalid};
use crate::inter;
use crate::intra::{self, Refs};
use crate::mvpred::{MotionList, PredictionArea, ReferenceTarget};
use crate::params::{IntraPrediction, Layout, PictureTool, Pps, PredictionWeighting, Sps};
use crate::picture::{Component, Frame, FrameRef, RefPic};
use crate::places::{read, tail, window_mut, write};
use crate::slice::{PredWeightTable, SliceHeader, SliceType};
use crate::spec_tables::*;
use crate::tables::qpc_420;

mod residual;
use console_core_adaptive_arithmetic_decoding::BytePosition;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};
use console_core_video_decoding_bitstream::BitReader;

pub const F_INTRA: u8 = 1;
pub const F_SKIP: u8 = 2;

pub const F_NOFILTER: u8 = 4;

pub const F_CBF: u8 = 8;

pub const F_CODED: u8 = 16;

pub const E_TU_V: u8 = 1;
pub const E_PU_V: u8 = 2;
pub const E_TU_H: u8 = 4;
pub const E_PU_H: u8 = 8;

#[derive(Clone, Copy, Default)]
pub struct Blk {
  pub flags: u8,
  pub depth: u8,
  pub qp: i8,

  pub ipm: u8,
  pub edges: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferencePrediction {
  Used,
  Unused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeighborAvailability {
  Available,
  Unavailable,
}

impl From<bool> for NeighborAvailability {
  fn from(available: bool) -> Self {
    match available.then_some(()) {
      Some(()) => Self::Available,
      None => Self::Unavailable,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockPrediction {
  Intra,
  Inter,
}

impl From<bool> for BlockPrediction {
  fn from(intra: bool) -> Self {
    match intra.then_some(()) {
      Some(()) => Self::Intra,
      None => Self::Inter,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CodedBlock {
  Coefficients,
  Empty,
}

impl From<bool> for CodedBlock {
  fn from(coded: bool) -> Self {
    match coded.then_some(()) {
      Some(()) => Self::Coefficients,
      None => Self::Empty,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResidualTransform {
  Skip,
  Inverse,
}

impl From<bool> for ResidualTransform {
  fn from(skip: bool) -> Self {
    match skip.then_some(()) {
      Some(()) => Self::Skip,
      None => Self::Inverse,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PredictionUnitSyntax {
  Skip,
  Coded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MergeMode {
  Merge,
  Explicit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MvField {
  pub mv: [[i16; 2]; 2],
  pub ref_idx: [i8; 2],
}

impl Default for MvField {
  fn default() -> Self {
    MvField {
      mv: [[0; 2]; 2],
      ref_idx: [-1, -1],
    }
  }
}

impl MvField {
  #[inline]
  pub fn prediction(&self, list: MotionList) -> core::result::Result<ReferencePrediction, Never> {
    let [front, back] = self.ref_idx;
    let reference = match list {
      MotionList::First => front,
      MotionList::Second => back,
    };
    Ok(match reference {
      0.. => ReferencePrediction::Used,
      _ => ReferencePrediction::Unused,
    })
  }
}

#[derive(Clone, Copy, Default, Debug)]
pub enum SaoPlane {
  #[default]
  Off,
  Band {
    first: u8,
    offsets: [i16; 4],
  },
  Edge {
    class: u8,
    offsets: [i16; 4],
  },
}

#[derive(Clone, Copy, Default, Debug)]
pub struct SaoParams(pub [SaoPlane; 3]);

fn edge_magnitudes([a, b, c, d]: [i16; 4]) -> core::result::Result<[i16; 4], Never> {
  Ok([a, b, c.wrapping_neg(), d.wrapping_neg()])
}

enum SaoCoding {
  ReadType,
  ChromaType(SaoPlane),
}

enum SaoType {
  Off,
  Band,
  ReadEdges,
  SharedEdges(u8),
}

pub struct SliceInfo {
  pub addr_rs: u32,
  pub deblocking: PictureTool,
  pub beta_offset: i32,
  pub tc_offset: i32,
  pub lf_across: PictureTool,
  pub sao_luma: PictureTool,
  pub sao_chroma: PictureTool,

  pub ref_ids: [Vec<u32>; 2],
  pub ref_pocs: [Vec<i32>; 2],
  pub ref_lt: [Vec<bool>; 2],
}

pub struct SliceJob {
  pub sh: SliceHeader,
  pub pps: ParameterSnapshot<Pps>,
  pub sps: ParameterSnapshot<Sps>,
  pub rbsp: Vec<u8>,
  pub refs: [Vec<RefPic>; 2],
}

pub struct PicState {
  pub frame: FrameRef,
  pub pps: ParameterSnapshot<Pps>,
  pub layout: ParameterSnapshot<Layout>,
  pub poc: i32,
  pub width: u64,
  pub height: u64,
  pub cwidth: u64,
  pub cheight: u64,
  pub w4: u64,
  pub h4: u64,
  pub log2_ctb: u32,
  pub wctb: u64,
  pub hctb: u64,
  pub bd_y: u32,
  pub bd_c: u32,

  pub planes: [Vec<u16>; 3],
  pub blk: Vec<Blk>,
  pub mvf: Vec<MvField>,

  pub ctb_slice: Vec<Option<u32>>,

  pub ctb_addr: Vec<u32>,
  pub sao: Vec<SaoParams>,
  pub slices: Vec<SliceInfo>,
  pub row_done: Vec<u32>,
  pub rows_complete: u64,
  pub rows_deblocked: u64,
  pub rows_published: u64,
  pub wpp_ctx: Option<Box<Contexts>>,
  pub ds_ctx: Option<(Box<Contexts>, i32)>,

  pub scaling: Option<ScalingMatrices>,
  pub deblocking: PictureTool,

  pub draft: crate::DecodeQuality,
}

impl PicState {
  pub fn new(
    frame: FrameRef,
    sps: &Sps,
    pps: ParameterSnapshot<Pps>,
    layout: ParameterSnapshot<Layout>,
    scaling: Option<ScalingMatrices>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let width = u64::from(sps.width);
    let height = u64::from(sps.height);
    let (cwidth, cheight) = (width.wrapping_div(2), height.wrapping_div(2));
    let (w4, h4) = (width.div_ceil(4), height.div_ceil(4));
    let log2_ctb = sps.log2_ctb;
    let Ok(columns) = sps.pic_width_in_ctbs();
    let Ok(rows) = sps.pic_height_in_ctbs();
    let wctb = u64::from(columns);
    let hctb = u64::from(rows);
    let (bd_y, bd_c) = (sps.bit_depth_luma, sps.bit_depth_chroma);
    let gy = 1u16.wrapping_shl(bd_y.wrapping_sub(1));
    let gc = 1u16.wrapping_shl(bd_c.wrapping_sub(1));

    let Ok(luma) = index(width.wrapping_mul(height));
    let Ok(chroma) = index(cwidth.wrapping_mul(cheight));
    let Ok(blocks) = index(w4.wrapping_mul(h4));
    let Ok(ctbs) = index(wctb.wrapping_mul(hctb));
    let Ok(ctb_rows) = index(hctb);

    Ok(PicState {
      poc: frame.poc,
      frame,
      pps,
      layout,
      width,
      height,
      cwidth,
      cheight,
      w4,
      h4,
      log2_ctb,
      wctb,
      hctb,
      bd_y,
      bd_c,
      planes: [
        vec![gy; luma.min(285_204_544)],
        vec![gc; chroma.min(71_301_136)],
        vec![gc; chroma.min(71_301_136)],
      ],
      blk: vec![Blk::default(); blocks.min(17_825_284)],
      mvf: vec![MvField::default(); blocks.min(17_825_284)],
      ctb_slice: vec![None; ctbs.min(1_115_136)],
      ctb_addr: vec![u32::MAX; ctbs.min(1_115_136)],
      sao: vec![SaoParams::default(); ctbs.min(1_115_136)],
      slices: Vec::new(),
      row_done: vec![0; ctb_rows.min(1056)],
      rows_complete: 0,
      rows_deblocked: 0,
      rows_published: 0,
      wpp_ctx: None,
      ds_ctx: None,
      scaling,
      deblocking: PictureTool::Disabled,
      draft: crate::DecodeQuality::Full,
    })
  }

  #[inline]
  pub fn neighbor(
    &self,
    query: NeighborQuery,
  ) -> core::result::Result<NeighborAvailability, Never> {
    let NeighborQuery {
      current,
      candidate,
      segment,
    } = query;
    let Ok(width) = wrapped::<_, i32>(self.width);
    let Ok(height) = wrapped::<_, i32>(self.height);

    match (candidate.x >= 0 && candidate.y >= 0 && candidate.x < width && candidate.y < height)
      .then_some(())
    {
      Some(()) => {}
      None => return Ok(NeighborAvailability::Unavailable),
    }

    let Ok(neighbor) = self.raster_address(candidate);
    let Ok(center) = self.raster_address(current);
    let available = match (neighbor == center).then_some(()) {
      Some(()) => {
        let mask = 1i32.wrapping_shl(self.log2_ctb).wrapping_sub(1);
        let Ok(a) = morton(candidate, mask);
        let Ok(b) = morton(current, mask);
        a <= b
      }
      None => {
        let Ok(a) = read(&self.layout.rs_to_ts, neighbor);
        let Ok(b) = read(&self.layout.rs_to_ts, center);
        let Ok(address) = read(&self.ctb_addr, neighbor);
        let Ok(tile_a) = read(&self.layout.tile_id, u64::from(a));
        let Ok(tile_b) = read(&self.layout.tile_id, u64::from(b));
        a < b && address == segment && tile_a == tile_b
      }
    };

    Ok(NeighborAvailability::from(available))
  }

  fn raster_address(&self, at: Point<i32>) -> core::result::Result<u64, Never> {
    let Ok(x) = wrapped::<_, u64>(at.x.wrapping_shr(self.log2_ctb));
    let Ok(y) = wrapped::<_, u64>(at.y.wrapping_shr(self.log2_ctb));
    Ok(y.wrapping_mul(self.wctb).wrapping_add(x))
  }

  #[inline]
  pub fn blk_at(&self, at: Point<i32>) -> core::result::Result<Blk, Never> {
    let Ok(x) = wrapped::<_, u64>(at.x.wrapping_shr(2));
    let Ok(y) = wrapped::<_, u64>(at.y.wrapping_shr(2));
    read(&self.blk, y.wrapping_mul(self.w4).wrapping_add(x))
  }
}

pub struct NeighborQuery {
  pub current: Point<i32>,
  pub candidate: Point<i32>,
  pub segment: u32,
}

#[inline]
fn morton(point: Point<i32>, mask: i32) -> core::result::Result<u32, Never> {
  let Ok(x) = wrapped::<_, u32>((point.x & mask).wrapping_shr(2));
  let Ok(y) = wrapped::<_, u32>((point.y & mask).wrapping_shr(2));
  let mut address = 0;

  for bit in 0u32..5 {
    let shift = bit.wrapping_mul(2);
    address |= ((x.wrapping_shr(bit)) & 1).wrapping_shl(shift);
    address |= ((y.wrapping_shr(bit)) & 1).wrapping_shl(shift.wrapping_add(1));
  }

  Ok(address)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PartMode {
  P2Nx2N,
  P2NxN,
  PNx2N,
  PNxN,
  P2NxnU,
  P2NxnD,
  PnLx2N,
  PnRx2N,
}

#[derive(Clone, Copy)]
struct PcmRegion {
  origin: Point<i32>,
  side: u64,
  stride: u64,
  source_depth: u32,
  target_depth: u32,
}

fn read_pcm(reader: &mut BitReader, plane: &mut [u16], region: PcmRegion) -> Result<()> {
  let Ok(start) = sample_address(region.origin, region.stride);
  let shift = region.target_depth.wrapping_sub(region.source_depth);

  for y in 0..region.side {
    let first = start.wrapping_add(y.wrapping_mul(region.stride));
    let Ok(row) = window_mut(plane, first..first.wrapping_add(region.side));
    let Ok(length) = fixed_index(row.len());
    ensure(
      length == region.side,
      || "index or range outside the buffer",
    )?;

    for sample in row {
      let raw = reader.read_bits(region.source_depth)?;
      let Ok(value) = wrapped::<_, u16>(raw);
      *sample = value.wrapping_shl(shift);
    }
  }

  Ok(())
}

fn intra_block_mode(
  picture: &PicState,
  query: NeighborQuery,
) -> core::result::Result<u32, Never> {
  let candidate = query.candidate;
  let Ok(available) = picture.neighbor(query);

  match available {
    NeighborAvailability::Unavailable => Ok(1),
    NeighborAvailability::Available => {
      let Ok(block) = picture.blk_at(candidate);
      Ok(match (block.flags & F_INTRA != 0).then_some(()) {
        Some(()) => u32::from(block.ipm),
        None => 1,
      })
    }
  }
}

#[derive(Clone, Copy)]
struct SampleBlock {
  origin: Point<i32>,
  log2: u32,
  component: Component,
}

#[derive(Clone, Copy)]
struct PlaneGeometry {
  scale: i32,
  stride: u64,
  depth: u32,
  component: Component,
}

impl SampleBlock {
  fn geometry(self, picture: &PicState) -> core::result::Result<PlaneGeometry, Never> {
    let component = self.component;
    let (scale, width, depth) = match component {
      Component::Luma => (1, picture.width, picture.bd_y),
      Component::Blue | Component::Red => (2, picture.cwidth, picture.bd_c),
    };
    Ok(PlaneGeometry {
      scale,
      stride: width,
      depth,
      component,
    })
  }
}

fn sample_address(at: Point<i32>, stride: u64) -> core::result::Result<u64, Never> {
  let Ok(x) = wrapped::<_, u64>(at.x);
  let Ok(y) = wrapped::<_, u64>(at.y);
  Ok(y.wrapping_mul(stride).wrapping_add(x))
}

fn plane_mut(
  planes: &mut [Vec<u16>; 3],
  component: Component,
) -> core::result::Result<&mut [u16], Never> {
  let [y, cb, cr] = planes;
  Ok(match component {
    Component::Luma => y,
    Component::Blue => cb,
    Component::Red => cr,
  })
}

struct IntraSampleLookup<'a> {
  picture: &'a PicState,
  origin: Point<i32>,
  plane: PlaneGeometry,
  segment: u32,
  constraint: IntraPrediction,
}

struct AvailableSamples<'a> {
  left: &'a mut [bool; 129],
  top: &'a mut [bool; 129],
}

impl IntraSampleLookup<'_> {
  fn gather(
    &self,
    side: u64,
    refs: &mut Refs,
    mut present: AvailableSamples<'_>,
  ) -> core::result::Result<(), Never> {
    let origin = self.origin;
    let scale = self.plane.scale;
    let corner = Point {
      x: origin.x.wrapping_sub(1),
      y: origin.y.wrapping_sub(1),
    };
    let current = Point {
      x: origin.x.wrapping_mul(scale),
      y: origin.y.wrapping_mul(scale),
    };
    let left_x = current.x.wrapping_sub(1);
    let top_y = current.y.wrapping_sub(1);
    let Ok(()) = self.gather_corner(current, refs, &mut present);
    let unit = match self.plane.component {
      Component::Luma => 4u64,
      Component::Blue | Component::Red => 2,
    };
    let Ok(step) = index(unit);

    for offset in (0..side.wrapping_mul(2)).step_by(step.max(1)) {
      let Ok(delta) = wrapped::<_, i32>(offset);
      let row = origin.y.wrapping_add(delta);
      let col = origin.x.wrapping_add(delta);
      let left = Point {
        x: left_x,
        y: row.wrapping_mul(scale),
      };
      let top = Point {
        x: col.wrapping_mul(scale),
        y: top_y,
      };
      let Ok(left_present) = self.available(NeighborQuery {
        current,
        candidate: left,
        segment: self.segment,
      });
      let Ok(top_present) = self.available(NeighborQuery {
        current,
        candidate: top,
        segment: self.segment,
      });
      let Ok(()) = self.gather_band(
        IntraBand {
          availability: left_present,
          origin: Point {
            x: corner.x,
            y: row,
          },
          offset,
          count: unit,
          axis: IntraAxis::Left,
        },
        &mut refs.left,
        present.left,
      );
      let Ok(()) = self.gather_band(
        IntraBand {
          availability: top_present,
          origin: Point {
            x: col,
            y: corner.y,
          },
          offset,
          count: unit,
          axis: IntraAxis::Top,
        },
        &mut refs.top,
        present.top,
      );
    }

    Ok(())
  }

  fn gather_corner(
    &self,
    current: Point<i32>,
    refs: &mut Refs,
    present: &mut AvailableSamples<'_>,
  ) -> core::result::Result<(), Never> {
    let neighbor = Point {
      x: current.x.wrapping_sub(1),
      y: current.y.wrapping_sub(1),
    };
    let Ok(available) = self.available(NeighborQuery {
      current,
      candidate: neighbor,
      segment: self.segment,
    });

    match available {
      NeighborAvailability::Available => {
        let corner = Point {
          x: self.origin.x.wrapping_sub(1),
          y: self.origin.y.wrapping_sub(1),
        };
        let Ok(sample) = self.sample(corner);
        let [left, ..] = &mut refs.left;
        let [top, ..] = &mut refs.top;
        *left = sample;
        *top = sample;
        let [left_2, ..] = present.left;
        let [top_2, ..] = present.top;
        *left_2 = true;
        *top_2 = true;
        Ok(())
      }
      NeighborAvailability::Unavailable => Ok(()),
    }
  }

  fn gather_band(
    &self,
    band: IntraBand,
    samples: &mut [u16],
    present: &mut [bool],
  ) -> core::result::Result<(), Never> {
    match band.availability {
      NeighborAvailability::Unavailable => return Ok(()),
      NeighborAvailability::Available => {}
    }

    for offset in 0..band.count {
      let Ok(delta) = wrapped::<_, i32>(offset);
      let mut at = band.origin;

      match band.axis {
        IntraAxis::Left => at.y = at.y.wrapping_add(delta),
        IntraAxis::Top => at.x = at.x.wrapping_add(delta),
      }

      let Ok(sample) = self.sample(at);
      let destination = 1u64.wrapping_add(band.offset).wrapping_add(offset);
      let Ok(()) = write(samples, destination, sample);
      let Ok(()) = write(present, destination, true);
    }

    Ok(())
  }

  fn sample(&self, at: Point<i32>) -> core::result::Result<u16, Never> {
    let [y, cb, cr] = &self.picture.planes;
    let plane = match self.plane.component {
      Component::Luma => y,
      Component::Blue => cb,
      Component::Red => cr,
    };
    let Ok(address) = sample_address(at, self.plane.stride);
    read(plane, address)
  }

  fn available(&self, query: NeighborQuery) -> core::result::Result<NeighborAvailability, Never> {
    let candidate = query.candidate;
    let Ok(available) = self.picture.neighbor(query);

    match (available, self.constraint) {
      (NeighborAvailability::Unavailable, _) | (_, IntraPrediction::AnyNeighbor) => Ok(available),
      (NeighborAvailability::Available, IntraPrediction::IntraNeighbors) => {
        let Ok(block) = self.picture.blk_at(candidate);
        Ok(NeighborAvailability::from(block.flags & F_INTRA != 0))
      }
    }
  }
}

enum IntraAxis {
  Top,
  Left,
}

struct IntraBand {
  availability: NeighborAvailability,
  origin: Point<i32>,
  offset: u64,
  count: u64,
  axis: IntraAxis,
}

fn collocated_reference(job: &SliceJob) -> core::result::Result<Option<RefPic>, Never> {
  let header = &job.sh;

  match (header.temporal_mvp, header.slice_type) {
    (false, _) | (_, SliceType::I) => return Ok(None),
    (true, SliceType::P | SliceType::B) => {}
  }

  let [front, back] = &job.refs;
  let list = match (header.slice_type, header.collocated_from_l0) {
    (SliceType::B, false) => back,
    _ => front,
  };
  let Ok(at) = index(header.collocated_ref_idx);
  Ok(list.get(at).cloned())
}

enum InterDirection {
  Forward,
  Backward,
  Bidirectional,
}

enum Difference {
  Read,
  Zero,
}

struct ReferenceRequest {
  count: u32,
  difference: Difference,
}

struct ExplicitMotion {
  reference: u64,
  delta: [i32; 2],
  predictor: u64,
}

pub enum ReferenceDirections {
  PastOnly,
  IncludesFuture,
}

pub struct SliceDecoder<'a> {
  pub pic: &'a mut PicState,
  pub sh: &'a SliceHeader,
  pub sps: &'a Sps,
  pub pps: &'a Pps,
  pub refs: &'a [Vec<RefPic>; 2],
  pub c: Cabac<'a>,
  pub slice_idx: u32,
  pub slice_addr: u32,
  pub slice_qp: i32,
  pub context_init: Initialization,

  pub qp_y: i32,
  pub qp_y_pred: i32,
  pub last_qp: i32,
  cu_qp_delta: QpDelta,
  pub log2_min_cu_qp_delta: u32,
  pub qp_cb: i32,
  pub qp_cr: i32,

  cu_transquant_bypass: PictureTool,
  cu_prediction: BlockPrediction,
  pub intra_chroma_mode: u32,
  pub max_trafo_depth: u32,
  pub cu_part: PartMode,

  pub coeffs: Vec<i32>,
  pub pred0: Vec<i16>,
  pub pred1: Vec<i16>,
  pub mc: crate::inter::McScratch,
  pub refs_buf: Refs,

  pub col: Option<RefPic>,

  pub reference_directions: ReferenceDirections,
  pub ctb_x: i32,
  pub ctb_y: i32,
}

const MAX_QUADTREE_NODES: u32 = 5_461;

struct TreeNodes<T> {
  pending: [T; 19],
  len: u64,
}

impl<T: Copy> TreeNodes<T> {
  fn with_root(root: T) -> core::result::Result<Self, Never> {
    Ok(Self {
      pending: [root; 19],
      len: 1,
    })
  }

  fn push(&mut self, node: T) -> Result<()> {
    let Ok(at) = index(self.len);

    match self.pending.get_mut(at) {
      Some(place) => {
        *place = node;
        self.len = self.len.wrapping_add(1);
        Ok(())
      }
      None => invalid("coding tree exceeds supported CTB depth"),
    }
  }

  fn pop(&mut self) -> core::result::Result<Option<T>, Never> {
    match self.len {
      0 => Ok(None),
      _ => {
        self.len = self.len.wrapping_sub(1);
        let Ok(at) = index(self.len);
        Ok(self.pending.get(at).copied())
      }
    }
  }
}

#[derive(Clone, Copy)]
struct CodingNode {
  origin: Point<i32>,
  log2: u32,
  depth: u32,
}

impl CodingNode {
  fn region(self) -> core::result::Result<SampleRegion, Never> {
    let side = 1i32.wrapping_shl(self.log2);
    Ok(SampleRegion {
      origin: self.origin,
      size: Size {
        width: side,
        height: side,
      },
    })
  }
}

fn push_coding_children(
  nodes: &mut TreeNodes<CodingNode>,
  parent: CodingNode,
  bounds: Size<i32>,
) -> Result<()> {
  let child_log2 = parent.log2.wrapping_sub(1);
  let depth = parent.depth.wrapping_add(1);
  let Ok(children) = quadtree_positions(parent.origin, child_log2);

  for origin in children.into_iter().rev() {
    match (origin.x < bounds.width && origin.y < bounds.height).then_some(()) {
      Some(()) => nodes.push(CodingNode {
        origin,
        log2: child_log2,
        depth,
      })?,
      None => {}
    }
  }

  Ok(())
}

enum TreeSplit {
  Leaf,
  Divide,
}

impl From<bool> for TreeSplit {
  fn from(split: bool) -> Self {
    match split.then_some(()) {
      Some(()) => Self::Divide,
      None => Self::Leaf,
    }
  }
}

#[derive(Clone, Copy)]
enum QpDelta {
  Pending,
  Decoded(i32),
}

#[derive(Clone, Copy)]
enum Quadrant {
  TopLeft,
  TopRight,
  BottomLeft,
  BottomRight,
}

#[derive(Clone, Copy)]
struct TransformNode {
  block: CodingNode,
  parent: Point<i32>,
  quadrant: Quadrant,
  chroma: [CodedBlock; 2],
}

fn quadtree_positions(
  origin: Point<i32>,
  child_log2: u32,
) -> core::result::Result<[Point<i32>; 4], Never> {
  let half = 1i32.wrapping_shl(child_log2);
  let right = origin.x.wrapping_add(half);
  let bottom = origin.y.wrapping_add(half);
  Ok([
    origin,
    Point {
      x: right,
      y: origin.y,
    },
    Point {
      x: origin.x,
      y: bottom,
    },
    Point {
      x: right,
      y: bottom,
    },
  ])
}

#[derive(Clone, Copy)]
struct SampleRegion {
  origin: Point<i32>,
  size: Size<i32>,
}

struct BlockGrid {
  cols: core::ops::Range<u64>,
  rows: core::ops::Range<u64>,
}

impl SampleRegion {
  fn block_grid(self, picture: &PicState) -> core::result::Result<BlockGrid, Never> {
    let Ok(left) = wrapped::<_, u64>(self.origin.x.wrapping_shr(2));
    let Ok(top) = wrapped::<_, u64>(self.origin.y.wrapping_shr(2));
    let Ok(right) = wrapped::<_, u64>(self.origin.x.wrapping_add(self.size.width));
    let Ok(bottom) = wrapped::<_, u64>(self.origin.y.wrapping_add(self.size.height));
    let right_2 = right.min(picture.width).div_ceil(4);
    let bottom_2 = bottom.min(picture.height).div_ceil(4);
    Ok(BlockGrid {
      cols: left..right_2.max(left),
      rows: top..bottom_2.max(top),
    })
  }
}

struct EdgeFlags {
  vertical: u8,
  horizontal: u8,
}

#[derive(Clone, Copy)]
struct CtbGeometry {
  columns: core::num::NonZeroU32,
  side: i32,
}

#[derive(Clone, Copy)]
struct CtbAddress {
  scan: u64,
  raster: u32,
}

impl CtbAddress {
  fn starts_tile(self, layout: &Layout) -> core::result::Result<CtbBoundary, Never> {
    let Ok(tile) = read(&layout.tile_id, self.scan);
    let Ok(previous) = read(&layout.tile_id, self.scan.wrapping_sub(1));
    Ok(CtbBoundary::from(self.scan == 0 || tile != previous))
  }

  fn prior_tile(
    self,
    layout: &Layout,
    preceding: u32,
  ) -> core::result::Result<TileRelation, Never> {
    let raster = match self.raster.checked_sub(preceding) {
      Some(raster) => raster,
      None => return Ok(TileRelation::Within),
    };
    let Ok(scan) = read(&layout.rs_to_ts, u64::from(raster));
    let Ok(tile) = read(&layout.tile_id, self.scan);
    let Ok(previous) = read(&layout.tile_id, u64::from(scan));
    Ok(
      match ::core::primitive::bool::then_some(tile == previous, ()) {
        ::core::option::Option::Some(()) => TileRelation::Within,
        ::core::option::Option::None => TileRelation::Across,
      },
    )
  }
}

fn layout_value(values: &[u32], at: u64) -> Result<u32> {
  let Ok(at_2) = index(at);

  match values.get(at_2) {
    Some(&value) => Ok(value),
    None => invalid("index or range outside the buffer"),
  }
}

#[derive(PartialEq, Eq)]
enum TileRelation {
  Within,
  Across,
}

enum CtbBoundary {
  Start,
  Within,
}

impl From<bool> for CtbBoundary {
  fn from(start: bool) -> Self {
    match start.then_some(()) {
      Some(()) => Self::Start,
      None => Self::Within,
    }
  }
}

enum CtbSpan {
  First,
  Later,
}

impl From<bool> for CtbSpan {
  fn from(first: bool) -> Self {
    match first.then_some(()) {
      Some(()) => Self::First,
      None => Self::Later,
    }
  }
}

enum SliceEnd {
  Reached,
  More,
}

impl<'a> SliceDecoder<'a> {
  #[inline(always)]
  fn dd(&mut self, ctx: u64) -> core::result::Result<u32, console_core_never::Never> {
    Ok({
      let Ok(explicit_value) = self.c.decode_decision(ctx);
      explicit_value
    })
  }

  #[inline(always)]
  fn bypass(&mut self) -> core::result::Result<u32, console_core_never::Never> {
    Ok({
      let Ok(explicit_value) = self.c.decode_bypass();
      explicit_value
    })
  }

  #[inline]
  fn bypass_bits(&mut self, bits: u32) -> core::result::Result<u32, Never> {
    let mut value = 0u32;
    let chunks = bits.saturating_sub(1).wrapping_div(16);

    for _ in 0..chunks {
      let Ok(word) = self.c.decode_bypass_bits(16);
      value = value.wrapping_shl(16) | word;
    }

    let Ok(rest) = wrapped::<_, u8>(bits.wrapping_sub(chunks.wrapping_mul(16)));
    let Ok(word) = self.c.decode_bypass_bits(rest);
    Ok(value.wrapping_shl(u32::from(rest)) | word)
  }

  fn bypass_unary(&mut self, limit: u32) -> core::result::Result<u32, Never> {
    for count in 0..limit {
      let Ok(code) = self.bypass();

      match code {
        1 => {}
        _ => return Ok(count),
      }
    }

    Ok(limit)
  }

  fn cabac_initialization(
    sh: &SliceHeader,
    pps: &Pps,
  ) -> core::result::Result<Initialization, Never> {
    let table = match (sh.slice_type, sh.cabac_init_flag) {
      (SliceType::I, _) => ContextTable::Intra,
      (SliceType::P, false) | (SliceType::B, true) => ContextTable::Predictive,
      (SliceType::B, false) | (SliceType::P, true) => ContextTable::Bidirectional,
    };
    Ok(Initialization {
      table,
      qp: i32::wrapping_add(pps.init_qp, sh.qp_delta),
    })
  }

  pub fn decode(pic: &'a mut PicState, job: &'a SliceJob) -> Result<()> {
    let sh = &job.sh;
    let (sps, pps) = (&*job.sps, &*job.pps);
    ensure(
      sh.data_offset < job.rbsp.len(),
      || "empty slice segment data",
    )?;
    let Ok(context_init) = Self::cabac_initialization(sh, pps);
    let slice_qp = context_init.qp;

    let slice_addr = match ::core::primitive::bool::then_some(sh.dependent, ()) {
      ::core::option::Option::Some(()) => match pic.slices.last() {
        Some(s) => s.addr_rs,
        None => return invalid("dependent slice segment without a preceding slice"),
      },
      ::core::option::Option::None => sh.segment_address,
    };

    match ::core::primitive::bool::then_some(!sh.dependent, ()) {
      ::core::option::Option::Some(()) => {
        let ids = |list: &[RefPic]| {
          list
            .iter()
            .map(|reference| reference.frame.id)
            .collect::<Vec<_>>()
        };
        let pocs = |list: &[RefPic]| {
          list
            .iter()
            .map(|reference| reference.poc)
            .collect::<Vec<_>>()
        };
        let lts = |list: &[RefPic]| {
          list
            .iter()
            .map(|reference| reference.long_term)
            .collect::<Vec<_>>()
        };
        let [front, back] = &job.refs;
        pic.slices.push(SliceInfo {
          addr_rs: slice_addr,
          deblocking: PictureTool::from(!sh.deblocking_disabled),
          beta_offset: i32::wrapping_mul(sh.beta_offset_div2, 2),
          tc_offset: i32::wrapping_mul(sh.tc_offset_div2, 2),
          lf_across: PictureTool::from(sh.loop_filter_across_slices),
          sao_luma: PictureTool::from(sh.sao_luma),
          sao_chroma: PictureTool::from(sh.sao_chroma),
          ref_ids: [ids(front), ids(back)],
          ref_pocs: [pocs(front), pocs(back)],
          ref_lt: [lts(front), lts(back)],
        });

        match ::core::primitive::bool::then_some(!sh.deblocking_disabled, ()) {
          ::core::option::Option::Some(()) => {
            pic.deblocking = PictureTool::Enabled;
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    let slice_idx = {
      let Ok(numeric_value) =
        console_core_number_conversion::wrapped::<_, u32>(pic.slices.len().wrapping_sub(1));
      numeric_value
    };
    let Ok(col) = collocated_reference(job);
    let reference_directions = match ::core::primitive::bool::then_some(
      job
        .refs
        .iter()
        .flatten()
        .all(|reference| reference.poc <= pic.poc),
      (),
    ) {
      ::core::option::Option::Some(()) => ReferenceDirections::PastOnly,
      ::core::option::Option::None => ReferenceDirections::IncludesFuture,
    };
    let log2_min_cu_qp_delta = u32::wrapping_sub(sps.log2_ctb, pps.diff_cu_qp_delta_depth);
    let Ok(offset) = console_core_number_conversion::fixed_index(sh.data_offset);
    let c = Cabac::new(&job.rbsp, BytePosition(offset), context_init)?;
    let mut d = SliceDecoder {
      pic,
      sh,
      sps,
      pps,
      refs: &job.refs,
      c,
      slice_idx,
      slice_addr,
      slice_qp,
      context_init,
      qp_y: slice_qp,
      qp_y_pred: slice_qp,
      last_qp: slice_qp,
      cu_qp_delta: QpDelta::Pending,
      log2_min_cu_qp_delta,
      qp_cb: 0,
      qp_cr: 0,
      cu_transquant_bypass: PictureTool::Disabled,
      cu_prediction: BlockPrediction::Inter,
      intra_chroma_mode: 0,
      max_trafo_depth: 0,
      cu_part: PartMode::P2Nx2N,
      coeffs: vec![0; 1024],
      pred0: vec![0; 4096],
      pred1: vec![0; 4096],
      mc: Default::default(),
      refs_buf: Refs::default(),
      col,
      reference_directions,
      ctb_x: 0,
      ctb_y: 0,
    };
    d.decode_data()
  }

  fn decode_data(&mut self) -> Result<()> {
    let layout = ParameterSnapshot::clone(&self.pic.layout);
    let start = layout_value(&layout.rs_to_ts, u64::from(self.sh.segment_address))?;
    let start_2 = u64::from(start);
    let Ok(remaining) = tail(&layout.ts_to_rs, start_2);
    ensure(
      !remaining.is_empty(),
      || "index or range outside the buffer",
    )?;
    let columns = match core::num::NonZeroU32::new(layout.width_ctbs) {
      Some(columns) => columns,
      None => return invalid("empty CTB picture"),
    };
    let geometry = CtbGeometry {
      columns,
      side: 1i32.wrapping_shl(self.sps.log2_ctb),
    };

    for (scan, &raster) in (start_2..).zip(remaining) {
      let ctb = CtbAddress { scan, raster };
      let first = scan == start_2;
      let outcome = self.decode_ctb(&layout, ctb, geometry, CtbSpan::from(first))?;

      match outcome {
        SliceEnd::Reached => {
          match self.pps.dependent_slice_segments_enabled {
            PictureTool::Enabled => {
              let Ok(contexts) = self.c.contexts();
              self.pic.ds_ctx = Some((Box::new(contexts), self.last_qp));
            }
            PictureTool::Disabled => {}
          }

          return Ok(());
        }
        SliceEnd::More => {
          let next = scan.wrapping_add(1);
          let Ok(count) = fixed_index(layout.ts_to_rs.len());
          ensure(next < count, || "slice data continues past the last CTB")?;
          let raster_2 = layout_value(&layout.ts_to_rs, next)?;
          self.open_subset(
            &layout,
            CtbAddress {
              scan: next,
              raster: raster_2,
            },
            geometry,
          )?;
        }
      }
    }

    invalid("slice data continues past the last CTB")
  }

  fn decode_ctb(
    &mut self,
    layout: &Layout,
    ctb: CtbAddress,
    geometry: CtbGeometry,
    span: CtbSpan,
  ) -> Result<SliceEnd> {
    let address = u64::from(ctb.raster);
    let Ok(previous) = read(&self.pic.ctb_slice, address);
    let rs = ctb.raster;
    ensure(previous.is_none(), || format!("CTB {rs} decoded twice"))?;
    let Ok(column) = wrapped::<_, i32>(rs % geometry.columns);
    let Ok(row) = wrapped::<_, i32>(rs / geometry.columns);
    let position = Point { x: column, y: row };
    self.ctb_x = column.wrapping_mul(geometry.side);
    self.ctb_y = row.wrapping_mul(geometry.side);
    let Ok(()) = write(&mut self.pic.ctb_slice, address, Some(self.slice_idx));
    let Ok(()) = write(&mut self.pic.ctb_addr, address, self.slice_addr);
    self.restore_ctb_context(layout, ctb, geometry, span)?;

    match (self.sh.sao_luma || self.sh.sao_chroma).then_some(()) {
      Some(()) => self.parse_sao(ctb, position)?,
      None => {}
    }

    self.coding_quadtree(
      Point {
        x: self.ctb_x,
        y: self.ctb_y,
      },
      self.sps.log2_ctb,
    )?;
    let Ok(terminate) = self.c.decode_terminate();
    let Ok(exhaustion) = self.c.exhaustion();
    ensure(
      exhaustion != console_core_adaptive_arithmetic_decoding::Exhaustion::Overrun,
      || "slice data overrun",
    )?;
    self.save_wavefront(layout, ctb, geometry)?;

    let Ok(row_2) = wrapped::<_, u64>(position.y);
    let Ok(()) = crate::filter::ctb_decoded(self.pic, row_2);
    Ok(match terminate {
      1 => SliceEnd::Reached,
      _ => SliceEnd::More,
    })
  }

  fn restore_ctb_context(
    &mut self,
    layout: &Layout,
    ctb: CtbAddress,
    geometry: CtbGeometry,
    span: CtbSpan,
  ) -> Result<()> {
    let Ok(tile) = ctb.starts_tile(layout);
    let Ok(wpp) = self.wavefront_start(layout, ctb, geometry);

    match (tile, wpp, span) {
      (CtbBoundary::Start, _, CtbSpan::First) => self.last_qp = self.slice_qp,
      (CtbBoundary::Start, _, CtbSpan::Later) => {
        let Ok(()) = self.c.initialize_contexts(self.context_init);
        self.last_qp = self.slice_qp;
      }
      (CtbBoundary::Within, CtbBoundary::Start, _) => {
        let current = Point {
          x: self.ctb_x,
          y: self.ctb_y,
        };
        let candidate = Point {
          x: current.x.wrapping_add(geometry.side),
          y: current.y.wrapping_sub(geometry.side),
        };
        let Ok(neighbor) = self.pic.neighbor(NeighborQuery {
          current,
          candidate,
          segment: self.slice_addr,
        });

        match (&self.pic.wpp_ctx, neighbor) {
          (Some(saved), NeighborAvailability::Available) => {
            let Ok(()) = self.c.restore_contexts(saved);
          }
          _ => {
            let Ok(()) = self.c.initialize_contexts(self.context_init);
          }
        }

        self.last_qp = self.slice_qp;
      }
      (CtbBoundary::Within, CtbBoundary::Within, CtbSpan::First) => {
        match (self.sh.dependent, &self.pic.ds_ctx) {
          (true, Some((saved, qp))) => {
            let Ok(()) = self.c.restore_contexts(saved);
            self.last_qp = *qp;
          }
          (true, None) => return invalid("dependent slice segment without stored contexts"),
          (false, _) => {}
        }
      }
      (CtbBoundary::Within, CtbBoundary::Within, CtbSpan::Later) => {}
    }

    Ok(())
  }

  fn wavefront_start(
    &self,
    layout: &Layout,
    ctb: CtbAddress,
    geometry: CtbGeometry,
  ) -> core::result::Result<CtbBoundary, Never> {
    match self.pps.entropy_coding_sync {
      PictureTool::Disabled => Ok(CtbBoundary::Within),
      PictureTool::Enabled => {
        let Ok(prior_tile) = ctb.prior_tile(layout, 1);
        Ok(CtbBoundary::from(
          ctb.raster.is_multiple_of(geometry.columns.get()) || prior_tile == TileRelation::Across,
        ))
      }
    }
  }

  fn save_wavefront(
    &mut self,
    layout: &Layout,
    ctb: CtbAddress,
    geometry: CtbGeometry,
  ) -> Result<()> {
    match self.pps.entropy_coding_sync {
      PictureTool::Enabled => {
        let Ok(prior_tile) = ctb.prior_tile(layout, 2);
        let save = ctb.raster % geometry.columns == 1 || prior_tile == TileRelation::Across;

        match save.then_some(()) {
          Some(()) => {
            let Ok(contexts) = self.c.contexts();
            self.pic.wpp_ctx = Some(Box::new(contexts));
          }
          None => {}
        }
      }
      PictureTool::Disabled => {}
    }

    Ok(())
  }

  fn open_subset(
    &mut self,
    layout: &Layout,
    next: CtbAddress,
    geometry: CtbGeometry,
  ) -> Result<()> {
    let Ok(wpp) = self.wavefront_start(layout, next, geometry);
    let Ok(tile) = next.starts_tile(layout);

    match (wpp, tile, self.pps.tiles_enabled) {
      (CtbBoundary::Start, _, _) | (_, CtbBoundary::Start, PictureTool::Enabled) => {
        let Ok(end) = self.c.decode_terminate();
        ensure(end == 1, || "end_of_subset_one_bit is 0")?;
        let Ok(position) = self.c.bit_pos();
        let Ok(()) = self.c.set_byte_pos(BytePosition(position.div_ceil(8)));
        self.c.init_engine()?;
      }
      _ => {}
    }

    Ok(())
  }

  fn parse_sao(&mut self, ctb: CtbAddress, position: Point<i32>) -> Result<()> {
    let rs = u64::from(ctb.raster);
    let left = (position.x > 0).then_some(rs.wrapping_sub(1));
    let above = (position.y > 0).then_some(rs.wrapping_sub(self.pic.wctb));
    let layout = ParameterSnapshot::clone(&self.pic.layout);
    let tile = layout_value(&layout.tile_id, ctb.scan)?;

    for neighbor in [left, above].into_iter().flatten() {
      let slice = layout_value(&self.pic.ctb_addr, neighbor)?;
      let next = layout_value(&layout.rs_to_ts, neighbor)?;
      let other_tile = layout_value(&layout.tile_id, u64::from(next))?;

      match (slice == self.slice_addr && tile == other_tile).then_some(()) {
        Some(()) => {
          let Ok(merge) = self.dd(SAO_MERGE);

          match merge {
            1 => {
              let Ok(inherited) = read(&self.pic.sao, neighbor);
              let Ok(()) = write(&mut self.pic.sao, rs, inherited);
              return Ok(());
            }
            _ => {}
          }
        }
        None => {}
      }
    }

    let luma = PictureTool::from(self.sh.sao_luma);
    let chroma = PictureTool::from(self.sh.sao_chroma);
    let Ok(y) = self.read_sao_plane(luma, self.pic.bd_y, SaoCoding::ReadType);
    let Ok(cb) = self.read_sao_plane(chroma, self.pic.bd_c, SaoCoding::ReadType);
    let Ok(cr) = self.read_sao_plane(chroma, self.pic.bd_c, SaoCoding::ChromaType(cb));
    let Ok(()) = write(&mut self.pic.sao, rs, SaoParams([y, cb, cr]));
    Ok(())
  }

  fn read_sao_plane(
    &mut self,
    enabled: PictureTool,
    depth: u32,
    coding: SaoCoding,
  ) -> core::result::Result<SaoPlane, Never> {
    match enabled {
      PictureTool::Disabled => return Ok(SaoPlane::Off),
      PictureTool::Enabled => {}
    }

    let Ok(mode) = self.sao_type(coding);

    match mode {
      SaoType::Off => return Ok(SaoPlane::Off),
      SaoType::Band | SaoType::ReadEdges | SaoType::SharedEdges(_) => {}
    }

    let maximum = 1u32
      .wrapping_shl(depth.min(10).wrapping_sub(5))
      .wrapping_sub(1);
    let mut magnitudes = [0i16; 4];

    for magnitude in &mut magnitudes {
      let Ok(value) = self.bypass_unary(maximum);
      let Ok(sample) = wrapped::<_, i16>(value);
      *magnitude = sample;
    }

    Ok(match mode {
      SaoType::Off => SaoPlane::Off,
      SaoType::Band => {
        for magnitude in &mut magnitudes {
          let Ok(value) = self.sao_signed(*magnitude);
          *magnitude = value;
        }

        let Ok(position) = self.bypass_bits(5);
        let Ok(first) = wrapped::<_, u8>(position);
        SaoPlane::Band {
          first,
          offsets: magnitudes,
        }
      }
      SaoType::ReadEdges => {
        let Ok(edge) = self.bypass_bits(2);
        let Ok(class) = wrapped::<_, u8>(edge);
        let Ok(offsets) = edge_magnitudes(magnitudes);
        SaoPlane::Edge { class, offsets }
      }
      SaoType::SharedEdges(class) => {
        let Ok(offsets) = edge_magnitudes(magnitudes);
        SaoPlane::Edge { class, offsets }
      }
    })
  }

  fn sao_signed(&mut self, magnitude: i16) -> core::result::Result<i16, Never> {
    match magnitude {
      0 => Ok(0),
      _ => {
        let Ok(sign) = self.bypass();
        Ok(match sign {
          1 => magnitude.wrapping_neg(),
          _ => magnitude,
        })
      }
    }
  }

  fn sao_type(&mut self, coding: SaoCoding) -> core::result::Result<SaoType, Never> {
    Ok(match coding {
      SaoCoding::ChromaType(SaoPlane::Off) => SaoType::Off,
      SaoCoding::ChromaType(SaoPlane::Band { .. }) => SaoType::Band,
      SaoCoding::ChromaType(SaoPlane::Edge { class, .. }) => SaoType::SharedEdges(class),
      SaoCoding::ReadType => {
        let Ok(present) = self.dd(SAO_TYPE);

        match present {
          0 => SaoType::Off,
          _ => {
            let Ok(edge) = self.bypass();

            match edge {
              0 => SaoType::Band,
              _ => SaoType::ReadEdges,
            }
          }
        }
      }
    })
  }

  fn coding_quadtree(&mut self, origin: Point<i32>, log2: u32) -> Result<()> {
    let Ok(width) = wrapped::<_, i32>(self.pic.width);
    let Ok(height) = wrapped::<_, i32>(self.pic.height);
    let bounds = Size { width, height };
    let Ok(mut nodes) = TreeNodes::with_root(CodingNode {
      origin,
      log2,
      depth: 0,
    });

    for _ in 0..MAX_QUADTREE_NODES {
      let Ok(next) = nodes.pop();
      let node = match next {
        Some(node) => node,
        None => return Ok(()),
      };
      let Ok(split) = self.split_coding_node(node, bounds);

      match self.pps.cu_qp_delta_enabled {
        PictureTool::Enabled => match (node.log2 >= self.log2_min_cu_qp_delta).then_some(()) {
          Some(()) => self.cu_qp_delta = QpDelta::Pending,
          None => {}
        },
        PictureTool::Disabled => {}
      }

      match split {
        TreeSplit::Leaf => self.coding_unit(node)?,
        TreeSplit::Divide => push_coding_children(&mut nodes, node, bounds)?,
      }
    }

    match nodes.len {
      0 => Ok(()),
      _ => invalid("coding tree exceeds supported CTB depth"),
    }
  }

  fn split_coding_node(
    &mut self,
    node: CodingNode,
    bounds: Size<i32>,
  ) -> core::result::Result<TreeSplit, Never> {
    let CodingNode {
      origin,
      log2,
      depth,
    } = node;
    let side = 1i32.wrapping_shl(log2);
    let inside =
      origin.x.wrapping_add(side) <= bounds.width && origin.y.wrapping_add(side) <= bounds.height;
    let above_minimum = log2 > self.sps.log2_min_cb;
    let split = match (inside, above_minimum) {
      (_, false) => TreeSplit::Leaf,
      (false, true) => TreeSplit::Divide,
      (true, true) => {
        let left = Point {
          x: origin.x.wrapping_sub(1),
          y: origin.y,
        };
        let top = Point {
          x: origin.x,
          y: origin.y.wrapping_sub(1),
        };
        let mut context = SPLIT_CU;

        for candidate in [left, top] {
          let Ok(available) = self.pic.neighbor(NeighborQuery {
            current: origin,
            candidate,
            segment: self.slice_addr,
          });

          match available {
            NeighborAvailability::Available => {
              let Ok(block) = self.pic.blk_at(candidate);
              context = context.wrapping_add(u64::from(u32::from(block.depth) > depth));
            }
            NeighborAvailability::Unavailable => {}
          }
        }

        let Ok(flag) = self.dd(context);
        TreeSplit::from(flag == 1)
      }
    };
    Ok(split)
  }

  fn fill_blk(
    &mut self,
    region: SampleRegion,
    update: impl Fn(&mut Blk),
  ) -> core::result::Result<(), Never> {
    let Ok(grid) = region.block_grid(self.pic);

    for y in grid.rows {
      let row = y.wrapping_mul(self.pic.w4);
      let Ok(blocks) = window_mut(
        &mut self.pic.blk,
        row.wrapping_add(grid.cols.start)..row.wrapping_add(grid.cols.end),
      );

      for block in blocks {
        update(block);
      }
    }

    Ok(())
  }

  fn derive_qp_pred(&mut self, origin: Point<i32>) -> core::result::Result<(), Never> {
    let left = Point {
      x: origin.x.wrapping_sub(1),
      y: origin.y,
    };
    let top = Point {
      x: origin.x,
      y: origin.y.wrapping_sub(1),
    };
    let Ok(a) = self.neighbor_qp(NeighborQuery {
      current: origin,
      candidate: left,
      segment: self.slice_addr,
    });
    let Ok(b) = self.neighbor_qp(NeighborQuery {
      current: origin,
      candidate: top,
      segment: self.slice_addr,
    });
    self.qp_y_pred = a.wrapping_add(b).wrapping_add(1).wrapping_shr(1);
    Ok(())
  }

  fn neighbor_qp(&self, query: NeighborQuery) -> core::result::Result<i32, Never> {
    let candidate = query.candidate;
    let a = query.current;
    let shift = self.sps.log2_ctb;
    let same_ctb = a.x.wrapping_shr(shift) == candidate.x.wrapping_shr(shift)
      && a.y.wrapping_shr(shift) == candidate.y.wrapping_shr(shift);
    let Ok(available) = self.pic.neighbor(query);
    Ok(match (available, same_ctb) {
      (NeighborAvailability::Available, true) => {
        let Ok(block) = self.pic.blk_at(candidate);
        i32::from(block.qp)
      }
      _ => self.last_qp,
    })
  }

  fn update_qp(&mut self) -> core::result::Result<(), Never> {
    let delta = match self.cu_qp_delta {
      QpDelta::Pending => 0,
      QpDelta::Decoded(delta) => delta,
    };
    let Ok(luma) = wrapped::<_, i32>(self.pic.bd_y);
    let offset = luma.wrapping_sub(8).wrapping_mul(6);
    let modulus = match core::num::NonZeroI32::new(52i32.wrapping_add(offset)) {
      Some(modulus) => modulus,
      None => return Ok(()),
    };
    let relative = self
      .qp_y_pred
      .wrapping_add(delta)
      .wrapping_add(52)
      .wrapping_add(offset.wrapping_mul(2));
    self.qp_y = relative.wrapping_rem(modulus.get()).wrapping_sub(offset);
    let Ok(chroma) = wrapped::<_, i32>(self.pic.bd_c);
    let offset_c = chroma.wrapping_sub(8).wrapping_mul(6);
    let qpi_cb = self
      .qp_y
      .wrapping_add(self.pps.cb_qp_offset)
      .wrapping_add(self.sh.cb_qp_offset)
      .clamp(offset_c.wrapping_neg(), 57);
    let qpi_cr = self
      .qp_y
      .wrapping_add(self.pps.cr_qp_offset)
      .wrapping_add(self.sh.cr_qp_offset)
      .clamp(offset_c.wrapping_neg(), 57);
    let Ok(cb) = qpc_420(qpi_cb);
    let Ok(cr) = qpc_420(qpi_cr);
    self.qp_cb = cb.wrapping_add(offset_c);
    self.qp_cr = cr.wrapping_add(offset_c);
    Ok(())
  }

  fn coding_unit(&mut self, node: CodingNode) -> Result<()> {
    let Ok(region) = node.region();
    let Ok(syntax) = self.begin_coding_unit(node, region);

    match syntax {
      PredictionUnitSyntax::Skip => {
        self.cu_prediction = BlockPrediction::Inter;
        let area = PredictionArea {
          coding_origin: node.origin,
          coding_side: region.size.width,
          origin: node.origin,
          size: region.size,
          partition: PartMode::P2Nx2N,
          part_index: 0,
        };
        self.prediction_unit(area, PredictionUnitSyntax::Skip)?;
      }
      PredictionUnitSyntax::Coded => {
        let prediction = match self.sh.slice_type {
          SliceType::I => BlockPrediction::Intra,
          SliceType::P | SliceType::B => {
            let Ok(intra) = self.dd(PRED_MODE);
            BlockPrediction::from(intra == 1)
          }
        };
        self.cu_prediction = prediction;
        let part = match (prediction, node.log2 == self.sps.log2_min_cb) {
          (BlockPrediction::Inter, _) | (BlockPrediction::Intra, true) => {
            self.parse_part_mode(prediction, node.log2)?
          }
          (BlockPrediction::Intra, false) => PartMode::P2Nx2N,
        };
        self.cu_part = part;

        match prediction {
          BlockPrediction::Intra => self.intra_coding_unit(node, part, region)?,
          BlockPrediction::Inter => self.inter_coding_unit(node, part, region)?,
        }
      }
    }

    let Ok(qp) = wrapped::<_, i8>(self.qp_y);
    let Ok(()) = self.fill_blk(region, |block| block.qp = qp);
    self.last_qp = self.qp_y;
    Ok(())
  }

  fn begin_coding_unit(
    &mut self,
    node: CodingNode,
    region: SampleRegion,
  ) -> core::result::Result<PredictionUnitSyntax, Never> {
    let mask = 1i32.wrapping_shl(self.log2_min_cu_qp_delta).wrapping_sub(1);

    match ::core::primitive::bool::then_some((node.origin.x | node.origin.y) & mask == 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.derive_qp_pred(node.origin);
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = self.update_qp();
    self.cu_transquant_bypass = match self.pps.transquant_bypass {
      PictureTool::Enabled => {
        let Ok(bypass) = self.dd(CU_TRANSQUANT_BYPASS);
        PictureTool::from(bypass == 1)
      }
      PictureTool::Disabled => PictureTool::Disabled,
    };
    let Ok(syntax) = self.coding_syntax(node.origin);
    let Ok(qp) = wrapped::<_, i8>(self.qp_y);
    let Ok(depth) = wrapped::<_, u8>(node.depth);
    let flags = F_CODED
      | match syntax {
        PredictionUnitSyntax::Skip => F_SKIP,
        PredictionUnitSyntax::Coded => 0,
      }
      | match self.cu_transquant_bypass {
        PictureTool::Enabled => F_NOFILTER,
        PictureTool::Disabled => 0,
      };
    let Ok(()) = self.fill_blk(region, |block| {
      *block = Blk {
        flags,
        depth,
        qp,
        ipm: 1,
        edges: 0,
      }
    });
    let Ok(()) = self.mark_edges(
      region,
      EdgeFlags {
        vertical: E_TU_V | E_PU_V,
        horizontal: E_TU_H | E_PU_H,
      },
    );
    Ok(syntax)
  }

  fn coding_syntax(
    &mut self,
    origin: Point<i32>,
  ) -> core::result::Result<PredictionUnitSyntax, Never> {
    match self.sh.slice_type {
      SliceType::I => return Ok(PredictionUnitSyntax::Coded),
      SliceType::P | SliceType::B => {}
    }

    let left = Point {
      x: origin.x.wrapping_sub(1),
      y: origin.y,
    };
    let above = Point {
      x: origin.x,
      y: origin.y.wrapping_sub(1),
    };
    let mut context = CU_SKIP;

    for neighbor in [left, above] {
      let Ok(available) = self.pic.neighbor(NeighborQuery {
        current: origin,
        candidate: neighbor,
        segment: self.slice_addr,
      });

      match available {
        NeighborAvailability::Available => {
          let Ok(block) = self.pic.blk_at(neighbor);
          context = context.wrapping_add(u64::from(block.flags & F_SKIP != 0));
        }
        NeighborAvailability::Unavailable => {}
      }
    }

    let Ok(bit) = self.dd(context);
    Ok(match bit {
      1 => PredictionUnitSyntax::Skip,
      _ => PredictionUnitSyntax::Coded,
    })
  }

  fn intra_coding_unit(
    &mut self,
    node: CodingNode,
    part: PartMode,
    region: SampleRegion,
  ) -> Result<()> {
    let Ok(()) = self.fill_blk(region, |block| block.flags |= F_INTRA);
    let can_pcm = part == PartMode::P2Nx2N
      && self.sps.pcm == PictureTool::Enabled
      && (self.sps.log2_min_pcm..=self.sps.log2_max_pcm).contains(&node.log2);
    let pcm = match can_pcm.then_some(()) {
      Some(()) => {
        let Ok(terminal) = self.c.decode_terminate();
        terminal
      }
      None => 0,
    };

    match pcm {
      1 => {
        self.pcm_sample(node)?;

        match self.sps.pcm_loop_filter_disabled.then_some(()) {
          Some(()) => {
            let Ok(()) = self.fill_blk(region, |block| block.flags |= F_NOFILTER);
          }
          None => {}
        }

        Ok(())
      }
      _ => {
        self.parse_intra_modes(node, part)?;
        self.max_trafo_depth = self
          .sps
          .max_th_depth_intra
          .wrapping_add(u32::from(part == PartMode::PNxN));
        self.transform_tree(node.origin, node.log2)
      }
    }
  }

  fn inter_coding_unit(
    &mut self,
    node: CodingNode,
    part: PartMode,
    region: SampleRegion,
  ) -> Result<()> {
    let mut merged_whole = MergeMode::Explicit;
    let side = region.size.width;
    let half = side.wrapping_div(2);
    let quarter = side.wrapping_div(4);
    let rest = side.wrapping_sub(quarter);
    let parts: &[(i32, i32, i32, i32)] = match part {
      PartMode::P2Nx2N => &[(0, 0, side, side)],
      PartMode::P2NxN => &[(0, 0, side, half), (0, half, side, half)],
      PartMode::PNx2N => &[(0, 0, half, side), (half, 0, half, side)],
      PartMode::P2NxnU => &[(0, 0, side, quarter), (0, quarter, side, rest)],
      PartMode::P2NxnD => &[(0, 0, side, rest), (0, rest, side, quarter)],
      PartMode::PnLx2N => &[(0, 0, quarter, side), (quarter, 0, rest, side)],
      PartMode::PnRx2N => &[(0, 0, rest, side), (rest, 0, quarter, side)],
      PartMode::PNxN => &[
        (0, 0, half, half),
        (half, 0, half, half),
        (0, half, half, half),
        (half, half, half, half),
      ],
    };

    for (part_index, &(left, top, width, height)) in (0..).zip(parts) {
      let origin = Point {
        x: node.origin.x.wrapping_add(left),
        y: node.origin.y.wrapping_add(top),
      };
      let size = Size { width, height };
      let area = PredictionArea {
        coding_origin: node.origin,
        coding_side: side,
        origin,
        size,
        partition: part,
        part_index,
      };
      let merge = self.prediction_unit(area, PredictionUnitSyntax::Coded)?;

      match ::core::primitive::bool::then_some(part == PartMode::P2Nx2N, ()) {
        ::core::option::Option::Some(()) => merged_whole = merge,
        ::core::option::Option::None => {}
      };

      let vertical = E_PU_V.wrapping_mul(u8::from(left != 0));
      let horizontal = E_PU_H.wrapping_mul(u8::from(top != 0));
      let Ok(()) = self.mark_edges(
        SampleRegion { origin, size },
        EdgeFlags {
          vertical,
          horizontal,
        },
      );
    }

    let root_cbf = match merged_whole {
      MergeMode::Merge => 1,
      MergeMode::Explicit => {
        let Ok(coded) = self.dd(RQT_ROOT_CBF);
        coded
      }
    };

    match root_cbf {
      1 => {
        self.max_trafo_depth = self.sps.max_th_depth_inter;
        self.transform_tree(node.origin, node.log2)
      }
      _ => Ok(()),
    }
  }

  fn mark_edges(
    &mut self,
    region: SampleRegion,
    flags: EdgeFlags,
  ) -> core::result::Result<(), Never> {
    let Ok(grid) = region.block_grid(self.pic);
    let left = grid.cols.start;
    let first_row = grid.rows.start.wrapping_mul(self.pic.w4);

    match flags.vertical {
      0 => {}
      flags_2 => {
        for y in grid.rows {
          let Ok(at) = index(y.wrapping_mul(self.pic.w4).wrapping_add(left));

          match self.pic.blk.get_mut(at) {
            Some(block) => block.edges |= flags_2,
            None => {}
          }
        }
      }
    }

    match flags.horizontal {
      0 => {}
      flags_3 => {
        let Ok(row) = window_mut(
          &mut self.pic.blk,
          first_row.wrapping_add(left)..first_row.wrapping_add(grid.cols.end),
        );

        for block in row {
          block.edges |= flags_3;
        }
      }
    }

    Ok(())
  }

  fn parse_part_mode(&mut self, prediction: BlockPrediction, log2: u32) -> Result<PartMode> {
    match prediction {
      BlockPrediction::Intra => {
        return Ok(
          match ::core::primitive::bool::then_some(
            ({
              let Ok(explicit_value) = self.dd(PART_MODE);
              explicit_value
            }) == 1,
            (),
          ) {
            ::core::option::Option::Some(()) => PartMode::P2Nx2N,
            ::core::option::Option::None => PartMode::PNxN,
          },
        );
      }
      BlockPrediction::Inter => {}
    }

    match ::core::primitive::bool::then_some(
      ({
        let Ok(explicit_value) = self.dd(PART_MODE);
        explicit_value
      }) == 1,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(PartMode::P2Nx2N);
      }
      ::core::option::Option::None => {}
    }

    let min = log2 == self.sps.log2_min_cb;

    match ::core::primitive::bool::then_some(min, ()) {
      ::core::option::Option::Some(()) => {
        match ::core::primitive::bool::then_some(
          ({
            let Ok(explicit_value) = self.dd(PART_MODE.wrapping_add(1));
            explicit_value
          }) == 1,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            return Ok(PartMode::P2NxN);
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(log2 == 3, ()) {
          ::core::option::Option::Some(()) => {
            return Ok(PartMode::PNx2N);
          }
          ::core::option::Option::None => {}
        }

        return Ok(
          match ::core::primitive::bool::then_some(
            ({
              let Ok(explicit_value) = self.dd(PART_MODE.wrapping_add(2));
              explicit_value
            }) == 1,
            (),
          ) {
            ::core::option::Option::Some(()) => PartMode::PNx2N,
            ::core::option::Option::None => PartMode::PNxN,
          },
        );
      }
      ::core::option::Option::None => {}
    }

    let amp = self.sps.amp;
    let hor = ({
      let Ok(explicit_value) = self.dd(PART_MODE.wrapping_add(1));
      explicit_value
    }) == 1;

    match amp {
      PictureTool::Disabled => {
        return Ok(match ::core::primitive::bool::then_some(hor, ()) {
          ::core::option::Option::Some(()) => PartMode::P2NxN,
          ::core::option::Option::None => PartMode::PNx2N,
        });
      }
      PictureTool::Enabled => {}
    }

    let no_amp = ({
      let Ok(explicit_value) = self.dd(PART_MODE.wrapping_add(3));
      explicit_value
    }) == 1;

    match ::core::primitive::bool::then_some(no_amp, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(match ::core::primitive::bool::then_some(hor, ()) {
          ::core::option::Option::Some(()) => PartMode::P2NxN,
          ::core::option::Option::None => PartMode::PNx2N,
        });
      }
      ::core::option::Option::None => {}
    }

    let b = {
      let Ok(explicit_value) = self.bypass();
      explicit_value
    };
    Ok(match (hor, b) {
      (true, 0) => PartMode::P2NxnU,
      (true, _) => PartMode::P2NxnD,
      (false, 0) => PartMode::PnLx2N,
      (false, _) => PartMode::PnRx2N,
    })
  }

  fn pcm_sample(&mut self, node: CodingNode) -> Result<()> {
    let Ok(position) = self.c.bit_pos();
    let start = position.div_ceil(8);
    let Ok(data) = self.c.data();
    let side = 1u64.wrapping_shl(node.log2);
    let cside = side.wrapping_div(2);
    let luma_depth = self.sps.pcm_bit_depth_luma;
    let chroma_depth = self.sps.pcm_bit_depth_chroma;
    let luma_bits = side.wrapping_mul(side).wrapping_mul(u64::from(luma_depth));
    let chroma_bits = 2u64
      .wrapping_mul(cside)
      .wrapping_mul(cside)
      .wrapping_mul(u64::from(chroma_depth));
    let Ok(length) = fixed_index(data.len());
    ensure(
      start.wrapping_add(luma_bits.wrapping_add(chroma_bits).div_ceil(8)) <= length,
      || "PCM samples truncated",
    )?;
    let Ok(payload) = tail(data, start);
    let Ok(mut reader) = BitReader::new(payload);

    let [y, cb, cr] = &mut self.pic.planes;
    let width = self.pic.width;
    let chroma_width = self.pic.cwidth;
    let chroma_origin = Point {
      x: node.origin.x.wrapping_div(2),
      y: node.origin.y.wrapping_div(2),
    };

    read_pcm(
      &mut reader,
      y,
      PcmRegion {
        origin: node.origin,
        side,
        stride: width,
        source_depth: luma_depth,
        target_depth: self.pic.bd_y,
      },
    )?;
    let chroma = PcmRegion {
      origin: chroma_origin,
      side: cside,
      stride: chroma_width,
      source_depth: chroma_depth,
      target_depth: self.pic.bd_c,
    };
    read_pcm(&mut reader, cb, chroma)?;
    read_pcm(&mut reader, cr, chroma)?;

    let Ok(read) = reader.position();
    let Ok(()) = self
      .c
      .set_byte_pos(BytePosition(start.wrapping_add(read.div_ceil(8))));
    self.c.init_engine()
  }

  fn parse_intra_modes(&mut self, node: CodingNode, part: PartMode) -> Result<()> {
    let split = part == PartMode::PNxN;
    let log2 = node.log2.wrapping_sub(u32::from(split));
    let side = 1i32.wrapping_shl(log2);
    let Ok(positions) = quadtree_positions(node.origin, log2);
    let mut previous = [0; 4];
    let count: u32 = match ::core::primitive::bool::then_some(split, ()) {
      ::core::option::Option::Some(()) => 4,
      ::core::option::Option::None => 1,
    };

    for (present, _) in previous.iter_mut().zip(0..count) {
      let Ok(flag) = self.dd(PREV_INTRA_LUMA);
      *present = flag;
    }

    for ((origin, previous_2), _) in positions.into_iter().zip(previous).zip(0..count) {
      let Ok(mut candidates) = self.intra_candidates(origin);
      let mode = match previous_2 {
        1 => {
          let Ok(ordinal) = self.read_mpm_index();
          let Ok(mode) = read(&candidates, ordinal);
          mode
        }
        _ => {
          let Ok(mut mode) = self.bypass_bits(5);
          candidates.sort_unstable();

          for candidate in candidates {
            mode = mode.wrapping_add(u32::from(mode >= candidate));
          }

          mode
        }
      };

      let Ok(mode_2) = wrapped::<_, u8>(mode);
      let region = SampleRegion {
        origin,
        size: Size {
          width: side,
          height: side,
        },
      };
      let Ok(()) = self.fill_blk(region, |block| block.ipm = mode_2);
    }

    let Ok(chroma_signal) = self.dd(INTRA_CHROMA);
    let Ok(block) = self.pic.blk_at(node.origin);
    let luma = u32::from(block.ipm);
    self.intra_chroma_mode = match chroma_signal {
      0 => luma,
      _ => {
        let Ok(ordinal) = self.bypass_bits(2);
        let Ok(mode) = read(&[0, 26, 10, 1], u64::from(ordinal));

        match ::core::primitive::bool::then_some(mode == luma, ()) {
          ::core::option::Option::Some(()) => 34,
          ::core::option::Option::None => mode,
        }
      }
    };

    Ok(())
  }

  fn intra_candidates(&self, origin: Point<i32>) -> core::result::Result<[u32; 3], Never> {
    let left = NeighborQuery {
      current: origin,
      candidate: Point {
        x: origin.x.wrapping_sub(1),
        y: origin.y,
      },
      segment: self.slice_addr,
    };
    let Ok(left_2) = intra_block_mode(self.pic, left);
    let top = origin
      .y
      .wrapping_shr(self.sps.log2_ctb)
      .wrapping_shl(self.sps.log2_ctb);
    let above = match (origin.y > top).then_some(()) {
      Some(()) => {
        let query = NeighborQuery {
          current: origin,
          candidate: Point {
            x: origin.x,
            y: origin.y.wrapping_sub(1),
          },
          segment: self.slice_addr,
        };
        let Ok(mode) = intra_block_mode(self.pic, query);
        mode
      }
      None => 1,
    };

    Ok(match (left_2 == above, left_2 < 2) {
      (true, true) => [0, 1, 26],
      (true, false) => [
        left_2,
        left_2.wrapping_add(29).wrapping_rem(32).wrapping_add(2),
        left_2.wrapping_sub(1).wrapping_rem(32).wrapping_add(2),
      ],
      (false, _) => {
        let third = match (left_2 == 0 || above == 0, left_2 == 1 || above == 1) {
          (false, _) => 0,
          (true, false) => 1,
          (true, true) => 26,
        };
        [left_2, above, third]
      }
    })
  }

  fn read_mpm_index(&mut self) -> core::result::Result<u64, Never> {
    let Ok(first) = self.bypass();

    match first {
      0 => Ok(0),
      _ => {
        let Ok(second) = self.bypass();
        Ok(u64::from(second).wrapping_add(1))
      }
    }
  }

  fn transform_tree(&mut self, origin: Point<i32>, log2: u32) -> Result<()> {
    let Ok(mut nodes) = TreeNodes::with_root(TransformNode {
      block: CodingNode {
        origin,
        log2,
        depth: 0,
      },
      parent: origin,
      quadrant: Quadrant::TopLeft,
      chroma: [CodedBlock::Coefficients; 2],
    });

    for _ in 0..MAX_QUADTREE_NODES {
      let Ok(next) = nodes.pop();
      let node = match next {
        Some(node) => node,
        None => return Ok(()),
      };
      let Ok(split) = self.split_transform(node.block);
      let Ok(chroma) = self.transform_chroma(node);

      match split {
        TreeSplit::Divide => {
          let CodingNode {
            origin: origin_2,
            log2: log2_2,
            depth,
          } = node.block;
          let child_log2 = log2_2.wrapping_sub(1);
          let child_depth = depth.wrapping_add(1);
          let Ok(points) = quadtree_positions(origin_2, child_log2);

          let quadrants = [
            Quadrant::TopLeft,
            Quadrant::TopRight,
            Quadrant::BottomLeft,
            Quadrant::BottomRight,
          ];
          for (child, quadrant) in points.into_iter().zip(quadrants).rev() {
            nodes.push(TransformNode {
              block: CodingNode {
                origin: child,
                log2: child_log2,
                depth: child_depth,
              },
              parent: origin_2,
              quadrant,
              chroma,
            })?;
          }
        }
        TreeSplit::Leaf => self.transform_leaf(TransformNode { chroma, ..node })?,
      }
    }

    match nodes.len {
      0 => Ok(()),
      _ => invalid("transform tree exceeds supported coding depth"),
    }
  }

  fn split_transform(&mut self, block: CodingNode) -> core::result::Result<TreeSplit, Never> {
    let CodingNode { log2, depth, .. } = block;
    let intra_split =
      self.cu_prediction == BlockPrediction::Intra && self.cu_part == PartMode::PNxN;
    let root_split = intra_split && depth == 0;
    let signal = log2 <= self.sps.log2_max_tb
      && log2 > self.sps.log2_min_tb
      && depth < self.max_trafo_depth
      && !root_split;
    let split = match signal.then_some(()) {
      Some(()) => {
        let Ok(flag) = self.dd(SPLIT_TRANSFORM.wrapping_add(5u64.wrapping_sub(u64::from(log2))));
        flag == 1
      }
      None => {
        let inter_split = self.sps.max_th_depth_inter == 0
          && self.cu_prediction == BlockPrediction::Inter
          && self.cu_part != PartMode::P2Nx2N
          && depth == 0;
        log2 > self.sps.log2_max_tb || root_split || inter_split
      }
    };
    Ok(TreeSplit::from(split))
  }

  fn transform_chroma(
    &mut self,
    node: TransformNode,
  ) -> core::result::Result<[CodedBlock; 2], Never> {
    let CodingNode { log2, depth, .. } = node.block;
    let mut chroma = [CodedBlock::Empty; 2];

    match (log2 > 2, depth == 0) {
      (true, root) => {
        for (coefficients, parent) in chroma.iter_mut().zip(node.chroma) {
          match (root, parent) {
            (true, _) | (false, CodedBlock::Coefficients) => {
              let Ok(flag) = self.dd(CBF_CHROMA.wrapping_add(u64::from(depth)));
              *coefficients = CodedBlock::from(flag == 1);
            }
            (false, CodedBlock::Empty) => {}
          }
        }
      }
      (false, false) => chroma = node.chroma,
      (false, true) => {}
    }

    Ok(chroma)
  }

  fn transform_leaf(&mut self, node: TransformNode) -> Result<()> {
    let CodingNode {
      origin,
      log2,
      depth,
    } = node.block;
    let chroma_present = node.chroma.contains(&CodedBlock::Coefficients);
    let luma =
      match (self.cu_prediction == BlockPrediction::Intra || depth != 0 || chroma_present)
        .then_some(())
      {
        Some(()) => {
          let Ok(flag) = self.dd(CBF_LUMA.wrapping_add(u64::from(depth == 0)));
          CodedBlock::from(flag == 1)
        }
        None => CodedBlock::Coefficients,
      };
    let side = 1i32.wrapping_shl(log2);
    let region = SampleRegion {
      origin,
      size: Size {
        width: side,
        height: side,
      },
    };

    match depth {
      0 => {}
      _ => {
        let Ok(()) = self.mark_edges(
          region,
          EdgeFlags {
            vertical: E_TU_V,
            horizontal: E_TU_H,
          },
        );
      }
    }

    match (luma == CodedBlock::Coefficients || chroma_present).then_some(()) {
      Some(()) => self.parse_qp_delta()?,
      None => {}
    }

    match self.cu_prediction {
      BlockPrediction::Intra => {
        let Ok(block) = self.pic.blk_at(origin);
        let Ok(()) = self.intra_predict(
          SampleBlock {
            origin,
            log2,
            component: Component::Luma,
          },
          u32::from(block.ipm),
        );
      }
      BlockPrediction::Inter => {}
    }

    match luma {
      CodedBlock::Coefficients => {
        let Ok(depth_2) = wrapped::<_, i32>(self.pic.bd_y);
        let qp = self
          .qp_y
          .wrapping_add(depth_2.wrapping_sub(8).wrapping_mul(6));
        self.residual(
          SampleBlock {
            origin,
            log2,
            component: Component::Luma,
          },
          qp,
        )?;
        let Ok(()) = self.fill_blk(region, |block| block.flags |= F_CBF);
      }
      CodedBlock::Empty => {}
    }

    self.transform_chroma_leaves(node)
  }

  fn transform_chroma_leaves(&mut self, node: TransformNode) -> Result<()> {
    let CodingNode { origin, log2, .. } = node.block;
    let (origin_2, log2_2) = match (log2 > 2, node.quadrant) {
      (true, _) => (origin, log2.wrapping_sub(1)),
      (false, Quadrant::BottomRight) => (node.parent, 2),
      (false, Quadrant::TopLeft | Quadrant::TopRight | Quadrant::BottomLeft) => return Ok(()),
    };
    let chroma = Point {
      x: origin_2.x.wrapping_div(2),
      y: origin_2.y.wrapping_div(2),
    };

    for ((component, qp), coefficients) in
      [(Component::Blue, self.qp_cb), (Component::Red, self.qp_cr)]
        .into_iter()
        .zip(node.chroma)
    {
      let block = SampleBlock {
        origin: chroma,
        log2: log2_2,
        component,
      };

      match self.cu_prediction {
        BlockPrediction::Intra => {
          let Ok(()) = self.intra_predict(block, self.intra_chroma_mode);
        }
        BlockPrediction::Inter => {}
      }

      match coefficients {
        CodedBlock::Coefficients => self.residual(block, qp)?,
        CodedBlock::Empty => {}
      }
    }

    Ok(())
  }

  fn parse_qp_delta(&mut self) -> Result<()> {
    match (self.pps.cu_qp_delta_enabled, self.cu_qp_delta) {
      (PictureTool::Enabled, QpDelta::Pending) => {}
      (PictureTool::Disabled, _) | (PictureTool::Enabled, QpDelta::Decoded(_)) => return Ok(()),
    }

    let mut magnitude = 0i32;

    for prefix in 0..5 {
      let Ok(flag) = self.dd(CU_QP_DELTA_ABS.wrapping_add(u64::from(prefix != 0)));

      match flag {
        1 => magnitude = magnitude.wrapping_add(1),
        _ => break,
      }
    }

    match magnitude {
      5 => {
        magnitude = {
          let checked_value = self.extend_qp_delta()?;
          checked_value
        }
      }
      _ => {}
    }

    let signed = match magnitude {
      0 => 0,
      _ => {
        let Ok(sign) = self.bypass();

        match sign {
          1 => magnitude.wrapping_neg(),
          _ => magnitude,
        }
      }
    };
    let Ok(depth) = wrapped::<_, i32>(self.pic.bd_y);
    let half = depth.wrapping_sub(8).wrapping_mul(6).wrapping_div(2);
    ensure(
      signed >= 26i32.wrapping_add(half).wrapping_neg() && signed <= 25i32.wrapping_add(half),
      || "CuQpDeltaVal out of range",
    )?;
    self.cu_qp_delta = QpDelta::Decoded(signed);
    let Ok(()) = self.update_qp();
    Ok(())
  }

  fn extend_qp_delta(&mut self) -> Result<i32> {
    let mut magnitude = 5i32;

    for prefix in 0..32 {
      let Ok(bit) = self.bypass();

      match bit {
        1 => magnitude = magnitude.wrapping_add(1i32.wrapping_shl(prefix)),
        _ => {
          let Ok(extra) = self.bypass_bits(prefix);
          let Ok(extra_2) = wrapped::<_, i32>(extra);
          return Ok(magnitude.wrapping_add(extra_2));
        }
      }
    }

    invalid("invalid cu_qp_delta_abs")
  }

  fn intra_predict(&mut self, block: SampleBlock, mode: u32) -> core::result::Result<(), Never> {
    let SampleBlock {
      origin,
      log2,
      component,
    } = block;
    let Ok(geometry) = block.geometry(self.pic);
    let side = 1u64.wrapping_shl(log2);
    let prediction_block = intra::Block {
      side,
      bit_depth: geometry.depth,
    };
    let mut left = [false; 129];
    let mut top = [false; 129];

    let lookup = IntraSampleLookup {
      picture: self.pic,
      origin,
      plane: geometry,
      segment: self.slice_addr,
      constraint: self.pps.constrained_intra_pred,
    };
    let Ok(()) = lookup.gather(
      side,
      &mut self.refs_buf,
      AvailableSamples {
        left: &mut left,
        top: &mut top,
      },
    );

    let available = intra::Available {
      left: &left,
      top: &top,
    };
    let Ok(()) = intra::substitute(&mut self.refs_buf, available, prediction_block);
    let edges = match component {
      Component::Luma => {
        let Ok(()) = intra::filter(
          &mut self.refs_buf,
          mode,
          prediction_block,
          self.sps.strong_intra_smoothing,
        );
        intra::PredictionEdges::Filtered
      }
      Component::Blue | Component::Red => intra::PredictionEdges::Unfiltered,
    };
    let Ok(start) = sample_address(origin, geometry.stride);
    let Ok(plane) = plane_mut(&mut self.pic.planes, component);
    let Ok(samples) = crate::places::tail_mut(plane, start);
    let prediction = intra::Prediction {
      mode,
      block: prediction_block,
      edges,
    };
    intra::predict(
      &self.refs_buf,
      prediction,
      intra::Surface {
        samples,
        stride: geometry.stride,
      },
    )
  }

  fn prediction_unit(
    &mut self,
    area: PredictionArea,
    syntax: PredictionUnitSyntax,
  ) -> Result<MergeMode> {
    let merge = match syntax {
      PredictionUnitSyntax::Skip => MergeMode::Merge,
      PredictionUnitSyntax::Coded => {
        let Ok(flag) = self.dd(MERGE_FLAG);

        match flag {
          1 => MergeMode::Merge,
          _ => MergeMode::Explicit,
        }
      }
    };
    let motion = match merge {
      MergeMode::Merge => {
        let Ok(index) = self.merge_index();
        let Ok(candidate) = self.derive_merge(area, index);
        candidate
      }
      MergeMode::Explicit => self.explicit_motion(area)?,
    };

    for (reference, list) in motion.ref_idx.into_iter().zip(self.refs) {
      match reference {
        0.. => {
          let Ok(ordinal) = wrapped::<_, u64>(reference);
          let Ok(length) = fixed_index(list.len());
          ensure(ordinal < length, || "reference index out of range")?;
        }
        _ => {}
      }
    }

    let Ok(()) = self.store_motion(area, motion);
    let Ok(width) = wrapped::<_, u64>(area.size.width);
    let Ok(height) = wrapped::<_, u64>(area.size.height);
    let block = inter::Block {
      origin: area.origin,
      size: Size { width, height },
    };
    let Ok(()) = self.motion_compensate(block, &motion);
    Ok(merge)
  }

  fn merge_index(&mut self) -> core::result::Result<u64, Never> {
    let count = self.sh.max_num_merge_cand;

    match count {
      0 | 1 => return Ok(0),
      _ => {}
    }

    let Ok(first) = self.dd(MERGE_IDX);

    match first {
      1 => {
        let Ok(extra) = self.bypass_unary(count.wrapping_sub(2));
        Ok(u64::from(extra).wrapping_add(1))
      }
      _ => Ok(0),
    }
  }

  fn explicit_motion(&mut self, area: PredictionArea) -> Result<MvField> {
    let Ok(direction) = self.inter_direction(area);
    let [front, back] = self.sh.num_ref_idx;
    let requests = match direction {
      InterDirection::Forward => [
        Some(ReferenceRequest {
          count: front,
          difference: Difference::Read,
        }),
        None,
      ],
      InterDirection::Backward => [
        None,
        Some(ReferenceRequest {
          count: back,
          difference: Difference::Read,
        }),
      ],
      InterDirection::Bidirectional => [
        Some(ReferenceRequest {
          count: front,
          difference: Difference::Read,
        }),
        Some(ReferenceRequest {
          count: back,
          difference: match ::core::primitive::bool::then_some(self.sh.mvd_l1_zero, ()) {
            ::core::option::Option::Some(()) => Difference::Zero,
            ::core::option::Option::None => Difference::Read,
          },
        }),
      ],
    };
    let mut input = [const { None }; 2];

    for (request, output) in requests.into_iter().zip(&mut input) {
      match request {
        Some(request_2) => {
          let prediction = self.motion_request(request_2)?;
          *output = Some(prediction);
        }
        None => {}
      }
    }

    let mut field = MvField::default();
    let output = field.ref_idx.iter_mut().zip(&mut field.mv);

    for ((list, prediction), (reference, vector)) in [MotionList::First, MotionList::Second]
      .into_iter()
      .zip(input)
      .zip(output)
    {
      match prediction {
        Some(prediction_2) => {
          let target = ReferenceTarget {
            list,
            index: prediction_2.reference,
          };
          let Ok(base) = self.derive_amvp(area, target, prediction_2.predictor);
          let Ok(ref_index) = wrapped::<_, i8>(prediction_2.reference);

          *reference = ref_index;
          for ((&start, change), sample) in base.iter().zip(prediction_2.delta).zip(vector) {
            let Ok(value) = wrapped::<_, i16>(i32::from(start).wrapping_add(change));
            *sample = value;
          }
        }
        None => {}
      }
    }

    Ok(field)
  }

  fn inter_direction(
    &mut self,
    area: PredictionArea,
  ) -> core::result::Result<InterDirection, Never> {
    match self.sh.slice_type {
      SliceType::I | SliceType::P => return Ok(InterDirection::Forward),
      SliceType::B => {}
    }

    match area.size.width.wrapping_add(area.size.height) {
      12 => {}
      _ => {
        let Ok(block) = self.pic.blk_at(area.coding_origin);
        let Ok(both) = self.dd(INTER_PRED_IDC.wrapping_add(u64::from(block.depth)));

        match both {
          1 => return Ok(InterDirection::Bidirectional),
          _ => {}
        }
      }
    }

    let Ok(list) = self.dd(INTER_PRED_IDC.wrapping_add(4));
    Ok(match list {
      0 => InterDirection::Forward,
      _ => InterDirection::Backward,
    })
  }

  fn motion_request(&mut self, request: ReferenceRequest) -> Result<ExplicitMotion> {
    let mut reference = 0u64;

    for ordinal in 0..request.count.saturating_sub(1) {
      let Ok(bit) = match ordinal {
        0 | 1 => self.dd(REF_IDX.wrapping_add(u64::from(ordinal))),
        _ => self.bypass(),
      };

      match bit {
        0 => break,
        _ => reference = reference.wrapping_add(1),
      }
    }

    let delta = match request.difference {
      Difference::Read => self.mvd_coding()?,
      Difference::Zero => [0; 2],
    };
    let Ok(predictor) = self.dd(MVP_FLAG);
    Ok(ExplicitMotion {
      reference,
      delta,
      predictor: u64::from(predictor),
    })
  }

  fn store_motion(
    &mut self,
    area: PredictionArea,
    field: MvField,
  ) -> core::result::Result<(), Never> {
    let origin = area.origin;
    let Ok(left) = wrapped::<_, u64>(origin.x.wrapping_shr(2));
    let Ok(right) = wrapped::<_, u64>(origin.x.wrapping_add(area.size.width).wrapping_shr(2));
    let Ok(top) = wrapped::<_, u64>(origin.y.wrapping_shr(2));
    let Ok(bottom) = wrapped::<_, u64>(origin.y.wrapping_add(area.size.height).wrapping_shr(2));

    for y in top..bottom {
      let row = y.wrapping_mul(self.pic.w4);
      let Ok(slice) = window_mut(
        &mut self.pic.mvf,
        row.wrapping_add(left)..row.wrapping_add(right),
      );
      slice.fill(field);
    }

    Ok(())
  }

  fn mvd_coding(&mut self) -> Result<[i32; 2]> {
    let Ok(x_nonzero) = self.dd(ABS_MVD_GT0);
    let Ok(y_nonzero) = self.dd(ABS_MVD_GT0);
    let nonzero = [x_nonzero, y_nonzero];
    let mut longer = [0; 2];

    for (present, magnitude) in nonzero.into_iter().zip(&mut longer) {
      match present {
        1 => {
          let Ok(flag) = self.dd(ABS_MVD_GT1);
          *magnitude = flag;
        }
        _ => {}
      }
    }

    let mut delta = [0; 2];

    for ((present, longer_2), value) in nonzero.into_iter().zip(longer).zip(&mut delta) {
      match present {
        1 => {
          let checked_value = self.motion_delta(longer_2)?;
          *value = checked_value;
        }
        _ => {}
      }
    }

    Ok(delta)
  }

  fn motion_delta(&mut self, longer: u32) -> Result<i32> {
    let magnitude = match longer {
      1 => {
        let Ok(prefix) = self.bypass_unary(30);
        ensure(prefix < 30, || "invalid abs_mvd_minus2")?;
        let bits = prefix.wrapping_add(1);
        let Ok(suffix) = self.bypass_bits(bits);
        let Ok(value) = wrapped::<_, i32>(1u32.wrapping_shl(bits).wrapping_add(suffix));
        value
      }
      _ => 1,
    };
    let Ok(negative) = self.bypass();
    Ok(match negative {
      1 => magnitude.wrapping_neg(),
      _ => magnitude,
    })
  }

  fn motion_compensate(
    &mut self,
    shape: inter::Block,
    vectors: &MvField,
  ) -> std::result::Result<(), console_core_never::Never> {
    let weighted = match self.sh.slice_type {
      SliceType::P => self.pps.weighted_pred == PredictionWeighting::Weighted,
      SliceType::B => self.pps.weighted_bipred == PredictionWeighting::Weighted,
      SliceType::I => false,
    };
    let weights = self.sh.pwt.as_ref().filter(|_| weighted);
    let Ok(references) = reference_pair(self.refs, vectors);
    let ReferencePair { primary, alternate } = match references {
      Some(references_2) => references_2,
      None => return Ok(()),
    };

    let chroma = inter::Block {
      origin: Point {
        x: shape.origin.x.wrapping_div(2),
        y: shape.origin.y.wrapping_div(2),
      },
      size: Size {
        width: shape.size.width.wrapping_div(2),
        height: shape.size.height.wrapping_div(2),
      },
    };
    let luma_stride = self.pic.width;
    let chroma_stride = self.pic.cwidth;
    let planes = [
      (Component::Luma, shape, luma_stride, self.pic.bd_y),
      (Component::Blue, chroma, chroma_stride, self.pic.bd_c),
      (Component::Red, chroma, chroma_stride, self.pic.bd_c),
    ];

    for (picture, (plane, block, stride, bit_depth)) in self.pic.planes.iter_mut().zip(planes) {
      let Ok(()) = inter::compensate(
        primary.frame,
        inter::Motion {
          block,
          vector: primary.vector,
          plane,
        },
        &mut self.pred0,
        &mut self.mc,
      );
      let secondary = match &alternate {
        Some(alternate_2) => {
          let Ok(()) = inter::compensate(
            alternate_2.frame,
            inter::Motion {
              block,
              vector: alternate_2.vector,
              plane,
            },
            &mut self.pred1,
            &mut self.mc,
          );
          Some((self.pred1.as_slice(), alternate_2.index))
        }
        None => None,
      };
      let Ok(x) = console_core_number_conversion::wrapped::<_, u64>(block.origin.x);
      let Ok(y) = console_core_number_conversion::wrapped::<_, u64>(block.origin.y);
      let Ok(samples) = crate::places::tail_mut(picture, y.wrapping_mul(stride).wrapping_add(x));
      let target = inter::Surface {
        samples,
        size: block.size,
        stride,
        bit_depth,
      };
      let blend = BlendConfig {
        weights,
        plane,
        bit_depth,
      };
      let Ok(source) = blend.prepare((self.pred0.as_slice(), primary.index), secondary);
      let Ok(()) = inter::put(source, target);
    }

    Ok(())
  }
}

#[derive(Clone, Copy)]
enum WeightIndex {
  First(i8),
  Second(i8),
}

struct ReferenceMotion<'a> {
  frame: &'a Frame,
  vector: [i16; 2],
  index: WeightIndex,
}

struct ReferencePair<'a> {
  primary: ReferenceMotion<'a>,
  alternate: Option<ReferenceMotion<'a>>,
}

fn reference_pair<'a>(
  lists: &'a [Vec<RefPic>; 2],
  vectors: &MvField,
) -> core::result::Result<Option<ReferencePair<'a>>, Never> {
  let [first, second] = lists;
  let [first_index, second_index] = vectors.ref_idx;
  let [first_mv, second_mv] = vectors.mv;
  let Ok(list0) = selected_motion(
    first,
    {
      let Ok(prediction) = vectors.prediction(MotionList::First);
      prediction
    },
    WeightIndex::First(first_index),
    first_mv,
  );
  let Ok(list1) = selected_motion(
    second,
    {
      let Ok(prediction) = vectors.prediction(MotionList::Second);
      prediction
    },
    WeightIndex::Second(second_index),
    second_mv,
  );
  Ok(match (list0, list1) {
    (Some(primary), alternate) => Some(ReferencePair { primary, alternate }),
    (None, Some(primary)) => Some(ReferencePair {
      primary,
      alternate: None,
    }),
    (None, None) => None,
  })
}

fn selected_motion(
  references: &[RefPic],
  used: ReferencePrediction,
  index: WeightIndex,
  vector: [i16; 2],
) -> std::result::Result<Option<ReferenceMotion<'_>>, console_core_never::Never> {
  match used {
    ReferencePrediction::Unused => return Ok(None),
    ReferencePrediction::Used => {}
  }

  let (WeightIndex::First(slot) | WeightIndex::Second(slot)) = index;
  let Ok(native) = console_core_number_conversion::wrapped_index(slot);
  Ok(references.get(native).map(|entry| ReferenceMotion {
    frame: &entry.frame,
    vector,
    index,
  }))
}

struct BlendConfig<'a> {
  weights: Option<&'a PredWeightTable>,
  plane: Component,
  bit_depth: u32,
}

type WeightedInput<'a> = (&'a [i16], WeightIndex);

impl BlendConfig<'_> {
  fn prepare<'a>(
    self,
    primary: WeightedInput<'a>,
    secondary: Option<WeightedInput<'a>>,
  ) -> std::result::Result<inter::Blending<'a>, console_core_never::Never> {
    let weights = match self.weights {
      Some(weights) => weights,
      None => {
        return Ok(match secondary {
          Some((second, _)) => inter::Blending::Average {
            first: primary.0,
            second,
          },
          None => inter::Blending::Single(primary.0),
        });
      }
    };
    let denominator = match self.plane {
      Component::Luma => weights.luma_log2_denom,
      Component::Blue | Component::Red => weights.chroma_log2_denom,
    };
    let Ok(first) = self.reference_weight(weights, denominator, primary);
    let second = secondary.map(|reference| {
      let Ok(weight) = self.reference_weight(weights, denominator, reference);
      weight
    });

    Ok(inter::Blending::Weighted {
      first,
      second,
      denominator,
    })
  }

  fn reference_weight<'a>(
    &self,
    weights: &PredWeightTable,
    denominator: u32,
    (samples, position): WeightedInput<'a>,
  ) -> std::result::Result<inter::WeightedSamples<'a>, console_core_never::Never> {
    let [first, second] = &weights.l;
    let (row, index) = match position {
      WeightIndex::First(slot) => (first, slot),
      WeightIndex::Second(slot) => (second, slot),
    };
    let Ok(at) = console_core_number_conversion::wrapped_index(index);
    let (value, offset) = match row.get(at) {
      Some(entry) => {
        let [blue, red] = entry.chroma;

        match self.plane {
          Component::Luma => entry.luma,
          Component::Blue => blue,
          Component::Red => red,
        }
      }
      None => (1i32.wrapping_shl(denominator), 0),
    };
    let offset_2 = offset.wrapping_shl(self.bit_depth.wrapping_sub(8));
    Ok(inter::WeightedSamples {
      samples,
      weight: inter::Weight {
        value,
        offset: offset_2,
      },
    })
  }
}
