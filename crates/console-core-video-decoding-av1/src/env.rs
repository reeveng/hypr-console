#![forbid(unsafe_code)]
//! Context selection from decoded neighbors and reference motion.
use crate::include::common::intops::apply_sign;
use crate::include::dav1d::headers::Rav1dFilterMode;
use crate::include::dav1d::headers::Rav1dFrameHeader;
use crate::include::dav1d::headers::Rav1dWarpedMotionParams;
use crate::include::dav1d::headers::Rav1dWarpedMotionType;
use crate::src::align::Align8;
use crate::src::disjoint_mut::DisjointMut;
use crate::src::disjoint_mut::DisjointMutSlice;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use crate::src::internal::Bxy;
use crate::src::levels::BlockLevel;
use crate::src::levels::CompInterType;
use crate::src::levels::DCT_DCT;
use crate::src::levels::H_ADST;
use crate::src::levels::H_FLIPADST;
use crate::src::levels::IDTX;
use crate::src::levels::Mv;
use crate::src::levels::SegmentId;
use crate::src::levels::TxfmSize;
use crate::src::levels::TxfmType;
use crate::src::levels::V_ADST;
use crate::src::levels::V_FLIPADST;
use crate::src::refmvs::RefMvsCandidate;
use crate::src::tables::TxfmInfo;
use console_core_geometry::Rectangle;
use std::cmp::Ordering;
use std::ffi::c_int;

#[derive(Clone, Copy)]
pub(crate) enum NeighborAvailability {
  Neither,
  Top,
  Left,
  Both,
}

impl From<(bool, bool)> for NeighborAvailability {
  fn from((top, left): (bool, bool)) -> Self {
    match (top.then_some(()), left.then_some(())) {
      (Some(()), Some(())) => Self::Both,
      (Some(()), None) => Self::Top,
      (None, Some(())) => Self::Left,
      (None, None) => Self::Neither,
    }
  }
}

#[derive(Clone, Copy)]
pub(crate) enum ReferenceArity {
  Single,
  Compound,
}

impl ReferenceArity {
  #[must_use]
  const fn filter_context_offset(self) -> Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::Single => 0,
      Self::Compound => 4,
    })
  }
}

#[derive(Clone, Copy)]
pub(crate) enum FilterAxis {
  Vertical,
  Horizontal,
}

impl FilterAxis {
  fn select<T>(self, [vertical, horizontal]: &[T; 2]) -> Result<&T, console_core_never::Never> {
    Ok(match self {
      Self::Vertical => vertical,
      Self::Horizontal => horizontal,
    })
  }
}

#[derive(Default)]
pub struct BlockContext {
  pub mode: DisjointMut<Align8<[u8; 32]>>,
  pub lcoef: DisjointMut<Align8<[u8; 32]>>,
  pub ccoef: [DisjointMut<Align8<[u8; 32]>>; 2],
  pub seg_pred: DisjointMut<Align8<[u8; 32]>>,
  pub skip: DisjointMut<Align8<[u8; 32]>>,
  pub skip_mode: DisjointMut<Align8<[u8; 32]>>,
  pub intra: DisjointMut<Align8<[u8; 32]>>,
  pub comp_type: DisjointMut<Align8<[Option<CompInterType>; 32]>>,
  pub r#ref: [DisjointMut<Align8<[i8; 32]>>; 2],

  pub filter: [DisjointMut<Align8<[Rav1dFilterMode; 32]>>; 2],

  pub tx_intra: DisjointMut<Align8<[i8; 32]>>,
  pub tx: DisjointMut<Align8<[TxfmSize; 32]>>,
  pub tx_lpf_y: DisjointMut<Align8<[u8; 32]>>,
  pub tx_lpf_uv: DisjointMut<Align8<[u8; 32]>>,
  pub partition: DisjointMut<Align8<[u8; 16]>>,
  pub uvmode: DisjointMut<Align8<[u8; 32]>>,
  pub pal_sz: DisjointMut<Align8<[u8; 32]>>,
}

pub(crate) fn read_context<T: Copy, const N: usize>(
  storage: &DisjointMut<Align8<[T; N]>>,
  coordinate: i32,
) -> Rav1dResult<T> {
  let position = u8::try_from(coordinate).map_err(|_invalid_coordinate| EINVAL)?;
  let Ok(index) = console_core_number_conversion::index(position);

  match (index < N).then_some(()) {
    Some(()) => Ok(*storage.index(index)),
    None => Err(EINVAL),
  }
}

#[inline]
pub(crate) fn get_intra_ctx(neighbors: BlockNeighbors<'_>) -> Rav1dResult<u8> {
  match neighbors.availability {
    NeighborAvailability::Neither => Ok(0),
    NeighborAvailability::Top => {
      let sample = read_context(&neighbors.above.intra, neighbors.location.x)?;
      Ok(sample.wrapping_mul(2))
    }
    NeighborAvailability::Left => {
      let sample = read_context(&neighbors.left.intra, neighbors.location.y)?;
      Ok(sample.wrapping_mul(2))
    }
    NeighborAvailability::Both => {
      let above = read_context(&neighbors.above.intra, neighbors.location.x)?;
      let left = read_context(&neighbors.left.intra, neighbors.location.y)?;
      let sum = above.wrapping_add(left);
      Ok(sum.wrapping_add(u8::from(sum == 2)))
    }
  }
}

#[inline]
pub(crate) fn get_tx_ctx(neighbors: BlockNeighbors<'_>, max_tx: &TxfmInfo) -> Rav1dResult<u8> {
  let above = read_context(&neighbors.above.tx_intra, neighbors.location.x)?;
  let left = read_context(&neighbors.left.tx_intra, neighbors.location.y)?;
  Ok(
    u8::from(i32::from(above) >= i32::from(max_tx.lw))
      .wrapping_add(u8::from(i32::from(left) >= i32::from(max_tx.lh))),
  )
}

