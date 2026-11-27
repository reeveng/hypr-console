#![forbid(unsafe_code)]
use crate::include::common::bitdepth::AsPrimitive;
use crate::include::common::bitdepth::BPC;
use crate::include::common::bitdepth::BitDepth;
use crate::include::common::bitdepth::ToPrimitive;
use crate::include::common::dump::coef_dump;
use crate::include::common::dump::hex_dump;
use crate::include::common::dump::hex_dump_pic;
use crate::include::common::intops::apply_sign64;
use crate::include::common::intops::ulog2;
use crate::include::dav1d::dav1d::Rav1dInloopFilterType;
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::headers::Rav1dPixelLayoutSubSampled;
use crate::include::dav1d::headers::Rav1dWarpedMotionParams;
use crate::include::dav1d::headers::Rav1dWarpedMotionType;
use crate::include::dav1d::headers::{Rav1dFilterMode, Rav1dFrameHeader};
use crate::include::dav1d::picture::PicOffset;
use crate::include::dav1d::picture::Rav1dPictureDataComponent;

use crate::src::cdef_apply::{CdefRowSpan, CdefRows, rav1d_cdef_brow};
use crate::src::ctx::case_set_al;
use crate::src::ctx::small_memset;
use crate::src::ctx::{CaseSet, ContextRun};
use crate::src::env::get_uv_inter_txtp;
use crate::src::error::Rav1dError::{EINVAL, EIO};
use crate::src::error::Rav1dResult;
use crate::src::pal::PalettePlane;
use console_core_geometry::{Point, Rectangle, Size};
#[path = "palette_cache.rs"]
mod palette_cache;
use crate::src::in_range::InRange;
use crate::src::internal::Bxy;
use crate::src::internal::Cf;
use crate::src::internal::ChromaNeighborPrediction;
use crate::src::internal::CodedBlockInfo;
use crate::src::internal::Rav1dContext;
use crate::src::internal::Rav1dFrameData;
use crate::src::internal::Rav1dTaskContext;
use crate::src::internal::ScratchEmuEdge;
use crate::src::internal::TaskContextScratch;
use crate::src::internal::TileStateRef;
use crate::src::internal::{Rav1dTileState, Rav1dTileStateContext};
use crate::src::intra_edge::{EdgeAvailability, EdgeFlags};
use crate::src::ipred::{CflAcExtraction, CflPrediction, IntraPrediction};
use crate::src::ipred_prepare::sm_flag;
use crate::src::ipred_prepare::sm_uv_flag;
use crate::src::ipred_prepare::{
  AboveEdge, IntraEdgeDestination, IntraEdgePreparation, PrefilteredTop,
  rav1d_prepare_intra_edges,
};
use crate::src::levels::Av1Block;
use crate::src::levels::Av1BlockInter;
use crate::src::levels::Av1BlockIntra;
use crate::src::levels::Av1BlockIntraInter;
use crate::src::levels::BlockSize;
use crate::src::levels::CFL_PRED;
use crate::src::levels::CompInterType;
use crate::src::levels::DC_PRED;
use crate::src::levels::DCT_DCT;
use crate::src::levels::FILTER_PRED;
use crate::src::levels::Filter2d;
use crate::src::levels::GLOBALMV;
use crate::src::levels::GLOBALMV_GLOBALMV;
use crate::src::levels::HOR_PRED;
use crate::src::levels::IDTX;
use crate::src::levels::InterIntraPredMode;
use crate::src::levels::InterIntraType;
use crate::src::levels::IntraPredMode;
use crate::src::levels::MotionMode;
use crate::src::levels::Mv;
use crate::src::levels::SMOOTH_PRED;
use crate::src::levels::TxClass;
use crate::src::levels::TxfmSize;
use crate::src::levels::TxfmType;
use crate::src::levels::VERT_PRED;
use crate::src::levels::WHT_WHT;
use crate::src::lf_apply::rav1d_copy_lpf;
use crate::src::lf_apply::rav1d_loopfilter_sbrow_cols;
use crate::src::lf_apply::rav1d_loopfilter_sbrow_rows;
use crate::src::lr_apply::rav1d_lr_sbrow;
use crate::src::mc::{ResizePhase, ResizeSampling};
use crate::src::msac::EntropyBit;
use crate::src::msac::MsacContext;
use crate::src::msac::rav1d_msac_decode_bool_adapt;
use crate::src::msac::rav1d_msac_decode_bool_equi;
use crate::src::msac::rav1d_msac_decode_bools;
use crate::src::msac::rav1d_msac_decode_hi_tok;
use crate::src::msac::rav1d_msac_decode_symbol_adapt4;
use crate::src::msac::rav1d_msac_decode_symbol_adapt8;
use crate::src::msac::rav1d_msac_decode_symbol_adapt16;
use crate::src::picture::Rav1dThreadPicture;
use crate::src::relaxed_atomic::RelaxedAtomic;
use crate::src::scan::{Scan, dav1d_scans};
use crate::src::strided::Strided as _;
use crate::src::tables::TxfmInfo;
use crate::src::tables::dav1d_filter_2d;
use crate::src::tables::dav1d_filter_mode_to_y_mode;
use crate::src::tables::dav1d_lo_ctx_offsets;
use crate::src::tables::dav1d_skip_ctx;
use crate::src::tables::dav1d_tx_type_class;
use crate::src::tables::dav1d_tx_types_per_set;
use crate::src::tables::dav1d_txfm_dimensions;
use crate::src::tables::dav1d_txtp_from_uvmode;
use crate::src::wedge::dav1d_ii_masks;
use crate::src::wedge::dav1d_wedge_masks;
use crate::src::with_offset::WithOffset;
use std::cmp;
use std::ffi::c_int;
use std::ffi::c_uint;
use std::ptr;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockTrace {
  Enabled,
  Disabled,
}

const BLOCK_TRACE: BlockTrace = BlockTrace::Disabled;
const PIXEL_TRACE: BlockTrace = BlockTrace::Disabled;
const COEFFICIENT_TRACE: BlockTrace = BlockTrace::Disabled;

pub(crate) fn debug_block_info(
  frame: &Rav1dFrameData,
  origin: Bxy,
) -> Result<BlockTrace, console_core_never::Never> {
  Ok(match BLOCK_TRACE {
    BlockTrace::Disabled => BlockTrace::Disabled,
    BlockTrace::Enabled => match (
      frame.frame_hdr.as_ref().map(|header| header.frame_offset),
      origin,
    ) {
      (
        Some(2),
        Bxy {
          x: 8..=11,
          y: 0..=3,
        },
      ) => BlockTrace::Enabled,
      _ => BlockTrace::Disabled,
    },
  })
}

pub(crate) type ReconBIntraFn = fn(
  &Rav1dFrameData,
  &mut Rav1dTaskContext,
  Option<&mut Rav1dTileStateContext>,
  BlockSize,
  EdgeFlags,
  &Av1Block,
  &Av1BlockIntra,
) -> Rav1dResult;

pub(crate) type ReconBInterFn = fn(
  &Rav1dFrameData,
  &mut Rav1dTaskContext,
  Option<&mut Rav1dTileStateContext>,
  BlockSize,
  &Av1Block,
  &Av1BlockInter,
) -> Result<(), ()>;

pub(crate) type FilterSbrowFn =
  fn(&Rav1dContext, &Rav1dFrameData, &mut Rav1dTaskContext, c_int) -> Rav1dResult;

pub(crate) type BackupIpredEdgeFn = fn(&Rav1dFrameData, &mut Rav1dTaskContext) -> Rav1dResult;

pub(crate) type ReadCoefBlocksFn = fn(
  &Rav1dFrameData,
  &mut Rav1dTaskContext,
  &mut Rav1dTileStateContext,
  BlockSize,
  &Av1Block,
) -> Rav1dResult;

pub(crate) struct PaletteCopy {
  pub(crate) region: Rectangle<i32>,
  pub(crate) plane: PalettePlane,
}

pub(crate) struct PaletteRead {
  pub(crate) size_context: u8,
  pub(crate) position: Point<i32>,
}

pub(crate) type CopyPalBlockFn =
  fn(&mut Rav1dTaskContext, &Rav1dFrameData, PaletteCopy) -> Rav1dResult;

pub(crate) type ReadPalFn = fn(
  &mut Rav1dTaskContext,
  &Rav1dFrameData,
  &mut Rav1dTileStateContext,
  PaletteRead,
) -> Rav1dResult<u8>;

#[inline]
fn read_golomb(msac: &mut MsacContext) -> Result<u32, console_core_never::Never> {
  let mut length = 0u32;

  for prefix in 0..=31 {
    let Ok(bit) = rav1d_msac_decode_bool_equi(msac);
    length = prefix;

    match bit {
      EntropyBit::Zero => {}
      EntropyBit::One => break,
    }
  }

  let mut value = 1u32;

  for _ in 0..length {
    let Ok(bit) = rav1d_msac_decode_bool_equi(msac);
    value = value
      .wrapping_shl(1)
      .wrapping_add(u32::from(matches!(bit, EntropyBit::One)));
  }

  Ok(value.wrapping_sub(1))
}

#[derive(Clone, Copy)]
enum ResidualPlane {
  Luma,
  Chroma(Rav1dPixelLayout),
}

struct ResidualNeighbors<'a> {
  above: &'a [u8],
  left: &'a [u8],
}

fn chroma_neighbor_evidence(context: &[u8]) -> Rav1dResult<u8> {
  match context.len() {
    1 | 2 | 4 | 8 => Ok(u8::from(context.iter().any(|&entry| entry != 0x40))),
    _ => Err(EINVAL),
  }
}

fn luma_neighbor_level(context: &[u8]) -> Rav1dResult<u8> {
  match context.len() {
    1 | 2 | 4 | 8 | 16 => {
      let merged = context
        .iter()
        .copied()
        .fold(0, |combined, entry| combined | entry);
      Ok((merged & 0x3f).min(4))
    }
    _ => Err(EINVAL),
  }
}

#[inline]
fn get_skip_ctx(
  t_dim: &TxfmInfo,
  bs: BlockSize,
  ResidualNeighbors { above, left }: ResidualNeighbors<'_>,
  plane: ResidualPlane,
) -> Rav1dResult<InRange<u8, 0, 12>> {
  let Ok(&[_, _, width, height]) = bs.dimensions();
  let skip_ctx = match plane {
    ResidualPlane::Luma => match (width.cmp(&t_dim.lw), height.cmp(&t_dim.lh)) {
      (cmp::Ordering::Equal, cmp::Ordering::Equal) => 0,
      _ => {
        let top_level = luma_neighbor_level(above)?;
        let left_level = luma_neighbor_level(left)?;
        let Ok(top) = console_core_number_conversion::index(top_level);
        let Ok(side) = console_core_number_conversion::index(left_level);
        let above_context = dav1d_skip_ctx.get(top).ok_or(EINVAL)?;
        ::core::result::Result::<&_, _>::copied(above_context.get(side).ok_or(EINVAL))?
      }
    },
    ResidualPlane::Chroma(layout) => {
      let horizontal = u8::from(width != 0 && layout != Rav1dPixelLayout::I444);
      let vertical = u8::from(height != 0 && layout == Rav1dPixelLayout::I420);
      let more_than_one =
        width.wrapping_sub(horizontal) > t_dim.lw || height.wrapping_sub(vertical) > t_dim.lh;
      let base = 7_u8.wrapping_add(u8::from(more_than_one).wrapping_mul(3));
      let top = chroma_neighbor_evidence(above)?;
      let side = chroma_neighbor_evidence(left)?;
      base.wrapping_add(top).wrapping_add(side)
    }
  };
  let Ok(context) = InRange::new(skip_ctx);
  context.ok_or(EINVAL)
}

#[inline]
fn dc_neighbor_signs(context: &[u8]) -> Result<i32, console_core_never::Never> {
  let (words, tail) = context.as_chunks::<8>();
  let mut sum = 0_i32;

  for word in words {
    let masked = u64::from_le_bytes(*word) & 0xc0c0_c0c0_c0c0_c0c0;
    let signs = masked.wrapping_shr(6);
    let packed = signs.wrapping_mul(0x0101_0101_0101_0101).wrapping_shr(56);
    let Ok(word_sum) = console_core_number_conversion::wrapped::<_, i32>(packed);
    sum = sum.wrapping_add(word_sum);
  }

  for &byte in tail {
    sum = sum.wrapping_add(i32::from(byte.wrapping_shr(6)));
  }

  let Ok(length) = console_core_number_conversion::wrapped::<_, i32>(context.len());
  Ok(sum.wrapping_sub(length))
}

#[inline]
fn get_dc_sign_ctx(tx: TxfmSize, above: &[u8], left: &[u8]) -> Rav1dResult<c_uint> {
  let Ok(ordinal) = tx.ordinal();
  let Ok(table) = console_core_number_conversion::index(ordinal);
  let dimensions = dav1d_txfm_dimensions.get(table).ok_or(EINVAL)?;
  let Ok(width) = console_core_number_conversion::index(dimensions.w);
  let Ok(height) = console_core_number_conversion::index(dimensions.h);
  let above_2 = above.get(..width).ok_or(EINVAL)?;
  let left_2 = left.get(..height).ok_or(EINVAL)?;
  let Ok(horizontal) = dc_neighbor_signs(above_2);
  let Ok(vertical) = dc_neighbor_signs(left_2);
  let sum = horizontal.wrapping_add(vertical);
  Ok(u32::from(sum != 0).wrapping_add(u32::from(sum > 0)))
}

#[cfg(test)]
mod coefficient_neighborhood_tests {
  use super::{EINVAL, Rav1dResult, TxfmSize, dav1d_txfm_dimensions, get_dc_sign_ctx};

  #[test]
  fn each_transform_requires_complete_top_and_left_contexts() -> Rav1dResult {
    let neutral = [0x40; 17];

    let Ok(last_size) = TxfmSize::R64x16.ordinal();

    for size in 0..=last_size {
      let tx = TxfmSize::from_repr(
        match ::console_core_number_conversion::index::<::core::primitive::u8>(size) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        },
      )
      .ok_or(EINVAL)?;
      let dimensions = dav1d_txfm_dimensions
        .get(
          match ::console_core_number_conversion::index::<::core::primitive::u8>(size) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          },
        )
        .ok_or(EINVAL)?;
      let top = neutral
        .get(
          ..match ::console_core_number_conversion::index::<::core::primitive::u8>(dimensions.w) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          },
        )
        .ok_or(EINVAL)?;
      let left = neutral
        .get(
          ..match ::console_core_number_conversion::index::<::core::primitive::u8>(dimensions.h) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          },
        )
        .ok_or(EINVAL)?;
      let (_, short_top) = top.split_last().ok_or(EINVAL)?;
      let (_, short_left) = left.split_last().ok_or(EINVAL)?;
      assert_eq!(get_dc_sign_ctx(tx, top, left), Ok(0), "{tx:?}");
      assert_eq!(get_dc_sign_ctx(tx, &neutral, &neutral), Ok(0), "{tx:?}");
      assert_eq!(
        get_dc_sign_ctx(tx, short_top, left),
        Err(EINVAL),
        "{tx:?} top"
      );
      assert_eq!(
        get_dc_sign_ctx(tx, top, short_left),
        Err(EINVAL),
        "{tx:?} left"
      );
    }

    Ok(())
  }
}

#[inline]
fn get_lo_ctx(
  levels: &[u8],
  tx_class: TxClass,
  hi_mag: &mut u32,
  ctx_offsets: Option<&[[u8; 5]; 5]>,
  Point { x, y }: Point<u8>,
  stride: u8,
) -> Rav1dResult<u8> {
  let level = |offset: Point<u16>| -> Rav1dResult<u32> {
    let position = offset
      .y
      .wrapping_mul(u16::from(stride))
      .wrapping_add(offset.x);
    let Ok(index) = console_core_number_conversion::index(position);
    levels.get(index).copied().map(u32::from).ok_or(EINVAL)
  };
  let right = level(Point { x: 1, y: 0 })?;
  let below = level(Point { x: 0, y: 1 })?;
  let farther = level(Point { x: 2, y: 0 })?;
  let first = right.wrapping_add(below);
  let (magnitude, remainder, offset) = match (tx_class, ctx_offsets) {
    (TxClass::TwoD, Some(offsets)) => {
      let diagonal = level(Point { x: 1, y: 1 })?;
      let downward = level(Point { x: 0, y: 2 })?;
      let Ok(column) = console_core_number_conversion::index(x.min(4));
      let Ok(row) = console_core_number_conversion::index(y.min(4));
      let offset = offsets
        .get(row)
        .and_then(|values| values.get(column))
        .copied()
        .ok_or(EINVAL)?;
      (
        first.wrapping_add(diagonal),
        farther.wrapping_add(downward),
        offset,
      )
    }
    (TxClass::H | TxClass::V, None) => {
      let third = level(Point { x: 3, y: 0 })?;
      let fourth = level(Point { x: 4, y: 0 })?;
      let offset = 26u8.wrapping_add(y.min(2).wrapping_mul(5));
      (
        first.wrapping_add(farther),
        third.wrapping_add(fourth),
        offset,
      )
    }
    (TxClass::TwoD, None) | (TxClass::H | TxClass::V, Some(_)) => return Err(EINVAL),
  };
  *hi_mag = magnitude;
  let combined = magnitude.wrapping_add(remainder);
  let Ok(quantized) = console_core_number_conversion::fitted::<_, u8>(
    combined.wrapping_add(64).wrapping_shr(7).min(4),
  );
  Ok(offset.wrapping_add(quantized))
}

struct CoefficientDecoder<'a> {
  tile: &'a Rav1dTileState,
  state: &'a mut Rav1dTileStateContext,
  scratch: &'a mut TaskContextScratch,
  coefficients: &'a mut Cf,
}

struct CoefficientBlock<'a> {
  trace: BlockTrace,
  above: &'a [u8],
  left: &'a [u8],
  transform: TxfmSize,
  size: BlockSize,
  syntax: &'a Av1Block,
  plane: u8,
  storage: CfSelect,
}

struct CoefficientOutput<'a> {
  transform: &'a mut TxfmType,
  residual_context: &'a mut u8,
}

struct TransformRequest<'a> {
  block: &'a Av1Block,
  dimensions: &'a TxfmInfo,
  size: TxfmSize,
  plane: ResidualPlane,
  quantization: CoefficientQuantization,
  luma_transform: TxfmType,
  trace: BlockTrace,
}

#[derive(Clone, Copy)]
enum CoefficientQuantization {
  Lossless,
  Quantized,
}

struct TransformTables<'a> {
  size: TxfmSize,
  dimensions: &'a TxfmInfo,
  set: TransformSet,
  trace: BlockTrace,
}

#[derive(Clone, Copy)]
enum TransformSet {
  Reduced,
  Full,
}

fn read_block_transform(
  tile: &mut Rav1dTileStateContext,
  frame: &Rav1dFrameHeader,
  request: TransformRequest<'_>,
) -> Rav1dResult<TxfmType> {
  let dimensions = request.dimensions;

  match request.quantization {
    CoefficientQuantization::Lossless => {
      let Ok(expected) = TxfmSize::S4x4.ordinal();
      return match dimensions.max.cmp(&expected) {
        cmp::Ordering::Equal => Ok(WHT_WHT),
        cmp::Ordering::Less | cmp::Ordering::Greater => Err(EINVAL),
      };
    }
    CoefficientQuantization::Quantized => {}
  }

  let limit = match &request.block.ii {
    Av1BlockIntraInter::Intra(_) => TxfmSize::S32x32,
    Av1BlockIntraInter::Inter(_) => TxfmSize::S64x64,
  };
  let Ok(maximum) = limit.ordinal();

  match dimensions.max.cmp(&maximum) {
    cmp::Ordering::Equal | cmp::Ordering::Greater => return Ok(DCT_DCT),
    cmp::Ordering::Less => {}
  }

  match (request.plane, &request.block.ii) {
    (ResidualPlane::Chroma(_), Av1BlockIntraInter::Intra(prediction)) => {
      let Ok(mode) = console_core_number_conversion::index(prediction.uv_mode);
      return dav1d_txtp_from_uvmode.get(mode).copied().ok_or(EINVAL);
    }
    (ResidualPlane::Chroma(_), Av1BlockIntraInter::Inter(_)) => {
      let Ok(transform) = get_uv_inter_txtp(dimensions, request.luma_transform);
      return Ok(transform);
    }
    (ResidualPlane::Luma, _) => {}
  }

  let Ok(segment) = request.block.seg_id.get();
  let Ok(position) = console_core_number_conversion::index(segment);
  let &quantizer = frame.segmentation.qidx.get(position).ok_or(EINVAL)?;

  match quantizer {
    0 => return Ok(DCT_DCT),
    _ => {}
  }

  let set = match frame.reduced_txtp_set {
    0 => TransformSet::Full,
    _ => TransformSet::Reduced,
  };
  let tables = TransformTables {
    size: request.size,
    dimensions,
    set,
    trace: request.trace,
  };

  match &request.block.ii {
    Av1BlockIntraInter::Intra(prediction) => read_luma_intra_transform(tile, prediction, tables),
    Av1BlockIntraInter::Inter(_) => read_luma_inter_transform(tile, tables),
  }
}

