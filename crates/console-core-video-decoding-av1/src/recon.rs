#![forbid(unsafe_code)]
use crate::include::common::bitdepth::AsPrimitive;
use crate::include::common::bitdepth::BPC;
use crate::include::common::bitdepth::BitDepth;
use crate::include::common::bitdepth::ToPrimitive;
use crate::include::common::dump::coef_dump;
use crate::include::common::dump::hex_dump;
use crate::include::common::dump::hex_dump_pic;
use crate::include::common::intops::apply_sign64;
use crate::include::common::intops::clip;
use crate::include::common::intops::ulog2;
use crate::include::dav1d::dav1d::Rav1dInloopFilterType;
use crate::include::dav1d::headers::Rav1dFilterMode;
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::headers::Rav1dPixelLayoutSubSampled;
use crate::include::dav1d::headers::Rav1dWarpedMotionParams;
use crate::include::dav1d::headers::Rav1dWarpedMotionType;
use crate::include::dav1d::picture::PicOffset;
use crate::include::dav1d::picture::Rav1dPictureDataComponent;

use crate::src::cdef_apply::{CdefRowSpan, CdefRows, rav1d_cdef_brow};
use crate::src::ctx::{CaseSet, ContextRun};
use crate::src::ctx::case_set_al;
use crate::src::ctx::small_memset;
use crate::src::env::get_uv_inter_txtp;
use crate::src::error::Rav1dError::{EINVAL, EIO};
use console_core_geometry::{Point, Rectangle, Size};
use crate::src::error::Rav1dResult;
use crate::src::pal::PalettePlane;
#[path = "palette_cache.rs"]
mod palette_cache;
use crate::src::in_range::InRange;
use crate::src::internal::Bxy;
use crate::src::internal::ChromaNeighborPrediction;
use crate::src::internal::Cf;
use crate::src::internal::CodedBlockInfo;
use crate::src::internal::Rav1dContext;
use crate::src::internal::Rav1dFrameData;
use crate::src::internal::Rav1dTaskContext;
use crate::src::internal::{Rav1dTileState, Rav1dTileStateContext};
use crate::src::internal::ScratchEmuEdge;
use crate::src::internal::TaskContextScratch;
use crate::src::internal::TileStateRef;
use crate::src::intra_edge::{EdgeAvailability, EdgeFlags};
use crate::src::ipred::{CflPrediction, IntraPrediction};
use crate::src::ipred_prepare::{
  AboveEdge, IntraEdgeDestination, IntraEdgePreparation, PrefilteredTop, rav1d_prepare_intra_edges,
};
use crate::src::ipred_prepare::sm_flag;
use crate::src::ipred_prepare::sm_uv_flag;
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
use crate::src::levels::WHT_WHT;
use crate::src::lf_apply::rav1d_copy_lpf;
use crate::src::mc::{ResizePhase, ResizeSampling};
use crate::src::lf_apply::rav1d_loopfilter_sbrow_cols;
use crate::src::lf_apply::rav1d_loopfilter_sbrow_rows;
use crate::src::lr_apply::rav1d_lr_sbrow;
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
use crate::src::scan::dav1d_scans;
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
use std::array;
use std::cmp;
use std::ffi::c_int;
use std::ffi::c_uint;
use std::ptr;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockTrace {
  Enabled,
  Quiet,
}

pub(crate) const BLOCK_TRACE: BlockTrace = BlockTrace::Quiet;
const PIXEL_TRACE: BlockTrace = BlockTrace::Quiet;
const COEFFICIENT_TRACE: BlockTrace = BlockTrace::Quiet;

impl Bxy {
  #[must_use]
  pub(crate) fn trace(&self) -> BlockTrace {
    match *self {
      Self { x: 8..=11, y: 0..=3 } => BlockTrace::Enabled,
      _ => BlockTrace::Quiet,
    }
  }
}

macro_rules! debug_block_info {
  ($f:expr, $tb:expr) => {{
    use crate::src::internal::Bxy;
    use crate::src::recon::{BLOCK_TRACE, BlockTrace};

    let block: Bxy = $tb;
    match BLOCK_TRACE {
      BlockTrace::Enabled => match $f.frame_hdr.as_ref() {
        Some(header) if header.frame_offset == 2 => block.trace(),
        Some(_) | None => BlockTrace::Quiet,
      },
      BlockTrace::Quiet => BlockTrace::Quiet,
    }
  }};
}
pub(crate) use debug_block_info;

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

pub(crate) type ReadPalFn =
  fn(&mut Rav1dTaskContext, &Rav1dFrameData, &mut Rav1dTileStateContext, PaletteRead) -> Rav1dResult<u8>;