#[inline]
pub(crate) fn get_partition_ctx(
  neighbors: BlockNeighbors<'_>,
  level: BlockLevel,
) -> Rav1dResult<u8> {
  let shift = match level {
    BlockLevel::Bl128x128 => 4,
    BlockLevel::Bl64x64 => 3,
    BlockLevel::Bl32x32 => 2,
    BlockLevel::Bl16x16 => 1,
    BlockLevel::Bl8x8 => 0,
  };
  let above = read_context(&neighbors.above.partition, neighbors.location.x)?;
  let left = read_context(&neighbors.left.partition, neighbors.location.y)?;
  let above_bit = above.wrapping_shr(shift) & 1;
  let left_bit = left.wrapping_shr(shift) & 1;
  Ok(above_bit.wrapping_add(left_bit.wrapping_mul(2)))
}

#[derive(Clone, Copy)]
pub(crate) enum PartitionEdge {
  Top,
  Left,
}

#[inline]
pub(crate) fn gather_partition_probability(
  probabilities: &[u16; 16],
  level: BlockLevel,
  edge: PartitionEdge,
) -> Result<u32, console_core_never::Never> {
  let [
    none,
    horizontal,
    vertical,
    _,
    top_split,
    bottom_split,
    left_split,
    right_split,
    horizontal_four,
    ..,
  ] = *probabilities;
  let rectangles = match level {
    BlockLevel::Bl128x128 => 0,
    BlockLevel::Bl64x64 | BlockLevel::Bl32x32 | BlockLevel::Bl16x16 | BlockLevel::Bl8x8 => {
      i32::from(horizontal_four).wrapping_sub(i32::from(right_split))
    }
  };
  let total = match edge {
    PartitionEdge::Top => i32::from(horizontal)
      .wrapping_sub(i32::from(top_split))
      .wrapping_add(i32::from(bottom_split))
      .wrapping_add(rectangles),
    PartitionEdge::Left => i32::from(none)
      .wrapping_sub(i32::from(horizontal))
      .wrapping_add(i32::from(vertical).wrapping_sub(i32::from(left_split)))
      .wrapping_sub(rectangles),
  };
  console_core_number_conversion::wrapped(total)
}

#[inline]
pub fn get_uv_inter_txtp(
  uvt_dim: &TxfmInfo,
  ytxtp: TxfmType,
) -> core::result::Result<TxfmType, console_core_never::Never> {
  Ok({
    let Ok(tx_32x32) = TxfmSize::S32x32.ordinal();
    let Ok(tx_16x16) = TxfmSize::S16x16.ordinal();

    match ::core::primitive::bool::then_some(uvt_dim.max == tx_32x32, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(
          match ::core::primitive::bool::then_some(ytxtp == IDTX, ()) {
            ::core::option::Option::Some(()) => IDTX,
            ::core::option::Option::None => DCT_DCT,
          },
        );
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(
      uvt_dim.min == tx_16x16
        && (i32::wrapping_shl(1, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>({
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(ytxtp);
            numeric_value
          });
          numeric_value
        }) & (i32::wrapping_shl(1, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(H_FLIPADST);
          numeric_value
        }) | i32::wrapping_shl(1, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(V_FLIPADST);
          numeric_value
        }) | i32::wrapping_shl(1, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(H_ADST);
          numeric_value
        }) | i32::wrapping_shl(1, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(V_ADST);
          numeric_value
        })))
          != 0,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(DCT_DCT);
      }
      ::core::option::Option::None => {}
    }

    ytxtp
  })
}

fn filter_for_reference(
  block: &BlockContext,
  position: i32,
  axis: FilterAxis,
  wanted: i8,
) -> Rav1dResult<Rav1dFilterMode> {
  let [first_reference, second_reference] = &block.r#ref;
  let first = read_context(first_reference, position)?;
  let has_reference = match ::core::primitive::bool::then_some(first == wanted, ()) {
    ::core::option::Option::Some(()) => true,
    ::core::option::Option::None => {
      let second = read_context(second_reference, position)?;
      second == wanted
    }
  };

  match ::core::primitive::bool::then_some(has_reference, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(storage) = axis.select(&block.filter);
      read_context(storage, position)
    }
    ::core::option::Option::None => Ok(Rav1dFilterMode::N_SWITCHABLE_FILTERS),
  }
}

#[inline]
pub(crate) fn get_filter_ctx(
  neighbors: BlockNeighbors<'_>,
  references: ReferenceArity,
  axis: FilterAxis,
  reference: i8,
) -> Rav1dResult<u8> {
  let top = filter_for_reference(neighbors.above, neighbors.location.x, axis, reference)?;
  let left = filter_for_reference(neighbors.left, neighbors.location.y, axis, reference)?;
  let selected = match [top, left] {
    [Rav1dFilterMode::N_SWITCHABLE_FILTERS, known]
    | [known, Rav1dFilterMode::N_SWITCHABLE_FILTERS] => known,
    [first, second] => match (first == second).then_some(first) {
      Some(known) => known,
      None => Rav1dFilterMode::N_SWITCHABLE_FILTERS,
    },
  };
  let filter = crate::include::dav1d::headers::Dav1dFilterMode::from(selected);
  let Ok(ordinal) = console_core_number_conversion::wrapped::<_, u8>(filter);
  let Ok(offset) = references.filter_context_offset();
  Ok(offset.wrapping_add(ordinal))
}

#[derive(Clone, Copy)]
enum NeighborArity {
  Single { reference: i8 },
  Compound,
}

impl NeighborArity {
  fn read(block: &BlockContext, position: i32) -> Rav1dResult<Self> {
    let compound_type = read_context(&block.comp_type, position)?;

    match compound_type {
      Some(_mode) => Ok(Self::Compound),
      None => {
        let [first, _second] = &block.r#ref;
        let reference = read_context(first, position)?;
        Ok(Self::Single { reference })
      }
    }
  }

  fn score(self) -> Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::Single { reference } => u8::from(reference >= 4),
      Self::Compound => 3,
    })
  }
}