fn read_luma_intra_transform(
  tile: &mut Rav1dTileStateContext,
  prediction: &Av1BlockIntra,
  tables: TransformTables<'_>,
) -> Rav1dResult<TxfmType> {
  let y_mode = match prediction.y_mode {
    FILTER_PRED => {
      let Ok(position) = console_core_number_conversion::wrapped_index(prediction.y_angle);
      let &filtered = dav1d_filter_mode_to_y_mode.get(position).ok_or(EINVAL)?;
      filtered
    }
    mode => mode,
  };
  let Ok(mode) = console_core_number_conversion::index(y_mode);
  let Ok(size) = console_core_number_conversion::index(tables.dimensions.min);
  let Ok(square16) = TxfmSize::S16x16.ordinal();
  let (symbol, first) = match (tables.set, tables.dimensions.min.cmp(&square16)) {
    (TransformSet::Reduced, _) | (TransformSet::Full, cmp::Ordering::Equal) => {
      let distribution = tile
        .cdf
        .m
        .txtp_intra2
        .get_mut(size)
        .and_then(|modes| modes.get_mut(mode))
        .ok_or(EINVAL)?;
      let Ok(decoded) = rav1d_msac_decode_symbol_adapt8(&mut tile.msac, distribution, 4);
      (decoded, 0)
    }
    (TransformSet::Full, cmp::Ordering::Less | cmp::Ordering::Greater) => {
      let distribution = tile
        .cdf
        .m
        .txtp_intra1
        .get_mut(size)
        .and_then(|modes| modes.get_mut(mode))
        .ok_or(EINVAL)?;
      let Ok(decoded) = rav1d_msac_decode_symbol_adapt8(&mut tile.msac, distribution, 6);
      (decoded, 5)
    }
  };
  let Ok(position) = console_core_number_conversion::index(symbol.wrapping_add(first));
  let &transform = dav1d_tx_types_per_set.get(position).ok_or(EINVAL)?;

  match tables.trace {
    BlockTrace::Enabled => console_core_logging::debug!(
      "Post-txtp-intra[{:?}->{}][{}][{}->{}]: r={}",
      tables.size,
      tables.dimensions.min,
      y_mode,
      symbol,
      transform,
      tile.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  Ok(transform)
}

fn read_luma_inter_transform(
  tile: &mut Rav1dTileStateContext,
  tables: TransformTables<'_>,
) -> Rav1dResult<TxfmType> {
  let Ok(square16) = TxfmSize::S16x16.ordinal();
  let Ok(square32) = TxfmSize::S32x32.ordinal();
  let mode = match (tables.set, tables.dimensions.max.cmp(&square32)) {
    (TransformSet::Reduced, _) | (TransformSet::Full, cmp::Ordering::Equal) => {
      TransformSet::Reduced
    }
    (TransformSet::Full, cmp::Ordering::Less | cmp::Ordering::Greater) => TransformSet::Full,
  };
  let Ok(size) = console_core_number_conversion::index(tables.dimensions.min);
  let (symbol, transform) = match (mode, tables.dimensions.min.cmp(&square16)) {
    (TransformSet::Reduced, _) => {
      let distribution = tile.cdf.m.txtp_inter3.get_mut(size).ok_or(EINVAL)?;
      let Ok(bit) = rav1d_msac_decode_bool_adapt(&mut tile.msac, distribution);

      match bit {
        EntropyBit::One => (1, DCT_DCT),
        EntropyBit::Zero => (0, IDTX),
      }
    }
    (TransformSet::Full, cmp::Ordering::Equal) => {
      let Ok(index) =
        rav1d_msac_decode_symbol_adapt16(&mut tile.msac, &mut *tile.cdf.m.txtp_inter2, 11);
      let Ok(position) = console_core_number_conversion::index(index.wrapping_add(12));
      let &selected = dav1d_tx_types_per_set.get(position).ok_or(EINVAL)?;
      (index, selected)
    }
    (TransformSet::Full, cmp::Ordering::Less | cmp::Ordering::Greater) => {
      let distribution = tile.cdf.m.txtp_inter1.get_mut(size).ok_or(EINVAL)?;
      let Ok(index) = rav1d_msac_decode_symbol_adapt16(&mut tile.msac, distribution, 15);
      let Ok(position) = console_core_number_conversion::index(index.wrapping_add(24));
      let &selected = dav1d_tx_types_per_set.get(position).ok_or(EINVAL)?;
      (index, selected)
    }
  };

  match tables.trace {
    BlockTrace::Enabled => console_core_logging::debug!(
      "Post-txtp-inter[{:?}->{}][{}->{}]: r={}",
      tables.size,
      tables.dimensions.min,
      symbol,
      transform,
      tile.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  Ok(transform)
}

struct EobProbabilityContext {
  plane: u8,
  dimensions: u8,
}

fn paired_eob_distribution<const N: usize>(
  distributions: &mut [[[u16; N]; 2]; 2],
  context: EobProbabilityContext,
) -> Rav1dResult<&mut [u16]> {
  let Ok(plane) = console_core_number_conversion::index(context.plane);
  let Ok(dimensions) = console_core_number_conversion::index(context.dimensions);
  distributions
    .get_mut(plane)
    .and_then(|choices| choices.get_mut(dimensions))
    .map(<[u16; N]>::as_mut_slice)
    .ok_or(EINVAL)
}

fn read_eob_bin(
  tile: &mut Rav1dTileStateContext,
  size: u8,
  context: EobProbabilityContext,
) -> Rav1dResult<u8> {
  let Ok(plane) = console_core_number_conversion::index(context.plane);
  let tables = &mut tile.cdf.coef;
  let distribution = match size {
    0 => paired_eob_distribution(&mut tables.eob_bin_16, context),
    1 => paired_eob_distribution(&mut tables.eob_bin_32, context),
    2 => paired_eob_distribution(&mut tables.eob_bin_64, context),
    3 => paired_eob_distribution(&mut tables.eob_bin_128, context),
    4 => paired_eob_distribution(&mut tables.eob_bin_256, context),
    5 => tables
      .eob_bin_512
      .get_mut(plane)
      .map(<[u16; 16]>::as_mut_slice)
      .ok_or(EINVAL),
    6 => tables
      .eob_bin_1024
      .get_mut(plane)
      .map(<[u16; 16]>::as_mut_slice)
      .ok_or(EINVAL),
    _ => Err(EINVAL),
  };
  let probabilities = distribution?;
  let alphabet = 4_u8.wrapping_add(size);
  let Ok(bin) = match size {
    0..=3 => rav1d_msac_decode_symbol_adapt8(&mut tile.msac, probabilities, alphabet),
    _ => rav1d_msac_decode_symbol_adapt16(&mut tile.msac, probabilities, alphabet),
  };
  Ok(bin)
}

fn read_eob(
  tile: &mut Rav1dTileStateContext,
  dimensions: &TxfmInfo,
  class: TxClass,
  plane: ResidualPlane,
  trace: BlockTrace,
) -> Rav1dResult<u16> {
  let Ok(square32) = TxfmSize::S32x32.ordinal();
  let size = dimensions
    .lw
    .min(square32)
    .wrapping_add(dimensions.lh.min(square32));
  let plane_index = match plane {
    ResidualPlane::Luma => 0_u8,
    ResidualPlane::Chroma(_) => 1,
  };
  let classes = u8::from(class != TxClass::TwoD);
  let bin = read_eob_bin(
    tile,
    size,
    EobProbabilityContext {
      plane: plane_index,
      dimensions: classes,
    },
  )?;

  match trace {
    BlockTrace::Enabled => console_core_logging::debug!(
      "Post-eob_bin_{}[{}][{}][{}]: r={}",
      16_u32.wrapping_shl(u32::from(size)),
      plane_index,
      classes,
      bin,
      tile.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  match bin {
    0..=1 => return Ok(u16::from(bin)),
    2.. => {}
  }

  let Ok(ctx) = console_core_number_conversion::index(dimensions.ctx);
  let Ok(channel) = console_core_number_conversion::index(plane_index);
  let Ok(bin_index) = console_core_number_conversion::index(bin);
  let distribution = tile
    .cdf
    .coef
    .eob_hi_bit
    .get_mut(ctx)
    .and_then(|contexts| contexts.get_mut(channel))
    .and_then(|bins| bins.get_mut(bin_index))
    .ok_or(EINVAL)?;
  let Ok(high) = rav1d_msac_decode_bool_adapt(&mut tile.msac, distribution);
  let high_bit = u16::from(matches!(high, EntropyBit::One));

  match trace {
    BlockTrace::Enabled => console_core_logging::debug!(
      "Post-eob_hi_bit[{}][{}][{}][{}]: r={}",
      dimensions.ctx,
      plane_index,
      bin,
      high_bit,
      tile.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  let remaining = bin.wrapping_sub(2);
  let Ok(tail) = rav1d_msac_decode_bools(&mut tile.msac, remaining);
  let Ok(low_bits) = console_core_number_conversion::wrapped::<_, u16>(tail);
  let end_of_block = (high_bit | 2).wrapping_shl(u32::from(remaining)) | low_bits;

  match trace {
    BlockTrace::Enabled => {
      console_core_logging::debug!("Post-eob[{}]: r={}", end_of_block, tile.msac.rng)
    }
    BlockTrace::Disabled => {}
  }

  Ok(end_of_block)
}

enum CoefficientScanOrder {
  TwoDimensional {
    positions: &'static [Scan],
    offsets: &'static [[u8; 5]; 5],
  },
  Horizontal,
  Vertical {
    transposed_shift: u32,
  },
}

struct CoefficientScan {
  order: CoefficientScanOrder,
  shift: u32,
  mask: u8,
  stride: u8,
  padded_rows: u8,
}

struct LocatedCoefficient {
  residue: u16,
  point: Point<u8>,
  level_offset: u16,
}

impl CoefficientScan {
  fn new(tx: TxfmSize, class: TxClass, dimensions: &TxfmInfo) -> Rav1dResult<Self> {
    let width = dimensions.w.min(8);
    let height = dimensions.h.min(8);
    let (order, shift, width_in_blocks, stride, height_in_blocks) = match class {
      TxClass::TwoD => {
        let Ok(transform) = tx.ordinal();
        let rectangle = match tx.shape() {
          Ok(crate::src::levels::TransformShape::Square) => 0u8,
          Ok(crate::src::levels::TransformShape::Rectangular) => 1u8,
        };
        let Ok(layout) =
          console_core_number_conversion::index(rectangle.wrapping_add(transform & rectangle));
        let Ok(scan) = console_core_number_conversion::index(transform);
        let offsets = dav1d_lo_ctx_offsets.get(layout).ok_or(EINVAL)?;
        let &positions = dav1d_scans.get(scan).ok_or(EINVAL)?;
        let order = CoefficientScanOrder::TwoDimensional { positions, offsets };
        (
          order,
          dimensions.lh.wrapping_add(2).min(5),
          height,
          height.wrapping_mul(4),
          width,
        )
      }
      TxClass::H => (
        CoefficientScanOrder::Horizontal,
        dimensions.lh.wrapping_add(2),
        height,
        16,
        height,
      ),
      TxClass::V => {
        let transposed_shift = u32::from(dimensions.lh).wrapping_add(2);
        let order = CoefficientScanOrder::Vertical { transposed_shift };
        (order, dimensions.lw.wrapping_add(2), width, 16, width)
      }
    };
    Ok(Self {
      order,
      shift: u32::from(shift),
      mask: width_in_blocks.wrapping_mul(4).wrapping_sub(1),
      stride,
      padded_rows: height_in_blocks.wrapping_mul(4).wrapping_add(2),
    })
  }

  fn locate(&self, scan_position: u16) -> Rav1dResult<LocatedCoefficient> {
    let (residue, point, level_offset) = match self.order {
      CoefficientScanOrder::TwoDimensional { positions, .. } => {
        let Ok(index) = console_core_number_conversion::index(scan_position);
        let entry = positions.get(index).ok_or(EINVAL)?;
        let Ok(residue) = entry.get();
        let Ok(x) =
          console_core_number_conversion::wrapped::<_, u8>(residue.wrapping_shr(self.shift));
        let Ok(column) = console_core_number_conversion::wrapped::<_, u8>(residue);
        (
          residue,
          Point {
            x,
            y: column & self.mask,
          },
          residue,
        )
      }
      CoefficientScanOrder::Horizontal | CoefficientScanOrder::Vertical { .. } => {
        let Ok(low) = console_core_number_conversion::wrapped::<_, u8>(scan_position);
        let x = low & self.mask;
        let Ok(y) = console_core_number_conversion::wrapped::<_, u8>(
          scan_position.wrapping_shr(self.shift),
        );
        let offset = u16::from(x)
          .wrapping_mul(u16::from(self.stride))
          .wrapping_add(u16::from(y));
        let residue = match self.order {
          CoefficientScanOrder::Vertical { transposed_shift } => {
            u16::from(x).wrapping_shl(transposed_shift) | u16::from(y)
          }
          CoefficientScanOrder::Horizontal | CoefficientScanOrder::TwoDimensional { .. } => {
            scan_position
          }
        };
        (residue, Point { x, y }, offset)
      }
    };

    match (point.x, point.y) {
      (0..=31, 0..=31) => Ok(LocatedCoefficient {
        residue,
        point,
        level_offset,
      }),
      _ => Err(EINVAL),
    }
  }

  fn high_context(&self, point: Point<u8>) -> Result<u8, console_core_never::Never> {
    Ok(match self.order {
      CoefficientScanOrder::TwoDimensional { .. } => match point.x | point.y {
        0..=1 => 7,
        2.. => 14,
      },
      CoefficientScanOrder::Horizontal | CoefficientScanOrder::Vertical { .. } => match point.y {
        0 => 7,
        1.. => 14,
      },
    })
  }

  fn base_context(
    &self,
    levels: &[u8],
    class: TxClass,
    point: Point<u8>,
  ) -> Rav1dResult<CoefficientBaseContext> {
    let offsets = match self.order {
      CoefficientScanOrder::TwoDimensional { offsets, .. } => Some(offsets),
      CoefficientScanOrder::Horizontal | CoefficientScanOrder::Vertical { .. } => None,
    };
    let mut magnitude = 0;
    let cdf_position = get_lo_ctx(levels, class, &mut magnitude, offsets, point, self.stride)?;
    Ok(CoefficientBaseContext {
      cdf_position,
      magnitude,
    })
  }

  fn dc_neighbor_magnitude(&self, levels: &[u8]) -> Rav1dResult<u32> {
    let Ok(below) = console_core_number_conversion::index(self.stride);
    let Ok(diagonal) = console_core_number_conversion::index(self.stride.wrapping_add(1));
    let right = levels.get(1).copied().ok_or(EINVAL)?;
    let bottom = levels.get(below).copied().ok_or(EINVAL)?;
    let corner = levels.get(diagonal).copied().ok_or(EINVAL)?;
    Ok(
      u32::from(right)
        .wrapping_add(u32::from(bottom))
        .wrapping_add(u32::from(corner)),
    )
  }

  fn clear_levels(&self, levels: &mut [u8]) -> Rav1dResult {
    let length = u16::from(self.stride).wrapping_mul(u16::from(self.padded_rows));
    let Ok(end) = console_core_number_conversion::index(length);
    let active = levels.get_mut(..end).ok_or(EINVAL)?;
    active.fill(0);
    Ok(())
  }
}

struct CoefficientBaseContext {
  cdf_position: u8,
  magnitude: u32,
}

struct CoefficientWindow<'a, BD: BitDepth>(&'a mut [BD::Coef]);

impl<BD: BitDepth> CoefficientWindow<'_, BD> {
  fn position(&self, residue: u16) -> Result<u64, console_core_never::Never> {
    let Ok(length) = console_core_number_conversion::fixed_index(self.0.len());
    Ok(u64::from(residue) & length.wrapping_sub(1))
  }

  fn get(&self, residue: u16) -> Rav1dResult<i32> {
    let Ok(position) = self.position(residue);
    let Ok(index) = console_core_number_conversion::index(position);
    self
      .0
      .get(index)
      .map(|&value| <BD::Coef as Into<i32>>::into(value))
      .ok_or(EINVAL)
  }

  fn set<T: ToPrimitive<BD::Coef>>(&mut self, residue: u16, value: T) -> Rav1dResult {
    let Ok(position) = self.position(residue);
    let Ok(index) = console_core_number_conversion::index(position);
    let coefficient = self.0.get_mut(index).ok_or(EINVAL)?;
    *coefficient = value.as_();
    Ok(())
  }
}

struct CoefficientScanRequest<'a> {
  dimensions: &'a TxfmInfo,
  transform: TxfmSize,
  chroma: u8,
  end_of_block: u16,
  trace: BlockTrace,
}

#[derive(Clone, Copy)]
struct CoefficientTokenContext {
  scan_position: u16,
  residue: u16,
  cdf_position: u8,
}

struct CoefficientToken {
  magnitude: u8,
  level: u8,
}

struct CoefficientTokens<'a> {
  entropy: &'a mut MsacContext,
  end_of_block: &'a mut [[u16; 4]; 4],
  base: &'a mut [[u16; 4]; 41],
  range: &'a mut [[u16; 4]; 21],
  regular_context: u8,
  high_context: u8,
  chroma: u8,
  trace: BlockTrace,
}

impl<'a> CoefficientTokens<'a> {
  fn new(
    tile: &'a mut Rav1dTileStateContext,
    request: &CoefficientScanRequest<'_>,
  ) -> Rav1dResult<Self> {
    let regular_context = request.dimensions.ctx;
    let high_context = regular_context.min(3);
    let Ok(regular) = console_core_number_conversion::index(regular_context);
    let Ok(high) = console_core_number_conversion::index(high_context);
    let Ok(chroma) = console_core_number_conversion::index(request.chroma);
    let distributions = &mut tile.cdf.coef;
    let end_of_block = distributions
      .eob_base_tok
      .get_mut(regular)
      .and_then(|row| row.get_mut(chroma))
      .ok_or(EINVAL)?;
    let base = distributions
      .base_tok
      .get_mut(regular)
      .and_then(|row| row.get_mut(chroma))
      .ok_or(EINVAL)?;
    let range = distributions
      .br_tok
      .get_mut(high)
      .and_then(|row| row.get_mut(chroma))
      .ok_or(EINVAL)?;
    Ok(Self {
      entropy: &mut tile.msac,
      end_of_block,
      base,
      range,
      regular_context,
      high_context,
      chroma: request.chroma,
      trace: request.trace,
    })
  }

  fn read_last_base(&mut self, request: &CoefficientScanRequest<'_>) -> Rav1dResult<(u8, u8)> {
    let Ok(limit) = TxfmSize::S32x32.ordinal();
    let dims = request.dimensions;
    let shift = u32::from(dims.lw.min(limit).wrapping_add(dims.lh.min(limit)));
    let eob = request.end_of_block;
    let context = 1u8
      .wrapping_add(u8::from(eob > 2u16.wrapping_shl(shift)))
      .wrapping_add(u8::from(eob > 4u16.wrapping_shl(shift)));
    let code = self.read_terminal_base(context)?;
    Ok((context, code))
  }

  fn read_terminal_base(&mut self, context: u8) -> Rav1dResult<u8> {
    let Ok(position) = console_core_number_conversion::index(context);
    let distribution = self.end_of_block.get_mut(position).ok_or(EINVAL)?;
    let Ok(code) = rav1d_msac_decode_symbol_adapt4(self.entropy, distribution, 2);
    Ok(code.wrapping_add(1))
  }

  fn read_dc_only(&mut self) -> Rav1dResult<u32> {
    let low = self.read_terminal_base(0)?;

    match self.trace {
      BlockTrace::Enabled => console_core_logging::trace!(
        "Post-dc_lo_tok[{}][{}][0][{}]: r={}",
        self.regular_context,
        self.chroma,
        low,
        self.entropy.rng,
      ),
      BlockTrace::Disabled => {}
    }

    match low {
      1..=2 => Ok(u32::from(low)),
      3 => {
        let high = self.read_range(0)?;

        match self.trace {
          BlockTrace::Enabled => console_core_logging::trace!(
            "Post-dc_hi_tok[{}][{}][0][{}]: r={}",
            self.high_context,
            self.chroma,
            high,
            self.entropy.rng,
          ),
          BlockTrace::Disabled => {}
        }

        Ok(u32::from(high))
      }
      0 | 4.. => Err(EINVAL),
    }
  }

  fn read_base(&mut self, context: u8) -> Rav1dResult<u8> {
    let Ok(position) = console_core_number_conversion::index(context);
    let distribution = self.base.get_mut(position).ok_or(EINVAL)?;
    let Ok(code) = rav1d_msac_decode_symbol_adapt4(self.entropy, distribution, 3);
    Ok(code)
  }

  fn read_range(&mut self, context: u8) -> Rav1dResult<u8> {
    let Ok(position) = console_core_number_conversion::index(context);
    let distribution = self.range.get_mut(position).ok_or(EINVAL)?;
    let Ok(code) = rav1d_msac_decode_hi_tok(self.entropy, distribution);
    Ok(code)
  }

  fn trace_base(
    &self,
    location: CoefficientTokenContext,
    token: u8,
  ) -> Result<(), console_core_never::Never> {
    match self.trace {
      BlockTrace::Enabled => console_core_logging::trace!(
        "Post-lo_tok[{}][{}][{}][{}={}={}]: r={}",
        self.regular_context,
        self.chroma,
        location.cdf_position,
        location.scan_position,
        location.residue,
        token,
        self.entropy.rng
      ),
      BlockTrace::Disabled => {}
    }

    Ok(())
  }

  fn read_non_dc(
    &mut self,
    low: u8,
    high: CoefficientTokenContext,
  ) -> Rav1dResult<CoefficientToken> {
    match low {
      3 => {
        let magnitude = self.read_range(high.cdf_position)?;
        let level = magnitude.wrapping_add(3u8.wrapping_shl(6));

        match self.trace {
          BlockTrace::Enabled => console_core_logging::trace!(
            "Post-hi_tok[{}][{}][{}][{}={}={}]: r={}",
            self.high_context,
            self.chroma,
            high.cdf_position,
            high.scan_position,
            high.residue,
            magnitude,
            self.entropy.rng
          ),
          BlockTrace::Disabled => {}
        }

        Ok(CoefficientToken { magnitude, level })
      }
      0..=2 => Ok(CoefficientToken {
        magnitude: low,
        level: low.wrapping_mul(0x41),
      }),
      4.. => Err(EINVAL),
    }
  }

  fn read_dc(&mut self, neighbors: CoefficientNeighbors<'_>, class: TxClass) -> Rav1dResult<u32> {
    let CoefficientNeighbors { layout, levels } = neighbors;
    let mut magnitude = 0;
    let context = match class {
      TxClass::TwoD => 0,
      TxClass::H | TxClass::V => {
        let context = get_lo_ctx(
          levels,
          class,
          &mut magnitude,
          None,
          Point { x: 0, y: 0 },
          layout.stride,
        )?;
        context
      }
    };
    let low = self.read_base(context)?;

    match self.trace {
      BlockTrace::Enabled => console_core_logging::trace!(
        "Post-dc_lo_tok[{}][{}][{}][{}]: r={}",
        self.regular_context,
        self.chroma,
        context,
        low,
        self.entropy.rng
      ),
      BlockTrace::Disabled => {}
    }

    match low {
      3 => {
        let dc_neighbors = match class {
          TxClass::TwoD => layout.dc_neighbor_magnitude(levels)?,
          TxClass::H | TxClass::V => magnitude,
        };
        let Ok(range_context) = coefficient_high_offset(dc_neighbors);
        let code = self.read_range(range_context)?;

        match self.trace {
          BlockTrace::Enabled => console_core_logging::trace!(
            "Post-dc_hi_tok[{}][{}][0][{}]: r={}",
            self.high_context,
            self.chroma,
            code,
            self.entropy.rng
          ),
          BlockTrace::Disabled => {}
        }

        Ok(u32::from(code))
      }
      0..=2 => Ok(u32::from(low)),
      4.. => Err(EINVAL),
    }
  }
}

struct CoefficientNeighbors<'a> {
  layout: &'a CoefficientScan,
  levels: &'a [u8],
}

fn coefficient_high_offset(magnitude: u32) -> Result<u8, console_core_never::Never> {
  let Ok(low_six) = console_core_number_conversion::wrapped::<_, u8>(magnitude & 63);
  Ok(match low_six {
    0..=12 => low_six.wrapping_add(1).wrapping_shr(1),
    13.. => 6,
  })
}

#[inline]
fn decode_coefs_class<const TX_CLASS: u8, BD: BitDepth>(
  tile: &mut Rav1dTileStateContext,
  scratch: &mut TaskContextScratch,
  request: &CoefficientScanRequest<'_>,
  coefficients: &mut CoefficientWindow<'_, BD>,
) -> Rav1dResult<(u16, u32)> {
  let class = const { TxClass::from_repr(TX_CLASS) }.ok_or(EINVAL)?;
  let eob = request.end_of_block;
  let mut reader = CoefficientTokens::new(tile, request)?;
  let intra = scratch.inter_intra_mut()?;
  let Ok(levels) = intra.levels_pal.levels_mut();
  let (last_context, last_base) = reader.read_last_base(request)?;
  let layout = CoefficientScan::new(request.transform, class, request.dimensions)?;
  layout.clear_levels(levels)?;
  let last = layout.locate(eob)?;
  let mut link = last.residue;
  let last_base_context = CoefficientTokenContext {
    scan_position: eob,
    residue: link,
    cdf_position: last_context,
  };
  let Ok(()) = reader.trace_base(last_base_context, last_base);
  let Ok(last_high_context) = layout.high_context(last.point);
  let last_range_context = CoefficientTokenContext {
    cdf_position: last_high_context,
    ..last_base_context
  };
  let last_token = reader.read_non_dc(last_base, last_range_context)?;
  coefficients.set(link, i16::from(last_token.magnitude).wrapping_shl(11))?;
  let Ok(last_index) = console_core_number_conversion::index(last.level_offset);
  let last_level = levels.get_mut(last_index).ok_or(EINVAL)?;
  *last_level = last_token.level;

  for scan_position in (1..eob).rev() {
    let position = layout.locate(scan_position)?;
    let Ok(level_index) = console_core_number_conversion::index(position.level_offset);
    let level = levels.get_mut(level_index..).ok_or(EINVAL)?;
    let base = layout.base_context(level, class, position.point)?;
    let low = reader.read_base(base.cdf_position)?;
    let base_location = CoefficientTokenContext {
      scan_position,
      residue: position.residue,
      cdf_position: base.cdf_position,
    };
    let Ok(()) = reader.trace_base(base_location, low);
    let Ok(offset) = coefficient_high_offset(base.magnitude);
    let Ok(high_context) = layout.high_context(position.point);
    let range_location = CoefficientTokenContext {
      cdf_position: high_context.wrapping_add(offset),
      ..base_location
    };
    let token = reader.read_non_dc(low, range_location)?;
    let current = level.first_mut().ok_or(EINVAL)?;
    *current = token.level;
    let packed = match token.magnitude {
      0 => 0,
      magnitude => {
        let previous = link;
        link = position.residue;
        u16::from(magnitude).wrapping_shl(11) | previous
      }
    };
    coefficients.set(position.residue, packed)?;
  }

  let neighbors = CoefficientNeighbors {
    layout: &layout,
    levels,
  };
  let direct = reader.read_dc(neighbors, class)?;
  Ok((link, direct))
}

fn read_coefficient_skip(
  tile: &mut Rav1dTileStateContext,
  dimensions: &TxfmInfo,
  size: BlockSize,
  neighbors: ResidualNeighbors<'_>,
  plane: ResidualPlane,
  trace: BlockTrace,
) -> Rav1dResult<EntropyBit> {
  let context = get_skip_ctx(dimensions, size, neighbors, plane)?;
  let Ok(transform_position) = console_core_number_conversion::index(dimensions.ctx);
  let Ok(skip_code) = context.get();
  let Ok(skip_position) = console_core_number_conversion::index(skip_code);
  let distribution = tile
    .cdf
    .coef
    .skip
    .get_mut(transform_position)
    .and_then(|contexts| contexts.get_mut(skip_position))
    .ok_or(EINVAL)?;
  let Ok(skip) = rav1d_msac_decode_bool_adapt(&mut tile.msac, distribution);

  match trace {
    BlockTrace::Enabled => console_core_logging::trace!(
      "Post-non-zero[{}][{}][{}]: r={}",
      dimensions.ctx,
      context,
      matches!(skip, EntropyBit::One),
      tile.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  Ok(skip)
}

struct DirectCoefficientContext<'a> {
  transform: TxfmSize,
  neighbors: ResidualNeighbors<'a>,
  chroma: u8,
  trace: BlockTrace,
}

struct CoefficientDequantization<'a> {
  factor: &'a RelaxedAtomic<u16>,
  matrix: Option<&'a [u8]>,
  shift: u32,
  limit: i32,
}

struct DequantizedCoefficient {
  value: i32,
  level: u32,
  residual_sign: u32,
}

enum CoefficientMagnitude {
  Base(u32),
  Golomb(u32),
}

fn read_direct_sign(
  tile: &mut Rav1dTileStateContext,
  context: DirectCoefficientContext<'_>,
) -> Rav1dResult<EntropyBit> {
  let sign_context = get_dc_sign_ctx(
    context.transform,
    context.neighbors.above,
    context.neighbors.left,
  )?;
  let Ok(plane_position) = console_core_number_conversion::index(context.chroma);
  let Ok(sign_position) = console_core_number_conversion::index(sign_context);
  let cdf = tile
    .cdf
    .coef
    .dc_sign
    .get_mut(plane_position)
    .and_then(|contexts| contexts.get_mut(sign_position))
    .ok_or(EINVAL)?;
  let Ok(sign) = rav1d_msac_decode_bool_adapt(&mut tile.msac, cdf);

  match context.trace {
    BlockTrace::Enabled => console_core_logging::trace!(
      "Post-dc_sign[{}][{}][{}]: r={}",
      context.chroma,
      sign_context,
      u8::from(matches!(sign, EntropyBit::One)),
      tile.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  Ok(sign)
}

fn read_direct_coefficient(
  tile: &mut Rav1dTileStateContext,
  context: DirectCoefficientContext<'_>,
  quantization: CoefficientDequantization<'_>,
  token: u32,
) -> Rav1dResult<DequantizedCoefficient> {
  let trace = context.trace;
  let sign = read_direct_sign(tile, context)?;
  let Ok(base) = quantization.factor.get();
  let factor = quantization.factor_at(u32::from(base), 0)?;
  let magnitude = match token {
    15 => {
      let Ok(remainder) = read_golomb(&mut tile.msac);
      let expanded = remainder.wrapping_add(15);

      match trace {
        BlockTrace::Enabled => console_core_logging::trace!(
          "Post-dc_residual[{}->{}]: r={}",
          expanded.wrapping_sub(15),
          expanded,
          tile.msac.rng,
        ),
        BlockTrace::Disabled => {}
      }

      CoefficientMagnitude::Golomb(expanded & 0xfffff)
    }
    value => CoefficientMagnitude::Base(value),
  };
  quantization.signed_value(factor, magnitude, sign)
}

impl CoefficientDequantization<'_> {
  fn factor_at(&self, base: u32, residue: u16) -> Rav1dResult<u32> {
    match self.matrix {
      Some(weights) => {
        let Ok(position) = console_core_number_conversion::index(residue);
        let weight = weights.get(position).copied().ok_or(EINVAL)?;
        Ok(
          base
            .wrapping_mul(u32::from(weight))
            .wrapping_add(16)
            .wrapping_shr(5),
        )
      }
      None => Ok(base),
    }
  }

  fn signed_value(
    &self,
    factor: u32,
    magnitude: CoefficientMagnitude,
    sign: EntropyBit,
  ) -> Rav1dResult<DequantizedCoefficient> {
    let sign_bit = i32::from(matches!(sign, EntropyBit::One));
    let limit = self.limit.wrapping_add(sign_bit);
    let (level, scaled) = match magnitude {
      CoefficientMagnitude::Golomb(level) => {
        let product = factor.wrapping_mul(level) & 0xffffff;
        let Ok(value) =
          console_core_number_conversion::wrapped::<_, i32>(product.wrapping_shr(self.shift));
        (level, value.min(limit))
      }
      CoefficientMagnitude::Base(level) => {
        let product = factor.wrapping_mul(level);
        let candidate = product.wrapping_shr(self.shift);
        let value = i32::try_from(candidate).map_err(|_outside_coefficient| EINVAL)?;
        let valid = match self.matrix {
          Some(_) => match product {
            0..=0xffffff => value.min(limit),
            0x1000000.. => return Err(EINVAL),
          },
          None => match value.cmp(&self.limit) {
            cmp::Ordering::Greater => return Err(EINVAL),
            cmp::Ordering::Equal | cmp::Ordering::Less => value,
          },
        };
        (level, valid)
      }
    };
    let (value, residual_sign) = match sign {
      EntropyBit::Zero => (scaled, 0x80),
      EntropyBit::One => (scaled.wrapping_neg(), 0),
    };
    Ok(DequantizedCoefficient {
      value,
      level,
      residual_sign,
    })
  }
}

struct AcTraversal {
  first: u16,
  length: u64,
  trace: BlockTrace,
}

fn dequantize_ac<BD: BitDepth>(
  tile: &mut Rav1dTileStateContext,
  coefficients: &mut CoefficientWindow<'_, BD>,
  quantization: CoefficientDequantization<'_>,
  traversal: AcTraversal,
) -> Rav1dResult<u32> {
  let Ok(base) = quantization.factor.get();
  let mut residue = traversal.first;
  let mut level = 0u32;

  for _ in 0..traversal.length {
    let Ok(sign) = rav1d_msac_decode_bool_equi(&mut tile.msac);

    match traversal.trace {
      BlockTrace::Enabled => console_core_logging::trace!(
        "Post-sign[{}={}]: r={}",
        residue,
        u8::from(matches!(sign, EntropyBit::One)),
        tile.msac.rng
      ),
      BlockTrace::Disabled => {}
    }

    let packed = coefficients.get(residue)?;
    let Ok(encoded) = console_core_number_conversion::wrapped::<_, u32>(packed);
    let factor = quantization.factor_at(u32::from(base), residue)?;
    let magnitude = match encoded.wrapping_shr(11) {
      direct @ 0..=14 => CoefficientMagnitude::Base(direct),
      15.. => {
        let Ok(remainder) = read_golomb(&mut tile.msac);
        let expanded = remainder.wrapping_add(15);

        match traversal.trace {
          BlockTrace::Enabled => console_core_logging::trace!(
            "Post-residual[{}={}->{}]: r={}",
            residue,
            remainder,
            expanded,
            tile.msac.rng,
          ),
          BlockTrace::Disabled => {}
        }

        CoefficientMagnitude::Golomb(expanded & 0xfffff)
      }
    };
    let coefficient = quantization.signed_value(factor, magnitude, sign)?;
    level = level.wrapping_add(coefficient.level);
    coefficients.set(residue, coefficient.value)?;
    let Ok(link) = console_core_number_conversion::wrapped::<_, u16>(encoded & 0x3ff);
    residue = link;

    match residue {
      0 => return Ok(level),
      _ => {}
    }
  }

  Err(EINVAL)
}

struct CoefficientChain {
  first_ac: u16,
  dc: u32,
}

fn read_coefficient_chain<BD: BitDepth>(
  tile: &mut Rav1dTileStateContext,
  scratch: &mut TaskContextScratch,
  request: &CoefficientScanRequest<'_>,
  class: TxClass,
  coefficients: &mut CoefficientWindow<'_, BD>,
) -> Rav1dResult<CoefficientChain> {
  let (first_ac, dc) = match request.end_of_block {
    0 => {
      let mut tokens = CoefficientTokens::new(tile, request)?;
      let token = tokens.read_dc_only()?;
      (0, token)
    }
    1.. => {
      let result = match class {
        TxClass::TwoD => decode_coefs_class::<
          {
            let Ok(ordinal) = TxClass::TwoD.ordinal();
            ordinal
          },
          BD,
        >(tile, scratch, request, coefficients),
        TxClass::H => decode_coefs_class::<
          {
            let Ok(ordinal) = TxClass::H.ordinal();
            ordinal
          },
          BD,
        >(tile, scratch, request, coefficients),
        TxClass::V => decode_coefs_class::<
          {
            let Ok(ordinal) = TxClass::V.ordinal();
            ordinal
          },
          BD,
        >(tile, scratch, request, coefficients),
      };
      result?
    }
  };
  Ok(CoefficientChain { first_ac, dc })
}

struct DequantizationSelection<'a> {
  dimensions: &'a TxfmInfo,
  input: TxfmSize,
  output: TxfmType,
  segment: u8,
  plane: u8,
}

struct ResidualQuantization<'a> {
  direct: CoefficientDequantization<'a>,
  alternating: CoefficientDequantization<'a>,
}

fn select_dequantization<'a, BD: BitDepth>(
  frame: &'a Rav1dFrameData,
  tile: &'a Rav1dTileState,
  selection: DequantizationSelection<'_>,
) -> Rav1dResult<ResidualQuantization<'a>> {
  let Ok(get) = tile.dq.get();
  let dq = match get {
    TileStateRef::Frame => &frame.dq,
    TileStateRef::Local => &tile.dqmem,
  };
  let Ok(plane_index) = console_core_number_conversion::index(selection.plane);
  let Ok(segment_index) = console_core_number_conversion::index(selection.segment);
  let dq_tbl = dq
    .get(segment_index)
    .and_then(|table| table.get(plane_index))
    .ok_or(EINVAL)?;
  let qm_tbl = match ::core::primitive::bool::then_some(selection.output < IDTX, ()) {
    Some(()) => {
      let Ok(transform_code) = selection.input.ordinal();
      let Ok(transform_index) = console_core_number_conversion::index(transform_code);
      ::core::result::Result::<&_, _>::copied(
        frame
          .qm
          .get(transform_index)
          .and_then(|table| table.get(plane_index))
          .ok_or(EINVAL),
      )?
    }
    None => None,
  };
  let dq_shift = u32::from(selection.dimensions.ctx.saturating_sub(2));
  let pixel_depth = match BD::BPC {
    BPC::BPC8 => 8,
    BPC::BPC16 => frame.cur.p.bpc,
  };
  let Ok(cf_max) = console_core_number_conversion::wrapped::<_, i32>(
    !(!127u32).wrapping_shl(u32::from(pixel_depth)),
  );
  let [direct, alternating] = dq_tbl;
  Ok(ResidualQuantization {
    direct: CoefficientDequantization {
      factor: direct,
      matrix: qm_tbl,
      shift: dq_shift,
      limit: cf_max,
    },
    alternating: CoefficientDequantization {
      factor: alternating,
      matrix: qm_tbl,
      shift: dq_shift,
      limit: cf_max,
    },
  })
}

fn dequantize_coefficients<BD: BitDepth>(
  tile: &mut Rav1dTileStateContext,
  cf: &mut CoefficientWindow<'_, BD>,
  context: DirectCoefficientContext<'_>,
  quantization: ResidualQuantization<'_>,
  chain: CoefficientChain,
  count: u64,
) -> Rav1dResult<u8> {
  let trace = context.trace;
  let (direct_level, dc_sign_level) = match chain.dc {
    0 => (0, 0x40),
    1.. => {
      let coefficient = read_direct_coefficient(tile, context, quantization.direct, chain.dc)?;
      cf.set(0, coefficient.value)?;
      (coefficient.level, coefficient.residual_sign)
    }
  };

  let ac_level = match (chain.dc, chain.first_ac) {
    (1.., 0) => 0,
    (_, _) => {
      let traversal = AcTraversal {
        first: chain.first_ac,
        length: count,
        trace,
      };
      let level = dequantize_ac(tile, cf, quantization.alternating, traversal)?;
      level
    }
  };
  let cul_level = direct_level.wrapping_add(ac_level);

  let value = cul_level.min(63) | dc_sign_level;
  let Ok(residual_context) = console_core_number_conversion::wrapped::<_, u8>(value);
  Ok(residual_context)
}

struct EncodedCoefficientHeader<'a> {
  scan: CoefficientScanRequest<'static>,
  neighbors: ResidualNeighbors<'a>,
  segment: u8,
  class: TxClass,
  output: TxfmType,
}

enum CoefficientHeader<'a> {
  Skipped(TxfmType),
  Encoded(EncodedCoefficientHeader<'a>),
}

fn read_coefficient_header<'a>(
  f: &Rav1dFrameData,
  ts_c: &mut Rav1dTileStateContext,
  block: &CoefficientBlock<'a>,
  luma_transform: TxfmType,
) -> Rav1dResult<CoefficientHeader<'a>> {
  let &CoefficientBlock {
    trace: dbg_block_info,
    above: above_context,
    left: left_context,
    transform: tx,
    size: bs,
    syntax: b,
    plane,
    ..
  } = block;
  let chroma = u8::from(plane != 0);
  let frame_hdr = f.frame_hdr()?;
  let Ok(segment) = b.seg_id.get();
  let Ok(segment_at) = console_core_number_conversion::index(segment);
  let segment_modes = &frame_hdr.segmentation.lossless;
  let &lossless = segment_modes.get(segment_at).ok_or(EINVAL)?;
  let t_dim = coefficient_dimensions(tx)?;
  let Ok(width) = console_core_number_conversion::index(t_dim.w);
  let Ok(height) = console_core_number_conversion::index(t_dim.h);
  let a = above_context.get(..width).ok_or(EINVAL)?;
  let l = left_context.get(..height).ok_or(EINVAL)?;
  let dbg = match (dbg_block_info, plane, COEFFICIENT_TRACE) {
    (BlockTrace::Enabled, 1.., BlockTrace::Enabled) => BlockTrace::Enabled,
    _ => BlockTrace::Disabled,
  };

  match dbg {
    BlockTrace::Enabled => {
      console_core_logging::trace!("Start: r={}", ts_c.msac.rng);
    }
    BlockTrace::Disabled => {}
  }

  let residual_plane = match plane {
    0 => ResidualPlane::Luma,
    _ => ResidualPlane::Chroma(f.cur.p.layout),
  };
  let skip = read_coefficient_skip(
    ts_c,
    t_dim,
    bs,
    ResidualNeighbors { above: a, left: l },
    residual_plane,
    dbg,
  )?;
  let quantization = match lossless.then_some(()) {
    Some(()) => CoefficientQuantization::Lossless,
    None => CoefficientQuantization::Quantized,
  };

  match skip {
    EntropyBit::One => {
      let transform = match quantization {
        CoefficientQuantization::Lossless => WHT_WHT,
        CoefficientQuantization::Quantized => DCT_DCT,
      };
      return Ok(CoefficientHeader::Skipped(transform));
    }
    EntropyBit::Zero => {}
  }

  let decoded_transform = read_block_transform(
    ts_c,
    frame_hdr,
    TransformRequest {
      block: b,
      dimensions: t_dim,
      size: tx,
      plane: residual_plane,
      quantization,
      luma_transform,
      trace: dbg,
    },
  )?;

  let Ok(transform) = console_core_number_conversion::index(decoded_transform);
  let &tx_class = dav1d_tx_type_class.get(transform).ok_or(EINVAL)?;
  let eob = read_eob(ts_c, t_dim, tx_class, residual_plane, dbg)?;
  Ok(CoefficientHeader::Encoded(EncodedCoefficientHeader {
    scan: CoefficientScanRequest {
      dimensions: t_dim,
      transform: tx,
      chroma,
      end_of_block: eob,
      trace: dbg,
    },
    neighbors: ResidualNeighbors { above: a, left: l },
    segment,
    class: tx_class,
    output: decoded_transform,
  }))
}