#[inline]
fn read_golomb(msac: &mut MsacContext) -> core::result::Result<u32, console_core_never::Never> {
  Ok({
    let mut len = 0u32;
    let mut val = 1u32;

    while !matches!(rav1d_msac_decode_bool_equi(msac), Ok(EntropyBit::One)) && len < 31 {
      {
        let numeric_amount = 1;
        let numeric_target = &mut (len);
        *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount);
      };
    }

    for _ in 0..len {
      val = u32::wrapping_add(
        u32::wrapping_shl(val, 1_u32),
        u32::from(matches!(rav1d_msac_decode_bool_equi(msac), Ok(EntropyBit::One))),
      );
    }

    val.wrapping_sub(1)
  })
}

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
      let merged = context.iter().copied().fold(0, |combined, entry| combined | entry);
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
      let more_than_one = width.wrapping_sub(horizontal) > t_dim.lw
        || height.wrapping_sub(vertical) > t_dim.lh;
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
  x: u8,
  y: u8,
  stride: u8,
) -> Rav1dResult<u8> {
  let level = |offset: Point<u16>| -> Rav1dResult<u32> {
    let position = offset.y.wrapping_mul(u16::from(stride)).wrapping_add(offset.x);
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
      let offset = offsets.get(row).and_then(|values| values.get(column)).copied().ok_or(EINVAL)?;
      (first.wrapping_add(diagonal), farther.wrapping_add(downward), offset)
    }
    (TxClass::H | TxClass::V, None) => {
      let third = level(Point { x: 3, y: 0 })?;
      let fourth = level(Point { x: 4, y: 0 })?;
      let offset = 26u8.wrapping_add(y.min(2).wrapping_mul(5));
      (first.wrapping_add(farther), third.wrapping_add(fourth), offset)
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

fn decode_coefs<BD: BitDepth>(
  f: &Rav1dFrameData,
  CoefficientDecoder { tile: ts, state: ts_c, scratch, coefficients: t_cf }: CoefficientDecoder<'_>,
  CoefficientBlock {
    trace: dbg_block_info, above: a, left: l, transform: tx, size: bs,
    syntax: b, plane, storage: cf,
  }: CoefficientBlock<'_>,
  CoefficientOutput { transform: txtp, residual_context: res_ctx }: CoefficientOutput<'_>,
) -> Rav1dResult<c_int> {
  Ok({
    let dc_sign_ctx;
    let dc_sign;
    let mut dc_dq;
    let chroma = plane != 0;
    let frame_hdr = f.frame_hdr.as_ref().ok_or(EINVAL)?;
    let lossless = *match <[_]>::get(&frame_hdr.segmentation.lossless, {
      match b.seg_id.get() { Ok(value) => match console_core_number_conversion::index(value) { Ok(position) => position } }
    }) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
      }
    };
    let t_dim = &*match <[_]>::get(&dav1d_txfm_dimensions, { let Ok(table_index) = console_core_number_conversion::index({ let Ok(ordinal) = tx.ordinal(); ordinal }); table_index }) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
      }
    };

    let a = a
      .get(
        ..match ::console_core_number_conversion::index::<::core::primitive::u8>(t_dim.w) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        },
      )
      .ok_or(EINVAL)?;
    let l = l
      .get(
        ..match ::console_core_number_conversion::index::<::core::primitive::u8>(t_dim.h) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        },
      )
      .ok_or(EINVAL)?;
    let dbg = match (dbg_block_info, plane, COEFFICIENT_TRACE) {
      (BlockTrace::Enabled, 1.., BlockTrace::Enabled) => BlockTrace::Enabled,
      _ => BlockTrace::Quiet,
    };

    match dbg {
      BlockTrace::Enabled => {
        println!("Start: r={}", ts_c.msac.rng);
      }
      BlockTrace::Quiet => {}
    }

    let residual_plane = match plane {
      0 => ResidualPlane::Luma,
      _ => ResidualPlane::Chroma(f.cur.p.layout),
    };
    let neighbors = ResidualNeighbors { above: a, left: l };
    let sctx = get_skip_ctx(t_dim, bs, neighbors, residual_plane)?;
    let all_skip = matches!(rav1d_msac_decode_bool_adapt(
      &mut ts_c.msac,
      (match <[_]>::get_mut(&mut *ts_c.cdf.coef.skip, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t_dim.ctx);
        numeric_value
      }) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
        }
      }).get_mut({
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index({
          let Ok(explicit_value) = sctx.get();
          explicit_value
        });
        numeric_value
      }).ok_or(EINVAL)?,
    ), Ok(EntropyBit::One));

    match dbg {
      BlockTrace::Enabled => {
        println!(
          "Post-non-zero[{}][{}][{}]: r={}",
          t_dim.ctx, sctx, all_skip, ts_c.msac.rng,
        );
      }
      BlockTrace::Quiet => {}
    }

    match ::core::primitive::bool::then_some(all_skip, ()) {
      ::core::option::Option::Some(()) => {
        *res_ctx = 0x40;
        *txtp = match ::core::primitive::bool::then_some(lossless, ()) {
          ::core::option::Option::Some(()) => WHT_WHT,
          ::core::option::Option::None => DCT_DCT,
        };
        return Ok(-1);
      }
      ::core::option::Option::None => {}
    }

    use Av1BlockIntraInter::*;
    let Ok(tx_4x4) = TxfmSize::S4x4.ordinal();
    let Ok(tx_16x16) = TxfmSize::S16x16.ordinal();
    let Ok(tx_32x32) = TxfmSize::S32x32.ordinal();
    let Ok(tx_64x64) = TxfmSize::S64x64.ordinal();
    *txtp = match &b.ii {
      _ if lossless => {
        assert!(t_dim.max == tx_4x4);
        WHT_WHT
      }
      Intra(_) if t_dim.max >= tx_32x32 => DCT_DCT,
      Inter(_) if t_dim.max >= tx_64x64 => DCT_DCT,
      Intra(intra) if chroma => *match <[_]>::get(&dav1d_txtp_from_uvmode, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(intra.uv_mode);
        numeric_value
      }) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
        }
      },

      Inter(_) if chroma => {
        let Ok(explicit_value) = get_uv_inter_txtp(t_dim, *txtp);
        explicit_value
      }

      _ if *match <[_]>::get(&frame_hdr.segmentation.qidx, {
        match b.seg_id.get() { Ok(value) => match console_core_number_conversion::index(value) { Ok(position) => position } }
      }) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
        }
      } == 0 =>
      {
        DCT_DCT
      }
      Intra(intra) => {
        let y_mode_nofilt = match ::core::primitive::bool::then_some(intra.y_mode == FILTER_PRED, ()) {
          ::core::option::Option::Some(()) => *match <[_]>::get(&dav1d_filter_mode_to_y_mode, {
            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(intra.y_angle);
            numeric_value
          }) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
            }
          },
          ::core::option::Option::None => intra.y_mode,
        };
        let idx;
        let txtp = match ::core::primitive::bool::then_some(frame_hdr.reduced_txtp_set != 0 || t_dim.min == tx_16x16, ()) {
          ::core::option::Option::Some(()) => {
            idx = {
              let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt8(
                &mut ts_c.msac,
                (match <[_]>::get_mut(&mut *ts_c.cdf.m.txtp_intra2, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped_index(t_dim.min);
                  numeric_value
                }) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                }).get_mut({
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped_index(y_mode_nofilt);
                  numeric_value
                }).ok_or(EINVAL)?,
                4,
              );
              explicit_value
            };
            *match <[_]>::get(
              &dav1d_tx_types_per_set,
              ({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(idx);
                numeric_value
              })
              .wrapping_add(0),
            ) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            }
          }
          ::core::option::Option::None => {
            idx = {
              let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt8(
                &mut ts_c.msac,
                (match <[_]>::get_mut(&mut *ts_c.cdf.m.txtp_intra1, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped_index(t_dim.min);
                  numeric_value
                }) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                }).get_mut({
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped_index(y_mode_nofilt);
                  numeric_value
                }).ok_or(EINVAL)?,
                6,
              );
              explicit_value
            };
            *match <[_]>::get(
              &dav1d_tx_types_per_set,
              ({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(idx);
                numeric_value
              })
              .wrapping_add(5),
            ) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            }
          }
        };

        match dbg {
          BlockTrace::Enabled => {
            println!(
              "Post-txtp-intra[{:?}->{}][{}][{}->{}]: r={}",
              tx, t_dim.min, y_mode_nofilt, idx, txtp, ts_c.msac.rng,
            );
          }
          BlockTrace::Quiet => {}
        }

        txtp
      }
      Inter(_) => {
        let idx;
        let txtp = match ::core::primitive::bool::then_some(frame_hdr.reduced_txtp_set != 0 || t_dim.max == tx_32x32, ()) {
          ::core::option::Option::Some(()) => {
            let bool_idx = matches!(rav1d_msac_decode_bool_adapt(
              &mut ts_c.msac,
              &mut *match <[_]>::get_mut(&mut *ts_c.cdf.m.txtp_inter3, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t_dim.min);
                numeric_value
              }) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              },
            ), Ok(EntropyBit::One));
            idx = u8::from(bool_idx);

            match ::core::primitive::bool::then_some(bool_idx, ()) {
              ::core::option::Option::Some(()) => DCT_DCT,
              ::core::option::Option::None => IDTX,
            }
          }
          ::core::option::Option::None => match ::core::primitive::bool::then_some(t_dim.min == tx_16x16, ()) {
            ::core::option::Option::Some(()) => {
              idx = {
                let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt16(
                  &mut ts_c.msac,
                  &mut *ts_c.cdf.m.txtp_inter2,
                  11,
                );
                explicit_value
              };
              *match <[_]>::get(
                &dav1d_tx_types_per_set,
                ({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(idx);
                  numeric_value
                })
                .wrapping_add(12),
              ) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              }
            }
            ::core::option::Option::None => {
              idx = {
                let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt16(
                  &mut ts_c.msac,
                  &mut *match <[_]>::get_mut(&mut *ts_c.cdf.m.txtp_inter1, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped_index(t_dim.min);
                    numeric_value
                  }) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => {
                      return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                    }
                  },
                  15,
                );
                explicit_value
              };
              *match <[_]>::get(
                &dav1d_tx_types_per_set,
                ({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(idx);
                  numeric_value
                })
                .wrapping_add(24),
              ) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              }
            }
          },
        };

        match dbg {
          BlockTrace::Enabled => {
            println!(
              "Post-txtp-inter[{:?}->{}][{}->{}]: r={}",
              tx, t_dim.min, idx, txtp, ts_c.msac.rng,
            );
          }
          BlockTrace::Quiet => {}
        }

        txtp
      }
    };

    let tx2dszctx = u8::wrapping_add(
      cmp::min(t_dim.lw, { let Ok(ordinal) = TxfmSize::S32x32.ordinal(); ordinal }),
      cmp::min(t_dim.lh, { let Ok(ordinal) = TxfmSize::S32x32.ordinal(); ordinal }),
    );
    let tx_class = *match <[_]>::get(&dav1d_tx_type_class, {
      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(*txtp);
      numeric_value
    }) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
      }
    };
    let ::core::result::Result::Ok(chroma) = ::console_core_number_conversion::index::<
      ::core::primitive::u8,
    >(::core::primitive::u8::from(chroma));
    let ::core::result::Result::Ok(is_1d) =
      ::console_core_number_conversion::index::<::core::primitive::u8>(
        ::core::primitive::u8::from(tx_class != TxClass::TwoD),
      );
    let eob_bin = match tx2dszctx {
      0 => {
        let eob_bin_cdf = (match <[_]>::get_mut(&mut *ts_c.cdf.coef.eob_bin_16, chroma) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        }).get_mut(is_1d).ok_or(EINVAL)?;

        {
          let Ok(explicit_value) =
            rav1d_msac_decode_symbol_adapt8(&mut ts_c.msac, eob_bin_cdf, u8::wrapping_add(4, 0));
          explicit_value
        }
      }
      1 => {
        let eob_bin_cdf = (match <[_]>::get_mut(&mut *ts_c.cdf.coef.eob_bin_32, chroma) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        }).get_mut(is_1d).ok_or(EINVAL)?;

        {
          let Ok(explicit_value) =
            rav1d_msac_decode_symbol_adapt8(&mut ts_c.msac, eob_bin_cdf, u8::wrapping_add(4, 1));
          explicit_value
        }
      }
      2 => {
        let eob_bin_cdf = (match <[_]>::get_mut(&mut *ts_c.cdf.coef.eob_bin_64, chroma) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        }).get_mut(is_1d).ok_or(EINVAL)?;

        {
          let Ok(explicit_value) =
            rav1d_msac_decode_symbol_adapt8(&mut ts_c.msac, eob_bin_cdf, u8::wrapping_add(4, 2));
          explicit_value
        }
      }
      3 => {
        let eob_bin_cdf = (match <[_]>::get_mut(&mut *ts_c.cdf.coef.eob_bin_128, chroma) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        }).get_mut(is_1d).ok_or(EINVAL)?;

        {
          let Ok(explicit_value) =
            rav1d_msac_decode_symbol_adapt8(&mut ts_c.msac, eob_bin_cdf, u8::wrapping_add(4, 3));
          explicit_value
        }
      }
      4 => {
        let eob_bin_cdf = (match <[_]>::get_mut(&mut *ts_c.cdf.coef.eob_bin_256, chroma) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        }).get_mut(is_1d).ok_or(EINVAL)?;

        {
          let Ok(explicit_value) =
            rav1d_msac_decode_symbol_adapt16(&mut ts_c.msac, eob_bin_cdf, u8::wrapping_add(4, 4));
          explicit_value
        }
      }
      5 => {
        let eob_bin_cdf = &mut *match <[_]>::get_mut(&mut *ts_c.cdf.coef.eob_bin_512, chroma) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        };

        {
          let Ok(explicit_value) =
            rav1d_msac_decode_symbol_adapt16(&mut ts_c.msac, eob_bin_cdf, u8::wrapping_add(4, 5));
          explicit_value
        }
      }
      6 => {
        let eob_bin_cdf = &mut *match <[_]>::get_mut(&mut *ts_c.cdf.coef.eob_bin_1024, chroma) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        };

        {
          let Ok(explicit_value) =
            rav1d_msac_decode_symbol_adapt16(&mut ts_c.msac, eob_bin_cdf, u8::wrapping_add(4, 6));
          explicit_value
        }
      }

      _ => return Err(EINVAL),
    };

    match dbg {
      BlockTrace::Enabled => {
        println!(
          "Post-eob_bin_{}[{}][{}][{}]: r={}",
          i32::wrapping_shl(16, {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(tx2dszctx);
            numeric_value
          }),
          chroma,
          is_1d,
          eob_bin,
          ts_c.msac.rng,
        );
      }
      BlockTrace::Quiet => {}
    }

    let eob = match ::core::primitive::bool::then_some(eob_bin > 1, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(context_index) = console_core_number_conversion::index(t_dim.ctx);
        let Ok(bin_index) = console_core_number_conversion::index(eob_bin);
        let eob_hi_bit_cdf = ts_c.cdf.coef.eob_hi_bit.get_mut(context_index)
          .and_then(|contexts| contexts.get_mut(chroma))
          .and_then(|bins| bins.get_mut(bin_index)).ok_or(EINVAL)?;
        let eob_hi_bit = u16::from(matches!(rav1d_msac_decode_bool_adapt(&mut ts_c.msac, eob_hi_bit_cdf), Ok(EntropyBit::One)));

        match dbg {
          BlockTrace::Enabled => {
            println!(
              "Post-eob_hi_bit[{}][{}][{}][{}]: r={}",
              t_dim.ctx, chroma, eob_bin, eob_hi_bit, ts_c.msac.rng,
            );
          }
          BlockTrace::Quiet => {}
        }

        let eob = u16::wrapping_shl(eob_hi_bit | 2, {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, u32>(u8::wrapping_sub(eob_bin, 2));
          numeric_value
        }) | ({
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>({
            let Ok(explicit_value) =
              rav1d_msac_decode_bools(&mut ts_c.msac, u8::wrapping_sub(eob_bin, 2));
            explicit_value
          });
          numeric_value
        });

        match dbg {
          BlockTrace::Enabled => {
            println!("Post-eob[{}]: r={}", eob, ts_c.msac.rng);
          }
          BlockTrace::Quiet => {}
        }

        eob
      }
      ::core::option::Option::None => {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(eob_bin);
        numeric_value
      }
    };

    struct Cf<'a, BD: BitDepth>(&'a mut [BD::Coef]);

    impl<'a, BD: BitDepth> Cf<'a, BD> {
      fn position(&self, residue: u16) -> Result<u64, console_core_never::Never> {
        let Ok(length) = console_core_number_conversion::fixed_index(self.0.len());
        Ok(u64::from(residue) & length.wrapping_sub(1))
      }

      fn get(&self, residue: u16) -> Rav1dResult<i32> {
        let Ok(position) = self.position(residue);
        let Ok(index) = console_core_number_conversion::index(position);
        self.0.get(index).map(|&value| <_ as ::core::convert::Into<::core::primitive::i32>>::into(value)).ok_or(EINVAL)
      }

      fn set<T: ToPrimitive<BD::Coef>>(&mut self, residue: u16, value: T) -> Rav1dResult {
        let Ok(position) = self.position(residue);
        let Ok(index) = console_core_number_conversion::index(position);
        let coefficient = self.0.get_mut(index).ok_or(EINVAL)?;
        *coefficient = value.as_();
        Ok(())
      }
    }

    let Ok(sw) = console_core_number_conversion::wrapped_index(cmp::min(
      i32::wrapping_shl(1, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(t_dim.lw);
        numeric_value
      }),
      8,
    ));
    let Ok(sh) = console_core_number_conversion::wrapped_index(cmp::min(
      i32::wrapping_shl(1, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(t_dim.lh);
        numeric_value
      }),
      8,
    ));
    let cf_len = sw.wrapping_mul(4).wrapping_mul(sh).wrapping_mul(4);
    let cf = match cf {
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
    let mut cf = Cf::<BD>(cf);

    let mut rc;
    let mut dc_tok;

    #[inline]
    fn decode_coefs_class<const TX_CLASS: usize, BD: BitDepth>(
      ts_c: &mut Rav1dTileStateContext,
      t_dim: &TxfmInfo,
      chroma: usize,
      scratch: &mut TaskContextScratch,
      eob: u16,
      tx: TxfmSize,
      dbg: BlockTrace,
      cf: &mut Cf<BD>,
    ) -> Rav1dResult<(u16, u32)> {
      Ok({
        let tx_class = const { TxClass::from_repr(TX_CLASS) }.ok_or(EINVAL)?;

        let Ok(context_index) = console_core_number_conversion::index(t_dim.ctx);
        let Ok(high_index) = console_core_number_conversion::index(t_dim.ctx.min(3));
        let eob_cdf = ts_c.cdf.coef.eob_base_tok.get_mut(context_index)
          .and_then(|contexts| contexts.get_mut(chroma)).ok_or(EINVAL)?;
        let hi_cdf = ts_c.cdf.coef.br_tok.get_mut(high_index)
          .and_then(|contexts| contexts.get_mut(chroma)).ok_or(EINVAL)?;
        let lo_cdf = ts_c.cdf.coef.base_tok.get_mut(context_index)
          .and_then(|contexts| contexts.get_mut(chroma)).ok_or(EINVAL)?;
        let Ok(levels) = ({
          let explicit_value = scratch.inter_intra_mut()?;
          explicit_value
        })
        .levels_pal
        .levels_mut();

        let slw = cmp::min(t_dim.lw, { let Ok(ordinal) = TxfmSize::S32x32.ordinal(); ordinal });
        let slh = cmp::min(t_dim.lh, { let Ok(ordinal) = TxfmSize::S32x32.ordinal(); ordinal });
        let tx2dszctx = u8::wrapping_add(slw, slh);
        let sw = cmp::min(t_dim.w, 8);
        let sh = cmp::min(t_dim.h, 8);

        let mut ctx = u8::wrapping_add(
          u8::wrapping_add(
            1,
            u8::from(
              eob
                > u16::wrapping_shl(2u16, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(tx2dszctx);
                  numeric_value
                }),
            ),
          ),
          u8::from(
            eob
              > u16::wrapping_shl(4u16, {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u32>(tx2dszctx);
                numeric_value
              }),
          ),
        );
        let eob_tok = {
          let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt4(
            &mut ts_c.msac,
            eob_cdf.get_mut({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(ctx);
              numeric_value
            }).ok_or(EINVAL)?,
            2,
          );
          explicit_value
        };
        let mut tok = u8::wrapping_add(eob_tok, 1);
        let mut level_tok = u8::wrapping_mul(tok, 0x41);
        let mut mag = 0;

        let lo_ctx_offsets;
        let scan;
        let stride;

        match tx_class {
          TxClass::TwoD => {
            let Ok(code) = tx.ordinal();
            let rectangle = match tx.shape() {
              Ok(crate::src::levels::TransformShape::Square) => 0u8,
              Ok(crate::src::levels::TransformShape::Rectangular) => 1u8,
            };
            let Ok(layout_index) = console_core_number_conversion::index(
              rectangle.wrapping_add(code & rectangle),
            );
            let Ok(scan_index) = console_core_number_conversion::index(code);
            let offsets = dav1d_lo_ctx_offsets.get(layout_index).ok_or(EINVAL)?;
            lo_ctx_offsets = Some(offsets);
            let &selected_scan = dav1d_scans.get(scan_index).ok_or(EINVAL)?;
            scan = selected_scan;
            stride = u8::wrapping_mul(4, sh);
          }
          TxClass::H | TxClass::V => {
            lo_ctx_offsets = None;
            scan = &[];
            stride = 16;
          }
        }

        let shift;
        let shift2;
        let mask;
        let swh_zero;

        match tx_class {
          TxClass::TwoD => {
            shift = match ::core::primitive::bool::then_some(t_dim.lh < 4, ()) {
              ::core::option::Option::Some(()) => u8::wrapping_add(t_dim.lh, 2),
              ::core::option::Option::None => 5,
            };
            shift2 = 0;
            mask = u8::wrapping_sub(u8::wrapping_mul(4, sh), 1);
            swh_zero = sw;
          }
          TxClass::H => {
            shift = u8::wrapping_add(t_dim.lh, 2);
            shift2 = 0;
            mask = u8::wrapping_sub(u8::wrapping_mul(4, sh), 1);
            swh_zero = sh;
          }
          TxClass::V => {
            shift = u8::wrapping_add(t_dim.lw, 2);
            shift2 = u8::wrapping_add(t_dim.lh, 2);
            mask = u8::wrapping_sub(u8::wrapping_mul(4, sw), 1);
            swh_zero = sw;
          }
        }

        for i in 0..({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(stride);
          numeric_value
        })
        .wrapping_mul(
          (::core::num::Wrapping(4)
            * ::core::num::Wrapping({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(swh_zero);
              numeric_value
            }))
          .0
          .wrapping_add(2),
        ) {
          *{ let checked_value_13 = levels.get_mut(i).ok_or(EINVAL)?; checked_value_13 } = 0;
        }

        let mut rc;
        let mut x;
        let mut y;

        match tx_class {
          TxClass::TwoD => {
            rc = {
              let checked_value_14 = scan.get({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(eob);
                numeric_value
              }).ok_or(EINVAL)?;
              let Ok(explicit_value) = (*checked_value_14)
              .get();
              explicit_value
            };
            x = {
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped::<_, u8>(u16::wrapping_shr(rc, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(shift);
                  numeric_value
                }));
              numeric_value
            };
            y = ({
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(rc);
              numeric_value
            }) & mask;
          }
          TxClass::H => {
            x = ({
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(eob);
              numeric_value
            }) & mask;
            y = {
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped::<_, u8>(u16::wrapping_shr(eob, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(shift);
                  numeric_value
                }));
              numeric_value
            };
            rc = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(eob);
              numeric_value
            };
          }
          TxClass::V => {
            x = ({
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(eob);
              numeric_value
            }) & mask;
            y = {
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped::<_, u8>(u16::wrapping_shr(eob, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(shift);
                  numeric_value
                }));
              numeric_value
            };
            rc = u16::wrapping_shl(
              {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(x);
                numeric_value
              },
              {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(shift2);
                numeric_value
              },
            ) | ({
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(y);
              numeric_value
            });
          }
        }

        match dbg {
          BlockTrace::Enabled => {
            println!(
              "Post-lo_tok[{}][{}][{}][{}={}={}]: r={}",
              t_dim.ctx, chroma, ctx, eob, rc, tok, ts_c.msac.rng,
            );
          }
          BlockTrace::Quiet => {}
        }

        match ::core::primitive::bool::then_some(eob_tok == 2, ()) {
          ::core::option::Option::Some(()) => {
            ctx = match ::core::primitive::bool::then_some(match ::core::primitive::bool::then_some(tx_class == TxClass::TwoD, ()) {
              ::core::option::Option::Some(()) => (x | y) > 1,
              ::core::option::Option::None => y != 0,
            }, ()) {
              ::core::option::Option::Some(()) => 14,
              ::core::option::Option::None => 7,
            };
            tok = {
              let Ok(explicit_value) = rav1d_msac_decode_hi_tok(
                &mut ts_c.msac,
                hi_cdf.get_mut({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(ctx);
                  numeric_value
                }).ok_or(EINVAL)?,
              );
              explicit_value
            };
            level_tok = u8::wrapping_add(tok, u8::wrapping_shl(3, 6_u32));

            match dbg {
              BlockTrace::Enabled => {
                println!(
                  "Post-hi_tok[{}][{}][{}][{}={}={}]: r={}",
                  cmp::min(t_dim.ctx, 3),
                  chroma,
                  ctx,
                  eob,
                  rc,
                  tok,
                  ts_c.msac.rng,
                );
              }
              BlockTrace::Quiet => {}
            }
          }
          ::core::option::Option::None => {}
        }

        cf.set(rc, i16::wrapping_shl(i16::from(tok), 11_u32))?;

        match ::core::primitive::bool::then_some(tx_class == TxClass::TwoD, ()) {
          ::core::option::Option::Some(()) => {
            *{ let checked_value_15 = levels.get_mut({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(rc);
              numeric_value
            }).ok_or(EINVAL)?; checked_value_15 } = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(level_tok);
              numeric_value
            };
          }
          ::core::option::Option::None => {
            *{ let checked_value_16 = levels.get_mut(({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(x);
              numeric_value
            })
            .wrapping_mul({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(stride);
              numeric_value
            })
            .wrapping_add({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(y);
              numeric_value
            })).ok_or(EINVAL)?; checked_value_16 } = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(level_tok);
              numeric_value
            };
          }
        }

        for i in (1..eob).rev() {
          let rc_i;

          match tx_class {
            TxClass::TwoD => {
              rc_i = {
                let checked_value_17 = scan.get({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(i);
                  numeric_value
                }).ok_or(EINVAL)?;
                let Ok(explicit_value) = (*checked_value_17)
                .get();
                explicit_value
              };
              x = {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u8>(u16::wrapping_shr(rc_i, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(shift);
                    numeric_value
                  }));
                numeric_value
              };
              y = ({
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(rc_i);
                numeric_value
              }) & mask;
            }
            TxClass::H => {
              x = ({
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(i);
                numeric_value
              }) & mask;
              y = {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u8>(u16::wrapping_shr(i, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(shift);
                    numeric_value
                  }));
                numeric_value
              };
              rc_i = {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(i);
                numeric_value
              };
            }
            TxClass::V => {
              x = ({
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(i);
                numeric_value
              }) & mask;
              y = {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u8>(u16::wrapping_shr(i, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(shift);
                    numeric_value
                  }));
                numeric_value
              };
              rc_i = u16::wrapping_shl(
                {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(x);
                  numeric_value
                },
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(shift2);
                  numeric_value
                },
              ) | ({
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(y);
                numeric_value
              });
            }
          }

          debug_assert!(x < 32 && y < 32);
          x = ::core::primitive::u8::wrapping_rem(x, 32);
          y = ::core::primitive::u8::wrapping_rem(y, 32);

          let level = match ::core::primitive::bool::then_some(tx_class == TxClass::TwoD, ()) {
            ::core::option::Option::Some(()) => {
              levels.get_mut(({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(rc_i);
                numeric_value
              })..).ok_or(EINVAL)?
            }
            ::core::option::Option::None => {
              levels.get_mut(({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(x);
                numeric_value
              })
              .wrapping_mul({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(stride);
                numeric_value
              })
              .wrapping_add({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(y);
                numeric_value
              })..).ok_or(EINVAL)?
            }
          };
          ctx = {
            get_lo_ctx(level, tx_class, &mut mag, lo_ctx_offsets, x, y, stride)?
          };

          match ::core::primitive::bool::then_some(tx_class == TxClass::TwoD, ()) {
            ::core::option::Option::Some(()) => {
              y |= x;
            }
            ::core::option::Option::None => {}
          }

          tok = {
            let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt4(
              &mut ts_c.msac,
              lo_cdf.get_mut({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(ctx);
                numeric_value
              }).ok_or(EINVAL)?,
              3,
            );
            explicit_value
          };

          match dbg {
            BlockTrace::Enabled => {
              println!(
                "Post-lo_tok[{}][{}][{}][{}={}={}]: r={}",
                t_dim.ctx, chroma, ctx, i, rc_i, tok, ts_c.msac.rng,
              );
            }
            BlockTrace::Quiet => {}
          }

          match ::core::primitive::bool::then_some(tok == 3, ()) {
            ::core::option::Option::Some(()) => {
              let mag = ({
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(mag);
                numeric_value
              }) & 63;
              ctx = u8::wrapping_add(
                match ::core::primitive::bool::then_some(y > u8::from(tx_class == TxClass::TwoD), ()) {
                  ::core::option::Option::Some(()) => 14,
                  ::core::option::Option::None => 7,
                },
                match ::core::primitive::bool::then_some(mag > 12, ()) {
                  ::core::option::Option::Some(()) => 6,
                  ::core::option::Option::None => u8::wrapping_shr(u8::wrapping_add(mag, 1), 1_u32),
                },
              );
              tok = {
                let Ok(explicit_value) = rav1d_msac_decode_hi_tok(
                  &mut ts_c.msac,
                  hi_cdf.get_mut({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(ctx);
                    numeric_value
                  }).ok_or(EINVAL)?,
                );
                explicit_value
              };

              match dbg {
                BlockTrace::Enabled => {
                  println!(
                    "Post-hi_tok[{}][{}][{}][{}={}={}]: r={}",
                    cmp::min(t_dim.ctx, 3),
                    chroma,
                    ctx,
                    i,
                    rc_i,
                    tok,
                    ts_c.msac.rng,
                  );
                }
                BlockTrace::Quiet => {}
              }

              *{ let checked_value_18 = level.get_mut(0).ok_or(EINVAL)?; checked_value_18 } = {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(
                  u8::wrapping_add(tok, u8::wrapping_shl(3, 6_u32)),
                );
                numeric_value
              };
              cf.set(
                rc_i,
                u16::wrapping_shl(
                  {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u16>(tok);
                    numeric_value
                  },
                  11_u32,
                ) | rc,
              )?;
              rc = rc_i;
            }
            ::core::option::Option::None => {
              let tok = u32::wrapping_mul(
                {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(tok);
                  numeric_value
                },
                0x17ff41,
              );
              *{ let checked_value_19 = level.get_mut(0).ok_or(EINVAL)?; checked_value_19 } = {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(tok);
                numeric_value
              };

              let tok_check = match ::core::primitive::bool::then_some(tok != 0, ()) {
                ::core::option::Option::Some(()) => {
                  u16::wrapping_shl(
                    {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u16>(tok);
                      numeric_value
                    },
                    11_u32,
                  ) | rc
                }
                ::core::option::Option::None => 0,
              };

              let mask = u32::wrapping_shr(tok, 9_u32);
              let tok = mask
                & u32::wrapping_add(
                  {
                    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(rc);
                    numeric_value
                  },
                  !0x7ff,
                );
              let Ok(mask) = console_core_number_conversion::wrapped::<_, u16>(mask);
              rc = (rc_i & mask) | (rc & !mask);

              debug_assert!(
                tok
                  == ({
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(tok_check);
                    numeric_value
                  })
              );
              cf.set(rc_i, tok)?;
            }
          }
        }

        ctx = match ::core::primitive::bool::then_some(tx_class == TxClass::TwoD, ()) {
          ::core::option::Option::Some(()) => 0,
          ::core::option::Option::None => {
            get_lo_ctx(levels, tx_class, &mut mag, lo_ctx_offsets, 0, 0, stride)?
          }
        };
        let mut dc_tok = {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>({
            let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt4(
              &mut ts_c.msac,
              lo_cdf.get_mut({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(ctx);
                numeric_value
              }).ok_or(EINVAL)?,
              3,
            );
            explicit_value
          });
          numeric_value
        };

        match dbg {
          BlockTrace::Enabled => {
            println!(
              "Post-dc_lo_tok[{}][{}][{}][{}]: r={}",
              t_dim.ctx, chroma, ctx, dc_tok, ts_c.msac.rng,
            );
          }
          BlockTrace::Quiet => {}
        }

        match ::core::primitive::bool::then_some(dc_tok == 3, ()) {
          ::core::option::Option::Some(()) => {
            match ::core::primitive::bool::then_some(tx_class == TxClass::TwoD, ()) {
              ::core::option::Option::Some(()) => {
                mag = u32::wrapping_add(
                  u32::wrapping_add(
                    {
                      let checked_value_20 = levels.get((::core::num::Wrapping(0)
                          * ::core::num::Wrapping({
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_index(stride);
                            numeric_value
                          }))
                        .0
                        .wrapping_add(1)).ok_or(EINVAL)?;
                      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                        (*checked_value_20),
                      );
                      numeric_value
                    },
                    {
                      let checked_value_21 = levels.get((::core::num::Wrapping(1)
                          * ::core::num::Wrapping({
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_index(stride);
                            numeric_value
                          }))
                        .0
                        .wrapping_add(0)).ok_or(EINVAL)?;
                      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                        (*checked_value_21),
                      );
                      numeric_value
                    },
                  ),
                  {
                    let checked_value_22 = levels.get((::core::num::Wrapping(1)
                        * ::core::num::Wrapping({
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(stride);
                          numeric_value
                        }))
                      .0
                      .wrapping_add(1)).ok_or(EINVAL)?;
                    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                      (*checked_value_22),
                    );
                    numeric_value
                  },
                );
              }
              ::core::option::Option::None => {}
            }

            let mag = ({
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(mag);
              numeric_value
            }) & 63;
            ctx = match ::core::primitive::bool::then_some(mag > 12, ()) {
              ::core::option::Option::Some(()) => 6,
              ::core::option::Option::None => u8::wrapping_shr(u8::wrapping_add(mag, 1), 1_u32),
            };
            dc_tok = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>({
                let Ok(explicit_value) = rav1d_msac_decode_hi_tok(
                  &mut ts_c.msac,
                  hi_cdf.get_mut({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(ctx);
                    numeric_value
                  }).ok_or(EINVAL)?,
                );
                explicit_value
              });
              numeric_value
            };

            match dbg {
              BlockTrace::Enabled => {
                println!(
                  "Post-dc_hi_tok[{}][{}][0][{}]: r={}",
                  cmp::min(t_dim.ctx, 3),
                  chroma,
                  dc_tok,
                  ts_c.msac.rng,
                );
              }
              BlockTrace::Quiet => {}
            }
          }
          ::core::option::Option::None => {}
        }

        (rc, dc_tok)
      })
    }

    match ::core::primitive::bool::then_some(eob != 0, ()) {
      ::core::option::Option::Some(()) => {
        let cf = &mut cf;
        (rc, dc_tok) = match tx_class {
          TxClass::TwoD => {
            decode_coefs_class::<{ { let Ok(table_index) = console_core_number_conversion::const_u8_index({ let Ok(ordinal) = TxClass::TwoD.ordinal(); ordinal }); table_index } }, BD>(

              ts_c, t_dim, chroma, scratch, eob, tx, dbg, cf,
            )?
          }
          TxClass::H => {
            decode_coefs_class::<{ { let Ok(table_index) = console_core_number_conversion::const_u8_index({ let Ok(ordinal) = TxClass::H.ordinal(); ordinal }); table_index } }, BD>(

              ts_c, t_dim, chroma, scratch, eob, tx, dbg, cf,
            )?
          }
          TxClass::V => {
            decode_coefs_class::<{ { let Ok(table_index) = console_core_number_conversion::const_u8_index({ let Ok(ordinal) = TxClass::V.ordinal(); ordinal }); table_index } }, BD>(

              ts_c, t_dim, chroma, scratch, eob, tx, dbg, cf,
            )?
          }
        };
      }
      ::core::option::Option::None => {
        let eob_cdf = (match <[_]>::get_mut(&mut *ts_c.cdf.coef.eob_base_tok, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t_dim.ctx);
          numeric_value
        }) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        }).get_mut(chroma).ok_or(EINVAL)?;
        let hi_cdf = (match <[_]>::get_mut(&mut *ts_c.cdf.coef.br_tok, {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped_index(cmp::min(t_dim.ctx, 3));
          numeric_value
        }) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        }).get_mut(chroma).ok_or(EINVAL)?;

        let Ok(tok_br) = console_core_number_conversion::wrapped::<_, u32>({
          let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt4(
            &mut ts_c.msac,
            {
              let [_explicit_part, ..] = &mut *eob_cdf;
              _explicit_part
            },
            2,
          );
          explicit_value
        });
        dc_tok = u32::wrapping_add(1, tok_br);

        match dbg {
          BlockTrace::Enabled => {
            println!(
              "Post-dc_lo_tok[{}][{}][{}][{}]: r={}",
              t_dim.ctx, chroma, 0, dc_tok, ts_c.msac.rng,
            );
          }
          BlockTrace::Quiet => {}
        }

        match ::core::primitive::bool::then_some(tok_br == 2, ()) {
          ::core::option::Option::Some(()) => {
            dc_tok = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>({
                let Ok(explicit_value) = rav1d_msac_decode_hi_tok(&mut ts_c.msac, {
                  let [_explicit_part, ..] = &mut *hi_cdf;
                  _explicit_part
                });
                explicit_value
              });
              numeric_value
            };

            match dbg {
              BlockTrace::Enabled => {
                println!(
                  "Post-dc_hi_tok[{}][{}][0][{}]: r={}",
                  cmp::min(t_dim.ctx, 3),
                  chroma,
                  dc_tok,
                  ts_c.msac.rng,
                );
              }
              BlockTrace::Quiet => {}
            }
          }
          ::core::option::Option::None => {}
        }

        rc = 0;
      }
    }

    let Ok(get) = ts.dq.get();
    let dq = match get {
      TileStateRef::Frame => &f.dq,
      TileStateRef::Local => &ts.dqmem,
    };
    let Ok(segment) = b.seg_id.get();
    let Ok(plane_index) = console_core_number_conversion::index(plane);
    let Ok(segment_index) = console_core_number_conversion::index(segment);
    let dq_tbl = dq.get(segment_index).and_then(|table| table.get(plane_index)).ok_or(EINVAL)?;
    let qm_tbl = match ::core::primitive::bool::then_some(*txtp < IDTX, ()) {
      Some(()) => {
        let Ok(transform) = tx.ordinal();
        let Ok(transform_index) = console_core_number_conversion::index(transform);
        ::core::result::Result::<&_, _>::copied(f.qm.get(transform_index).and_then(|table| table.get(plane_index)).ok_or(EINVAL))?
      }
      None => None,
    };
    let dq_shift = cmp::max(
      0,
      i32::wrapping_sub(
        {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(t_dim.ctx);
          numeric_value
        },
        2,
      ),
    );
    let cf_max = {
      let Ok(numeric_value) =
        console_core_number_conversion::wrapped::<_, i32>(!u32::wrapping_shl(!127u32, {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, u32>(match BD::BPC {
              BPC::BPC8 => 8,
              BPC::BPC16 => f.cur.p.bpc,
            });
          numeric_value
        }));
      numeric_value
    };
    let mut cul_level: c_uint;
    let dc_sign_level: c_uint;

    enum Ac<'a> {
      Qm(&'a [u8]),
      NoQm,
    }

    let ac;

    match ::core::primitive::bool::then_some(dc_tok == 0, ()) {
      ::core::option::Option::Some(()) => {
        cul_level = 0;
        dc_sign_level = u32::wrapping_shl(1, 6_u32);
        ac = Some(match qm_tbl {
          Some(qm_tbl) => Ac::Qm(qm_tbl),
          None => Ac::NoQm,
        });
      }
      ::core::option::Option::None => {
        dc_sign_ctx = {
          let checked_value = get_dc_sign_ctx(tx, a, l)?;
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, i32>(checked_value);
          numeric_value
        };
        let dc_sign_cdf = (match <[_]>::get_mut(&mut *ts_c.cdf.coef.dc_sign, chroma) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        }).get_mut({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(dc_sign_ctx);
          numeric_value
        }).ok_or(EINVAL)?;
        dc_sign = i32::from(matches!(rav1d_msac_decode_bool_adapt(&mut ts_c.msac, dc_sign_cdf), Ok(EntropyBit::One)));

        match dbg {
          BlockTrace::Enabled => {
            println!(
              "Post-dc_sign[{}][{}][{}]: r={}",
              chroma, dc_sign_ctx, dc_sign, ts_c.msac.rng,
            );
          }
          BlockTrace::Quiet => {}
        }

        dc_dq = {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>({
            let Ok(explicit_value) = (*match &*dq_tbl {
              [element, ..] => element,
            })
            .get();
            explicit_value
          });
          numeric_value
        };
        dc_sign_level = {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
            i32::wrapping_sub(dc_sign, 1) & i32::wrapping_shl(2, 6_u32),
          );
          numeric_value
        };

        match qm_tbl {
          Some(qm_tbl) => {
            dc_dq = i32::wrapping_shr(
              i32::wrapping_add(
                i32::wrapping_mul(dc_dq, {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(
                    *match <[_]>::get(&*qm_tbl, 0) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => {
                        return ::core::result::Result::Err(
                          crate::src::error::Rav1dError::EINVAL,
                        );
                      }
                    },
                  );
                  numeric_value
                }),
                16,
              ),
              5_u32,
            );

            match ::core::primitive::bool::then_some(dc_tok == 15, ()) {
              ::core::option::Option::Some(()) => {
                dc_tok = ({
                  let Ok(explicit_value) = read_golomb(&mut ts_c.msac);
                  explicit_value
                })
                .wrapping_add(15);

                match dbg {
                  BlockTrace::Enabled => {
                    println!(
                      "Post-dc_residual[{}->{}]: r={}",
                      dc_tok.wrapping_sub(15),
                      dc_tok,
                      ts_c.msac.rng,
                    );
                  }
                  BlockTrace::Quiet => {}
                }

                dc_tok &= 0xfffff;
                dc_dq = {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(
                    ({
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u32>(dc_dq);
                      numeric_value
                    })
                    .wrapping_mul({
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u32>(dc_tok);
                      numeric_value
                    }) & 0xffffff,
                  );
                  numeric_value
                };
              }
              ::core::option::Option::None => {
                dc_dq = {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(
                    ({
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u32>(dc_dq);
                      numeric_value
                    })
                    .wrapping_mul({
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u32>(dc_tok);
                      numeric_value
                    }),
                  );
                  numeric_value
                };
                assert!(dc_dq <= 0xffffff);
              }
            }

            cul_level = dc_tok;

            {
              let numeric_amount = {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u32>(dq_shift);
                numeric_value
              };
              let numeric_target = &mut (dc_dq);
              *numeric_target = i32::wrapping_shr(*numeric_target, numeric_amount);
            };

            dc_dq = cmp::min(dc_dq, i32::wrapping_add(cf_max, dc_sign));
            cf.set(
              0,
              match ::core::primitive::bool::then_some(dc_sign != 0, ()) {
                ::core::option::Option::Some(()) => (dc_dq).wrapping_neg(),
                ::core::option::Option::None => dc_dq,
              },
            )?;

            ac = match ::core::primitive::bool::then_some(rc != 0, ()) {
              ::core::option::Option::Some(()) => Some(Ac::Qm(qm_tbl)),
              ::core::option::Option::None => None,
            };
          }
          None => {
            match ::core::primitive::bool::then_some(dc_tok == 15, ()) {
              ::core::option::Option::Some(()) => {
                dc_tok = ({
                  let Ok(explicit_value) = read_golomb(&mut ts_c.msac);
                  explicit_value
                })
                .wrapping_add(15);

                match dbg {
                  BlockTrace::Enabled => {
                    println!(
                      "Post-dc_residual[{}->{}]: r={}",
                      dc_tok.wrapping_sub(15),
                      dc_tok,
                      ts_c.msac.rng,
                    );
                  }
                  BlockTrace::Quiet => {}
                }

                dc_tok &= 0xfffff;
                dc_dq = {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(u32::wrapping_shr(
                      ({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>(dc_dq);
                        numeric_value
                      })
                      .wrapping_mul({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>(dc_tok);
                        numeric_value
                      }) & ({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>({
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, i32>(0xffffff);
                            numeric_value
                          });
                        numeric_value
                      }),
                      {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>(dq_shift);
                        numeric_value
                      },
                    ));
                  numeric_value
                };
                dc_dq = cmp::min(dc_dq, i32::wrapping_add(cf_max, dc_sign));
              }
              ::core::option::Option::None => {
                dc_dq = {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(u32::wrapping_shr(
                      ({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>(dc_dq);
                        numeric_value
                      })
                      .wrapping_mul({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>(dc_tok);
                        numeric_value
                      }),
                      {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>(dq_shift);
                        numeric_value
                      },
                    ));
                  numeric_value
                };
                assert!(dc_dq <= cf_max);
              }
            }

            cul_level = dc_tok;
            cf.set(
              0,
              match ::core::primitive::bool::then_some(dc_sign != 0, ()) {
                ::core::option::Option::Some(()) => (dc_dq).wrapping_neg(),
                ::core::option::Option::None => dc_dq,
              },
            )?;

            ac = match ::core::primitive::bool::then_some(rc != 0, ()) {
              ::core::option::Option::Some(()) => Some(Ac::NoQm),
              ::core::option::Option::None => None,
            };
          }
        }
      }
    }

    match ac {
      Some(Ac::Qm(qm_tbl)) => {
        let ac_dq: c_uint = {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>({
            let Ok(explicit_value) = (*match &*dq_tbl {
              [_, element, ..] => element,
            })
            .get();
            explicit_value
          });
          numeric_value
        };

        loop {
          let sign = matches!(rav1d_msac_decode_bool_equi(&mut ts_c.msac), Ok(EntropyBit::One));

          match dbg {
            BlockTrace::Enabled => {
              println!("Post-sign[{}={}]: r={}", rc, sign, ts_c.msac.rng);
            }
            BlockTrace::Quiet => {}
          }

          let Ok(rc_tok) = console_core_number_conversion::wrapped::<_, u32>({
            let explicit_value = cf.get(rc)?;
            explicit_value
          });
          let mut tok;
          let mut dq: c_uint = u32::wrapping_shr(
            ac_dq
              .wrapping_mul({
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                  *match <[_]>::get(&*qm_tbl, {
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(rc);
                    numeric_value
                  }) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => {
                      return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                    }
                  },
                );
                numeric_value
              })
              .wrapping_add(16),
            5_u32,
          );
          let dq_sat;

          match ::core::primitive::bool::then_some(rc_tok >= u32::wrapping_shl(15, 11_u32), ()) {
            ::core::option::Option::Some(()) => {
              tok = ({
                let Ok(explicit_value) = read_golomb(&mut ts_c.msac);
                explicit_value
              })
              .wrapping_add(15);

              match dbg {
                BlockTrace::Enabled => {
                  println!(
                    "Post-residual[{}={}->{}]: r={}",
                    rc,
                    tok.wrapping_sub(15),
                    tok,
                    ts_c.msac.rng,
                  );
                }
                BlockTrace::Quiet => {}
              }

              tok &= 0xfffff;
              dq = dq.wrapping_mul(tok) & 0xffffff;
            }
            ::core::option::Option::None => {
              tok = u32::wrapping_shr(rc_tok, 11_u32);
              dq = dq.wrapping_mul(tok);
              assert!(dq <= 0xffffff);
            }
          }

          cul_level = cul_level.wrapping_add(tok);

          {
            let numeric_amount = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(dq_shift);
              numeric_value
            };
            let numeric_target = &mut (dq);
            *numeric_target = u32::wrapping_shr(*numeric_target, numeric_amount);
          };

          dq_sat = cmp::min(
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(dq);
              numeric_value
            },
            i32::wrapping_add(cf_max, i32::from(sign)),
          );
          cf.set(
            rc,
            match ::core::primitive::bool::then_some(sign, ()) {
              ::core::option::Option::Some(()) => (dq_sat).wrapping_neg(),
              ::core::option::Option::None => dq_sat,
            },
          )?;

          rc = ({
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(rc_tok);
            numeric_value
          }) & 0x3ff;

          match ::core::primitive::bool::then_some(!(rc != 0), ()) {
            ::core::option::Option::Some(()) => {
              break;
            }
            ::core::option::Option::None => {}
          }
        }
      }
      Some(Ac::NoQm) => {
        let ac_dq: c_uint = {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>({
            let Ok(explicit_value) = (*match &*dq_tbl {
              [_, element, ..] => element,
            })
            .get();
            explicit_value
          });
          numeric_value
        };

        loop {
          let sign = i32::from(matches!(rav1d_msac_decode_bool_equi(&mut ts_c.msac), Ok(EntropyBit::One)));

          match dbg {
            BlockTrace::Enabled => {
              println!("Post-sign[{}={}]: r={}", rc, sign, ts_c.msac.rng);
            }
            BlockTrace::Quiet => {}
          }

          let Ok(rc_tok) = console_core_number_conversion::wrapped::<_, u32>({
            let explicit_value = cf.get(rc)?;
            explicit_value
          });
          let mut tok: c_uint;
          let mut dq;

          match ::core::primitive::bool::then_some(rc_tok >= u32::wrapping_shl(15, 11_u32), ()) {
            ::core::option::Option::Some(()) => {
              tok = ({
                let Ok(explicit_value) = read_golomb(&mut ts_c.msac);
                explicit_value
              })
              .wrapping_add(15);

              match dbg {
                BlockTrace::Enabled => {
                  println!(
                    "Post-residual[{}={}->{}]: r={}",
                    rc,
                    tok.wrapping_sub(15),
                    tok,
                    ts_c.msac.rng,
                  );
                }
                BlockTrace::Quiet => {}
              }

              tok &= 0xfffff;

              dq = {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(
                  u32::wrapping_shr(ac_dq.wrapping_mul(tok) & 0xffffff, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(dq_shift);
                    numeric_value
                  }),
                );
                numeric_value
              };
              dq = cmp::min(dq, i32::wrapping_add(cf_max, sign));
            }
            ::core::option::Option::None => {
              tok = u32::wrapping_shr(rc_tok, 11_u32);
              dq = {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(
                  u32::wrapping_shr(ac_dq.wrapping_mul(tok), {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(dq_shift);
                    numeric_value
                  }),
                );
                numeric_value
              };
              assert!(dq <= cf_max);
            }
          }

          cul_level = cul_level.wrapping_add(tok);
          cf.set(
            rc,
            match ::core::primitive::bool::then_some(sign != 0, ()) {
              ::core::option::Option::Some(()) => (dq).wrapping_neg(),
              ::core::option::Option::None => dq,
            },
          )?;

          rc = ({
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(rc_tok);
            numeric_value
          }) & 0x3ff;

          match ::core::primitive::bool::then_some(!(rc != 0), ()) {
            ::core::option::Option::Some(()) => {
              break;
            }
            ::core::option::Option::None => {}
          }
        }
      }
      None => {}
    }

    *res_ctx = {
      let Ok(numeric_value) =
        console_core_number_conversion::wrapped::<_, u8>(cmp::min(cul_level, 63) | dc_sign_level);
      numeric_value
    };

    {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(eob);
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
    let bit_index = self.split_cell.y.wrapping_mul(4).wrapping_add(self.split_cell.x);
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
        let y = i64::from(offset.y).wrapping_mul(i64::from(y_step)).wrapping_mul(pitch);
        let pixels = x.wrapping_add(y).wrapping_mul(4);
        let Ok(displacement) = console_core_number_conversion::wrapped_offset(pixels);
        dst + displacement
      });
      Self { position, transform: dimensions.sub, split_cell, output }
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
    position: t.b, transform: tree.transform, split_cell: tree.split_cell, output: tree.output,
  };
  let frame = Size { width: f.bw, height: f.bh };
  let [root_split, child_split] = tree.splits;
  let children = root.children::<BD>(frame, root_split)?;

  match children {
    Some(children) => {
      for child in children.into_iter().flatten() {
        let grandchildren = child.children::<BD>(frame, child_split)?;

        match grandchildren {
          Some(grandchildren) => {
            for leaf in grandchildren.into_iter().flatten() {
              read_coef_leaf::<BD>(f, t, ts_c.as_deref_mut(), &tree, leaf)?;
            }
          }
          None => read_coef_leaf::<BD>(f, t, ts_c.as_deref_mut(), &tree, child)?,
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
  let tile = f.tile(t.tile_index).ok_or(EINVAL)?;
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
      let Ok(offset) = pass.cf.get_update(|position| position.wrapping_add(coefficient_count));
      CfSelect::Frame(offset)
    }
    None => CfSelect::Task,
  };

  let (eob, kind) = match t.frame_thread.pass {
    2 => {
      let Ok(record) = reconstruct.cbi_idx.get_update(|position| position.wrapping_add(1));
      let Ok(index) = console_core_number_conversion::index(record);
      let info_slot = f.frame_thread.cbi.get(index).ok_or(EINVAL)?;
      let Ok(info) = info_slot.get();
      let Ok(eob) = info.eob();
      let Ok(kind) = info.txtp();
      (i32::from(eob), kind)
    }
    0 | 1 => {
      let tile_context = ts_c.ok_or(EINVAL)?;
      let above = f.above_context(t.above_index).ok_or(EINVAL)?;
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
      let trace = debug_block_info!(f, t.b);
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
        CoefficientOutput { transform: &mut kind, residual_context: &mut residual_context },
      )?;

      match trace {
        BlockTrace::Enabled => {
          console_core_logging::trace!(
            "Post-y-cf-blk[tx={transform:?},txtp={kind},eob={eob}]: r={}",
            tile_context.msac.rng,
          );
        }
        BlockTrace::Quiet => {}
      }

      let columns = i32::from(dim.w).min(f.bw.wrapping_sub(t.b.x));
      let rows = i32::from(dim.h).min(f.bh.wrapping_sub(t.b.y));
      let above_len = console_core_number_conversion::checked_index(columns).map_err(|_invalid_width| EINVAL)?;
      let left_len = console_core_number_conversion::checked_index(rows).map_err(|_invalid_height| EINVAL)?;
      let above_context = above_guard.get_mut(..above_len).ok_or(EINVAL)?;
      let left_context = left.get_mut(..left_len).ok_or(EINVAL)?;
      let Ok(()) = small_memset::<u8, 16, true>(above_context, residual_context);
      let Ok(()) = small_memset::<u8, 16, true>(left_context, residual_context);
      drop(above_guard);

      let scratch = t.scratch.inter_intra_mut()?;
      let transform_map = scratch.ac_txtp_map.txtp_map_mut()?;
      let Ok(start) = console_core_number_conversion::index(y.wrapping_mul(32).wrapping_add(x));
      let rows = transform_map.get_mut(start..).ok_or(EINVAL)?;
      let Ok(transform_rows) = console_core_number_conversion::index(height);
      let Ok(()) = CaseSet::<16, false>::one(rows, ContextRun { offset: 0, length: u64::from(width) }, |case, rows| {
        for row in rows.chunks_mut(32).take(transform_rows) {
          let Ok(()) = case.set(row, kind);
        }
      });

      match t.frame_thread.pass {
        1 => {
          let Ok(record) = entropy.cbi_idx.get_update(|position| position.wrapping_add(1));
          let Ok(index) = console_core_number_conversion::index(record);
          let info_slot = f.frame_thread.cbi.get(index).ok_or(EINVAL)?;
          let Ok(encoded_eob) = console_core_number_conversion::wrapped::<_, i16>(eob);
          let Ok(info) = CodedBlockInfo::new(encoded_eob, kind);
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
      let Ok(element_width) = console_core_number_conversion::fixed_index(core::mem::size_of::<BD::Coef>());
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
  let trace = debug_block_info!(f, t.b);

  match (trace, PIXEL_TRACE) {
    (BlockTrace::Enabled, BlockTrace::Enabled) => {
      let coded = Size {
        width: u64::from(height.min(8)).wrapping_mul(4),
        height: u64::from(width.min(8)).wrapping_mul(4),
      };
      coef_dump(coefficients, coded, 3, "dq").map_err(|_dump_error| EIO)?;
    }
    (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
  }

  let bd = BD::from_c(f.bitdepth_max);
  let mut output = crate::src::owned_recon::ReconDst::Pic(destination);
  f.dsp.itx.add::<BD>(transform, kind, &mut output, coefficients, eob, bd)?;

  match (trace, PIXEL_TRACE) {
    (BlockTrace::Enabled, BlockTrace::Enabled) => {
      let extent = Size { width: u64::from(width).wrapping_mul(4), height: u64::from(height).wrapping_mul(4) };
      hex_dump_pic::<BD>(&output, extent, "recon").map_err(|_dump_error| EIO)?;
    }
    (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
  }

  Ok(())
}

pub(crate) fn rav1d_read_coef_blocks<BD: BitDepth>(
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
  ts_c: &mut Rav1dTileStateContext,
  bs: BlockSize,
  b: &Av1Block,
) -> Rav1dResult {
  {
    let above_context = f.above_context(t.above_index).ok_or(EINVAL)?;
    let ss_ver = u8::from(f.cur.p.layout == Rav1dPixelLayout::I420);
    let ss_hor = u8::from(f.cur.p.layout != Rav1dPixelLayout::I444);
    let bx4 = ({
      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t.b.x);
      numeric_value
    }) & 31;
    let by4 = ({
      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t.b.y);
      numeric_value
    }) & 31;
    let cbx4 = bx4.wrapping_shr({
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
      numeric_value
    });
    let cby4 = by4.wrapping_shr({
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
      numeric_value
    });
    let Ok(b_dim) = bs.dimensions();
    let &[bw4, ..] = b_dim;
    let &[_, bh4, ..] = b_dim;
    let cbw4 = u8::wrapping_shr(u8::wrapping_add(bw4, ss_hor), {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
      numeric_value
    });
    let cbh4 = u8::wrapping_shr(u8::wrapping_add(bh4, ss_ver), {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
      numeric_value
    });
    let has_chroma = f.cur.p.layout != Rav1dPixelLayout::I400
      && (bw4 > ss_hor || t.b.x & 1 != 0)
      && (bh4 > ss_ver || t.b.y & 1 != 0);

    match ::core::primitive::bool::then_some(b.skip != 0, ()) {
      ::core::option::Option::Some(()) => {
        case_set_al! {
            <32, false>
            l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4); numeric_value }, by4),
            a: (above_context, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4); numeric_value }, bx4),
            lcoef = (0x40, 0x40),
        }

        match ::core::primitive::bool::then_some(has_chroma, ()) {
          ::core::option::Option::Some(()) => {
            case_set_al! {
                <32, false>
                l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbh4); numeric_value }, cby4),
                a: (above_context, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbw4); numeric_value }, cbx4),
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

    let ts = &*match f.tile(t.tile_index) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
      }
    };
    let Ok(w4) = console_core_number_conversion::wrapped::<_, u8>(cmp::min(
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(bw4);
        numeric_value
      },
      i32::wrapping_sub(f.bw, t.b.x),
    ));
    let Ok(h4) = console_core_number_conversion::wrapped::<_, u8>(cmp::min(
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(bh4);
        numeric_value
      },
      i32::wrapping_sub(f.bh, t.b.y),
    ));
    let cw4 = u8::wrapping_shr(u8::wrapping_add(w4, ss_hor), {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
      numeric_value
    });
    let ch4 = u8::wrapping_shr(u8::wrapping_add(h4, ss_ver), {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
      numeric_value
    });
    assert!(t.frame_thread.pass == 1);
    assert!(b.skip == 0);
    let uv_t_dim = &*match <[_]>::get(&dav1d_txfm_dimensions, { let Ok(table_index) = console_core_number_conversion::index({ let Ok(ordinal) = b.uvtx.ordinal(); ordinal }); table_index }) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
      }
    };
    let t_dim = &*match <[_]>::get(
      &dav1d_txfm_dimensions,
      { let Ok(table_index) = console_core_number_conversion::index({ let Ok(ordinal) = (match &b.ii {
        Av1BlockIntraInter::Intra(intra) => intra.tx,
        Av1BlockIntraInter::Inter(inter) => inter.max_ytx,
      }).ordinal(); ordinal }); table_index },
    ) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
      }
    };

    for init_y in (0..h4).step_by(16) {
      let sub_h4 = cmp::min(h4, u8::wrapping_add(16, init_y));

      'over_init_x_2: for init_x in (0..w4).step_by(16) {
        let sub_w4 = cmp::min(w4, u8::wrapping_add(init_x, 16));
        let mut y_off = i32::from(init_y != 0);
        let mut y;
        let mut x;
        y = init_y;

        {
          let numeric_amount = {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(init_y);
            numeric_value
          };
          let numeric_target = &mut (t.b.y);
          *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
        };

        while y < sub_h4 {
          let mut x_off = i32::from(init_x != 0);
          x = init_x;

          {
            let numeric_amount = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(init_x);
              numeric_value
            };
            let numeric_target = &mut (t.b.x);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };

          while x < sub_w4 {
            match &b.ii {
              Av1BlockIntraInter::Inter(inter) => {
                let tx_split = [
                  ({
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u16>(inter.tx_split0);
                    numeric_value
                  }),
                  inter.tx_split1,
                ];
                read_coef_tree::<BD>(
                  f,
                  t,
                  Some(ts_c),
                  LumaCoefficientTree {
                    size: bs,
                    syntax: b,
                    transform: inter.max_ytx,
                    splits: tx_split,
                    split_cell: Point { x: x_off, y: y_off },
                    output: None,
                  },
                )?;
              }
              Av1BlockIntraInter::Intra(intra) => {
                let mut cf_ctx = 0x40;
                let mut txtp = DCT_DCT;
                let ::core::result::Result::Ok(a_start) =
                  ::console_core_number_conversion::fixed_index(bx4.wrapping_add({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(x);
                    numeric_value
                  }));
                let Ok(a_len) = console_core_number_conversion::wrapped_index(t_dim.w);
                let l_start = by4.wrapping_add({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(y);
                  numeric_value
                });
                let Ok(l_len) = console_core_number_conversion::wrapped_index(t_dim.h);

                let cf = CfSelect::Frame({
                  let Ok(explicit_value) = (*match &ts.frame_thread {
                    [_, element, ..] => element,
                  })
                  .cf
                  .get_update(|i| {
                    u32::wrapping_add(
                      i,
                      u32::wrapping_mul(
                        u32::wrapping_mul(
                          {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(cmp::min(
                                t_dim.w, 8,
                              ));
                            numeric_value
                          },
                          {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(cmp::min(
                                t_dim.h, 8,
                              ));
                            numeric_value
                          },
                        ),
                        16,
                      ),
                    )
                  });
                  explicit_value
                });
                let eob = {
                  decode_coefs::<BD>(
                    f,
                    CoefficientDecoder {
                      tile: ts,
                      state: ts_c,
                      scratch: &mut t.scratch,
                      coefficients: &mut t.cf,
                    },
                    CoefficientBlock {
                      trace: debug_block_info!(f, t.b),
                      above: &(*match f.above_context(t.above_index) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      })
                      .lcoef
                      .index((
                        match ::console_core_number_conversion::index(a_start) {
                          ::core::result::Result::Ok(native_position) => native_position,
                        }..,
                        ..a_len,
                      )),
                      left: &*match <[_]>::get(
                        &*match <[_]>::get(&**t.l.lcoef.get_mut(), l_start..) {
                          ::core::option::Option::Some(indexed_value_3) => indexed_value_3,
                          ::core::option::Option::None => {
                            return ::core::result::Result::Err(
                              crate::src::error::Rav1dError::EINVAL,
                            );
                          }
                        },
                        ..l_len,
                      ) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      },
                      transform: intra.tx,
                      size: bs,
                      syntax: b,
                      plane: 0,
                      storage: cf,
                    },
                    CoefficientOutput {
                      transform: &mut txtp,
                      residual_context: &mut cf_ctx,
                    },
                  )?
                };

                match debug_block_info!(f, t.b) {
                  BlockTrace::Enabled => {
                    println!(
                      "Post-y-cf-blk[tx={:?},txtp={},eob={}]: r={}",
                      intra.tx, txtp, eob, ts_c.msac.rng,
                    );
                  }
                  BlockTrace::Quiet => {}
                }

                let Ok(cbi_idx) = (*match &ts.frame_thread {
                  [_, element, ..] => element,
                })
                .cbi_idx
                .get_update(|i| u32::wrapping_add(i, 1));
                ({
                  let Ok(explicit_value) = (*match <[_]>::get(&f.frame_thread.cbi, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped_index(cbi_idx);
                    numeric_value
                  }) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => {
                      return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                    }
                  })
                  .set({
                    let Ok(explicit_value) = CodedBlockInfo::new(
                      {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, i16>(eob);
                        numeric_value
                      },
                      txtp,
                    );
                    explicit_value
                  });
                  explicit_value
                });
                case_set_al! {
                    <16, true>
                    l: (
                        &mut t.l,
                        { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cmp::min({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(t_dim.h); numeric_value }, i32::wrapping_sub(f.bh, t.b.y))); numeric_value },
                        by4.wrapping_add({ let Ok(numeric_value) = console_core_number_conversion::wrapped_index(y); numeric_value })
                    ),
                    a: (
                        above_context,
                        { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cmp::min({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(t_dim.w); numeric_value }, i32::wrapping_sub(f.bw, t.b.x))); numeric_value },
                        bx4.wrapping_add({ let Ok(numeric_value) = console_core_number_conversion::wrapped_index(x); numeric_value })
                    ),
                    lcoef = (cf_ctx, cf_ctx),
                }
              }
            }

            {
              let numeric_amount = t_dim.w;
              let numeric_target = &mut (x);
              *numeric_target = u8::wrapping_add(*numeric_target, numeric_amount);
            };

            {
              let numeric_amount = {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(t_dim.w);
                numeric_value
              };
              let numeric_target = &mut (t.b.x);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };

            {
              let numeric_amount = 1;
              let numeric_target = &mut (x_off);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };
          }

          {
            let numeric_amount = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(x);
              numeric_value
            };
            let numeric_target = &mut (t.b.x);
            *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
          };

          {
            let numeric_amount = t_dim.h;
            let numeric_target = &mut (y);
            *numeric_target = u8::wrapping_add(*numeric_target, numeric_amount);
          };

          {
            let numeric_amount = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(t_dim.h);
              numeric_value
            };
            let numeric_target = &mut (t.b.y);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };

          {
            let numeric_amount = 1;
            let numeric_target = &mut (y_off);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };
        }

        {
          let numeric_amount = {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(y);
            numeric_value
          };
          let numeric_target = &mut (t.b.y);
          *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
        };

        match ::core::primitive::bool::then_some(!has_chroma, ()) {
          ::core::option::Option::Some(()) => {
            continue 'over_init_x_2;
          }
          ::core::option::Option::None => {}
        }

        let sub_ch4 = cmp::min(
          ch4,
          u8::wrapping_shr(u8::wrapping_add(init_y, 16), {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
            numeric_value
          }),
        );
        let sub_cw4 = cmp::min(
          cw4,
          u8::wrapping_shr(u8::wrapping_add(init_x, 16), {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
            numeric_value
          }),
        );
        let mut pl = 0;

        while pl < 2 {
          y = u8::wrapping_shr(init_y, {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
            numeric_value
          });

          {
            let numeric_amount = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(init_y);
              numeric_value
            };
            let numeric_target = &mut (t.b.y);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };

          while y < sub_ch4 {
            x = u8::wrapping_shr(init_x, {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
              numeric_value
            });

            {
              let numeric_amount = {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(init_x);
                numeric_value
              };
              let numeric_target = &mut (t.b.x);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };

            while x < sub_cw4 {
              let mut cf_ctx = 0x40;
              let mut txtp = match b.ii {
                Av1BlockIntraInter::Intra(_) => DCT_DCT,
                Av1BlockIntraInter::Inter(_) => *match <[_]>::get(
                  &*({
                    let explicit_value = ({
                      let explicit_value = t.scratch.inter_intra()?;
                      explicit_value
                    })
                    .ac_txtp_map
                    .txtp_map()?;
                    explicit_value
                  }),
                  ({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4);
                    numeric_value
                  })
                  .wrapping_add({
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped_index(u8::wrapping_shl(y, {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                        numeric_value
                      }));
                    numeric_value
                  })
                  .wrapping_mul(32)
                  .wrapping_add(bx4)
                  .wrapping_add({
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped_index(u8::wrapping_shl(x, {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                        numeric_value
                      }));
                    numeric_value
                  }),
                ) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                },
              };
              let ::core::result::Result::Ok(a_start) =
                ::console_core_number_conversion::fixed_index(cbx4.wrapping_add({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(x);
                  numeric_value
                }));
              let Ok(a_len) = console_core_number_conversion::wrapped_index(uv_t_dim.w);
              let a_ccoef = &*match <[_]>::get(
                &(*match f.above_context(t.above_index) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                })
                .ccoef,
                pl,
              ) {
                ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              };
              let l_start = cby4.wrapping_add({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(y);
                numeric_value
              });
              let Ok(l_len) = console_core_number_conversion::wrapped_index(uv_t_dim.h);
              let l_ccoef = &mut *match <[_]>::get_mut(&mut t.l.ccoef, pl) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              };
              let Ok(cf_idx) = (*match &ts.frame_thread {
                [_, element, ..] => element,
              })
              .cf
              .get();
              let eob = {
                decode_coefs::<BD>(
                  f,
                  CoefficientDecoder {
                    tile: ts,
                    state: ts_c,
                    scratch: &mut t.scratch,
                    coefficients: &mut t.cf,
                  },
                  CoefficientBlock {
                    trace: debug_block_info!(f, t.b),
                    above: &a_ccoef.index((
                      match ::console_core_number_conversion::index(a_start) {
                        ::core::result::Result::Ok(native_position) => native_position,
                      }..,
                      ..a_len,
                    )),
                    left: &*match <[_]>::get(
                      &*match <[_]>::get(&**l_ccoef.get_mut(), l_start..) {
                        ::core::option::Option::Some(indexed_value_3) => indexed_value_3,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      },
                      ..l_len,
                    ) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => {
                        return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                      }
                    },
                    transform: b.uvtx,
                    size: bs,
                    syntax: b,
                    plane: { let checked_value_23 = u8::try_from(pl).map_err(|_invalid_plane| EINVAL)?; checked_value_23 }.wrapping_add(1),
                    storage: CfSelect::Frame(cf_idx),
                  },
                  CoefficientOutput {
                    transform: &mut txtp,
                    residual_context: &mut cf_ctx,
                  },
                )?
              };

              match debug_block_info!(f, t.b) {
                BlockTrace::Enabled => {
                  println!(
                    "Post-uv-cf-blk[pl={},tx={:?},txtp={},eob={}]: r={}",
                    pl, b.uvtx, txtp, eob, ts_c.msac.rng,
                  );
                }
                BlockTrace::Quiet => {}
              }

              let Ok(cbi_idx) = (*match &ts.frame_thread {
                [_, element, ..] => element,
              })
              .cbi_idx
              .get_update(|i| u32::wrapping_add(i, 1));
              ({
                let Ok(explicit_value) = (*match <[_]>::get(&f.frame_thread.cbi, {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbi_idx);
                  numeric_value
                }) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                })
                .set({
                  let Ok(explicit_value) = CodedBlockInfo::new(
                    {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, i16>(eob);
                      numeric_value
                    },
                    txtp,
                  );
                  explicit_value
                });
                explicit_value
              });
              ({
                let Ok(explicit_value) = (*match &ts.frame_thread {
                  [_, element, ..] => element,
                })
                .cf
                .set(u32::wrapping_add(
                  cf_idx,
                  u32::wrapping_mul(
                    u32::wrapping_mul(
                      {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>(uv_t_dim.w);
                        numeric_value
                      },
                      {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>(uv_t_dim.h);
                        numeric_value
                      },
                    ),
                    16,
                  ),
                ));
                explicit_value
              });
              case_set_al! {
                  <16, true>
                  l: (
                      &mut t.l,
                      { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cmp::min(
                          { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h); numeric_value },
                          i32::wrapping_shr(i32::wrapping_add(i32::wrapping_sub(f.bh, t.b.y), { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(ss_ver); numeric_value }), { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver); numeric_value }),
                      )); numeric_value },
                      cby4.wrapping_add({ let Ok(numeric_value) = console_core_number_conversion::wrapped_index(y); numeric_value })
                  ),
                  a: (
                      above_context,
                      { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cmp::min(
                          { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w); numeric_value },
                          i32::wrapping_shr(i32::wrapping_add(i32::wrapping_sub(f.bw, t.b.x), { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(ss_hor); numeric_value }), { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor); numeric_value }),
                      )); numeric_value },
                      ({ let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbx4); numeric_value }).wrapping_add({ let Ok(numeric_value) = console_core_number_conversion::wrapped_index(x); numeric_value })
                  ),
                  ccoef[pl] = (cf_ctx, cf_ctx),
              }

              {
                let numeric_amount = uv_t_dim.w;
                let numeric_target = &mut (x);
                *numeric_target = u8::wrapping_add(*numeric_target, numeric_amount);
              };

              {
                let numeric_amount = i32::wrapping_shl(
                  {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w);
                    numeric_value
                  },
                  {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                    numeric_value
                  },
                );
                let numeric_target = &mut (t.b.x);
                *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
              };
            }

            {
              let numeric_amount = {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(u8::wrapping_shl(x, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                    numeric_value
                  }));
                numeric_value
              };
              let numeric_target = &mut (t.b.x);
              *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
            };

            {
              let numeric_amount = uv_t_dim.h;
              let numeric_target = &mut (y);
              *numeric_target = u8::wrapping_add(*numeric_target, numeric_amount);
            };

            {
              let numeric_amount = i32::wrapping_shl(
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h);
                  numeric_value
                },
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                  numeric_value
                },
              );
              let numeric_target = &mut (t.b.y);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };
          }

          {
            let numeric_amount = {
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped::<_, i32>(u8::wrapping_shl(y, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                  numeric_value
                }));
              numeric_value
            };
            let numeric_target = &mut (t.b.y);
            *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
          };

          let ::core::result::Result::Ok(()) =
            ::console_core_number_conversion::wrapping_add_index(1, &mut pl);
        }
      }
    }
  };

  Ok(())
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
  Ok(PicOffset { data: component, offset: offset_2 })
}

