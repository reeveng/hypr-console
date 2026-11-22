#![deny(unsafe_op_in_unsafe_fn)]

use crate::include::common::intops::apply_sign;
use crate::include::common::intops::iclip;
use crate::include::dav1d::headers::Rav1dFrameHeader;
use crate::include::dav1d::headers::Rav1dSequenceHeader;
use crate::include::dav1d::headers::Rav1dWarpedMotionType;
use crate::src::align::Align16;
use crate::src::align::AlignedVec64;
use crate::src::cpu::CpuFlags;
use crate::src::disjoint_mut::DisjointMut;
use crate::src::disjoint_mut::DisjointMutArcSlice;
use crate::src::disjoint_mut::DisjointMutGuard;
use crate::src::disjoint_mut::DisjointMutSlice;
use crate::src::enum_map::DefaultValue;
use crate::src::env::fix_mv_precision;
use crate::src::env::get_gmv_2d;
use crate::src::env::{OrderHintComparison, get_poc_diff};
use crate::src::error::Rav1dError::{EINVAL, ENOMEM};
use crate::src::error::Rav1dResult;
use crate::src::internal::Bxy;
use crate::src::intra_edge::{EdgeAvailability, EdgeFlags};
use crate::src::levels::BlockSize;
use crate::src::levels::Mv;
use crate::src::wrap_fn_ptr::wrap_fn_ptr;
use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use std::cmp;
use std::mem;
use std::num::NonZeroU8;
use std::ops::{Range, RangeInclusive};
use zerocopy::FromZeros;
use zerocopy::byteorder::native_endian::I16;

#[doc = "Assembly record with two byte-aligned, native-endian signed vector lanes."]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct RefMvsTemporalBlock {
  vector_y: I16,
  vector_x: I16,
  pub r#ref: i8,
}

const _: () = {
  assert!(mem::size_of::<RefMvsTemporalBlock>() == 5);
  assert!(mem::align_of::<RefMvsTemporalBlock>() == 1);
  assert!(mem::offset_of!(RefMvsTemporalBlock, vector_y) == 0);
  assert!(mem::offset_of!(RefMvsTemporalBlock, vector_x) == 2);
  assert!(mem::offset_of!(RefMvsTemporalBlock, r#ref) == 4);
};

impl RefMvsTemporalBlock {
  #[must_use]
  fn new(vector: Mv, reference: i8) -> Result<Self, Never> {
    Ok(Self {
      vector_y: I16::new(vector.y),
      vector_x: I16::new(vector.x),
      r#ref: reference,
    })
  }

  #[must_use]
  fn vector(self) -> Result<Mv, Never> {
    Ok(Mv { y: self.vector_y.get(), x: self.vector_x.get() })
  }
}

#[derive(Clone, Copy, PartialEq, Eq, FromZeros)]
#[repr(C, align(2))]
pub struct RefMvsRefPair {
  pub r#ref: [i8; 2],
}

const _: () = assert!(mem::size_of::<RefMvsRefPair>() == 2);

impl From<[i8; 2]> for RefMvsRefPair {
  fn from(from: [i8; 2]) -> Self {
    RefMvsRefPair { r#ref: from }
  }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, FromZeros)]
#[repr(C)]
pub struct RefMvsMvPair {
  pub mv: [Mv; 2],
}

const _: () = assert!(mem::size_of::<RefMvsMvPair>() == 8);

#[derive(Clone, Copy, FromZeros)]
#[repr(C, align(4))]
pub struct RefMvsBlock {
  pub mv: RefMvsMvPair,
  pub r#ref: RefMvsRefPair,
  pub bs: BlockSize,
  pub mf: u8,
}

const _: () = assert!(mem::size_of::<RefMvsBlock>() == 12);

#[cfg(all(test, target_arch = "aarch64"))]
#[repr(C)]
pub(crate) struct AsmRefMvsFrame<'a> {
  _lifetime: std::marker::PhantomData<&'a ()>,

  _frm_hdr: *const (),
  pub iw4: i32,
  pub ih4: i32,
  pub iw8: i32,
  pub ih8: i32,
  pub sbsz: i32,
  pub use_ref_frame_mvs: i32,
  pub sign_bias: [u8; 7],
  pub mfmv_sign: [u8; 7],
  pub pocdiff: [i8; 7],
  pub mfmv_ref: [u8; 3],
  pub mfmv_ref2cur: [i32; 3],
  pub mfmv_ref2ref: [[i32; 7]; 3],
  pub n_mfmvs: i32,
  pub n_blocks: i32,
  pub rp: *mut RefMvsTemporalBlock,
  pub rp_ref: *const *mut RefMvsTemporalBlock,
  pub rp_proj: *mut RefMvsTemporalBlock,
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(explicit051_no_machine_width, reason = "the AArch64 assembly parity fixture mirrors the native ptrdiff_t stride field")
  )]
  pub rp_stride: console_core_api_compatibility::NativeOffset,
  pub r: *mut RefMvsBlock,
  pub n_tile_threads: i32,
  pub n_frame_threads: i32,
}

const R_PAD: u64 = 1;

#[derive(Default)]
pub struct RefMvsFrame {
  pub iw4: i32,
  pub ih4: i32,
  pub iw8: i32,
  pub ih8: i32,
  pub sbsz: i32,
  pub sign_bias: [u8; 7],
  pub mfmv_sign: [u8; 7],
  pub pocdiff: [i8; 7],
  temporal_references: [Option<TemporalReference>; 3],
  pub n_blocks: u32,

  pub rp_proj: DisjointMut<AlignedVec64<RefMvsTemporalBlock>>,
  pub rp_stride: u32,
  pub r: DisjointMut<AlignedVec64<RefMvsBlock>>,
  pub n_tile_threads: u32,
  pub n_frame_threads: u32,
}

#[derive(Clone, Copy)]
struct TemporalReference {
  source: u8,
  current_distance: Option<i32>,
  other_distances: [Option<NonZeroU8>; 7],
}

#[derive(Default)]
#[repr(C)]
pub struct RefmvsTileRange {
  pub start: i32,
  pub end: i32,
}

pub struct RefmvsTile {
  pub r: [u64; 37],

  pub rp_proj: u64,

  pub tile_col: RefmvsTileRange,
  pub tile_row: RefmvsTileRange,
}

impl Default for RefmvsTile {
  fn default() -> Self {
    Self {
      r: [Default::default(); 37],
      rp_proj: Default::default(),
      tile_col: Default::default(),
      tile_row: Default::default(),
    }
  }
}

#[derive(Copy, Clone, Default)]
#[repr(C)]
pub struct RefMvsCandidate {
  pub mv: RefMvsMvPair,
  pub weight: i32,
}

pub(crate) struct TemporalMvRegion {
  pub columns: Range<i32>,
  pub rows: Range<i32>,
}

impl TemporalMvRegion {
  fn validate(&self) -> Rav1dResult {
    let rows = self.rows.end.checked_sub(self.rows.start).ok_or(EINVAL)?;

    match (self.columns.start >= 0 && self.columns.end >= self.columns.start,
        self.rows.start >= 0 && (0..=16).contains(&rows)) {
      (true, true) => Ok(()),
      (true, false) | (false, true) | (false, false) => Err(EINVAL),
    }
  }
}

wrap_fn_ptr!(pub(crate) load_tmvs);

impl load_tmvs::Fn {
  pub(crate) fn call(
    &self,
    frame: &RefMvsFrame,
    motion_references: &[Option<DisjointMutArcSlice<RefMvsTemporalBlock>>; 7],
    tile_row: i32,
    region: TemporalMvRegion,
  ) -> Rav1dResult {
    region.validate()?;

    load_tmvs_rust(frame, motion_references, tile_row, region)
  }
}

wrap_fn_ptr!(pub save_tmvs);

impl save_tmvs::Fn {
  pub(crate) fn call(
    &self,
    tile: &RefmvsTile,
    frame: &RefMvsFrame,
    output: &Option<DisjointMutArcSlice<RefMvsTemporalBlock>>,
    region: TemporalMvRegion,
  ) -> Rav1dResult {
    region.validate()?;
    let destination = output.as_ref().ok_or(EINVAL)?;
    let [_, _, _, _, _, _, row_offsets @ ..] = &tile.r;

    save_tmvs_rust(frame, row_offsets, &destination.inner, region)
  }
}

wrap_fn_ptr!(pub splat_mv);

impl splat_mv::Fn {
  pub fn call(
    &self,
    frame: &RefMvsFrame,
    tile: &RefmvsTile,
    motion: &Align16<RefMvsBlock>,
    origin: Bxy,
    extent: Size<i32>,
  ) -> Rav1dResult {
    let width = u64::try_from(extent.width).map_err(|_| EINVAL)?;
    let height = u64::try_from(extent.height).map_err(|_| EINVAL)?;

    match (width, height) {
      (0, _) | (_, 0) => return Ok(()),
      _ => {}
    }

    let x = u64::try_from(origin.x).map_err(|_| EINVAL)?;
    let ring_start = u64::from(origin.y.cast_unsigned() & 31).wrapping_add(5);
    let ring_end = ring_start.checked_add(height).ok_or(EINVAL)?;
    let Ok(selection) = console_core_number_conversion::indices(ring_start..ring_end);
    let row_offsets = tile.r.get(selection).ok_or(EINVAL)?;
    let Ok(capacity) = console_core_number_conversion::fixed_index(frame.r.len());
    let limit = capacity.checked_sub(R_PAD).ok_or(EINVAL)?;
    type Guard<'a> = DisjointMutGuard<'a, AlignedVec64<RefMvsBlock>, [RefMvsBlock]>;
    let mut guards: [Option<Guard<'_>>; 37] = [const { None }; 37];

    for (&base, guard) in row_offsets.iter().zip(&mut guards) {
      match base.cmp(&limit) {
        cmp::Ordering::Less => {}
        cmp::Ordering::Equal | cmp::Ordering::Greater => continue,
      }

      let start = base.checked_add(x).ok_or(EINVAL)?;
      let end = start.checked_add(width).ok_or(EINVAL)?;

      match end.cmp(&capacity) {
        cmp::Ordering::Greater => return Err(EINVAL),
        cmp::Ordering::Less | cmp::Ordering::Equal => {}
      }

      let Ok(range) = console_core_number_conversion::indices(start..end);
      *guard = Some(frame.r.index_mut(range));
    }

    for row in guards.iter_mut().flatten() {
      row.fill(**motion);
    }

    Ok(())
  }
}

pub struct Rav1dRefmvsDSPContext {
  pub load_tmvs: load_tmvs::Fn,
  pub save_tmvs: save_tmvs::Fn,
  pub splat_mv: splat_mv::Fn,
}

struct SpatialMatches<'a> {
  new_motion: &'a mut i32,
  reference_motion: &'a mut i32,
}