fn decode_coefs<BD: BitDepth>(
  f: &Rav1dFrameData,
  CoefficientDecoder {
    tile: ts,
    state: ts_c,
    scratch,
    coefficients: t_cf,
  }: CoefficientDecoder<'_>,
  block: CoefficientBlock<'_>,
  CoefficientOutput {
    transform: txtp,
    residual_context: res_ctx,
  }: CoefficientOutput<'_>,
) -> Rav1dResult<c_int> {
  Ok({
    let header = read_coefficient_header(f, ts_c, &block, *txtp)?;

    let EncodedCoefficientHeader {
      scan,
      neighbors,
      segment,
      class,
      output,
    } = match header {
      CoefficientHeader::Skipped(transform) => {
        *res_ctx = 0x40;
        *txtp = transform;
        return Ok(-1);
      }
      CoefficientHeader::Encoded(fields) => fields,
    };
    *txtp = output;

    let sw = 1u64.wrapping_shl(u32::from(scan.dimensions.lw)).min(8);
    let sh = 1u64.wrapping_shl(u32::from(scan.dimensions.lh)).min(8);
    let coefficient_count = sw.wrapping_mul(4).wrapping_mul(sh).wrapping_mul(4);
    let Ok(cf_len) = console_core_number_conversion::index(coefficient_count);
    let coefficient_slice = match block.storage {
      CfSelect::Frame(offset) => &mut *f.frame_thread.cf.mut_slice_as((
        ({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(offset);
          numeric_value
        })..,
        ..cf_len,
      )),
      CfSelect::Task => {
        let explicit_value = t_cf.select_mut::<BD>()?;
        explicit_value
      }
    };
    let mut cf = CoefficientWindow::<BD>(coefficient_slice);
    let chain = read_coefficient_chain(ts_c, scratch, &scan, class, &mut cf)?;
    let selection = DequantizationSelection {
      dimensions: scan.dimensions,
      input: scan.transform,
      output,
      segment,
      plane: block.plane,
    };
    let dequantization = select_dequantization::<BD>(f, ts, selection)?;
    let context = DirectCoefficientContext {
      transform: scan.transform,
      neighbors,
      chroma: scan.chroma,
      trace: scan.trace,
    };
    let residual = dequantize_coefficients(
      ts_c,
      &mut cf,
      context,
      dequantization,
      chain,
      coefficient_count,
    )?;
    *res_ctx = residual;

    {
      let Ok(numeric_value) =
        console_core_number_conversion::wrapped::<_, i32>(scan.end_of_block);
      numeric_value
    }
  })
}

#[derive(Clone, Copy)]
enum CfSelect {
  Frame(u32),

  Task,
}

struct LumaCoefficientTree<'a> {
  size: BlockSize,
  syntax: &'a Av1Block,
  transform: TxfmSize,
  splits: [u16; 2],
  split_cell: Point<i32>,
  output: Option<PicOffset<'a>>,
}

#[derive(Clone, Copy)]
struct CoefficientNode<'a> {
  position: Bxy,
  transform: TxfmSize,
  split_cell: Point<i32>,
  output: Option<PicOffset<'a>>,
}

fn coefficient_dimensions(transform: TxfmSize) -> Rav1dResult<&'static TxfmInfo> {
  let Ok(ordinal) = transform.ordinal();
  let Ok(index) = console_core_number_conversion::index(ordinal);
  dav1d_txfm_dimensions.get(index).ok_or(EINVAL)
}

impl<'a> CoefficientNode<'a> {
  fn children<BD: BitDepth>(
    self,
    frame: Size<i32>,
    split_mask: u16,
  ) -> Rav1dResult<Option<[Option<Self>; 4]>> {
    let bit_index = self
      .split_cell
      .y
      .wrapping_mul(4)
      .wrapping_add(self.split_cell.x);
    let Ok(shift) = console_core_number_conversion::wrapped::<_, u32>(bit_index);

    match (split_mask & 1_u16.wrapping_shl(shift) != 0).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    let dimensions = coefficient_dimensions(self.transform)?;
    let sub = coefficient_dimensions(dimensions.sub)?;
    let x_step = i32::from(sub.w);
    let y_step = i32::from(sub.h);
    let right = self.position.x.wrapping_add(x_step);
    let bottom = self.position.y.wrapping_add(y_step);
    let has_right = dimensions.w >= dimensions.h && right < frame.width;
    let has_bottom = dimensions.h >= dimensions.w && bottom < frame.height;
    let split_x = self.split_cell.x.wrapping_mul(2);
    let split_y = self.split_cell.y.wrapping_mul(2);

    let make_child = |offset: Point<i32>| {
      let position = Bxy {
        x: self.position.x.wrapping_add(offset.x.wrapping_mul(x_step)),
        y: self.position.y.wrapping_add(offset.y.wrapping_mul(y_step)),
      };
      let split_cell = Point {
        x: split_x.wrapping_add(offset.x),
        y: split_y.wrapping_add(offset.y),
      };
      let output = self.output.map(|dst| {
        let Ok(pitch) = dst.fixed_pixel_stride::<BD>();
        let x = i64::from(offset.x).wrapping_mul(i64::from(x_step));
        let y = i64::from(offset.y)
          .wrapping_mul(i64::from(y_step))
          .wrapping_mul(pitch);
        let pixels = x.wrapping_add(y).wrapping_mul(4);
        let Ok(displacement) = console_core_number_conversion::wrapped_offset(pixels);
        dst + displacement
      });
      Self {
        position,
        transform: dimensions.sub,
        split_cell,
        output,
      }
    };

    let top_left = Some(make_child(Point { x: 0, y: 0 }));
    let top_right = has_right.then(|| make_child(Point { x: 1, y: 0 }));
    let bottom_left = has_bottom.then(|| make_child(Point { x: 0, y: 1 }));
    let bottom_right = (has_right && has_bottom).then(|| make_child(Point { x: 1, y: 1 }));
    Ok(Some([top_left, top_right, bottom_left, bottom_right]))
  }
}

fn read_coef_tree<BD: BitDepth>(
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
  mut ts_c: Option<&mut Rav1dTileStateContext>,
  tree: LumaCoefficientTree<'_>,
) -> Rav1dResult {
  let root = CoefficientNode {
    position: t.b,
    transform: tree.transform,
    split_cell: tree.split_cell,
    output: tree.output,
  };
  let frame = Size {
    width: f.bw,
    height: f.bh,
  };
  let [root_split, child_split] = tree.splits;
  let children = root.children::<BD>(frame, root_split)?;

  match children {
    Some(children_2) => {
      for child in children_2.into_iter().flatten() {
        let grandchildren = child.children::<BD>(frame, child_split)?;
        let leaves = match grandchildren {
          Some(nodes) => nodes,
          None => [Some(child), None, None, None],
        };

        for leaf in leaves.into_iter().flatten() {
          read_coef_leaf::<BD>(f, t, ts_c.as_deref_mut(), &tree, leaf)?;
        }
      }
    }
    None => read_coef_leaf::<BD>(f, t, ts_c, &tree, root)?,
  }

  t.b = root.position;
  Ok(())
}

fn read_coef_leaf<BD: BitDepth>(
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
  ts_c: Option<&mut Rav1dTileStateContext>,
  tree: &LumaCoefficientTree<'_>,
  leaf: CoefficientNode<'_>,
) -> Rav1dResult {
  t.b = leaf.position;
  let tile = f.tile(t.tile_index)?;
  let transform = leaf.transform;
  let dim = coefficient_dimensions(transform)?;
  let width = u32::from(dim.w);
  let height = u32::from(dim.h);
  let coefficient_count = width.min(8).wrapping_mul(height.min(8)).wrapping_mul(16);
  let [reconstruct, entropy] = &tile.frame_thread;
  let frame_pass = match t.frame_thread.pass {
    0 => None,
    1 => Some(entropy),
    2 => Some(reconstruct),
    _ => return Err(EINVAL),
  };
  let storage = match frame_pass {
    Some(pass) => {
      let Ok(offset) = pass
        .cf
        .get_update(|position| position.wrapping_add(coefficient_count));
      CfSelect::Frame(offset)
    }
    None => CfSelect::Task,
  };

  let (eob, kind) = match t.frame_thread.pass {
    2 => {
      let Ok(record) = reconstruct
        .cbi_idx
        .get_update(|position| position.wrapping_add(1));
      let Ok(index) = console_core_number_conversion::index(record);
      let info_slot = f.frame_thread.cbi.get(index).ok_or(EINVAL)?;
      let Ok(info) = info_slot.get();
      let Ok(eob) = info.eob();
      let Ok(kind) = info.txtp();
      (i32::from(eob), kind)
    }
    0 | 1 => {
      let tile_context = ts_c.ok_or(EINVAL)?;
      let above = f.above_context(t.above_index)?;
      let x = u32::try_from(t.b.x & 31).map_err(|_invalid_column| EINVAL)?;
      let y = u32::try_from(t.b.y & 31).map_err(|_invalid_row| EINVAL)?;
      let Ok(above_range) = console_core_number_conversion::indices(x..x.wrapping_add(width));
      let Ok(left_range) = console_core_number_conversion::indices(y..y.wrapping_add(height));
      let left = t.l.lcoef.get_mut().get_mut(left_range).ok_or(EINVAL)?;

      match x.wrapping_add(width).cmp(&32) {
        cmp::Ordering::Less | cmp::Ordering::Equal => {}
        cmp::Ordering::Greater => return Err(EINVAL),
      }

      let mut above_guard = above.lcoef.index_mut(above_range);
      let mut kind = DCT_DCT;
      let mut residual_context = 0;
      let trace = {
        let Ok(active_trace) = debug_block_info(f, t.b);
        active_trace
      };
      let eob = decode_coefs::<BD>(
        f,
        CoefficientDecoder {
          tile,
          state: tile_context,
          scratch: &mut t.scratch,
          coefficients: &mut t.cf,
        },
        CoefficientBlock {
          trace,
          above: &above_guard,
          left,
          transform,
          size: tree.size,
          syntax: tree.syntax,
          plane: 0,
          storage,
        },
        CoefficientOutput {
          transform: &mut kind,
          residual_context: &mut residual_context,
        },
      )?;

      match trace {
        BlockTrace::Enabled => {
          console_core_logging::trace!(
            "Post-y-cf-blk[tx={transform:?},txtp={kind},eob={eob}]: r={}",
            tile_context.msac.rng,
          );
        }
        BlockTrace::Disabled => {}
      }

      let columns = i32::from(dim.w).min(f.bw.wrapping_sub(t.b.x));
      let rows = i32::from(dim.h).min(f.bh.wrapping_sub(t.b.y));
      let above_len = console_core_number_conversion::checked_index(columns)
        .map_err(|_invalid_width| EINVAL)?;
      let left_len =
        console_core_number_conversion::checked_index(rows).map_err(|_invalid_height| EINVAL)?;
      let above_context = above_guard.get_mut(..above_len).ok_or(EINVAL)?;
      let left_context = left.get_mut(..left_len).ok_or(EINVAL)?;
      let Ok(()) = small_memset::<u8, 16, true>(above_context, residual_context);
      let Ok(()) = small_memset::<u8, 16, true>(left_context, residual_context);
      drop(above_guard);

      let scratch = t.scratch.inter_intra_mut()?;
      let transform_map = scratch.ac_txtp_map.txtp_map_mut()?;
      let Ok(start) = console_core_number_conversion::index(y.wrapping_mul(32).wrapping_add(x));
      let rows_2 = transform_map.get_mut(start..).ok_or(EINVAL)?;
      let Ok(transform_rows) = console_core_number_conversion::index(height);
      let Ok(()) = CaseSet::<16, false>::one(
        rows_2,
        ContextRun {
          offset: 0,
          length: u64::from(width),
        },
        |case, rows_3| {
          for row in rows_3.chunks_mut(32).take(transform_rows) {
            let Ok(()) = case.set(row, kind);
          }
        },
      );

      match t.frame_thread.pass {
        1 => {
          let Ok(record) = entropy
            .cbi_idx
            .get_update(|position| position.wrapping_add(1));
          let Ok(index) = console_core_number_conversion::index(record);
          let info_slot = f.frame_thread.cbi.get(index).ok_or(EINVAL)?;
          let encoded_eob = i16::try_from(eob).map_err(|_outside_eob_field| EINVAL)?;
          let info = CodedBlockInfo::new(encoded_eob, kind)?;
          let Ok(()) = info_slot.set(info);
        }
        _ => {}
      }

      (eob, kind)
    }
    _ => return Err(EINVAL),
  };

  match t.frame_thread.pass {
    1 => return Ok(()),
    0 | 2 => {}
    _ => return Err(EINVAL),
  }

  let destination = leaf.output.ok_or(EINVAL)?;

  match eob.cmp(&0) {
    cmp::Ordering::Less => return Ok(()),
    cmp::Ordering::Equal | cmp::Ordering::Greater => {}
  }

  let mut frame_coefficients;
  let coefficients = match storage {
    CfSelect::Frame(offset) => {
      let end = offset.checked_add(coefficient_count).ok_or(EINVAL)?;
      let Ok(bytes) = console_core_number_conversion::fixed_index(f.frame_thread.cf.len());
      let Ok(element_width) =
        console_core_number_conversion::fixed_index(core::mem::size_of::<BD::Coef>());
      let capacity = bytes.checked_div(element_width).ok_or(EINVAL)?;

      match u64::from(end).cmp(&capacity) {
        cmp::Ordering::Less | cmp::Ordering::Equal => {}
        cmp::Ordering::Greater => return Err(EINVAL),
      }

      let Ok(range) = console_core_number_conversion::indices(offset..end);
      frame_coefficients = f.frame_thread.cf.mut_slice_as(range);
      &mut *frame_coefficients
    }
    CfSelect::Task => t.cf.select_mut::<BD>()?,
  };
  let trace = {
    let Ok(active_trace) = debug_block_info(f, t.b);
    active_trace
  };

  match (trace, PIXEL_TRACE) {
    (BlockTrace::Enabled, BlockTrace::Enabled) => {
      let coded = Size {
        width: u64::from(height.min(8)).wrapping_mul(4),
        height: u64::from(width.min(8)).wrapping_mul(4),
      };
      coef_dump(coefficients, coded, 3, "dq").map_err(|_dump_error| EIO)?;
    }
    (BlockTrace::Disabled, _) | (_, BlockTrace::Disabled) => {}
  }

  let bd = BD::from_c(f.bitdepth_max);
  let mut output = crate::src::owned_recon::ReconDst::Pic(destination);
  f.dsp
    .itx
    .add::<BD>(transform, kind, &mut output, coefficients, eob, bd)?;

  match (trace, PIXEL_TRACE) {
    (BlockTrace::Enabled, BlockTrace::Enabled) => {
      let extent = Size {
        width: u64::from(width).wrapping_mul(4),
        height: u64::from(height).wrapping_mul(4),
      };
      hex_dump_pic::<BD>(&output, extent, "recon").map_err(|_dump_error| EIO)?;
    }
    (BlockTrace::Disabled, _) | (_, BlockTrace::Disabled) => {}
  }

  Ok(())
}

fn transform_steps(dimensions: &TxfmInfo) -> Rav1dResult<Size<core::num::NonZeroU8>> {
  let width = core::num::NonZeroU8::new(dimensions.w).ok_or(EINVAL)?;
  let height = core::num::NonZeroU8::new(dimensions.h).ok_or(EINVAL)?;
  Ok(Size { width, height })
}

#[derive(Clone, Copy)]
enum EntropyCoefficientPlane {
  Luma,
  Chroma(u8),
}

struct EntropyCoefficientBlock<'a> {
  size: BlockSize,
  syntax: &'a Av1Block,
  transform: TxfmSize,
  dimensions: &'a TxfmInfo,
  context: Point<u32>,
  subsampling: Point<u32>,
  plane: EntropyCoefficientPlane,
  kind: TxfmType,
}

fn store_coefficient_record(
  frame: &Rav1dFrameData,
  entropy: &crate::src::internal::Rav1dTileStateFrameThread,
  end_of_block: i32,
  transform: TxfmType,
) -> Rav1dResult {
  let Ok(record) = entropy
    .cbi_idx
    .get_update(|position| position.wrapping_add(1));
  let Ok(index) = console_core_number_conversion::index(record);
  let slot = frame.frame_thread.cbi.get(index).ok_or(EINVAL)?;
  let encoded = i16::try_from(end_of_block).map_err(|_outside_eob_field| EINVAL)?;
  let info = CodedBlockInfo::new(encoded, transform)?;
  let Ok(()) = slot.set(info);
  Ok(())
}

fn store_coefficient_context(
  above: &mut [u8],
  left: &mut [u8],
  available: Size<i32>,
  context: u8,
) -> Rav1dResult {
  let width = console_core_number_conversion::checked_index(available.width)
    .map_err(|_outside_context| EINVAL)?;
  let height = console_core_number_conversion::checked_index(available.height)
    .map_err(|_outside_context| EINVAL)?;
  let above_samples = above.get_mut(..width).ok_or(EINVAL)?;
  let left_samples = left.get_mut(..height).ok_or(EINVAL)?;
  let Ok(()) = small_memset::<u8, 16, true>(above_samples, context);
  let Ok(()) = small_memset::<u8, 16, true>(left_samples, context);
  Ok(())
}

fn read_queued_coefficient_block<BD: BitDepth>(
  frame: &Rav1dFrameData,
  task: &mut Rav1dTaskContext,
  tile: &Rav1dTileState,
  state: &mut Rav1dTileStateContext,
  block: EntropyCoefficientBlock<'_>,
) -> Rav1dResult {
  let [_, entropy] = &tile.frame_thread;
  let above = frame.above_context(task.above_index)?;
  let (above_plane, left_plane, plane) = match block.plane {
    EntropyCoefficientPlane::Luma => (&above.lcoef, &mut task.l.lcoef, 0),
    EntropyCoefficientPlane::Chroma(ordinal) => {
      let Ok(index) = console_core_number_conversion::index(ordinal);
      let upper = above.ccoef.get(index).ok_or(EINVAL)?;
      let left = task.l.ccoef.get_mut(index).ok_or(EINVAL)?;
      (upper, left, ordinal.wrapping_add(1))
    }
  };
  let width = u32::from(block.dimensions.w);
  let height = u32::from(block.dimensions.h);
  let offset = match block.plane {
    EntropyCoefficientPlane::Luma => {
      let count = width.min(8).wrapping_mul(height.min(8)).wrapping_mul(16);
      let Ok(first) = entropy
        .cf
        .get_update(|position| position.wrapping_add(count));
      first
    }
    EntropyCoefficientPlane::Chroma(_) => {
      let Ok(first) = entropy.cf.get();
      first
    }
  };
  let above_end = block.context.x.wrapping_add(width);
  let left_end = block.context.y.wrapping_add(height);

  match above_end {
    0..=32 => {}
    33.. => return Err(EINVAL),
  }

  let Ok(above_range) = console_core_number_conversion::indices(block.context.x..above_end);
  let Ok(left_range) = console_core_number_conversion::indices(block.context.y..left_end);
  let mut above_context = above_plane.index_mut(above_range);
  let left = left_plane.get_mut().get_mut(left_range).ok_or(EINVAL)?;
  let mut kind = block.kind;
  let mut residual = 0x40;
  let Ok(trace) = debug_block_info(frame, task.b);
  let eob = decode_coefs::<BD>(
    frame,
    CoefficientDecoder {
      tile,
      state,
      scratch: &mut task.scratch,
      coefficients: &mut task.cf,
    },
    CoefficientBlock {
      trace,
      above: &above_context,
      left,
      transform: block.transform,
      size: block.size,
      syntax: block.syntax,
      plane,
      storage: CfSelect::Frame(offset),
    },
    CoefficientOutput {
      transform: &mut kind,
      residual_context: &mut residual,
    },
  )?;

  match trace {
    BlockTrace::Enabled => console_core_logging::trace!(
      "Post-cf-blk[pl={plane},tx={:?},txtp={kind},eob={eob}]: r={}",
      block.transform,
      state.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  store_coefficient_record(frame, entropy, eob, kind)?;

  match block.plane {
    EntropyCoefficientPlane::Luma => {}
    EntropyCoefficientPlane::Chroma(_) => {
      let count = width.wrapping_mul(height).wrapping_mul(16);
      let Ok(()) = entropy.cf.set(offset.wrapping_add(count));
    }
  }

  let visible = |remaining: i32, shift| {
    remaining
      .wrapping_add(i32::from(shift != 0))
      .wrapping_shr(shift)
  };
  let available = Size {
    width: i32::from(block.dimensions.w).min(visible(
      frame.bw.wrapping_sub(task.b.x),
      block.subsampling.x,
    )),
    height: i32::from(block.dimensions.h).min(visible(
      frame.bh.wrapping_sub(task.b.y),
      block.subsampling.y,
    )),
  };
  store_coefficient_context(&mut above_context, left, available, residual)
}

struct EntropyBlockScan<'a> {
  origin: Bxy,
  size: BlockSize,
  syntax: &'a Av1Block,
  luma: TxfmSize,
  block_offset: Point<u32>,
  visible: Size<i32>,
  chroma: Option<Point<u32>>,
}

struct TransformChunk {
  first: Point<i32>,
  end: Point<i32>,
}

fn scan_luma_coefficients<BD: BitDepth>(
  frame: &Rav1dFrameData,
  task: &mut Rav1dTaskContext,
  tile: &Rav1dTileState,
  state: &mut Rav1dTileStateContext,
  scan: &EntropyBlockScan<'_>,
  chunk: &TransformChunk,
) -> Rav1dResult {
  let dimensions = coefficient_dimensions(scan.luma)?;
  let steps = transform_steps(dimensions)?;
  let rows = (chunk.first.y..chunk.end.y).step_by(core::num::NonZero::from(steps.height).get());
  let columns = (chunk.first.x..chunk.end.x).step_by(core::num::NonZero::from(steps.width).get());
  let first_row = i32::from(chunk.first.y != 0);
  let first_column = i32::from(chunk.first.x != 0);

  for (split_row, y) in (first_row..).zip(rows) {
    for (split_column, x) in (first_column..).zip(columns.clone()) {
      task.b = Bxy {
        x: scan.origin.x.wrapping_add(x),
        y: scan.origin.y.wrapping_add(y),
      };

      match &scan.syntax.ii {
        Av1BlockIntraInter::Inter(inter) => {
          let tree = LumaCoefficientTree {
            size: scan.size,
            syntax: scan.syntax,
            transform: scan.luma,
            splits: [u16::from(inter.tx_split0), inter.tx_split1],
            split_cell: Point {
              x: split_column,
              y: split_row,
            },
            output: None,
          };
          read_coef_tree::<BD>(frame, task, Some(state), tree)?;
        }
        Av1BlockIntraInter::Intra(_) => {
          let block = EntropyCoefficientBlock {
            size: scan.size,
            syntax: scan.syntax,
            transform: scan.luma,
            dimensions,
            context: Point {
              x: scan.block_offset.x.wrapping_add(x.cast_unsigned()),
              y: scan.block_offset.y.wrapping_add(y.cast_unsigned()),
            },
            subsampling: Point { x: 0, y: 0 },
            plane: EntropyCoefficientPlane::Luma,
            kind: DCT_DCT,
          };
          read_queued_coefficient_block::<BD>(frame, task, tile, state, block)?;
        }
      }
    }
  }

  Ok(())
}

fn scan_chroma_coefficients<BD: BitDepth>(
  frame: &Rav1dFrameData,
  task: &mut Rav1dTaskContext,
  tile: &Rav1dTileState,
  state: &mut Rav1dTileStateContext,
  scan: &EntropyBlockScan<'_>,
  chunk: &TransformChunk,
  shifts: Point<u32>,
) -> Rav1dResult {
  let transform = scan.syntax.uvtx;
  let dimensions = coefficient_dimensions(transform)?;
  let steps = transform_steps(dimensions)?;
  let padded = |length: i32, shift| {
    length
      .wrapping_add(i32::from(shift != 0))
      .wrapping_shr(shift)
  };
  let last_row = padded(scan.visible.height, shifts.y)
    .min(chunk.first.y.wrapping_add(16).wrapping_shr(shifts.y));
  let last_column = padded(scan.visible.width, shifts.x)
    .min(chunk.first.x.wrapping_add(16).wrapping_shr(shifts.x));
  let rows = (chunk.first.y.wrapping_shr(shifts.y)..last_row)
    .step_by(core::num::NonZero::from(steps.height).get());
  let columns = (chunk.first.x.wrapping_shr(shifts.x)..last_column)
    .step_by(core::num::NonZero::from(steps.width).get());
  let above_start = scan.block_offset.x.wrapping_shr(shifts.x);
  let left_start = scan.block_offset.y.wrapping_shr(shifts.y);

  for plane in 0_u8..2 {
    for y in rows.clone() {
      for x in columns.clone() {
        let luma_x = x.wrapping_shl(shifts.x);
        let luma_y = y.wrapping_shl(shifts.y);
        task.b = Bxy {
          x: scan.origin.x.wrapping_add(luma_x),
          y: scan.origin.y.wrapping_add(luma_y),
        };
        let kind = match scan.syntax.ii {
          Av1BlockIntraInter::Intra(_) => DCT_DCT,
          Av1BlockIntraInter::Inter(_) => {
            let column = scan.block_offset.x.wrapping_add(luma_x.cast_unsigned());
            let row = scan.block_offset.y.wrapping_add(luma_y.cast_unsigned());
            let Ok(index) =
              console_core_number_conversion::index(row.wrapping_mul(32).wrapping_add(column));
            let scratch = task.scratch.inter_intra()?;
            let transforms = scratch.ac_txtp_map.txtp_map()?;
            transforms.get(index).copied().ok_or(EINVAL)?
          }
        };
        let block = EntropyCoefficientBlock {
          size: scan.size,
          syntax: scan.syntax,
          transform,
          dimensions,
          context: Point {
            x: above_start.wrapping_add(x.cast_unsigned()),
            y: left_start.wrapping_add(y.cast_unsigned()),
          },
          subsampling: shifts,
          plane: EntropyCoefficientPlane::Chroma(plane),
          kind,
        };
        read_queued_coefficient_block::<BD>(frame, task, tile, state, block)?;
      }
    }
  }

  Ok(())
}

fn scan_frame_coefficients<BD: BitDepth>(
  frame: &Rav1dFrameData,
  task: &mut Rav1dTaskContext,
  state: &mut Rav1dTileStateContext,
  scan: &EntropyBlockScan<'_>,
) -> Rav1dResult {
  let tile = frame.tile(task.tile_index)?;

  for y in (0..scan.visible.height).step_by(16) {
    for x in (0..scan.visible.width).step_by(16) {
      let chunk = TransformChunk {
        first: Point { x, y },
        end: Point {
          x: scan.visible.width.min(x.wrapping_add(16)),
          y: scan.visible.height.min(y.wrapping_add(16)),
        },
      };
      scan_luma_coefficients::<BD>(frame, task, tile, state, scan, &chunk)?;

      match scan.chroma {
        Some(shifts) => {
          scan_chroma_coefficients::<BD>(frame, task, tile, state, scan, &chunk, shifts)?
        }
        None => {}
      }
    }
  }

  Ok(())
}

fn clear_skipped_coefficient_context(
  frame: &Rav1dFrameData,
  task: &mut Rav1dTaskContext,
  position: Point<u32>,
  size: Size<i32>,
  chroma: Option<Point<u32>>,
) -> Rav1dResult {
  let above = frame.above_context(task.above_index)?;
  let Ok(luma_x) = console_core_number_conversion::index(position.x);
  let Ok(luma_y) = console_core_number_conversion::index(position.y);
  let luma_width =
    console_core_number_conversion::checked_index(size.width).map_err(|_range| EINVAL)?;
  let luma_height =
    console_core_number_conversion::checked_index(size.height).map_err(|_range| EINVAL)?;
  case_set_al! {
    <32, false>
    l: (&mut task.l, luma_height, luma_y),
    a: (above, luma_width, luma_x),
    lcoef = (0x40, 0x40),
  }

  match chroma {
    Some(shifts) => {
      let Ok(x) = console_core_number_conversion::index(position.x.wrapping_shr(shifts.x));
      let Ok(y) = console_core_number_conversion::index(position.y.wrapping_shr(shifts.y));
      let chroma_extent = |length: i32, shift| {
        length
          .wrapping_add(i32::from(shift != 0))
          .wrapping_shr(shift)
      };
      let w = console_core_number_conversion::checked_index(chroma_extent(size.width, shifts.x))
        .map_err(|_range| EINVAL)?;
      let h = console_core_number_conversion::checked_index(chroma_extent(size.height, shifts.y))
        .map_err(|_range| EINVAL)?;
      case_set_al! {
        <32, false>
        l: (&mut task.l, h, y),
        a: (above, w, x),
        ccoef[0] = (0x40, 0x40),
        ccoef[1] = (0x40, 0x40),
      }
    }
    None => {}
  }

  Ok(())
}

pub(crate) fn rav1d_read_coef_blocks<BD: BitDepth>(
  frame: &Rav1dFrameData,
  task: &mut Rav1dTaskContext,
  state: &mut Rav1dTileStateContext,
  size: BlockSize,
  syntax: &Av1Block,
) -> Rav1dResult {
  let origin = task.b;
  let position = Point {
    x: (origin.x & 31).cast_unsigned(),
    y: (origin.y & 31).cast_unsigned(),
  };
  let Ok(&[width, height, ..]) = size.dimensions();
  let extent = Size {
    width: i32::from(width),
    height: i32::from(height),
  };
  let layout = frame.cur.p.layout;
  let shifts = Point {
    x: u32::from(layout != Rav1dPixelLayout::I444),
    y: u32::from(layout == Rav1dPixelLayout::I420),
  };
  let chroma = (layout != Rav1dPixelLayout::I400
    && (u32::from(width) > shifts.x || origin.x & 1 != 0)
    && (u32::from(height) > shifts.y || origin.y & 1 != 0))
    .then_some(shifts);

  match syntax.skip {
    0 => {}
    _ => return clear_skipped_coefficient_context(frame, task, position, extent, chroma),
  }

  match task.frame_thread.pass {
    1 => {}
    _ => return Err(EINVAL),
  }

  let luma = match &syntax.ii {
    Av1BlockIntraInter::Intra(intra) => intra.tx,
    Av1BlockIntraInter::Inter(inter) => inter.max_ytx,
  };
  let visible = Size {
    width: extent.width.min(frame.bw.wrapping_sub(origin.x)),
    height: extent.height.min(frame.bh.wrapping_sub(origin.y)),
  };
  let scan = EntropyBlockScan {
    origin,
    size,
    syntax,
    luma,
    block_offset: position,
    visible,
    chroma,
  };
  let result = scan_frame_coefficients::<BD>(frame, task, state, &scan);
  task.b = origin;
  result
}

enum MaybeTempPixels<'a, TmpStride> {
  NonTemp {
    dst: PicOffset<'a>,
  },
  Temp {
    tmp: &'a mut [i16],
    tmp_stride: TmpStride,
  },
}

struct MotionPrediction<'a> {
  position: Bxy,
  size: Size<i32>,
  origin: Point<i32>,
  plane: u8,
  motion: Mv,
  reference: &'a Rav1dThreadPicture,
  reference_index: u8,
  filter: Filter2d,
}

fn motion_picture_position<BD: BitDepth>(
  view: PicOffset<'_>,
  position: Point<i32>,
) -> Result<PicOffset<'_>, ()> {
  let component = view.data;
  let Ok(pitch) = component.fixed_pixel_stride::<BD>();
  let Ok(base) = console_core_number_conversion::fixed_index(view.offset);
  let row = i64::from(position.y).checked_mul(pitch).ok_or(())?;
  let shift = row.checked_add(i64::from(position.x)).ok_or(())?;
  let offset = base.checked_add_signed(shift).ok_or(())?;
  let Ok(capacity) = console_core_number_conversion::fixed_index(component.pixel_len::<BD>());

  match offset.cmp(&capacity) {
    cmp::Ordering::Less => {}
    cmp::Ordering::Equal | cmp::Ordering::Greater => return Err(()),
  }

  let Ok(offset_2) = console_core_number_conversion::index(offset);
  Ok(PicOffset {
    data: component,
    offset: offset_2,
  })
}

#[derive(Clone, Copy)]
enum ReferenceSampling {
  Direct,
  Scaled,
}

fn trace_edge_emulation(trace: BlockTrace) -> Result<(), console_core_never::Never> {
  match trace {
    BlockTrace::Enabled => console_core_logging::trace!("Emu"),
    BlockTrace::Disabled => {}
  }

  Ok(())
}