#[inline]
pub(crate) fn get_comp_ctx(neighbors: BlockNeighbors<'_>) -> Rav1dResult<u8> {
  use NeighborArity::{Compound, Single};

  match neighbors.availability {
    NeighborAvailability::Neither => Ok(1),
    NeighborAvailability::Top => {
      let above = NeighborArity::read(neighbors.above, neighbors.location.x)?;
      let Ok(score) = above.score();
      Ok(score)
    }
    NeighborAvailability::Left => {
      let left = NeighborArity::read(neighbors.left, neighbors.location.y)?;
      let Ok(score) = left.score();
      Ok(score)
    }
    NeighborAvailability::Both => {
      let above = NeighborArity::read(neighbors.above, neighbors.location.x)?;
      let left = NeighborArity::read(neighbors.left, neighbors.location.y)?;
      Ok(match (above, left) {
        (Compound, Compound) => 4,
        (Single { reference }, Compound) | (Compound, Single { reference }) => {
          2u8.wrapping_add(u8::from(i32::from(reference).cast_unsigned() >= 4))
        }
        (Single { reference: top }, Single { reference: side }) => {
          u8::from((top >= 4) ^ (side >= 4))
        }
      })
    }
  }
}

#[derive(Clone, Copy)]
enum CompoundDirection {
  Bidirectional,
  Unidirectional,
}

impl CompoundDirection {
  fn from_refs([first, second]: [i8; 2]) -> Result<Self, console_core_never::Never> {
    Ok(match (first, second) {
      (i8::MIN..=3, i8::MIN..=3) | (4.., 4..) => Self::Unidirectional,
      _ => Self::Bidirectional,
    })
  }

  fn weight(self) -> Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::Bidirectional => 0,
      Self::Unidirectional => 1,
    })
  }
}

#[derive(Clone, Copy)]
enum NeighborPrediction {
  Intra,
  Single {
    reference: i8,
  },
  Compound {
    first: i8,
    direction: CompoundDirection,
  },
}

impl NeighborPrediction {
  fn read(block: &BlockContext, position: i32) -> Rav1dResult<Self> {
    let intra = read_context(&block.intra, position)?;

    match intra {
      0 => {}
      _ => return Ok(Self::Intra),
    }

    let [first_reference, second_reference] = &block.r#ref;
    let first = read_context(first_reference, position)?;
    let compound = read_context(&block.comp_type, position)?;

    match compound {
      Some(_mode) => {
        let second = read_context(second_reference, position)?;
        let Ok(direction) = CompoundDirection::from_refs([first, second]);
        Ok(Self::Compound { first, direction })
      }
      None => Ok(Self::Single { reference: first }),
    }
  }

  fn score(self) -> Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::Compound {
        direction: CompoundDirection::Bidirectional,
        ..
      } => 0,
      Self::Compound {
        direction: CompoundDirection::Unidirectional,
        ..
      } => 4,
      Self::Intra | Self::Single { .. } => 2,
    })
  }
}

fn pair_direction(
  [above, left]: [NeighborPrediction; 2],
) -> Result<u8, console_core_never::Never> {
  use CompoundDirection::{Bidirectional, Unidirectional};
  use NeighborPrediction::{Compound, Intra, Single};

  Ok(match (above, left) {
    (Intra, Intra) | (Intra, Single { .. }) | (Single { .. }, Intra) => 2,
    (Intra, Compound { direction, .. }) | (Compound { direction, .. }, Intra) => {
      let Ok(weight) = direction.weight();
      1u8.wrapping_add(weight.wrapping_mul(2))
    }
    (Single { reference: top }, Single { reference: side }) => {
      1u8.wrapping_add(2u8.wrapping_mul(u8::from((top >= 4) == (side >= 4))))
    }
    (Single { reference }, Compound { first, direction })
    | (Compound { first, direction }, Single { reference }) => match direction {
      Bidirectional => 1,
      Unidirectional => 3u8.wrapping_add(u8::from((first >= 4) == (reference >= 4))),
    },
    (
      Compound {
        direction: Bidirectional,
        ..
      },
      Compound {
        direction: Bidirectional,
        ..
      },
    ) => 0,
    (
      Compound {
        direction: Bidirectional,
        ..
      },
      Compound {
        direction: Unidirectional,
        ..
      },
    )
    | (
      Compound {
        direction: Unidirectional,
        ..
      },
      Compound {
        direction: Bidirectional,
        ..
      },
    ) => 2,
    (
      Compound {
        first: top,
        direction: Unidirectional,
      },
      Compound {
        first: side,
        direction: Unidirectional,
      },
    ) => 3u8.wrapping_add(u8::from((top == 4) == (side == 4))),
  })
}

#[inline]
pub(crate) fn get_comp_dir_ctx(neighbors: BlockNeighbors<'_>) -> Rav1dResult<u8> {
  match neighbors.availability {
    NeighborAvailability::Neither => Ok(2),
    NeighborAvailability::Top => {
      let above = NeighborPrediction::read(neighbors.above, neighbors.location.x)?;
      let Ok(score) = above.score();
      Ok(score)
    }
    NeighborAvailability::Left => {
      let left = NeighborPrediction::read(neighbors.left, neighbors.location.y)?;
      let Ok(score) = left.score();
      Ok(score)
    }
    NeighborAvailability::Both => {
      let above = NeighborPrediction::read(neighbors.above, neighbors.location.x)?;
      let left = NeighborPrediction::read(neighbors.left, neighbors.location.y)?;
      let Ok(score) = pair_direction([above, left]);
      Ok(score)
    }
  }
}

pub(crate) struct OrderHintComparison {
  pub(crate) target: i32,
  pub(crate) origin: i32,
}

#[inline]
pub(crate) fn get_poc_diff(
  order_hint_n_bits: u8,
  comparison: OrderHintComparison,
) -> Result<i32, console_core_never::Never> {
  match ::core::primitive::bool::then_some(order_hint_n_bits == 0, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(0);
    }
    ::core::option::Option::None => {}
  }

  let mask = i32::wrapping_shl(1, {
    let Ok(numeric_value) =
      console_core_number_conversion::wrapped::<_, u32>(u8::wrapping_sub(order_hint_n_bits, 1));
    numeric_value
  });
  let diff = comparison.target.wrapping_sub(comparison.origin);
  return Ok(i32::wrapping_sub(
    diff & i32::wrapping_sub(mask, 1),
    diff & mask,
  ));
}

struct CompoundReference {
  mode: Option<CompInterType>,
  reference: i8,
}