#[derive(Clone, Copy)]
enum ReferenceSampling {
  Direct,
  Scaled,
}

fn mc<BD: BitDepth>(
  f: &Rav1dFrameData,
  emu_edge: &mut ScratchEmuEdge,
  dst: MaybeTempPixels<()>,
  prediction: MotionPrediction<'_>,
) -> Result<(), ()> {
  let MotionPrediction { position, size, origin, plane, motion, reference, reference_index, filter } = prediction;
  let bd = BD::from_c(f.bitdepth_max);
  let ref_data = &reference.p.data.as_ref().ok_or(())?.data;
  let cur_data = &f.cur.data.as_ref().ok_or(())?.data;
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
  let mode = match (reference.p.p.w.cmp(&f.cur.p.w), reference.p.p.h.cmp(&f.cur.p.h)) {
    (cmp::Ordering::Equal, cmp::Ordering::Equal) => ReferenceSampling::Direct,
    _ => ReferenceSampling::Scaled,
  };
  let mv = Point { x: i32::from(motion.x), y: i32::from(motion.y) };

  match mode {
    ReferenceSampling::Direct => {
      let [first_reference, ..] = ref_data;
      let [first_current, ..] = cur_data;
      let visible = match first_reference.ref_eq(first_current).then_some(()) {
        Some(()) => Size { width: f.bw.wrapping_mul(4), height: f.bh.wrapping_mul(4) },
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
        x: start.x.wrapping_add(mv.x.wrapping_shr(3_u32.wrapping_add(subsample.x))),
        y: start.y.wrapping_add(mv.y.wrapping_shr(3_u32.wrapping_add(subsample.y))),
      };
      let phase = Point {
        x: mv.x & 15_i32.wrapping_shr(u32::from(subsample.x == 0)),
        y: mv.y & 15_i32.wrapping_shr(u32::from(subsample.y == 0)),
      };
      let taps = Point { x: i32::from(phase.x != 0), y: i32::from(phase.y != 0) };
      let lower = Point { x: taps.x.wrapping_mul(3), y: taps.y.wrapping_mul(3) };
      let upper = Point { x: taps.x.wrapping_mul(4), y: taps.y.wrapping_mul(4) };
      let outside = source.x < lower.x || source.y < lower.y
        || source.x.wrapping_add(block.width).wrapping_add(upper.x) > extent.width
        || source.y.wrapping_add(block.height).wrapping_add(upper.y) > extent.height;
      let edge_component;
      let input = match outside.then_some(()) {
        Some(()) => {
          const STRIDE: u32 = 192;
          let buffer = emu_edge.buf_mut::<BD>().map_err(|_invalid_scratch| ())?;
          let window = crate::src::mc::EdgeWindow {
            block: Size {
              width: block.width.wrapping_add(taps.x.wrapping_mul(7)),
              height: block.height.wrapping_add(taps.y.wrapping_mul(7)),
            },
            reference: extent,
            origin: Point { x: source.x.wrapping_sub(lower.x), y: source.y.wrapping_sub(lower.y) },
          };
          f.dsp.mc.emu_edge.call::<BD>(window, buffer, STRIDE, component).map_err(|_edge_layout| ())?;
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
          let mc = f.dsp.mc.mc.checked(filter).map_err(|_invalid_dispatch| ())?;
          mc.call::<BD>(filter, dst_2, input, sampling, bd)
        }
        MaybeTempPixels::Temp { tmp, .. } => {
          let mct = f.dsp.mc.mct.checked(filter).map_err(|_invalid_dispatch| ())?;
          mct.call::<BD>(filter, tmp, input, sampling, bd)
        }
      };
      result.map_err(|_invalid_motion| ())
    }
    ReferenceSampling::Scaled => {
      match ptr::eq(reference, &f.sr_cur).then_some(()) {
        Some(()) => return Err(()),
        None => {}
      }

      let origin = Point {
        x: start.x.wrapping_shl(4).wrapping_add(mv.x.wrapping_shl(u32::from(subsample.x == 0))),
        y: start.y.wrapping_shl(4).wrapping_add(mv.y.wrapping_shl(u32::from(subsample.y == 0))),
      };
      let Ok(reference_position) = console_core_number_conversion::index(reference_index);
      let [horizontal, vertical] = f.svc.get(reference_position).ok_or(())?;
      let scale = |value: i32, multiplier: i32| {
        let correction = multiplier.wrapping_sub(0x4000).wrapping_mul(8);
        let scaled = i64::from(value).wrapping_mul(i64::from(multiplier)).wrapping_add(i64::from(correction));
        let rounded = scaled.wrapping_abs().wrapping_add(128).wrapping_shr(8);
        let Ok(quotient) = console_core_number_conversion::wrapped::<_, i32>(rounded);
        apply_sign64(quotient, scaled).wrapping_add(32)
      };
      let sample = Point { x: scale(origin.x, horizontal.scale), y: scale(origin.y, vertical.scale) };
      let source = Point { x: sample.x.wrapping_shr(10), y: sample.y.wrapping_shr(10) };
      let right = sample.x.wrapping_add(block.width.wrapping_sub(1).wrapping_mul(horizontal.step)).wrapping_shr(10).wrapping_add(1);
      let bottom = sample.y.wrapping_add(block.height.wrapping_sub(1).wrapping_mul(vertical.step)).wrapping_shr(10).wrapping_add(1);

      match debug_block_info!(f, position) {
        BlockTrace::Enabled => {
          console_core_logging::trace!(
            "Off {}x{} [{},{},{}], size {}x{} [{},{}]",
            source.x, source.y, origin.x, horizontal.scale, reference_index,
            right.wrapping_sub(source.x), bottom.wrapping_sub(source.y), horizontal.step, vertical.step,
          );
        }
        BlockTrace::Quiet => {}
      }

      let extent = Size {
        width: reference.p.p.w.wrapping_add(i32::from(subsample.x != 0)).wrapping_shr(subsample.x),
        height: reference.p.p.h.wrapping_add(i32::from(subsample.y != 0)).wrapping_shr(subsample.y),
      };
      let outside = source.x < 3 || source.y < 3 || right.wrapping_add(4) > extent.width || bottom.wrapping_add(4) > extent.height;
      let edge_component;
      let input = match outside.then_some(()) {
        Some(()) => {
          const STRIDE: u32 = 320;
          let buffer = emu_edge.buf_mut::<BD>().map_err(|_invalid_scratch| ())?;
          let window = crate::src::mc::EdgeWindow {
            block: Size { width: right.wrapping_sub(source.x).wrapping_add(7), height: bottom.wrapping_sub(source.y).wrapping_add(7) },
            reference: extent,
            origin: Point { x: source.x.wrapping_sub(3), y: source.y.wrapping_sub(3) },
          };
          f.dsp.mc.emu_edge.call::<BD>(window, buffer, STRIDE, component).map_err(|_edge_layout| ())?;

          match debug_block_info!(f, position) {
            BlockTrace::Enabled => { console_core_logging::trace!("Emu"); }
            BlockTrace::Quiet => {}
          }

          let Ok(stride) = console_core_number_conversion::index(STRIDE);
          edge_component = Rav1dPictureDataComponent::wrap_buf::<BD>(buffer, stride);
          motion_picture_position::<BD>(edge_component.with_offset::<BD>(), Point { x: 3, y: 3 })?
        }
        None => motion_picture_position::<BD>(component.with_offset::<BD>(), source)?,
      };
      let sampling = crate::src::mc::ScaledSampling {
        size: block,
        phase: Point { x: sample.x & 0x3ff, y: sample.y & 0x3ff },
        step: Point { x: horizontal.step, y: vertical.step },
      };
      let result = match dst {
        MaybeTempPixels::NonTemp { dst: dst_3 } => {
          let mc = f.dsp.mc.mc_scaled.checked(filter).map_err(|_invalid_dispatch| ())?;
          mc.call::<BD>(filter, dst_3, input, sampling, bd)
        }
        MaybeTempPixels::Temp { tmp, .. } => {
          let mct = f.dsp.mc.mct_scaled.checked(filter).map_err(|_invalid_dispatch| ())?;
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

fn obmc<BD: BitDepth>(
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
  dst: PicOffset,
  OverlapReconstruction {
    visible, dimensions: [block_width, block_height, horizontal_limit, vertical_limit], context, plane,
  }: OverlapReconstruction,
) -> Result<(), ()> {
  match {
      ::core::primitive::bool::then_some((t.b.x | t.b.y) & 1 != 0, ())
  } {
      ::core::option::Option::Some(()) => {
        return Err(());
      },
      ::core::option::Option::None => {},
  }

  let tile = f.tile(t.tile_index).ok_or(())?;
  let above = f.above_context(t.above_index).ok_or(())?;
  let [above_x, above_y] = &above.filter;
  let [left_x, left_y] = &mut t.l.filter;
  let position = t.b;
  let ring_row = (position.y & 31).wrapping_add(5);
  let layout = f.cur.p.layout;
  let x_shift = u32::from(plane != 0 && layout != Rav1dPixelLayout::I444);
  let y_shift = u32::from(plane != 0 && layout == Rav1dPixelLayout::I420);
  let pixel_width = 4_i32.wrapping_shr(x_shift);
  let pixel_height = 4_i32.wrapping_shr(y_shift);
  let luma = plane == 0;
  let fits_above = i32::from(block_width).wrapping_mul(pixel_width)
    .wrapping_add(i32::from(block_height).wrapping_mul(pixel_height)) >= 16;
  let scratch = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
  let lap = scratch.lap_inter.lap_mut::<BD>().map_err(|_invalid_scratch| ())?;

  use crate::src::safe_simd::mc::BlendAxis;

  for axis in [BlendAxis::Horizontal, BlendAxis::Vertical] {
    let (visible_extent, max_neighbors) = match axis {
      BlendAxis::Horizontal if position.y > tile.tiling.row_start && (luma || fits_above) => {
        (visible.width, horizontal_limit.min(4))
      }
      BlendAxis::Vertical if position.x > tile.tiling.col_start => (visible.height, vertical_limit.min(4)),
      BlendAxis::Horizontal | BlendAxis::Vertical => continue,
    };
    let mut count = 0_u8;
    let mut distance = 0_i32;

    'inner_for_2: for _ in 0..visible_extent {
      match {
          ::core::primitive::bool::then_some(distance >= visible_extent || count >= max_neighbors, ())
      } {
          ::core::option::Option::Some(()) => {
            break 'inner_for_2;
          },
          ::core::option::Option::None => {},
      }

      let one_ahead = distance.wrapping_add(1);
      let neighbor = match axis {
        BlendAxis::Horizontal => Point { x: position.x.wrapping_add(one_ahead), y: ring_row.wrapping_sub(1) },
        BlendAxis::Vertical => Point { x: position.x.wrapping_sub(1), y: ring_row.wrapping_add(one_ahead) },
      };
      let block = crate::src::refmvs::read_spatial_block(&f.rf.r, &t.rt.r, neighbor).map_err(|_invalid_neighbor| ())?;
      let Ok(&[neighbor_width, neighbor_height, ..]) = block.bs.dimensions();
      let span = match axis { BlendAxis::Horizontal => neighbor_width, BlendAxis::Vertical => neighbor_height };
      let step = span.max(2).min(16);
      let [reference, _] = block.r#ref.r#ref;

      match {
          ::core::primitive::bool::then_some(reference > 0, ())
      } {
          ::core::option::Option::Some(()) => {
            let reference_code = u8::try_from(reference.wrapping_sub(1)).map_err(|_invalid_reference| ())?;
            let Ok(reference_index) = console_core_number_conversion::index(reference_code);
            let reference_picture = f.refp.get(reference_index).ok_or(())?;
            let [motion, _] = block.mv.mv;
            let (width, height, filter_position) = match axis {
              BlendAxis::Horizontal => (step.min(block_width), block_height.min(16).wrapping_shr(1), context.x.wrapping_add(one_ahead)),
              BlendAxis::Vertical => (block_width.min(16).wrapping_shr(1), step.min(block_height), context.y.wrapping_add(one_ahead)),
            };
            let filter_position = console_core_number_conversion::checked_index(filter_position).map_err(|_outside_filter| ())?;
            let [filter_x, filter_y] = match axis {
              BlendAxis::Horizontal => {
                match {
                    ::core::primitive::bool::then_some(filter_position >= above_x.len() || filter_position >= above_y.len(), ())
                } {
                    ::core::option::Option::Some(()) => {
                      return Err(());
                    },
                    ::core::option::Option::None => {},
                }

                [*above_x.index(filter_position), *above_y.index(filter_position)]
              }
              BlendAxis::Vertical => [
                *left_x.get_mut().get(filter_position).ok_or(())?,
                *left_y.get_mut().get(filter_position).ok_or(())?,
              ],
            };
            let Ok(filter_column) = console_core_number_conversion::index(u8::from(filter_x));
            let Ok(filter_row) = console_core_number_conversion::index(u8::from(filter_y));
            let &filter = dav1d_filter_2d.get(filter_row).and_then(|row| row.get(filter_column)).ok_or(())?;
            let width = i32::from(width);
            let height = i32::from(height);
            let size = Size { width: width.wrapping_mul(pixel_width), height: height.wrapping_mul(pixel_height) };
            let lap_stride = console_core_number_conversion::checked_index(size.width).map_err(|_invalid_stride| ())?;
            let lap_component = Rav1dPictureDataComponent::wrap_buf::<BD>(lap, lap_stride);
            let (height_to_predict, reference_origin, output) = match axis {
              BlendAxis::Horizontal => {
                let origin = Point { x: position.x.wrapping_add(distance), y: position.y };
                let height_to_predict = height.wrapping_mul(3).wrapping_add(3).wrapping_shr(2);
                let offset = console_core_number_conversion::checked_index(distance.wrapping_mul(pixel_width)).map_err(|_outside_destination| ())?;
                (height_to_predict, origin, dst + offset)
              }
              BlendAxis::Vertical => {
                let origin = Point { x: position.x, y: position.y.wrapping_add(distance) };
                let Ok(row) = console_core_number_conversion::wrapped_offset(distance.wrapping_mul(pixel_height));
                let Ok(stride) = ::console_core_number_conversion::fixed_offset(match dst.pixel_stride::<BD>() { ::core::result::Result::Ok(stride) => stride });
                (height, origin, dst + row.wrapping_mul(match ::console_core_number_conversion::wrapped_offset(stride) { ::core::result::Result::Ok(native_position) => native_position }))
              }
            };
            mc::<BD>(
              f,
              &mut scratch.emu_edge,
              MaybeTempPixels::NonTemp { dst: PicOffset { data: &lap_component, offset: 0 } },
              MotionPrediction {
                position,
                size: Size { width, height: height_to_predict },
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
            let blended = match axis {
              BlendAxis::Horizontal => f.dsp.mc.blend_h.call::<BD>(axis, output, lap, size),
              BlendAxis::Vertical => f.dsp.mc.blend_v.call::<BD>(axis, output, lap, size),
            };
            blended.map_err(|_invalid_destination| ())?;
            count = count.wrapping_add(1);
          },
          ::core::option::Option::None => {},
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

fn warp_affine<BD: BitDepth>(
  f: &Rav1dFrameData,
  emu_edge: &mut ScratchEmuEdge,
  mut dst: MaybeTempPixels<u64>,
  WarpPrediction { position, dimensions, plane, reference, parameters }: WarpPrediction<'_>,
) -> Result<(), ()> {
  let Ok(coefficients) = parameters.abcd.get();
  let [x_offset, y_offset, xx, xy, yx, yy] = parameters.matrix;
  let depth = BD::from_c(f.bitdepth_max);
  let ref_data = &reference.p.data.as_ref().ok_or(())?.data;
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

  let extent = Size {
    width: reference.p.p.w.wrapping_add(i32::from(subsample.x != 0)).wrapping_shr(subsample.x),
    height: reference.p.p.h.wrapping_add(i32::from(subsample.y != 0)).wrapping_shr(subsample.y),
  };
  let alpha = i32::from(parameters.alpha()).wrapping_mul(4);
  let beta = i32::from(parameters.beta()).wrapping_mul(7);
  let gamma = i32::from(parameters.gamma()).wrapping_mul(4);
  let delta = i32::from(parameters.delta()).wrapping_mul(4);

  for y in (0..block.height).step_by(8) {
    let src_y = position.y.wrapping_mul(4).wrapping_add(y.wrapping_add(4).wrapping_shl(subsample.y));
    let tx = i64::from(xy).wrapping_mul(i64::from(src_y)).wrapping_add(i64::from(x_offset));
    let ty = i64::from(yy).wrapping_mul(i64::from(src_y)).wrapping_add(i64::from(y_offset));

    for x in (0..block.width).step_by(8) {
      let src_x = position.x.wrapping_mul(4).wrapping_add(x.wrapping_add(4).wrapping_shl(subsample.x));
      let transformed_x = i64::from(xx).wrapping_mul(i64::from(src_x)).wrapping_add(tx).wrapping_shr(subsample.x);
      let transformed_y = i64::from(yx).wrapping_mul(i64::from(src_x)).wrapping_add(ty).wrapping_shr(subsample.y);
      let Ok(center_x) = console_core_number_conversion::wrapped::<_, i32>(transformed_x.wrapping_shr(16));
      let Ok(center_y) = console_core_number_conversion::wrapped::<_, i32>(transformed_y.wrapping_shr(16));
      let source = Point { x: center_x.wrapping_sub(4), y: center_y.wrapping_sub(4) };
      let Ok(phase_x) = console_core_number_conversion::wrapped::<_, i32>(transformed_x & 0xffff);
      let Ok(phase_y) = console_core_number_conversion::wrapped::<_, i32>(transformed_y & 0xffff);
      let sampling = crate::src::mc::WarpSampling {
        coefficients,
        phase: Point {
          x: phase_x.wrapping_sub(alpha).wrapping_sub(beta) & !0x3f,
          y: phase_y.wrapping_sub(gamma).wrapping_sub(delta) & !0x3f,
        },
      };
      let outside = source.x < 3 || source.y < 3
        || source.x.wrapping_add(12) > extent.width || source.y.wrapping_add(12) > extent.height;
      let edge_component;
      let input = match outside.then_some(()) {
        Some(()) => {
          const STRIDE: u32 = 32;
          let buffer = emu_edge.buf_mut::<BD>().map_err(|_invalid_scratch| ())?;
          let window = crate::src::mc::EdgeWindow {
            block: Size { width: 15, height: 15 },
            reference: extent,
            origin: Point { x: source.x.wrapping_sub(3), y: source.y.wrapping_sub(3) },
          };
          f.dsp.mc.emu_edge.call::<BD>(window, buffer, STRIDE, component).map_err(|_edge_layout| ())?;
          let Ok(stride) = console_core_number_conversion::index(STRIDE);
          edge_component = Rav1dPictureDataComponent::wrap_buf::<BD>(buffer, stride);
          motion_picture_position::<BD>(edge_component.with_offset::<BD>(), Point { x: 3, y: 3 })?
        }
        None => motion_picture_position::<BD>(component.with_offset::<BD>(), source)?,
      };
      let result = match &mut dst {
        MaybeTempPixels::Temp { tmp, tmp_stride } => {
          let row = u64::try_from(y).map_err(|_invalid_row| ())?;
          let column = u64::try_from(x).map_err(|_invalid_column| ())?;
          let samples = row.checked_mul(*tmp_stride).ok_or(())?;
          let offset = samples.checked_add(column).ok_or(())?;
          let Ok(first_sample) = console_core_number_conversion::index(offset);
          let output = tmp.get_mut(first_sample..).ok_or(())?;
          f.dsp.mc.warp8x8t.call(output, *tmp_stride, input, sampling, depth)
        }
        MaybeTempPixels::NonTemp { dst: dst_2 } => {
          let target = motion_picture_position::<BD>(*dst_2, Point { x, y })?;
          f.dsp.mc.warp8x8.call(target, input, sampling, depth)
        }
      };
      result.map_err(|_invalid_warp| ())?;
    }
  }

  Ok(())
}

fn block_plane_origin(position: Bxy, sampling: Point<u32>) -> Result<Point<i32>, console_core_never::Never> {
  let x = position.x.wrapping_shr(sampling.x).wrapping_mul(4);
  let y = position.y.wrapping_shr(sampling.y).wrapping_mul(4);
  Ok(Point { x, y })
}

struct PredictionEdgeRow {
  block_y: i32,
  chroma_rounding: i32,
  plane: u8,
}

fn prefiltered_top(f: &Rav1dFrameData, request: PredictionEdgeRow) -> Rav1dResult<Option<PrefilteredTop<'_>>> {
  let boundary = request.block_y & !request.chroma_rounding & f.sb_step.wrapping_sub(1);

  match boundary {
    0 => {}
    _ => return Ok(None),
  }

  let shift = u32::try_from(f.sb_shift).map_err(|_invalid_shift| EINVAL)?;
  let previous = request.block_y.wrapping_shr(shift).wrapping_sub(1);
  let row_width = i64::from(f.sb128w).wrapping_mul(128);
  let rows = i64::from(previous).checked_mul(row_width).ok_or(EINVAL)?;
  let bank = f.ipred_edge_off.checked_mul(u64::from(request.plane)).ok_or(EINVAL)?;
  let plane = i64::try_from(bank).map_err(|_outside_edge_buffer| EINVAL)?;
  let row_start = plane.checked_add(rows).ok_or(EINVAL)?;
  Ok(Some(PrefilteredTop { samples: &f.ipred_edge, row_start }))
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
  {
    let above_context = f.above_context(t.above_index).ok_or(EINVAL)?;
    let bd = BD::from_c(f.bitdepth_max);

    let picture = f.cur.data.as_ref().ok_or(EINVAL)?;
    let Ok(mut planes) = crate::src::owned_recon::ReconPlanes::bind(
      &picture.data,
      &mut t.recon_band,
    );
    let ts = &*match f.tile(t.tile_index) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
      }
    };

    let bx4 = t.b.x & 31;
    let by4 = t.b.y & 31;
    let ss_ver = i32::from(f.cur.p.layout == Rav1dPixelLayout::I420);
    let ss_hor = i32::from(f.cur.p.layout != Rav1dPixelLayout::I444);
    let chroma_shifts = Point {
      x: u32::from(f.cur.p.layout != Rav1dPixelLayout::I444),
      y: u32::from(f.cur.p.layout == Rav1dPixelLayout::I420),
    };
    let cbx4 = i32::wrapping_shr(bx4, {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
      numeric_value
    });
    let cby4 = i32::wrapping_shr(by4, {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
      numeric_value
    });
    let Ok(b_dim) = bs.dimensions();
    let Ok(bw4) = console_core_number_conversion::wrapped::<_, i32>(match b_dim {
      &[element, ..] => element,
    });
    let Ok(bh4) = console_core_number_conversion::wrapped::<_, i32>(match b_dim {
      &[_, element, ..] => element,
    });
    let w4 = cmp::min(bw4, i32::wrapping_sub(f.bw, t.b.x));
    let h4 = cmp::min(bh4, i32::wrapping_sub(f.bh, t.b.y));
    let cw4 = i32::wrapping_shr(i32::wrapping_add(w4, ss_hor), {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
      numeric_value
    });
    let ch4 = i32::wrapping_shr(i32::wrapping_add(h4, ss_ver), {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
      numeric_value
    });
    let has_chroma = f.cur.p.layout != Rav1dPixelLayout::I400
      && (bw4 > ss_hor || t.b.x & 1 != 0)
      && (bh4 > ss_ver || t.b.y & 1 != 0);
    let t_dim = &*match <[_]>::get(&dav1d_txfm_dimensions, { let Ok(table_index) = console_core_number_conversion::index({ let Ok(ordinal) = intra.tx.ordinal(); ordinal }); table_index }) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
      }
    };
    let uv_t_dim = &*match <[_]>::get(&dav1d_txfm_dimensions, { let Ok(table_index) = console_core_number_conversion::index({ let Ok(ordinal) = b.uvtx.ordinal(); ordinal }); table_index }) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
      }
    };

    let cbw4 = i32::wrapping_shr(i32::wrapping_add(bw4, ss_hor), {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
      numeric_value
    });
    let cbh4 = i32::wrapping_shr(i32::wrapping_add(bh4, ss_ver), {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
      numeric_value
    });

    let intra_edge_filter = f.seq_hdr.as_ref().ok_or(EINVAL)?.intra_edge_filter;
    let intra_edge_filter_flag = i32::wrapping_shl(
      {
        let Ok(numeric_value) =
          console_core_number_conversion::wrapped::<_, i32>(intra_edge_filter);
        numeric_value
      },
      10_u32,
    );

    for init_y in (0..h4).step_by(16) {
      let sub_h4 = cmp::min(h4, i32::wrapping_add(16, init_y));
      let sub_ch4 = cmp::min(
        ch4,
        i32::wrapping_shr(i32::wrapping_add(init_y, 16), {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
          numeric_value
        }),
      );

      'over_init_x_2: for init_x in (0..w4).step_by(16) {
        match ::core::primitive::bool::then_some((match intra.pal_sz {
          [element, ..] => element,
        }) != 0, ())
        {
          ::core::option::Option::Some(()) => {
            let mut y_dst = planes.dst::<BD>(0, Point { x: t.b.x.wrapping_mul(4), y: t.b.y.wrapping_mul(4) })?;
            let scratch = t.scratch.inter_intra_mut()?;
            let pal_idx = match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
              ::core::option::Option::Some(()) => {
                let Ok(p) =
                  console_core_number_conversion::wrapped_index(t.frame_thread.pass & 1);
                let frame_thread = &*match <[_]>::get(&ts.frame_thread, p) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                };
                let Ok(len) = console_core_number_conversion::wrapped::<_, u32>(
                  i32::wrapping_mul(i32::wrapping_mul(bw4, bh4), 8),
                );
                let Ok(pal_idx) = frame_thread
                  .pal_idx
                  .get_update(|i| u32::wrapping_add(i, len));
                &*f.frame_thread.pal_idx.index((
                  ({
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped_index(pal_idx);
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
                frame_palette = f.frame_thread.pal.checked_index::<BD>(record)?;
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
                Size { width: i32::wrapping_mul(bw4, 4), height: i32::wrapping_mul(bh4, 4) },
              )?
            });

            match (debug_block_info!(f, t.b), PIXEL_TRACE) {
              (BlockTrace::Enabled, BlockTrace::Enabled) => {
                hex_dump_pic::<BD>(
                  &y_dst,
                  Size {
                    width: { let checked_value_5 = u64::try_from(bw4).map_err(|_negative_extent| EINVAL)?; checked_value_5 }.wrapping_mul(4),
                    height: { let checked_value_6 = u64::try_from(bh4).map_err(|_negative_extent| EINVAL)?; checked_value_6 }.wrapping_mul(4),
                  },
                  "y-pal-pred",
                ).map_err(|_dump_error| EIO)?;
              }
              (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
            }
          }
          ::core::option::Option::None => {}
        }

        let above_sm = sm_flag(
          &*match f.above_context(t.above_index) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
            }
          },
          bx4,
        )?;
        let left_sm = sm_flag(&t.l, by4)?;
        let intra_flags = above_sm | left_sm
          | intra_edge_filter_flag;
        let sb_has_tr = match ::core::primitive::bool::then_some(i32::wrapping_add(init_x, 16) < w4, ()) {
          ::core::option::Option::Some(()) => true,
          ::core::option::Option::None => match ::core::primitive::bool::then_some(init_y != 0, ()) {
            ::core::option::Option::Some(()) => false,
            ::core::option::Option::None => matches!(intra_edge_flags.availability(EdgeFlags::I444_TOP_HAS_RIGHT), Ok(EdgeAvailability::Present)),
          },
        };
        let sb_has_bl = match ::core::primitive::bool::then_some(init_x != 0, ()) {
          ::core::option::Option::Some(()) => false,
          ::core::option::Option::None => match ::core::primitive::bool::then_some(i32::wrapping_add(init_y, 16) < h4, ()) {
            ::core::option::Option::Some(()) => true,
            ::core::option::Option::None => matches!(intra_edge_flags.availability(EdgeFlags::I444_LEFT_HAS_BOTTOM), Ok(EdgeAvailability::Present)),
          },
        };
        let mut y;
        let mut x;
        let sub_w4 = cmp::min(w4, i32::wrapping_add(init_x, 16));
        y = init_y;

        {
          let numeric_amount = init_y;
          let numeric_target = &mut (t.b.y);
          *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
        };

        while y < sub_h4 {
          x = init_x;

          {
            let numeric_amount = init_x;
            let numeric_target = &mut (t.b.x);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };

          while x < sub_w4 {
            let mut y_dst = planes.dst::<BD>(0, Point { x: t.b.x.wrapping_mul(4), y: t.b.y.wrapping_mul(4) })?;
            let mut angle;
            let edge_flags;
            let m;

            match ::core::primitive::bool::then_some(!((match intra.pal_sz {
              [element, ..] => element,
            }) != 0), ())
            {
              ::core::option::Option::Some(()) => {
                angle = {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(intra.y_angle);
                  numeric_value
                };
                edge_flags = {
                  let Ok(explicit_value) = EdgeFlags::union_all([
                    ({
                      let Ok(explicit_value) = EdgeFlags::I444_TOP_HAS_RIGHT.select(
                        EdgeAvailability::from(!((y > init_y || !sb_has_tr)
                          && i32::wrapping_add(x, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, i32>(t_dim.w);
                            numeric_value
                          }) >= sub_w4)),
                      );
                      explicit_value
                    }),
                    ({
                      let Ok(explicit_value) = EdgeFlags::I444_LEFT_HAS_BOTTOM.select(
                        EdgeAvailability::from(!(x > init_x
                          || (!sb_has_bl
                            && i32::wrapping_add(y, {
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped::<_, i32>(t_dim.h);
                              numeric_value
                            }) >= sub_h4))),
                      );
                      explicit_value
                    }),
                  ]);
                  explicit_value
                };
                let saved_row = PredictionEdgeRow { block_y: t.b.y, chroma_rounding: 0, plane: 0 };
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
                let edge_offset = 128;
                m = {
                  rav1d_prepare_intra_edges(
                    IntraEdgePreparation {
                      position: Point { x: t.b.x, y: t.b.y },
                      frame: Size { width: ts.tiling.col_end, height: ts.tiling.row_end },
                      transform: Size { width: {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, i32>(t_dim.w);
                      numeric_value
                    }, height: {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, i32>(t_dim.h);
                      numeric_value
                    } },
                      has_left: t.b.x > ts.tiling.col_start,
                      above: {
                        let Ok(above) = AboveEdge::select(EdgeAvailability::from(t.b.y > ts.tiling.row_start), prefiltered);
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
                      center: {
                        let Ok(center) = console_core_number_conversion::fixed_index(edge_offset);
                        center
                      },
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
                      edge_center: {
                        let Ok(center) = console_core_number_conversion::fixed_index(edge_offset);
                        center
                      },
                      size: Size { width: i32::wrapping_mul(
                      {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, i32>(t_dim.w);
                        numeric_value
                      },
                      4,
                    ), height: i32::wrapping_mul(
                      {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, i32>(t_dim.h);
                        numeric_value
                      },
                      4,
                    ) },
                      reach: Size { width: i32::wrapping_sub(i32::wrapping_mul(4, f.bw), i32::wrapping_mul(4, t.b.x)), height: i32::wrapping_sub(i32::wrapping_mul(4, f.bh), i32::wrapping_mul(4, t.b.y)) },
                      angle: angle | intra_flags,
                    },
                    bd,
                  )?
                });

                match (debug_block_info!(f, t.b), PIXEL_TRACE) {
                  (BlockTrace::Enabled, BlockTrace::Enabled) => {
                    hex_dump::<BD>(
                      &*match <[_]>::get(
                        &*edge_array,
                        (::core::num::Wrapping(edge_offset)
                          - ::core::num::Wrapping(
                            ({
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped_index(t_dim.h);
                              numeric_value
                            })
                            .wrapping_mul(4),
                          ))
                        .0..,
                      ) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      },
                      Size {
                        width: u64::from(t_dim.h).wrapping_mul(4),
                        height: 2,
                      },
                      u64::from(t_dim.h).wrapping_mul(4),
                      "l",
                    ).map_err(|_dump_error| EIO)?;
                    hex_dump::<BD>(
                      &*match <[_]>::get(&*edge_array, edge_offset..) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      },
                      Size {
                        width: 1,
                        height: 1,
                      },
                      0,
                      "tl",
                    ).map_err(|_dump_error| EIO)?;
                    hex_dump::<BD>(
                      &*match <[_]>::get(&*edge_array, usize::wrapping_add(edge_offset, 1)..) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      },
                      Size {
                        width: u64::from(t_dim.w).wrapping_mul(4),
                        height: 2,
                      },
                      u64::from(t_dim.w).wrapping_mul(4),
                      "t",
                    ).map_err(|_dump_error| EIO)?;
                    hex_dump_pic::<BD>(
                      &y_dst,
                      Size {
                        width: u64::from(t_dim.w).wrapping_mul(4),
                        height: u64::from(t_dim.h).wrapping_mul(4),
                      },
                      "y-intra-pred",
                    ).map_err(|_dump_error| EIO)?;
                  }
                  (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
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
                    let Ok(p) =
                      console_core_number_conversion::wrapped_index(t.frame_thread.pass & 1);
                    let len = u32::wrapping_mul(
                      u32::wrapping_mul(
                        u32::wrapping_mul(
                          {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(cmp::min(
                                t_dim.h, 8,
                              ));
                            numeric_value
                          },
                          4,
                        ),
                        {
                          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                            cmp::min(t_dim.w, 8),
                          );
                          numeric_value
                        },
                      ),
                      4,
                    );
                    let Ok(cf_idx) = (*match <[_]>::get(&ts.frame_thread, p) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => {
                        return ::core::result::Result::Err(
                          crate::src::error::Rav1dError::EINVAL,
                        );
                      }
                    })
                    .cf
                    .get_update(|i| u32::wrapping_add(i, len));
                    cf_guard = f.frame_thread.cf.mut_slice_as((
                      ({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(cf_idx);
                        numeric_value
                      })..,
                      ..({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(len);
                        numeric_value
                      }),
                    ));
                    cf = &mut *cf_guard;
                    let Ok(cbi_idx) = (*match <[_]>::get(&ts.frame_thread, p) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => {
                        return ::core::result::Result::Err(
                          crate::src::error::Rav1dError::EINVAL,
                        );
                      }
                    })
                    .cbi_idx
                    .get_update(|i| u32::wrapping_add(i, 1));
                    let Ok(cbi) = (*match <[_]>::get(&f.frame_thread.cbi, {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped_index(cbi_idx);
                      numeric_value
                    }) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => {
                        return ::core::result::Result::Err(
                          crate::src::error::Rav1dError::EINVAL,
                        );
                      }
                    })
                    .get();
                    eob = <::core::primitive::i32 as ::core::convert::From<
                      ::core::primitive::i16,
                    >>::from({
                      let Ok(explicit_value) = cbi.eob();
                      explicit_value
                    });
                    txtp = {
                      let Ok(explicit_value) = cbi.txtp();
                      explicit_value
                    };
                  }
                  ::core::option::Option::None => {
                    let mut cf_ctx = 0;
                    let Ok(a_start) =
                      console_core_number_conversion::wrapped_index(i32::wrapping_add(bx4, x));
                    let Ok(l_start) =
                      console_core_number_conversion::wrapped_index(i32::wrapping_add(by4, y));
                    eob = {
                      decode_coefs::<BD>(
                        f,
                        CoefficientDecoder {
                          tile: ts,
                          state: ts_c.as_deref_mut().ok_or(EINVAL)?,
                          scratch: &mut t.scratch,
                          coefficients: &mut t.cf,
                        },
                        CoefficientBlock {
                          trace: debug_block_info!(f, t.b),
                          above: &(*match f.above_context(t.above_index) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None => {
                              return ::core::result::Result::Err(
                                crate::src::error::Rav1dError::EINVAL,
                              );
                            }
                          })
                          .lcoef
                          .index(
                            a_start..a_start.wrapping_add({
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped_index(t_dim.w);
                              numeric_value
                            }),
                          ),
                          left: &*match <[_]>::get(
                            &**t.l.lcoef.get_mut(),
                            l_start..l_start.wrapping_add({
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped_index(t_dim.h);
                              numeric_value
                            }),
                          ) {
                            ::core::option::Option::Some(indexed_value_3) => indexed_value_3,
                            ::core::option::Option::None => {
                              return ::core::result::Result::Err(
                                crate::src::error::Rav1dError::EINVAL,
                              );
                            }
                          },
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

                    match debug_block_info!(f, t.b) {
                      BlockTrace::Enabled => {
                        println!(
                          "Post-y-cf-blk[tx={:?},txtp={},eob={}]: r={}",
                          intra.tx,
                          txtp,
                          eob,
                          ts_c.as_deref().ok_or(EINVAL)?.msac.rng,
                        );
                      }
                      BlockTrace::Quiet => {}
                    }

                    case_set_al! {
                        <16, true>
                        l: (
                            &mut t.l,
                            { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cmp::min({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(t_dim.h); numeric_value }, i32::wrapping_sub(f.bh, t.b.y))); numeric_value },
                            { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(i32::wrapping_add(by4, y)); numeric_value }
                        ),
                        a: (
                            above_context,
                            { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cmp::min({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(t_dim.w); numeric_value }, i32::wrapping_sub(f.bw, t.b.x))); numeric_value },
                            { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(i32::wrapping_add(bx4, x)); numeric_value }
                        ),
                        lcoef = (cf_ctx, cf_ctx),
                    }
                  }
                }

                match ::core::primitive::bool::then_some(eob >= 0, ()) {
                  ::core::option::Option::Some(()) => {
                    match (debug_block_info!(f, t.b), PIXEL_TRACE) {
                      (BlockTrace::Enabled, BlockTrace::Enabled) => {
                        coef_dump(
                          cf,
                          Size {
                            width: u64::from(t_dim.h.min(8)).wrapping_mul(4),
                            height: u64::from(t_dim.w.min(8)).wrapping_mul(4),
                          },
                          3,
                          "dq",
                        ).map_err(|_dump_error| EIO)?;
                      }
                      (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
                    }

                    f.dsp
                      .itx
                      .add::<BD>(intra.tx, txtp, &mut y_dst, cf, eob, bd)?;

                    match (debug_block_info!(f, t.b), PIXEL_TRACE) {
                      (BlockTrace::Enabled, BlockTrace::Enabled) => {
                        hex_dump_pic::<BD>(
                          &y_dst,
                          Size {
                            width: u64::from(t_dim.w).wrapping_mul(4),
                            height: u64::from(t_dim.h).wrapping_mul(4),
                          },
                          "recon",
                        ).map_err(|_dump_error| EIO)?;
                      }
                      (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
                    }
                  }
                  ::core::option::Option::None => {}
                }
              }
              ::core::option::Option::None => match ::core::primitive::bool::then_some(t.frame_thread.pass == 0, ()) {
                ::core::option::Option::Some(()) => {
                  case_set_al! {
                      <16, false>
                      l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t_dim.h); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(i32::wrapping_add(by4, y)); numeric_value }),
                      a: (above_context, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t_dim.w); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(i32::wrapping_add(bx4, x)); numeric_value }),
                      lcoef = (0x40, 0x40),
                  }
                }
                ::core::option::Option::None => {}
              },
            }

            {
              let numeric_amount = {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(t_dim.w);
                numeric_value
              };
              let numeric_target = &mut (x);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };

            {
              let numeric_amount = {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(t_dim.w);
                numeric_value
              };
              let numeric_target = &mut (t.b.x);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };
          }

          {
            let numeric_amount = x;
            let numeric_target = &mut (t.b.x);
            *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
          };

          {
            let numeric_amount = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(t_dim.h);
              numeric_value
            };
            let numeric_target = &mut (y);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };

          {
            let numeric_amount = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(t_dim.h);
              numeric_value
            };
            let numeric_target = &mut (t.b.y);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };
        }

        {
          let numeric_amount = y;
          let numeric_target = &mut (t.b.y);
          *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
        };

        match ::core::primitive::bool::then_some(!has_chroma, ()) {
          ::core::option::Option::Some(()) => {
            continue 'over_init_x_2;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(intra.uv_mode == CFL_PRED, ()) {
          ::core::option::Option::Some(()) => {
            assert!(init_x == 0 && init_y == 0);

            let scratch = t.scratch.inter_intra_mut()?;
            let ac = scratch.ac_txtp_map.ac_mut()?;
            let y_src = {
              let explicit_value = planes.src::<BD>(0, Point { x: (t.b.x & !ss_hor).wrapping_mul(4), y: (t.b.y & !ss_ver).wrapping_mul(4) })?;
              explicit_value
            };

            let furthest_r = i32::wrapping_sub(
              i32::wrapping_add(
                i32::wrapping_shl(cw4, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                  numeric_value
                }),
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(t_dim.w);
                  numeric_value
                },
              ),
              1,
            ) & !i32::wrapping_sub(
              {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(t_dim.w);
                numeric_value
              },
              1,
            );
            let furthest_b = i32::wrapping_sub(
              i32::wrapping_add(
                i32::wrapping_shl(ch4, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                  numeric_value
                }),
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(t_dim.h);
                  numeric_value
                },
              ),
              1,
            ) & !i32::wrapping_sub(
              {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(t_dim.h);
                numeric_value
              },
              1,
            );
            let layout = f.cur.p.layout.try_into().map_err(|_no_chroma| EINVAL)?;
            ({
              let checked_value_2 = f.dsp.ipred.cfl_ac.checked(layout)?;
              let Ok(explicit_value) = checked_value_2.call::<BD>(
                layout,
                ac,
                &y_src,
                i32::wrapping_sub(
                  cbw4,
                  i32::wrapping_shr(furthest_r, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                    numeric_value
                  }),
                ),
                i32::wrapping_sub(
                  cbh4,
                  i32::wrapping_shr(furthest_b, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                    numeric_value
                  }),
                ),
                i32::wrapping_mul(cbw4, 4),
                i32::wrapping_mul(cbh4, 4),
              );
              explicit_value
            });

            'over_pl_3: for pl in 0..2 {
              match ::core::primitive::bool::then_some(*match <[_]>::get(&intra.cfl_alpha, pl) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              } == 0, ())
              {
                ::core::option::Option::Some(()) => {
                  continue 'over_pl_3;
                }
                ::core::option::Option::None => {}
              }

              let mut angle = 0;
              let chroma_plane = u8::try_from(pl).map_err(|_invalid_plane| EINVAL)?;
              let saved_row = PredictionEdgeRow { block_y: t.b.y, chroma_rounding: ss_ver, plane: chroma_plane.wrapping_add(1) };
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
              let edge_offset = 128;
              let mut uv_dst = {
                let explicit_value = planes.dst::<BD>({ let checked_value_24 = u8::try_from(pl).map_err(|_invalid_plane| EINVAL)?; checked_value_24 }.wrapping_add(1), { let Ok(origin) = block_plane_origin(t.b, chroma_shifts); origin })?;
                explicit_value
              };
              let m: IntraPredMode = {
                rav1d_prepare_intra_edges(
                  IntraEdgePreparation {
                    position: Point { x: xpos, y: ypos },
                    frame: Size { width: i32::wrapping_shr(ts.tiling.col_end, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                    numeric_value
                  }), height: i32::wrapping_shr(ts.tiling.row_end, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                    numeric_value
                  }) },
                    transform: Size { width: {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w);
                    numeric_value
                  }, height: {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h);
                    numeric_value
                  } },
                    has_left: xpos > xstart,
                    above: {
                      let Ok(above) = AboveEdge::select(EdgeAvailability::from(ypos > ystart), prefiltered);
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
                    center: {
                      let Ok(center) = console_core_number_conversion::fixed_index(edge_offset);
                      center
                    },
                  },
                  bd,
                )?
              };
              ({
                (*match <[_]>::get(&f.dsp.ipred.cfl_pred, {
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
                  ac,
                  CflPrediction {
                    mode: u64::from(m),
                    edge_center: {
                      let Ok(center) = console_core_number_conversion::fixed_index(edge_offset);
                      center
                    },
                    size: Size { width: i32::wrapping_mul(
                    {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w);
                      numeric_value
                    },
                    4,
                  ), height: i32::wrapping_mul(
                    {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h);
                      numeric_value
                    },
                    4,
                  ) },
                    alpha: {
                    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(
                      *match <[_]>::get(&intra.cfl_alpha, pl) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      },
                    );
                    numeric_value
                  },
                  },
                  bd,
                )?
              });
            }

            match (debug_block_info!(f, t.b), PIXEL_TRACE) {
              (BlockTrace::Enabled, BlockTrace::Enabled) => {
                coef_dump(
                  ac,
                  Size {
                    width: { let checked_value_7 = u64::try_from(cbw4).map_err(|_negative_extent| EINVAL)?; checked_value_7 }.wrapping_mul(4),
                    height: { let checked_value_8 = u64::try_from(cbh4).map_err(|_negative_extent| EINVAL)?; checked_value_8 }.wrapping_mul(4),
                  },
                  3,
                  "ac",
                ).map_err(|_dump_error| EIO)?;

                for pl in 1..3 {
                  let uv_dst = {
                    let explicit_value = planes.dst::<BD>({ let checked_value_25 = u8::try_from(pl).map_err(|_invalid_plane| EINVAL)?; checked_value_25 }, { let Ok(origin) = block_plane_origin(t.b, chroma_shifts); origin })?;
                    explicit_value
                  };
                  hex_dump_pic::<BD>(
                    &uv_dst,
                    Size {
                      width: { let checked_value_9 = u64::try_from(cbw4).map_err(|_negative_extent| EINVAL)?; checked_value_9 }.wrapping_mul(4),
                      height: { let checked_value_10 = u64::try_from(cbh4).map_err(|_negative_extent| EINVAL)?; checked_value_10 }.wrapping_mul(4),
                    },
                    *match <[_]>::get(&(["", "u-cfl-pred", "v-cfl-pred"]), pl) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => {
                        return ::core::result::Result::Err(
                          crate::src::error::Rav1dError::EINVAL,
                        );
                      }
                    },
                  ).map_err(|_dump_error| EIO)?;
                }
              }
              (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
            }
          }
          ::core::option::Option::None => match ::core::primitive::bool::then_some((match intra.pal_sz {
            [_, element, ..] => element,
          }) != 0, ())
          {
            ::core::option::Option::Some(()) => {
              let Ok(uv_origin) = block_plane_origin(t.b, chroma_shifts);
              let frame_palette;
              let (pal, pal_idx) = match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
                ::core::option::Option::Some(()) => {
                  let Ok(p) =
                    console_core_number_conversion::wrapped_index(t.frame_thread.pass & 1);
                  let stride = i64::from(f.b4_stride);
                  let position = palette_cache::PalettePosition { x: t.b.x, y: t.b.y };
                  let record = palette_cache::record_index(position, stride)?;
                  let Ok(len) = console_core_number_conversion::wrapped::<_, u32>(
                    i32::wrapping_mul(i32::wrapping_mul(cbw4, cbh4), 8),
                  );
                  let Ok(pal_idx_offset) = (*match <[_]>::get(&ts.frame_thread, p) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => {
                      return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                    }
                  })
                  .pal_idx
                  .get_update(|i| u32::wrapping_add(i, len));
                  frame_palette = f.frame_thread.pal.checked_index::<BD>(record)?;
                  (
                    &*frame_palette,
                    &*f.frame_thread.pal_idx.index((
                      ({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(pal_idx_offset);
                        numeric_value
                      })..,
                      ..({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(len);
                        numeric_value
                      }),
                    )),
                  )
                }
                ::core::option::Option::None => {
                  let scratch = t.scratch.inter_intra_mut()?;
                  (
                    ({
                      let explicit_value = scratch.interintra_edge_pal.pal.buf::<BD>()?;
                      explicit_value
                    }),
                    scratch.pal_idx_uv.as_slice(),
                  )
                }
              };

              for pl in 1..3 {
                let mut uv = planes.dst::<BD>({ let checked_value_26 = u8::try_from(pl).map_err(|_invalid_plane| EINVAL)?; checked_value_26 }, uv_origin)?;
                ({
                  f.dsp.ipred.pal_pred.call::<BD>(
                    &mut uv,
                    &*match <[_]>::get(&*pal, pl) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => {
                        return ::core::result::Result::Err(
                          crate::src::error::Rav1dError::EINVAL,
                        );
                      }
                    },
                    pal_idx,
                    Size { width: i32::wrapping_mul(cbw4, 4), height: i32::wrapping_mul(cbh4, 4) },
                  )?
                });

                match (debug_block_info!(f, t.b), PIXEL_TRACE) {
                  (BlockTrace::Enabled, BlockTrace::Enabled) => {
                    hex_dump_pic::<BD>(
                      &uv,
                      Size {
                        width: { let checked_value_11 = u64::try_from(cbw4).map_err(|_negative_extent| EINVAL)?; checked_value_11 }.wrapping_mul(4),
                        height: { let checked_value_12 = u64::try_from(cbh4).map_err(|_negative_extent| EINVAL)?; checked_value_12 }.wrapping_mul(4),
                      },
                      *match <[_]>::get(&(["", "u-pal-pred", "v-pal-pred"]), pl) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      },
                    ).map_err(|_dump_error| EIO)?;
                  }
                  (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
                }
              }
            }
            ::core::option::Option::None => {}
          },
        }

        let above_sm_uv = sm_uv_flag(
          &*match f.above_context(t.above_index) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
            }
          },
          cbx4,
        )?;
        let left_sm_2 = sm_uv_flag(&t.l, cby4)?;
        let sm_uv_fl = above_sm_uv | left_sm_2;
        let uv_sb_has_tr = match ::core::primitive::bool::then_some(i32::wrapping_shr(i32::wrapping_add(init_x, 16), {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
          numeric_value
        }) < cw4, ())
        {
          ::core::option::Option::Some(()) => true,
          ::core::option::Option::None => match ::core::primitive::bool::then_some(init_y != 0, ()) {
            ::core::option::Option::Some(()) => false,
            ::core::option::Option::None => matches!(intra_edge_flags.availability(EdgeFlags::I420_TOP_HAS_RIGHT >> f.cur.p.layout), Ok(EdgeAvailability::Present)),
          },
        };
        let uv_sb_has_bl = match ::core::primitive::bool::then_some(init_x != 0, ()) {
          ::core::option::Option::Some(()) => false,
          ::core::option::Option::None => match ::core::primitive::bool::then_some(i32::wrapping_shr(i32::wrapping_add(init_y, 16), {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
            numeric_value
          }) < ch4, ())
          {
            ::core::option::Option::Some(()) => true,
            ::core::option::Option::None => matches!(intra_edge_flags.availability(EdgeFlags::I420_LEFT_HAS_BOTTOM >> f.cur.p.layout), Ok(EdgeAvailability::Present)),
          },
        };
        let sub_cw4 = cmp::min(
          cw4,
          i32::wrapping_shr(i32::wrapping_add(init_x, 16), {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
            numeric_value
          }),
        );

        for pl in 0..2 {
          y = i32::wrapping_shr(init_y, {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
            numeric_value
          });

          {
            let numeric_amount = init_y;
            let numeric_target = &mut (t.b.y);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };

          while y < sub_ch4 {
            x = i32::wrapping_shr(init_x, {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
              numeric_value
            });

            {
              let numeric_amount = init_x;
              let numeric_target = &mut (t.b.x);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };

            while x < sub_cw4 {
              let mut uv_dst = {
                let explicit_value = planes.dst::<BD>({ let checked_value_27 = u8::try_from(pl).map_err(|_invalid_plane| EINVAL)?; checked_value_27 }.wrapping_add(1), { let Ok(origin) = block_plane_origin(t.b, chroma_shifts); origin })?;
                explicit_value
              };
              let mut angle;
              let edge_flags;
              let uv_mode;
              let xpos;
              let ypos;
              let xstart;
              let ystart;
              let m;

              match ::core::primitive::bool::then_some(!(intra.uv_mode == CFL_PRED
                && *match <[_]>::get(&intra.cfl_alpha, pl) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                } != 0
                || (match intra.pal_sz {
                  [_, element, ..] => element,
                }) != 0), ())
              {
                ::core::option::Option::Some(()) => {
                  angle = {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, i32>(intra.uv_angle);
                    numeric_value
                  };

                  edge_flags = ({
                    let Ok(explicit_value) = EdgeFlags::I444_TOP_HAS_RIGHT.select(
                      EdgeAvailability::from(!((y
                        > i32::wrapping_shr(init_y, {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                          numeric_value
                        })
                        || !uv_sb_has_tr)
                        && i32::wrapping_add(x, {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w);
                          numeric_value
                        }) >= sub_cw4)),
                    );
                    explicit_value
                  }) | ({
                    let Ok(explicit_value) = EdgeFlags::I444_LEFT_HAS_BOTTOM.select(
                      EdgeAvailability::from(!(x
                        > i32::wrapping_shr(init_x, {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                          numeric_value
                        })
                        || !uv_sb_has_bl
                          && i32::wrapping_add(y, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h);
                            numeric_value
                          }) >= sub_ch4)),
                    );
                    explicit_value
                  });
                  let chroma_plane = u8::try_from(pl).map_err(|_invalid_plane| EINVAL)?;
                  let saved_row = PredictionEdgeRow { block_y: t.b.y, chroma_rounding: ss_ver, plane: chroma_plane.wrapping_add(1) };
                  let prefiltered = prefiltered_top(f, saved_row)?;
                  uv_mode = match ::core::primitive::bool::then_some(intra.uv_mode == CFL_PRED, ()) {
                    ::core::option::Option::Some(()) => DC_PRED,
                    ::core::option::Option::None => intra.uv_mode,
                  };
                  xpos = i32::wrapping_shr(t.b.x, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                    numeric_value
                  });
                  ypos = i32::wrapping_shr(t.b.y, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                    numeric_value
                  });
                  xstart = i32::wrapping_shr(ts.tiling.col_start, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                    numeric_value
                  });
                  ystart = i32::wrapping_shr(ts.tiling.row_start, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ss_ver);
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
                  let edge_offset = 128;
                  m = {
                    rav1d_prepare_intra_edges(
                      IntraEdgePreparation {
                        position: Point { x: xpos, y: ypos },
                        frame: Size { width: i32::wrapping_shr(ts.tiling.col_end, {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                        numeric_value
                      }), height: i32::wrapping_shr(ts.tiling.row_end, {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                        numeric_value
                      }) },
                        transform: Size { width: {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w);
                        numeric_value
                      }, height: {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h);
                        numeric_value
                      } },
                        has_left: xpos > xstart,
                        above: {
                          let Ok(above) = AboveEdge::select(EdgeAvailability::from(ypos > ystart), prefiltered);
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
                        center: {
                          let Ok(center) = console_core_number_conversion::fixed_index(edge_offset);
                          center
                        },
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
                        return ::core::result::Result::Err(
                          crate::src::error::Rav1dError::EINVAL,
                        );
                      }
                    })
                    .call(
                      &mut uv_dst,
                      edge_array,
                      IntraPrediction {
                        mode: u64::from(m),
                        edge_center: {
                          let Ok(center) = console_core_number_conversion::fixed_index(edge_offset);
                          center
                        },
                        size: Size { width: i32::wrapping_mul(
                        {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w);
                          numeric_value
                        },
                        4,
                      ), height: i32::wrapping_mul(
                        {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h);
                          numeric_value
                        },
                        4,
                      ) },
                        reach: Size { width: i32::wrapping_shr(
                        i32::wrapping_sub(
                          i32::wrapping_add(i32::wrapping_mul(4, f.bw), ss_hor),
                          i32::wrapping_mul(4, t.b.x & !ss_hor),
                        ),
                        {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                          numeric_value
                        },
                      ), height: i32::wrapping_shr(
                        i32::wrapping_sub(
                          i32::wrapping_add(i32::wrapping_mul(4, f.bh), ss_ver),
                          i32::wrapping_mul(4, t.b.y & !ss_ver),
                        ),
                        {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                          numeric_value
                        },
                      ) },
                        angle: angle | sm_uv_fl,
                      },
                      bd,
                    )?
                  });

                  match (debug_block_info!(f, t.b), PIXEL_TRACE) {
                    (BlockTrace::Enabled, BlockTrace::Enabled) => {
                      hex_dump::<BD>(
                        &*match <[_]>::get(
                          &*edge_array,
                          (::core::num::Wrapping(edge_offset)
                            - ::core::num::Wrapping(
                              ({
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(uv_t_dim.h);
                                numeric_value
                              })
                              .wrapping_mul(4),
                            ))
                          .0..,
                        ) {
                          ::core::option::Option::Some(indexed_value) => indexed_value,
                          ::core::option::Option::None => {
                            return ::core::result::Result::Err(
                              crate::src::error::Rav1dError::EINVAL,
                            );
                          }
                        },
                        Size {
                          width: u64::from(uv_t_dim.h).wrapping_mul(4),
                          height: 2,
                        },
                        u64::from(uv_t_dim.h).wrapping_mul(4),
                        "l",
                      ).map_err(|_dump_error| EIO)?;
                      hex_dump::<BD>(
                        &*match <[_]>::get(&*edge_array, edge_offset..) {
                          ::core::option::Option::Some(indexed_value) => indexed_value,
                          ::core::option::Option::None => {
                            return ::core::result::Result::Err(
                              crate::src::error::Rav1dError::EINVAL,
                            );
                          }
                        },
                        Size {
                          width: 1,
                          height: 1,
                        },
                        0,
                        "tl",
                      ).map_err(|_dump_error| EIO)?;
                      hex_dump::<BD>(
                        &*match <[_]>::get(&*edge_array, usize::wrapping_add(edge_offset, 1)..) {
                          ::core::option::Option::Some(indexed_value) => indexed_value,
                          ::core::option::Option::None => {
                            return ::core::result::Result::Err(
                              crate::src::error::Rav1dError::EINVAL,
                            );
                          }
                        },
                        Size {
                          width: u64::from(uv_t_dim.w).wrapping_mul(4),
                          height: 2,
                        },
                        u64::from(uv_t_dim.w).wrapping_mul(4),
                        "t",
                      ).map_err(|_dump_error| EIO)?;
                      hex_dump_pic::<BD>(
                        &uv_dst,
                        Size {
                          width: u64::from(uv_t_dim.w).wrapping_mul(4),
                          height: u64::from(uv_t_dim.h).wrapping_mul(4),
                        },
                        *match <[_]>::get(&(["u-intra-pred", "v-intra-pred"]), pl) {
                          ::core::option::Option::Some(indexed_value) => indexed_value,
                          ::core::option::Option::None => {
                            return ::core::result::Result::Err(
                              crate::src::error::Rav1dError::EINVAL,
                            );
                          }
                        },
                      ).map_err(|_dump_error| EIO)?;
                    }
                    (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
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
                      let Ok(p) =
                        console_core_number_conversion::wrapped_index(t.frame_thread.pass & 1);
                      let len = u32::wrapping_mul(
                        u32::wrapping_mul(
                          u32::wrapping_mul(
                            {
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped::<_, u32>(uv_t_dim.w);
                              numeric_value
                            },
                            4,
                          ),
                          {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(uv_t_dim.h);
                            numeric_value
                          },
                        ),
                        4,
                      );
                      let Ok(cf_idx) = (*match <[_]>::get(&ts.frame_thread, p) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      })
                      .cf
                      .get_update(|i| u32::wrapping_add(i, len));
                      cf_guard = f.frame_thread.cf.mut_slice_as((
                        ({
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(cf_idx);
                          numeric_value
                        })..,
                        ..({
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(len);
                          numeric_value
                        }),
                      ));
                      cf = &mut *cf_guard;
                      let Ok(cbi_idx) = (*match <[_]>::get(&ts.frame_thread, p) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      })
                      .cbi_idx
                      .get_update(|i| u32::wrapping_add(i, 1));
                      let Ok(cbi) = (*match <[_]>::get(&f.frame_thread.cbi, {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(cbi_idx);
                        numeric_value
                      }) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      })
                      .get();
                      eob = <::core::primitive::i32 as ::core::convert::From<
                        ::core::primitive::i16,
                      >>::from({
                        let Ok(explicit_value) = cbi.eob();
                        explicit_value
                      });
                      txtp = {
                        let Ok(explicit_value) = cbi.txtp();
                        explicit_value
                      };
                    }
                    ::core::option::Option::None => {
                      let mut cf_ctx: u8 = 0;
                      let Ok(a_start) =
                        console_core_number_conversion::wrapped_index(i32::wrapping_add(cbx4, x));
                      let a_ccoef = &*match <[_]>::get(
                        &(*match f.above_context(t.above_index) {
                          ::core::option::Option::Some(indexed_value) => indexed_value,
                          ::core::option::Option::None => {
                            return ::core::result::Result::Err(
                              crate::src::error::Rav1dError::EINVAL,
                            );
                          }
                        })
                        .ccoef,
                        pl,
                      ) {
                        ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      };
                      let Ok(l_start) =
                        console_core_number_conversion::wrapped_index(i32::wrapping_add(cby4, y));
                      let l_ccoef = &mut *match <[_]>::get_mut(&mut t.l.ccoef, pl) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      };
                      eob = {
                        decode_coefs::<BD>(
                          f,
                          CoefficientDecoder {
                            tile: ts,
                            state: ts_c.as_deref_mut().ok_or(EINVAL)?,
                            scratch: &mut t.scratch,
                            coefficients: &mut t.cf,
                          },
                          CoefficientBlock {
                            trace: debug_block_info!(f, t.b),
                            above: &a_ccoef.index(
                              a_start..a_start.wrapping_add({
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(uv_t_dim.w);
                                numeric_value
                              }),
                            ),
                            left: &*match <[_]>::get(
                              &**l_ccoef.get_mut(),
                              l_start..l_start.wrapping_add({
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(uv_t_dim.h);
                                numeric_value
                              }),
                            ) {
                              ::core::option::Option::Some(indexed_value_3) => indexed_value_3,
                              ::core::option::Option::None => {
                                return ::core::result::Result::Err(
                                  crate::src::error::Rav1dError::EINVAL,
                                );
                              }
                            },
                            transform: b.uvtx,
                            size: bs,
                            syntax: b,
                            plane: { let checked_value_23 = u8::try_from(pl).map_err(|_invalid_plane| EINVAL)?; checked_value_23 }.wrapping_add(1),
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

                      match debug_block_info!(f, t.b) {
                        BlockTrace::Enabled => {
                          println!(
                            "Post-uv-cf-blk[pl={},tx={:?},txtp={},eob={}]: r={} [x={},cbx4={}]",
                            pl,
                            b.uvtx,
                            txtp,
                            eob,
                            ts_c.as_deref().ok_or(EINVAL)?.msac.rng,
                            x,
                            cbx4,
                          );
                        }
                        BlockTrace::Quiet => {}
                      }

                      case_set_al! {
                          <16, true>
                          l: (
                              &mut t.l,
                              { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cmp::min({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h); numeric_value }, i32::wrapping_shr(i32::wrapping_add(i32::wrapping_sub(f.bh, t.b.y), ss_ver), { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver); numeric_value }))); numeric_value },
                              { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(i32::wrapping_add(cby4, y)); numeric_value }
                          ),
                          a: (
                              above_context,
                              { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cmp::min({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w); numeric_value }, i32::wrapping_shr(i32::wrapping_add(i32::wrapping_sub(f.bw, t.b.x), ss_hor), { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor); numeric_value }))); numeric_value },
                              { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(i32::wrapping_add(cbx4, x)); numeric_value }
                          ),
                          ccoef[pl] = (cf_ctx, cf_ctx),
                      }
                    }
                  }

                  match ::core::primitive::bool::then_some(eob >= 0, ()) {
                    ::core::option::Option::Some(()) => {
                      match (debug_block_info!(f, t.b), PIXEL_TRACE) {
                        (BlockTrace::Enabled, BlockTrace::Enabled) => {
                          coef_dump(
                            cf,
                            Size {
                              width: u64::from(uv_t_dim.h).wrapping_mul(4),
                              height: u64::from(uv_t_dim.w).wrapping_mul(4),
                            },
                            3,
                            "dq",
                          ).map_err(|_dump_error| EIO)?;
                        }
                        (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
                      }

                      f.dsp
                        .itx
                        .add::<BD>(b.uvtx, txtp, &mut uv_dst, cf, eob, bd)?;

                      match (debug_block_info!(f, t.b), PIXEL_TRACE) {
                        (BlockTrace::Enabled, BlockTrace::Enabled) => {
                          hex_dump_pic::<BD>(
                            &uv_dst,
                            Size {
                              width: u64::from(uv_t_dim.w).wrapping_mul(4),
                              height: u64::from(uv_t_dim.h).wrapping_mul(4),
                            },
                            "recon",
                          ).map_err(|_dump_error| EIO)?;
                        }
                        (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
                      }
                    }
                    ::core::option::Option::None => {}
                  }
                }
                ::core::option::Option::None => match ::core::primitive::bool::then_some(t.frame_thread.pass == 0, ()) {
                  ::core::option::Option::Some(()) => {
                    case_set_al! {
                        <16, false>
                        l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(uv_t_dim.h); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(i32::wrapping_add(cby4, y)); numeric_value }),
                        a: (above_context, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(uv_t_dim.w); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(i32::wrapping_add(cbx4, x)); numeric_value }),
                        ccoef[pl] = (0x40, 0x40),
                    }
                  }
                  ::core::option::Option::None => {}
                },
              }

              {
                let numeric_amount = {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w);
                  numeric_value
                };
                let numeric_target = &mut (x);
                *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
              };

              {
                let numeric_amount = i32::wrapping_shl(
                  {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.w);
                    numeric_value
                  },
                  {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                    numeric_value
                  },
                );
                let numeric_target = &mut (t.b.x);
                *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
              };
            }

            {
              let numeric_amount = i32::wrapping_shl(x, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                numeric_value
              });
              let numeric_target = &mut (t.b.x);
              *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
            };

            {
              let numeric_amount = {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h);
                numeric_value
              };
              let numeric_target = &mut (y);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };

            {
              let numeric_amount = i32::wrapping_shl(
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(uv_t_dim.h);
                  numeric_value
                },
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                  numeric_value
                },
              );
              let numeric_target = &mut (t.b.y);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };
          }

          {
            let numeric_amount = i32::wrapping_shl(y, {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
              numeric_value
            });
            let numeric_target = &mut (t.b.y);
            *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
          };
        }
      }
    }
  };

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
  dav1d_wedge_masks.get(block_index)
    .and_then(|block| block.get(layout_index))
    .and_then(|layout| layout.get(polarity_index))
    .and_then(|variants| variants.get(wedge_index))
    .copied().ok_or(())
}