fn mc<BD: BitDepth>(
  f: &Rav1dFrameData,
  emu_edge: &mut ScratchEmuEdge,
  dst: MaybeTempPixels<()>,
  prediction: MotionPrediction<'_>,
) -> Result<(), ()> {
  let MotionPrediction {
    position,
    size,
    origin,
    plane,
    motion,
    reference,
    reference_index,
    filter,
  } = prediction;
  let Ok(trace) = debug_block_info(f, position);
  let bd = BD::from_c(f.bitdepth_max);
  let reference_picture = reference.p.data.as_ref().ok_or(())?;
  let ref_data = &reference_picture.data;
  let current_picture = f.cur.data.as_ref().ok_or(())?;
  let cur_data = &current_picture.data;
  let Ok(plane_index) = console_core_number_conversion::index(plane);
  let component = ref_data.get(plane_index).ok_or(())?;
  let layout = f.cur.p.layout;
  let subsample = Point {
    x: u32::from(plane != 0 && layout != Rav1dPixelLayout::I444),
    y: u32::from(plane != 0 && layout == Rav1dPixelLayout::I420),
  };
  let block = Size {
    width: size.width.wrapping_mul(4_i32.wrapping_shr(subsample.x)),
    height: size.height.wrapping_mul(4_i32.wrapping_shr(subsample.y)),
  };
  let start = Point {
    x: origin.x.wrapping_mul(4_i32.wrapping_shr(subsample.x)),
    y: origin.y.wrapping_mul(4_i32.wrapping_shr(subsample.y)),
  };
  let mode = match (
    reference.p.p.w.cmp(&f.cur.p.w),
    reference.p.p.h.cmp(&f.cur.p.h),
  ) {
    (cmp::Ordering::Equal, cmp::Ordering::Equal) => ReferenceSampling::Direct,
    _ => ReferenceSampling::Scaled,
  };
  let mv = Point {
    x: i32::from(motion.x),
    y: i32::from(motion.y),
  };

  match mode {
    ReferenceSampling::Direct => {
      let [first_reference, ..] = ref_data;
      let [first_current, ..] = cur_data;
      let visible = match first_reference.ref_eq(first_current).then_some(()) {
        Some(()) => Size {
          width: f.bw.wrapping_mul(4),
          height: f.bh.wrapping_mul(4),
        },
        None => Size {
          width: f.cur.p.w.wrapping_add(i32::from(subsample.x != 0)),
          height: f.cur.p.h.wrapping_add(i32::from(subsample.y != 0)),
        },
      };
      let extent = Size {
        width: visible.width.wrapping_shr(subsample.x),
        height: visible.height.wrapping_shr(subsample.y),
      };
      let source = Point {
        x: start
          .x
          .wrapping_add(mv.x.wrapping_shr(3_u32.wrapping_add(subsample.x))),
        y: start
          .y
          .wrapping_add(mv.y.wrapping_shr(3_u32.wrapping_add(subsample.y))),
      };
      let phase = Point {
        x: mv.x & 15_i32.wrapping_shr(u32::from(subsample.x == 0)),
        y: mv.y & 15_i32.wrapping_shr(u32::from(subsample.y == 0)),
      };
      let footprint = console_core_video_av1_digital_signal_processing::read_extent(
        Size {
          width: u64::try_from(block.width).map_err(|_| ())?,
          height: u64::try_from(block.height).map_err(|_| ())?,
        },
        Point {
          x: phase.x != 0,
          y: phase.y != 0,
        },
      )
      .map_err(|_| ())?;
      let Ok(window_width) =
        console_core_number_conversion::wrapped::<_, i32>(footprint.size.width);
      let Ok(window_height) =
        console_core_number_conversion::wrapped::<_, i32>(footprint.size.height);
      let Ok(left) = console_core_number_conversion::wrapped::<_, i32>(footprint.before.x);
      let Ok(top) = console_core_number_conversion::wrapped::<_, i32>(footprint.before.y);
      let lower = Point { x: left, y: top };
      let upper = Point {
        x: window_width.wrapping_sub(block.width).wrapping_sub(left),
        y: window_height.wrapping_sub(block.height).wrapping_sub(top),
      };
      let outside = source.x < lower.x
        || source.y < lower.y
        || source.x.wrapping_add(block.width).wrapping_add(upper.x) > extent.width
        || source.y.wrapping_add(block.height).wrapping_add(upper.y) > extent.height;
      let (motion_scratch, edge_pixels) = emu_edge
        .motion_and_edges::<BD>()
        .map_err(|_invalid_scratch| ())?;
      let edge_component;
      let input = match outside.then_some(()) {
        Some(()) => {
          const STRIDE: u32 = 192;
          let buffer = edge_pixels;
          let window = crate::src::mc::EdgeWindow {
            block: Size {
              width: window_width,
              height: window_height,
            },
            reference: extent,
            origin: Point {
              x: source.x.wrapping_sub(lower.x),
              y: source.y.wrapping_sub(lower.y),
            },
          };
          f.dsp
            .mc
            .emu_edge
            .call::<BD>(window, buffer, STRIDE, component)
            .map_err(|_edge_layout| ())?;
          let Ok(stride) = console_core_number_conversion::index(STRIDE);
          edge_component = Rav1dPictureDataComponent::wrap_buf::<BD>(buffer, stride);
          motion_picture_position::<BD>(edge_component.with_offset::<BD>(), lower)?
        }
        None => motion_picture_position::<BD>(component.with_offset::<BD>(), source)?,
      };
      let sampling = crate::src::mc::UnscaledSampling {
        size: block,
        phase: Point {
          x: phase.x.wrapping_shl(u32::from(subsample.x == 0)),
          y: phase.y.wrapping_shl(u32::from(subsample.y == 0)),
        },
      };
      let result = match dst {
        MaybeTempPixels::NonTemp { dst: dst_2 } => {
          let mc = f
            .dsp
            .mc
            .mc
            .checked(filter)
            .map_err(|_invalid_dispatch| ())?;
          mc.call::<BD>(
            f.motion_compensation,
            motion_scratch,
            filter,
            dst_2,
            input,
            sampling,
            bd,
          )
        }
        MaybeTempPixels::Temp { tmp, .. } => {
          let mct = f
            .dsp
            .mc
            .mct
            .checked(filter)
            .map_err(|_invalid_dispatch| ())?;
          mct.call::<BD>(
            f.motion_compensation,
            motion_scratch,
            filter,
            tmp,
            input,
            sampling,
            bd,
          )
        }
      };
      result.map_err(|_invalid_motion| ())
    }
    ReferenceSampling::Scaled => {
      match ptr::eq(reference, &f.sr_cur).then_some(()) {
        Some(()) => return Err(()),
        None => {}
      }

      let scaled_origin = Point {
        x: start
          .x
          .wrapping_shl(4)
          .wrapping_add(mv.x.wrapping_shl(u32::from(subsample.x == 0))),
        y: start
          .y
          .wrapping_shl(4)
          .wrapping_add(mv.y.wrapping_shl(u32::from(subsample.y == 0))),
      };
      let Ok(reference_position) = console_core_number_conversion::index(reference_index);
      let [horizontal, vertical] = f.svc.get(reference_position).ok_or(())?;
      let scale = |value: i32, multiplier: i32| {
        let correction = multiplier.wrapping_sub(0x4000).wrapping_mul(8);
        let scaled = i64::from(value)
          .wrapping_mul(i64::from(multiplier))
          .wrapping_add(i64::from(correction));
        let rounded = scaled.wrapping_abs().wrapping_add(128).wrapping_shr(8);
        let Ok(quotient) = console_core_number_conversion::wrapped::<_, i32>(rounded);
        apply_sign64(quotient, scaled).wrapping_add(32)
      };
      let sample = Point {
        x: scale(scaled_origin.x, horizontal.scale),
        y: scale(scaled_origin.y, vertical.scale),
      };
      let source = Point {
        x: sample.x.wrapping_shr(10),
        y: sample.y.wrapping_shr(10),
      };
      let right = sample
        .x
        .wrapping_add(block.width.wrapping_sub(1).wrapping_mul(horizontal.step))
        .wrapping_shr(10)
        .wrapping_add(1);
      let bottom = sample
        .y
        .wrapping_add(block.height.wrapping_sub(1).wrapping_mul(vertical.step))
        .wrapping_shr(10)
        .wrapping_add(1);

      match trace {
        BlockTrace::Enabled => {
          console_core_logging::trace!(
            "Off {}x{} [{},{},{}], size {}x{} [{},{}]",
            source.x,
            source.y,
            origin.x,
            horizontal.scale,
            reference_index,
            right.wrapping_sub(source.x),
            bottom.wrapping_sub(source.y),
            horizontal.step,
            vertical.step,
          );
        }
        BlockTrace::Disabled => {}
      }

      let extent = Size {
        width: reference
          .p
          .p
          .w
          .wrapping_add(i32::from(subsample.x != 0))
          .wrapping_shr(subsample.x),
        height: reference
          .p
          .p
          .h
          .wrapping_add(i32::from(subsample.y != 0))
          .wrapping_shr(subsample.y),
      };
      let outside = source.x < 3
        || source.y < 3
        || right.wrapping_add(4) > extent.width
        || bottom.wrapping_add(4) > extent.height;
      let edge_component;
      let input = match outside.then_some(()) {
        Some(()) => {
          const STRIDE: u32 = 320;
          let buffer = emu_edge.buf_mut::<BD>().map_err(|_invalid_scratch| ())?;
          let window = crate::src::mc::EdgeWindow {
            block: Size {
              width: right.wrapping_sub(source.x).wrapping_add(7),
              height: bottom.wrapping_sub(source.y).wrapping_add(7),
            },
            reference: extent,
            origin: Point {
              x: source.x.wrapping_sub(3),
              y: source.y.wrapping_sub(3),
            },
          };
          f.dsp
            .mc
            .emu_edge
            .call::<BD>(window, buffer, STRIDE, component)
            .map_err(|_edge_layout| ())?;

          let Ok(()) = trace_edge_emulation(trace);

          let Ok(stride) = console_core_number_conversion::index(STRIDE);
          edge_component = Rav1dPictureDataComponent::wrap_buf::<BD>(buffer, stride);
          motion_picture_position::<BD>(edge_component.with_offset::<BD>(), Point { x: 3, y: 3 })?
        }
        None => motion_picture_position::<BD>(component.with_offset::<BD>(), source)?,
      };
      let sampling = crate::src::mc::ScaledSampling {
        size: block,
        phase: Point {
          x: sample.x & 0x3ff,
          y: sample.y & 0x3ff,
        },
        step: Point {
          x: horizontal.step,
          y: vertical.step,
        },
      };
      let result = match dst {
        MaybeTempPixels::NonTemp { dst: dst_3 } => {
          let mc = f
            .dsp
            .mc
            .mc_scaled
            .checked(filter)
            .map_err(|_invalid_dispatch| ())?;
          mc.call::<BD>(filter, dst_3, input, sampling, bd)
        }
        MaybeTempPixels::Temp { tmp, .. } => {
          let mct = f
            .dsp
            .mc
            .mct_scaled
            .checked(filter)
            .map_err(|_invalid_dispatch| ())?;
          mct.call::<BD>(filter, tmp, input, sampling, bd)
        }
      };
      result.map_err(|_invalid_scaled_reference| ())
    }
  }
}

struct OverlapReconstruction {
  visible: Size<i32>,
  dimensions: [u8; 4],
  context: Point<i32>,
  plane: u8,
}

struct OverlapTarget {
  origin: Bxy,
  distance: i32,
  block_height: i32,
  block_pixels: Size<i32>,
}

fn overlap_target<'a, BD: BitDepth>(
  axis: crate::src::safe_simd::mc::BlendAxis,
  dst: PicOffset<'a>,
  target: OverlapTarget,
) -> Result<(i32, Point<i32>, PicOffset<'a>), ()> {
  use crate::src::safe_simd::mc::BlendAxis;

  let OverlapTarget {
    origin,
    distance,
    block_height,
    block_pixels,
  } = target;

  match axis {
    BlendAxis::Horizontal => {
      let source = Point {
        x: origin.x.wrapping_add(distance),
        y: origin.y,
      };
      let lines = block_height.wrapping_mul(3).wrapping_add(3).wrapping_shr(2);
      let offset =
        console_core_number_conversion::checked_index(distance.wrapping_mul(block_pixels.width))
          .map_err(|_outside_destination| ())?;
      Ok((lines, source, dst + offset))
    }
    BlendAxis::Vertical => {
      let source = Point {
        x: origin.x,
        y: origin.y.wrapping_add(distance),
      };
      let row = i64::from(distance.wrapping_mul(block_pixels.height));
      let Ok(stride) = dst.fixed_pixel_stride::<BD>();
      let Ok(offset) = console_core_number_conversion::wrapped_offset(row.wrapping_mul(stride));
      Ok((block_height, source, dst + offset))
    }
  }
}

fn blend_overlap<BD: BitDepth>(
  f: &Rav1dFrameData,
  axis: crate::src::safe_simd::mc::BlendAxis,
  output: PicOffset<'_>,
  lap: &crate::src::internal::OverlapPrediction<BD::Pixel>,
  size: Size<i32>,
) -> Result<(), ()> {
  use crate::src::safe_simd::mc::BlendAxis;

  let blended = match axis {
    BlendAxis::Horizontal => f.dsp.mc.blend_h.call::<BD>(axis, output, lap, size),
    BlendAxis::Vertical => f.dsp.mc.blend_v.call::<BD>(axis, output, lap, size),
  };
  blended.map_err(|_invalid_destination| ())
}

fn obmc<BD: BitDepth>(
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
  dst: PicOffset,
  OverlapReconstruction {
    visible,
    dimensions: [block_width, block_height, horizontal_limit, vertical_limit],
    context,
    plane,
  }: OverlapReconstruction,
) -> Result<(), ()> {
  match { ::core::primitive::bool::then_some((t.b.x | t.b.y) & 1 != 0, ()) } {
    ::core::option::Option::Some(()) => {
      return Err(());
    }
    ::core::option::Option::None => {}
  }

  let tile = f.tile(t.tile_index).map_err(|_invalid_slot| ())?;
  let above = f.above_context(t.above_index).map_err(|_invalid_slot| ())?;
  let left = &t.l;
  let position = t.b;
  let ring_row = (position.y & 31).wrapping_add(5);
  let layout = f.cur.p.layout;
  let x_shift = u32::from(plane != 0 && layout != Rav1dPixelLayout::I444);
  let y_shift = u32::from(plane != 0 && layout == Rav1dPixelLayout::I420);
  let pixel_width = 4_i32.wrapping_shr(x_shift);
  let pixel_height = 4_i32.wrapping_shr(y_shift);
  let luma = plane == 0;
  let fits_above = i32::from(block_width)
    .wrapping_mul(pixel_width)
    .wrapping_add(i32::from(block_height).wrapping_mul(pixel_height))
    >= 16;
  let scratch = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
  let lap = scratch
    .lap_inter
    .lap_mut::<BD>()
    .map_err(|_invalid_scratch| ())?;

  use crate::src::safe_simd::mc::BlendAxis;

  for axis in [BlendAxis::Horizontal, BlendAxis::Vertical] {
    let overlap = match axis {
      BlendAxis::Horizontal => (position.y > tile.tiling.row_start && (luma || fits_above))
        .then_some((visible.width, horizontal_limit.min(4))),
      BlendAxis::Vertical => {
        (position.x > tile.tiling.col_start).then_some((visible.height, vertical_limit.min(4)))
      }
    };
    let (visible_extent, max_neighbors) = match overlap {
      Some(allowed) => allowed,
      None => continue,
    };
    let mut count = 0_u8;
    let mut distance = 0_i32;

    'inner_for_2: for _ in 0..visible_extent {
      match {
        ::core::primitive::bool::then_some(
          distance >= visible_extent || count >= max_neighbors,
          (),
        )
      } {
        ::core::option::Option::Some(()) => {
          break 'inner_for_2;
        }
        ::core::option::Option::None => {}
      }

      let one_ahead = distance.wrapping_add(1);
      let neighbor = match axis {
        BlendAxis::Horizontal => Point {
          x: position.x.wrapping_add(one_ahead),
          y: ring_row.wrapping_sub(1),
        },
        BlendAxis::Vertical => Point {
          x: position.x.wrapping_sub(1),
          y: ring_row.wrapping_add(one_ahead),
        },
      };
      let block = crate::src::refmvs::read_spatial_block(&f.rf.r, &t.rt.r, neighbor)
        .map_err(|_invalid_neighbor| ())?;
      let Ok(&[neighbor_width, neighbor_height, ..]) = block.bs.dimensions();
      let span = match axis {
        BlendAxis::Horizontal => neighbor_width,
        BlendAxis::Vertical => neighbor_height,
      };
      let step = span.max(2).min(16);
      let [reference, _] = block.r#ref.r#ref;

      match { ::core::primitive::bool::then_some(reference > 0, ()) } {
        ::core::option::Option::Some(()) => {
          let reference_code =
            u8::try_from(reference.wrapping_sub(1)).map_err(|_invalid_reference| ())?;
          let Ok(reference_index) = console_core_number_conversion::index(reference_code);
          let reference_picture = f.refp.get(reference_index).ok_or(())?;
          let [motion, _] = block.mv.mv;
          let (blend_width, blend_height, filter_position) = match axis {
            BlendAxis::Horizontal => (
              step.min(block_width),
              block_height.min(16).wrapping_shr(1),
              context.x.wrapping_add(one_ahead),
            ),
            BlendAxis::Vertical => (
              block_width.min(16).wrapping_shr(1),
              step.min(block_height),
              context.y.wrapping_add(one_ahead),
            ),
          };
          let [horizontal, vertical] = match axis {
            BlendAxis::Horizontal => &above.filter,
            BlendAxis::Vertical => &left.filter,
          };
          let filter_x = crate::src::env::read_context(horizontal, filter_position)
            .map_err(|_outside_filter| ())?;
          let filter_y = crate::src::env::read_context(vertical, filter_position)
            .map_err(|_outside_filter| ())?;
          let Ok(filter_column) = console_core_number_conversion::index(u8::from(filter_x));
          let Ok(filter_row) = console_core_number_conversion::index(u8::from(filter_y));
          let &filter = dav1d_filter_2d
            .get(filter_row)
            .and_then(|row| row.get(filter_column))
            .ok_or(())?;
          let width = i32::from(blend_width);
          let height = i32::from(blend_height);
          let size = Size {
            width: width.wrapping_mul(pixel_width),
            height: height.wrapping_mul(pixel_height),
          };
          let lap_stride = console_core_number_conversion::checked_index(size.width)
            .map_err(|_invalid_stride| ())?;
          let lap_component = Rav1dPictureDataComponent::wrap_buf::<BD>(lap, lap_stride);
          let target_geometry = OverlapTarget {
            origin: position,
            distance,
            block_height: height,
            block_pixels: Size {
              width: pixel_width,
              height: pixel_height,
            },
          };
          let (height_to_predict, reference_origin, output) =
            overlap_target::<BD>(axis, dst, target_geometry)?;
          mc::<BD>(
            f,
            &mut scratch.emu_edge,
            MaybeTempPixels::NonTemp {
              dst: PicOffset {
                data: &lap_component,
                offset: 0,
              },
            },
            MotionPrediction {
              position,
              size: Size {
                width,
                height: height_to_predict,
              },
              origin: reference_origin,
              plane,
              motion,
              reference: reference_picture,
              reference_index: reference_code,
              filter,
            },
          )?;
          #[cfg(not(feature = "c-ffi"))]
          lap_component.copy_pixels_to::<BD>(lap);
          blend_overlap::<BD>(f, axis, output, lap, size)?;
          count = count.wrapping_add(1);
        }
        ::core::option::Option::None => {}
      }

      distance = distance.wrapping_add(i32::from(step));
    }
  }

  Ok(())
}

struct WarpPrediction<'a> {
  position: Bxy,
  dimensions: [u8; 4],
  plane: u8,
  reference: &'a Rav1dThreadPicture,
  parameters: &'a Rav1dWarpedMotionParams,
}

struct WarpPlacement {
  source: Point<i32>,
  destination: Point<i32>,
  extent: Size<i32>,
  sampling: crate::src::mc::WarpSampling,
}

fn apply_warp_subblock<BD: BitDepth>(
  f: &Rav1dFrameData,
  emu_edge: &mut ScratchEmuEdge,
  dst: &mut MaybeTempPixels<u64>,
  component: &Rav1dPictureDataComponent,
  depth: BD,
  WarpPlacement {
    source,
    destination,
    extent,
    sampling,
  }: WarpPlacement,
) -> Result<(), ()> {
  let outside = source.x < 3
    || source.y < 3
    || source.x.wrapping_add(12) > extent.width
    || source.y.wrapping_add(12) > extent.height;
  let edge_component;
  let input = match outside.then_some(()) {
    Some(()) => {
      const STRIDE: u32 = 32;
      let buffer = emu_edge.buf_mut::<BD>().map_err(|_invalid_scratch| ())?;
      let window = crate::src::mc::EdgeWindow {
        block: Size {
          width: 15,
          height: 15,
        },
        reference: extent,
        origin: Point {
          x: source.x.wrapping_sub(3),
          y: source.y.wrapping_sub(3),
        },
      };
      f.dsp
        .mc
        .emu_edge
        .call::<BD>(window, buffer, STRIDE, component)
        .map_err(|_edge_layout| ())?;
      let Ok(stride) = console_core_number_conversion::index(STRIDE);
      edge_component = Rav1dPictureDataComponent::wrap_buf::<BD>(buffer, stride);
      motion_picture_position::<BD>(edge_component.with_offset::<BD>(), Point { x: 3, y: 3 })?
    }
    None => motion_picture_position::<BD>(component.with_offset::<BD>(), source)?,
  };
  let result = match dst {
    MaybeTempPixels::Temp { tmp, tmp_stride } => {
      let row = u64::try_from(destination.y).map_err(|_invalid_row| ())?;
      let column = u64::try_from(destination.x).map_err(|_invalid_column| ())?;
      let samples = row.checked_mul(*tmp_stride).ok_or(())?;
      let offset = samples.checked_add(column).ok_or(())?;
      let Ok(first_sample) = console_core_number_conversion::index(offset);
      let output = tmp.get_mut(first_sample..).ok_or(())?;
      f.dsp
        .mc
        .warp8x8t
        .call(output, *tmp_stride, input, sampling, depth)
    }
    MaybeTempPixels::NonTemp { dst: dst_2 } => {
      let target = motion_picture_position::<BD>(*dst_2, destination)?;
      f.dsp.mc.warp8x8.call(target, input, sampling, depth)
    }
  };
  result.map_err(|_invalid_warp| ())
}

fn warp_phase(
  projected: Point<i64>,
  correction: Point<i32>,
  coefficients: [i16; 4],
) -> Result<(Point<i32>, crate::src::mc::WarpSampling), console_core_never::Never> {
  let Ok(center_x) =
    console_core_number_conversion::wrapped::<_, i32>(projected.x.wrapping_shr(16));
  let Ok(center_y) =
    console_core_number_conversion::wrapped::<_, i32>(projected.y.wrapping_shr(16));
  let source = Point {
    x: center_x.wrapping_sub(4),
    y: center_y.wrapping_sub(4),
  };
  let Ok(phase_x) = console_core_number_conversion::wrapped::<_, i32>(projected.x & 0xffff);
  let Ok(phase_y) = console_core_number_conversion::wrapped::<_, i32>(projected.y & 0xffff);
  let sampling = crate::src::mc::WarpSampling {
    coefficients,
    phase: Point {
      x: phase_x.wrapping_sub(correction.x) & !0x3f,
      y: phase_y.wrapping_sub(correction.y) & !0x3f,
    },
  };
  Ok((source, sampling))
}

fn warp_phase_correction(
  parameters: &Rav1dWarpedMotionParams,
) -> Result<Point<i32>, console_core_never::Never> {
  let alpha = i32::from(parameters.alpha()).wrapping_mul(4);
  let beta = i32::from(parameters.beta()).wrapping_mul(7);
  let gamma = i32::from(parameters.gamma()).wrapping_mul(4);
  let delta = i32::from(parameters.delta()).wrapping_mul(4);
  Ok(Point {
    x: alpha.wrapping_add(beta),
    y: gamma.wrapping_add(delta),
  })
}

fn warp_affine<BD: BitDepth>(
  f: &Rav1dFrameData,
  emu_edge: &mut ScratchEmuEdge,
  mut dst: MaybeTempPixels<u64>,
  WarpPrediction {
    position,
    dimensions,
    plane,
    reference,
    parameters,
  }: WarpPrediction<'_>,
) -> Result<(), ()> {
  let Ok(coefficients) = parameters.abcd.get();
  let [x_offset, y_offset, xx, xy, yx, yy] = parameters.matrix;
  let depth = BD::from_c(f.bitdepth_max);
  let reference_picture = reference.p.data.as_ref().ok_or(())?;
  let ref_data = &reference_picture.data;
  let Ok(plane_index) = console_core_number_conversion::index(plane);
  let component = ref_data.get(plane_index).ok_or(())?;
  let layout = f.cur.p.layout;
  let subsample = Point {
    x: u32::from(plane != 0 && layout != Rav1dPixelLayout::I444),
    y: u32::from(plane != 0 && layout == Rav1dPixelLayout::I420),
  };
  let [width, height, ..] = dimensions;
  let block = Size {
    width: i32::from(width).wrapping_mul(4_i32.wrapping_shr(subsample.x)),
    height: i32::from(height).wrapping_mul(4_i32.wrapping_shr(subsample.y)),
  };

  match (block.width & 7, block.height & 7) {
    (0, 0) => {}
    _ => return Err(()),
  }

  let rounded_subsample = |amount: i32, shift| {
    amount
      .wrapping_add(i32::from(shift != 0))
      .wrapping_shr(shift)
  };
  let extent = Size {
    width: rounded_subsample(reference.p.p.w, subsample.x),
    height: rounded_subsample(reference.p.p.h, subsample.y),
  };
  let Ok(correction) = warp_phase_correction(parameters);

  for y in (0..block.height).step_by(8) {
    let src_y = position
      .y
      .wrapping_mul(4)
      .wrapping_add(y.wrapping_add(4).wrapping_shl(subsample.y));
    let tx = i64::from(xy)
      .wrapping_mul(i64::from(src_y))
      .wrapping_add(i64::from(x_offset));
    let ty = i64::from(yy)
      .wrapping_mul(i64::from(src_y))
      .wrapping_add(i64::from(y_offset));

    for x in (0..block.width).step_by(8) {
      let src_x = position
        .x
        .wrapping_mul(4)
        .wrapping_add(x.wrapping_add(4).wrapping_shl(subsample.x));
      let transformed_x = i64::from(xx)
        .wrapping_mul(i64::from(src_x))
        .wrapping_add(tx)
        .wrapping_shr(subsample.x);
      let transformed_y = i64::from(yx)
        .wrapping_mul(i64::from(src_x))
        .wrapping_add(ty)
        .wrapping_shr(subsample.y);
      let projected = Point {
        x: transformed_x,
        y: transformed_y,
      };
      let Ok((source, sampling)) = warp_phase(projected, correction, coefficients);
      let placement = WarpPlacement {
        source,
        destination: Point { x, y },
        extent,
        sampling,
      };
      apply_warp_subblock(f, emu_edge, &mut dst, component, depth, placement)?;
    }
  }

  Ok(())
}

fn block_plane_origin(
  position: Bxy,
  sampling: Point<u32>,
) -> Result<Point<i32>, console_core_never::Never> {
  let x = position.x.wrapping_shr(sampling.x).wrapping_mul(4);
  let y = position.y.wrapping_shr(sampling.y).wrapping_mul(4);
  Ok(Point { x, y })
}

struct PredictionEdgeRow {
  block_y: i32,
  chroma_rounding: i32,
  plane: u8,
}

fn prefiltered_top(
  f: &Rav1dFrameData,
  request: PredictionEdgeRow,
) -> Rav1dResult<Option<PrefilteredTop<'_>>> {
  let boundary = request.block_y & !request.chroma_rounding & f.sb_step.wrapping_sub(1);

  match boundary {
    0 => {}
    _ => return Ok(None),
  }

  let shift = u32::try_from(f.sb_shift).map_err(|_invalid_shift| EINVAL)?;
  let previous = request.block_y.wrapping_shr(shift).wrapping_sub(1);
  let row_width = i64::from(f.sb128w).wrapping_mul(128);
  let rows = i64::from(previous).checked_mul(row_width).ok_or(EINVAL)?;
  let bank = f
    .ipred_edge_off
    .checked_mul(u64::from(request.plane))
    .ok_or(EINVAL)?;
  let plane = i64::try_from(bank).map_err(|_outside_edge_buffer| EINVAL)?;
  let row_start = plane.checked_add(rows).ok_or(EINVAL)?;
  Ok(Some(PrefilteredTop {
    samples: &f.ipred_edge,
    row_start,
  }))
}

const INTRA_EDGE_CENTER: u64 = 128;

fn dump_intra_edge<BD: BitDepth>(samples: &[BD::Pixel], size: Size<u64>) -> Rav1dResult {
  let left_start = INTRA_EDGE_CENTER.checked_sub(size.height).ok_or(EINVAL)?;
  let Ok(left_index) = console_core_number_conversion::index(left_start);
  let Ok(center_index) = console_core_number_conversion::index(INTRA_EDGE_CENTER);
  let Ok(top_index) = console_core_number_conversion::index(INTRA_EDGE_CENTER.wrapping_add(1));
  let left = samples.get(left_index..).ok_or(EINVAL)?;
  let center = samples.get(center_index..).ok_or(EINVAL)?;
  let top = samples.get(top_index..).ok_or(EINVAL)?;
  hex_dump::<BD>(
    left,
    Size {
      width: size.height,
      height: 2,
    },
    size.height,
    "l",
  )
  .map_err(|_dump_error| EIO)?;
  hex_dump::<BD>(
    center,
    Size {
      width: 1,
      height: 1,
    },
    0,
    "tl",
  )
  .map_err(|_dump_error| EIO)?;
  hex_dump::<BD>(
    top,
    Size {
      width: size.width,
      height: 2,
    },
    size.width,
    "t",
  )
  .map_err(|_dump_error| EIO)?;
  Ok(())
}

struct IntraPlaneGeometry {
  extent: Size<i32>,
  visible: Size<i32>,
  context: Point<i32>,
  dimensions: &'static TxfmInfo,
  steps: Size<core::num::NonZeroU8>,
}

impl IntraPlaneGeometry {
  fn new(
    extent: Size<i32>,
    visible: Size<i32>,
    context: Point<i32>,
    transform: TxfmSize,
  ) -> Rav1dResult<Self> {
    let dimensions = coefficient_dimensions(transform)?;
    let steps = transform_steps(dimensions)?;
    Ok(Self {
      extent,
      visible,
      context,
      dimensions,
      steps,
    })
  }

  fn subsampled(&self, shifts: Point<u32>, transform: TxfmSize) -> Rav1dResult<Self> {
    let round = |length: i32, shift| {
      length
        .wrapping_add(i32::from(shift != 0))
        .wrapping_shr(shift)
    };
    Self::new(
      Size {
        width: round(self.extent.width, shifts.x),
        height: round(self.extent.height, shifts.y),
      },
      Size {
        width: round(self.visible.width, shifts.x),
        height: round(self.visible.height, shifts.y),
      },
      Point {
        x: self.context.x.wrapping_shr(shifts.x),
        y: self.context.y.wrapping_shr(shifts.y),
      },
      transform,
    )
  }
}

struct IntraReconstruction<'a, BD: BitDepth> {
  f: &'a Rav1dFrameData,
  ts: &'a Rav1dTileState,
  above_context: &'a crate::src::env::BlockContext,
  bd: BD,
  bs: BlockSize,
  b: &'a Av1Block,
  intra: &'a Av1BlockIntra,
  intra_edge_flags: EdgeFlags,
  intra_edge_filter: u8,
  luma: IntraPlaneGeometry,
  chroma: IntraPlaneGeometry,
  chroma_shifts: Point<u32>,
}

struct IntraWork<'a> {
  b: Bxy,
  cf: &'a mut Cf,
  frame_thread: &'a crate::src::internal::Rav1dTaskContextFrameThread,
  l: &'a mut crate::src::env::BlockContext,
  scratch: &'a mut TaskContextScratch,
}

struct IntraCorners {
  top_right: EdgeAvailability,
  bottom_left: EdgeAvailability,
  smooth_flags: i32,
}