impl CompoundReference {
  fn read(block: &BlockContext, coordinate: i32) -> Rav1dResult<Self> {
    let mode = read_context(&block.comp_type, coordinate)?;
    let [first, _second] = &block.r#ref;
    let reference = read_context(first, coordinate)?;
    Ok(Self { mode, reference })
  }

  fn joint_weight(self) -> Result<u8, console_core_never::Never> {
    Ok(u8::from(
      self.mode >= Some(CompInterType::Avg) || self.reference == 6,
    ))
  }

  fn mask_weight(self) -> Result<u8, console_core_never::Never> {
    Ok(match self.mode.cmp(&Some(CompInterType::Seg)) {
      Ordering::Equal | Ordering::Greater => 1,
      Ordering::Less => match self.reference {
        6 => 3,
        _ => 0,
      },
    })
  }
}

pub(crate) struct CompoundOrderHints {
  pub width: u8,
  pub current: u32,
  pub reference_hints: [u32; 2],
}

#[inline]
pub(crate) fn get_jnt_comp_ctx(
  order: CompoundOrderHints,
  neighbors: BlockNeighbors<'_>,
) -> Rav1dResult<u8> {
  let [first_hint, second_hint] = order.reference_hints;
  let current = order.current.cast_signed();
  let Ok(first_offset) = get_poc_diff(
    order.width,
    OrderHintComparison {
      target: first_hint.cast_signed(),
      origin: current,
    },
  );
  let Ok(second_offset) = get_poc_diff(
    order.width,
    OrderHintComparison {
      target: current,
      origin: second_hint.cast_signed(),
    },
  );
  let same_distance = first_offset.unsigned_abs() == second_offset.unsigned_abs();
  let above = CompoundReference::read(neighbors.above, neighbors.location.x)?;
  let left = CompoundReference::read(neighbors.left, neighbors.location.y)?;
  let Ok(top_score) = above.joint_weight();
  let Ok(left_score) = left.joint_weight();
  Ok(
    u8::from(same_distance)
      .wrapping_mul(3)
      .wrapping_add(top_score)
      .wrapping_add(left_score),
  )
}

#[inline]
pub(crate) fn get_mask_comp_ctx(neighbors: BlockNeighbors<'_>) -> Rav1dResult<u8> {
  let above = CompoundReference::read(neighbors.above, neighbors.location.x)?;
  let left = CompoundReference::read(neighbors.left, neighbors.location.y)?;
  let Ok(top_score) = above.mask_weight();
  let Ok(left_score) = left.mask_weight();
  Ok(top_score.wrapping_add(left_score).min(5))
}

#[derive(Clone, Copy)]
pub(crate) struct BlockNeighbors<'a> {
  pub above: &'a BlockContext,
  pub left: &'a BlockContext,
  pub location: Bxy,
  pub availability: NeighborAvailability,
}

#[derive(Clone, Copy)]
pub(crate) enum ReferenceContext {
  Direction,
  ForwardGroup,
  NearForward,
  FarForward,
  BackwardGroup,
  NearBackward,
  UnidirectionalPair,
}

#[derive(Default)]
pub(crate) struct ReferenceCounts([u8; 7]);

impl ReferenceCounts {
  pub(crate) fn from_neighbors(neighbors: BlockNeighbors<'_>) -> Rav1dResult<Self> {
    let mut counts = Self::default();

    match neighbors.availability {
      NeighborAvailability::Top => counts.record_row(neighbors.above, neighbors.location.x)?,
      NeighborAvailability::Left => counts.record_row(neighbors.left, neighbors.location.y)?,
      NeighborAvailability::Both => {
        counts.record_row(neighbors.above, neighbors.location.x)?;
        counts.record_row(neighbors.left, neighbors.location.y)?;
      }
      NeighborAvailability::Neither => {}
    }

    Ok(counts)
  }

  fn record_row(&mut self, block: &BlockContext, location: i32) -> Rav1dResult {
    let coordinate = u8::try_from(location).map_err(|_invalid_coordinate| EINVAL)?;

    match coordinate {
      0..=31 => {}
      _ => return Err(EINVAL),
    }

    let Ok(position) = console_core_number_conversion::index(coordinate);

    match *block.intra.index(position) {
      0 => {}
      _ => return Ok(()),
    }

    let [first, second] = &block.r#ref;
    self.record(*first.index(position))?;

    match *block.comp_type.index(position) {
      Some(_compound) => self.record(*second.index(position)),
      None => Ok(()),
    }
  }

  fn record(&mut self, reference: i8) -> Rav1dResult {
    let slot = u8::try_from(reference).map_err(|_invalid_reference| EINVAL)?;
    let Ok(position) = console_core_number_conversion::index(slot);
    let count = self.0.get_mut(position).ok_or(EINVAL)?;
    *count = count.wrapping_add(1);
    Ok(())
  }

  pub(crate) fn context(
    &self,
    context: ReferenceContext,
  ) -> Result<u8, console_core_never::Never> {
    let [last, last2, last3, golden, backward, alternate2, alternate] = self.0;
    let [first, second] = match context {
      ReferenceContext::Direction => [
        last
          .wrapping_add(last2)
          .wrapping_add(last3)
          .wrapping_add(golden),
        backward.wrapping_add(alternate2).wrapping_add(alternate),
      ],
      ReferenceContext::ForwardGroup => [last.wrapping_add(last2), last3.wrapping_add(golden)],
      ReferenceContext::NearForward => [last, last2],
      ReferenceContext::FarForward => [last3, golden],
      ReferenceContext::BackwardGroup => [backward.wrapping_add(alternate2), alternate],
      ReferenceContext::NearBackward => [backward, alternate2],
      ReferenceContext::UnidirectionalPair => [last2, last3.wrapping_add(golden)],
    };

    Ok(match first.cmp(&second) {
      Ordering::Less => 0,
      Ordering::Equal => 1,
      Ordering::Greater => 2,
    })
  }
}