enum MotionCandidate {
  Single(Mv),
  Compound([Mv; 2]),
}

fn record_candidate(
  candidates: &mut [RefMvsCandidate],
  count: &mut u8,
  candidate: MotionCandidate,
  weight: i32,
) -> Rav1dResult {
  let Ok(occupied) = console_core_number_conversion::index(*count);
  let (recorded, vacant) = candidates.split_at_mut_checked(occupied).ok_or(EINVAL)?;
  let duplicate = recorded.iter_mut().find(|previous| match candidate {
    MotionCandidate::Single(vector) => {
      let [first, _] = previous.mv.mv;
      first == vector
    }
    MotionCandidate::Compound(vectors) => previous.mv.mv == vectors,
  });

  match duplicate {
    Some(previous) => {
      previous.weight = previous.weight.wrapping_add(weight);
      return Ok(());
    }
    None => {}
  }

  match (*count, vacant) {
    (0..=7, [slot, ..]) => {
      match candidate {
        MotionCandidate::Single(vector) => {
          let [first, _] = &mut slot.mv.mv;
          *first = vector;
        }
        MotionCandidate::Compound(vectors) => slot.mv.mv = vectors,
      }

      slot.weight = weight;
      *count = count.wrapping_add(1);
    }
    _ => {}
  }

  Ok(())
}

fn add_spatial_candidate(
  candidates: &mut [RefMvsCandidate],
  count: &mut u8,
  weight: i32,
  neighbor: RefMvsBlock,
  reference: RefMvsRefPair,
  global: &[Mv; 2],
  matches: SpatialMatches<'_>,
) -> Rav1dResult {
  let [first, second] = neighbor.mv.mv;

  match first.validity() {
    crate::src::levels::MotionVectorValidity::Invalid => return Ok(()),
    crate::src::levels::MotionVectorValidity::Valid => {}
  }

  let choose_vector = |local, predicted: Mv| match (neighbor.mf & 1, predicted.validity()) {
    (1, crate::src::levels::MotionVectorValidity::Valid) => predicted,
    _ => local,
  };
  let [global_first, global_second] = *global;
  let candidate = match (reference.r#ref, neighbor.r#ref == reference) {
    ([requested, -1], _) => {
      let mut vectors = neighbor.r#ref.r#ref.into_iter().zip([first, second]);
      let local = match vectors.find(|(neighbor_ref, _)| *neighbor_ref == requested) {
        Some((_, vector)) => vector,
        None => return Ok(()),
      };
      MotionCandidate::Single(choose_vector(local, global_first))
    }
    (_, true) => MotionCandidate::Compound([
      choose_vector(first, global_first),
      choose_vector(second, global_second),
    ]),
    (_, false) => return Ok(()),
  };

  *matches.reference_motion = 1;
  *matches.new_motion |= ::core::primitive::i32::wrapping_shr(i32::from(neighbor.mf), 1_u32);
  record_candidate(candidates, count, candidate, weight)
}

enum NeighborEdge<'a> {
  Row { start: u64 },
  Column { row_starts: &'a [u64], x: i32 },
}

fn spatial_row_start(rows: &[u64], position: Point<i32>) -> Rav1dResult<u64> {
  let row = u32::try_from(position.y).map_err(|_| EINVAL)?;
  let column = u32::try_from(position.x).map_err(|_| EINVAL)?;
  let Ok(index) = console_core_number_conversion::index(row);
  let base = rows.get(index).copied().ok_or(EINVAL)?;
  base.checked_add(u64::from(column)).ok_or(EINVAL)
}

fn read_motion_block_at_address(
  grid: &DisjointMut<AlignedVec64<RefMvsBlock>>,
  address: u64,
) -> Rav1dResult<RefMvsBlock> {
  let Ok(length) = console_core_number_conversion::fixed_index(grid.len());

  match address.cmp(&length) {
    cmp::Ordering::Less => {
      let Ok(index) = console_core_number_conversion::index(address);
      Ok(*grid.index(index))
    }
    cmp::Ordering::Equal | cmp::Ordering::Greater => Err(EINVAL),
  }
}

pub(crate) fn read_spatial_block(
  grid: &DisjointMut<AlignedVec64<RefMvsBlock>>,
  rows: &[u64],
  position: Point<i32>,
) -> Rav1dResult<RefMvsBlock> {
  let address = spatial_row_start(rows, position)?;
  read_motion_block_at_address(grid, address)
}

impl NeighborEdge<'_> {
  fn location(&self, distance: u32) -> Rav1dResult<u64> {
    match self {
      Self::Row { start } => start.checked_add(u64::from(distance)).ok_or(EINVAL),
      Self::Column { row_starts, x } => {
        let Ok(index) = console_core_number_conversion::index(distance);
        let base = row_starts.get(index).copied().ok_or(EINVAL)?;
        let column = u32::try_from(*x).map_err(|_| EINVAL)?;
        base.checked_add(u64::from(column)).ok_or(EINVAL)
      }
    }
  }

  #[must_use]
  fn spans(&self, block: RefMvsBlock) -> Result<[u32; 2], console_core_never::Never> {
    let Ok(&[width, height, ..]) = block.bs.dimensions();

    Ok(match self {
      Self::Row { .. } => [u32::from(width), u32::from(height)],
      Self::Column { .. } => [u32::from(height), u32::from(width)],
    })
  }

  fn read(
    &self,
    grid: &DisjointMut<AlignedVec64<RefMvsBlock>>,
    distance: u32,
  ) -> Rav1dResult<RefMvsBlock> {
    let address = self.location(distance)?;
    read_motion_block_at_address(grid, address)
  }
}

struct NeighborScan {
  block_span: i32,
  visible_span: i32,
  available_perpendicular: u32,
  step: u32,
}

struct CandidateSearch<'a> {
  candidates: &'a mut [RefMvsCandidate],
  count: &'a mut u8,
  reference: RefMvsRefPair,
  global: &'a [Mv; 2],
  matches: SpatialMatches<'a>,
}

impl CandidateSearch<'_> {
  fn consider(&mut self, neighbor: RefMvsBlock, weight: u32) -> Rav1dResult {
    let signed_weight = i32::try_from(weight).map_err(|_| EINVAL)?;
    add_spatial_candidate(
      self.candidates,
      self.count,
      signed_weight,
      neighbor,
      self.reference,
      self.global,
      SpatialMatches {
        new_motion: self.matches.new_motion,
        reference_motion: self.matches.reference_motion,
      },
    )
  }
}

fn scan_spatial_edge(
  grid: &DisjointMut<AlignedVec64<RefMvsBlock>>,
  edge: NeighborEdge<'_>,
  scan: NeighborScan,
  mut search: CandidateSearch<'_>,
) -> Rav1dResult<u32> {
  let block_span = u32::try_from(scan.block_span).map_err(|_| EINVAL)?;
  let visible_span = u32::try_from(scan.visible_span).map_err(|_| EINVAL)?;
  let first = edge.read(grid, 0)?;
  let Ok([first_span, perpendicular]) = edge.spans(first);
  let initial_span = scan.step.max(block_span.min(first_span));

  match block_span.cmp(&first_span) {
    cmp::Ordering::Less | cmp::Ordering::Equal => {
      let weight = match block_span {
        1 => 2,
        _ => 2.max(scan.available_perpendicular.saturating_mul(2).min(perpendicular)),
      };
      search.consider(first, initial_span.saturating_mul(weight))?;
      return Ok(::core::primitive::u32::wrapping_shr(weight, 1_u32));
    }
    cmp::Ordering::Greater => {}
  }

  search.consider(first, initial_span.saturating_mul(2))?;
  let mut distance = initial_span;

  for _ in 0..visible_span {
    match distance.cmp(&visible_span) {
      cmp::Ordering::Less => {}
      cmp::Ordering::Equal | cmp::Ordering::Greater => break,
    }

    let neighbor = edge.read(grid, distance)?;
    let Ok([span, _]) = edge.spans(neighbor);

    match (span, span.cmp(&block_span)) {
      (1.., cmp::Ordering::Less) => {}
      (0, _) | (1.., cmp::Ordering::Equal | cmp::Ordering::Greater) => return Err(EINVAL),
    }

    let advance = scan.step.max(span);
    search.consider(neighbor, advance.saturating_mul(2))?;
    let checked_value = distance.checked_add(advance).ok_or(EINVAL)?;
    distance = checked_value;
  }

  Ok(1)
}

enum ExtendedScan {
  Continue,
  Complete,
}