fn interintra_mask(block: BlockSize, layout: u8, mode: InterIntraPredMode) -> Result<&'static [u8], ()> {
  let Ok(block_ordinal) = block.ordinal();
  let Ok(mode_ordinal) = mode.ordinal();
  let Ok(block_index) = console_core_number_conversion::index(block_ordinal);
  let Ok(layout_index) = console_core_number_conversion::index(layout);
  let Ok(mode_index) = console_core_number_conversion::index(mode_ordinal);
  dav1d_ii_masks.get(block_index)
    .and_then(|block_masks| block_masks.get(layout_index))
    .and_then(|modes| modes.get(mode_index))
    .copied().ok_or(())
}

pub(crate) fn rav1d_recon_b_inter<BD: BitDepth>(
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
  mut ts_c: Option<&mut Rav1dTileStateContext>,
  bs: BlockSize,
  b: &Av1Block,
  inter: &Av1BlockInter,
) -> Result<(), ()> {
  let above_context = f.above_context(t.above_index).ok_or(())?;
  let bd = BD::from_c(f.bitdepth_max);
  let cur_data = &f.cur.data.as_ref().ok_or(())?.data;

  let ts = &*match f.tile(t.tile_index) {
    ::core::option::Option::Some(indexed_value) => indexed_value,
    ::core::option::Option::None => return ::core::result::Result::Err(()),
  };
  let bx4 = t.b.x & 31;
  let by4 = t.b.y & 31;
  let ss_ver = i32::from(f.cur.p.layout == Rav1dPixelLayout::I420);
  let ss_hor = i32::from(f.cur.p.layout != Rav1dPixelLayout::I444);
  let cbx4 = i32::wrapping_shr(bx4, {
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
    numeric_value
  });
  let cby4 = i32::wrapping_shr(by4, {
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
    numeric_value
  });
  let Ok(b_dim) = bs.dimensions();
  let Ok(bw4) = console_core_number_conversion::wrapped::<_, i32>(match b_dim {
    &[element, ..] => element,
  });
  let Ok(bh4) = console_core_number_conversion::wrapped::<_, i32>(match b_dim {
    &[_, element, ..] => element,
  });
  let w4 = cmp::min(bw4, i32::wrapping_sub(f.bw, t.b.x));
  let h4 = cmp::min(bh4, i32::wrapping_sub(f.bh, t.b.y));
  let has_chroma = f.cur.p.layout != Rav1dPixelLayout::I400
    && (bw4 > ss_hor || t.b.x & 1 != 0)
    && (bh4 > ss_ver || t.b.y & 1 != 0);
  let chroma_layout = match f.cur.p.layout {
    Rav1dPixelLayout::I400 | Rav1dPixelLayout::I444 => Rav1dPixelLayoutSubSampled::I444,
    Rav1dPixelLayout::I420 => Rav1dPixelLayoutSubSampled::I420,
    Rav1dPixelLayout::I422 => Rav1dPixelLayoutSubSampled::I422,
  };
  let mask_layout = match chroma_layout {
    Rav1dPixelLayoutSubSampled::I444 => 0_u8,
    Rav1dPixelLayoutSubSampled::I422 => 1,
    Rav1dPixelLayoutSubSampled::I420 => 2,
  };

  let cbh4 = i32::wrapping_shr(i32::wrapping_add(bh4, ss_ver), {
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
    numeric_value
  });
  let cbw4 = i32::wrapping_shr(i32::wrapping_add(bw4, ss_hor), {
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
    numeric_value
  });
  let [luma, ..] = cur_data;
  let y_position = Point { x: t.b.x.wrapping_mul(4), y: t.b.y.wrapping_mul(4) };
  let mut y_dst = motion_picture_position::<BD>(luma.with_offset::<BD>(), y_position)?;
  let shifts = Point { x: u32::from(ss_hor != 0), y: u32::from(ss_ver != 0) };
  let Ok(chroma_origin) = block_plane_origin(t.b, shifts);
  let chroma_destination = |plane: u8, displacement: Point<i32>| {
    let Ok(index) = console_core_number_conversion::index(plane);
    let component = cur_data.get(index).ok_or(())?;
    let position = Point {
      x: chroma_origin.x.wrapping_add(displacement.x),
      y: chroma_origin.y.wrapping_add(displacement.y),
    };
    motion_picture_position::<BD>(component.with_offset::<BD>(), position)
  };
  let frame_hdr = &***f.frame_hdr.as_ref().ok_or(())?;

  match ::core::primitive::bool::then_some(frame_hdr.frame_type.is_key_or_intra(), ()) {
    ::core::option::Option::Some(()) => {
      assert!(!frame_hdr.size.super_res.enabled);
      let scratch = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
      mc::<BD>(
        f,
        &mut scratch.emu_edge,
        MaybeTempPixels::NonTemp { dst: y_dst },
        MotionPrediction {
          position: t.b,
          size: Size { width: bw4, height: bh4 },
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
              MaybeTempPixels::NonTemp {
                dst: destination,
              },
              MotionPrediction {
                position: t.b,
                size: Size { width: i32::wrapping_shl(bw4, {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u32>(i32::from(bw4 == ss_hor));
                numeric_value
              }), height: i32::wrapping_shl(bh4, {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u32>(i32::from(bh4 == ss_ver));
                numeric_value
              }) },
                origin: Point { x: t.b.x & !ss_hor, y: t.b.y & !ss_ver },
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
    }
    ::core::option::Option::None => match inter.comp_type {
      Some(comp_inter_type) => {
        let filter_2d = inter.filter2d;

        let scratch = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
        let scratch_inter = scratch.lap_inter.inter_mut().map_err(|_invalid_scratch| ())?;
        let tmp = &mut scratch_inter.compinter;
        let mut jnt_weight = 0;
        let seg_mask = &mut scratch_inter.seg_mask;

        for i in 0..2 {
          let refp = &*match <[_]>::get(&f.refp, {
            let Ok(numeric_value) =
              console_core_number_conversion::wrapped_index(*match <[_]>::get(&inter.r#ref, i) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              });
            numeric_value
          }) {
            ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
            ::core::option::Option::None => return ::core::result::Result::Err(()),
          };

          match ::core::primitive::bool::then_some(inter.inter_mode == GLOBALMV_GLOBALMV
            && *match <[_]>::get(&f.gmv_warp_allowed, {
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                *match <[_]>::get(&inter.r#ref, i) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                },
              );
              numeric_value
            }) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            } != 0, ())
          {
            ::core::option::Option::Some(()) => {
              warp_affine::<BD>(
                f,
                &mut scratch.emu_edge,
                MaybeTempPixels::Temp {
                  tmp: &mut *match <[_]>::get_mut(&mut *tmp, i) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                  tmp_stride: u64::from(match b_dim { &[_explicit_element, ..] => _explicit_element }).wrapping_mul(4),
                },
                WarpPrediction {
                  position: t.b,
                  dimensions: *b_dim,
                  plane: 0,
                  reference: refp,
                  parameters: &*match <[_]>::get(&frame_hdr.gmv, {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                    *match <[_]>::get(&inter.r#ref, i) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                  );
                  numeric_value
                }) {
                  ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                },
                },
              )?;
            }
            ::core::option::Option::None => {
              let &reference_code = inter.r#ref.get(i).ok_or(())?;
              let motion_reference = u8::try_from(reference_code).map_err(|_invalid_reference| ())?;
              mc::<BD>(
                f,
                &mut scratch.emu_edge,
                MaybeTempPixels::Temp {
                  tmp: &mut *match <[_]>::get_mut(&mut *tmp, i) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                  tmp_stride: (),
                },
                MotionPrediction {
                  position: t.b,
                  size: Size { width: bw4, height: bh4 },
                  origin: Point { x: t.b.x, y: t.b.y },
                  plane: 0,
                  motion: *match <[_]>::get(&inter.nd.one_d.mv, i) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                  reference: refp,
                  reference_index: motion_reference,
                  filter: filter_2d,
                },
              )?;
            }
          }
        }

        let mut mask = &*match <[_]>::get(&([]), ..) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => return ::core::result::Result::Err(()),
        };

        match comp_inter_type {
          CompInterType::Avg => {
            let [first, second] = &*tmp;
            let predictions = [first, second];
            let extent = Size { width: bw4.wrapping_mul(4), height: bh4.wrapping_mul(4) };
            f.dsp.mc.avg.call::<BD>(y_dst, predictions, extent, bd)
              .map_err(|_| ())?;
          }
          CompInterType::WeightedAvg => {
            jnt_weight = {
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped::<_, i32>(*match <[_]>::get(
                  &(*match <[_]>::get(&f.jnt_weights, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped_index(match inter.r#ref {
                        [element, ..] => element,
                      });
                    numeric_value
                  }) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  }),
                  {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped_index(match inter.r#ref {
                        [_, element, ..] => element,
                      });
                    numeric_value
                  },
                ) {
                  ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                });
              numeric_value
            };
            let [first, second] = &*tmp;
            let predictions = [first, second];
            let extent = Size { width: bw4.wrapping_mul(4), height: bh4.wrapping_mul(4) };
            f.dsp.mc.w_avg.call::<BD>(y_dst, predictions, extent, jnt_weight, bd)
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
            let checked_value_2 = f.dsp.mc.w_mask.checked(chroma_layout).map_err(|_invalid_dispatch| ())?;
            checked_value_2
              .call(
                chroma_layout,
                y_dst,
                predictions,
                Size { width: bw4.wrapping_mul(4), height: bh4.wrapping_mul(4) },
                seg_mask,
                polarity,
                bd,
              )
              .map_err(|_invalid_mask| ())?;
            mask = &*seg_mask;
          }
          CompInterType::Wedge => {
            let lookup = WedgeMaskLookup {
              block: bs, layout: 0, polarity: crate::src::levels::MaskPolarity::Normal, wedge: inter.nd.one_d.wedge_idx,
            };
            let checked_value_4 = wedge_mask(lookup)?;
            mask = checked_value_4;
            let [first, second] = &*tmp;
            let Ok(wedge_polarity) = inter.nd.one_d.mask_polarity();
            let predictions = match wedge_polarity {
              crate::src::levels::MaskPolarity::Normal => [first, second],
              crate::src::levels::MaskPolarity::Inverted => [second, first],
            };
            let extent = Size { width: bw4.wrapping_mul(4), height: bh4.wrapping_mul(4) };
            f.dsp.mc.mask.call::<BD>(y_dst, predictions, extent, mask, bd)
              .map_err(|_| ())?;

            match ::core::primitive::bool::then_some(has_chroma, ()) {
              ::core::option::Option::Some(()) => {
                let lookup = WedgeMaskLookup {
                  block: bs, layout: mask_layout, polarity: wedge_polarity, wedge: inter.nd.one_d.wedge_idx,
                };
                let checked_value_7 = wedge_mask(lookup)?;
                mask = checked_value_7;
              }
              ::core::option::Option::None => {}
            }
          }
        }

        match ::core::primitive::bool::then_some(has_chroma, ()) {
          ::core::option::Option::Some(()) => {
            for pl in 0..2 {
              for i in 0..2 {
                let refp = &*match <[_]>::get(&f.refp, {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                    *match <[_]>::get(&inter.r#ref, i) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                  );
                  numeric_value
                }) {
                  ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                };

                match ::core::primitive::bool::then_some(inter.inter_mode == GLOBALMV_GLOBALMV
                  && cmp::min(cbw4, cbh4) > 1
                  && *match <[_]>::get(&f.gmv_warp_allowed, {
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                      *match <[_]>::get(&inter.r#ref, i) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                    );
                    numeric_value
                  }) {
                    ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  } != 0, ())
                {
                  ::core::option::Option::Some(()) => {
                    let warp_plane = u8::try_from(pl).map_err(|_invalid_plane| ())?;
                    warp_affine::<BD>(
                      f,
                      &mut scratch.emu_edge,
                      MaybeTempPixels::Temp {
                        tmp: &mut *match <[_]>::get_mut(&mut *tmp, i) {
                          ::core::option::Option::Some(indexed_value) => indexed_value,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        },
                        tmp_stride: u64::from(match b_dim { &[_explicit_element, ..] => _explicit_element }).wrapping_mul(4).wrapping_shr(u32::from(ss_hor != 0)),
                      },
                      WarpPrediction {
                        position: t.b,
                        dimensions: *b_dim,
                        plane: warp_plane.wrapping_add(1),
                        reference: refp,
                        parameters: &*match <[_]>::get(&frame_hdr.gmv, {
                        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                          *match <[_]>::get(&inter.r#ref, i) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None => {
                              return ::core::result::Result::Err(());
                            }
                          },
                        );
                        numeric_value
                      }) {
                        ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                      },
                    )?;
                  }
                  ::core::option::Option::None => {
                    let motion_plane = u8::try_from(pl).map_err(|_invalid_plane| ())?;
                    let &reference_code = inter.r#ref.get(i).ok_or(())?;
                    let motion_reference = u8::try_from(reference_code).map_err(|_invalid_reference| ())?;
                    mc::<BD>(
                      f,
                      &mut scratch.emu_edge,
                      MaybeTempPixels::Temp {
                        tmp: &mut *match <[_]>::get_mut(&mut *tmp, i) {
                          ::core::option::Option::Some(indexed_value) => indexed_value,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        },
                        tmp_stride: (),
                      },
                      MotionPrediction {
                        position: t.b,
                        size: Size { width: bw4, height: bh4 },
                        origin: Point { x: t.b.x, y: t.b.y },
                        plane: motion_plane.wrapping_add(1),
                        motion: *match <[_]>::get(&inter.nd.one_d.mv, i) {
                          ::core::option::Option::Some(indexed_value) => indexed_value,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        },
                        reference: refp,
                        reference_index: motion_reference,
                        filter: filter_2d,
                      },
                    )?;
                  }
                }
              }

              let destination_plane = u8::try_from(pl).map_err(|_invalid_plane| ())?;
              let uv_dst = chroma_destination(destination_plane.wrapping_add(1), Point { x: 0, y: 0 })?;

              match comp_inter_type {
                CompInterType::Avg => {
                  let [first, second] = &*tmp;
                  let predictions = [first, second];
                  let Ok(horizontal_shift) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                  let Ok(vertical_shift) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                  let extent = Size {
                    width: bw4.wrapping_mul(4).wrapping_shr(horizontal_shift),
                    height: bh4.wrapping_mul(4).wrapping_shr(vertical_shift),
                  };
                  f.dsp.mc.avg.call::<BD>(uv_dst, predictions, extent, bd)
                    .map_err(|_| ())?;
                }
                CompInterType::WeightedAvg => {
                  let [first, second] = &*tmp;
                  let predictions = [first, second];
                  let Ok(horizontal_shift) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                  let Ok(vertical_shift) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                  let extent = Size {
                    width: bw4.wrapping_mul(4).wrapping_shr(horizontal_shift),
                    height: bh4.wrapping_mul(4).wrapping_shr(vertical_shift),
                  };
                  f.dsp.mc.w_avg.call::<BD>(uv_dst, predictions, extent, jnt_weight, bd)
                    .map_err(|_| ())?;
                }
                CompInterType::Seg | CompInterType::Wedge => {
                  let [first, second] = &*tmp;
                  let predictions = match inter.nd.one_d.mask_polarity() {
                    Ok(crate::src::levels::MaskPolarity::Normal) => [first, second],
                    Ok(crate::src::levels::MaskPolarity::Inverted) => [second, first],
                  };
                  let Ok(horizontal_shift) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                  let Ok(vertical_shift) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                  let extent = Size {
                    width: bw4.wrapping_mul(4).wrapping_shr(horizontal_shift),
                    height: bh4.wrapping_mul(4).wrapping_shr(vertical_shift),
                  };
                  f.dsp.mc.mask.call::<BD>(uv_dst, predictions, extent, mask, bd)
                    .map_err(|_| ())?;
                }
              }
            }
          }
          ::core::option::Option::None => {}
        }
      }
      None => {
        let refp = &*match <[_]>::get(&f.refp, {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped_index(match inter.r#ref {
              [element, ..] => element,
            });
          numeric_value
        }) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => return ::core::result::Result::Err(()),
        };
        let filter_2d = inter.filter2d;

        match ::core::primitive::bool::then_some(cmp::min(bw4, bh4) > 1
          && (inter.inter_mode == GLOBALMV
            && *match <[_]>::get(&f.gmv_warp_allowed, {
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped_index(match inter.r#ref {
                  [element, ..] => element,
                });
              numeric_value
            }) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            } != 0
            || inter.motion_mode == MotionMode::Warp
              && t.warpmv.r#type > Rav1dWarpedMotionType::Translation), ())
        {
          ::core::option::Option::Some(()) => {
            warp_affine::<BD>(
              f,
              &mut ({
                let explicit_value = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
                explicit_value
              })
              .emu_edge,
              MaybeTempPixels::NonTemp { dst: y_dst },
              WarpPrediction {
                position: t.b,
                dimensions: *b_dim,
                plane: 0,
                reference: refp,
                parameters: match ::core::primitive::bool::then_some(inter.motion_mode == MotionMode::Warp, ()) {
                ::core::option::Option::Some(()) => &t.warpmv,
                ::core::option::Option::None => &*match <[_]>::get(&frame_hdr.gmv, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped_index(match inter.r#ref {
                      [element, ..] => element,
                    });
                  numeric_value
                }) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                },
              },
              },
            )?;
          }
          ::core::option::Option::None => {
            let [reference_code, _] = inter.r#ref;
            let motion_reference = u8::try_from(reference_code).map_err(|_invalid_reference| ())?;
            mc::<BD>(
              f,
              &mut ({
                let explicit_value = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
                explicit_value
              })
              .emu_edge,
              MaybeTempPixels::NonTemp { dst: y_dst },
              MotionPrediction {
                position: t.b,
                size: Size { width: bw4, height: bh4 },
                origin: Point { x: t.b.x, y: t.b.y },
                plane: 0,
                motion: match inter.nd.one_d.mv {
                  [_explicit_element, ..] => _explicit_element,
                },
                reference: refp,
                reference_index: motion_reference,
                filter: filter_2d,
              },
            )?;

            match ::core::primitive::bool::then_some(inter.motion_mode == MotionMode::Obmc, ()) {
              ::core::option::Option::Some(()) => {
                obmc::<BD>(
                  f, t, y_dst,
                  OverlapReconstruction {
                    visible: Size { width: w4, height: h4 },
                    dimensions: *b_dim,
                    context: Point { x: bx4, y: by4 },
                    plane: 0,
                  },
                )?;
              }
              ::core::option::Option::None => {}
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
            let tl_edge_array = interintra_edge_pal.edge.buf_mut::<BD>().map_err(|_invalid_scratch| ())?;
            let tl_edge_offset = 32;
            let Ok(get) = inter.nd.one_d.interintra_mode.get();
            let mut m = match get {
              InterIntraPredMode::Smooth => SMOOTH_PRED,
              mode => mode as IntraPredMode,
            };
            let mut angle = 0;
            let saved_row = PredictionEdgeRow { block_y: t.b.y, chroma_rounding: 0, plane: 0 };
            let prefiltered = prefiltered_top(f, saved_row).map_err(|_invalid_edge_row| ())?;
            let source_picture = crate::src::owned_recon::ReconDst::Pic(y_dst);
            let Ok(source_pixels) = source_picture.as_src();
            m = {
              rav1d_prepare_intra_edges(
                IntraEdgePreparation {
                  position: Point { x: t.b.x, y: t.b.y },
                  frame: Size { width: ts.tiling.col_end, height: ts.tiling.row_end },
                  transform: Size { width: bw4, height: bh4 },
                  has_left: t.b.x > ts.tiling.col_start,
                  above: {
                    let Ok(above) = AboveEdge::select(EdgeAvailability::from(t.b.y > ts.tiling.row_start), prefiltered);
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
                  center: {
                    let Ok(center) = console_core_number_conversion::fixed_index(tl_edge_offset);
                    center
                  },
                },
                bd,
              ).map_err(|_| ())?
            };
            let tmp = interintra_edge_pal.interintra.buf_mut::<BD>().map_err(|_invalid_scratch| ())?;
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
                  edge_center: {
                    let Ok(center) = console_core_number_conversion::fixed_index(tl_edge_offset);
                    center
                  },
                  size: Size { width: i32::wrapping_mul(bw4, 4), height: i32::wrapping_mul(bh4, 4) },
                  reach: Size { width: 0, height: 0 },
                  angle: 0,
                },
                bd,
              ).map_err(|_| ())?
            });
            #[cfg(not(feature = "c-ffi"))]
            tmp_component.copy_pixels_to::<BD>(tmp);
            let Ok(blend_mode) = inter.nd.one_d.interintra_mode.get();
            let ii_mask = match interintra_type {
              InterIntraType::Blend => interintra_mask(bs, 0, blend_mode)?,
              InterIntraType::Wedge => {
                let lookup = WedgeMaskLookup {
                  block: bs, layout: 0, polarity: crate::src::levels::MaskPolarity::Normal, wedge: inter.nd.one_d.wedge_idx,
                };
                wedge_mask(lookup)?
              }
            };
            f.dsp.mc.blend.call::<BD>(
              y_dst,
              tmp,
              Size { width: i32::wrapping_mul(bw4, 4), height: i32::wrapping_mul(bh4, 4) },
              ii_mask,
            ).map_err(|_| ())?;
          }
          None => {}
        }

        match ::core::primitive::bool::then_some(has_chroma, ()) {
          ::core::option::Option::Some(()) => {
            let extent = Size { width: bw4, height: bh4 };
            match f.chroma_motion_neighbors(t, extent).map_err(|_invalid_neighbor| ())? {
              Some(neighbors) => {
                let x_offset = i32::from(neighbors.left.is_some()).wrapping_mul(2);
                let y_offset = i32::from(neighbors.above.is_some()).wrapping_mul(2);
                let filter_at = |[horizontal, vertical]: [Rav1dFilterMode; 2]| {
                  let Ok(column) = console_core_number_conversion::index(u8::from(horizontal));
                  let Ok(row) = console_core_number_conversion::index(u8::from(vertical));
                  dav1d_filter_2d.get(row).and_then(|filters| filters.get(column)).copied().ok_or(())
                };
                let draw_neighbor =
                  |task: &mut Rav1dTaskContext, prediction: ChromaNeighborPrediction,
                   displacement: Point<i32>, local_filter: Filter2d| -> Result<(), ()> {
                    let Ok(reference) = console_core_number_conversion::index(prediction.reference);
                    let picture = f.refp.get(reference).ok_or(())?;
                    let filter = match task.frame_thread.pass {
                      2 => {
                        let address = f.coded_block_position(prediction.position).map_err(|_invalid_neighbor| ())?;
                        let Ok(index) = console_core_number_conversion::index(address);
                        let Ok(filter) = f.frame_thread.b.index(index).ii.filter2d();
                        filter
                      }
                      _ => local_filter,
                    };
                    let position = task.b;
                    let scratch = task.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
                    for plane in [1_u8, 2] {
                      let dst = chroma_destination(plane, displacement)?;
                      mc::<BD>(f, &mut scratch.emu_edge, MaybeTempPixels::NonTemp { dst }, MotionPrediction {
                        position,
                        size: extent,
                        origin: Point { x: prediction.position.x, y: prediction.position.y },
                        plane,
                        motion: prediction.vector,
                        reference: picture,
                        reference_index: prediction.reference,
                        filter,
                      })?;
                    }
                    Ok(())
                  };
                if let Some(diagonal) = neighbors.diagonal {
                  let filter = t.tl_4x4_filter;
                  draw_neighbor(t, diagonal, Point { x: 0, y: 0 }, filter)?;
                }
                if let Some(left) = neighbors.left {
                  let index = console_core_number_conversion::checked_index(by4).map_err(|_invalid_row| ())?;
                  let [horizontal, vertical] = &mut t.l.filter;
                  let filter = filter_at([
                    *horizontal.get_mut().get(index).ok_or(())?,
                    *vertical.get_mut().get(index).ok_or(())?,
                  ])?;
                  draw_neighbor(t, left, Point { x: 0, y: y_offset }, filter)?;
                }
                if let Some(above) = neighbors.above {
                  let index = console_core_number_conversion::checked_index(bx4).map_err(|_invalid_column| ())?;
                  let [horizontal, vertical] = &above_context.filter;
                  let filter = filter_at([*horizontal.index(index), *vertical.index(index)])?;
                  draw_neighbor(t, above, Point { x: x_offset, y: 0 }, filter)?;
                }

                let [reference_code, _] = inter.r#ref;
                let reference_index = u8::try_from(reference_code).map_err(|_invalid_reference| ())?;
                let [motion, _] = inter.nd.one_d.mv;
                let scratch = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
                let displacement = Point { x: x_offset, y: y_offset };
                for plane in [1_u8, 2] {
                  let dst = chroma_destination(plane, displacement)?;
                  mc::<BD>(f, &mut scratch.emu_edge, MaybeTempPixels::NonTemp { dst }, MotionPrediction {
                    position: t.b,
                    size: extent,
                    origin: Point { x: t.b.x, y: t.b.y },
                    plane,
                    motion,
                    reference: refp,
                    reference_index,
                    filter: filter_2d,
                  })?;
                }
              }
              ::core::option::Option::None => {
                match ::core::primitive::bool::then_some(cmp::min(cbw4, cbh4) > 1
                  && (inter.inter_mode == GLOBALMV
                    && *match <[_]>::get(&f.gmv_warp_allowed, {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped_index(match inter.r#ref {
                          [element, ..] => element,
                        });
                      numeric_value
                    }) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    } != 0
                    || inter.motion_mode == MotionMode::Warp
                      && t.warpmv.r#type > Rav1dWarpedMotionType::Translation), ())
                {
                  ::core::option::Option::Some(()) => {
                    for pl in 0..2 {
                      let warp_plane = u8::try_from(pl).map_err(|_invalid_plane| ())?;
                      let destination = chroma_destination(warp_plane.wrapping_add(1), Point { x: 0, y: 0 })?;
                      warp_affine::<BD>(
                        f,
                        &mut ({
                          let explicit_value = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
                          explicit_value
                        })
                        .emu_edge,
                        MaybeTempPixels::NonTemp {
                          dst: destination,
                        },
                        WarpPrediction {
                          position: t.b,
                          dimensions: *b_dim,
                          plane: warp_plane.wrapping_add(1),
                          reference: refp,
                          parameters: match ::core::primitive::bool::then_some(inter.motion_mode == MotionMode::Warp, ()) {
                          ::core::option::Option::Some(()) => &t.warpmv,
                          ::core::option::Option::None => &*match <[_]>::get(&frame_hdr.gmv, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_index(match inter.r#ref {
                                [element, ..] => element,
                              });
                            numeric_value
                          }) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None => {
                              return ::core::result::Result::Err(());
                            }
                          },
                        },
                        },
                      )?;
                    }
                  }
                  ::core::option::Option::None => {
                    for pl in 0..2 {
                      let motion_plane = u8::try_from(pl).map_err(|_invalid_plane| ())?;
                      let [reference_code, _] = inter.r#ref;
                      let motion_reference = u8::try_from(reference_code).map_err(|_invalid_reference| ())?;
                      let destination = chroma_destination(motion_plane.wrapping_add(1), Point { x: 0, y: 0 })?;
                      mc::<BD>(
                        f,
                        &mut ({
                          let explicit_value = t.scratch.inter_mut().map_err(|_invalid_scratch| ())?;
                          explicit_value
                        })
                        .emu_edge,
                        MaybeTempPixels::NonTemp {
                          dst: destination,
                        },
                        MotionPrediction {
                          position: t.b,
                          size: Size { width: i32::wrapping_shl(bw4, {
                          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                            i32::from(bw4 == ss_hor),
                          );
                          numeric_value
                        }), height: i32::wrapping_shl(bh4, {
                          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                            i32::from(bh4 == ss_ver),
                          );
                          numeric_value
                        }) },
                          origin: Point { x: t.b.x & !ss_hor, y: t.b.y & !ss_ver },
                          plane: motion_plane.wrapping_add(1),
                          motion: match inter.nd.one_d.mv {
                            [_explicit_element, ..] => _explicit_element,
                          },
                          reference: refp,
                          reference_index: motion_reference,
                          filter: filter_2d,
                        },
                      )?;
                      let destination_plane = u8::try_from(pl).map_err(|_invalid_plane| ())?;
                      let uv_dst = chroma_destination(destination_plane.wrapping_add(1), Point { x: 0, y: 0 })?;

                      match ::core::primitive::bool::then_some(inter.motion_mode == MotionMode::Obmc, ()) {
                        ::core::option::Option::Some(()) => {
                          let checked_value_3 = u8::try_from(pl).map_err(|_invalid_plane| ())?;
                          let plane = checked_value_3.wrapping_add(1);
                          obmc::<BD>(
                            f, t, uv_dst,
                            OverlapReconstruction {
                              visible: Size { width: w4, height: h4 },
                              dimensions: *b_dim,
                              context: Point { x: bx4, y: by4 },
                              plane,
                            },
                          )?;
                        }
                        ::core::option::Option::None => {}
                      }
                    }
                  }
                }

                match inter.interintra_type {
                  Some(interintra_type) => {
                    let Ok(blend_mode) = inter.nd.one_d.interintra_mode.get();
                    let ii_mask = match interintra_type {
                      InterIntraType::Blend => interintra_mask(bs, mask_layout, blend_mode)?,
                      InterIntraType::Wedge => {
                        let lookup = WedgeMaskLookup {
                          block: bs, layout: mask_layout, polarity: crate::src::levels::MaskPolarity::Normal, wedge: inter.nd.one_d.wedge_idx,
                        };
                        wedge_mask(lookup)?
                      }
                    };

                    for pl in 0..2 {
                      let interintra_edge_pal = &mut ({
                        let explicit_value = t.scratch.inter_intra_mut().map_err(|_invalid_scratch| ())?;
                        explicit_value
                      })
                      .interintra_edge_pal;
                      let tl_edge_array = interintra_edge_pal.edge.buf_mut::<BD>().map_err(|_invalid_scratch| ())?;
                      let tl_edge_offset = 32;
                      let Ok(get) = inter.nd.one_d.interintra_mode.get();
                      let mut m = match get {
                        InterIntraPredMode::Smooth => SMOOTH_PRED,
                        mode => mode as IntraPredMode,
                      };
                      let mut angle = 0;
                      let destination_plane = u8::try_from(pl).map_err(|_invalid_plane| ())?;
                      let uv_dst = chroma_destination(destination_plane.wrapping_add(1), Point { x: 0, y: 0 })?;
                      let chroma_plane = u8::try_from(pl).map_err(|_invalid_plane| ())?;
                      let saved_row = PredictionEdgeRow { block_y: t.b.y, chroma_rounding: 0, plane: chroma_plane.wrapping_add(1) };
                      let prefiltered = prefiltered_top(f, saved_row).map_err(|_invalid_edge_row| ())?;
                      let source_picture = crate::src::owned_recon::ReconDst::Pic(uv_dst);
                      let Ok(source_pixels) = source_picture.as_src();
                      m = {
                        rav1d_prepare_intra_edges(
                          IntraEdgePreparation {
                            position: Point { x: i32::wrapping_shr(t.b.x, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                            numeric_value
                          }), y: i32::wrapping_shr(t.b.y, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                            numeric_value
                          }) },
                            frame: Size { width: i32::wrapping_shr(ts.tiling.col_end, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                            numeric_value
                          }), height: i32::wrapping_shr(ts.tiling.row_end, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                            numeric_value
                          }) },
                            transform: Size { width: cbw4, height: cbh4 },
                            has_left: i32::wrapping_shr(t.b.x, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                            numeric_value
                          }) > i32::wrapping_shr(ts.tiling.col_start, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                            numeric_value
                          }),
                            above: {
                              let Ok(above) = AboveEdge::select(EdgeAvailability::from(i32::wrapping_shr(t.b.y, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                            numeric_value
                          }) > i32::wrapping_shr(ts.tiling.row_start, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                            numeric_value
                          })), prefiltered);
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
                            center: {
                              let Ok(center) = console_core_number_conversion::fixed_index(tl_edge_offset);
                              center
                            },
                          },
                          bd,
                        ).map_err(|_| ())?
                      };
                      let tmp = interintra_edge_pal.interintra.buf_mut::<BD>().map_err(|_invalid_scratch| ())?;
                      let tmp_component = Rav1dPictureDataComponent::wrap_buf::<BD>(
                        tmp,
                        (::core::num::Wrapping(4)
                          * ::core::num::Wrapping({
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_index(cbw4);
                            numeric_value
                          }))
                        .0,
                      );
                      ({
                        (*match <[_]>::get(&f.dsp.ipred.intra_pred, {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(m);
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
                            edge_center: {
                              let Ok(center) = console_core_number_conversion::fixed_index(tl_edge_offset);
                              center
                            },
                            size: Size { width: i32::wrapping_mul(cbw4, 4), height: i32::wrapping_mul(cbh4, 4) },
                            reach: Size { width: 0, height: 0 },
                            angle: 0,
                          },
                          bd,
                        ).map_err(|_| ())?
                      });
                      #[cfg(not(feature = "c-ffi"))]
                      tmp_component.copy_pixels_to::<BD>(tmp);
                      f.dsp.mc.blend.call::<BD>(
                        uv_dst,
                        tmp,
                        Size { width: i32::wrapping_mul(cbw4, 4), height: i32::wrapping_mul(cbh4, 4) },
                        ii_mask,
                      ).map_err(|_| ())?;
                    }
                  }
                  None => {}
                }
              }
            }
          }
          ::core::option::Option::None => {}
        }

        t.tl_4x4_filter = filter_2d;
      }
    },
  }

  match (debug_block_info!(f, t.b), PIXEL_TRACE) {
    (BlockTrace::Enabled, BlockTrace::Enabled) => {
      hex_dump_pic::<BD>(
        &crate::src::owned_recon::ReconDst::Pic(y_dst),
        Size {
          width: u64::from(match b_dim { &[element, ..] => element }).wrapping_mul(4),
          height: u64::from(match b_dim { &[_, element, ..] => element }).wrapping_mul(4),
        },
        "y-pred",
      ).map_err(|_dump_error| ())?;

      match ::core::primitive::bool::then_some(has_chroma, ()) {
        ::core::option::Option::Some(()) => {
          for pl in 1..3 {
            let destination_plane = u8::try_from(pl).map_err(|_invalid_plane| ())?;
            let uv_dst = chroma_destination(destination_plane, Point { x: 0, y: 0 })?;
            hex_dump_pic::<BD>(
              &crate::src::owned_recon::ReconDst::Pic(uv_dst),
              Size {
                width: { let checked_value_5 = u64::try_from(cbw4).map_err(|_negative_extent| ())?; checked_value_5 }.wrapping_mul(4),
                height: { let checked_value_6 = u64::try_from(cbh4).map_err(|_negative_extent| ())?; checked_value_6 }.wrapping_mul(4),
              },
              *match <[_]>::get(&(["", "u-pred", "v-pred"]), pl) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              },
            ).map_err(|_dump_error| ())?;
          }
        }
        ::core::option::Option::None => {}
      }
    }
    (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
  }

  let cw4 = i32::wrapping_shr(i32::wrapping_add(w4, ss_hor), {
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
    numeric_value
  });
  let ch4 = i32::wrapping_shr(i32::wrapping_add(h4, ss_ver), {
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
    numeric_value
  });

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

  let uvtx = &*match <[_]>::get(&dav1d_txfm_dimensions, { let Ok(table_index) = console_core_number_conversion::index({ let Ok(ordinal) = b.uvtx.ordinal(); ordinal }); table_index }) {
    ::core::option::Option::Some(indexed_value) => indexed_value,
    ::core::option::Option::None => return ::core::result::Result::Err(()),
  };
  let ytx = &*match <[_]>::get(&dav1d_txfm_dimensions, { let Ok(table_index) = console_core_number_conversion::index({ let Ok(ordinal) = inter.max_ytx.ordinal(); ordinal }); table_index }) {
    ::core::option::Option::Some(indexed_value) => indexed_value,
    ::core::option::Option::None => return ::core::result::Result::Err(()),
  };
  let tx_split = [
    ({
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(inter.tx_split0);
      numeric_value
    }),
    inter.tx_split1,
  ];

  for init_y in (0..bh4).step_by(16) {
    for init_x in (0..bw4).step_by(16) {
      let mut y_off = i32::from(init_y != 0);
      let mut y;
      y_dst += ({
        match y_dst.pixel_stride::<BD>() {
          ::core::result::Result::Ok(value__) => value__,
        }
      })
      .wrapping_mul(4)
      .wrapping_mul({
        let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(init_y);
        numeric_value
      });
      y = init_y;

      {
        let numeric_amount = init_y;
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      while y < cmp::min(h4, i32::wrapping_add(init_y, 16)) {
        let mut x;
        let mut x_off = i32::from(init_x != 0);
        x = init_x;

        {
          let numeric_amount = init_x;
          let numeric_target = &mut (t.b.x);
          *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
        };

        while x < cmp::min(w4, i32::wrapping_add(init_x, 16)) {
          read_coef_tree::<BD>(
            f,
            t,
            ts_c.as_deref_mut(),
            LumaCoefficientTree {
              size: bs,
              syntax: b,
              transform: inter.max_ytx,
              splits: tx_split,
              split_cell: Point { x: x_off, y: y_off },
              output: Some(y_dst + {
                let Ok(offset) = console_core_number_conversion::wrapped_index(x.wrapping_mul(4));
                offset
              }),
            },
          )
          .map_err(|_| ())?;

          {
            let numeric_amount = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(ytx.w);
              numeric_value
            };
            let numeric_target = &mut (t.b.x);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };

          {
            let numeric_amount = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(ytx.w);
              numeric_value
            };
            let numeric_target = &mut (x);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };

          {
            let numeric_amount = 1;
            let numeric_target = &mut (x_off);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };
        }

        y_dst += ({
          match y_dst.pixel_stride::<BD>() {
            ::core::result::Result::Ok(value__) => value__,
          }
        })
        .wrapping_mul(4)
        .wrapping_mul({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(ytx.h);
          numeric_value
        });

        {
          let numeric_amount = x;
          let numeric_target = &mut (t.b.x);
          *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
        };

        {
          let numeric_amount = {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(ytx.h);
            numeric_value
          };
          let numeric_target = &mut (t.b.y);
          *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
        };

        {
          let numeric_amount = {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(ytx.h);
            numeric_value
          };
          let numeric_target = &mut (y);
          *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
        };

        {
          let numeric_amount = 1;
          let numeric_target = &mut (y_off);
          *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
        };
      }

      y_dst -= ({
        match y_dst.pixel_stride::<BD>() {
          ::core::result::Result::Ok(value__) => value__,
        }
      })
      .wrapping_mul(4)
      .wrapping_mul({
        let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(y);
        numeric_value
      });

      {
        let numeric_amount = y;
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
      };

      match ::core::primitive::bool::then_some(has_chroma, ()) {
        ::core::option::Option::Some(()) => {
          for pl in 0..2 {
            let plane = u8::try_from(pl).map_err(|_invalid_plane| ())?;
            let row = init_y.wrapping_mul(4).wrapping_shr(shifts.y);
            let mut uv_dst = chroma_destination(plane.wrapping_add(1), Point { x: 0, y: row })?;
            y = i32::wrapping_shr(init_y, {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
              numeric_value
            });

            {
              let numeric_amount = init_y;
              let numeric_target = &mut (t.b.y);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };

            while y
              < cmp::min(
                ch4,
                i32::wrapping_shr(i32::wrapping_add(init_y, 16), {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                  numeric_value
                }),
              )
            {
              let mut x;
              x = i32::wrapping_shr(init_x, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                numeric_value
              });

              {
                let numeric_amount = init_x;
                let numeric_target = &mut (t.b.x);
                *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
              };

              while x
                < cmp::min(
                  cw4,
                  i32::wrapping_shr(i32::wrapping_add(init_x, 16), {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                    numeric_value
                  }),
                )
              {
                let mut cf_guard;
                let cf;
                let eob;
                let mut txtp;

                match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
                  ::core::option::Option::Some(()) => {
                    let ::core::result::Result::Ok(p) =
                      ::console_core_number_conversion::fixed_index(
                        ({
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(t.frame_thread.pass);
                          numeric_value
                        }) & 1,
                      );
                    let len = u32::wrapping_mul(
                      u32::wrapping_mul(
                        u32::wrapping_mul(
                          {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(uvtx.h);
                            numeric_value
                          },
                          4,
                        ),
                        {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped::<_, u32>(uvtx.w);
                          numeric_value
                        },
                      ),
                      4,
                    );
                    let Ok(cf_idx) = (*match <[_]>::get(
                      &ts.frame_thread,
                      match ::console_core_number_conversion::index(p) {
                        ::core::result::Result::Ok(native_position) => native_position,
                      },
                    ) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    })
                    .cf
                    .get_update(|i| u32::wrapping_add(i, len));
                    cf_guard = f.frame_thread.cf.mut_slice_as((
                      ({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(cf_idx);
                        numeric_value
                      })..,
                      ..({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(len);
                        numeric_value
                      }),
                    ));
                    cf = &mut *cf_guard;
                    let Ok(cbi_idx) = (*match <[_]>::get(
                      &ts.frame_thread,
                      match ::console_core_number_conversion::index(p) {
                        ::core::result::Result::Ok(native_position) => native_position,
                      },
                    ) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    })
                    .cbi_idx
                    .get_update(|i| u32::wrapping_add(i, 1));
                    let Ok(cbi) = (*match <[_]>::get(&f.frame_thread.cbi, {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped_index(cbi_idx);
                      numeric_value
                    }) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    })
                    .get();
                    eob = <::core::primitive::i32 as ::core::convert::From<
                      ::core::primitive::i16,
                    >>::from({
                      let Ok(explicit_value) = cbi.eob();
                      explicit_value
                    });
                    txtp = {
                      let Ok(explicit_value) = cbi.txtp();
                      explicit_value
                    };
                  }
                  ::core::option::Option::None => {
                    let mut cf_ctx = 0;
                    txtp = *match <[_]>::get(
                      &*({
                        let explicit_value = ({
                          let explicit_value = t.scratch.inter_intra().map_err(|_invalid_scratch| ())?;
                          explicit_value
                        })
                        .ac_txtp_map
                        .txtp_map().map_err(|_invalid_scratch| ())?;
                        explicit_value
                      }),
                      {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(i32::wrapping_add(
                            i32::wrapping_add(
                              i32::wrapping_mul(
                                i32::wrapping_add(
                                  by4,
                                  i32::wrapping_shl(y, {
                                    let Ok(numeric_value) =
                                      console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                                    numeric_value
                                  }),
                                ),
                                32,
                              ),
                              bx4,
                            ),
                            i32::wrapping_shl(x, {
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                              numeric_value
                            }),
                          ));
                        numeric_value
                      },
                    ) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    };
                    let a_ccoef = &*match <[_]>::get(
                      &(*match f.above_context(t.above_index) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      })
                      .ccoef,
                      pl,
                    ) {
                      ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    };
                    let Ok(a_start) =
                      console_core_number_conversion::wrapped_index(i32::wrapping_add(cbx4, x));
                    let l_ccoef = &mut *match <[_]>::get_mut(&mut t.l.ccoef, pl) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    };
                    let Ok(l_start) =
                      console_core_number_conversion::wrapped_index(i32::wrapping_add(cby4, y));
                    eob = {
                      decode_coefs::<BD>(
                        f,
                        CoefficientDecoder {
                          tile: ts,
                          state: ts_c.as_deref_mut().ok_or(())?,
                          scratch: &mut t.scratch,
                          coefficients: &mut t.cf,
                        },
                        CoefficientBlock {
                          trace: debug_block_info!(f, t.b),
                          above: &a_ccoef.index((
                            a_start..,
                            ..({
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped_index(uvtx.w);
                              numeric_value
                            }),
                          )),
                          left: &*match <[_]>::get(
                            &*match <[_]>::get(&**l_ccoef.get_mut(), l_start..) {
                              ::core::option::Option::Some(indexed_value_5) => indexed_value_5,
                              ::core::option::Option::None => {
                                return ::core::result::Result::Err(());
                              }
                            },
                            ..({
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped_index(uvtx.h);
                              numeric_value
                            }),
                          ) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None => return ::core::result::Result::Err(()),
                          },
                          transform: b.uvtx,
                          size: bs,
                          syntax: b,
                          plane: { let checked_value_23 = u8::try_from(pl).map_err(|_invalid_plane| ())?; checked_value_23 }.wrapping_add(1),
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

                    match debug_block_info!(f, t.b) {
                      BlockTrace::Enabled => {
                        println!(
                          "Post-uv-cf-blk[pl={},tx={:?},txtp={},eob={}]: r={}",
                          pl,
                          b.uvtx,
                          txtp,
                          eob,
                          ts_c.as_deref().ok_or(())?.msac.rng,
                        );
                      }
                      BlockTrace::Quiet => {}
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
                    match (debug_block_info!(f, t.b), PIXEL_TRACE) {
                      (BlockTrace::Enabled, BlockTrace::Enabled) => {
                        coef_dump(
                          cf,
                          Size {
                            width: u64::from(uvtx.h).wrapping_mul(4),
                            height: u64::from(uvtx.w).wrapping_mul(4),
                          },
                          3,
                          "dq",
                        ).map_err(|_dump_error| ())?;
                      }
                      (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
                    }

                    let mut uv_dst_tx = crate::src::owned_recon::ReconDst::Pic(
                      uv_dst
                        + (::core::num::Wrapping(4)
                          * ::core::num::Wrapping({
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_index(x);
                            numeric_value
                          }))
                        .0,
                    );
                    f.dsp
                      .itx
                      .add::<BD>(b.uvtx, txtp, &mut uv_dst_tx, cf, eob, bd)
                      .map_err(|_| ())?;

                    match (debug_block_info!(f, t.b), PIXEL_TRACE) {
                      (BlockTrace::Enabled, BlockTrace::Enabled) => {
                        hex_dump_pic::<BD>(
                          &uv_dst_tx,
                          Size {
                            width: u64::from(uvtx.w).wrapping_mul(4),
                            height: u64::from(uvtx.h).wrapping_mul(4),
                          },
                          "recon",
                        ).map_err(|_dump_error| ())?;
                      }
                      (BlockTrace::Quiet, _) | (_, BlockTrace::Quiet) => {}
                    }
                  }
                  ::core::option::Option::None => {}
                }

                {
                  let numeric_amount = i32::wrapping_shl(
                    {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, i32>(uvtx.w);
                      numeric_value
                    },
                    {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                      numeric_value
                    },
                  );
                  let numeric_target = &mut (t.b.x);
                  *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                };

                {
                  let numeric_amount = {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, i32>(uvtx.w);
                    numeric_value
                  };
                  let numeric_target = &mut (x);
                  *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                };
              }

              uv_dst += ({
                match uv_dst.pixel_stride::<BD>() {
                  ::core::result::Result::Ok(value__) => value__,
                }
              })
              .wrapping_mul(4)
              .wrapping_mul({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(uvtx.h);
                numeric_value
              });

              {
                let numeric_amount = i32::wrapping_shl(x, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                  numeric_value
                });
                let numeric_target = &mut (t.b.x);
                *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
              };

              {
                let numeric_amount = i32::wrapping_shl(
                  {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, i32>(uvtx.h);
                    numeric_value
                  },
                  {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                    numeric_value
                  },
                );
                let numeric_target = &mut (t.b.y);
                *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
              };

              {
                let numeric_amount = {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(uvtx.h);
                  numeric_value
                };
                let numeric_target = &mut (y);
                *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
              };
            }

            {
              let numeric_amount = i32::wrapping_shl(y, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
                numeric_value
              });
              let numeric_target = &mut (t.b.y);
              *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
            };
          }
        }
        ::core::option::Option::None => {}
      }
    }
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
      ::core::primitive::bool::then_some(!c.inloop_filters.contains(Rav1dInloopFilterType::DEBLOCK), ())
  } {
      ::core::option::Option::Some(()) => {
        return Ok(());
      },
      ::core::option::Option::None => {},
  }

  let frame_hdr = f.frame_hdr.as_ref().ok_or(EINVAL)?;

  match {
      ::core::primitive::bool::then_some(frame_hdr.loopfilter.level_y == [0; 2], ())
  } {
      ::core::option::Option::Some(()) => {
        return Ok(());
      },
      ::core::option::Option::None => {},
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
      ::core::primitive::bool::then_some(c.inloop_filters.contains(Rav1dInloopFilterType::DEBLOCK) && frame_hdr.loopfilter.level_y != [0; 2], ())
  } {
      ::core::option::Option::Some(()) => {
        let row = u32::try_from(sby).map_err(|_invalid_row| EINVAL)?;
        rav1d_loopfilter_sbrow_rows::<BD>(f, pictures, row)?;
      },
      ::core::option::Option::None => {},
  }

  match {
      ::core::primitive::bool::then_some(seq_hdr.cdef != 0 || !f.lf.restore_planes.is_empty(), ())
  } {
      ::core::option::Option::Some(()) => {
        rav1d_copy_lpf::<BD>(c, f, pictures, sby)?;
      },
      ::core::option::Option::None => {},
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
    match ::core::primitive::bool::then_some(!c.inloop_filters.contains(Rav1dInloopFilterType::CDEF), ()) {
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
        let p_up = array::from_fn(|i| {
          let ss_ver = f.cur.p.layout == Rav1dPixelLayout::I420 && i != 0;
          p[i]
            - (::core::num::Wrapping(8)

              * ::core::num::Wrapping({
                match p[i].pixel_stride::<BD>() {
                  ::core::result::Result::Ok(value__) => value__,
                }
              }))
            .0
            .wrapping_shr({
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped::<_, u32>(u8::from(ss_ver));
              numeric_value
            })
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
  {
    let bd = BD::from_c(f.bitdepth_max);

    let sbsz = f.sb_step;
    let y = i32::wrapping_mul(i32::wrapping_mul(sby, sbsz), 4);
    let p = f.cur.lf_offsets::<BD>(y);
    let sr_p = f.sr_cur.p.lf_offsets::<BD>(y);
    let ::core::result::Result::Ok(has_chroma) =
      ::console_core_number_conversion::index::<::core::primitive::u8>(
        ::core::primitive::u8::from(f.cur.p.layout != Rav1dPixelLayout::I400),
      );

    for pl in 0..(::core::num::Wrapping(1) + ::core::num::Wrapping(has_chroma.wrapping_mul(2))).0
    {
      let ss_ver = i32::from(pl != 0 && f.cur.p.layout == Rav1dPixelLayout::I420);
      let h_start = i32::wrapping_shr(i32::wrapping_mul(8, i32::from(sby != 0)), {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
        numeric_value
      });
      let dst = sr_p[pl];
      let dst_2 = dst
        - ({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(h_start);
          numeric_value
        })
        .wrapping_mul(match dst.pixel_stride::<BD>() {
          ::core::result::Result::Ok(checked_value_3) => checked_value_3,
        });
      let src = p[pl];
      let src_2 = src
        - ({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(h_start);
          numeric_value
        })
        .wrapping_mul(match src.pixel_stride::<BD>() {
          ::core::result::Result::Ok(checked_value_4) => checked_value_4,
        });
      let h_end = i32::wrapping_shr(
        i32::wrapping_mul(
          4,
          i32::wrapping_sub(
            sbsz,
            i32::wrapping_mul(2, i32::from(i32::wrapping_add(sby, 1) < f.sbh)),
          ),
        ),
        {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
          numeric_value
        },
      );
      let ss_hor = i32::from(pl != 0 && f.cur.p.layout != Rav1dPixelLayout::I444);
      let dst_w = i32::wrapping_shr(i32::wrapping_add(f.sr_cur.p.p.w, ss_hor), {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
        numeric_value
      });
      let src_w = i32::wrapping_shr(i32::wrapping_add(i32::wrapping_mul(4, f.bw), ss_hor), {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
        numeric_value
      });
      let img_h = i32::wrapping_shr(
        i32::wrapping_add(
          i32::wrapping_sub(
            f.cur.p.h,
            i32::wrapping_mul(i32::wrapping_mul(sbsz, 4), sby),
          ),
          ss_ver,
        ),
        {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
          numeric_value
        },
      );

      let Ok(destination) = WithOffset::pic(dst_2);
      let width = u64::try_from(dst_w).map_err(|_| EINVAL)?;
      let source_width = u64::try_from(src_w).map_err(|_| EINVAL)?;
      let output_rows = cmp::min(img_h, h_end).wrapping_add(h_start);
      let height = u64::try_from(output_rows).map_err(|_| EINVAL)?;
      let [luma_step, chroma_step] = f.resize_step;
      let [luma_start, chroma_start] = f.resize_start;
      let phase = match pl {
        0 => ResizePhase { step: luma_step, initial: luma_start },
        _ => ResizePhase { step: chroma_step, initial: chroma_start },
      };
      let sampling = ResizeSampling { source_width, destination: Size { width, height }, phase };
      f.dsp.mc.resize.call::<BD>(destination, src_2, sampling, bd)?;
    }
  };

  Ok(())
}

pub(crate) fn rav1d_filter_sbrow_lr<BD: BitDepth>(
  c: &Rav1dContext,
  f: &Rav1dFrameData,
  _t: &mut Rav1dTaskContext,
  sby: c_int,
) -> Rav1dResult {
  {
    match ::core::primitive::bool::then_some(!c
      .inloop_filters
      .contains(Rav1dInloopFilterType::RESTORATION), ())
    {
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

    match ::core::primitive::bool::then_some((match frame_hdr.size.width {
      [element, ..] => element,
    }) != (match frame_hdr.size.width {
      [_, element, ..] => element,
    }), ()) {
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
  let tile = f.tile(t.tile_index).ok_or(EINVAL)?;
  let superblock_shift = u32::try_from(f.sb_shift).map_err(|_invalid_shift| EINVAL)?;
  let superblock_row = u64::try_from(t.b.y.wrapping_shr(superblock_shift)).map_err(|_invalid_row| EINVAL)?;
  let superblocks = u64::try_from(f.sb128w).map_err(|_invalid_width| EINVAL)?;
  let row = superblock_row.checked_mul(superblocks).ok_or(EINVAL)?;
  let row_origin = row.checked_mul(128).ok_or(EINVAL)?;
  let first_column = tile.tiling.col_start.wrapping_mul(4);
  let luma_rows = t.b.y.wrapping_add(f.sb_step).wrapping_mul(4);
  let tile_width = tile.tiling.col_end.wrapping_sub(tile.tiling.col_start).wrapping_mul(4);
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
    let bank = f.ipred_edge_off.checked_mul(u64::from(plane)).ok_or(EINVAL)?;
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
    let Ok(source) = input.slice::<BD>(samples);
    BD::pixel_copy(&mut f.ipred_edge.mut_slice_as(destination), &source, samples);
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
      frame_palette = f.frame_thread.pal.checked_index::<BD>(record)?;
      &*frame_palette
    }
  };
  let Rectangle { origin, size } = region;
  let x_end = origin.x.checked_add(size.width).ok_or(EINVAL)?;
  let y_end = origin.y.checked_add(size.height).ok_or(EINVAL)?;
  let x = console_core_number_conversion::checked_index(origin.x).map_err(|_outside_palette| EINVAL)?;
  let y = console_core_number_conversion::checked_index(origin.y).map_err(|_outside_palette| EINVAL)?;
  let right = console_core_number_conversion::checked_index(x_end).map_err(|_outside_palette| EINVAL)?;
  let bottom = console_core_number_conversion::checked_index(y_end).map_err(|_outside_palette| EINVAL)?;
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

fn rav1d_read_pal_plane<BD: BitDepth>(
  t: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
  ts_c: &mut Rav1dTileStateContext,
  plane: PalettePlane,
  PaletteRead { size_context: sz_ctx, position }: PaletteRead,
) -> Rav1dResult<u8> {
  Ok({
    let bx4 = console_core_number_conversion::checked_index(position.x).map_err(|_outside_palette| EINVAL)?;
    let by4 = console_core_number_conversion::checked_index(position.y).map_err(|_outside_palette| EINVAL)?;
    let (plane_code, not_pl) = match plane {
      PalettePlane::Luma => (0u8, 1u16),
      PalettePlane::Chroma => (1, 0),
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
        let above_context = f.above_context(t.above_index).ok_or(EINVAL)?;

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
    let neighborhoods = palette_cache::NeighborColors { left, left_count, above, above_count };
    let neighbors = palette_cache::merge_neighbors(neighborhoods, &mut cache)?;
    let [luma_cdf, chroma_cdf] = &mut *ts_c.cdf.m.pal_sz;
    let plane_cdf = match plane {
      PalettePlane::Chroma => chroma_cdf,
      PalettePlane::Luma => luma_cdf,
    };
    let cdf = plane_cdf
      .get_mut(
        match ::console_core_number_conversion::index::<::core::primitive::u8>(sz_ctx) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        },
      )
      .ok_or(EINVAL)?;
    let Ok(symbol) = rav1d_msac_decode_symbol_adapt8(&mut ts_c.msac, cdf, 6);
    let pal_sz_u8 = symbol
      .checked_add(2)
      .filter(|size| *size <= 8)
      .ok_or(EINVAL)?;
    let ::core::result::Result::Ok(pal_sz) =
      ::console_core_number_conversion::index::<::core::primitive::u8>(pal_sz_u8);
    let mut used_cache = [0.as_::<BD::Pixel>(); 8];
    let mut i = 0;

    for neighbor_color in neighbors {
      match ::core::primitive::bool::then_some(!(i < pal_sz), ()) {
        ::core::option::Option::Some(()) => {
          break;
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(matches!(rav1d_msac_decode_bool_equi(&mut ts_c.msac), Ok(EntropyBit::One)), ()) {
        ::core::option::Option::Some(()) => {
          *{
            let checked_value_2 = used_cache.get_mut(i).ok_or(EINVAL)?;
            checked_value_2
          } = *neighbor_color;

          let ::core::result::Result::Ok(()) =
            ::console_core_number_conversion::wrapping_add_index(1, &mut i);
        }
        ::core::option::Option::None => {}
      }
    }

    let selected_colors = used_cache.get(..i).ok_or(EINVAL)?;

    let mut frame_palette;
    let planes = match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
      ::core::option::Option::Some(()) => {
        let stride = i64::from(f.b4_stride);
        let position_2 = palette_cache::PalettePosition { x: t.b.x, y: t.b.y };
        let record = palette_cache::record_index(position_2, stride)?;
        frame_palette = f.frame_thread.pal.checked_index_mut::<BD>(record)?;
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
    let pal = colors.get_mut(..pal_sz).ok_or(EINVAL)?;

    let new_colors = pal.get_mut(selected_colors.len()..).ok_or(EINVAL)?;

    match new_colors.split_first_mut() {
      Some((first_color, remaining)) => {
        let Ok(mut prev) = console_core_number_conversion::wrapped::<_, u16>({
          let Ok(explicit_value) = rav1d_msac_decode_bools(&mut ts_c.msac, f.cur.p.bpc);
          explicit_value
        });
        *first_color = prev.as_::<BD::Pixel>();

        match ::core::primitive::bool::then_some(!remaining.is_empty(), ()) {
          ::core::option::Option::Some(()) => {
            let mut bits = u8::wrapping_sub(
              u8::wrapping_add(f.cur.p.bpc, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>({
                  let Ok(explicit_value) = rav1d_msac_decode_bools(&mut ts_c.msac, 2);
                  explicit_value
                });
                numeric_value
              }),
              3,
            );
            let max = u16::wrapping_sub(
              u16::wrapping_shl(1, {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u32>(f.cur.p.bpc);
                numeric_value
              }),
              1,
            );

            let mut remaining_colors = remaining.iter_mut();

            while let Some(color) = remaining_colors.next() {
              let Ok(delta) = console_core_number_conversion::wrapped::<_, u16>({
                let Ok(explicit_value) = rav1d_msac_decode_bools(&mut ts_c.msac, bits);
                explicit_value
              });
              prev = cmp::min(
                u16::wrapping_add(u16::wrapping_add(prev, delta), not_pl),
                max,
              );
              *color = prev.as_::<BD::Pixel>();

              match ::core::primitive::bool::then_some(u16::wrapping_add(prev, not_pl) >= max, ()) {
                ::core::option::Option::Some(()) => {
                  remaining_colors.into_slice().fill(max.as_::<BD::Pixel>());
                  break;
                }
                ::core::option::Option::None => {
                  bits = cmp::min(
                    bits,
                    u8::wrapping_add(1, {
                      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(
                        ulog2({
                          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                            u16::wrapping_sub(u16::wrapping_sub(max, prev), not_pl),
                          );
                          numeric_value
                        }),
                      );
                      numeric_value
                    }),
                  );
                }
              }
            }
          }
          ::core::option::Option::None => {}
        }
      }
      None => {}
    }

    palette_cache::merge_decoded(pal, selected_colors)?;

    match debug_block_info!(f, t.b) {
      BlockTrace::Enabled => {
        print!(
          "Post-pal[pl={},sz={},cache_size={},used_cache={}]: r={}, cache=",
          plane_code,
          pal_sz,
          neighbors.len(),
          selected_colors.len(),
          ts_c.msac.rng
        );

        for (n, cached_color) in neighbors.iter().enumerate() {
          print!(
            "{}{:02x}",
            match { n != 0 } {
              true => {
                ' '
              }
              false => {
                '['
              }
            },
            (*cached_color).as_::<c_int>()
          );
        }

        print!(
          "{}, pal=",
          match { !neighbors.is_empty() } {
            true => {
              "]"
            }
            false => {
              "[]"
            }
          }
        );

        for (n, pal_2) in pal.iter().enumerate() {
          print!(
            "{}{:02x}",
            match { n != 0 } {
              true => {
                ' '
              }
              false => {
                '['
              }
            },
            (*pal_2).as_::<c_int>()
          );
        }

        println!("]");
      }
      BlockTrace::Quiet => {}
    }

    pal_sz_u8
  })
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
        frame_palette = f.frame_thread.pal.checked_index_mut::<BD>(record)?;
        &mut *frame_palette
      }
      ::core::option::Option::None => {
        let scratch = t.scratch.inter_intra_mut()?;
        let planes = scratch.interintra_edge_pal.pal.buf_mut::<BD>()?;
        planes
      }
    };
    let [_, _, v] = planes;
    let pal_3 = v
      .get_mut(
        ..match ::console_core_number_conversion::index::<::core::primitive::u8>(pal_sz) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        },
      )
      .ok_or(EINVAL)?;

    match ::core::primitive::bool::then_some(matches!(rav1d_msac_decode_bool_equi(&mut ts_c.msac), Ok(EntropyBit::One)), ()) {
      ::core::option::Option::Some(()) => {
        let bits = u8::wrapping_sub(
          u8::wrapping_add(f.cur.p.bpc, {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>({
              let Ok(explicit_value) = rav1d_msac_decode_bools(&mut ts_c.msac, 2);
              explicit_value
            });
            numeric_value
          }),
          4,
        );
        let Ok(mut prev) = console_core_number_conversion::wrapped::<_, u16>({
          let Ok(explicit_value) = rav1d_msac_decode_bools(&mut ts_c.msac, f.cur.p.bpc);
          explicit_value
        });
        let (first, rest) = pal_3.split_first_mut().ok_or(EINVAL)?;
        *first = prev.as_::<BD::Pixel>();
        let max = u16::wrapping_sub(
          u16::wrapping_shl(1, {
            let Ok(numeric_value) =
              console_core_number_conversion::wrapped::<_, u32>(f.cur.p.bpc);
            numeric_value
          }),
          1,
        );

        for pal_2 in rest {
          let Ok(mut delta) = console_core_number_conversion::wrapped::<_, i16>({
            let Ok(explicit_value) = rav1d_msac_decode_bools(&mut ts_c.msac, bits);
            explicit_value
          });

          match ::core::primitive::bool::then_some(delta != 0 && matches!(rav1d_msac_decode_bool_equi(&mut ts_c.msac), Ok(EntropyBit::One)), ()) {
            ::core::option::Option::Some(()) => {
              delta = (delta).wrapping_neg();
            }
            ::core::option::Option::None => {}
          }

          prev = ({
            let Ok(numeric_value) =
              console_core_number_conversion::wrapped::<_, u16>(i16::wrapping_add(
                {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(prev);
                  numeric_value
                },
                delta,
              ));
            numeric_value
          }) & max;
          *pal_2 = prev.as_::<BD::Pixel>();
        }
      }
      ::core::option::Option::None => {
        pal_3.fill_with(|| {
          ({
            let Ok(explicit_value) = rav1d_msac_decode_bools(&mut ts_c.msac, f.cur.p.bpc);
            explicit_value
          })
          .as_::<BD::Pixel>()
        });
      }
    }

    match debug_block_info!(f, t.b) {
      BlockTrace::Enabled => {
        print!("Post-pal[pl=2]: r={} ", ts_c.msac.rng);

        for (n, pal_4) in pal_3.iter().enumerate() {
          print!(
            "{}{:02x}",
            match { n != 0 } {
              true => {
                ' '
              }
              false => {
                '['
              }
            },
            (*pal_4).as_::<c_int>()
          );
        }

        println!("]");
      }
      BlockTrace::Quiet => {}
    }

    pal_sz
  })
}