#[inline]
pub(crate) fn get_drl_context(
  ref_mv_stack: &[RefMvsCandidate; 8],
  ref_idx: u8,
) -> Rav1dResult<c_int> {
  let Ok(position) = console_core_number_conversion::index(ref_idx);
  let [first, second] = ref_mv_stack
    .get(position..)
    .and_then(<[_]>::first_chunk::<2>)
    .ok_or(EINVAL)?;

  Ok(match (first.weight >= 640, second.weight < 640) {
    (true, true) => 1,
    (false, true) => 2,
    (_, false) => 0,
  })
}

#[inline]
pub(crate) fn get_cur_frame_segid(
  b: Bxy,
  neighbors: NeighborAvailability,
  cur_seg_map: &DisjointMutSlice<SegmentId>,
  stride: u64,
) -> Rav1dResult<(SegmentId, u8)> {
  let column = u64::try_from(b.x).map_err(|_negative_column| EINVAL)?;
  let row = u64::try_from(b.y).map_err(|_negative_row| EINVAL)?;
  let current = row
    .checked_mul(stride)
    .and_then(|offset| offset.checked_add(column))
    .ok_or(EINVAL)?;

  match neighbors {
    NeighborAvailability::Neither => Ok((SegmentId::default(), 0)),
    NeighborAvailability::Left => {
      let offset = current.checked_sub(1).ok_or(EINVAL)?;
      let segment = segment_from_neighbor(cur_seg_map, offset)?;
      Ok((segment, 0))
    }
    NeighborAvailability::Top => {
      let offset = current.checked_sub(stride).ok_or(EINVAL)?;
      let segment = segment_from_neighbor(cur_seg_map, offset)?;
      Ok((segment, 0))
    }
    NeighborAvailability::Both => {
      segment_from_corner(cur_seg_map, SegmentMapPosition { current, stride })
    }
  }
}

fn segment_from_neighbor(
  map: &DisjointMutSlice<SegmentId>,
  offset: u64,
) -> Rav1dResult<SegmentId> {
  let Ok(length) = console_core_number_conversion::fixed_index(map.len());

  match (offset < length).then_some(()) {
    Some(()) => {
      let Ok(position) = console_core_number_conversion::index(offset);
      Ok(*map.index(position))
    }
    None => Err(EINVAL),
  }
}

struct SegmentMapPosition {
  current: u64,
  stride: u64,
}

fn segment_from_corner(
  map: &DisjointMutSlice<SegmentId>,
  position: SegmentMapPosition,
) -> Rav1dResult<(SegmentId, u8)> {
  let SegmentMapPosition { current, stride } = position;
  let left_offset = current.checked_sub(1).ok_or(EINVAL)?;
  let top_offset = current.checked_sub(stride).ok_or(EINVAL)?;
  let corner_offset = left_offset.checked_sub(stride).ok_or(EINVAL)?;
  let left = segment_from_neighbor(map, left_offset)?;
  let top = segment_from_neighbor(map, top_offset)?;
  let corner = segment_from_neighbor(map, corner_offset)?;
  let context = match (left == top, left == corner, top == corner) {
    (true, true, _) => 2,
    (true, _, _) | (_, true, _) | (_, _, true) => 1,
    (false, false, false) => 0,
  };
  let prediction = match ::core::primitive::bool::then_some(top == corner, ()) {
    ::core::option::Option::Some(()) => top,
    ::core::option::Option::None => left,
  };
  Ok((prediction, context))
}

#[inline]
fn fix_int_mv_precision(mv: &mut Mv) -> core::result::Result<(), console_core_never::Never> {
  {
    mv.x = i16::wrapping_add(i16::wrapping_sub(mv.x, i16::wrapping_shr(mv.x, 15_u32)), 3) & !7;
    mv.y = i16::wrapping_add(i16::wrapping_sub(mv.y, i16::wrapping_shr(mv.y, 15_u32)), 3) & !7;
  };

  Ok(())
}

#[inline]
pub(crate) fn fix_mv_precision(
  hdr: &Rav1dFrameHeader,
  mv: &mut Mv,
) -> core::result::Result<(), console_core_never::Never> {
  {
    match ::core::primitive::bool::then_some(hdr.force_integer_mv, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = fix_int_mv_precision(mv);
      }
      ::core::option::Option::None => match ::core::primitive::bool::then_some(!(*hdr).hp, ()) {
        ::core::option::Option::Some(()) => {
          mv.x = i16::wrapping_sub(mv.x, i16::wrapping_shr(mv.x, 15_u32)) & !1;
          mv.y = i16::wrapping_sub(mv.y, i16::wrapping_shr(mv.y, 15_u32)) & !1;
        }
        ::core::option::Option::None => {}
      },
    }
  };

  Ok(())
}

#[inline]
pub(crate) fn get_gmv_2d(
  motion: &Rav1dWarpedMotionParams,
  block_4x4: Rectangle<i32>,
  header: &Rav1dFrameHeader,
) -> Result<Mv, console_core_never::Never> {
  let [first_offset, second_offset, xx, xy, yx, yy] = motion.matrix;
  let mut vector = match motion.r#type {
    Rav1dWarpedMotionType::Identity => return Ok(Mv::ZERO),
    Rav1dWarpedMotionType::Translation => {
      let Ok(y) =
        console_core_number_conversion::wrapped::<_, i16>(first_offset.wrapping_shr(13));
      let Ok(x) =
        console_core_number_conversion::wrapped::<_, i16>(second_offset.wrapping_shr(13));
      Mv { x, y }
    }
    Rav1dWarpedMotionType::RotZoom | Rav1dWarpedMotionType::Affine => {
      let x = block_4x4
        .origin
        .x
        .wrapping_mul(4)
        .wrapping_add(block_4x4.size.width.wrapping_mul(2))
        .wrapping_sub(1);
      let y = block_4x4
        .origin
        .y
        .wrapping_mul(4)
        .wrapping_add(block_4x4.size.height.wrapping_mul(2))
        .wrapping_sub(1);
      let horizontal = xx
        .wrapping_sub(::core::primitive::i32::wrapping_shl(1, 16_u32))
        .wrapping_mul(x)
        .wrapping_add(xy.wrapping_mul(y))
        .wrapping_add(first_offset);
      let vertical = yy
        .wrapping_sub(::core::primitive::i32::wrapping_shl(1, 16_u32))
        .wrapping_mul(y)
        .wrapping_add(yx.wrapping_mul(x))
        .wrapping_add(second_offset);
      let precision_loss = u32::from(!header.hp);
      let shift = 13u32.wrapping_add(precision_loss);
      let rounding = 1i32.wrapping_shl(shift.wrapping_sub(1));
      let component = |signed: i32| -> Result<i16, console_core_never::Never> {
        let magnitude = signed
          .wrapping_abs()
          .wrapping_add(rounding)
          .wrapping_shr(shift)
          .wrapping_shl(precision_loss);
        let Ok(value) =
          console_core_number_conversion::wrapped::<_, i16>(apply_sign(magnitude, signed));
        Ok(value)
      };
      let Ok(x_2) = component(horizontal);
      let Ok(y_2) = component(vertical);
      Mv { x: x_2, y: y_2 }
    }
  };

  match header.force_integer_mv.then_some(()) {
    Some(()) => {
      let Ok(()) = fix_int_mv_precision(&mut vector);
    }
    None => {}
  }

  Ok(vector)
}