fn paint_luma_palette<BD: BitDepth>(
  scan: &IntraReconstruction<'_, BD>,
  t: &mut IntraWork<'_>,
  planes: &mut crate::src::owned_recon::ReconPlanes<'_>,
) -> Rav1dResult {
  let f = scan.f;
  let ts = scan.ts;
  let intra = scan.intra;
  let bw4 = scan.luma.extent.width;
  let bh4 = scan.luma.extent.height;

  let [luma_palette_size, _] = intra.pal_sz;

  match luma_palette_size {
    0 => return Ok(()),
    _ => {}
  }

  let mut y_dst = planes.dst::<BD>(
    0,
    Point {
      x: t.b.x.wrapping_mul(4),
      y: t.b.y.wrapping_mul(4),
    },
  )?;
  let scratch = t.scratch.inter_intra_mut()?;
  let pal_idx = match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(p) = console_core_number_conversion::wrapped_index(t.frame_thread.pass & 1);
      let frame_thread = &*match <[_]>::get(&ts.frame_thread, p) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
        }
      };
      let Ok(len) = console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_mul(
        i32::wrapping_mul(bw4, bh4),
        8,
      ));
      let Ok(pal_idx) = frame_thread
        .pal_idx
        .get_update(|i| u32::wrapping_add(i, len));
      &*f.frame_thread.pal_idx.index((
        ({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(pal_idx);
          numeric_value
        })..,
        ..({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(len);
          numeric_value
        }),
      ))
    }
    ::core::option::Option::None => &scratch.pal_idx_y,
  };
  let frame_palette;
  let pal = match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
    ::core::option::Option::Some(()) => {
      let stride = i64::from(f.b4_stride);
      let position = palette_cache::PalettePosition { x: t.b.x, y: t.b.y };
      let record = palette_cache::record_index(position, stride)?;
      let palette = f.frame_thread.pal.checked_index::<BD>(record)?;
      frame_palette = palette;
      &*frame_palette
    }
    ::core::option::Option::None => {
      let explicit_value = scratch.interintra_edge_pal.pal.buf::<BD>()?;
      explicit_value
    }
  };
  ({
    f.dsp.ipred.pal_pred.call::<BD>(
      &mut y_dst,
      {
        let [_explicit_part, ..] = &*pal;
        _explicit_part
      },
      pal_idx,
      Size {
        width: i32::wrapping_mul(bw4, 4),
        height: i32::wrapping_mul(bh4, 4),
      },
    )?
  });

  match (
    {
      let Ok(active_trace) = debug_block_info(f, t.b);
      active_trace
    },
    PIXEL_TRACE,
  ) {
    (BlockTrace::Enabled, BlockTrace::Enabled) => {
      hex_dump_pic::<BD>(
        &y_dst,
        Size {
          width: {
            let checked_value_5 = u64::try_from(bw4).map_err(|_negative_extent| EINVAL)?;
            checked_value_5
          }
          .wrapping_mul(4),
          height: {
            let checked_value_6 = u64::try_from(bh4).map_err(|_negative_extent| EINVAL)?;
            checked_value_6
          }
          .wrapping_mul(4),
        },
        "y-pal-pred",
      )
      .map_err(|_dump_error| EIO)?;
    }
    (BlockTrace::Disabled, _) | (_, BlockTrace::Disabled) => {}
  }

  Ok(())
}

fn transform_context_span(start: i32, count: u8) -> Rav1dResult<core::ops::Range<u32>> {
  let first = u32::try_from(start).map_err(|_outside_context| EINVAL)?;
  let end = first.checked_add(u32::from(count)).ok_or(EINVAL)?;

  match end {
    0..=32 => Ok(first..end),
    33.. => Err(EINVAL),
  }
}

struct QueuedResidual {
  range: core::ops::Range<u32>,
  end_of_block: i32,
  transform: TxfmType,
}

fn take_queued_residual<BD: BitDepth>(
  frame: &Rav1dFrameData,
  tile: &Rav1dTileState,
  pass: i32,
  count: u32,
) -> Rav1dResult<QueuedResidual> {
  let Ok(pass_index) = console_core_number_conversion::index((pass & 1).cast_unsigned());
  let thread = tile.frame_thread.get(pass_index).ok_or(EINVAL)?;
  let Ok(start) = thread.cf.get_update(|offset| offset.wrapping_add(count));
  let end = start.checked_add(count).ok_or(EINVAL)?;
  let Ok(bytes) = console_core_number_conversion::fixed_index(frame.frame_thread.cf.len());
  let Ok(element_width) =
    console_core_number_conversion::fixed_index(core::mem::size_of::<BD::Coef>());
  let capacity = bytes.checked_div(element_width).ok_or(EINVAL)?;

  match u64::from(end).cmp(&capacity) {
    cmp::Ordering::Greater => return Err(EINVAL),
    cmp::Ordering::Equal | cmp::Ordering::Less => {}
  }

  let Ok(record) = thread.cbi_idx.get_update(|offset| offset.wrapping_add(1));
  let Ok(index) = console_core_number_conversion::index(record);
  let storage = frame.frame_thread.cbi.get(index).ok_or(EINVAL)?;
  let Ok(info) = storage.get();
  let Ok(end_of_block) = info.eob();
  let Ok(transform) = info.txtp();
  Ok(QueuedResidual {
    range: start..end,
    end_of_block: i32::from(end_of_block),
    transform,
  })
}

#[derive(Clone, Copy)]
struct DecodedIntraResidual {
  transform: TxfmSize,
  kind: TxfmType,
  end_of_block: i32,
}

#[derive(Clone, Copy)]
enum IntraResidualPlane {
  Luma,
  Chroma { plane: u8, columns: Point<i32> },
}

fn trace_intra_residual(
  trace: BlockTrace,
  state: Option<&Rav1dTileStateContext>,
  residual: DecodedIntraResidual,
  location: IntraResidualPlane,
) -> Rav1dResult {
  match trace {
    BlockTrace::Disabled => return Ok(()),
    BlockTrace::Enabled => {}
  }

  let entropy = state.ok_or(EINVAL)?;
  let DecodedIntraResidual {
    transform,
    kind,
    end_of_block,
  } = residual;
  let range = entropy.msac.rng;

  match location {
    IntraResidualPlane::Luma => console_core_logging::trace!(
      "Post-y-cf-blk[tx={transform:?},txtp={kind},eob={end_of_block}]: r={range}",
    ),
    IntraResidualPlane::Chroma { plane, columns } => console_core_logging::trace!(
      "Post-uv-cf-blk[pl={plane},tx={transform:?},txtp={kind},eob={end_of_block}]: r={range} [x={},cbx4={}]",
      columns.x,
      columns.y,
    ),
  }

  Ok(())
}

fn apply_intra_residual<BD: BitDepth>(
  frame: &Rav1dFrameData,
  depth: BD,
  output: &mut crate::src::owned_recon::ReconDst<'_>,
  coefficients: &mut [BD::Coef],
  residual: DecodedIntraResidual,
  trace: BlockTrace,
  location: IntraResidualPlane,
) -> Rav1dResult {
  match residual.end_of_block {
    ..=-1 => return Ok(()),
    0.. => {}
  }

  let debug_pixels = match (trace, PIXEL_TRACE) {
    (BlockTrace::Enabled, BlockTrace::Enabled) => {
      let transform = coefficient_dimensions(residual.transform)?;
      let width = u64::from(transform.w);
      let height = u64::from(transform.h);
      let coded = match location {
        IntraResidualPlane::Luma => Size {
          width: height.min(8),
          height: width.min(8),
        },
        IntraResidualPlane::Chroma { .. } => Size {
          width: height,
          height: width,
        },
      };
      let grid = Size {
        width: coded.width.wrapping_mul(4),
        height: coded.height.wrapping_mul(4),
      };
      coef_dump(coefficients, grid, 3, "dq").map_err(|_dump_error| EIO)?;
      Some(Size {
        width: width.wrapping_mul(4),
        height: height.wrapping_mul(4),
      })
    }
    (BlockTrace::Disabled, _) | (_, BlockTrace::Disabled) => None,
  };
  frame.dsp.itx.add::<BD>(
    residual.transform,
    residual.kind,
    output,
    coefficients,
    residual.end_of_block,
    depth,
  )?;

  match debug_pixels {
    Some(size) => hex_dump_pic::<BD>(output, size, "recon").map_err(|_dump_error| EIO),
    None => Ok(()),
  }
}

fn reconstruct_luma_site<BD: BitDepth>(
  scan: &IntraReconstruction<'_, BD>,
  t: &mut IntraWork<'_>,
  planes: &mut crate::src::owned_recon::ReconPlanes<'_>,
  ts_c: &mut Option<&mut Rav1dTileStateContext>,
  chunk: &TransformChunk,
  corners: &IntraCorners,
  site: Point<i32>,
) -> Rav1dResult {
  let f = scan.f;
  let Ok(block_trace) = debug_block_info(f, t.b);
  let ts = scan.ts;
  let above_context = scan.above_context;
  let bd = scan.bd;
  let bs = scan.bs;
  let b = scan.b;
  let intra = scan.intra;
  let intra_edge_filter = scan.intra_edge_filter;
  let bx4 = scan.luma.context.x;
  let by4 = scan.luma.context.y;
  let t_dim = scan.luma.dimensions;
  let init_x = chunk.first.x;
  let init_y = chunk.first.y;
  let sub_w4 = chunk.end.x;
  let sub_h4 = chunk.end.y;
  let Point { x, y } = site;
  let sb_has_tr = matches!(corners.top_right, EdgeAvailability::Present);
  let sb_has_bl = matches!(corners.bottom_left, EdgeAvailability::Present);
  let intra_flags = corners.smooth_flags;
  let mut y_dst = planes.dst::<BD>(
    0,
    Point {
      x: t.b.x.wrapping_mul(4),
      y: t.b.y.wrapping_mul(4),
    },
  )?;
  let mut angle;
  let edge_flags;
  let m;

  match ::core::primitive::bool::then_some(
    !((match intra.pal_sz {
      [element, ..] => element,
    }) != 0),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      angle = {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(intra.y_angle);
        numeric_value
      };
      edge_flags = {
        let Ok(explicit_value) = EdgeFlags::union_all([
          ({
            let Ok(explicit_value) =
              EdgeFlags::I444_TOP_HAS_RIGHT.select(EdgeAvailability::from(
                !((y > init_y || !sb_has_tr)
                  && i32::wrapping_add(x, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, i32>(t_dim.w);
                    numeric_value
                  }) >= sub_w4),
              ));
            explicit_value
          }),
          ({
            let Ok(explicit_value) =
              EdgeFlags::I444_LEFT_HAS_BOTTOM.select(EdgeAvailability::from(
                !(x > init_x
                  || (!sb_has_bl
                    && i32::wrapping_add(y, {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, i32>(t_dim.h);
                      numeric_value
                    }) >= sub_h4)),
              ));
            explicit_value
          }),
        ]);
        explicit_value
      };
      let saved_row = PredictionEdgeRow {
        block_y: t.b.y,
        chroma_rounding: 0,
        plane: 0,
      };
      let prefiltered = prefiltered_top(f, saved_row)?;
      let edge_array = {
        let explicit_value = ({
          let explicit_value = t.scratch.inter_intra_mut()?;
          explicit_value
        })
        .interintra_edge_pal
        .edge
        .buf_mut::<BD>()?;
        explicit_value
      };
      m = {
        rav1d_prepare_intra_edges(
          IntraEdgePreparation {
            position: Point { x: t.b.x, y: t.b.y },
            frame: Size {
              width: ts.tiling.col_end,
              height: ts.tiling.row_end,
            },
            transform: Size {
              width: {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(t_dim.w);
                numeric_value
              },
              height: {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(t_dim.h);
                numeric_value
              },
            },
            has_left: t.b.x > ts.tiling.col_start,
            above: {
              let Ok(above) = AboveEdge::select(
                EdgeAvailability::from(t.b.y > ts.tiling.row_start),
                prefiltered,
              );
              above
            },
            available: edge_flags,
            filter_corner: intra_edge_filter,
          },
          &({
            let Ok(explicit_value) = y_dst.as_src();
            explicit_value
          }),
          {
            let Ok(numeric_value) =
              console_core_number_conversion::wrapped::<_, u8>(intra.y_mode);
            numeric_value
          },
          &mut angle,
          IntraEdgeDestination {
            samples: edge_array,
            center: INTRA_EDGE_CENTER,
          },
          bd,
        )?
      };
      ({
        (*match <[_]>::get(&f.dsp.ipred.intra_pred, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(m);
          numeric_value
        }) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        })
        .call(
          &mut y_dst,
          edge_array,
          IntraPrediction {
            mode: u64::from(m),
            edge_center: INTRA_EDGE_CENTER,
            size: Size {
              width: i32::wrapping_mul(
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(t_dim.w);
                  numeric_value
                },
                4,
              ),
              height: i32::wrapping_mul(
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(t_dim.h);
                  numeric_value
                },
                4,
              ),
            },
            reach: Size {
              width: i32::wrapping_sub(i32::wrapping_mul(4, f.bw), i32::wrapping_mul(4, t.b.x)),
              height: i32::wrapping_sub(i32::wrapping_mul(4, f.bh), i32::wrapping_mul(4, t.b.y)),
            },
            angle: angle | intra_flags,
          },
          bd,
        )?
      });

      match (block_trace, PIXEL_TRACE) {
        (BlockTrace::Enabled, BlockTrace::Enabled) => {
          dump_intra_edge::<BD>(
            edge_array,
            Size {
              width: u64::from(t_dim.w).wrapping_mul(4),
              height: u64::from(t_dim.h).wrapping_mul(4),
            },
          )?;
          hex_dump_pic::<BD>(
            &y_dst,
            Size {
              width: u64::from(t_dim.w).wrapping_mul(4),
              height: u64::from(t_dim.h).wrapping_mul(4),
            },
            "y-intra-pred",
          )
          .map_err(|_dump_error| EIO)?;
        }
        (BlockTrace::Disabled, _) | (_, BlockTrace::Disabled) => {}
      }
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(b.skip == 0, ()) {
    ::core::option::Option::Some(()) => {
      let mut cf_guard;
      let cf;
      let eob;
      let mut txtp = DCT_DCT;

      match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
        ::core::option::Option::Some(()) => {
          let count = u32::from(t_dim.w.min(8))
            .wrapping_mul(u32::from(t_dim.h.min(8)))
            .wrapping_mul(16);
          let queued = take_queued_residual::<BD>(f, ts, t.frame_thread.pass, count)?;
          let Ok(range) = console_core_number_conversion::indices(queued.range);
          cf_guard = f.frame_thread.cf.mut_slice_as(range);
          cf = &mut *cf_guard;
          eob = queued.end_of_block;
          txtp = queued.transform;
        }
        ::core::option::Option::None => {
          let mut cf_ctx = 0;
          let column = bx4.wrapping_add(x);
          let row = by4.wrapping_add(y);
          let above_span = transform_context_span(column, t_dim.w)?;
          let left_span = transform_context_span(row, t_dim.h)?;
          let Ok(above_range) = console_core_number_conversion::indices(above_span);
          let Ok(left_range) = console_core_number_conversion::indices(left_span);
          let residual_left = t.l.lcoef.get_mut().get(left_range).ok_or(EINVAL)?;
          eob = {
            let state = ts_c.as_deref_mut().ok_or(EINVAL)?;

            decode_coefs::<BD>(
              f,
              CoefficientDecoder {
                tile: ts,
                state,
                scratch: &mut t.scratch,
                coefficients: &mut t.cf,
              },
              CoefficientBlock {
                trace: block_trace,
                above: &above_context.lcoef.index(above_range),
                left: residual_left,
                transform: intra.tx,
                size: bs,
                syntax: b,
                plane: 0,
                storage: CfSelect::Task,
              },
              CoefficientOutput {
                transform: &mut txtp,
                residual_context: &mut cf_ctx,
              },
            )?
          };
          cf = {
            let explicit_value = t.cf.select_mut::<BD>()?;
            explicit_value
          };

          let decoded = DecodedIntraResidual {
            transform: intra.tx,
            kind: txtp,
            end_of_block: eob,
          };
          let location = IntraResidualPlane::Luma;
          trace_intra_residual(block_trace, ts_c.as_deref(), decoded, location)?;

          let remaining_width = f.bw.wrapping_sub(t.b.x);
          let remaining_height = f.bh.wrapping_sub(t.b.y);
          let width = i32::from(t_dim.w).min(remaining_width);
          let height = i32::from(t_dim.h).min(remaining_height);
          let Ok(above_count) = console_core_number_conversion::wrapped_index(width);
          let Ok(left_count) = console_core_number_conversion::wrapped_index(height);
          let Ok(above_start) =
            console_core_number_conversion::wrapped_index(bx4.wrapping_add(x));
          let Ok(left_start) = console_core_number_conversion::wrapped_index(by4.wrapping_add(y));
          case_set_al! {
            <16, true>
            l: (&mut t.l, left_count, left_start),
            a: (above_context, above_count, above_start),
            lcoef = (cf_ctx, cf_ctx),
          }
        }
      }

      let decoded = DecodedIntraResidual {
        transform: intra.tx,
        kind: txtp,
        end_of_block: eob,
      };
      let location = IntraResidualPlane::Luma;
      apply_intra_residual(f, bd, &mut y_dst, cf, decoded, block_trace, location)?;
    }
    ::core::option::Option::None => {
      match ::core::primitive::bool::then_some(t.frame_thread.pass == 0, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(above_count) = console_core_number_conversion::index(t_dim.w);
          let Ok(left_count) = console_core_number_conversion::index(t_dim.h);
          let Ok(above_start) =
            console_core_number_conversion::wrapped_index(bx4.wrapping_add(x));
          let Ok(left_start) = console_core_number_conversion::wrapped_index(by4.wrapping_add(y));
          case_set_al! {
            <16, false>
            l: (&mut t.l, left_count, left_start),
            a: (above_context, above_count, above_start),
            lcoef = (0x40, 0x40),
          }
        }
        ::core::option::Option::None => {}
      }
    }
  }

  Ok(())
}

fn intra_chunk_corners(
  chunk: &TransformChunk,
  visible: Size<i32>,
  shifts: Point<u32>,
  available: EdgeFlags,
  [right_edge, bottom_edge]: [EdgeFlags; 2],
  smooth_flags: i32,
) -> Result<IntraCorners, console_core_never::Never> {
  let right = chunk.first.x.wrapping_add(16).wrapping_shr(shifts.x);
  let bottom = chunk.first.y.wrapping_add(16).wrapping_shr(shifts.y);
  let top_right = right < visible.width
    || (chunk.first.y == 0
      && matches!(
        available.availability(right_edge),
        Ok(EdgeAvailability::Present)
      ));
  let bottom_left = chunk.first.x == 0
    && (bottom < visible.height
      || matches!(
        available.availability(bottom_edge),
        Ok(EdgeAvailability::Present)
      ));
  Ok(IntraCorners {
    top_right: EdgeAvailability::from(top_right),
    bottom_left: EdgeAvailability::from(bottom_left),
    smooth_flags,
  })
}

fn reconstruct_intra_luma<BD: BitDepth>(
  scan: &IntraReconstruction<'_, BD>,
  t: &mut IntraWork<'_>,
  planes: &mut crate::src::owned_recon::ReconPlanes<'_>,
  ts_c: &mut Option<&mut Rav1dTileStateContext>,
  chunk: &TransformChunk,
) -> Rav1dResult {
  paint_luma_palette(scan, t, planes)?;
  let above = sm_flag(scan.above_context, scan.luma.context.x)?;
  let left = sm_flag(&t.l, scan.luma.context.y)?;
  let filter = i32::from(scan.intra_edge_filter).wrapping_shl(10);
  let Ok(edges) = intra_chunk_corners(
    chunk,
    scan.luma.visible,
    Point { x: 0, y: 0 },
    scan.intra_edge_flags,
    [
      EdgeFlags::I444_TOP_HAS_RIGHT,
      EdgeFlags::I444_LEFT_HAS_BOTTOM,
    ],
    above | left | filter,
  );
  let origin = t.b;
  let steps = scan.luma.steps;

  for y in (chunk.first.y..chunk.end.y).step_by(core::num::NonZero::from(steps.height).get()) {
    for x in (chunk.first.x..chunk.end.x).step_by(core::num::NonZero::from(steps.width).get()) {
      t.b = Bxy {
        x: origin.x.wrapping_add(x),
        y: origin.y.wrapping_add(y),
      };
      reconstruct_luma_site(scan, t, planes, ts_c, chunk, &edges, Point { x, y })?;
    }
  }

  t.b = origin;

  Ok(())
}

fn paint_chroma_palette<BD: BitDepth>(
  scan: &IntraReconstruction<'_, BD>,
  t: &mut IntraWork<'_>,
  planes: &mut crate::src::owned_recon::ReconPlanes<'_>,
) -> Rav1dResult {
  let [_, palette_count] = scan.intra.pal_sz;

  match palette_count {
    0 => return Ok(()),
    _ => {}
  }

  let f = scan.f;
  let Size { width, height } = scan.chroma.extent;
  let Ok(origin) = block_plane_origin(t.b, scan.chroma_shifts);
  let stored;
  let (colors, indices) = match t.frame_thread.pass {
    0 => {
      let scratch = t.scratch.inter_intra_mut()?;
      let cache = scratch.interintra_edge_pal.pal.buf::<BD>()?;
      (cache, scratch.pal_idx_uv.as_slice())
    }
    pass => {
      let Ok(pass_index) = console_core_number_conversion::wrapped_index(pass & 1);
      let pass_context = scan.ts.frame_thread.get(pass_index).ok_or(EINVAL)?;
      let stride = i64::from(f.b4_stride);
      let position = palette_cache::PalettePosition { x: t.b.x, y: t.b.y };
      let record = palette_cache::record_index(position, stride)?;
      let Ok(count) = console_core_number_conversion::wrapped::<_, u32>(
        width.wrapping_mul(height).wrapping_mul(8),
      );
      let Ok(offset) = pass_context.pal_idx.get_update(|i| i.wrapping_add(count));
      let end = offset.checked_add(count).ok_or(EINVAL)?;
      let Ok(range) = console_core_number_conversion::indices(offset..end);
      let palette = f.frame_thread.pal.checked_index::<BD>(record)?;
      stored = palette;
      (&*stored, &*f.frame_thread.pal_idx.index(range))
    }
  };
  let pixels = Size {
    width: width.wrapping_mul(4),
    height: height.wrapping_mul(4),
  };
  let Ok(trace) = debug_block_info(f, t.b);

  for (plane, label) in [(1_u8, "u-pal-pred"), (2, "v-pal-pred")] {
    let Ok(index) = console_core_number_conversion::index(plane);
    let colors_for_plane = colors.get(index).ok_or(EINVAL)?;
    let mut output = planes.dst::<BD>(plane, origin)?;
    f.dsp
      .ipred
      .pal_pred
      .call::<BD>(&mut output, colors_for_plane, indices, pixels)?;

    match (trace, PIXEL_TRACE) {
      (BlockTrace::Enabled, BlockTrace::Enabled) => {
        let dump_width = u64::try_from(pixels.width).map_err(|_invalid_width| EINVAL)?;
        let dump_height = u64::try_from(pixels.height).map_err(|_invalid_height| EINVAL)?;
        let dimensions = Size {
          width: dump_width,
          height: dump_height,
        };
        hex_dump_pic::<BD>(&output, dimensions, label).map_err(|_dump_error| EIO)?;
      }
      (BlockTrace::Disabled, _) | (_, BlockTrace::Disabled) => {}
    }
  }

  Ok(())
}

fn prepare_chroma_intra_predictors<BD: BitDepth>(
  scan: &IntraReconstruction<'_, BD>,
  t: &mut IntraWork<'_>,
  planes: &mut crate::src::owned_recon::ReconPlanes<'_>,
  chunk: &TransformChunk,
) -> Rav1dResult {
  let f = scan.f;
  let ts = scan.ts;
  let bd = scan.bd;
  let intra = scan.intra;
  let cw4 = scan.chroma.visible.width;
  let ch4 = scan.chroma.visible.height;
  let t_dim = scan.luma.dimensions;
  let uv_t_dim = scan.chroma.dimensions;
  let cbw4 = scan.chroma.extent.width;
  let cbh4 = scan.chroma.extent.height;
  let Ok(ss_hor) = console_core_number_conversion::wrapped::<_, i32>(scan.chroma_shifts.x);
  let Ok(ss_ver) = console_core_number_conversion::wrapped::<_, i32>(scan.chroma_shifts.y);
  let chroma_shifts = scan.chroma_shifts;
  let init_x = chunk.first.x;
  let init_y = chunk.first.y;

  match ::core::primitive::bool::then_some(intra.uv_mode == CFL_PRED, ()) {
    ::core::option::Option::Some(()) => {
      assert!(init_x == 0 && init_y == 0);

      let scratch = t.scratch.inter_intra_mut()?;
      let ac = scratch.ac_txtp_map.ac_mut()?;
      let y_src = {
        let explicit_value = planes.src::<BD>(
          0,
          Point {
            x: (t.b.x & !ss_hor).wrapping_mul(4),
            y: (t.b.y & !ss_ver).wrapping_mul(4),
          },
        )?;
        explicit_value
      };

      let furthest_r = i32::wrapping_sub(
        i32::wrapping_add(
          i32::wrapping_shl(cw4, {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
            numeric_value
          }),
          {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(t_dim.w);
            numeric_value
          },
        ),
        1,
      ) & !i32::wrapping_sub(
        {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(t_dim.w);
          numeric_value
        },
        1,
      );
      let furthest_b = i32::wrapping_sub(
        i32::wrapping_add(
          i32::wrapping_shl(ch4, {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
            numeric_value
          }),
          {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(t_dim.h);
            numeric_value
          },
        ),
        1,
      ) & !i32::wrapping_sub(
        {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(t_dim.h);
          numeric_value
        },
        1,
      );
      let layout = f.cur.p.layout.try_into().map_err(|_no_chroma| EINVAL)?;
      let cfl_ac = f.dsp.ipred.cfl_ac.checked(layout)?;
      let Ok(horizontal_shift) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
      let Ok(vertical_shift) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
      let extraction = CflAcExtraction {
        layout,
        size: Size {
          width: cbw4.wrapping_mul(4),
          height: cbh4.wrapping_mul(4),
        },
        padding: Size {
          width: cbw4.wrapping_sub(furthest_r.wrapping_shr(horizontal_shift)),
          height: cbh4.wrapping_sub(furthest_b.wrapping_shr(vertical_shift)),
        },
      };
      let Ok(()) = cfl_ac.call::<BD>(ac, &y_src, extraction);

      for (plane, alpha) in [1_u8, 2].into_iter().zip(intra.cfl_alpha) {
        match alpha {
          0 => continue,
          _ => {}
        }

        let mut angle = 0;
        let saved_row = PredictionEdgeRow {
          block_y: t.b.y,
          chroma_rounding: ss_ver,
          plane,
        };
        let prefiltered = prefiltered_top(f, saved_row)?;
        let xpos = i32::wrapping_shr(t.b.x, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
          numeric_value
        });
        let ypos = i32::wrapping_shr(t.b.y, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
          numeric_value
        });
        let xstart = i32::wrapping_shr(ts.tiling.col_start, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
          numeric_value
        });
        let ystart = i32::wrapping_shr(ts.tiling.row_start, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
          numeric_value
        });
        let edge_array = scratch.interintra_edge_pal.edge.buf_mut::<BD>()?;
        let Ok(origin) = block_plane_origin(t.b, chroma_shifts);
        let mut uv_dst = planes.dst::<BD>(plane, origin)?;
        let m: IntraPredMode = {
          rav1d_prepare_intra_edges(
            IntraEdgePreparation {
              position: Point { x: xpos, y: ypos },
              frame: Size {
                width: i32::wrapping_shr(ts.tiling.col_end, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                  numeric_value
                }),
                height: i32::wrapping_shr(ts.tiling.row_end, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                  numeric_value
                }),
              },
              transform: Size {
                width: {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w);
                  numeric_value
                },
                height: {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h);
                  numeric_value
                },
              },
              has_left: xpos > xstart,
              above: {
                let Ok(above) =
                  AboveEdge::select(EdgeAvailability::from(ypos > ystart), prefiltered);
                above
              },
              available: EdgeFlags::EMPTY,
              filter_corner: 0,
            },
            &({
              let Ok(explicit_value) = uv_dst.as_src();
              explicit_value
            }),
            DC_PRED,
            &mut angle,
            IntraEdgeDestination {
              samples: edge_array,
              center: INTRA_EDGE_CENTER,
            },
            bd,
          )?
        };
        let Ok(index) = console_core_number_conversion::index(m);
        let prediction = f.dsp.ipred.cfl_pred.get(index).ok_or(EINVAL)?;
        prediction.call(
          &mut uv_dst,
          edge_array,
          ac,
          CflPrediction {
            mode: u64::from(m),
            edge_center: INTRA_EDGE_CENTER,
            size: Size {
              width: i32::from(uv_t_dim.w).wrapping_mul(4),
              height: i32::from(uv_t_dim.h).wrapping_mul(4),
            },
            alpha: i32::from(alpha),
          },
          bd,
        )?;
      }

      match (
        {
          let Ok(active_trace) = debug_block_info(f, t.b);
          active_trace
        },
        PIXEL_TRACE,
      ) {
        (BlockTrace::Enabled, BlockTrace::Enabled) => {
          let width = u64::try_from(cbw4).map_err(|_outside_width| EINVAL)?;
          let height = u64::try_from(cbh4).map_err(|_outside_height| EINVAL)?;
          let output_size = Size {
            width: width.wrapping_mul(4),
            height: height.wrapping_mul(4),
          };
          coef_dump(ac, output_size, 3, "ac").map_err(|_dump_error| EIO)?;
          let Ok(origin) = block_plane_origin(t.b, chroma_shifts);

          for (plane, label) in [(1_u8, "u-cfl-pred"), (2, "v-cfl-pred")] {
            let uv_dst = planes.dst::<BD>(plane, origin)?;
            hex_dump_pic::<BD>(&uv_dst, output_size, label).map_err(|_dump_error| EIO)?;
          }
        }
        (BlockTrace::Disabled, _) | (_, BlockTrace::Disabled) => {}
      }
    }
    ::core::option::Option::None => paint_chroma_palette(scan, t, planes)?,
  }

  Ok(())
}