fn scan_extended_edges<State>(
  grid: &DisjointMut<AlignedVec64<RefMvsBlock>>,
  edges: &[Option<NeighborEdge<'_>>; 2],
  span: i32,
  state: &mut State,
  mut consider: impl FnMut(RefMvsBlock, &mut State) -> Rav1dResult<ExtendedScan>,
) -> Rav1dResult {
  let span_2 = u32::try_from(span.max(0)).map_err(|_| EINVAL)?;

  for edge in edges.iter().flatten() {
    let mut distance = 0;

    'inner_for_2: for _ in 0..span_2 {
      match distance.cmp(&span_2) {
        cmp::Ordering::Less => {}
        cmp::Ordering::Equal | cmp::Ordering::Greater => break 'inner_for_2,
      }

      let neighbor = edge.read(grid, distance)?;
      let outcome = consider(neighbor, state)?;

      match outcome {
        ExtendedScan::Continue => {}
        ExtendedScan::Complete => return Ok(()),
      }

      let Ok([advance, _]) = edge.spans(neighbor);
      let checked_value = distance.checked_add(advance).ok_or(EINVAL)?;
      distance = checked_value;
    }
  }

  Ok(())
}

struct MotionProjection {
  destination_distance: i32,
  source_distance: NonZeroU8,
}

fn mv_projection(vector: Mv, projection: MotionProjection) -> Rav1dResult<Mv> {
  const RECIPROCALS: [u16; 32] = [
    0, 16384, 8192, 5461, 4096, 3276, 2730, 2340, 2048, 1820, 1638, 1489, 1365, 1260, 1170, 1092,
    1024, 963, 910, 862, 819, 780, 744, 712, 682, 655, 630, 606, 585, 564, 546, 528,
  ];
  let reciprocal = {
    let Ok(index) = console_core_number_conversion::index(projection.source_distance.get());
    RECIPROCALS.get(index).copied().ok_or(EINVAL)?
  };
  let scale = match projection.destination_distance {
    distance @ -31..=31 => distance.wrapping_mul(i32::from(reciprocal)),
    _ => return Err(EINVAL),
  };
  let component = |value: i16| {
    let scaled = i32::from(value).wrapping_mul(scale);
    let rounded = ::core::primitive::i32::wrapping_shr(scaled.wrapping_add(8192).wrapping_add(::core::primitive::i32::wrapping_shr(scaled, 31_u32)), 14_u32);
    let Ok(clipped) = console_core_number_conversion::wrapped::<_, i16>(iclip(rounded, -16383, 16383));
    clipped
  };
  Ok(Mv { x: component(vector.x), y: component(vector.y) })
}

struct GlobalMotionContext<'a> {
  differs: &'a mut i32,
  expected: &'a [Mv; 2],
}

fn add_temporal_candidate(
  frame: &RefMvsFrame,
  candidates: &mut [RefMvsCandidate],
  count: &mut u8,
  block: RefMvsTemporalBlock,
  reference: RefMvsRefPair,
  global: Option<GlobalMotionContext<'_>>,
  header: &Rav1dFrameHeader,
) -> Rav1dResult {
  let Ok(vector) = block.vector();

  match vector.validity() {
    crate::src::levels::MotionVectorValidity::Invalid => return Ok(()),
    crate::src::levels::MotionVectorValidity::Valid => {}
  }

  let distance = u8::try_from(block.r#ref).map_err(|_| EINVAL)?;
  let source_distance = NonZeroU8::new(distance).ok_or(EINVAL)?;
  let project = |target_reference: i8| -> Rav1dResult<Mv> {
    let ordinal = reference_index(target_reference)?;
    let Ok(position) = console_core_number_conversion::index(ordinal);
    let destination = frame.pocdiff.get(position).ok_or(EINVAL)?;
    let projection = MotionProjection { destination_distance: i32::from(*destination), source_distance };
    let mut projected = mv_projection(vector, projection)?;
    let Ok(()) = fix_mv_precision(header, &mut projected);
    Ok(projected)
  };
  let [first, second] = reference.r#ref;
  let primary = project(first)?;
  let candidate = match second {
    -1 => {
      match global {
        Some(GlobalMotionContext { differs, expected: &[primary_global, _] }) => {
          let x_difference = primary.x.wrapping_sub(primary_global.x).wrapping_abs();
          let y_difference = primary.y.wrapping_sub(primary_global.y).wrapping_abs();
          *differs = i32::from(x_difference | y_difference >= 16);
        }
        None => {}
      }

      MotionCandidate::Single(primary)
    }
    secondary_reference => {
      let secondary = project(secondary_reference)?;
      MotionCandidate::Compound([primary, secondary])
    }
  };
  record_candidate(candidates, count, candidate, 2)
}

struct TemporalCandidateSearch<'a> {
  candidates: &'a mut [RefMvsCandidate; 8],
  count: &'a mut u8,
  reference: RefMvsRefPair,
  global: GlobalMotionContext<'a>,
  header: &'a Rav1dFrameHeader,
}

struct TemporalCandidateArea {
  block: Rectangle<i32>,
  visible: Size<i32>,
}

fn scan_temporal_candidates(
  frame: &RefMvsFrame,
  tile: &RefmvsTile,
  area: TemporalCandidateArea,
  mut search: TemporalCandidateSearch<'_>,
) -> Rav1dResult {
  let origin = Point { x: ::core::primitive::i32::wrapping_shr(area.block.origin.x, 1_u32), y: ::core::primitive::i32::wrapping_shr(area.block.origin.y, 1_u32) };
  let stride = u64::from(frame.rp_stride);
  let grid_base = tile.rp_proj;
  let Ok(grid_length) = console_core_number_conversion::fixed_index(frame.rp_proj.len());
  let consider = |relative: Point<i32>, search_2: &mut TemporalCandidateSearch<'_>| -> Rav1dResult {
    let row = (origin.y & 15).checked_add(relative.y).ok_or(EINVAL)?;
    let column = origin.x.checked_add(relative.x).ok_or(EINVAL)?;
    let fixed_row = u64::try_from(row).map_err(|_| EINVAL)?;
    let fixed_column = u64::try_from(column).map_err(|_| EINVAL)?;
    let index = fixed_row.checked_mul(stride)
      .and_then(|offset| offset.checked_add(fixed_column))
      .and_then(|offset| grid_base.checked_add(offset))
      .filter(|index| *index < grid_length)
      .ok_or(EINVAL)?;
    let Ok(native_index) = console_core_number_conversion::index(index);
    let block = *frame.rp_proj.index(native_index);
    let global = match relative {
      Point { x: 0, y: 0 } => Some(GlobalMotionContext {
        differs: &mut *search_2.global.differs,
        expected: search_2.global.expected,
      }),
      _ => None,
    };
    add_temporal_candidate(frame, search_2.candidates, search_2.count, block, search_2.reference, global, search_2.header)
  };
  let w8 = area.visible.width.wrapping_add(1).wrapping_shr(1).min(8);
  let h8 = area.visible.height.wrapping_add(1).wrapping_shr(1).min(8);
  let horizontal_shift = match area.block.size.width { 16.. => 1_u32, _ => 0 };
  let vertical_shift = match area.block.size.height { 16.. => 1_u32, _ => 0 };
  let positions = |count: i32, shift: u32| {
    let limit = count.wrapping_add(shift.cast_signed()).wrapping_shr(shift);
    (0..limit).map(move |slot| slot.wrapping_shl(shift))
  };

  for y in positions(h8, vertical_shift) {
    for x in positions(w8, horizontal_shift) {
      consider(Point { x, y }, &mut search)?;
    }
  }

  let Size { width, height } = area.block.size;

  match (width, height) {
    (2..=15, 2..=15) => {
      let bw8 = ::core::primitive::i32::wrapping_shr(width, 1_u32);
      let bh8 = ::core::primitive::i32::wrapping_shr(height, 1_u32);
      let left = ::core::primitive::i32::wrapping_shr(tile.tile_col.start, 1_u32).max(origin.x & !7);
      let right = ::core::primitive::i32::wrapping_shr(tile.tile_col.end, 1_u32).min((origin.x & !7).wrapping_add(8));
      let bottom = ::core::primitive::i32::wrapping_shr(tile.tile_row.end, 1_u32).min((origin.y & !7).wrapping_add(8));
      let has_bottom = origin.y.wrapping_add(bh8) < bottom;
      let has_right = origin.x.wrapping_add(bw8) < right;
      let adjacent = [
        (has_bottom && origin.x > left).then_some(Point { x: -1, y: bh8 }),
        (has_bottom && has_right).then_some(Point { x: bw8, y: bh8 }),
        (has_right && origin.y.wrapping_add(bh8).wrapping_sub(1) < bottom)
          .then_some(Point { x: bw8, y: bh8.wrapping_sub(1) }),
      ];

      for candidate in adjacent.into_iter().flatten() {
        consider(candidate, &mut search)?;
      }
    }
    _ => {}
  }

  Ok(())
}

enum CompoundMember {
  Primary,
  Secondary,
}

fn append_compound_member(
  slots: &mut [RefMvsCandidate; 2],
  count: &mut u8,
  target: CompoundMember,
  motion: Mv,
) -> Result<(), console_core_never::Never> {
  let Ok(slot_index) = console_core_number_conversion::index(*count);

  match slots.get_mut(slot_index) {
    Some(slot) => {
      let [primary, secondary] = &mut slot.mv.mv;

      match target {
        CompoundMember::Primary => *primary = motion,
        CompoundMember::Secondary => *secondary = motion,
      }

      *count = count.wrapping_add(1);
    }
    None => {}
  }

  Ok(())
}