#[cfg(test)]
mod left_split_parity {
  use super::*;
  use crate::src::levels::N_INTRA_PRED_MODES;
  use crate::src::levels::N_UV_INTRA_PRED_MODES;
  use crate::src::tables::dav1d_txfm_dimensions;
  use console_core_checksums::{Checksum, md5::Md5};
  use strum::EnumCount as _;

  struct Rng(u64);

  impl Rng {
    fn next(&mut self) -> core::result::Result<u32, console_core_never::Never> {
      Ok({
        let mut x = self.0;
        x ^= u64::wrapping_shl(x, 13_u32);
        x ^= u64::wrapping_shr(x, 7_u32);
        x ^= u64::wrapping_shl(x, 17_u32);
        self.0 = x;

        {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, u32>(u64::wrapping_shr(x, 32_u32));
          numeric_value
        }
      })
    }

    fn below(&mut self, limit: u32) -> Rav1dResult<u32> {
      let Ok(next) = self.next();
      let value = next.checked_rem(limit).ok_or(EINVAL)?;
      Ok(value)
    }
  }

  fn comp_type_of(
    v: u32,
  ) -> core::result::Result<Option<CompInterType>, console_core_never::Never> {
    Ok({
      match v {
        0 => None,
        1 => Some(CompInterType::WeightedAvg),
        2 => Some(CompInterType::Avg),
        3 => Some(CompInterType::Seg),
        _ => Some(CompInterType::Wedge),
      }
    })
  }

  fn filter_of(v: u32) -> core::result::Result<Rav1dFilterMode, console_core_never::Never> {
    Ok({
      match v {
        0 => Rav1dFilterMode::Regular8Tap,
        1 => Rav1dFilterMode::Smooth8Tap,
        2 => Rav1dFilterMode::Sharp8Tap,
        3 => Rav1dFilterMode::Bilinear,
        _ => Rav1dFilterMode::Switchable,
      }
    })
  }

  fn block_level_of(v: u32) -> core::result::Result<BlockLevel, console_core_never::Never> {
    Ok({
      match v {
        0 => BlockLevel::Bl128x128,
        1 => BlockLevel::Bl64x64,
        2 => BlockLevel::Bl32x32,
        3 => BlockLevel::Bl16x16,
        _ => BlockLevel::Bl8x8,
      }
    })
  }

  fn write_context<T: Copy, const N: usize>(
    storage: &mut DisjointMut<Align8<[T; N]>>,
    coordinate: u8,
    value: T,
  ) -> Rav1dResult {
    let Ok(position) = console_core_number_conversion::index(coordinate);
    let destination = storage.get_mut().get_mut(position).ok_or(EINVAL)?;
    *destination = value;
    Ok(())
  }

  fn random_ctx(rng: &mut Rng) -> Rav1dResult<BlockContext> {
    let mut block = BlockContext::default();
    let Ok(luma_modes) = console_core_number_conversion::wrapped::<_, u32>(N_INTRA_PRED_MODES);
    let Ok(chroma_modes) =
      console_core_number_conversion::wrapped::<_, u32>(N_UV_INTRA_PRED_MODES);

    for position in 0u8..32 {
      let intra_flag = rng.below(2)?;
      let is_intra = intra_flag == 1;
      let compound = match ::core::primitive::bool::then_some(is_intra, ()) {
        ::core::option::Option::Some(()) => None,
        ::core::option::Option::None => {
          let code = rng.below(5)?;
          let Ok(mode) = comp_type_of(code);
          mode
        }
      };
      let primary = match ::core::primitive::bool::then_some(is_intra, ()) {
        ::core::option::Option::Some(()) => -1,
        ::core::option::Option::None => {
          let code = rng.below(7)?;
          let Ok(reference) = console_core_number_conversion::wrapped::<_, i8>(code);
          reference
        }
      };
      let secondary = match compound {
        Some(_mode) => {
          let code = rng.below(7)?;
          let Ok(reference) = console_core_number_conversion::wrapped::<_, i8>(code);
          reference
        }
        None => -1,
      };

      let luma_code = rng.below(luma_modes)?;
      let Ok(luma) = console_core_number_conversion::wrapped::<_, u8>(luma_code);
      let chroma_code = rng.below(chroma_modes)?;
      let Ok(chroma) = console_core_number_conversion::wrapped::<_, u8>(chroma_code);
      let transform_code = rng.below(9)?;
      let Ok(transform) = console_core_number_conversion::wrapped::<_, i8>(transform_code);
      let vertical_code = rng.below(5)?;
      let Ok(vertical) = filter_of(vertical_code);
      let horizontal_code = rng.below(5)?;
      let Ok(horizontal) = filter_of(horizontal_code);
      let [first_reference, second_reference] = &mut block.r#ref;
      let [vertical_filter, horizontal_filter] = &mut block.filter;
      write_context(&mut block.intra, position, u8::from(is_intra))?;
      write_context(&mut block.comp_type, position, compound)?;
      write_context(first_reference, position, primary)?;
      write_context(second_reference, position, secondary)?;
      write_context(&mut block.mode, position, luma)?;
      write_context(&mut block.uvmode, position, chroma)?;
      write_context(&mut block.tx_intra, position, transform.wrapping_sub(1))?;
      write_context(vertical_filter, position, vertical)?;
      write_context(horizontal_filter, position, horizontal)?;
    }

    for position in 0u8..16 {
      let code = rng.below(32)?;
      let Ok(value) = console_core_number_conversion::wrapped::<_, u8>(code);
      write_context(&mut block.partition, position, value)?;
    }

    Ok(block)
  }