fn reconstruct_chroma_site<BD: BitDepth>(
  scan: &IntraReconstruction<'_, BD>,
  t: &mut IntraWork<'_>,
  planes: &mut crate::src::owned_recon::ReconPlanes<'_>,
  ts_c: &mut Option<&mut Rav1dTileStateContext>,
  chunk: &TransformChunk,
  corners: &IntraCorners,
  site: Point<i32>,
  plane: u8,
) -> Rav1dResult {
  let f = scan.f;
  let Ok(block_trace) = debug_block_info(f, t.b);
  let ts = scan.ts;
  let above_context = scan.above_context;
  let bd = scan.bd;
  let bs = scan.bs;
  let b = scan.b;
  let intra = scan.intra;
  let intra_edge_filter = scan.intra_edge_filter;
  let intra_edge_filter_flag = i32::from(scan.intra_edge_filter).wrapping_shl(10);
  let cbx4 = scan.chroma.context.x;
  let cby4 = scan.chroma.context.y;
  let uv_t_dim = scan.chroma.dimensions;
  let Ok(ss_hor) = console_core_number_conversion::wrapped::<_, i32>(scan.chroma_shifts.x);
  let Ok(ss_ver) = console_core_number_conversion::wrapped::<_, i32>(scan.chroma_shifts.y);
  let chroma_shifts = scan.chroma_shifts;
  let init_x = chunk.first.x;
  let init_y = chunk.first.y;
  let sub_cw4 = scan.chroma.visible.width.min(
    chunk
      .first
      .x
      .wrapping_add(16)
      .wrapping_shr(scan.chroma_shifts.x),
  );
  let sub_ch4 = scan.chroma.visible.height.min(
    chunk
      .first
      .y
      .wrapping_add(16)
      .wrapping_shr(scan.chroma_shifts.y),
  );
  let Ok(pl) = console_core_number_conversion::index(plane);
  let Point { x, y } = site;
  let uv_sb_has_tr = matches!(corners.top_right, EdgeAvailability::Present);
  let uv_sb_has_bl = matches!(corners.bottom_left, EdgeAvailability::Present);
  let sm_uv_fl = corners.smooth_flags;
  let Ok(origin) = block_plane_origin(t.b, chroma_shifts);
  let mut uv_dst = planes.dst::<BD>(plane.wrapping_add(1), origin)?;
  let mut angle;
  let edge_flags;
  let uv_mode;
  let xpos;
  let ypos;
  let xstart;
  let ystart;
  let m;

  match ::core::primitive::bool::then_some(
    !(intra.uv_mode == CFL_PRED
      && *match <[_]>::get(&intra.cfl_alpha, pl) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
        }
      } != 0
      || (match intra.pal_sz {
        [_, element, ..] => element,
      }) != 0),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      angle = {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(intra.uv_angle);
        numeric_value
      };

      edge_flags = ({
        let Ok(explicit_value) = EdgeFlags::I444_TOP_HAS_RIGHT.select(EdgeAvailability::from(
          !((y
            > i32::wrapping_shr(init_y, {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
              numeric_value
            })
            || !uv_sb_has_tr)
            && i32::wrapping_add(x, {
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w);
              numeric_value
            }) >= sub_cw4),
        ));
        explicit_value
      }) | ({
        let Ok(explicit_value) = EdgeFlags::I444_LEFT_HAS_BOTTOM.select(EdgeAvailability::from(
          !(x
            > i32::wrapping_shr(init_x, {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
              numeric_value
            })
            || !uv_sb_has_bl
              && i32::wrapping_add(y, {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h);
                numeric_value
              }) >= sub_ch4),
        ));
        explicit_value
      });
      let chroma_plane = u8::try_from(pl).map_err(|_invalid_plane| EINVAL)?;
      let saved_row = PredictionEdgeRow {
        block_y: t.b.y,
        chroma_rounding: ss_ver,
        plane: chroma_plane.wrapping_add(1),
      };
      let prefiltered = prefiltered_top(f, saved_row)?;
      uv_mode = match ::core::primitive::bool::then_some(intra.uv_mode == CFL_PRED, ()) {
        ::core::option::Option::Some(()) => DC_PRED,
        ::core::option::Option::None => intra.uv_mode,
      };
      xpos = i32::wrapping_shr(t.b.x, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
        numeric_value
      });
      ypos = i32::wrapping_shr(t.b.y, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
        numeric_value
      });
      xstart = i32::wrapping_shr(ts.tiling.col_start, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
        numeric_value
      });
      ystart = i32::wrapping_shr(ts.tiling.row_start, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
        numeric_value
      });
      let edge_array = {
        let explicit_value = ({
          let explicit_value = t.scratch.inter_intra_mut()?;
          explicit_value
        })
        .interintra_edge_pal
        .edge
        .buf_mut::<BD>()?;
        explicit_value
      };
      m = {
        rav1d_prepare_intra_edges(
          IntraEdgePreparation {
            position: Point { x: xpos, y: ypos },
            frame: Size {
              width: i32::wrapping_shr(ts.tiling.col_end, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                numeric_value
              }),
              height: i32::wrapping_shr(ts.tiling.row_end, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                numeric_value
              }),
            },
            transform: Size {
              width: {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w);
                numeric_value
              },
              height: {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h);
                numeric_value
              },
            },
            has_left: xpos > xstart,
            above: {
              let Ok(above) =
                AboveEdge::select(EdgeAvailability::from(ypos > ystart), prefiltered);
              above
            },
            available: edge_flags,
            filter_corner: intra_edge_filter,
          },
          &({
            let Ok(explicit_value) = uv_dst.as_src();
            explicit_value
          }),
          uv_mode,
          &mut angle,
          IntraEdgeDestination {
            samples: edge_array,
            center: INTRA_EDGE_CENTER,
          },
          bd,
        )?
      };
      angle |= intra_edge_filter_flag;
      ({
        (*match <[_]>::get(&f.dsp.ipred.intra_pred, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(m);
          numeric_value
        }) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        })
        .call(
          &mut uv_dst,
          edge_array,
          IntraPrediction {
            mode: u64::from(m),
            edge_center: INTRA_EDGE_CENTER,
            size: Size {
              width: i32::wrapping_mul(
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w);
                  numeric_value
                },
                4,
              ),
              height: i32::wrapping_mul(
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h);
                  numeric_value
                },
                4,
              ),
            },
            reach: Size {
              width: i32::wrapping_shr(
                i32::wrapping_sub(
                  i32::wrapping_add(i32::wrapping_mul(4, f.bw), ss_hor),
                  i32::wrapping_mul(4, t.b.x & !ss_hor),
                ),
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                  numeric_value
                },
              ),
              height: i32::wrapping_shr(
                i32::wrapping_sub(
                  i32::wrapping_add(i32::wrapping_mul(4, f.bh), ss_ver),
                  i32::wrapping_mul(4, t.b.y & !ss_ver),
                ),
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                  numeric_value
                },
              ),
            },
            angle: angle | sm_uv_fl,
          },
          bd,
        )?
      });

      match (block_trace, PIXEL_TRACE) {
        (BlockTrace::Enabled, BlockTrace::Enabled) => {
          let size = Size {
            width: u64::from(uv_t_dim.w).wrapping_mul(4),
            height: u64::from(uv_t_dim.h).wrapping_mul(4),
          };
          let &label = ["u-intra-pred", "v-intra-pred"].get(pl).ok_or(EINVAL)?;
          dump_intra_edge::<BD>(edge_array, size)?;
          hex_dump_pic::<BD>(&uv_dst, size, label).map_err(|_dump_error| EIO)?;
        }
        (BlockTrace::Disabled, _) | (_, BlockTrace::Disabled) => {}
      }
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(b.skip == 0, ()) {
    ::core::option::Option::Some(()) => {
      let mut txtp = DCT_DCT;
      let eob;
      let mut cf_guard;
      let cf;

      match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
        ::core::option::Option::Some(()) => {
          let count = u32::from(uv_t_dim.w)
            .wrapping_mul(u32::from(uv_t_dim.h))
            .wrapping_mul(16);
          let queued = take_queued_residual::<BD>(f, ts, t.frame_thread.pass, count)?;
          let Ok(range) = console_core_number_conversion::indices(queued.range);
          cf_guard = f.frame_thread.cf.mut_slice_as(range);
          cf = &mut *cf_guard;
          eob = queued.end_of_block;
          txtp = queued.transform;
        }
        ::core::option::Option::None => {
          let mut cf_ctx: u8 = 0;
          let column = cbx4.wrapping_add(x);
          let row = cby4.wrapping_add(y);
          let above_span = transform_context_span(column, uv_t_dim.w)?;
          let left_span = transform_context_span(row, uv_t_dim.h)?;
          let Ok(above_range) = console_core_number_conversion::indices(above_span);
          let Ok(left_range) = console_core_number_conversion::indices(left_span);
          let a_ccoef = above_context.ccoef.get(pl).ok_or(EINVAL)?;
          let l_ccoef = t.l.ccoef.get_mut(pl).ok_or(EINVAL)?;
          let residual_left = l_ccoef.get_mut().get(left_range).ok_or(EINVAL)?;
          eob = {
            let state = ts_c.as_deref_mut().ok_or(EINVAL)?;

            decode_coefs::<BD>(
              f,
              CoefficientDecoder {
                tile: ts,
                state,
                scratch: &mut t.scratch,
                coefficients: &mut t.cf,
              },
              CoefficientBlock {
                trace: block_trace,
                above: &a_ccoef.index(above_range),
                left: residual_left,
                transform: b.uvtx,
                size: bs,
                syntax: b,
                plane: plane.wrapping_add(1),
                storage: CfSelect::Task,
              },
              CoefficientOutput {
                transform: &mut txtp,
                residual_context: &mut cf_ctx,
              },
            )?
          };
          cf = {
            let explicit_value = t.cf.select_mut::<BD>()?;
            explicit_value
          };

          let decoded = DecodedIntraResidual {
            transform: b.uvtx,
            kind: txtp,
            end_of_block: eob,
          };
          let location = IntraResidualPlane::Chroma {
            plane,
            columns: Point { x, y: cbx4 },
          };
          trace_intra_residual(block_trace, ts_c.as_deref(), decoded, location)?;

          let remaining_width = f
            .bw
            .wrapping_sub(t.b.x)
            .wrapping_add(ss_hor)
            .wrapping_shr(chroma_shifts.x);
          let remaining_height = f
            .bh
            .wrapping_sub(t.b.y)
            .wrapping_add(ss_ver)
            .wrapping_shr(chroma_shifts.y);
          let width = i32::from(uv_t_dim.w).min(remaining_width);
          let height = i32::from(uv_t_dim.h).min(remaining_height);
          let Ok(above_count) = console_core_number_conversion::wrapped_index(width);
          let Ok(left_count) = console_core_number_conversion::wrapped_index(height);
          let Ok(above_start) =
            console_core_number_conversion::wrapped_index(cbx4.wrapping_add(x));
          let Ok(left_start) =
            console_core_number_conversion::wrapped_index(cby4.wrapping_add(y));
          case_set_al! {
            <16, true>
            l: (&mut t.l, left_count, left_start),
            a: (above_context, above_count, above_start),
            ccoef[pl] = (cf_ctx, cf_ctx),
          }
        }
      }

      let decoded = DecodedIntraResidual {
        transform: b.uvtx,
        kind: txtp,
        end_of_block: eob,
      };
      let location = IntraResidualPlane::Chroma {
        plane,
        columns: Point { x, y: cbx4 },
      };
      apply_intra_residual(f, bd, &mut uv_dst, cf, decoded, block_trace, location)?;
    }
    ::core::option::Option::None => {
      match ::core::primitive::bool::then_some(t.frame_thread.pass == 0, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(above_count) = console_core_number_conversion::index(uv_t_dim.w);
          let Ok(left_count) = console_core_number_conversion::index(uv_t_dim.h);
          let Ok(above_start) =
            console_core_number_conversion::wrapped_index(cbx4.wrapping_add(x));
          let Ok(left_start) =
            console_core_number_conversion::wrapped_index(cby4.wrapping_add(y));
          case_set_al! {
            <16, false>
            l: (&mut t.l, left_count, left_start),
            a: (above_context, above_count, above_start),
            ccoef[pl] = (0x40, 0x40),
          }
        }
        ::core::option::Option::None => {}
      }
    }
  }

  Ok(())
}

fn reconstruct_intra_chroma<BD: BitDepth>(
  scan: &IntraReconstruction<'_, BD>,
  t: &mut IntraWork<'_>,
  planes: &mut crate::src::owned_recon::ReconPlanes<'_>,
  ts_c: &mut Option<&mut Rav1dTileStateContext>,
  chunk: &TransformChunk,
) -> Rav1dResult {
  prepare_chroma_intra_predictors(scan, t, planes, chunk)?;
  let above = sm_uv_flag(scan.above_context, scan.chroma.context.x)?;
  let left = sm_uv_flag(&t.l, scan.chroma.context.y)?;
  let layout = scan.f.cur.p.layout;
  let Ok(edges) = intra_chunk_corners(
    chunk,
    scan.chroma.visible,
    scan.chroma_shifts,
    scan.intra_edge_flags,
    [
      EdgeFlags::I420_TOP_HAS_RIGHT >> layout,
      EdgeFlags::I420_LEFT_HAS_BOTTOM >> layout,
    ],
    above | left,
  );
  let origin = t.b;
  let shifts = scan.chroma_shifts;
  let steps = scan.chroma.steps;
  let final_row = scan
    .chroma
    .visible
    .height
    .min(chunk.first.y.wrapping_add(16).wrapping_shr(shifts.y));
  let final_column = scan
    .chroma
    .visible
    .width
    .min(chunk.first.x.wrapping_add(16).wrapping_shr(shifts.x));
  let rows = (chunk.first.y.wrapping_shr(shifts.y)..final_row)
    .step_by(core::num::NonZero::from(steps.height).get());
  let columns = (chunk.first.x.wrapping_shr(shifts.x)..final_column)
    .step_by(core::num::NonZero::from(steps.width).get());

  for plane in 0_u8..2 {
    for y in rows.clone() {
      for x in columns.clone() {
        t.b = Bxy {
          x: origin.x.wrapping_add(x.wrapping_shl(shifts.x)),
          y: origin.y.wrapping_add(y.wrapping_shl(shifts.y)),
        };
        reconstruct_chroma_site(scan, t, planes, ts_c, chunk, &edges, Point { x, y }, plane)?;
      }
    }
  }

  t.b = origin;

  Ok(())
}

#[derive(Clone, Copy)]
enum IntraPlanes {
  LumaOnly,
  LumaAndChroma,
}

pub(crate) fn rav1d_recon_b_intra<BD: BitDepth>(
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
  mut ts_c: Option<&mut Rav1dTileStateContext>,
  bs: BlockSize,
  intra_edge_flags: EdgeFlags,
  b: &Av1Block,
  intra: &Av1BlockIntra,
) -> Rav1dResult {
  let above_context = f.above_context(t.above_index)?;
  let ts = f.tile(t.tile_index)?;
  let picture = f.cur.data.as_ref().ok_or(EINVAL)?;
  let Ok(mut planes) =
    crate::src::owned_recon::ReconPlanes::bind(&picture.data, &mut t.recon_band);
  let layout = f.cur.p.layout;
  let chroma_shifts = Point {
    x: u32::from(layout != Rav1dPixelLayout::I444),
    y: u32::from(layout == Rav1dPixelLayout::I420),
  };
  let Ok(&[width, height, ..]) = bs.dimensions();
  let ext = Size {
    width: i32::from(width),
    height: i32::from(height),
  };
  let visible = Size {
    width: ext.width.min(f.bw.wrapping_sub(t.b.x)),
    height: ext.height.min(f.bh.wrapping_sub(t.b.y)),
  };
  let context = Point {
    x: t.b.x & 31,
    y: t.b.y & 31,
  };
  let luma = IntraPlaneGeometry::new(ext, visible, context, intra.tx)?;
  let chroma = luma.subsampled(chroma_shifts, b.uvtx)?;
  let planes_to_reconstruct = match (layout != Rav1dPixelLayout::I400
    && (u32::from(width) > chroma_shifts.x || t.b.x & 1 != 0)
    && (u32::from(height) > chroma_shifts.y || t.b.y & 1 != 0))
    .then_some(())
  {
    Some(()) => IntraPlanes::LumaAndChroma,
    None => IntraPlanes::LumaOnly,
  };
  let sequence = f.seq_hdr()?;
  let scan = IntraReconstruction {
    f,
    ts,
    above_context,
    bd: BD::from_c(f.bitdepth_max),
    bs,
    b,
    intra,
    intra_edge_flags,
    intra_edge_filter: sequence.intra_edge_filter,
    luma,
    chroma,
    chroma_shifts,
  };
  let mut work = IntraWork {
    b: t.b,
    cf: &mut t.cf,
    frame_thread: &t.frame_thread,
    l: &mut t.l,
    scratch: &mut t.scratch,
  };
  reconstruct_intra_chunks(
    &scan,
    &mut work,
    &mut planes,
    &mut ts_c,
    planes_to_reconstruct,
  )
}

fn reconstruct_intra_chunks<BD: BitDepth>(
  scan: &IntraReconstruction<'_, BD>,
  work: &mut IntraWork<'_>,
  planes: &mut crate::src::owned_recon::ReconPlanes<'_>,
  state: &mut Option<&mut Rav1dTileStateContext>,
  planes_to_reconstruct: IntraPlanes,
) -> Rav1dResult {
  let bounds = scan.luma.visible;

  for y in (0..bounds.height).step_by(16) {
    for x in (0..bounds.width).step_by(16) {
      let chunk = TransformChunk {
        first: Point { x, y },
        end: Point {
          x: bounds.width.min(x.wrapping_add(16)),
          y: bounds.height.min(y.wrapping_add(16)),
        },
      };
      reconstruct_intra_luma(scan, work, planes, state, &chunk)?;

      match planes_to_reconstruct {
        IntraPlanes::LumaAndChroma => {
          reconstruct_intra_chroma(scan, work, planes, state, &chunk)?
        }
        IntraPlanes::LumaOnly => {}
      }
    }
  }

  Ok(())
}

struct WedgeMaskLookup {
  block: BlockSize,
  layout: u8,
  polarity: crate::src::levels::MaskPolarity,
  wedge: u8,
}

fn wedge_mask(lookup: WedgeMaskLookup) -> Result<&'static [u8], ()> {
  let Ok(ordinal) = lookup.block.ordinal();
  let Ok(block_index) = console_core_number_conversion::index(ordinal);
  let Ok(layout_index) = console_core_number_conversion::index(lookup.layout);
  let Ok(polarity_index) = console_core_number_conversion::index(u8::from(lookup.polarity));
  let Ok(wedge_index) = console_core_number_conversion::index(lookup.wedge);
  dav1d_wedge_masks
    .get(block_index)
    .and_then(|block| block.get(layout_index))
    .and_then(|layout| layout.get(polarity_index))
    .and_then(|variants| variants.get(wedge_index))
    .copied()
    .ok_or(())
}

fn interintra_mask(
  block: BlockSize,
  layout: u8,
  inter: &Av1BlockInter,
  kind: InterIntraType,
) -> Result<&'static [u8], ()> {
  match kind {
    InterIntraType::Blend => {
      let Ok(mode) = inter.nd.one_d.interintra_mode.get();
      let Ok(block_ordinal) = block.ordinal();
      let Ok(mode_ordinal) = mode.ordinal();
      let Ok(block_index) = console_core_number_conversion::index(block_ordinal);
      let Ok(layout_index) = console_core_number_conversion::index(layout);
      let Ok(mode_index) = console_core_number_conversion::index(mode_ordinal);
      dav1d_ii_masks
        .get(block_index)
        .and_then(|block_masks| block_masks.get(layout_index))
        .and_then(|modes| modes.get(mode_index))
        .copied()
        .ok_or(())
    }
    InterIntraType::Wedge => wedge_mask(WedgeMaskLookup {
      block,
      layout,
      polarity: crate::src::levels::MaskPolarity::Normal,
      wedge: inter.nd.one_d.wedge_idx,
    }),
  }
}

const INTER_INTRA_EDGE_CENTER: u64 = 32;

fn interintra_intra_mode(
  mode: InterIntraPredMode,
) -> Result<IntraPredMode, console_core_never::Never> {
  Ok(match mode {
    InterIntraPredMode::Dc => DC_PRED,
    InterIntraPredMode::Vert => VERT_PRED,
    InterIntraPredMode::Hor => HOR_PRED,
    InterIntraPredMode::Smooth => SMOOTH_PRED,
  })
}

struct InterGeometry {
  dimensions: &'static [u8; 4],
  extent: Size<i32>,
  visible: Size<i32>,
  context_cell: Point<i32>,
  chroma_extent: Size<i32>,
  chroma_cell: Point<i32>,
  shifts: Point<u32>,
  chroma_layout: Rav1dPixelLayoutSubSampled,
  mask_layout: u8,
  has_chroma: bool,
}

impl InterGeometry {
  fn new(
    frame: &Rav1dFrameData,
    origin: Bxy,
    block: BlockSize,
  ) -> Result<Self, console_core_never::Never> {
    let Ok(dimensions) = block.dimensions();
    let &[width, height, ..] = dimensions;
    let extent = Size {
      width: i32::from(width),
      height: i32::from(height),
    };
    let visible = Size {
      width: extent.width.min(frame.bw.wrapping_sub(origin.x)),
      height: extent.height.min(frame.bh.wrapping_sub(origin.y)),
    };
    let layout = frame.cur.p.layout;
    let shifts = Point {
      x: u32::from(layout != Rav1dPixelLayout::I444),
      y: u32::from(layout == Rav1dPixelLayout::I420),
    };
    let horizontal = i32::from(shifts.x != 0);
    let vertical = i32::from(shifts.y != 0);
    let context_cell = Point {
      x: origin.x & 31,
      y: origin.y & 31,
    };
    let chroma_cell = Point {
      x: context_cell.x.wrapping_shr(shifts.x),
      y: context_cell.y.wrapping_shr(shifts.y),
    };
    let chroma_extent = Size {
      width: extent.width.wrapping_add(horizontal).wrapping_shr(shifts.x),
      height: extent.height.wrapping_add(vertical).wrapping_shr(shifts.y),
    };
    let has_chroma = layout != Rav1dPixelLayout::I400
      && (extent.width > horizontal || origin.x & 1 != 0)
      && (extent.height > vertical || origin.y & 1 != 0);
    let (chroma_layout, mask_layout) = match layout {
      Rav1dPixelLayout::I400 | Rav1dPixelLayout::I444 => (Rav1dPixelLayoutSubSampled::I444, 0),
      Rav1dPixelLayout::I420 => (Rav1dPixelLayoutSubSampled::I420, 2),
      Rav1dPixelLayout::I422 => (Rav1dPixelLayoutSubSampled::I422, 1),
    };
    Ok(Self {
      dimensions,
      extent,
      visible,
      context_cell,
      chroma_extent,
      chroma_cell,
      shifts,
      chroma_layout,
      mask_layout,
      has_chroma,
    })
  }
}

struct InterReconstruction<'frame, 'block> {
  frame: &'frame Rav1dFrameData,
  above: &'frame crate::src::env::BlockContext,
  tile: &'frame Rav1dTileState,
  planes: &'frame [Rav1dPictureDataComponent; 3],
  luma: PicOffset<'frame>,
  chroma_origin: Point<i32>,
  size: BlockSize,
  syntax: &'block Av1Block,
  motion: &'block Av1BlockInter,
  geometry: InterGeometry,
}

impl<'frame, 'block> InterReconstruction<'frame, 'block> {
  fn new<BD: BitDepth>(
    frame: &'frame Rav1dFrameData,
    task: &Rav1dTaskContext,
    size: BlockSize,
    syntax: &'block Av1Block,
    motion: &'block Av1BlockInter,
  ) -> Result<Self, ()> {
    let above = frame
      .above_context(task.above_index)
      .map_err(|_invalid_slot| ())?;
    let current = frame.cur.data.as_ref().ok_or(())?;
    let planes = &current.data;
    let tile = frame.tile(task.tile_index).map_err(|_invalid_tile| ())?;
    let Ok(geometry) = InterGeometry::new(frame, task.b, size);
    let [luma_component, ..] = planes;
    let luma = motion_picture_position::<BD>(
      luma_component.with_offset::<BD>(),
      Point {
        x: task.b.x.wrapping_mul(4),
        y: task.b.y.wrapping_mul(4),
      },
    )?;
    let Ok(chroma_origin) = block_plane_origin(task.b, geometry.shifts);

    Ok(Self {
      frame,
      above,
      tile,
      planes,
      luma,
      chroma_origin,
      size,
      syntax,
      motion,
      geometry,
    })
  }

  fn chroma_destination<BD: BitDepth>(
    &self,
    plane: u8,
    displacement: Point<i32>,
  ) -> Result<PicOffset<'frame>, ()> {
    let Ok(index) = console_core_number_conversion::index(plane);
    let component = self.planes.get(index).ok_or(())?;
    let position = Point {
      x: self.chroma_origin.x.wrapping_add(displacement.x),
      y: self.chroma_origin.y.wrapping_add(displacement.y),
    };
    motion_picture_position::<BD>(component.with_offset::<BD>(), position)
  }
}

pub(crate) fn rav1d_recon_b_inter<BD: BitDepth>(
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
  ts_c: Option<&mut Rav1dTileStateContext>,
  bs: BlockSize,
  b: &Av1Block,
  inter: &Av1BlockInter,
) -> Result<(), ()> {
  let view = InterReconstruction::new::<BD>(f, t, bs, b, inter)?;
  predict_inter::<BD>(&view, t)?;
  trace_inter_prediction::<BD>(&view, t)?;
  add_inter_residuals::<BD>(&view, t, ts_c)
}

fn predict_inter<BD: BitDepth>(
  view: &InterReconstruction<'_, '_>,
  t: &mut Rav1dTaskContext,
) -> Result<(), ()> {
  let header = view.frame.frame_hdr.as_ref().ok_or(())?;
  let frame_hdr = &***header;

  match frame_hdr.frame_type.is_key_or_intra().then_some(()) {
    Some(()) => predict_intrabc::<BD>(view, t, frame_hdr),
    None => match view.motion.comp_type {
      Some(blend) => predict_compound::<BD>(view, t, frame_hdr, blend),
      None => predict_single::<BD>(view, t, frame_hdr),
    },
  }
}