fn reference_index(reference: i8) -> Rav1dResult<u8> {
  let number = u8::try_from(reference).map_err(|_| EINVAL)?;
  number.checked_sub(1).ok_or(EINVAL)
}

fn reference_sign(reference: i8, signs: &[u8; 7]) -> Rav1dResult<u8> {
  let ordinal = reference_index(reference)?;
  let Ok(position) = console_core_number_conversion::index(ordinal);
  signs.get(position).copied().ok_or(EINVAL)
}

fn align_reference_sign(motion: Mv, [desired, source]: [u8; 2]) -> Result<Mv, console_core_never::Never> {
  Ok(match desired ^ source {
    0 => motion,
    _ => -motion,
  })
}

fn add_compound_extended_candidate(
  candidates: &mut [RefMvsCandidate],
  counts: &mut [u8; 4],
  block: RefMvsBlock,
  [primary_sign, secondary_sign]: [u8; 2],
  references: RefMvsRefPair,
  sign_bias: &[u8; 7],
) -> Rav1dResult {
  let (same, different) = candidates.split_at_mut_checked(2).ok_or(EINVAL)?;
  let same_2 = same.first_chunk_mut::<2>().ok_or(EINVAL)?;
  let different_2 = different.first_chunk_mut::<2>().ok_or(EINVAL)?;
  let [same_primary, same_secondary, different_primary, different_secondary] = counts;
  let [primary, secondary] = references.r#ref;
  let neighbors = block.r#ref.r#ref.into_iter().zip(block.mv.mv);

  for (reference, motion) in neighbors.take_while(|(reference, _)| *reference > 0) {
    let source_sign = reference_sign(reference, sign_bias)?;
    let Ok(for_primary) = align_reference_sign(motion, [primary_sign, source_sign]);
    let Ok(for_secondary) = align_reference_sign(motion, [secondary_sign, source_sign]);

    match (reference.cmp(&primary), reference.cmp(&secondary)) {
      (cmp::Ordering::Equal, _) => {
        let Ok(()) = append_compound_member(same_2, same_primary, CompoundMember::Primary, motion);
        let Ok(()) = append_compound_member(different_2, different_secondary, CompoundMember::Secondary, for_secondary);
      }
      (_, cmp::Ordering::Equal) => {
        let Ok(()) = append_compound_member(same_2, same_secondary, CompoundMember::Secondary, motion);
        let Ok(()) = append_compound_member(different_2, different_primary, CompoundMember::Primary, for_primary);
      }
      _ => {
        let Ok(()) = append_compound_member(different_2, different_primary, CompoundMember::Primary, for_primary);
        let Ok(()) = append_compound_member(different_2, different_secondary, CompoundMember::Secondary, for_secondary);
      }
    }
  }

  Ok(())
}

fn add_single_extended_candidate(
  candidates: &mut [RefMvsCandidate; 8],
  count: &mut u8,
  block: RefMvsBlock,
  target_sign: u8,
  sign_bias: &[u8; 7],
) -> Rav1dResult {
  let neighbors = block.r#ref.r#ref.into_iter().zip(block.mv.mv);

  for (reference, motion) in neighbors.take_while(|(reference, _)| *reference > 0) {
    let source_sign = reference_sign(reference, sign_bias)?;
    let Ok(aligned) = align_reference_sign(motion, [target_sign, source_sign]);
    let Ok(occupied) = console_core_number_conversion::index(*count);
    let (recorded, vacant) = candidates.split_at_mut_checked(occupied).ok_or(EINVAL)?;

    match ::core::primitive::bool::then_some(recorded.iter().any(|candidate| {
      let [first, _] = candidate.mv.mv;
      first == aligned
    }), ()) {
      ::core::option::Option::Some(()) => continue,
      ::core::option::Option::None => {}
    }

    let next = vacant.first_mut().ok_or(EINVAL)?;
    let [first, _] = &mut next.mv.mv;
    *first = aligned;
    next.weight = 2;
    *count = count.wrapping_add(1);
  }

  Ok(())
}

struct CompoundFallback {
  matching: u8,
  alternate: u8,
}

fn fill_compound_member(
  slots: [&mut Mv; 2],
  alternatives: [Mv; 2],
  available: CompoundFallback,
  global: Mv,
) -> Result<(), console_core_never::Never> {
  let Ok(alternate) = console_core_number_conversion::index(available.alternate);
  let Ok(matching) = console_core_number_conversion::index(available.matching);
  let fallback = alternatives.into_iter().take(alternate).chain([global; 2]);

  for (slot, fallback_motion) in slots.into_iter().skip(matching).zip(fallback) {
    *slot = fallback_motion;
  }

  Ok(())
}

fn fill_compound_candidates(
  slots: &mut [RefMvsCandidate],
  [matching_first, matching_second, alternate_first, alternate_second]: [u8; 4],
  [global_first, global_second]: [Mv; 2],
) -> Rav1dResult {
  let (first, second, alternate_a, alternate_b) = match slots {
    [first, second, alternate_a, alternate_b, ..] => (first, second, alternate_a, alternate_b),
    _ => return Err(EINVAL),
  };

  let [primary_a, secondary_a] = &mut first.mv.mv;
  let [primary_b, secondary_b] = &mut second.mv.mv;
  let [alternate_primary_a, alternate_secondary_a] = alternate_a.mv.mv;
  let [alternate_primary_b, alternate_secondary_b] = alternate_b.mv.mv;
  let Ok(()) = fill_compound_member(
    [primary_a, primary_b],
    [alternate_primary_a, alternate_primary_b],
    CompoundFallback { matching: matching_first, alternate: alternate_first },
    global_first,
  );
  let Ok(()) = fill_compound_member(
    [secondary_a, secondary_b],
    [alternate_secondary_a, alternate_secondary_b],
    CompoundFallback { matching: matching_second, alternate: alternate_second },
    global_second,
  );
  Ok(())
}

struct ReferenceMotionBounds {
  horizontal: RangeInclusive<i32>,
  vertical: RangeInclusive<i32>,
}

impl ReferenceMotionBounds {
  fn new(block: Rectangle<i32>, frame_size: Size<i32>) -> Result<Self, console_core_never::Never> {
    let bounds = |position: i32, span: i32, frame_span: i32| {
      let start = position.wrapping_add(span).wrapping_add(4).wrapping_neg().wrapping_mul(32);
      let end = frame_span.wrapping_sub(position).wrapping_add(4).wrapping_mul(32);
      start..=end
    };
    Ok(Self {
      horizontal: bounds(block.origin.x, block.size.width, frame_size.width),
      vertical: bounds(block.origin.y, block.size.height, frame_size.height),
    })
  }

  fn clamp(&self, motion: &mut Mv) -> Result<(), console_core_never::Never> {
    let x = iclip(i32::from(motion.x), *self.horizontal.start(), *self.horizontal.end());
    let y = iclip(i32::from(motion.y), *self.vertical.start(), *self.vertical.end());
    let Ok(horizontal) = console_core_number_conversion::wrapped::<_, i16>(x);
    let Ok(vertical) = console_core_number_conversion::wrapped::<_, i16>(y);
    *motion = Mv { x: horizontal, y: vertical };
    Ok(())
  }
}

pub(crate) struct RefMvsSearch {
  pub references: RefMvsRefPair,
  pub block: BlockSize,
  pub edges: EdgeFlags,
  pub origin: Point<i32>,
}

struct GlobalCandidatePair {
  warped: [Mv; 2],
  by_reference: [Mv; 2],
}

fn global_motion_candidates(
  references: RefMvsRefPair,
  block: Rectangle<i32>,
  frame_hdr: &Rav1dFrameHeader,
) -> Rav1dResult<GlobalCandidatePair> {
  let [requested_primary, requested_secondary] = references.r#ref;

  match (requested_primary, requested_secondary) {
    (0..=7, -1..=7) => {}
    _ => return Err(EINVAL),
  }

  let mut gmv = [Mv::INVALID, Mv::ZERO];
  let mut tgmv = [Mv::ZERO; 2];

  for (ordinal, (warped, predicted)) in references.r#ref.into_iter().zip(gmv.iter_mut().zip(tgmv.iter_mut())) {
    match ordinal {
      ..=0 => continue,
      _ => {}
    }

    let reference = reference_index(ordinal)?;
    let Ok(position) = console_core_number_conversion::index(reference);
    let parameters = frame_hdr.gmv.get(position).ok_or(EINVAL)?;
    let Ok(global) = get_gmv_2d(parameters, block, frame_hdr);
    *predicted = global;
    *warped = match parameters.r#type.cmp(&Rav1dWarpedMotionType::Translation) {
      cmp::Ordering::Greater => global,
      cmp::Ordering::Equal | cmp::Ordering::Less => Mv::INVALID,
    };
  }

  Ok(GlobalCandidatePair { warped: gmv, by_reference: tgmv })
}