  #[derive(Default)]
  struct Seen {
    values: u16,
    checksum: Md5,
  }

  impl Seen {
    fn note(&mut self, value: u8) -> Rav1dResult {
      let bit = 1u16.checked_shl(u32::from(value)).ok_or(EINVAL)?;
      self.values |= bit;
      let Ok(()) = self.checksum.process_byte(value);
      Ok(())
    }

    fn distinct(&self) -> Result<u32, console_core_never::Never> {
      Ok(self.values.count_ones())
    }

    fn digest(&self) -> Result<u128, console_core_never::Never> {
      let Ok(bytes) = self.checksum.md5();
      Ok(u128::from_be_bytes(bytes))
    }
  }

  fn expected_contexts() -> Result<(u32, [u128; 15]), console_core_never::Never> {
    #[cfg(miri)]
    let (trials, expected) = (
      200u32,
      [
        0xce406859_034c29fe_4edba3d6_c35902e3,
        0xce908e5b_ff707b85_ef355363_7ce01541,
        0xc4a51ce5_090f6dec_66c86b7e_cea161b1,
        0x5105e038_ac6646d4_f1f076dc_41ac50d9,
        0x2d81d9c9_1ae38541_60db2bf8_a9a1baaf,
        0x7138c866_e5055f0d_f81f26ab_21c67fa4,
        0x64412025_39c9fd5a_ace79a18_0eea1281,
        0x1e7f4f06_e45227bd_551f0420_fd147644,
        0x6b1d64a4_a867dbc4_3713ef04_f9244b1b,
        0x66d192b6_d39b56dd_0f335f83_e9fc40bd,
        0x466e3d94_a4747b75_4d028569_701c8029,
        0x8731dad3_b1364361_5aad7a25_560dd09c,
        0x2af9af5c_b7615957_4cc90528_4911c292,
        0x9e9fc03b_593647d7_a061bd61_a7d2e41d,
        0xaa1481d9_e1d95a92_d66a2dc4_98e5f772,
      ],
    );
    #[cfg(not(miri))]
    let (trials, expected) = (
      20_000u32,
      [
        0x38ab677c_4ebb1106_fac8a5e4_455d27b2,
        0xb4010d19_80d6edfb_3c73dc45_da4c4b7d,
        0xcefd3ca1_fbfe9288_6fcfb78f_5064261d,
        0x5bdf21d5_e84f1346_9e8685e2_e6edee48,
        0x9c8bb7f5_8adb05a7_5a731697_705005ab,
        0xe0f384db_18063e8a_c3502ec3_803867d1,
        0xcb520b0e_5e85875c_549fcef0_40fb1c70,
        0x70a3c7ed_583614e8_62520bc5_dbcbf3d7,
        0x0467f8fc_e7b78169_7bac75ed_e7a4c9de,
        0x9f617b12_b0d01e9f_7e652e46_20fe51ff,
        0xe7abcb49_aafbc20e_20038d4c_81db674a,
        0x53f4ad06_f383f24e_61531d6f_d01dca37,
        0xf8f440e1_957cc436_c4169db6_342d31f4,
        0x80923121_b648dbf4_bc719733_7bf4942d,
        0xec2a56dc_3e902436_f6bf2515_3c106b4a,
      ],
    );
    Ok((trials, expected))
  }

  fn reference_context_values(neighbors: BlockNeighbors<'_>) -> Rav1dResult<[u8; 7]> {
    let reference_counts = ReferenceCounts::from_neighbors(neighbors)?;
    let values = [
      ReferenceContext::Direction,
      ReferenceContext::ForwardGroup,
      ReferenceContext::NearForward,
      ReferenceContext::FarForward,
      ReferenceContext::BackwardGroup,
      ReferenceContext::NearBackward,
      ReferenceContext::UnidirectionalPair,
    ]
    .map(|context| {
      let Ok(value) = reference_counts.context(context);
      value
    });
    Ok(values)
  }