fn predict_intrabc<BD: BitDepth>(
  view: &InterReconstruction<'_, '_>,
  t: &mut Rav1dTaskContext,
  frame_hdr: &Rav1dFrameHeader,
) -> Result<(), ()> {
  let f = view.frame;
  let inter = view.motion;
  let y_dst = view.luma;
  let geom = &view.geometry;
  let (bw4, bh4) = (geom.extent.width, geom.extent.height);
  let ss_hor = i32::from(geom.shifts.x != 0);
  let ss_ver = i32::from(geom.shifts.y != 0);
  let has_chroma = geom.has_chroma;
  let chroma_destination =
    |plane, displacement| view.chroma_destination::<BD>(plane, displacement);

  assert!(!frame_hdr.size.super_res.enabled);
  let scratch = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
  mc::<BD>(
    f,
    &mut scratch.emu_edge,
    MaybeTempPixels::NonTemp { dst: y_dst },
    MotionPrediction {
      position: t.b,
      size: Size {
        width: bw4,
        height: bh4,
      },
      origin: Point { x: t.b.x, y: t.b.y },
      plane: 0,
      motion: match inter.nd.one_d.mv {
        [_explicit_element, ..] => _explicit_element,
      },
      reference: &f.sr_cur,
      reference_index: 0,
      filter: Filter2d::Bilinear,
    },
  )?;

  match ::core::primitive::bool::then_some(has_chroma, ()) {
    ::core::option::Option::Some(()) => {
      for pl in 1..3 {
        let motion_plane = u8::try_from(pl).map_err(|_invalid_plane| ())?;
        let destination = chroma_destination(motion_plane, Point { x: 0, y: 0 })?;
        mc::<BD>(
          f,
          &mut scratch.emu_edge,
          MaybeTempPixels::NonTemp { dst: destination },
          MotionPrediction {
            position: t.b,
            size: Size {
              width: i32::wrapping_shl(bw4, {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u32>(i32::from(bw4 == ss_hor));
                numeric_value
              }),
              height: i32::wrapping_shl(bh4, {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u32>(i32::from(bh4 == ss_ver));
                numeric_value
              }),
            },
            origin: Point {
              x: t.b.x & !ss_hor,
              y: t.b.y & !ss_ver,
            },
            plane: motion_plane,
            motion: match inter.nd.one_d.mv {
              [_explicit_element, ..] => _explicit_element,
            },
            reference: &f.sr_cur,
            reference_index: 0,
            filter: Filter2d::Bilinear,
          },
        )?;
      }
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

struct PredictionPlane {
  position: Bxy,
  plane: u8,
}

fn compound_predictions<BD: BitDepth>(
  view: &InterReconstruction<'_, '_>,
  frame_hdr: &Rav1dFrameHeader,
  edge: &mut ScratchEmuEdge,
  outputs: &mut [crate::src::internal::CompoundPrediction; 2],
  output: PredictionPlane,
) -> Result<(), ()> {
  let f = view.frame;
  let inter = view.motion;
  let geom = &view.geometry;
  let &[columns, ..] = geom.dimensions;
  let stride = u64::from(columns).wrapping_mul(4);
  let chroma = geom.chroma_extent;
  let (warp_stride, warp_size_allowed) = match output.plane {
    0 => (stride, true),
    _ => (
      stride.wrapping_shr(geom.shifts.x),
      chroma.width.min(chroma.height) > 1,
    ),
  };

  for ((dst, reference_code), vector) in
    outputs.iter_mut().zip(inter.r#ref).zip(inter.nd.one_d.mv)
  {
    let reference_index = u8::try_from(reference_code).map_err(|_invalid_reference| ())?;
    let Ok(index) = console_core_number_conversion::index(reference_index);
    let reference = f.refp.get(index).ok_or(())?;
    let warp = match (inter.inter_mode == GLOBALMV_GLOBALMV && warp_size_allowed).then_some(()) {
      Some(()) => {
        let allowed = f.gmv_warp_allowed.get(index).ok_or(())?;
        *allowed != 0
      }
      None => false,
    };

    match warp.then_some(()) {
      Some(()) => {
        let parameters = frame_hdr.gmv.get(index).ok_or(())?;
        warp_affine::<BD>(
          f,
          edge,
          MaybeTempPixels::Temp {
            tmp: dst,
            tmp_stride: warp_stride,
          },
          WarpPrediction {
            position: output.position,
            dimensions: *geom.dimensions,
            plane: output.plane,
            reference,
            parameters,
          },
        )?;
      }
      None => mc::<BD>(
        f,
        edge,
        MaybeTempPixels::Temp {
          tmp: dst,
          tmp_stride: (),
        },
        MotionPrediction {
          position: output.position,
          size: geom.extent,
          origin: Point {
            x: output.position.x,
            y: output.position.y,
          },
          plane: output.plane,
          motion: vector,
          reference,
          reference_index,
          filter: inter.filter2d,
        },
      )?,
    }
  }

  Ok(())
}

fn predict_compound<BD: BitDepth>(
  view: &InterReconstruction<'_, '_>,
  t: &mut Rav1dTaskContext,
  frame_hdr: &Rav1dFrameHeader,
  comp_inter_type: CompInterType,
) -> Result<(), ()> {
  let f = view.frame;
  let bs = view.size;
  let inter = view.motion;
  let y_dst = view.luma;
  let bd = BD::from_c(f.bitdepth_max);
  let geom = &view.geometry;
  let (bw4, bh4) = (geom.extent.width, geom.extent.height);
  let has_chroma = geom.has_chroma;
  let chroma_layout = geom.chroma_layout;
  let mask_layout = geom.mask_layout;
  let chroma_destination =
    |plane, displacement| view.chroma_destination::<BD>(plane, displacement);

  let scratch = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
  let scratch_inter = scratch
    .lap_inter
    .inter_mut()
    .map_err(|_invalid_scratch| ())?;
  let tmp = &mut scratch_inter.compinter;
  let mut jnt_weight = 0;
  let seg_mask = &mut scratch_inter.seg_mask;

  compound_predictions::<BD>(
    view,
    frame_hdr,
    &mut scratch.emu_edge,
    tmp,
    PredictionPlane {
      position: t.b,
      plane: 0,
    },
  )?;
  let mut mask: &[u8] = &[];

  match comp_inter_type {
    CompInterType::Avg => {
      let [first, second] = &*tmp;
      let predictions = [first, second];
      let extent = Size {
        width: bw4.wrapping_mul(4),
        height: bh4.wrapping_mul(4),
      };
      f.dsp
        .mc
        .avg
        .call::<BD>(y_dst, predictions, extent, bd)
        .map_err(|_| ())?;
    }
    CompInterType::WeightedAvg => {
      let [first_ref, second_ref] = inter.r#ref;
      let Ok(first_index) = console_core_number_conversion::wrapped_index(first_ref);
      let Ok(second_index) = console_core_number_conversion::wrapped_index(second_ref);
      let weights = f.jnt_weights.get(first_index).ok_or(())?;
      let weight = weights.get(second_index).ok_or(())?;
      jnt_weight = i32::from(*weight);
      let [first, second] = &*tmp;
      let predictions = [first, second];
      let extent = Size {
        width: bw4.wrapping_mul(4),
        height: bh4.wrapping_mul(4),
      };
      f.dsp
        .mc
        .w_avg
        .call::<BD>(y_dst, predictions, extent, jnt_weight, bd)
        .map_err(|_| ())?;
    }
    CompInterType::Seg => {
      use crate::src::levels::MaskPolarity::{Inverted, Normal};

      let [first, second] = &*tmp;
      let Ok(polarity) = inter.nd.one_d.mask_polarity();
      let predictions = match polarity {
        Normal => (first, second),
        Inverted => (second, first),
      };
      let checked_value_2 = f
        .dsp
        .mc
        .w_mask
        .checked(chroma_layout)
        .map_err(|_invalid_dispatch| ())?;
      checked_value_2
        .call(
          chroma_layout,
          y_dst,
          predictions,
          Size {
            width: bw4.wrapping_mul(4),
            height: bh4.wrapping_mul(4),
          },
          seg_mask,
          polarity,
          bd,
        )
        .map_err(|_invalid_mask| ())?;
      mask = &*seg_mask;
    }
    CompInterType::Wedge => {
      let lookup = WedgeMaskLookup {
        block: bs,
        layout: 0,
        polarity: crate::src::levels::MaskPolarity::Normal,
        wedge: inter.nd.one_d.wedge_idx,
      };
      let checked_value_4 = wedge_mask(lookup)?;
      mask = checked_value_4;
      let [first, second] = &*tmp;
      let Ok(wedge_polarity) = inter.nd.one_d.mask_polarity();
      let predictions = match wedge_polarity {
        crate::src::levels::MaskPolarity::Normal => [first, second],
        crate::src::levels::MaskPolarity::Inverted => [second, first],
      };
      let extent = Size {
        width: bw4.wrapping_mul(4),
        height: bh4.wrapping_mul(4),
      };
      f.dsp
        .mc
        .mask
        .call::<BD>(y_dst, predictions, extent, mask, bd)
        .map_err(|_| ())?;

      match ::core::primitive::bool::then_some(has_chroma, ()) {
        ::core::option::Option::Some(()) => {
          let chroma_lookup = WedgeMaskLookup {
            block: bs,
            layout: mask_layout,
            polarity: wedge_polarity,
            wedge: inter.nd.one_d.wedge_idx,
          };
          let checked_value_7 = wedge_mask(chroma_lookup)?;
          mask = checked_value_7;
        }
        ::core::option::Option::None => {}
      }
    }
  }

  match ::core::primitive::bool::then_some(has_chroma, ()) {
    ::core::option::Option::Some(()) => {
      for plane in [1_u8, 2] {
        compound_predictions::<BD>(
          view,
          frame_hdr,
          &mut scratch.emu_edge,
          tmp,
          PredictionPlane {
            position: t.b,
            plane,
          },
        )?;
        let uv_dst = chroma_destination(plane, Point { x: 0, y: 0 })?;

        let [first, second] = &*tmp;
        let predictions = [first, second];
        let extent = Size {
          width: bw4.wrapping_mul(4).wrapping_shr(geom.shifts.x),
          height: bh4.wrapping_mul(4).wrapping_shr(geom.shifts.y),
        };
        let masked_predictions = match inter.nd.one_d.mask_polarity() {
          Ok(crate::src::levels::MaskPolarity::Normal) => predictions,
          Ok(crate::src::levels::MaskPolarity::Inverted) => [second, first],
        };

        match comp_inter_type {
          CompInterType::Avg => {
            f.dsp
              .mc
              .avg
              .call::<BD>(uv_dst, predictions, extent, bd)
              .map_err(|_| ())?;
          }
          CompInterType::WeightedAvg => {
            f.dsp
              .mc
              .w_avg
              .call::<BD>(uv_dst, predictions, extent, jnt_weight, bd)
              .map_err(|_| ())?;
          }
          CompInterType::Seg | CompInterType::Wedge => {
            f.dsp
              .mc
              .mask
              .call::<BD>(uv_dst, masked_predictions, extent, mask, bd)
              .map_err(|_| ())?;
          }
        }
      }
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn single_warp<'a>(
  view: &InterReconstruction<'_, '_>,
  frame_hdr: &'a Rav1dFrameHeader,
  local: &'a Rav1dWarpedMotionParams,
  extent: Size<i32>,
) -> Result<Option<&'a Rav1dWarpedMotionParams>, ()> {
  match (extent.width.min(extent.height) > 1).then_some(()) {
    Some(()) => {}
    None => return Ok(None),
  }

  let inter = view.motion;
  let [reference, _] = inter.r#ref;
  let Ok(index) = console_core_number_conversion::wrapped_index(reference);
  let global_allowed = match (inter.inter_mode == GLOBALMV).then_some(()) {
    Some(()) => {
      let allowed = view.frame.gmv_warp_allowed.get(index).ok_or(())?;
      *allowed != 0
    }
    None => false,
  };
  let local_warp = inter.motion_mode == MotionMode::Warp;
  let allowed =
    global_allowed || (local_warp && local.r#type > Rav1dWarpedMotionType::Translation);

  match (allowed.then_some(()), local_warp.then_some(())) {
    (None, _) => Ok(None),
    (Some(()), Some(())) => Ok(Some(local)),
    (Some(()), None) => {
      let global = frame_hdr.gmv.get(index).ok_or(())?;
      Ok(Some(global))
    }
  }
}

fn predict_single<BD: BitDepth>(
  view: &InterReconstruction<'_, '_>,
  t: &mut Rav1dTaskContext,
  frame_hdr: &Rav1dFrameHeader,
) -> Result<(), ()> {
  let f = view.frame;
  let ts = view.tile;
  let bs = view.size;
  let inter = view.motion;
  let y_dst = view.luma;
  let bd = BD::from_c(f.bitdepth_max);
  let geom = &view.geometry;
  let b_dim = geom.dimensions;
  let (bw4, bh4) = (geom.extent.width, geom.extent.height);
  let has_chroma = geom.has_chroma;

  let [reference_code, _] = inter.r#ref;
  let reference_index = u8::try_from(reference_code).map_err(|_invalid_reference| ())?;
  let Ok(index) = console_core_number_conversion::index(reference_index);
  let refp = f.refp.get(index).ok_or(())?;
  let filter_2d = inter.filter2d;
  let warp = single_warp(view, frame_hdr, &t.warpmv, geom.extent)?;

  match warp {
    Some(parameters) => {
      let scratch = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
      warp_affine::<BD>(
        f,
        &mut scratch.emu_edge,
        MaybeTempPixels::NonTemp { dst: y_dst },
        WarpPrediction {
          position: t.b,
          dimensions: *b_dim,
          plane: 0,
          reference: refp,
          parameters,
        },
      )?;
    }
    None => {
      let [motion, _] = inter.nd.one_d.mv;
      let scratch = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
      mc::<BD>(
        f,
        &mut scratch.emu_edge,
        MaybeTempPixels::NonTemp { dst: y_dst },
        MotionPrediction {
          position: t.b,
          size: geom.extent,
          origin: Point { x: t.b.x, y: t.b.y },
          plane: 0,
          motion,
          reference: refp,
          reference_index,
          filter: filter_2d,
        },
      )?;

      match (inter.motion_mode == MotionMode::Obmc).then_some(()) {
        Some(()) => obmc::<BD>(
          f,
          t,
          y_dst,
          OverlapReconstruction {
            visible: geom.visible,
            dimensions: *b_dim,
            context: geom.context_cell,
            plane: 0,
          },
        )?,
        None => {}
      }
    }
  }

  match inter.interintra_type {
    Some(interintra_type) => {
      let interintra_edge_pal = &mut ({
        let explicit_value = t.scratch.inter_intra_mut().map_err(|_invalid_scratch| ())?;
        explicit_value
      })
      .interintra_edge_pal;
      let tl_edge_array = interintra_edge_pal
        .edge
        .buf_mut::<BD>()
        .map_err(|_invalid_scratch| ())?;
      let Ok(inter_mode) = inter.nd.one_d.interintra_mode.get();
      let Ok(mut m) = interintra_intra_mode(inter_mode);
      let mut angle = 0;
      let saved_row = PredictionEdgeRow {
        block_y: t.b.y,
        chroma_rounding: 0,
        plane: 0,
      };
      let prefiltered = prefiltered_top(f, saved_row).map_err(|_invalid_edge_row| ())?;
      let source_picture = crate::src::owned_recon::ReconDst::Pic(y_dst);
      let Ok(source_pixels) = source_picture.as_src();
      m = {
        rav1d_prepare_intra_edges(
          IntraEdgePreparation {
            position: Point { x: t.b.x, y: t.b.y },
            frame: Size {
              width: ts.tiling.col_end,
              height: ts.tiling.row_end,
            },
            transform: Size {
              width: bw4,
              height: bh4,
            },
            has_left: t.b.x > ts.tiling.col_start,
            above: {
              let Ok(above) = AboveEdge::select(
                EdgeAvailability::from(t.b.y > ts.tiling.row_start),
                prefiltered,
              );
              above
            },
            available: EdgeFlags::EMPTY,
            filter_corner: 0,
          },
          &source_pixels,
          m,
          &mut angle,
          IntraEdgeDestination {
            samples: tl_edge_array,
            center: INTER_INTRA_EDGE_CENTER,
          },
          bd,
        )
        .map_err(|_| ())?
      };
      let tmp = interintra_edge_pal
        .interintra
        .buf_mut::<BD>()
        .map_err(|_invalid_scratch| ())?;
      let tmp_component = Rav1dPictureDataComponent::wrap_buf::<BD>(
        tmp,
        (::core::num::Wrapping(4)
          * ::core::num::Wrapping({
            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4);
            numeric_value
          }))
        .0,
      );
      ({
        (*match <[_]>::get(&f.dsp.ipred.intra_pred, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(m);
          numeric_value
        }) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => return ::core::result::Result::Err(()),
        })
        .call(
          &mut crate::src::owned_recon::ReconDst::Pic(PicOffset {
            data: &tmp_component,
            offset: 0,
          }),
          tl_edge_array,
          IntraPrediction {
            mode: u64::from(m),
            edge_center: INTER_INTRA_EDGE_CENTER,
            size: Size {
              width: i32::wrapping_mul(bw4, 4),
              height: i32::wrapping_mul(bh4, 4),
            },
            reach: Size {
              width: 0,
              height: 0,
            },
            angle: 0,
          },
          bd,
        )
        .map_err(|_| ())?
      });
      #[cfg(not(feature = "c-ffi"))]
      tmp_component.copy_pixels_to::<BD>(tmp);
      let ii_mask = interintra_mask(bs, 0, inter, interintra_type)?;
      f.dsp
        .mc
        .blend
        .call::<BD>(
          y_dst,
          tmp,
          Size {
            width: i32::wrapping_mul(bw4, 4),
            height: i32::wrapping_mul(bh4, 4),
          },
          ii_mask,
        )
        .map_err(|_| ())?;
    }
    None => {}
  }

  match ::core::primitive::bool::then_some(has_chroma, ()) {
    ::core::option::Option::Some(()) => {
      let extent = Size {
        width: bw4,
        height: bh4,
      };

      match {
        let checked_value_28 = f
          .chroma_motion_neighbors(t, extent)
          .map_err(|_invalid_neighbor| ())?;
        checked_value_28
      } {
        Some(neighbors) => predict_neighbor_chroma::<BD>(view, t, neighbors)?,
        None => predict_chroma_reference::<BD>(view, t, frame_hdr, refp)?,
      }
    }
    ::core::option::Option::None => {}
  }

  t.tl_4x4_filter = filter_2d;

  Ok(())
}

fn predict_chroma_reference<BD: BitDepth>(
  view: &InterReconstruction<'_, '_>,
  t: &mut Rav1dTaskContext,
  frame_hdr: &Rav1dFrameHeader,
  refp: &Rav1dThreadPicture,
) -> Result<(), ()> {
  let geom = &view.geometry;
  let f = view.frame;
  let ts = view.tile;
  let bs = view.size;
  let inter = view.motion;
  let bd = BD::from_c(f.bitdepth_max);
  let b_dim = geom.dimensions;
  let (bw4, bh4) = (geom.extent.width, geom.extent.height);
  let (cbw4, cbh4) = (geom.chroma_extent.width, geom.chroma_extent.height);
  let ss_hor = i32::from(geom.shifts.x != 0);
  let ss_ver = i32::from(geom.shifts.y != 0);
  let mask_layout = geom.mask_layout;
  let filter_2d = inter.filter2d;
  let chroma_destination =
    |plane, displacement| view.chroma_destination::<BD>(plane, displacement);

  let warp = single_warp(view, frame_hdr, &t.warpmv, geom.chroma_extent)?;

  match warp {
    Some(parameters) => {
      let scratch = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;

      for plane in [1_u8, 2] {
        let destination = chroma_destination(plane, Point { x: 0, y: 0 })?;
        warp_affine::<BD>(
          f,
          &mut scratch.emu_edge,
          MaybeTempPixels::NonTemp { dst: destination },
          WarpPrediction {
            position: t.b,
            dimensions: *b_dim,
            plane,
            reference: refp,
            parameters,
          },
        )?;
      }
    }
    None => {
      let [reference_code, _] = inter.r#ref;
      let reference_index = u8::try_from(reference_code).map_err(|_invalid_reference| ())?;
      let [motion, _] = inter.nd.one_d.mv;
      let size = Size {
        width: bw4.wrapping_shl(u32::from(bw4 == ss_hor)),
        height: bh4.wrapping_shl(u32::from(bh4 == ss_ver)),
      };

      for plane in [1_u8, 2] {
        let destination = chroma_destination(plane, Point { x: 0, y: 0 })?;
        let scratch = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
        mc::<BD>(
          f,
          &mut scratch.emu_edge,
          MaybeTempPixels::NonTemp { dst: destination },
          MotionPrediction {
            position: t.b,
            size,
            origin: Point {
              x: t.b.x & !ss_hor,
              y: t.b.y & !ss_ver,
            },
            plane,
            motion,
            reference: refp,
            reference_index,
            filter: filter_2d,
          },
        )?;

        match (inter.motion_mode == MotionMode::Obmc).then_some(()) {
          Some(()) => obmc::<BD>(
            f,
            t,
            destination,
            OverlapReconstruction {
              visible: geom.visible,
              dimensions: *b_dim,
              context: geom.context_cell,
              plane,
            },
          )?,
          None => {}
        }
      }
    }
  }

  match inter.interintra_type {
    Some(interintra_type) => {
      let ii_mask = interintra_mask(bs, mask_layout, inter, interintra_type)?;

      for pl in 0..2 {
        let interintra_edge_pal = &mut ({
          let explicit_value = t.scratch.inter_intra_mut().map_err(|_invalid_scratch| ())?;
          explicit_value
        })
        .interintra_edge_pal;
        let tl_edge_array = interintra_edge_pal
          .edge
          .buf_mut::<BD>()
          .map_err(|_invalid_scratch| ())?;
        let Ok(inter_mode) = inter.nd.one_d.interintra_mode.get();
        let Ok(mode) = interintra_intra_mode(inter_mode);
        let mut angle = 0;
        let destination_plane = u8::try_from(pl).map_err(|_invalid_plane| ())?;
        let uv_dst = chroma_destination(destination_plane.wrapping_add(1), Point { x: 0, y: 0 })?;
        let chroma_plane = u8::try_from(pl).map_err(|_invalid_plane| ())?;
        let saved_row = PredictionEdgeRow {
          block_y: t.b.y,
          chroma_rounding: 0,
          plane: chroma_plane.wrapping_add(1),
        };
        let prefiltered = prefiltered_top(f, saved_row).map_err(|_invalid_edge_row| ())?;
        let source_picture = crate::src::owned_recon::ReconDst::Pic(uv_dst);
        let Ok(source_pixels) = source_picture.as_src();
        let shifts = geom.shifts;
        let position = Point {
          x: t.b.x.wrapping_shr(shifts.x),
          y: t.b.y.wrapping_shr(shifts.y),
        };
        let room = Size {
          width: ts.tiling.col_end.wrapping_shr(shifts.x),
          height: ts.tiling.row_end.wrapping_shr(shifts.y),
        };
        let tile_origin = Point {
          x: ts.tiling.col_start.wrapping_shr(shifts.x),
          y: ts.tiling.row_start.wrapping_shr(shifts.y),
        };
        let Ok(above) = AboveEdge::select(
          EdgeAvailability::from(position.y > tile_origin.y),
          prefiltered,
        );
        let m = rav1d_prepare_intra_edges(
          IntraEdgePreparation {
            position,
            frame: room,
            transform: geom.chroma_extent,
            has_left: position.x > tile_origin.x,
            above,
            available: EdgeFlags::EMPTY,
            filter_corner: 0,
          },
          &source_pixels,
          mode,
          &mut angle,
          IntraEdgeDestination {
            samples: tl_edge_array,
            center: INTER_INTRA_EDGE_CENTER,
          },
          bd,
        )
        .map_err(|_invalid_edge| ())?;
        let tmp = interintra_edge_pal
          .interintra
          .buf_mut::<BD>()
          .map_err(|_invalid_scratch| ())?;
        let tmp_component = Rav1dPictureDataComponent::wrap_buf::<BD>(
          tmp,
          (::core::num::Wrapping(4)
            * ::core::num::Wrapping({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbw4);
              numeric_value
            }))
          .0,
        );
        let Ok(mode_index) = console_core_number_conversion::wrapped_index(m);
        let predictor = f.dsp.ipred.intra_pred.get(mode_index).ok_or(())?;
        predictor
          .call(
            &mut crate::src::owned_recon::ReconDst::Pic(PicOffset {
              data: &tmp_component,
              offset: 0,
            }),
            tl_edge_array,
            IntraPrediction {
              mode: u64::from(m),
              edge_center: INTER_INTRA_EDGE_CENTER,
              size: Size {
                width: cbw4.wrapping_mul(4),
                height: cbh4.wrapping_mul(4),
              },
              reach: Size {
                width: 0,
                height: 0,
              },
              angle: 0,
            },
            bd,
          )
          .map_err(|_invalid_prediction| ())?;
        #[cfg(not(feature = "c-ffi"))]
        tmp_component.copy_pixels_to::<BD>(tmp);
        f.dsp
          .mc
          .blend
          .call::<BD>(
            uv_dst,
            tmp,
            Size {
              width: i32::wrapping_mul(cbw4, 4),
              height: i32::wrapping_mul(cbh4, 4),
            },
            ii_mask,
          )
          .map_err(|_| ())?;
      }
    }
    None => {}
  }

  Ok(())
}

fn chroma_filter([horizontal, vertical]: [Rav1dFilterMode; 2]) -> Result<Filter2d, ()> {
  let Ok(column) = console_core_number_conversion::index(u8::from(horizontal));
  let Ok(row) = console_core_number_conversion::index(u8::from(vertical));
  dav1d_filter_2d
    .get(row)
    .and_then(|filters| filters.get(column))
    .copied()
    .ok_or(())
}

fn draw_chroma_neighbor<BD: BitDepth>(
  view: &InterReconstruction<'_, '_>,
  task: &mut Rav1dTaskContext,
  prediction: ChromaNeighborPrediction,
  displacement: Point<i32>,
  local_filter: Filter2d,
) -> Result<(), ()> {
  let f = view.frame;
  let Ok(reference) = console_core_number_conversion::index(prediction.reference);
  let picture = f.refp.get(reference).ok_or(())?;
  let filter = match task.frame_thread.pass {
    2 => {
      let address = f
        .coded_block_position(prediction.position)
        .map_err(|_invalid_neighbor| ())?;
      let Ok(index) = console_core_number_conversion::index(address);
      let Ok(saved_filter) = f.frame_thread.b.index(index).ii.filter2d();
      saved_filter
    }
    _ => local_filter,
  };
  let position = task.b;
  let scratch = task.scratch.inter_mut().map_err(|_invalid_scratch| ())?;

  for plane in [1_u8, 2] {
    let dst = view.chroma_destination::<BD>(plane, displacement)?;
    mc::<BD>(
      f,
      &mut scratch.emu_edge,
      MaybeTempPixels::NonTemp { dst },
      MotionPrediction {
        position,
        size: view.geometry.extent,
        origin: Point {
          x: prediction.position.x,
          y: prediction.position.y,
        },
        plane,
        motion: prediction.vector,
        reference: picture,
        reference_index: prediction.reference,
        filter,
      },
    )?;
  }

  Ok(())
}

fn predict_neighbor_chroma<BD: BitDepth>(
  view: &InterReconstruction<'_, '_>,
  t: &mut Rav1dTaskContext,
  neighbors: crate::src::internal::ChromaMotionNeighbors,
) -> Result<(), ()> {
  let x_offset = i32::from(neighbors.left.is_some()).wrapping_mul(2);
  let y_offset = i32::from(neighbors.above.is_some()).wrapping_mul(2);

  match neighbors.diagonal {
    Some(diagonal) => {
      let filter = t.tl_4x4_filter;
      draw_chroma_neighbor::<BD>(view, t, diagonal, Point { x: 0, y: 0 }, filter)?;
    }
    None => {}
  }

  match neighbors.left {
    Some(left) => {
      let row = view.geometry.context_cell.y;
      let index =
        console_core_number_conversion::checked_index(row).map_err(|_invalid_row| ())?;
      let [horizontal, vertical] = &mut t.l.filter;
      let x = horizontal.get_mut().get(index).ok_or(())?;
      let y = vertical.get_mut().get(index).ok_or(())?;
      let filter = chroma_filter([*x, *y])?;
      draw_chroma_neighbor::<BD>(view, t, left, Point { x: 0, y: y_offset }, filter)?;
    }
    None => {}
  }

  match neighbors.above {
    Some(above) => {
      let column = view.geometry.context_cell.x;
      let index =
        console_core_number_conversion::checked_index(column).map_err(|_invalid_column| ())?;
      let [horizontal, vertical] = &view.above.filter;
      let filter = chroma_filter([*horizontal.index(index), *vertical.index(index)])?;
      draw_chroma_neighbor::<BD>(view, t, above, Point { x: x_offset, y: 0 }, filter)?;
    }
    None => {}
  }

  let [reference_code, _] = view.motion.r#ref;
  let reference_index = u8::try_from(reference_code).map_err(|_invalid_reference| ())?;
  let Ok(native_reference) = console_core_number_conversion::index(reference_index);
  let picture = view.frame.refp.get(native_reference).ok_or(())?;
  let [vector, _] = view.motion.nd.one_d.mv;
  let scratch = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
  let displacement = Point {
    x: x_offset,
    y: y_offset,
  };

  for plane in [1_u8, 2] {
    let dst = view.chroma_destination::<BD>(plane, displacement)?;
    mc::<BD>(
      view.frame,
      &mut scratch.emu_edge,
      MaybeTempPixels::NonTemp { dst },
      MotionPrediction {
        position: t.b,
        size: view.geometry.extent,
        origin: Point { x: t.b.x, y: t.b.y },
        plane,
        motion: vector,
        reference: picture,
        reference_index,
        filter: view.motion.filter2d,
      },
    )?;
  }

  Ok(())
}

fn trace_inter_prediction<BD: BitDepth>(
  view: &InterReconstruction<'_, '_>,
  t: &Rav1dTaskContext,
) -> Result<(), ()> {
  use crate::src::owned_recon::ReconDst;

  let Ok(trace) = debug_block_info(view.frame, t.b);

  match (trace, PIXEL_TRACE) {
    (BlockTrace::Enabled, BlockTrace::Enabled) => {}
    (BlockTrace::Disabled, _) | (_, BlockTrace::Disabled) => return Ok(()),
  }

  let &[width, height, ..] = view.geometry.dimensions;
  let luma_size = Size {
    width: u64::from(width).wrapping_mul(4),
    height: u64::from(height).wrapping_mul(4),
  };
  hex_dump_pic::<BD>(&ReconDst::Pic(view.luma), luma_size, "y-pred").map_err(|_dump_error| ())?;

  match view.geometry.has_chroma.then_some(()) {
    Some(()) => {
      for (plane, label) in [(1_u8, "u-pred"), (2, "v-pred")] {
        let dst = view.chroma_destination::<BD>(plane, Point { x: 0, y: 0 })?;
        let chroma_width =
          u64::try_from(view.geometry.chroma_extent.width).map_err(|_negative_extent| ())?;
        let chroma_height =
          u64::try_from(view.geometry.chroma_extent.height).map_err(|_negative_extent| ())?;
        let chroma_size = Size {
          width: chroma_width.wrapping_mul(4),
          height: chroma_height.wrapping_mul(4),
        };
        hex_dump_pic::<BD>(&ReconDst::Pic(dst), chroma_size, label).map_err(|_dump_error| ())?;
      }
    }
    None => {}
  }

  Ok(())
}

fn add_inter_residuals<BD: BitDepth>(
  view: &InterReconstruction<'_, '_>,
  t: &mut Rav1dTaskContext,
  mut ts_c: Option<&mut Rav1dTileStateContext>,
) -> Result<(), ()> {
  let above_context = view.above;
  let inter = view.motion;
  let b = view.syntax;
  let geom = &view.geometry;
  let (bw4, bh4) = (geom.extent.width, geom.extent.height);
  let (bx4, by4) = (geom.context_cell.x, geom.context_cell.y);
  let (cbw4, cbh4) = (geom.chroma_extent.width, geom.chroma_extent.height);
  let (cbx4, cby4) = (geom.chroma_cell.x, geom.chroma_cell.y);
  let has_chroma = geom.has_chroma;

  match ::core::primitive::bool::then_some(b.skip != 0, ()) {
    ::core::option::Option::Some(()) => {
      case_set_al! {
          <32, false>
          l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4); numeric_value }),
          a: (above_context, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx4); numeric_value }),
          lcoef = (0x40, 0x40),
      }

      match ::core::primitive::bool::then_some(has_chroma, ()) {
        ::core::option::Option::Some(()) => {
          case_set_al! {
              <32, false>
              l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbh4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cby4); numeric_value }),
              a: (above_context, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbw4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbx4); numeric_value }),
              ccoef[0] = (0x40, 0x40),
              ccoef[1] = (0x40, 0x40),
          }
        }
        ::core::option::Option::None => {}
      }

      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  let uvtx = coefficient_dimensions(b.uvtx).map_err(|_invalid_transform| ())?;
  let ytx = coefficient_dimensions(inter.max_ytx).map_err(|_invalid_transform| ())?;
  let Ok(split0) = console_core_number_conversion::wrapped::<_, u16>(inter.tx_split0);
  let splits = [split0, inter.tx_split1];
  let luma_steps = transform_steps(ytx).map_err(|_empty_transform| ())?;
  let chroma_steps = transform_steps(uvtx).map_err(|_empty_transform| ())?;
  let luma = InterLumaTransform {
    dimensions: ytx,
    steps: luma_steps,
    splits,
  };
  let chroma = InterChromaTransform {
    dimensions: uvtx,
    steps: chroma_steps,
  };

  for y in (0..bh4).step_by(16) {
    for x in (0..bw4).step_by(16) {
      let chunk = Point { x, y };
      add_inter_luma_chunk::<BD>(view, t, ts_c.as_deref_mut(), &luma, chunk)?;

      match has_chroma.then_some(()) {
        Some(()) => add_inter_chroma_chunk::<BD>(view, t, ts_c.as_deref_mut(), &chroma, chunk)?,
        None => {}
      }
    }
  }

  Ok(())
}

struct InterLumaTransform {
  dimensions: &'static TxfmInfo,
  steps: Size<core::num::NonZeroU8>,
  splits: [u16; 2],
}

fn add_inter_luma_chunk<BD: BitDepth>(
  view: &InterReconstruction<'_, '_>,
  t: &mut Rav1dTaskContext,
  mut ts_c: Option<&mut Rav1dTileStateContext>,
  layout: &InterLumaTransform,
  chunk: Point<i32>,
) -> Result<(), ()> {
  let Ok(pixel_stride) = view.luma.fixed_pixel_stride::<BD>();
  let row_pitch = pixel_stride.wrapping_mul(4);
  let width = i32::from(layout.dimensions.w);
  let height = i32::from(layout.dimensions.h);
  let Ok(start) =
    console_core_number_conversion::wrapped_offset(row_pitch.wrapping_mul(i64::from(chunk.y)));
  let Ok(row_step) =
    console_core_number_conversion::wrapped_offset(row_pitch.wrapping_mul(i64::from(height)));
  let mut dst = view.luma + start;
  let mut split_y = i32::from(chunk.y != 0);
  let mut y = chunk.y;
  t.b.y = t.b.y.wrapping_add(chunk.y);

  let bottom = view.geometry.visible.height.min(chunk.y.wrapping_add(16));
  let right = view.geometry.visible.width.min(chunk.x.wrapping_add(16));

  for _ in (y..bottom).step_by(core::num::NonZero::from(layout.steps.height).get()) {
    let mut x = chunk.x;
    let mut split_x = i32::from(chunk.x != 0);
    t.b.x = t.b.x.wrapping_add(chunk.x);

    for _ in (x..right).step_by(core::num::NonZero::from(layout.steps.width).get()) {
      let Ok(offset) = console_core_number_conversion::wrapped_index(x.wrapping_mul(4));
      read_coef_tree::<BD>(
        view.frame,
        t,
        ts_c.as_deref_mut(),
        LumaCoefficientTree {
          size: view.size,
          syntax: view.syntax,
          transform: view.motion.max_ytx,
          splits: layout.splits,
          split_cell: Point {
            x: split_x,
            y: split_y,
          },
          output: Some(dst + offset),
        },
      )
      .map_err(|_coefficient_fault| ())?;
      t.b.x = t.b.x.wrapping_add(width);
      x = x.wrapping_add(width);
      split_x = split_x.wrapping_add(1);
    }

    dst += row_step;
    t.b.x = t.b.x.wrapping_sub(x);
    t.b.y = t.b.y.wrapping_add(height);
    y = y.wrapping_add(height);
    split_y = split_y.wrapping_add(1);
  }

  t.b.y = t.b.y.wrapping_sub(y);
  Ok(())
}

struct InterChromaTransform {
  dimensions: &'static TxfmInfo,
  steps: Size<core::num::NonZeroU8>,
}

fn add_inter_chroma_chunk<BD: BitDepth>(
  view: &InterReconstruction<'_, '_>,
  t: &mut Rav1dTaskContext,
  mut ts_c: Option<&mut Rav1dTileStateContext>,
  layout: &InterChromaTransform,
  chunk: Point<i32>,
) -> Result<(), ()> {
  let geom = &view.geometry;
  let shifts = geom.shifts;
  let ss_hor = i32::from(shifts.x != 0);
  let ss_ver = i32::from(shifts.y != 0);
  let cw4 = geom
    .visible
    .width
    .wrapping_add(ss_hor)
    .wrapping_shr(shifts.x);
  let ch4 = geom
    .visible
    .height
    .wrapping_add(ss_ver)
    .wrapping_shr(shifts.y);
  let right = cw4.min(chunk.x.wrapping_add(16).wrapping_shr(shifts.x));
  let bottom = ch4.min(chunk.y.wrapping_add(16).wrapping_shr(shifts.y));
  let x_start = chunk.x.wrapping_shr(shifts.x);
  let y_start = chunk.y.wrapping_shr(shifts.y);
  let width = i32::from(layout.dimensions.w);
  let height = i32::from(layout.dimensions.h);

  for plane in [0_u8, 1] {
    let row = chunk.y.wrapping_mul(4).wrapping_shr(shifts.y);
    let mut uv_dst =
      view.chroma_destination::<BD>(plane.wrapping_add(1), Point { x: 0, y: row })?;
    let Ok(pixel_stride) = uv_dst.fixed_pixel_stride::<BD>();
    let Ok(row_step) = console_core_number_conversion::wrapped_offset(
      pixel_stride.wrapping_mul(4).wrapping_mul(i64::from(height)),
    );
    let mut y = y_start;
    t.b.y = t.b.y.wrapping_add(chunk.y);

    for _ in (y..bottom).step_by(core::num::NonZero::from(layout.steps.height).get()) {
      let mut x = x_start;
      t.b.x = t.b.x.wrapping_add(chunk.x);

      for _ in (x..right).step_by(core::num::NonZero::from(layout.steps.width).get()) {
        let cell = InterChromaCell {
          plane,
          position: Point { x, y },
          destination: uv_dst,
        };
        add_inter_chroma_cell::<BD>(view, t, ts_c.as_deref_mut(), layout.dimensions, cell)?;
        t.b.x = t.b.x.wrapping_add(width.wrapping_shl(shifts.x));
        x = x.wrapping_add(width);
      }

      uv_dst += row_step;
      t.b.x = t.b.x.wrapping_sub(x.wrapping_shl(shifts.x));
      t.b.y = t.b.y.wrapping_add(height.wrapping_shl(shifts.y));
      y = y.wrapping_add(height);
    }

    t.b.y = t.b.y.wrapping_sub(y.wrapping_shl(shifts.y));
  }

  Ok(())
}

struct InterChromaCell<'a> {
  plane: u8,
  position: Point<i32>,
  destination: PicOffset<'a>,
}