pub(crate) fn rav1d_refmvs_find(
  rt: &RefmvsTile,
  rf: &RefMvsFrame,
  mvstack: &mut [RefMvsCandidate; 8],
  ctx: &mut i32,
  search: RefMvsSearch,
  frame_hdr: &Rav1dFrameHeader,
) -> Rav1dResult<u8> {
  let RefMvsSearch { references: r#ref, block: bs, edges: edge_flags, origin: Point { x: bx4, y: by4 } } = search;
  let Ok(&[block_width, block_height, ..]) = bs.dimensions();
  let bw4 = i32::from(block_width);
  let bh4 = i32::from(block_height);
  let w4 = bw4.min(16).min(rt.tile_col.end.wrapping_sub(bx4));
  let h4 = bh4.min(16).min(rt.tile_row.end.wrapping_sub(by4));
  let block = Rectangle {
    origin: Point { x: bx4, y: by4 },
    size: Size { width: bw4, height: bh4 },
  };
  let cnt = &mut 0_u8;
  let GlobalCandidatePair { warped: gmv, by_reference: tgmv } = global_motion_candidates(r#ref, block, frame_hdr)?;

  let mut have_newmv = 0;
  let mut have_col_mvs = 0;
  let mut have_row_mvs = 0;
  let local_row = by4 & 31;
  let (max_rows, mut scanned_rows, top_start) = match ::core::primitive::bool::then_some(by4 > rt.tile_row.start, ()) {
    ::core::option::Option::Some(()) => {
      let available = ::core::primitive::i32::wrapping_shr(by4.wrapping_sub(rt.tile_row.start).wrapping_add(1), 1_u32);
      let depth_limit = 2i32.wrapping_add(i32::from(bh4 > 1));
      let maximum = u32::try_from(available.min(depth_limit)).map_err(|_| EINVAL)?;
      let start = spatial_row_start(&rt.r, Point { x: bx4, y: local_row.wrapping_add(4) })?;
      let scanned = scan_spatial_edge(
        &rf.r,
        NeighborEdge::Row { start },
        NeighborScan {
          block_span: bw4,
          visible_span: w4,
          available_perpendicular: maximum,
          step: match bw4 { 16.. => 4, _ => 1 },
        },
        CandidateSearch {
          candidates: mvstack,
          count: cnt,
          reference: r#ref,
          global: &gmv,
          matches: SpatialMatches { new_motion: &mut have_newmv, reference_motion: &mut have_row_mvs },
        },
      )?;
      (maximum, Some(scanned), Some(start))
    }
    ::core::option::Option::None => (0, None, None),
  };

  let (max_cols, mut scanned_columns, left_rows) = match ::core::primitive::bool::then_some(bx4 > rt.tile_col.start, ()) {
    ::core::option::Option::Some(()) => {
      let available = ::core::primitive::i32::wrapping_shr(bx4.wrapping_sub(rt.tile_col.start).wrapping_add(1), 1_u32);
      let depth_limit = 2i32.wrapping_add(i32::from(bw4 > 1));
      let maximum = u32::try_from(available.min(depth_limit)).map_err(|_| EINVAL)?;
      let Ok(row_index) = console_core_number_conversion::index(local_row.wrapping_add(5));
      let row_starts = rt.r.get(row_index..).ok_or(EINVAL)?;
      let scanned = scan_spatial_edge(
        &rf.r,
        NeighborEdge::Column { row_starts, x: bx4.wrapping_sub(1) },
        NeighborScan {
          block_span: bh4,
          visible_span: h4,
          available_perpendicular: maximum,
          step: match bh4 { 16.. => 4, _ => 1 },
        },
        CandidateSearch {
          candidates: mvstack,
          count: cnt,
          reference: r#ref,
          global: &gmv,
          matches: SpatialMatches { new_motion: &mut have_newmv, reference_motion: &mut have_col_mvs },
        },
      )?;
      (maximum, Some(scanned), Some(row_starts))
    }
    ::core::option::Option::None => (0, None, None),
  };

  let right_available = matches!(edge_flags.availability(EdgeFlags::I444_TOP_HAS_RIGHT), Ok(EdgeAvailability::Present))
    && cmp::max(bw4, bh4) <= 16
    && bw4.wrapping_add(bx4) < rt.tile_col.end;

  match (top_start, right_available) {
    (Some(start), true) => {
      let width = u32::try_from(bw4).map_err(|_| EINVAL)?;
      let neighbor = NeighborEdge::Row { start }.read(&rf.r, width)?;
      add_spatial_candidate(
        mvstack,
        cnt,
        4,
        neighbor,
        r#ref,
        &gmv,
        SpatialMatches { new_motion: &mut have_newmv, reference_motion: &mut have_row_mvs },
      )?;
    }
    (Some(_), false) | (None, _) => {}
  }

  let nearest_match = i32::wrapping_add(have_col_mvs, have_row_mvs);
  let nearest_count = u64::from(*cnt);
  let nearest_candidates = mvstack.get_mut(..match ::console_core_number_conversion::index(nearest_count) { ::core::result::Result::Ok(native_position) => native_position }).ok_or(EINVAL)?;

  for candidate in nearest_candidates {
    candidate.weight = candidate.weight.wrapping_add(640);
  }

  let mut globalmv_ctx = i32::from(frame_hdr.use_ref_frame_mvs);

  match ::core::primitive::bool::then_some(rf.temporal_references.iter().any(Option::is_some), ()) {
    ::core::option::Option::Some(()) => scan_temporal_candidates(
      rf,
      rt,
      TemporalCandidateArea {
        block,
        visible: Size { width: w4, height: h4 },
      },
      TemporalCandidateSearch {
        candidates: mvstack,
        count: cnt,
        reference: r#ref,
        global: GlobalMotionContext { differs: &mut globalmv_ctx, expected: &tgmv },
        header: frame_hdr,
      },
    )?,
    ::core::option::Option::None => {}
  }

  let mut have_dummy_newmv_match = 0;

  match (top_start, left_rows) {
    (Some(start), Some(_)) => {
      let before_origin = start.checked_sub(1).ok_or(EINVAL)?;
      let neighbor = NeighborEdge::Row { start: before_origin }.read(&rf.r, 0)?;
      add_spatial_candidate(
        mvstack,
        cnt,
        4,
        neighbor,
        r#ref,
        &gmv,
        SpatialMatches { new_motion: &mut have_dummy_newmv_match, reference_motion: &mut have_row_mvs },
      )?;
    }
    (Some(_), None) | (None, _) => {}
  }

  for n in 2..=3 {
    match scanned_rows.as_mut().filter(|scanned| n > **scanned && n <= max_rows) {
      Some(scanned) => {
        let Ok(ordinal) = console_core_number_conversion::wrapped::<_, i32>(n);
        let even_distance = ordinal.wrapping_mul(2);
        let previous_row = local_row.wrapping_sub(even_distance).wrapping_add(1) | 1;
        let start = spatial_row_start(
          &rt.r,
          Point { x: bx4 | 1, y: previous_row.wrapping_add(5) },
        )?;
        let nearby_rows = scan_spatial_edge(
          &rf.r,
          NeighborEdge::Row { start },
          NeighborScan {
            block_span: bw4,
            visible_span: w4,
            available_perpendicular: max_rows.wrapping_add(1).wrapping_sub(n),
            step: match bw4 { 16.. => 4, _ => 2 },
          },
          CandidateSearch {
            candidates: mvstack,
            count: cnt,
            reference: r#ref,
            global: &gmv,
            matches: SpatialMatches {
              new_motion: &mut have_dummy_newmv_match,
              reference_motion: &mut have_row_mvs,
            },
          },
        )?;
        *scanned = scanned.wrapping_add(nearby_rows);
      }
      None => {}
    }

    match scanned_columns.as_mut().filter(|scanned| n > **scanned && n <= max_cols) {
      Some(scanned) => {
        let Ok(row_index) = console_core_number_conversion::index((local_row | 1).wrapping_add(5));
        let row_starts = rt.r.get(row_index..).ok_or(EINVAL)?;
        let Ok(neighbor_distance) = console_core_number_conversion::wrapped::<_, i32>(n);
        let x = bx4.wrapping_sub(neighbor_distance.wrapping_mul(2)).wrapping_add(1) | 1;
        let nearby_cols = scan_spatial_edge(
          &rf.r,
          NeighborEdge::Column { row_starts, x },
          NeighborScan {
            block_span: bh4,
            visible_span: h4,
            available_perpendicular: max_cols.wrapping_add(1).wrapping_sub(n),
            step: match bh4 { 16.. => 4, _ => 2 },
          },
          CandidateSearch {
            candidates: mvstack,
            count: cnt,
            reference: r#ref,
            global: &gmv,
            matches: SpatialMatches {
              new_motion: &mut have_dummy_newmv_match,
              reference_motion: &mut have_col_mvs,
            },
          },
        )?;
        *scanned = scanned.wrapping_add(nearby_cols);
      }
      None => {}
    }
  }

  let ref_match_count = i32::wrapping_add(have_col_mvs, have_row_mvs);

  let (refmv_ctx, newmv_ctx) = match nearest_match {
    0 => (cmp::min(2, ref_match_count), i32::from(ref_match_count > 0)),
    1 => (
      cmp::min(i32::wrapping_mul(ref_match_count, 3), 4),
      i32::wrapping_sub(3, have_newmv),
    ),
    2 => (5, i32::wrapping_sub(5, have_newmv)),
    _ => (0, 0),
  };

  let Ok(active_count) = console_core_number_conversion::index(*cnt);
  let active = mvstack.get_mut(..active_count).ok_or(EINVAL)?;
  let (nearest, later) = active.split_at_mut_checked(match ::console_core_number_conversion::index(nearest_count) { ::core::result::Result::Ok(native_position) => native_position }).ok_or(EINVAL)?;
  nearest.sort_by_key(|candidate| candidate.weight.wrapping_neg());
  later.sort_by_key(|candidate| candidate.weight.wrapping_neg());

  let Ok(motion_bounds) = ReferenceMotionBounds::new(
    block,
    Size { width: rf.iw4, height: rf.ih4 },
  );

  let extended_span = cmp::min(w4, h4);
  let extended_edges = [
    top_start.map(|start| NeighborEdge::Row { start }),
    left_rows.map(|row_starts| NeighborEdge::Column { row_starts, x: bx4.wrapping_sub(1) }),
  ];

  let fallback = MotionFallbackEdges {
    frame: rf,
    extent: extended_span,
    edges: extended_edges,
    references: r#ref,
    global: tgmv,
  };
  let contexts = MotionContexts { nearest: refmv_ctx, new_motion: newmv_ctx, global: globalmv_ctx };
  complete_motion_candidates(mvstack, cnt, ctx, contexts, fallback, motion_bounds)
}

struct MotionFallbackEdges<'a> {
  frame: &'a RefMvsFrame,
  extent: i32,
  edges: [Option<NeighborEdge<'a>>; 2],
  references: RefMvsRefPair,
  global: [Mv; 2],
}

struct MotionContexts {
  nearest: i32,
  new_motion: i32,
  global: i32,
}

fn complete_motion_candidates(
  mvstack: &mut [RefMvsCandidate; 8],
  count: &mut u8,
  context: &mut i32,
  motion: MotionContexts,
  fallback: MotionFallbackEdges<'_>,
  bounds: ReferenceMotionBounds,
) -> Rav1dResult<u8> {
  let [primary, secondary] = fallback.references.r#ref;

  match secondary {
    1.. => {
      complete_compound_motion(mvstack, count, &fallback)?;
      let Ok(active) = console_core_number_conversion::index(*count);
      let candidates = mvstack.get_mut(..active).ok_or(EINVAL)?;

      for candidate in candidates {
        let [first, second] = &mut candidate.mv.mv;
        let Ok(()) = bounds.clamp(first);
        let Ok(()) = bounds.clamp(second);
      }

      *context = match motion.nearest.wrapping_shr(1) {
        0 => cmp::min(motion.new_motion, 1),
        1 => 1_i32.wrapping_add(cmp::min(motion.new_motion, 3)),
        2 => iclip(3_i32.wrapping_add(motion.new_motion), 4, 7),
        _ => *context,
      };
    }
    ..=0 => {
      match (*count, primary) {
        (0..=1, 1..) => {
          let sign = reference_sign(primary, &fallback.frame.sign_bias)?;
          let mut state = (&mut *mvstack, &mut *count);
          scan_extended_edges(
            &fallback.frame.r,
            &fallback.edges,
            fallback.extent,
            &mut state,
            |neighbor, (stack, written)| {
              add_single_extended_candidate(stack, written, neighbor, sign, &fallback.frame.sign_bias)?;

              match **written {
                0 | 1 => Ok(ExtendedScan::Continue),
                _ => Ok(ExtendedScan::Complete),
              }
            },
          )?;
        }
        _ => {}
      }

      let Ok(active) = console_core_number_conversion::index(*count);
      let candidates = mvstack.get_mut(..active).ok_or(EINVAL)?;

      for candidate in candidates {
        let [vector, _] = &mut candidate.mv.mv;
        let Ok(()) = bounds.clamp(vector);
      }

      let [global, _] = fallback.global;
      let Ok(available) = console_core_number_conversion::index(cmp::min(*count, 2));
      let missing = mvstack.get_mut(available..2).ok_or(EINVAL)?;

      for candidate in missing {
        let [vector, _] = &mut candidate.mv.mv;
        *vector = global;
      }

      *context = motion.nearest.wrapping_shl(4) | motion.global.wrapping_shl(3) | motion.new_motion;
    }
  }

  Ok(*count)
}

fn complete_compound_motion(
  stack: &mut [RefMvsCandidate; 8],
  count: &mut u8,
  fallback: &MotionFallbackEdges<'_>,
) -> Rav1dResult {
  match *count {
    0..=1 => {}
    2.. => return Ok(()),
  }

  let [primary, secondary] = fallback.references.r#ref;
  let sign_bias = &fallback.frame.sign_bias;
  let primary_sign = reference_sign(primary, sign_bias)?;
  let secondary_sign = reference_sign(secondary, sign_bias)?;
  let Ok(occupied) = console_core_number_conversion::index(*count);
  let vacancies = stack.get_mut(occupied..).ok_or(EINVAL)?;
  let mut state = (vacancies, [0_u8; 4]);
  scan_extended_edges(&fallback.frame.r, &fallback.edges, fallback.extent, &mut state, |neighbor, (same, counts)| {
    add_compound_extended_candidate(
      same, counts, neighbor, [primary_sign, secondary_sign], fallback.references, sign_bias,
    )?;
    Ok(ExtendedScan::Continue)
  })?;
  fill_compound_candidates(state.0, state.1, fallback.global)?;

  let [first, second, third, ..] = stack;

  match *count {
    0 => {
      first.weight = 2;
      second.weight = 2;
    }
    1 => {
      match (first.mv == second.mv).then_some(()) {
        Some(()) => second.mv = third.mv,
        None => {}
      }

      second.weight = 2;
    }
    _ => return Err(EINVAL),
  }

  *count = 2;
  Ok(())
}


pub(crate) struct SpatialMvRegion {
  pub columns: RefmvsTileRange,
  pub rows: RefmvsTileRange,
}

pub(crate) enum MotionRowPass {
  Primary,
  Second,
}

pub(crate) struct MotionTileRow {
  pub superblock_row: i32,
  pub tile_row: i32,
  pub pass: MotionRowPass,
}

const NO_SPATIAL_ROW: u64 = u64::MAX;

pub(crate) fn rav1d_refmvs_tile_sbrow_init(
  frame: &RefMvsFrame,
  region: SpatialMvRegion,
  row: MotionTileRow,
) -> Rav1dResult<RefmvsTile> {
  let tile_row = match frame.n_tile_threads {
    1 => 0,
    _ => u32::try_from(row.tile_row).map_err(|_| EINVAL)?,
  };
  let depth = match frame.sbsz {
    16 => 16_u32,
    32 => 32_u32,
    _ => return Err(EINVAL),
  };
  let superblock_row = u32::try_from(row.superblock_row).map_err(|_| EINVAL)?;
  let stride = u64::from(frame.rp_stride);
  let spatial_stride = ::core::primitive::u64::wrapping_mul(stride, 2);
  let pass_base = match (frame.n_frame_threads, row.pass) {
    (2.., MotionRowPass::Second) => ::core::primitive::u64::wrapping_mul(u64::from(frame.n_blocks), 70),
    _ => 0,
  };
  let row_base = ::core::primitive::u64::wrapping_mul(u64::from(tile_row), 35)
    .checked_mul(spatial_stride)
    .and_then(|tile_base| tile_base.checked_add(pass_base))
    .ok_or(EINVAL)?;
  let projected_base = ::core::primitive::u64::wrapping_mul(u64::from(tile_row), 16).checked_mul(stride).ok_or(EINVAL)?;
  console_core_number_conversion::checked_index(projected_base).map_err(|_| EINVAL)?;
  let alternate_row = ::core::primitive::u32::wrapping_rem(superblock_row, 2) != 0;
  let page_start = match (depth, alternate_row) {
    (16, true) => 16,
    _ => 0,
  };
  let mut r = [NO_SPATIAL_ROW; 37];
  let Ok(page_index) = console_core_number_conversion::index(page_start);
  let page = r.get_mut(page_index..).ok_or(EINVAL)?;

  for (slot, offset) in (0_u32..).zip(page) {
    let logical_slot = match (alternate_row, slot, slot.checked_sub(depth)) {
      (true, 0 | 2 | 4, _) => slot.wrapping_add(depth),
      (true, _, Some(previous @ (0 | 2 | 4))) => previous,
      _ => slot,
    };
    let active_row = logical_slot.checked_sub(5).filter(|row_2| *row_2 < depth);
    let buffer_row = match (logical_slot, active_row) {
      (0, _) => depth,
      (2, _) => depth.wrapping_add(1),
      (4, _) => depth.wrapping_add(2),
      (_, Some(row_3)) => row_3,
      _ => continue,
    };
    let address = row_base.checked_add(u64::from(buffer_row).wrapping_mul(spatial_stride)).ok_or(EINVAL)?;
    console_core_number_conversion::checked_index(address).map_err(|_| EINVAL)?;
    *offset = address;
  }

  Ok(RefmvsTile {
    r,
    rp_proj: projected_base,
    tile_col: RefmvsTileRange {
      start: region.columns.start,
      end: region.columns.end.min(frame.iw4),
    },
    tile_row: RefmvsTileRange {
      start: region.rows.start,
      end: region.rows.end.min(frame.ih4),
    },
  })
}

struct ProjectedMotion<'a> {
  cells: &'a DisjointMut<AlignedVec64<RefMvsTemporalBlock>>,
  region: TemporalMvRegion,
  tile_base: u64,
  stride: u64,
  length: u64,
}

impl ProjectedMotion<'_> {
  fn offset(&self, point: Point<i32>) -> Rav1dResult<u64> {
    let column = u32::try_from(point.x).map_err(|_negative_column| EINVAL)?;
    let row = u64::from((point.y & 15).cast_unsigned()).wrapping_mul(self.stride);

    self.tile_base
      .checked_add(row)
      .and_then(|row_base| row_base.checked_add(u64::from(column)))
      .ok_or(EINVAL)
  }

  fn clear(&self) -> Rav1dResult {
    for y in self.region.rows.clone() {
      let start = self.offset(Point { x: self.region.columns.start, y })?;
      let end = self.offset(Point { x: self.region.columns.end, y })?;
      let span = checked_motion_range(start..end, self.length)?;
      let Ok(range) = console_core_number_conversion::indices(span);

      for cell in &mut *self.cells.index_mut(range) {
        let Ok(cleared) = RefMvsTemporalBlock::new(Mv::INVALID, cell.r#ref);
        *cell = cleared;
      }
    }

    Ok(())
  }

  fn paint(&self, position: Point<i32>, block: RefMvsTemporalBlock) -> Rav1dResult {
    let address = self.offset(position)?;

    match address.cmp(&self.length) {
      cmp::Ordering::Less => {
        let Ok(index) = console_core_number_conversion::index(address);
        *self.cells.index_mut(index) = block;

        Ok(())
      }
      cmp::Ordering::Equal | cmp::Ordering::Greater => Err(EINVAL),
    }
  }
}

#[derive(Clone, Copy)]
struct TemporalProjection {
  displacement: Point<i32>,
  block: RefMvsTemporalBlock,
}

fn load_tmvs_rust(
  frame: &RefMvsFrame,
  motion: &[Option<DisjointMutArcSlice<RefMvsTemporalBlock>>; 7],
  tile_row: i32,
  TemporalMvRegion { columns, rows }: TemporalMvRegion,
) -> Rav1dResult {
  let projected_tile = match frame.n_tile_threads {
    1 => 0,
    _worker_count => u32::try_from(tile_row).map_err(|_negative_tile_row| EINVAL)?,
  };

  let stride = u64::from(frame.rp_stride);
  let tile_rows = u64::from(projected_tile).wrapping_mul(16);
  let tile_base = tile_rows.checked_mul(stride).ok_or(EINVAL)?;
  let Ok(length) = console_core_number_conversion::fixed_index(frame.rp_proj.len());
  let scan_columns = columns.start.saturating_sub(8).max(0)..columns.end.saturating_add(8).min(frame.iw8);
  let plane = ProjectedMotion {
    cells: &frame.rp_proj,
    region: TemporalMvRegion { columns, rows: rows.start..rows.end.min(frame.ih8) },
    tile_base,
    stride,
    length,
  };
  plane.clear()?;

  match scan_columns.is_empty().then_some(()) {
    Some(()) => return Ok(()),
    None => {}
  }

  for candidate in frame.temporal_references.iter().flatten() {
    let distance = match candidate.current_distance {
      Some(distance) => distance,
      None => continue,
    };
    let Ok(reference_index) = console_core_number_conversion::index(candidate.source);
    let reference = motion.get(reference_index).and_then(Option::as_ref).ok_or(EINVAL)?;
    project_temporal_rows(&plane, &reference.inner, &scan_columns, candidate, distance)?;
  }

  Ok(())
}

fn project_temporal_rows(
  plane: &ProjectedMotion<'_>,
  reference: &DisjointMutSlice<RefMvsTemporalBlock>,
  scan_columns: &Range<i32>,
  candidate: &TemporalReference,
  distance: i32,
) -> Rav1dResult {
  let Ok(reference_len) = console_core_number_conversion::fixed_index(reference.len());
  let scan_start = u64::from(scan_columns.start.cast_unsigned());
  let scan_end = u64::from(scan_columns.end.cast_unsigned());

  for y in plane.region.rows.clone() {
    let base = u64::from(y.cast_unsigned()).wrapping_mul(plane.stride);
    let row_start = base.checked_add(scan_start).ok_or(EINVAL)?;
    let row_end = base.checked_add(scan_end).ok_or(EINVAL)?;
    let addresses = checked_motion_range(row_start..row_end, reference_len)?;
    let row_band = y & !7;
    let allowed_rows = row_band.max(plane.region.rows.start)
      ..row_band.saturating_add(8).min(plane.region.rows.end);
    let mut cached_block = RefMvsTemporalBlock::default();
    let mut cached_projection = None;

    'scan_columns: for (x, address) in scan_columns.clone().zip(addresses) {
      let Ok(position) = console_core_number_conversion::index(address);
      let block = *reference.index(position);
      let projection = match (block == cached_block).then_some(cached_projection) {
        Some(previous) => previous,
        None => {
          let updated = project_temporal_block(block, candidate, distance)?;
          cached_block = block;
          cached_projection = updated;

          updated
        }
      };
      let TemporalProjection { displacement, block: translated } = match projection {
        Some(projected) => projected,
        None => continue 'scan_columns,
      };
      let position_2 = Point { x: x.wrapping_add(displacement.x), y: y.wrapping_add(displacement.y) };
      let column_band = x & !7;
      let allowed_columns = column_band.saturating_sub(8).max(plane.region.columns.start)
        ..column_band.saturating_add(16).min(plane.region.columns.end);

      match (allowed_rows.contains(&position_2.y) && allowed_columns.contains(&position_2.x)).then_some(position_2) {
        Some(inside_tile) => plane.paint(inside_tile, translated)?,
        None => {}
      }
    }
  }

  Ok(())
}