  #[test]
  fn all_fifteen_helpers_match_the_pre_split_implementation() -> Rav1dResult {
    let Ok((trials, expected)) = expected_contexts();
    let mut rng = Rng(0x5eed_1eaf_c0ff_ee01);
    let mut seen: [Seen; 15] = Default::default();
    let mut availability_seen = 0u8;

    for _ in 0..trials {
      let above = random_ctx(&mut rng)?;
      let left = random_ctx(&mut rng)?;
      let row_4x4 = rng.below(32)?;
      let Ok(y_4x4) = console_core_number_conversion::wrapped::<_, i32>(row_4x4);
      let column_4x4 = rng.below(32)?;
      let Ok(x_4x4) = console_core_number_conversion::wrapped::<_, i32>(column_4x4);
      let row_8x8 = rng.below(16)?;
      let Ok(y_8x8) = console_core_number_conversion::wrapped::<_, i32>(row_8x8);
      let column_8x8 = rng.below(16)?;
      let Ok(x_8x8) = console_core_number_conversion::wrapped::<_, i32>(column_8x8);
      let has_top = rng.below(2)?;
      let has_left = rng.below(2)?;
      let availability = NeighborAvailability::from((has_top == 1, has_left == 1));
      availability_seen |= match availability {
        NeighborAvailability::Neither => 0b0001,
        NeighborAvailability::Left => 0b0010,
        NeighborAvailability::Top => 0b0100,
        NeighborAvailability::Both => 0b1000,
      };
      let blocks = BlockNeighbors {
        above: &above,
        left: &left,
        location: Bxy { x: x_4x4, y: y_4x4 },
        availability,
      };
      let partitions = BlockNeighbors {
        location: Bxy { x: x_8x8, y: y_8x8 },
        ..blocks
      };
      let level_value = rng.below(5)?;
      let Ok(level) = block_level_of(level_value);
      let Ok(size_count) = console_core_number_conversion::wrapped::<_, u32>(TxfmSize::COUNT);
      let size_value = rng.below(size_count)?;
      let Ok(size_index) = console_core_number_conversion::index(size_value);
      let transform = dav1d_txfm_dimensions.get(size_index).ok_or(EINVAL)?;
      let arity = rng.below(2)?;
      let references = match arity {
        1 => ReferenceArity::Compound,
        _ => ReferenceArity::Single,
      };
      let filter_axis = rng.below(2)?;
      let axis = match filter_axis {
        1 => FilterAxis::Horizontal,
        _ => FilterAxis::Vertical,
      };
      let reference = rng.below(8)?;
      let Ok(reference_code) = console_core_number_conversion::wrapped::<_, i8>(reference);
      let selected_reference = reference_code.wrapping_sub(1);
      let hint_width = rng.below(9)?;
      let Ok(width) = console_core_number_conversion::wrapped::<_, u8>(hint_width);
      let current = rng.below(256)?;
      let first_hint = rng.below(256)?;
      let second_hint = rng.below(256)?;
      let order = CompoundOrderHints {
        width,
        current,
        reference_hints: [first_hint, second_hint],
      };
      let intra = get_intra_ctx(blocks)?;
      let transform_ctx = get_tx_ctx(blocks, transform)?;
      let partition = get_partition_ctx(partitions, level)?;
      let filter = get_filter_ctx(blocks, references, axis, selected_reference)?;
      let compound = get_comp_ctx(blocks)?;
      let compound_direction = get_comp_dir_ctx(blocks)?;
      let mask = get_mask_comp_ctx(blocks)?;
      let joint = get_jnt_comp_ctx(order, blocks)?;
      let [
        direction,
        forward,
        near_forward,
        far_forward,
        backward,
        near_backward,
        paired,
      ] = reference_context_values(blocks)?;
      let results = [
        intra,
        transform_ctx,
        partition,
        filter,
        compound,
        compound_direction,
        mask,
        direction,
        forward,
        near_forward,
        far_forward,
        backward,
        near_backward,
        paired,
        joint,
      ];

      for (observed, value) in seen.iter_mut().zip(results) {
        observed.note(value)?;
      }
    }

    assert_eq!(availability_seen, 0b1111);

    for ((observed, digest), context) in seen.iter().zip(expected).zip(0u8..) {
      let Ok(distinct) = observed.distinct();
      assert!(distinct >= 2, "context {context} returned a constant");
      assert_eq!(observed.digest(), Ok(digest), "context {context}");
    }

    Ok(())
  }

  #[test]
  fn trial_floor_is_not_vacuous() -> Rav1dResult {
    let mut rng = Rng(0x5eed_1eaf_c0ff_ee01);
    let mut tl_combos = 0u8;
    let mut seen5 = Seen::default();

    for _ in 0..200 {
      let a = random_ctx(&mut rng)?;
      let l = random_ctx(&mut rng)?;
      let below = rng.below(32)?;
      let Ok(yb4) = console_core_number_conversion::wrapped::<_, i32>(below);
      let rng_below = rng.below(32)?;
      let Ok(xb4) = console_core_number_conversion::wrapped::<_, i32>(rng_below);
      let top = rng.below(2)?;
      let left = rng.below(2)?;
      let neighbors = NeighborAvailability::from((top == 1, left == 1));
      tl_combos |= match neighbors {
        NeighborAvailability::Neither => 0b0001,
        NeighborAvailability::Left => 0b0010,
        NeighborAvailability::Top => 0b0100,
        NeighborAvailability::Both => 0b1000,
      };
      let comp_dir_ctx = get_comp_dir_ctx(BlockNeighbors {
        above: &a,
        left: &l,
        location: Bxy { x: xb4, y: yb4 },
        availability: neighbors,
      })?;
      seen5.note(comp_dir_ctx)?;
    }

    assert_eq!(
      tl_combos, 0b1111,
      "200 trials miss a have_top/have_left combination"
    );
    let Ok(distinct) = seen5.distinct();
    assert!(
      distinct >= 5,
      "200 trials reach only {} of get_comp_dir_ctx's 5 outputs",
      ({
        let Ok(explicit_value) = seen5.distinct();
        explicit_value
      })
    );
    Ok(())
  }

  #[test]
  fn smoothing_flags_match_context_metadata() -> Rav1dResult {
    use crate::src::ipred_prepare::{sm_flag, sm_uv_flag};
    use crate::src::levels::{SMOOTH_H_PRED, SMOOTH_PRED, SMOOTH_V_PRED};
    let mut rng = Rng(0x5eed_1eaf_c0ff_ee02);
    let mut nonzero = 0;
    let mut zero = 0;

    for _ in 0..2_000 {
      let b = random_ctx(&mut rng)?;

      for i in 0..32 {
        let Ok(index) = console_core_number_conversion::wrapped_index(i);
        let intra = *b.intra.index(index);
        let luma_mode = *b.mode.index(index);
        let chroma_mode = *b.uvmode.index(index);
        let want = match (intra, luma_mode) {
          (0, _) => 0,
          (_, SMOOTH_PRED | SMOOTH_H_PRED | SMOOTH_V_PRED) => 512,
          _ => 0,
        };
        let want_uv = match chroma_mode {
          SMOOTH_PRED | SMOOTH_H_PRED | SMOOTH_V_PRED => 512,
          _ => 0,
        };
        let luma = sm_flag(&b, i)?;
        let chroma = sm_uv_flag(&b, i)?;
        assert_eq!(luma, want);
        assert_eq!(chroma, want_uv);

        match ::core::primitive::bool::then_some(want | want_uv != 0, ()) {
          ::core::option::Option::Some(()) => {
            {
              let numeric_amount = 1;
              let numeric_target = &mut (nonzero);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };
          }
          ::core::option::Option::None => {
            {
              let numeric_amount = 1;
              let numeric_target = &mut (zero);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };
          }
        }
      }
    }

    assert!(
      nonzero > 0 && zero > 0,
      "only one branch ran: {nonzero} / {zero}"
    );
    Ok(())
  }
}