fn add_inter_chroma_cell<BD: BitDepth>(
  view: &InterReconstruction<'_, '_>,
  t: &mut Rav1dTaskContext,
  mut ts_c: Option<&mut Rav1dTileStateContext>,
  uvtx: &TxfmInfo,
  cell: InterChromaCell<'_>,
) -> Result<(), ()> {
  let f = view.frame;
  let ts = view.tile;
  let above_context = view.above;
  let b = view.syntax;
  let bs = view.size;
  let bd = BD::from_c(f.bitdepth_max);
  let geom = &view.geometry;
  let (bx4, by4) = (geom.context_cell.x, geom.context_cell.y);
  let (cbx4, cby4) = (geom.chroma_cell.x, geom.chroma_cell.y);
  let ss_hor = i32::from(geom.shifts.x != 0);
  let ss_ver = i32::from(geom.shifts.y != 0);
  let Ok(pl) = console_core_number_conversion::index(cell.plane);
  let Point { x, y } = cell.position;
  let uv_dst = cell.destination;

  let mut cf_guard;
  let cf;
  let eob;
  let mut txtp;

  match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(pass) = console_core_number_conversion::index(t.frame_thread.pass & 1);
      let sequence = ts.frame_thread.get(pass).ok_or(())?;
      let samples = u32::from(uvtx.h)
        .wrapping_mul(4)
        .wrapping_mul(u32::from(uvtx.w))
        .wrapping_mul(4);
      let Ok(offset) = sequence.cf.get_update(|i| i.wrapping_add(samples));
      let Ok(first) = console_core_number_conversion::wrapped_index(offset);
      let Ok(last) = console_core_number_conversion::wrapped_index(samples);
      cf_guard = f.frame_thread.cf.mut_slice_as((first.., ..last));
      cf = &mut *cf_guard;
      let Ok(cbi_offset) = sequence.cbi_idx.get_update(|i| i.wrapping_add(1));
      let Ok(cbi_index) = console_core_number_conversion::wrapped_index(cbi_offset);
      let cbi_record = f.frame_thread.cbi.get(cbi_index).ok_or(())?;
      let Ok(cbi) = cbi_record.get();
      let Ok(raw_eob) = cbi.eob();
      eob = i32::from(raw_eob);
      let Ok(raw_type) = cbi.txtp();
      txtp = raw_type;
    }
    ::core::option::Option::None => {
      let mut cf_ctx = 0;
      let map_row = by4.wrapping_add(y.wrapping_shl(geom.shifts.y));
      let map_column = bx4.wrapping_add(x.wrapping_shl(geom.shifts.x));
      let Ok(map_index) = console_core_number_conversion::wrapped_index(
        map_row.wrapping_mul(32).wrapping_add(map_column),
      );
      let scratch = t.scratch.inter_intra().map_err(|_invalid_scratch| ())?;
      let map = scratch
        .ac_txtp_map
        .txtp_map()
        .map_err(|_invalid_scratch| ())?;
      let mapped_type = map.get(map_index).ok_or(())?;
      txtp = *mapped_type;

      let a_ccoef = above_context.ccoef.get(pl).ok_or(())?;
      let Ok(a_start) = console_core_number_conversion::wrapped_index(cbx4.wrapping_add(x));
      let l_ccoef = t.l.ccoef.get_mut(pl).ok_or(())?;
      let Ok(l_start) = console_core_number_conversion::wrapped_index(cby4.wrapping_add(y));
      let Ok(width) = console_core_number_conversion::index(uvtx.w);
      let Ok(height) = console_core_number_conversion::index(uvtx.h);
      let left_row = l_ccoef.get_mut().get(l_start..).ok_or(())?;
      let left = left_row.get(..height).ok_or(())?;
      let Ok(trace) = debug_block_info(f, t.b);
      eob = {
        let state = ts_c.as_deref_mut().ok_or(())?;

        decode_coefs::<BD>(
          f,
          CoefficientDecoder {
            tile: ts,
            state,
            scratch: &mut t.scratch,
            coefficients: &mut t.cf,
          },
          CoefficientBlock {
            trace,
            above: &a_ccoef.index((a_start.., ..width)),
            left,
            transform: b.uvtx,
            size: bs,
            syntax: b,
            plane: cell.plane.wrapping_add(1),
            storage: CfSelect::Task,
          },
          CoefficientOutput {
            transform: &mut txtp,
            residual_context: &mut cf_ctx,
          },
        )
        .map_err(|_| ())?
      };
      cf = {
        let explicit_value = t.cf.select_mut::<BD>().map_err(|_invalid_scratch| ())?;
        explicit_value
      };

      match {
        let Ok(active_trace) = debug_block_info(f, t.b);
        active_trace
      } {
        BlockTrace::Enabled => {
          let entropy = ts_c.as_deref().ok_or(())?;
          console_core_logging::trace!(
            "Post-uv-cf-blk[pl={},tx={:?},txtp={},eob={}]: r={}",
            pl,
            b.uvtx,
            txtp,
            eob,
            entropy.msac.rng,
          );
        }
        BlockTrace::Disabled => {}
      }

      case_set_al! {
          <16, true>
          l: (
              &mut t.l,
              { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cmp::min({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(uvtx.h); numeric_value }, i32::wrapping_shr(i32::wrapping_add(i32::wrapping_sub(f.bh, t.b.y), ss_ver), { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver); numeric_value }))); numeric_value },
              { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(i32::wrapping_add(cby4, y)); numeric_value }
          ),
          a: (
              above_context,
              { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cmp::min({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(uvtx.w); numeric_value }, i32::wrapping_shr(i32::wrapping_add(i32::wrapping_sub(f.bw, t.b.x), ss_hor), { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor); numeric_value }))); numeric_value },
              { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(i32::wrapping_add(cbx4, x)); numeric_value }
          ),
          ccoef[pl] = (cf_ctx, cf_ctx),
      }
    }
  }

  match ::core::primitive::bool::then_some(eob >= 0, ()) {
    ::core::option::Option::Some(()) => {
      match (
        {
          let Ok(active_trace) = debug_block_info(f, t.b);
          active_trace
        },
        PIXEL_TRACE,
      ) {
        (BlockTrace::Enabled, BlockTrace::Enabled) => {
          coef_dump(
            cf,
            Size {
              width: u64::from(uvtx.h).wrapping_mul(4),
              height: u64::from(uvtx.w).wrapping_mul(4),
            },
            3,
            "dq",
          )
          .map_err(|_dump_error| ())?;
        }
        (BlockTrace::Disabled, _) | (_, BlockTrace::Disabled) => {}
      }

      let mut uv_dst_tx = crate::src::owned_recon::ReconDst::Pic(
        uv_dst
          + (::core::num::Wrapping(4)
            * ::core::num::Wrapping({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(x);
              numeric_value
            }))
          .0,
      );
      f.dsp
        .itx
        .add::<BD>(b.uvtx, txtp, &mut uv_dst_tx, cf, eob, bd)
        .map_err(|_| ())?;

      match (
        {
          let Ok(active_trace) = debug_block_info(f, t.b);
          active_trace
        },
        PIXEL_TRACE,
      ) {
        (BlockTrace::Enabled, BlockTrace::Enabled) => {
          hex_dump_pic::<BD>(
            &uv_dst_tx,
            Size {
              width: u64::from(uvtx.w).wrapping_mul(4),
              height: u64::from(uvtx.h).wrapping_mul(4),
            },
            "recon",
          )
          .map_err(|_dump_error| ())?;
        }
        (BlockTrace::Disabled, _) | (_, BlockTrace::Disabled) => {}
      }
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

pub(crate) fn rav1d_filter_sbrow_deblock_cols<BD: BitDepth>(
  c: &Rav1dContext,
  f: &Rav1dFrameData,
  _t: &mut Rav1dTaskContext,
  sby: c_int,
) -> Rav1dResult {
  match {
    ::core::primitive::bool::then_some(
      !c.inloop_filters.contains(Rav1dInloopFilterType::DEBLOCK),
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  let frame_hdr = f.frame_hdr.as_ref().ok_or(EINVAL)?;

  match { ::core::primitive::bool::then_some(frame_hdr.loopfilter.level_y == [0; 2], ()) } {
    ::core::option::Option::Some(()) => {
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  let row = u32::try_from(sby).map_err(|_invalid_row| EINVAL)?;
  let Ok(row_index) = console_core_number_conversion::index(row);
  let &tile_row = f.lf.start_of_tile_row.get(row_index).ok_or(EINVAL)?;
  let y = sby.wrapping_mul(f.sb_step).wrapping_mul(4);
  let pixels = f.cur.lf_offsets::<BD>(y);
  rav1d_loopfilter_sbrow_cols::<BD>(f, pixels, row, tile_row)
}

pub(crate) fn rav1d_filter_sbrow_deblock_rows<BD: BitDepth>(
  c: &Rav1dContext,
  f: &Rav1dFrameData,
  _t: &mut Rav1dTaskContext,
  sby: c_int,
) -> Rav1dResult {
  let y = sby.wrapping_mul(f.sb_step).wrapping_mul(4);
  let pictures = f.cur.lf_offsets::<BD>(y);
  let seq_hdr = f.seq_hdr.as_ref().ok_or(EINVAL)?;
  let frame_hdr = f.frame_hdr.as_ref().ok_or(EINVAL)?;

  match {
    ::core::primitive::bool::then_some(
      c.inloop_filters.contains(Rav1dInloopFilterType::DEBLOCK)
        && frame_hdr.loopfilter.level_y != [0; 2],
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      let row = u32::try_from(sby).map_err(|_invalid_row| EINVAL)?;
      rav1d_loopfilter_sbrow_rows::<BD>(f, pictures, row)?;
    }
    ::core::option::Option::None => {}
  }

  match {
    ::core::primitive::bool::then_some(seq_hdr.cdef != 0 || !f.lf.restore_planes.is_empty(), ())
  } {
    ::core::option::Option::Some(()) => {
      rav1d_copy_lpf::<BD>(c, f, pictures, sby)?;
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

pub(crate) fn rav1d_filter_sbrow_cdef<BD: BitDepth>(
  c: &Rav1dContext,
  f: &Rav1dFrameData,
  tc: &mut Rav1dTaskContext,
  sby: c_int,
) -> Rav1dResult {
  {
    match ::core::primitive::bool::then_some(
      !c.inloop_filters.contains(Rav1dInloopFilterType::CDEF),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let sequence_header = f.seq_hdr.as_ref().ok_or(EINVAL)?;
    let seq_hdr = &***sequence_header;
    let _frame_header = f.frame_hdr.as_ref().ok_or(EINVAL)?;
    let sbsz = f.sb_step;
    let y = i32::wrapping_mul(i32::wrapping_mul(sby, sbsz), 4);
    let p = f.cur.lf_offsets::<BD>(y);
    let prev_mask = i32::wrapping_mul(
      i32::wrapping_shr(i32::wrapping_sub(sby, 1), {
        let Ok(numeric_value) =
          console_core_number_conversion::wrapped::<_, u32>(i32::from(seq_hdr.sb128 == 0));
        numeric_value
      }),
      f.sb128w,
    );
    let mask_offset = i32::wrapping_mul(
      i32::wrapping_shr(sby, {
        let Ok(numeric_value) =
          console_core_number_conversion::wrapped::<_, u32>(i32::from(seq_hdr.sb128 == 0));
        numeric_value
      }),
      f.sb128w,
    );
    let start = i32::wrapping_mul(sby, sbsz);

    match ::core::primitive::bool::then_some(sby != 0, ()) {
      ::core::option::Option::Some(()) => {
        let [luma, chroma_u, chroma_v] = p;
        let chroma_shift = u32::from(f.cur.p.layout == Rav1dPixelLayout::I420);
        let p_up = [
          (luma, 0),
          (chroma_u, chroma_shift),
          (chroma_v, chroma_shift),
        ]
        .map(|(plane, shift)| {
          let Ok(pitch) = plane.fixed_pixel_stride::<BD>();
          let row = pitch.wrapping_mul(8).wrapping_shr(shift);
          let Ok(offset) = console_core_number_conversion::wrapped_offset(row);
          plane - offset
        });
        rav1d_cdef_brow::<BD>(
          c,
          tc,
          f,
          p_up,
          CdefRows {
            mask_offset: prev_mask,
            rows: start.wrapping_sub(2)..start,
            span: CdefRowSpan::AboveBoundary,
            superblock_row: sby,
          },
        )?;
      }
      ::core::option::Option::None => {}
    }

    let n_blks = i32::wrapping_sub(
      sbsz,
      i32::wrapping_mul(2, i32::from(i32::wrapping_add(sby, 1) < f.sbh)),
    );
    let end = cmp::min(i32::wrapping_add(start, n_blks), f.bh);
    rav1d_cdef_brow::<BD>(
      c,
      tc,
      f,
      p,
      CdefRows {
        mask_offset,
        rows: start..end,
        span: CdefRowSpan::Body,
        superblock_row: sby,
      },
    )?;
  };

  Ok(())
}

pub(crate) fn rav1d_filter_sbrow_resize<BD: BitDepth>(
  _c: &Rav1dContext,
  f: &Rav1dFrameData,
  _t: &mut Rav1dTaskContext,
  sby: c_int,
) -> Rav1dResult {
  let depth = BD::from_c(f.bitdepth_max);
  let row = sby.wrapping_mul(f.sb_step).wrapping_mul(4);
  let source = f.cur.lf_offsets::<BD>(row);
  let resized = f.sr_cur.p.lf_offsets::<BD>(row);
  let layout = f.cur.p.layout;
  let planes = match layout {
    Rav1dPixelLayout::I400 => 1_u8,
    Rav1dPixelLayout::I420 | Rav1dPixelLayout::I422 | Rav1dPixelLayout::I444 => 3,
  };
  let [luma_step, chroma_step] = f.resize_step;
  let [luma_start, chroma_start] = f.resize_start;
  let phases = [
    ResizePhase {
      step: luma_step,
      initial: luma_start,
    },
    ResizePhase {
      step: chroma_step,
      initial: chroma_start,
    },
    ResizePhase {
      step: chroma_step,
      initial: chroma_start,
    },
  ];
  let margin = 8_i32.wrapping_mul(i32::from(sby != 0));
  let body = f
    .sb_step
    .wrapping_sub(i32::from(sby.wrapping_add(1) < f.sbh).wrapping_mul(2))
    .wrapping_mul(4);
  let remaining = f.cur.p.h.wrapping_sub(row);

  for (plane, ((dst, src), phase)) in
    (0_u8..planes).zip(resized.into_iter().zip(source).zip(phases))
  {
    let vertical = u32::from(plane != 0 && layout == Rav1dPixelLayout::I420);
    let horizontal = u32::from(plane != 0 && layout != Rav1dPixelLayout::I444);
    let padded_width = |width: i32| {
      width
        .wrapping_add(i32::from(horizontal != 0))
        .wrapping_shr(horizontal)
    };
    let first = margin.wrapping_shr(vertical);
    let available = remaining
      .wrapping_add(i32::from(vertical != 0))
      .wrapping_shr(vertical);
    let rows = available
      .min(body.wrapping_shr(vertical))
      .wrapping_add(first);
    let width = u64::try_from(padded_width(f.sr_cur.p.p.w)).map_err(|_invalid_width| EINVAL)?;
    let height = u64::try_from(rows).map_err(|_invalid_rows| EINVAL)?;
    let source_width = u64::try_from(padded_width(f.bw.wrapping_mul(4)))
      .map_err(|_invalid_source_width| EINVAL)?;
    let Ok(dst_pitch) = dst.fixed_pixel_stride::<BD>();
    let Ok(src_pitch) = src.fixed_pixel_stride::<BD>();
    let Ok(dst_back) =
      console_core_number_conversion::wrapped_offset(i64::from(first).wrapping_mul(dst_pitch));
    let Ok(src_back) =
      console_core_number_conversion::wrapped_offset(i64::from(first).wrapping_mul(src_pitch));
    let Ok(destination) = WithOffset::pic(dst - dst_back);
    let sampling = ResizeSampling {
      source_width,
      destination: Size { width, height },
      phase,
    };
    f.dsp
      .mc
      .resize
      .call::<BD>(destination, src - src_back, sampling, depth)?;
  }

  Ok(())
}

pub(crate) fn rav1d_filter_sbrow_lr<BD: BitDepth>(
  c: &Rav1dContext,
  f: &Rav1dFrameData,
  _t: &mut Rav1dTaskContext,
  sby: c_int,
) -> Rav1dResult {
  {
    match ::core::primitive::bool::then_some(
      !c.inloop_filters
        .contains(Rav1dInloopFilterType::RESTORATION),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let y = i32::wrapping_mul(i32::wrapping_mul(sby, f.sb_step), 4);
    let sr_p = f.sr_cur.p.lf_offsets::<BD>(y);
    rav1d_lr_sbrow::<BD>(c, f, sr_p, sby)?;
  };

  Ok(())
}

pub(crate) fn rav1d_filter_sbrow<BD: BitDepth>(
  c: &Rav1dContext,
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
  sby: c_int,
) -> Rav1dResult {
  {
    let sequence = f.seq_hdr.as_ref().ok_or(EINVAL)?;
    let frame = f.frame_hdr.as_ref().ok_or(EINVAL)?;
    rav1d_filter_sbrow_deblock_cols::<BD>(c, f, t, sby)?;
    rav1d_filter_sbrow_deblock_rows::<BD>(c, f, t, sby)?;
    let seq_hdr = &***sequence;

    match ::core::primitive::bool::then_some(seq_hdr.cdef != 0, ()) {
      ::core::option::Option::Some(()) => {
        rav1d_filter_sbrow_cdef::<BD>(c, f, t, sby)?;
      }
      ::core::option::Option::None => {}
    }

    let frame_hdr = &***frame;

    match ::core::primitive::bool::then_some(
      (match frame_hdr.size.width {
        [element, ..] => element,
      }) != (match frame_hdr.size.width {
        [_, element, ..] => element,
      }),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        rav1d_filter_sbrow_resize::<BD>(c, f, t, sby)?;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(!f.lf.restore_planes.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        rav1d_filter_sbrow_lr::<BD>(c, f, t, sby)?;
      }
      ::core::option::Option::None => {}
    }
  };

  Ok(())
}

pub(crate) fn rav1d_backup_ipred_edge<BD: BitDepth>(
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
) -> Rav1dResult {
  let tile = f.tile(t.tile_index)?;
  let superblock_shift = u32::try_from(f.sb_shift).map_err(|_invalid_shift| EINVAL)?;
  let superblock_row =
    u64::try_from(t.b.y.wrapping_shr(superblock_shift)).map_err(|_invalid_row| EINVAL)?;
  let superblocks = u64::try_from(f.sb128w).map_err(|_invalid_width| EINVAL)?;
  let row = superblock_row.checked_mul(superblocks).ok_or(EINVAL)?;
  let row_origin = row.checked_mul(128).ok_or(EINVAL)?;
  let first_column = tile.tiling.col_start.wrapping_mul(4);
  let luma_rows = t.b.y.wrapping_add(f.sb_step).wrapping_mul(4);
  let tile_width = tile
    .tiling
    .col_end
    .wrapping_sub(tile.tiling.col_start)
    .wrapping_mul(4);
  let width = u64::try_from(tile_width).map_err(|_invalid_tile_width| EINVAL)?;
  let layout = f.cur.p.layout;
  let plane_count = match layout {
    Rav1dPixelLayout::I400 => 1_u8,
    Rav1dPixelLayout::I420 | Rav1dPixelLayout::I422 | Rav1dPixelLayout::I444 => 3,
  };
  let picture = f.cur.data.as_ref().ok_or(EINVAL)?;
  let Ok(planes) = crate::src::owned_recon::ReconPlanes::bind(&picture.data, &mut t.recon_band);
  let Ok(bytes) = console_core_number_conversion::fixed_index(f.ipred_edge.len());
  let capacity = BD::pxstride(bytes);

  for plane in 0..plane_count {
    let horizontal = u32::from(plane != 0 && layout != Rav1dPixelLayout::I444);
    let vertical = u32::from(plane != 0 && layout == Rav1dPixelLayout::I420);
    let origin = Point {
      x: first_column.wrapping_shr(horizontal),
      y: luma_rows.wrapping_shr(vertical).wrapping_sub(1),
    };
    let column = u64::try_from(origin.x).map_err(|_invalid_column| EINVAL)?;
    let bank = f
      .ipred_edge_off
      .checked_mul(u64::from(plane))
      .ok_or(EINVAL)?;
    let stripe = bank.checked_add(row_origin).ok_or(EINVAL)?;
    let start = stripe.checked_add(column).ok_or(EINVAL)?;
    let count = width.wrapping_shr(horizontal);
    let end = start.checked_add(count).ok_or(EINVAL)?;

    match end.cmp(&capacity) {
      cmp::Ordering::Less | cmp::Ordering::Equal => {}
      cmp::Ordering::Greater => return Err(EINVAL),
    }

    let input = planes.src::<BD>(plane, origin)?;
    let Ok(samples) = console_core_number_conversion::index(count);
    let Ok(destination) = console_core_number_conversion::indices(start..end);
    let source = input.slice::<BD>(samples)?;
    BD::pixel_copy(
      &mut f.ipred_edge.mut_slice_as(destination),
      &source,
      samples,
    );
  }

  Ok(())
}

pub(crate) fn rav1d_copy_pal_block<BD: BitDepth>(
  t: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
  PaletteCopy { region, plane }: PaletteCopy,
) -> Rav1dResult {
  let frame_palette;
  let palette = match t.frame_thread.pass {
    0 => {
      let scratch = t.scratch.inter_intra()?;
      let buffers = scratch.interintra_edge_pal.pal.buf::<BD>()?;
      buffers
    }
    _ => {
      let stride = i64::from(f.b4_stride);
      let position = palette_cache::PalettePosition { x: t.b.x, y: t.b.y };
      let record = palette_cache::record_index(position, stride)?;
      let palette = f.frame_thread.pal.checked_index::<BD>(record)?;
      frame_palette = palette;
      &*frame_palette
    }
  };
  let Rectangle { origin, size } = region;
  let x_end = origin.x.checked_add(size.width).ok_or(EINVAL)?;
  let y_end = origin.y.checked_add(size.height).ok_or(EINVAL)?;
  let x =
    console_core_number_conversion::checked_index(origin.x).map_err(|_outside_palette| EINVAL)?;
  let y =
    console_core_number_conversion::checked_index(origin.y).map_err(|_outside_palette| EINVAL)?;
  let right =
    console_core_number_conversion::checked_index(x_end).map_err(|_outside_palette| EINVAL)?;
  let bottom =
    console_core_number_conversion::checked_index(y_end).map_err(|_outside_palette| EINVAL)?;
  let [above, left] = t.al_pal.select_mut::<BD>()?;
  let above_2 = above.get_mut(x..right).ok_or(EINVAL)?;
  let left_2 = left.get_mut(y..bottom).ok_or(EINVAL)?;
  let [luma, u, v] = *palette;

  for [above_luma, above_u, above_v] in above_2.iter_mut().chain(left_2) {
    match plane {
      PalettePlane::Luma => *above_luma = luma,
      PalettePlane::Chroma => {
        *above_u = u;
        *above_v = v;
      }
    }
  }

  Ok(())
}

pub(crate) fn rav1d_read_pal_y<BD: BitDepth>(
  t: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
  ts_c: &mut Rav1dTileStateContext,
  point: PaletteRead,
) -> Rav1dResult<u8> {
  rav1d_read_pal_plane::<BD>(t, f, ts_c, PalettePlane::Luma, point)
}

struct PaletteTrace<'a, Pixel>(&'a [Pixel]);

impl<Pixel: Copy + ToPrimitive<i32>> core::fmt::Display for PaletteTrace<'_, Pixel> {
  fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(formatter, "[")?;
    let mut separator = "";

    for &color in self.0 {
      write!(formatter, "{separator}{:02x}", color.as_::<i32>())?;
      separator = " ";
    }

    write!(formatter, "]")
  }
}

fn read_palette_size(
  context: &mut Rav1dTileStateContext,
  plane: PalettePlane,
  size_context: u8,
) -> Rav1dResult<u8> {
  let [luma, chroma] = &mut *context.cdf.m.pal_sz;
  let probabilities = match plane {
    PalettePlane::Luma => luma,
    PalettePlane::Chroma => chroma,
  };
  let Ok(position) = console_core_number_conversion::index(size_context);
  let cdf = probabilities.get_mut(position).ok_or(EINVAL)?;
  let Ok(symbol) = rav1d_msac_decode_symbol_adapt8(&mut context.msac, cdf, 6);
  symbol
    .checked_add(2)
    .filter(|size| *size <= 8)
    .ok_or(EINVAL)
}

fn select_cached_palette<'a, Pixel: Copy>(
  entropy: &mut MsacContext,
  destination: &'a mut [Pixel],
  neighbors: &[Pixel],
) -> Rav1dResult<&'a [Pixel]> {
  let mut unused = destination.iter_mut();
  let mut selected = 0u8;
  let mut slot = unused.next();

  for &neighbor in neighbors {
    let entry = match slot {
      Some(entry) => entry,
      None => break,
    };
    let Ok(included) = rav1d_msac_decode_bool_equi(entropy);

    match included {
      EntropyBit::Zero => slot = Some(entry),
      EntropyBit::One => {
        *entry = neighbor;
        selected = selected.wrapping_add(1);
        slot = unused.next();
      }
    }
  }

  let Ok(end) = console_core_number_conversion::index(selected);
  destination.get(..end).ok_or(EINVAL)
}

struct PaletteDeltaFormat {
  plane: PalettePlane,
  bit_depth: u8,
}

enum PaletteDeltas {
  Reading { previous: u16, bits: u8 },
  Saturated,
}

fn decode_palette_deltas<BD: BitDepth>(
  entropy: &mut MsacContext,
  colors: &mut [BD::Pixel],
  format: PaletteDeltaFormat,
) -> Result<(), console_core_never::Never> {
  let (first, remaining) = match colors.split_first_mut() {
    Some(remaining) => remaining,
    None => return Ok(()),
  };
  let Ok(first_code) = rav1d_msac_decode_bools(entropy, format.bit_depth);
  let Ok(previous) = console_core_number_conversion::wrapped::<_, u16>(first_code);
  *first = previous.as_::<BD::Pixel>();

  match remaining {
    [] => return Ok(()),
    [_, ..] => {}
  }

  let increment = match format.plane {
    PalettePlane::Luma => 1u16,
    PalettePlane::Chroma => 0,
  };
  let Ok(extra) = rav1d_msac_decode_bools(entropy, 2);
  let Ok(extra_bits) = console_core_number_conversion::wrapped::<_, u8>(extra);
  let initial_width = format.bit_depth.wrapping_add(extra_bits).wrapping_sub(3);
  let maximum = 1u16
    .wrapping_shl(u32::from(format.bit_depth))
    .wrapping_sub(1);
  let mut state = PaletteDeltas::Reading {
    previous,
    bits: initial_width,
  };

  for color in remaining {
    match state {
      PaletteDeltas::Reading {
        previous: last_color,
        bits,
      } => {
        let Ok(code) = rav1d_msac_decode_bools(entropy, bits);
        let Ok(delta) = console_core_number_conversion::wrapped::<_, u16>(code);
        let sample = last_color
          .wrapping_add(delta)
          .wrapping_add(increment)
          .min(maximum);
        *color = sample.as_::<BD::Pixel>();
        let remaining_range = sample.wrapping_add(increment);
        state = match remaining_range.cmp(&maximum) {
          cmp::Ordering::Greater | cmp::Ordering::Equal => PaletteDeltas::Saturated,
          cmp::Ordering::Less => {
            let log = ulog2(u32::from(maximum.wrapping_sub(remaining_range)));
            let Ok(required_bits) = console_core_number_conversion::wrapped::<_, u8>(log);
            PaletteDeltas::Reading {
              previous: sample,
              bits: bits.min(required_bits.wrapping_add(1)),
            }
          }
        };
      }
      PaletteDeltas::Saturated => *color = maximum.as_::<BD::Pixel>(),
    }
  }

  Ok(())
}

fn rav1d_read_pal_plane<BD: BitDepth>(
  t: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
  ts_c: &mut Rav1dTileStateContext,
  plane: PalettePlane,
  PaletteRead {
    size_context: sz_ctx,
    position,
  }: PaletteRead,
) -> Rav1dResult<u8> {
  Ok({
    let bx4 = console_core_number_conversion::checked_index(position.x)
      .map_err(|_outside_palette| EINVAL)?;
    let by4 = console_core_number_conversion::checked_index(position.y)
      .map_err(|_outside_palette| EINVAL)?;
    let plane_code = match plane {
      PalettePlane::Luma => 0u8,
      PalettePlane::Chroma => 1,
    };
    let Ok(pli) = console_core_number_conversion::index(plane_code);

    let [above_sizes, left_sizes] = &t.pal_sz_uv;
    let left_count = match plane {
      PalettePlane::Chroma => left_sizes.get(by4).copied().ok_or(EINVAL)?,
      PalettePlane::Luma => t.l.pal_sz.get_mut().get(by4).copied().ok_or(EINVAL)?,
    };
    let above_count = match (position.y & 15, plane) {
      (0, _) => 0,
      (_, PalettePlane::Chroma) => above_sizes.get(bx4).copied().ok_or(EINVAL)?,
      (_, PalettePlane::Luma) => {
        let above_context = f.above_context(t.above_index)?;

        match (bx4 < above_context.pal_sz.len()).then_some(()) {
          Some(()) => *above_context.pal_sz.index(bx4),
          None => return Err(EINVAL),
        }
      }
    };
    let [above_planes, left_planes] = t.al_pal.select_mut::<BD>()?;
    let left = left_planes
      .get(by4)
      .and_then(|planes| planes.get(pli))
      .ok_or(EINVAL)?;
    let above = above_planes
      .get(bx4)
      .and_then(|planes| planes.get(pli))
      .ok_or(EINVAL)?;
    let mut cache = [0.as_::<BD::Pixel>(); 16];
    let neighborhoods = palette_cache::NeighborColors {
      left,
      left_count,
      above,
      above_count,
    };
    let neighbors = palette_cache::merge_neighbors(neighborhoods, &mut cache)?;
    let palette_size = read_palette_size(ts_c, plane, sz_ctx)?;
    let Ok(palette_end) = console_core_number_conversion::index(palette_size);
    let mut used_cache = [BD::Pixel::from(0); 8];
    let cache_slots = used_cache.get_mut(..palette_end).ok_or(EINVAL)?;
    let selected_colors = select_cached_palette(&mut ts_c.msac, cache_slots, neighbors)?;

    let mut frame_palette;
    let planes = match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
      ::core::option::Option::Some(()) => {
        let stride = i64::from(f.b4_stride);
        let position_2 = palette_cache::PalettePosition { x: t.b.x, y: t.b.y };
        let record = palette_cache::record_index(position_2, stride)?;
        let palette = f.frame_thread.pal.checked_index_mut::<BD>(record)?;
        frame_palette = palette;
        &mut *frame_palette
      }
      ::core::option::Option::None => {
        let scratch = t.scratch.inter_intra_mut()?;
        let planes = scratch.interintra_edge_pal.pal.buf_mut::<BD>()?;
        planes
      }
    };
    let [luma, chroma, _] = planes;
    let colors = match plane {
      PalettePlane::Chroma => chroma,
      PalettePlane::Luma => luma,
    };
    let pal = colors.get_mut(..palette_end).ok_or(EINVAL)?;

    let new_colors = pal.get_mut(selected_colors.len()..).ok_or(EINVAL)?;

    let format = PaletteDeltaFormat {
      plane,
      bit_depth: f.cur.p.bpc,
    };
    let Ok(()) = decode_palette_deltas::<BD>(&mut ts_c.msac, new_colors, format);
    palette_cache::merge_decoded(pal, selected_colors)?;

    match {
      let Ok(active_trace) = debug_block_info(f, t.b);
      active_trace
    } {
      BlockTrace::Enabled => console_core_logging::trace!(
        "Post-pal[pl={plane_code},sz={palette_size},cache_size={},used_cache={}]: r={}, cache={}, pal={}",
        neighbors.len(),
        selected_colors.len(),
        ts_c.msac.rng,
        PaletteTrace(neighbors),
        PaletteTrace(pal),
      ),
      BlockTrace::Disabled => {}
    }

    palette_size
  })
}

fn read_v_palette_deltas<BD: BitDepth>(
  entropy: &mut crate::src::msac::MsacContext,
  samples: &mut [BD::Pixel],
  depth: u8,
) -> Rav1dResult {
  let Ok(extra) = rav1d_msac_decode_bools(entropy, 2);
  let Ok(extra_bits) = console_core_number_conversion::wrapped::<_, u8>(extra);
  let bits = depth.wrapping_add(extra_bits).wrapping_sub(4);
  let Ok(first_sample) = rav1d_msac_decode_bools(entropy, depth);
  let Ok(mut previous) = console_core_number_conversion::wrapped::<_, u16>(first_sample);
  let (first, rest) = samples.split_first_mut().ok_or(EINVAL)?;
  *first = previous.as_::<BD::Pixel>();
  let mask = 1_u16.wrapping_shl(u32::from(depth)).wrapping_sub(1);

  for sample in rest {
    let Ok(magnitude) = rav1d_msac_decode_bools(entropy, bits);
    let Ok(delta) = console_core_number_conversion::wrapped::<_, i16>(magnitude);
    let signed = match delta {
      0 => 0,
      _ => {
        let Ok(sign) = rav1d_msac_decode_bool_equi(entropy);

        match sign {
          EntropyBit::One => delta.wrapping_neg(),
          EntropyBit::Zero => delta,
        }
      }
    };
    previous = previous.wrapping_add(signed.cast_unsigned()) & mask;
    *sample = previous.as_::<BD::Pixel>();
  }

  Ok(())
}

pub(crate) fn rav1d_read_pal_uv<BD: BitDepth>(
  t: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
  ts_c: &mut Rav1dTileStateContext,
  point: PaletteRead,
) -> Rav1dResult<u8> {
  Ok({
    let pal_sz = rav1d_read_pal_plane::<BD>(t, f, ts_c, PalettePlane::Chroma, point)?;
    let mut frame_palette;
    let planes = match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
      ::core::option::Option::Some(()) => {
        let stride = i64::from(f.b4_stride);
        let position = palette_cache::PalettePosition { x: t.b.x, y: t.b.y };
        let record = palette_cache::record_index(position, stride)?;
        let palette = f.frame_thread.pal.checked_index_mut::<BD>(record)?;
        frame_palette = palette;
        &mut *frame_palette
      }
      ::core::option::Option::None => {
        let scratch = t.scratch.inter_intra_mut()?;
        let planes = scratch.interintra_edge_pal.pal.buf_mut::<BD>()?;
        planes
      }
    };
    let [_, _, v] = planes;
    let Ok(end) = console_core_number_conversion::index(pal_sz);
    let samples = v.get_mut(..end).ok_or(EINVAL)?;
    let depth = f.cur.p.bpc;
    let Ok(differential) = rav1d_msac_decode_bool_equi(&mut ts_c.msac);

    match differential {
      EntropyBit::One => read_v_palette_deltas::<BD>(&mut ts_c.msac, samples, depth)?,
      EntropyBit::Zero => {
        for sample in samples {
          let Ok(decoded) = rav1d_msac_decode_bools(&mut ts_c.msac, depth);
          *sample = decoded.as_::<BD::Pixel>();
        }
      }
    }

    pal_sz
  })
}