fn project_temporal_block(
  block: RefMvsTemporalBlock,
  candidate: &TemporalReference,
  distance: i32,
) -> Rav1dResult<Option<TemporalProjection>> {
  let reference = match block.r#ref {
    0 => return Ok(None),
    reference @ 1..=7 => reference.unsigned_abs().wrapping_sub(1),
    _invalid_reference => return Err(EINVAL),
  };
  let Ok(slot) = console_core_number_conversion::index(reference);
  let distance_slot = candidate.other_distances.get(slot).ok_or(EINVAL)?;
  let other_distance = match distance_slot {
    Some(nonzero) => *nonzero,
    None => return Ok(None),
  };
  let projection = MotionProjection { destination_distance: distance, source_distance: other_distance };
  let Ok(vector) = block.vector();
  let offset = mv_projection(vector, projection)?;
  let sign = i32::from(candidate.source).wrapping_sub(4);
  let offset_x = i32::from(offset.x);
  let offset_y = i32::from(offset.y);
  let displacement = Point {
    x: apply_sign(offset_x.abs().wrapping_shr(6), offset_x ^ sign),
    y: apply_sign(offset_y.abs().wrapping_shr(6), offset_y ^ sign),
  };
  let Ok(r#ref) = console_core_number_conversion::wrapped::<_, i8>(other_distance.get());

  Ok(Some(TemporalProjection { displacement, block: RefMvsTemporalBlock { r#ref, ..block } }))
}

fn checked_motion_range(span: Range<u64>, length: u64) -> Rav1dResult<Range<u64>> {
  match (span.start.cmp(&span.end), span.end.cmp(&length)) {
    (cmp::Ordering::Greater, _) | (_, cmp::Ordering::Greater) => Err(EINVAL),
    _inside_plane => Ok(span),
  }
}

fn save_tmvs_rust(
  frame: &RefMvsFrame,
  row_offsets: &[u64; 31],
  output: &DisjointMutSlice<RefMvsTemporalBlock>,
  TemporalMvRegion { columns, rows }: TemporalMvRegion,
) -> Rav1dResult {
  let clipped_columns = columns.start..columns.end.min(frame.iw8);
  let clipped_rows = rows.start..rows.end.min(frame.ih8);
  let Ok(spatial_length) = console_core_number_conversion::fixed_index(frame.r.len());
  let Ok(temporal_length) = console_core_number_conversion::fixed_index(output.len());
  let stride = u64::from(frame.rp_stride);

  for y in clipped_rows {
    let row_number = y.cast_unsigned();
    let row_slot = (row_number & 15).wrapping_mul(2);
    let Ok(row_index) = console_core_number_conversion::index(row_slot);
    let spatial_base = row_offsets.get(row_index).copied().ok_or(EINVAL)?;
    let temporal_base = u64::from(row_number).wrapping_mul(stride);
    let mut x = clipped_columns.start;

    'save_columns: for _column in clipped_columns.clone() {
      let column = x.cast_unsigned();
      let block_index = spatial_base
        .checked_add(u64::from(column).wrapping_mul(2).wrapping_add(1))
        .ok_or(EINVAL)?;

      match block_index.cmp(&spatial_length) {
        cmp::Ordering::Less => {}
        cmp::Ordering::Equal | cmp::Ordering::Greater => return Err(EINVAL),
      }

      let Ok(native_block) = console_core_number_conversion::index(block_index);
      let candidate = *frame.r.index(native_block);
      let Ok(&[width4, ..]) = candidate.bs.dimensions();
      let width = width4.div_ceil(2);
      let selected = select_temporal_block(candidate, &frame.mfmv_sign)?;
      let offset = temporal_base.checked_add(u64::from(column)).ok_or(EINVAL)?;
      let end = offset.checked_add(u64::from(width)).ok_or(EINVAL)?;

      let span = checked_motion_range(offset..end, temporal_length)?;
      let Ok(range) = console_core_number_conversion::indices(span);
      output.index_mut(range).fill(selected);
      let next_column = x.checked_add(i32::from(width)).ok_or(EINVAL)?;
      x = next_column;

      match x.cmp(&clipped_columns.end) {
        cmp::Ordering::Less => {}
        cmp::Ordering::Equal | cmp::Ordering::Greater => break 'save_columns,
      }
    }
  }

  Ok(())
}

fn select_temporal_block(block: RefMvsBlock, signs: &[u8; 7]) -> Rav1dResult<RefMvsTemporalBlock> {
  let [first, second] = block.mv.mv;
  let [first_reference, second_reference] = block.r#ref.r#ref;

  for (mv, reference) in [(second, second_reference), (first, first_reference)] {
    match reference {
      ..=0 => continue,
      1.. => {}
    }

    let reference_slot = reference.unsigned_abs().wrapping_sub(1);
    let Ok(index) = console_core_number_conversion::index(reference_slot);
    let sign = signs.get(index).ok_or(EINVAL)?;
    let magnitude = mv.y.unsigned_abs() | mv.x.unsigned_abs();

    match ::core::primitive::bool::then_some(*sign != 0 && magnitude < 4096, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(candidate) = RefMvsTemporalBlock::new(mv, reference);
        return Ok(candidate);
      }
      ::core::option::Option::None => {}
    }
  }

  Ok(RefMvsTemporalBlock::default())
}

pub(crate) struct RefMvsParallelism {
  pub tile_threads: u32,
  pub frame_threads: u32,
}

pub(crate) fn rav1d_refmvs_init_frame(
  rf: &mut RefMvsFrame,
  seq_hdr: &Rav1dSequenceHeader,
  frm_hdr: &Rav1dFrameHeader,
  ref_poc: &[u32; 7],
  ref_ref_poc: &[[u32; 7]; 7],
  rp_ref: &[Option<DisjointMutArcSlice<RefMvsTemporalBlock>>; 7],
  parallelism: RefMvsParallelism,
) -> Rav1dResult {
  let RefMvsParallelism { tile_threads: n_tile_threads, frame_threads: n_frame_threads } = parallelism;
  let [frame_width, _output_width] = frm_hdr.size.width;
  let width = u32::try_from(frame_width).map_err(|_| EINVAL)?;
  let height = u32::try_from(frm_hdr.size.height).map_err(|_| EINVAL)?;
  let padded_width = width.checked_next_multiple_of(128).ok_or(EINVAL)?;
  let rp_stride = ::core::primitive::u32::wrapping_shr(padded_width, 3_u32);
  let tile_rows = match n_tile_threads {
    0..=1 => 1,
    _ => u32::from(frm_hdr.tiling.rows),
  };
  let n_blocks = rp_stride.checked_mul(tile_rows).ok_or(ENOMEM)?;
  let sbsz = match seq_hdr.sb128 {
    0 => 16,
    1 => 32,
    _ => return Err(EINVAL),
  };
  let checked_value = width.checked_add(7).ok_or(EINVAL)?;
  let width_eight = ::core::primitive::u32::wrapping_shr(checked_value, 3_u32);
  let checked_value_2 = height.checked_add(7).ok_or(EINVAL)?;
  let height_eight = ::core::primitive::u32::wrapping_shr(checked_value_2, 3_u32);
  let iw8 = i32::try_from(width_eight).map_err(|_| EINVAL)?;
  let ih8 = i32::try_from(height_eight).map_err(|_| EINVAL)?;
  let iw4 = iw8.checked_mul(2).ok_or(EINVAL)?;
  let ih4 = ih8.checked_mul(2).ok_or(EINVAL)?;
  let bits = seq_hdr.order_hint_n_bits;
  let current_poc = i32::from(frm_hdr.frame_offset);
  let mut references = [None; 3];

  match (frm_hdr.use_ref_frame_mvs, bits) {
    (0, _) | (_, 0) => {}
    (_, _) => fill_temporal_references(&mut references, current_poc, bits, ref_poc, ref_ref_poc, rp_ref)?,
  }

  match n_blocks.cmp(&rf.n_blocks) {
    cmp::Ordering::Equal => {}
    cmp::Ordering::Less | cmp::Ordering::Greater => {
      let passes = match n_frame_threads {
        0..=1 => 1,
        _ => 2,
      };
      let cells = u64::from(n_blocks).wrapping_mul(70).wrapping_mul(passes).wrapping_add(R_PAD);
      let projections = u64::from(n_blocks).wrapping_mul(16);
      let grid_size = console_core_number_conversion::checked_index(cells).map_err(|_| ENOMEM)?;
      let projected_size = console_core_number_conversion::checked_index(projections).map_err(|_| ENOMEM)?;
      rf.r.try_resize(grid_size, FromZeros::new_zeroed()).map_err(|_| ENOMEM)?;
      rf.rp_proj.try_resize(projected_size, Default::default()).map_err(|_| ENOMEM)?;
      rf.n_blocks = n_blocks;
    }
  }

  rf.sbsz = sbsz;
  rf.iw8 = iw8;
  rf.ih8 = ih8;
  rf.iw4 = iw4;
  rf.ih4 = ih4;
  rf.rp_stride = rp_stride;
  rf.n_tile_threads = n_tile_threads;
  rf.n_frame_threads = n_frame_threads;
  rf.temporal_references = references;

  for (((sign_bias, motion_sign), picture_distance), reference_poc) in rf
    .sign_bias
    .iter_mut()
    .zip(&mut rf.mfmv_sign)
    .zip(&mut rf.pocdiff)
    .zip(ref_poc)
  {
    let reference_order = reference_poc.cast_signed();
    let Ok(forward) = get_poc_diff(bits, OrderHintComparison { target: reference_order, origin: current_poc });
    let Ok(backward) = get_poc_diff(bits, OrderHintComparison { target: current_poc, origin: reference_order });
    *sign_bias = u8::from(forward > 0);
    *motion_sign = u8::from(forward < 0);
    let Ok(distance) = console_core_number_conversion::wrapped::<_, i8>(backward.clamp(-31, 31));
    *picture_distance = distance;
  }

  Ok(())
}

fn fill_temporal_references(
  temporal_references: &mut [Option<TemporalReference>; 3],
  current_poc: i32,
  bits: u8,
  ref_poc: &[u32; 7],
  ref_ref_poc: &[[u32; 7]; 7],
  motion: &[Option<DisjointMutArcSlice<RefMvsTemporalBlock>>; 7],
) -> Rav1dResult {
  let [last_buffer, ..] = motion;
  let [[.., altref_of_last], ..] = ref_ref_poc;
  let [_, _, _, golden_poc, ..] = ref_poc;
  let use_last = last_buffer.is_some() && altref_of_last != golden_poc;
  let mut slots = match use_last.then_some(()) {
    Some(()) => temporal_references.iter_mut().take(3),
    None => temporal_references.iter_mut().take(2),
  };

  for source in [0_u8, 4, 5, 6, 1] {
    let Ok(index) = console_core_number_conversion::index(source);
    let motion_buffer = motion.get(index).ok_or(EINVAL)?;
    let picture_order_bits = ref_poc.get(index).ok_or(EINVAL)?;
    let picture_order = picture_order_bits.cast_signed();
    let Ok(distance) = get_poc_diff(bits, OrderHintComparison { target: picture_order, origin: current_poc });
    let is_eligible = match source {
      0 => use_last,
      1 => motion_buffer.is_some(),
      4..=6 => motion_buffer.is_some() && distance > 0,
      _invalid_slot => return Err(EINVAL),
    };

    match ::core::primitive::bool::then_some(is_eligible, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => continue,
    }

    let slot = match slots.next() {
      Some(next) => next,
      None => break,
    };
    let mut other_distances = [None; 7];
    let current_distance = match distance {
      -31..=31 => {
        let other_pocs = ref_ref_poc.get(index).ok_or(EINVAL)?;

        let Ok(distances) = temporal_reference_distances(bits, picture_order, other_pocs);
        other_distances = distances;

        Some(match source {
          0..=3 => distance.wrapping_neg(),
          4.. => distance,
        })
      }
      _outside_usable_range => None,
    };
    *slot = Some(TemporalReference { source, current_distance, other_distances });
  }

  Ok(())
}

fn temporal_reference_distances(
  bits: u8,
  reference_order: i32,
  other_pocs: &[u32; 7],
) -> Result<[Option<NonZeroU8>; 7], console_core_never::Never> {
  Ok(other_pocs.map(|poc| {
    let Ok(delta) = get_poc_diff(bits, OrderHintComparison { target: reference_order, origin: poc.cast_signed() });

    match delta {
      1..=31 => {
        let Ok(value) = console_core_number_conversion::wrapped::<_, u8>(delta);
        NonZeroU8::new(value)
      }
      _outside_usable_range => None,
    }
  }))
}

impl Rav1dRefmvsDSPContext {
  pub const fn default() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        load_tmvs: load_tmvs::Fn::DEFAULT,
        save_tmvs: save_tmvs::Fn::DEFAULT,
        splat_mv: splat_mv::Fn::DEFAULT,
      }
    })
  }

  #[inline(always)]
  const fn init(self, flags: CpuFlags) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      {
        let _ = flags;
        self
      }
    })
  }

  pub const fn new(flags: CpuFlags) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let Ok(rav1d_refmvs_dsp_context) = Self::default();
      let Ok(init) = rav1d_refmvs_dsp_context.init(flags);

      init
    })
  }
}

impl Default for Rav1dRefmvsDSPContext {
  fn default() -> Self {
    let Ok(rav1d_refmvs_dsp_context) = Self::default();

    rav1d_refmvs_dsp_context
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use aligned::Aligned;

  #[test]
  fn splat_checks_all_rows_before_writing_and_skips_unavailable_rows() -> Rav1dResult {
    let mut frame = RefMvsFrame::default();
    frame.r.try_resize(33, RefMvsBlock::new_zeroed()).map_err(|_| ENOMEM)?;
    let mut tile = RefmvsTile::default();
    let [_, _, _, _, _, first, second, absent, ..] = &mut tile.r;
    *first = 0;
    *second = 30;
    *absent = 32;
    let origin = Bxy { x: 2, y: 32 };
    let extent = Size { width: 3, height: 3 };
    let template = Aligned(RefMvsBlock { mf: 5, ..RefMvsBlock::new_zeroed() });
    let splat = splat_mv::Fn::DEFAULT;

    assert_eq!(splat.call(&frame, &tile, &template, origin, extent), Err(EINVAL));
    assert!(frame.r.index(..).iter().all(|block| block.mf == 0));

    let [_, _, _, _, _, _, middle_row, ..] = &mut tile.r;
    *middle_row = 8;
    splat.call(&frame, &tile, &template, origin, extent)?;

    for (block, ordinal) in frame.r.index(..).iter().zip(0_u32..) {
      let expected = match ordinal { 2..=4 | 10..=12 => 5, _ => 0 };
      assert_eq!(block.mf, expected);
    }

    Ok(())
  }
}
