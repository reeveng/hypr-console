#![forbid(unsafe_code)]
use crate::include::common::bitdepth::BPC;
use crate::include::common::intops::apply_sign64;
use crate::include::common::intops::clip;
use crate::include::common::intops::clip_u8;
use crate::include::common::intops::iclip;
use crate::include::dav1d::common::Rav1dDataProps;
use crate::include::dav1d::headers::RAV1D_PRIMARY_REF_NONE;
use crate::include::dav1d::headers::Rav1dFilterMode;
use crate::include::dav1d::headers::Rav1dFrameHeader;
use crate::include::dav1d::headers::Rav1dFrameHeaderQuant;
use crate::include::dav1d::headers::Rav1dFrameHeaderTiling;
use crate::include::dav1d::headers::Rav1dFrameType;
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::headers::Rav1dRestorationType;
use crate::include::dav1d::headers::Rav1dSegmentationData;
use crate::include::dav1d::headers::Rav1dSequenceHeader;
use crate::include::dav1d::headers::Rav1dTxfmMode;
use crate::include::dav1d::headers::Rav1dWarpedMotionParams;
use crate::include::dav1d::headers::Rav1dWarpedMotionType;
use crate::include::dav1d::headers::SgrIdx;
use crate::include::dav1d::picture::Rav1dPicture;
use crate::src::align::AlignedVec64;
use crate::src::c_arc::CArc;
use crate::src::cdf::CdfMvContext;
use crate::src::cdf::CdfThreadContext;
use crate::src::cdf::CdfThreading;
use crate::src::cdf::rav1d_cdf_thread_alloc;
use crate::src::cdf::rav1d_cdf_thread_copy;
use crate::src::cdf::rav1d_cdf_thread_init_static;
use crate::src::cdf::rav1d_cdf_thread_update;
use crate::src::cdf::{CdfModeInterContext, CdfMvComponent};
use crate::src::ctx::case_set_al;
use crate::src::ctx::{CaseSet, ContextRun};
use crate::src::dequant_tables::dav1d_dq_tbl;
use crate::src::disjoint_mut::DisjointMut;
use crate::src::disjoint_mut::DisjointMutSlice;
use crate::src::disjoint_mut::TryResizable;
use crate::src::disjoint_mut::TryResizableWith;
use crate::src::enum_map::enum_map_ty;
use crate::src::enum_map::{DefaultValue, EnumMap};
use crate::src::env::fix_mv_precision;
use crate::src::env::get_comp_ctx;
use crate::src::env::get_comp_dir_ctx;
use crate::src::env::get_cur_frame_segid;
use crate::src::env::get_drl_context;
use crate::src::env::get_filter_ctx;
use crate::src::env::get_gmv_2d;
use crate::src::env::get_intra_ctx;
use crate::src::env::get_mask_comp_ctx;
use crate::src::env::get_partition_ctx;
use crate::src::env::{BlockContext, FilterAxis, NeighborAvailability, ReferenceArity};
use crate::src::env::{BlockNeighbors, ReferenceContext, ReferenceCounts};
use crate::src::env::{CompoundOrderHints, get_jnt_comp_ctx};
use crate::src::env::{OrderHintComparison, get_poc_diff};
use crate::src::env::{PartitionEdge, gather_partition_probability};
use crate::src::env::{get_tx_ctx, read_context};
use crate::src::error::Rav1dError::ECANCELED;
use crate::src::error::Rav1dError::EGeneric;
use crate::src::error::Rav1dError::EINVAL;
use crate::src::error::Rav1dError::ENOMEM;
use crate::src::error::Rav1dError::ENOPROTOOPT;
use crate::src::error::Rav1dResult;
use crate::src::internal::ChromaNeighborPrediction;
use crate::src::internal::CodedBlockInfo;
use crate::src::internal::Rav1dBitDepthDSPContext;
use crate::src::internal::Rav1dContext;
use crate::src::internal::Rav1dContextTaskType;
use crate::src::internal::Rav1dFrameContext;
use crate::src::internal::Rav1dFrameContextFrameThread;
use crate::src::internal::Rav1dFrameContextLf;
use crate::src::internal::Rav1dFrameData;
use crate::src::internal::Rav1dState;
use crate::src::internal::Rav1dTaskContext;
use crate::src::internal::Rav1dTileState;
use crate::src::internal::ScalableMotionParams;
use crate::src::internal::TileStateRef;
use crate::src::internal::{Bxy, FlushState};
use crate::src::internal::{Rav1dTileStateContext, Rav1dTileStateTiling};
use crate::src::internal::{RightEdgePixels, RightEdgeRows};
use crate::src::intra_edge::EdgeIndex;
use crate::src::intra_edge::IntraEdges;
use crate::src::intra_edge::SuperblockSize;
use crate::src::intra_edge::{EdgeAvailability, EdgeFlags};
use crate::src::levels::Av1Block;
use crate::src::levels::Av1BlockInter;
use crate::src::levels::Av1BlockInter1d;
use crate::src::levels::Av1BlockInter2d;
use crate::src::levels::Av1BlockInterNd;
use crate::src::levels::Av1BlockIntra;
use crate::src::levels::Av1BlockIntraInter;
use crate::src::levels::BlockLevel;
use crate::src::levels::BlockPartition;
use crate::src::levels::BlockSize;
use crate::src::levels::BlockWarp;
use crate::src::levels::CFL_PRED;
use crate::src::levels::CompInterType;
use crate::src::levels::DC_PRED;
use crate::src::levels::DrlProximity;
use crate::src::levels::FILTER_PRED;
use crate::src::levels::Filter2d;
use crate::src::levels::GLOBALMV;
use crate::src::levels::GLOBALMV_GLOBALMV;
use crate::src::levels::InterIntraPredMode;
use crate::src::levels::InterIntraType;
use crate::src::levels::MotionMode;
use crate::src::levels::Mv;
use crate::src::levels::N_COMP_INTER_PRED_MODES;
use crate::src::levels::N_INTRA_PRED_MODES;
use crate::src::levels::N_UV_INTRA_PRED_MODES;
use crate::src::levels::NEARESTMV;
use crate::src::levels::NEARESTMV_NEARESTMV;
use crate::src::levels::NEARMV;
use crate::src::levels::NEWMV;
use crate::src::levels::NEWMV_NEWMV;
use crate::src::levels::SegmentId;
use crate::src::levels::TxfmSize;
use crate::src::levels::VERT_LEFT_PRED;
use crate::src::levels::VERT_PRED;
use crate::src::lf_mask::Av1RestorationUnit;
use crate::src::lf_mask::rav1d_calc_eih;
use crate::src::lf_mask::rav1d_calc_lf_values;
use crate::src::lf_mask::{
  EdgeNeighbors, EdgeScan, FilterMaskBlock, FilterMaskMotion, FilterMaskPrediction,
  create_lf_mask,
};
use crate::src::log::Rav1dLog as _;
use crate::src::lr_apply::LrRestorePlanes;
use crate::src::managed::Strictness;
use crate::src::msac::EntropyBit;
use crate::src::msac::rav1d_msac_decode_bool;
use crate::src::msac::rav1d_msac_decode_bool_adapt;
use crate::src::msac::rav1d_msac_decode_bool_equi;
use crate::src::msac::rav1d_msac_decode_bools;
use crate::src::msac::rav1d_msac_decode_subexp;
use crate::src::msac::rav1d_msac_decode_symbol_adapt4;
use crate::src::msac::rav1d_msac_decode_symbol_adapt8;
use crate::src::msac::rav1d_msac_decode_symbol_adapt16;
use crate::src::msac::rav1d_msac_decode_uniform;
use crate::src::msac::{MsacContext, SubexpParameters};
use crate::src::pal::{
  PaletteBlock, PaletteCellBlock, PaletteOutput, PalettePlane, Rav1dPalDSPContext,
};
use crate::src::picture::rav1d_picture_alloc_copy;
use crate::src::picture::rav1d_thread_picture_alloc;
use crate::src::picture::{Rav1dThreadPicture, ThreadPictureRequest};
use crate::src::qm::dav1d_qm_tbl;
use crate::src::recon::{BlockTrace, PaletteCopy, PaletteRead, debug_block_info};
use crate::src::refmvs::MotionRowPass;
use crate::src::refmvs::MotionTileRow;
use crate::src::refmvs::RefMvsBlock;
use crate::src::refmvs::RefMvsFrame;
use crate::src::refmvs::RefMvsMvPair;
use crate::src::refmvs::RefMvsRefPair;
use crate::src::refmvs::SpatialMvRegion;
use crate::src::refmvs::TemporalMvRegion;
use crate::src::refmvs::rav1d_refmvs_find;
use crate::src::refmvs::rav1d_refmvs_init_frame;
use crate::src::refmvs::rav1d_refmvs_tile_sbrow_init;
use crate::src::refmvs::read_spatial_block;
use crate::src::refmvs::{RefmvsTile, RefmvsTileRange};
use crate::src::relaxed_atomic::RelaxedAtomic;
use crate::src::tables::TxfmInfo;
use crate::src::tables::cfl_allowed_mask;
use crate::src::tables::dav1d_al_part_ctx;
use crate::src::tables::dav1d_block_sizes;
use crate::src::tables::dav1d_comp_inter_pred_modes;
use crate::src::tables::dav1d_filter_2d;
use crate::src::tables::dav1d_filter_dir;
use crate::src::tables::dav1d_intra_mode_context;
use crate::src::tables::dav1d_max_txfm_size_for_bs;
use crate::src::tables::dav1d_partition_type_count;
use crate::src::tables::dav1d_sgr_params;
use crate::src::tables::dav1d_txfm_dimensions;
use crate::src::tables::dav1d_wedge_ctx_lut;
use crate::src::tables::dav1d_ymode_size_context;
use crate::src::tables::interintra_allowed_mask;
use crate::src::tables::wedge_allowed_mask;
use crate::src::thread_task::TILE_ERROR;
use crate::src::thread_task::TileQueueStage;
use crate::src::thread_task::rav1d_task_create_tile_sbrow;
use crate::src::thread_task::rav1d_task_frame_init;
use crate::src::thread_task::{FRAME_ERROR, FrameReservation, reserve_delayed_frame};
use crate::src::warpmv::rav1d_get_shear_params;
use crate::src::warpmv::rav1d_set_affine_mv2d;
use crate::src::warpmv::{WarpValidity, rav1d_find_affine_int};
use aligned::Aligned;
use console_core_geometry::{Point, Rectangle, Size};
use console_core_number_conversion::checked_index;
use console_core_pixel_buffers::RowLayout;
use console_core_synchronization::nonpoison::{MutexGuard, wait_while};
use std::cmp;
use std::ffi::c_int;
use std::iter;
use std::mem;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;
use strum::EnumCount;

fn init_quant_tables(
  seq_hdr: &Rav1dSequenceHeader,
  frame_hdr: &Rav1dFrameHeader,
  qidx: u8,

  dq: &[[[RelaxedAtomic<u16>; 2]; 3]; {
     match console_core_number_conversion::const_u8_index(SegmentId::COUNT) {
       Ok(extent) => extent,
     }
   }],
) -> Rav1dResult {
  let [depth8, depth10, depth12] = &dav1d_dq_tbl;
  let table = match seq_hdr.hbd {
    0 => depth8,
    1 => depth10,
    2 => depth12,
    _ => return Err(EINVAL),
  };
  let segmentation = &frame_hdr.segmentation;
  let segments: &[_] = match segmentation.enabled {
    0 => {
      let [first, ..] = dq;
      core::slice::from_ref(first)
    }
    1.. => dq,
  };
  let quant = &frame_hdr.quant;
  let plane_deltas = [
    [quant.ydc_delta, 0],
    [quant.udc_delta, quant.uac_delta],
    [quant.vdc_delta, quant.vac_delta],
  ];

  for (destination, segment) in segments.iter().zip(&segmentation.seg_data.d) {
    let base = match segmentation.enabled {
      0 => qidx,
      1.. => clip_u8(i32::from(qidx).wrapping_add(i32::from(segment.delta_q))),
    };

    for ([dc, ac], [dc_delta, ac_delta]) in destination.iter().zip(plane_deltas) {
      let dc_quantizer = clip_u8(i16::from(base).wrapping_add(i16::from(dc_delta)));
      let ac_quantizer = clip_u8(i16::from(base).wrapping_add(i16::from(ac_delta)));
      let Ok(dc_index) = console_core_number_conversion::index(dc_quantizer);
      let Ok(ac_index) = console_core_number_conversion::index(ac_quantizer);
      let &[dc_coefficient, _] = table.get(dc_index).ok_or(EINVAL)?;
      let &[_, ac_coefficient] = table.get(ac_index).ok_or(EINVAL)?;
      let Ok(()) = dc.set(dc_coefficient);
      let Ok(()) = ac.set(ac_coefficient);
    }
  }

  Ok(())
}

fn init_quant_matrices(
  quant: &Rav1dFrameHeaderQuant,
  matrices: &mut [[Option<&'static [u8]>; 3]; TxfmSize::COUNT],
) -> Rav1dResult {
  match quant.qm {
    0 => {
      matrices.fill([None; 3]);
      return Ok(());
    }
    1.. => {}
  }

  let Ok(y_level) = console_core_number_conversion::index(quant.qm_y);
  let Ok(u_level) = console_core_number_conversion::index(quant.qm_u);
  let Ok(v_level) = console_core_number_conversion::index(quant.qm_v);
  let [luma, _] = dav1d_qm_tbl.get(y_level).ok_or(EINVAL)?;
  let [_, cb] = dav1d_qm_tbl.get(u_level).ok_or(EINVAL)?;
  let [_, cr] = dav1d_qm_tbl.get(v_level).ok_or(EINVAL)?;

  for (((matrix, &y), &u), &v) in matrices.iter_mut().zip(luma).zip(cb).zip(cr) {
    *matrix = [y, u, v];
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum ResidualPrecision {
  Integer,
  QuarterPel,
  EighthPel,
}

fn read_mv_component_diff(
  msac: &mut MsacContext,
  probabilities: &mut CdfMvComponent,
  precision: ResidualPrecision,
) -> Rav1dResult<i16> {
  let sign = matches!(
    rav1d_msac_decode_bool_adapt(msac, &mut probabilities.sign),
    Ok(EntropyBit::One)
  );
  let Ok(class) = rav1d_msac_decode_symbol_adapt16(msac, &mut *probabilities.classes, 10);
  let (whole_value, fractions, high_bits) = match class {
    0 => {
      let whole = matches!(
        rav1d_msac_decode_bool_adapt(msac, &mut probabilities.class_0),
        Ok(EntropyBit::One)
      );
      let [zero, one] = &mut *probabilities.class_0_fp;
      let fractional = match ::core::primitive::bool::then_some(whole, ()) {
        ::core::option::Option::Some(()) => one,
        ::core::option::Option::None => zero,
      };
      (u16::from(whole), fractional, &mut *probabilities.class_0_hp)
    }
    1..=10 => {
      let Ok(class_size) = console_core_number_conversion::index(class);
      let mut whole = 1u16.wrapping_shl(u32::from(class));

      for (distribution, shift) in probabilities
        .class_n
        .iter_mut()
        .take(class_size)
        .zip(0u32..)
      {
        let bit = matches!(
          rav1d_msac_decode_bool_adapt(msac, distribution),
          Ok(EntropyBit::One)
        );
        whole |= u16::from(bit).wrapping_shl(shift);
      }

      (
        whole,
        &mut *probabilities.class_n_fp,
        &mut *probabilities.class_n_hp,
      )
    }
    _ => return Err(EINVAL),
  };
  let fraction = match precision {
    ResidualPrecision::Integer => 3,
    ResidualPrecision::QuarterPel | ResidualPrecision::EighthPel => {
      let Ok(value) = rav1d_msac_decode_symbol_adapt4(msac, fractions, 3);
      value
    }
  };
  let high = match precision {
    ResidualPrecision::Integer | ResidualPrecision::QuarterPel => true,
    ResidualPrecision::EighthPel => matches!(
      rav1d_msac_decode_bool_adapt(msac, high_bits),
      Ok(EntropyBit::One)
    ),
  };
  let unsigned =
    (whole_value.wrapping_shl(3) | u16::from(fraction).wrapping_shl(1) | u16::from(high))
      .wrapping_add(1);
  let Ok(distance) = console_core_number_conversion::fitted::<_, i16>(unsigned);
  Ok(match ::core::primitive::bool::then_some(sign, ()) {
    ::core::option::Option::Some(()) => distance.wrapping_neg(),
    ::core::option::Option::None => distance,
  })
}

fn read_mv_residual(
  tile: &mut Rav1dTileStateContext,
  reference: &mut Mv,
  precision: ResidualPrecision,
) -> Rav1dResult {
  let Ok(joint) = rav1d_msac_decode_symbol_adapt4(
    &mut tile.msac,
    &mut *tile.cdf.mv.joint,
    CdfMvContext::JOINT_THRESHOLDS,
  );
  let [vertical, horizontal] = &mut tile.cdf.mv.comp;

  match joint & 0b10 {
    0 => {}
    _ => {
      let delta = read_mv_component_diff(&mut tile.msac, vertical, precision)?;
      reference.y = reference.y.wrapping_add(delta);
    }
  }

  match joint & 0b01 {
    0 => {}
    _ => {
      let delta = read_mv_component_diff(&mut tile.msac, horizontal, precision)?;
      reference.x = reference.x.wrapping_add(delta);
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum TransformDepth {
  Root,
  Child,
  Leaf,
}

impl TransformDepth {
  fn split_level(self) -> Result<Option<u8>, console_core_never::Never> {
    Ok(match self {
      Self::Root => Some(0),
      Self::Child => Some(1),
      Self::Leaf => None,
    })
  }
}

#[derive(Clone, Copy)]
struct TransformNode {
  size: TxfmSize,
  origin: Bxy,
  mask_position: Point<u8>,
  depth: TransformDepth,
}

struct TransformNeighborhood<'a> {
  above: &'a BlockContext,
  left: &'a mut BlockContext,
}

fn transform_info(size: TxfmSize) -> Rav1dResult<&'static TxfmInfo> {
  let Ok(ordinal) = size.ordinal();
  let Ok(position) = console_core_number_conversion::index(ordinal);
  dav1d_txfm_dimensions.get(position).ok_or(EINVAL)
}

fn square_transform(log_extent: u8) -> Rav1dResult<TxfmSize> {
  let Ok(position) = console_core_number_conversion::index(log_extent);
  TxfmSize::from_repr(position).ok_or(EINVAL)
}

fn update_transform_context<const LIMIT: usize>(
  neighbors: &mut TransformNeighborhood<'_>,
  local: Bxy,
  extent: Size<u8>,
  transforms: [TxfmSize; 2],
) -> Rav1dResult {
  let row = u64::try_from(local.y).map_err(|_outside_context| EINVAL)?;
  let column = u64::try_from(local.x).map_err(|_outside_context| EINVAL)?;
  let height = u64::from(extent.height);
  let width = u64::from(extent.width);

  match (row.saturating_add(height) <= 32 && column.saturating_add(width) <= 32).then_some(()) {
    Some(()) => {}
    None => return Err(EINVAL),
  }

  let [left_value, above_value] = transforms;
  let left_run = ContextRun {
    offset: row,
    length: height,
  };
  let above_run = ContextRun {
    offset: column,
    length: width,
  };
  let Ok(()) = CaseSet::<LIMIT, false>::one(
    (&mut neighbors.left.tx, left_value),
    left_run,
    |writer, (field, value)| {
      let Ok(()) = writer.set_exclusive(field, value);
    },
  );
  let Ok(()) = CaseSet::<LIMIT, false>::one(
    (&neighbors.above.tx, above_value),
    above_run,
    |writer, (field, value)| {
      let Ok(()) = writer.set_disjoint(field, value);
    },
  );
  Ok(())
}

enum TransformPartition {
  Whole,
  Split,
}

fn transform_partition(
  tile: &mut Rav1dTileStateContext,
  neighbors: &mut TransformNeighborhood<'_>,
  local: Bxy,
  dimensions: &TxfmInfo,
  depth: u8,
) -> Rav1dResult<TransformPartition> {
  let from_largest = 4u8.checked_sub(dimensions.max).ok_or(EINVAL)?;
  let category = from_largest
    .wrapping_mul(2)
    .checked_sub(depth)
    .ok_or(EINVAL)?;
  let Ok(left_position) = console_core_number_conversion::index(local.y);
  let left = ::core::result::Result::<&_, _>::copied(
    neighbors.left.tx.get_mut().get(left_position).ok_or(EINVAL),
  )?;
  let above = read_context(&neighbors.above.tx, local.x)?;
  let Ok(left_value) = left.ordinal();
  let Ok(above_value) = above.ordinal();
  let score =
    u8::from(above_value < dimensions.lw).wrapping_add(u8::from(left_value < dimensions.lh));
  let Ok(category_position) = console_core_number_conversion::index(category);
  let Ok(context_position) = console_core_number_conversion::index(score);
  let context = tile
    .cdf
    .m
    .txpart
    .get_mut(category_position)
    .and_then(|contexts| contexts.get_mut(context_position))
    .ok_or(EINVAL)?;
  Ok(
    match matches!(
      rav1d_msac_decode_bool_adapt(&mut tile.msac, context),
      Ok(EntropyBit::One)
    )
    .then_some(())
    {
      Some(()) => TransformPartition::Split,
      None => TransformPartition::Whole,
    },
  )
}

fn subdivide_transform(
  parent: TransformNode,
  dimensions: &TxfmInfo,
  frame: Size<i32>,
) -> Rav1dResult<[Option<TransformNode>; 4]> {
  let sub = transform_info(dimensions.sub)?;
  let depth = match parent.depth {
    TransformDepth::Root => TransformDepth::Child,
    TransformDepth::Child => TransformDepth::Leaf,
    TransformDepth::Leaf => return Err(EINVAL),
  };
  let left = parent.mask_position.x.wrapping_mul(2);
  let top = parent.mask_position.y.wrapping_mul(2);
  let right = left.wrapping_add(1);
  let bottom = top.wrapping_add(1);
  let x = parent.origin.x.wrapping_add(i32::from(sub.w));
  let y = parent.origin.y.wrapping_add(i32::from(sub.h));
  let top_left = TransformNode {
    size: dimensions.sub,
    origin: parent.origin,
    mask_position: Point { x: left, y: top },
    depth,
  };
  let east = dimensions.lw >= dimensions.lh && x < frame.width;
  let south = dimensions.lh >= dimensions.lw && y < frame.height;
  Ok([
    Some(top_left),
    east.then_some(TransformNode {
      origin: Bxy {
        x,
        y: parent.origin.y,
      },
      mask_position: Point { x: right, y: top },
      ..top_left
    }),
    south.then_some(TransformNode {
      origin: Bxy {
        x: parent.origin.x,
        y,
      },
      mask_position: Point { x: left, y: bottom },
      ..top_left
    }),
    (east && south).then_some(TransformNode {
      origin: Bxy { x, y },
      mask_position: Point {
        x: right,
        y: bottom,
      },
      ..top_left
    }),
  ])
}

fn visit_transform_node(
  tile: &mut Rav1dTileStateContext,
  neighbors: &mut TransformNeighborhood<'_>,
  node: TransformNode,
  frame: Size<i32>,
  masks: &mut [u16; 2],
) -> Rav1dResult<[Option<TransformNode>; 4]> {
  let dimensions = transform_info(node.size)?;
  let local = Bxy {
    x: node.origin.x & 31,
    y: node.origin.y & 31,
  };
  let Ok(level) = node.depth.split_level();
  let partition = match (level, node.size) {
    (None, _) | (_, TxfmSize::S4x4) => TransformPartition::Whole,
    (Some(depth), _) => transform_partition(tile, neighbors, local, dimensions, depth)?,
  };
  let extent = Size {
    width: dimensions.w,
    height: dimensions.h,
  };
  let values = match partition {
    TransformPartition::Whole => [
      {
        let checked_value_3 = square_transform(dimensions.lh)?;
        checked_value_3
      },
      {
        let checked_value_7 = square_transform(dimensions.lw)?;
        checked_value_7
      },
    ],
    TransformPartition::Split => {
      let [root_mask, child_mask] = masks;
      let mask = match node.depth {
        TransformDepth::Root => root_mask,
        TransformDepth::Child => child_mask,
        TransformDepth::Leaf => return Err(EINVAL),
      };
      let shift = u32::from(node.mask_position.y)
        .wrapping_mul(4)
        .wrapping_add(u32::from(node.mask_position.x));
      *mask |= 1u16.wrapping_shl(shift);
      let Ok(small) = TxfmSize::S8x8.ordinal();

      match (dimensions.max > small).then_some(()) {
        Some(()) => return subdivide_transform(node, dimensions, frame),
        None => [TxfmSize::S4x4; 2],
      }
    }
  };
  update_transform_context::<16>(neighbors, local, extent, values)?;
  Ok([None; 4])
}

fn read_tx_tree(
  tile: &mut Rav1dTileStateContext,
  neighbors: &mut TransformNeighborhood<'_>,
  root: TransformNode,
  frame: Size<i32>,
  masks: &mut [u16; 2],
) -> Rav1dResult {
  let children = visit_transform_node(tile, neighbors, root, frame, masks)?;

  for child in children.into_iter().flatten() {
    let leaves = visit_transform_node(tile, neighbors, child, frame, masks)?;

    for leaf in leaves.into_iter().flatten() {
      let _terminal = visit_transform_node(tile, neighbors, leaf, frame, masks)?;
    }
  }

  Ok(())
}

struct SegmentRemap {
  difference: u8,
  limit: u8,
}

fn neg_deinterleave(
  SegmentRemap { difference, limit }: SegmentRemap,
  reference: SegmentId,
) -> Result<u8, console_core_never::Never> {
  let Ok(segment) = reference.get();
  let Ok(center) = console_core_number_conversion::wrapped::<_, u8>(segment);

  match bool::then_some(center == 0, ()) {
    Some(()) => return Ok(difference),
    None => {}
  }

  let mirrored = limit.wrapping_sub(difference.wrapping_add(1));

  match bool::then_some(center.wrapping_add(1) >= limit, ()) {
    Some(()) => return Ok(mirrored),
    None => {}
  }

  let twice_center = center.wrapping_mul(2);
  let (paired, unpaired) = match bool::then_some(twice_center < limit, ()) {
    Some(()) => (twice_center, difference),
    None => (
      limit.wrapping_sub(center).wrapping_sub(1).wrapping_mul(2),
      mirrored,
    ),
  };

  match bool::then_some(difference > paired, ()) {
    Some(()) => return Ok(unpaired),
    None => {}
  }

  Ok(match bool::then_some(difference & 1 != 0, ()) {
    Some(()) => center.wrapping_add(difference.wrapping_add(1).wrapping_shr(1)),
    None => center.wrapping_sub(difference.wrapping_shr(1)),
  })
}

struct WarpNeighborGrid<'a> {
  blocks: &'a DisjointMut<AlignedVec64<RefMvsBlock>>,
  tile: &'a RefmvsTile,
  origin: Point<i32>,
}

impl WarpNeighborGrid<'_> {
  fn sample(&self, offset: Point<i32>) -> Rav1dResult<RefMvsBlock> {
    let ring_row = (self.origin.y & 31).wrapping_add(5);
    read_spatial_block(
      self.blocks,
      &self.tile.r,
      Point {
        x: self.origin.x.wrapping_add(offset.x),
        y: ring_row.wrapping_add(offset.y),
      },
    )
  }
}

struct WarpNeighborQuery {
  block: Size<i32>,
  visible: Size<i32>,
  availability: NeighborAvailability,
  edge_flags: EdgeFlags,
  reference: i8,
}

#[derive(Clone, Copy)]
enum WarpEdge {
  Above,
  Left,
}

impl WarpEdge {
  fn neighbor(self, distance: i32) -> Result<Point<i32>, console_core_never::Never> {
    Ok(match self {
      Self::Above => Point { x: distance, y: -1 },
      Self::Left => Point { x: -1, y: distance },
    })
  }

  fn extent(self, size: Size<i32>) -> Result<i32, console_core_never::Never> {
    Ok(match self {
      Self::Above => size.width,
      Self::Left => size.height,
    })
  }

  fn neighbor_span(self, neighbor: RefMvsBlock) -> Result<i32, console_core_never::Never> {
    let Ok(&[width, height, ..]) = neighbor.bs.dimensions();
    Ok(i32::from(match self {
      Self::Above => width,
      Self::Left => height,
    }))
  }
}

#[derive(Default)]
struct WarpEdgeMatches {
  mask: u64,
  count: u32,
  first_span: i32,
}

fn scan_warp_edge(
  grid: &WarpNeighborGrid<'_>,
  query: &WarpNeighborQuery,
  edge: WarpEdge,
  limit: u32,
) -> Rav1dResult<WarpEdgeMatches> {
  let Ok(first_cell) = edge.neighbor(0);
  let first = grid.sample(first_cell)?;
  let Ok(first_span) = edge.neighbor_span(first);
  let references = [query.reference.wrapping_add(1), -1];
  let first_match = first.r#ref.r#ref == references;
  let mut found = WarpEdgeMatches {
    first_span,
    mask: u64::from(first_match),
    count: u32::from(first_match),
  };

  let Ok(block_span) = edge.extent(query.block);

  match (first_span < block_span).then_some(()) {
    Some(()) => {}
    None => return Ok(found),
  }

  let Ok(visible_span) = edge.extent(query.visible);
  let mut distance = first_span;

  for _cell in 1..32 {
    match (distance < visible_span && found.count < limit).then_some(()) {
      Some(()) => {}
      None => break,
    }

    let Ok(cell) = edge.neighbor(distance);
    let neighbor = grid.sample(cell)?;

    match (neighbor.r#ref.r#ref == references).then_some(()) {
      Some(()) => {
        let bit = u32::try_from(distance).map_err(|_| EINVAL)?;
        found.mask |= 1_u64.wrapping_shl(bit);
        found.count = found.count.wrapping_add(1);
      }
      None => {}
    }

    let Ok(span) = edge.neighbor_span(neighbor);
    distance = distance.wrapping_add(span);
  }

  Ok(found)
}

fn find_matching_ref(
  f: &Rav1dFrameData,
  t: &Rav1dTaskContext,
  query: WarpNeighborQuery,
) -> Rav1dResult<[u64; 2]> {
  const SAMPLE_LIMIT: u32 = 8;
  const CORNER: u64 = 1 << 32;
  let origin = Point { x: t.b.x, y: t.b.y };
  let grid = WarpNeighborGrid {
    blocks: &f.rf.r,
    tile: &t.rt,
    origin,
  };
  let top_available = matches!(
    query.availability,
    NeighborAvailability::Both | NeighborAvailability::Top
  );
  let left_available = matches!(
    query.availability,
    NeighborAvailability::Both | NeighborAvailability::Left
  );
  let mut above = match top_available.then_some(()) {
    Some(()) => scan_warp_edge(&grid, &query, WarpEdge::Above, SAMPLE_LIMIT)?,
    None => WarpEdgeMatches::default(),
  };
  let mut left = match (left_available && above.count < SAMPLE_LIMIT).then_some(()) {
    Some(()) => scan_warp_edge(
      &grid,
      &query,
      WarpEdge::Left,
      SAMPLE_LIMIT.wrapping_sub(above.count),
    )?,
    None => WarpEdgeMatches::default(),
  };
  let mut samples = above.count.wrapping_add(left.count);
  let references = [query.reference.wrapping_add(1), -1];
  let above_offset = origin.x & above.first_span.wrapping_sub(1);
  let left_offset = origin.y & left.first_span.wrapping_sub(1);
  let blocks_above_corner = above.first_span >= query.block.width && above_offset != 0;
  let blocks_left_corner = left.first_span >= query.block.height && left_offset != 0;

  let top_left_available = top_available
    && left_available
    && samples < SAMPLE_LIMIT
    && !blocks_above_corner
    && !blocks_left_corner;

  match top_left_available.then_some(()) {
    Some(()) => {
      let corner = grid.sample(Point { x: -1, y: -1 })?;

      match (corner.r#ref.r#ref == references).then_some(()) {
        Some(()) => {
          left.mask |= CORNER;
          samples = samples.wrapping_add(1);
        }
        None => {}
      }
    }
    None => {}
  }

  let overlaps_above = above.first_span >= query.block.width
    && above.first_span.wrapping_sub(above_offset) > query.block.width;
  let top_right_available = top_available
    && samples < SAMPLE_LIMIT
    && !overlaps_above
    && query.block.width.max(query.block.height) < 32
    && matches!(
      query.edge_flags.availability(EdgeFlags::I444_TOP_HAS_RIGHT),
      Ok(EdgeAvailability::Present)
    );

  match top_right_available.then_some(()) {
    Some(()) => {
      let tile = f.tile(t.tile_index)?;

      match (origin.x.wrapping_add(query.block.width) < tile.tiling.col_end).then_some(()) {
        Some(()) => {
          let corner = grid.sample(Point {
            x: query.block.width,
            y: -1,
          })?;
          above.mask |= CORNER.wrapping_mul(u64::from(corner.r#ref.r#ref == references));
        }
        None => {}
      }
    }
    None => {}
  }

  Ok([above.mask, left.mask])
}

type WarpFeaturePair = [[i32; 2]; 2];

struct WarpSamplePlacement {
  relative: Point<i32>,
  sign: Point<i32>,
}

impl WarpEdge {
  fn sample_placement(
    self,
    distance: i32,
  ) -> Result<WarpSamplePlacement, console_core_never::Never> {
    Ok(match self {
      Self::Above => WarpSamplePlacement {
        relative: Point { x: distance, y: 0 },
        sign: Point { x: 1, y: -1 },
      },
      Self::Left => WarpSamplePlacement {
        relative: Point { x: 0, y: distance },
        sign: Point { x: -1, y: 1 },
      },
    })
  }

  fn block_axis(self, origin: Point<i32>) -> Result<i32, console_core_never::Never> {
    Ok(match self {
      Self::Above => origin.x,
      Self::Left => origin.y,
    })
  }
}

#[derive(Clone, Copy)]
enum WarpEdgeAlignment {
  BlockOrigin,
  NeighborOrigin,
}

#[derive(Default)]
struct WarpSamples {
  features: [WarpFeaturePair; 8],
  count: u8,
}

impl WarpSamples {
  const LIMIT: u8 = 8;

  fn record(&mut self, neighbor: RefMvsBlock, placement: WarpSamplePlacement) -> Rav1dResult {
    let Ok(&[width, height, ..]) = neighbor.bs.dimensions();
    let [motion, _other] = neighbor.mv.mv;
    let sample_coordinate = |relative: i32, signed_span: i32| {
      relative
        .wrapping_mul(2)
        .wrapping_add(signed_span)
        .wrapping_mul(16)
        .wrapping_sub(8)
    };
    let x = sample_coordinate(
      placement.relative.x,
      placement.sign.x.wrapping_mul(i32::from(width)),
    );
    let y = sample_coordinate(
      placement.relative.y,
      placement.sign.y.wrapping_mul(i32::from(height)),
    );
    let Ok(index) = console_core_number_conversion::index(self.count);
    let slot = self.features.get_mut(index).ok_or(EINVAL)?;
    *slot = [
      [x, y],
      [
        x.wrapping_add(i32::from(motion.x)),
        y.wrapping_add(i32::from(motion.y)),
      ],
    ];
    self.count = self.count.wrapping_add(1);
    Ok(())
  }

  fn record_edge(
    &mut self,
    grid: &WarpNeighborGrid<'_>,
    edge: WarpEdge,
    mask: u64,
    alignment: WarpEdgeAlignment,
  ) -> Rav1dResult {
    match (self.count, mask, alignment) {
      (Self::LIMIT.., _, _) => return Ok(()),
      (_, 1, WarpEdgeAlignment::NeighborOrigin) => {
        let Ok(leading_cell) = edge.neighbor(0);
        let nearest = grid.sample(leading_cell)?;
        let Ok(origin) = edge.block_axis(grid.origin);
        let Ok(span) = edge.neighbor_span(nearest);
        let offset = origin & span.wrapping_sub(1);
        let distance = offset.wrapping_neg();
        let neighbor = match edge {
          WarpEdge::Above => nearest,
          WarpEdge::Left => {
            let Ok(cell) = edge.neighbor(distance);
            grid.sample(cell)?
          }
        };
        let Ok(placement) = edge.sample_placement(distance);
        return self.record(neighbor, placement);
      }
      (_, _, WarpEdgeAlignment::BlockOrigin | WarpEdgeAlignment::NeighborOrigin) => {}
    }

    let mut remaining = mask;

    for _sample in 0..Self::LIMIT {
      match (remaining, self.count) {
        (0, _) | (_, Self::LIMIT..) => break,
        (_, _) => {}
      }

      let distance = i32::try_from(remaining.trailing_zeros()).map_err(|_| EINVAL)?;
      let Ok(cell) = edge.neighbor(distance);
      let neighbor = grid.sample(cell)?;
      let Ok(placement) = edge.sample_placement(distance);
      self.record(neighbor, placement)?;
      remaining &= remaining.wrapping_sub(1);
    }

    Ok(())
  }

  fn record_corner(
    &mut self,
    grid: &WarpNeighborGrid<'_>,
    mask: u64,
    cell: Point<i32>,
    placement: WarpSamplePlacement,
  ) -> Rav1dResult {
    match (
      self.count,
      ::core::primitive::u64::wrapping_shr(mask, 32_u32),
    ) {
      (Self::LIMIT.., _) | (_, 0) => Ok(()),
      (_, _) => {
        let neighbor = grid.sample(cell)?;
        self.record(neighbor, placement)
      }
    }
  }

  fn trim(&mut self, motion: Mv, extent: Size<i32>) -> Rav1dResult<&[WarpFeaturePair]> {
    let Ok(count) = console_core_number_conversion::index(self.count);
    let features = self.features.get_mut(..count).ok_or(EINVAL)?;
    let threshold = extent.width.max(extent.height).clamp(4, 28).wrapping_mul(4);
    let mut kept = [false; 8];
    let mut retained = 0_u8;

    for (keep, &[[x, y], [output_x, output_y]]) in kept.iter_mut().zip(features.iter()) {
      let dx = output_x
        .wrapping_sub(x)
        .wrapping_sub(i32::from(motion.x))
        .wrapping_abs();
      let dy = output_y
        .wrapping_sub(y)
        .wrapping_sub(i32::from(motion.y))
        .wrapping_abs();
      *keep = dx.wrapping_add(dy) <= threshold;
      retained = retained.wrapping_add(u8::from(*keep));
    }

    match retained {
      0 => features.get(..1).ok_or(EINVAL),
      _ => {
        let Ok(prefix_count) = console_core_number_conversion::index(retained);
        let (prefix, tail) = features.split_at_mut_checked(prefix_count).ok_or(EINVAL)?;
        let flags = kept.get(..count).ok_or(EINVAL)?;
        let (prefix_flags, tail_flags) = flags.split_at_checked(prefix_count).ok_or(EINVAL)?;
        let mut replacements = tail
          .iter()
          .zip(tail_flags)
          .rev()
          .filter_map(|(&feature, &keep)| keep.then_some(feature));

        for (feature, &keep) in prefix.iter_mut().zip(prefix_flags) {
          match keep.then_some(()) {
            Some(()) => {}
            None => {
              let checked_value = replacements.next().ok_or(EINVAL)?;
              *feature = checked_value;
            }
          }
        }

        Ok(prefix)
      }
    }
  }
}

fn derive_warpmv(
  grid: &WarpNeighborGrid<'_>,
  extent: Size<i32>,
  masks: [u64; 2],
  motion: Mv,
  mut params: Rav1dWarpedMotionParams,
) -> Rav1dResult<Rav1dWarpedMotionParams> {
  let [above, left] = masks;
  let edge_bits = u64::from(u32::MAX);
  let align_above = match ::core::primitive::u64::wrapping_shr(left, 32_u32) {
    0 => WarpEdgeAlignment::NeighborOrigin,
    _ => WarpEdgeAlignment::BlockOrigin,
  };
  let mut samples = WarpSamples::default();
  samples.record_edge(grid, WarpEdge::Above, above & edge_bits, align_above)?;
  samples.record_edge(
    grid,
    WarpEdge::Left,
    left & edge_bits,
    WarpEdgeAlignment::NeighborOrigin,
  )?;
  samples.record_corner(
    grid,
    left,
    Point { x: -1, y: -1 },
    WarpSamplePlacement {
      relative: Point { x: 0, y: 0 },
      sign: Point { x: -1, y: -1 },
    },
  )?;
  let Ok(right_cell) = WarpEdge::Above.neighbor(extent.width);
  let Ok(right_placement) = WarpEdge::Above.sample_placement(extent.width);
  samples.record_corner(grid, above, right_cell, right_placement)?;

  let selected = samples.trim(motion, extent)?;
  let block = Rectangle {
    origin: grid.origin,
    size: extent,
  };
  let Ok(validity) = match rav1d_find_affine_int(selected, block, motion, &mut params) {
    Ok(WarpValidity::Usable) => rav1d_get_shear_params(&params),
    Ok(WarpValidity::Rejected) => Ok(WarpValidity::Rejected),
  };
  params.r#type = match validity {
    WarpValidity::Usable => Rav1dWarpedMotionType::Affine,
    WarpValidity::Rejected => Rav1dWarpedMotionType::Identity,
  };
  Ok(params)
}

enum OddNeighborPrediction {
  IntraOnly,
  HasInter,
}

#[inline]
fn odd_neighbor_prediction(
  context: &[u8],
) -> Result<OddNeighborPrediction, console_core_never::Never> {
  let (pairs, _unpaired_cell) = context.as_chunks::<2>();
  Ok(
    match pairs.iter().any(|&[_even, odd]| odd == 0).then_some(()) {
      Some(()) => OddNeighborPrediction::HasInter,
      None => OddNeighborPrediction::IntraOnly,
    },
  )
}

enum PaletteColorPriority {
  One(u8),
  Two([u8; 2]),
  Three([u8; 3]),
}

struct PaletteContext {
  index: u8,
  priority: PaletteColorPriority,
}

impl PaletteContext {
  fn order(&self) -> Rav1dResult<[u8; 8]> {
    let first = match &self.priority {
      PaletteColorPriority::One(color) => core::slice::from_ref(color),
      PaletteColorPriority::Two(colors) => colors.as_slice(),
      PaletteColorPriority::Three(colors) => colors.as_slice(),
    };
    let mut ordered = [0u8; 8];
    let mut available = ordered.iter_mut();
    let mut mask = 0u8;

    for &color in first {
      let bit = 1u8.checked_shl(u32::from(color)).ok_or(EINVAL)?;
      let destination = available.next().ok_or(EINVAL)?;
      *destination = color;
      mask |= bit;
    }

    for value in 0u8..8 {
      match mask & 1u8.wrapping_shl(u32::from(value)) {
        0 => {
          let destination = available.next().ok_or(EINVAL)?;
          *destination = value;
        }
        _ => {}
      }
    }

    Ok(ordered)
  }
}

fn compare_palette_neighbors(
  [left, above, diagonal]: [u8; 3],
) -> Result<PaletteContext, console_core_never::Never> {
  let context = match (left == above, above == diagonal, left == diagonal) {
    (true, true, true) => PaletteContext {
      index: 4,
      priority: PaletteColorPriority::One(above),
    },
    (true, false, false) => PaletteContext {
      index: 3,
      priority: PaletteColorPriority::Two([above, diagonal]),
    },
    (false, true, false) => PaletteContext {
      index: 2,
      priority: PaletteColorPriority::Two([diagonal, left]),
    },
    (false, false, true) => PaletteContext {
      index: 2,
      priority: PaletteColorPriority::Two([diagonal, above]),
    },
    _ => PaletteContext {
      index: 1,
      priority: PaletteColorPriority::Three([left.min(above), left.max(above), diagonal]),
    },
  };
  Ok(context)
}

fn palette_sample(indices: &[u8], position: u64) -> Rav1dResult<u8> {
  let Ok(sample) = console_core_number_conversion::index(position);
  indices.get(sample).copied().ok_or(EINVAL)
}

fn palette_context(
  indices: &[u8],
  position: Point<u64>,
  stride: u64,
) -> Rav1dResult<PaletteContext> {
  let offset = position.y.wrapping_mul(stride).wrapping_add(position.x);

  match (position.x, position.y) {
    (0, 0) => Err(EINVAL),
    (0, 1..) => {
      let above = palette_sample(indices, offset.wrapping_sub(stride))?;
      Ok(PaletteContext {
        index: 0,
        priority: PaletteColorPriority::One(above),
      })
    }
    (1.., 0) => {
      let left = palette_sample(indices, offset.wrapping_sub(1))?;
      Ok(PaletteContext {
        index: 0,
        priority: PaletteColorPriority::One(left),
      })
    }
    (1.., 1..) => {
      let left = palette_sample(indices, offset.wrapping_sub(1))?;
      let above = palette_sample(indices, offset.wrapping_sub(stride))?;
      let diagonal = palette_sample(indices, offset.wrapping_sub(stride).wrapping_sub(1))?;
      let Ok(context) = compare_palette_neighbors([left, above, diagonal]);
      Ok(context)
    }
  }
}

struct PaletteIndexInput<'a> {
  staging: &'a mut [u8],
  packed: PaletteOutput<'a>,
  cells: PaletteCellBlock,
  entries: u8,
  plane: PalettePlane,
}

fn read_pal_indices(
  tile: &mut Rav1dTileStateContext,
  dsp: &Rav1dPalDSPContext,
  input: PaletteIndexInput<'_>,
) -> Rav1dResult {
  let PaletteIndexInput {
    staging,
    packed,
    cells,
    entries,
    plane,
  } = input;
  let shape = PaletteBlock::from_cells(cells)?;
  let layouts = shape.layouts()?;
  layouts
    .source
    .validate_len(staging.len())
    .map_err(|_short_staging| EINVAL)?;
  layouts.validate_output(&packed)?;
  let palette_index = match entries {
    2..=8 => entries.wrapping_sub(2),
    _ => return Err(EINVAL),
  };
  let Ok(cdf_index) = console_core_number_conversion::index(palette_index);
  let [luma_cdf, chroma_cdf] = &mut *tile.cdf.m.color_map;
  let entries_cdf = match plane {
    PalettePlane::Luma => luma_cdf.get_mut(cdf_index),
    PalettePlane::Chroma => chroma_cdf.get_mut(cdf_index),
  }
  .ok_or(EINVAL)?;
  let Ok(initial) = rav1d_msac_decode_uniform(&mut tile.msac, u32::from(entries));
  let Ok(initial_index) = console_core_number_conversion::wrapped::<_, u8>(initial);
  let first = staging.first_mut().ok_or(EINVAL)?;
  *first = initial_index;

  decode_palette_diagonals(&mut tile.msac, entries_cdf, staging, shape, entries)?;
  pad_palette_right(staging, shape, &layouts.source)?;
  dsp.pal_idx_finish.call(packed, staging, &layouts)
}

fn decode_palette_diagonals(
  msac: &mut MsacContext,
  cdf: &mut [[u16; 8]; 5],
  staging: &mut [u8],
  shape: PaletteBlock,
  entries: u8,
) -> Rav1dResult {
  let Size { width, height } = shape.visible;
  let end = width.wrapping_add(height).wrapping_sub(1);
  let stride = shape.block.width;
  let threshold_count = entries.wrapping_sub(1);

  for diagonal in 1..end {
    let first = diagonal.wrapping_add(1).saturating_sub(height);
    let last = diagonal.min(width.wrapping_sub(1));

    for column in (first..=last).rev() {
      let row = diagonal.wrapping_sub(column);
      let context = palette_context(staging, Point { x: column, y: row }, stride)?;
      let order = context.order()?;
      let Ok(cdf_index) = console_core_number_conversion::index(context.index);
      let table = cdf.get_mut(cdf_index).ok_or(EINVAL)?;
      let Ok(selected) = rav1d_msac_decode_symbol_adapt8(msac, table, threshold_count);
      let Ok(color_index) = console_core_number_conversion::index(selected);
      let color = order.get(color_index).ok_or(EINVAL)?;
      let Ok(destination) =
        console_core_number_conversion::index(row.wrapping_mul(stride).wrapping_add(column));
      let index = staging.get_mut(destination).ok_or(EINVAL)?;
      *index = *color;
    }
  }

  Ok(())
}

fn pad_palette_right(staging: &mut [u8], shape: PaletteBlock, layout: &RowLayout) -> Rav1dResult {
  let rows = layout.rows_mut(staging).map_err(|_short_staging| EINVAL)?;
  let Ok(width) = console_core_number_conversion::index(shape.visible.width);
  let Ok(height) = console_core_number_conversion::index(shape.visible.height);

  for row in rows.take(height) {
    let (active, padding) = row.split_at_mut_checked(width).ok_or(EINVAL)?;
    let edge = active.last().ok_or(EINVAL)?;
    padding.fill(*edge);
  }

  Ok(())
}

struct VarTx {
  uvtx: TxfmSize,
  max_ytx: TxfmSize,
  tx_split0: u8,
  tx_split1: u16,
}

struct VariableTransformBlock {
  size: BlockSize,
  local: Bxy,
}

fn read_variable_partitions(
  tile: &mut Rav1dTileStateContext,
  neighbors: &mut TransformNeighborhood<'_>,
  frame: Size<i32>,
  block: Rectangle<i32>,
  size: TxfmSize,
) -> Rav1dResult<[u16; 2]> {
  let transform = transform_info(size)?;
  let rows = block
    .size
    .height
    .checked_div(i32::from(transform.h))
    .ok_or(EINVAL)?;
  let columns = block
    .size
    .width
    .checked_div(i32::from(transform.w))
    .ok_or(EINVAL)?;
  let mut split_masks = [0u16; 2];

  for (row, local_y) in (0..rows).zip(0u8..) {
    for (column, local_x) in (0..columns).zip(0u8..) {
      let x = block
        .origin
        .x
        .wrapping_add(column.wrapping_mul(i32::from(transform.w)));
      let y = block
        .origin
        .y
        .wrapping_add(row.wrapping_mul(i32::from(transform.h)));
      let root = TransformNode {
        size,
        origin: Bxy { x, y },
        mask_position: Point {
          x: local_x,
          y: local_y,
        },
        depth: TransformDepth::Root,
      };
      read_tx_tree(tile, neighbors, root, frame, &mut split_masks)?;
    }
  }

  Ok(split_masks)
}

#[inline(never)]
fn read_vartx_tree(
  task: &mut Rav1dTaskContext,
  frame: &Rav1dFrameData,
  tile: &mut Rav1dTileStateContext,
  coded: &Av1Block,
  block: VariableTransformBlock,
) -> Rav1dResult<VarTx> {
  let Ok(&[width, height, log_width, log_height]) = block.size.dimensions();
  let Ok(ordinal) = block.size.ordinal();
  let Ok(position) = console_core_number_conversion::index(ordinal);
  let &[luma, i420, i422, i444] = dav1d_max_txfm_size_for_bs.get(position).ok_or(EINVAL)?;
  let header = frame.frame_hdr.as_ref().ok_or(EINVAL)?;
  let Ok(segment) = coded.seg_id.get();
  let &lossless = header
    .segmentation
    .lossless
    .get(match ::console_core_number_conversion::index(segment) {
      ::core::result::Result::Ok(native_position) => native_position,
    })
    .ok_or(EINVAL)?;
  let above = frame.above_context(task.above_index)?;
  let mut neighbors = TransformNeighborhood {
    above,
    left: &mut task.l,
  };
  let extent = Size { width, height };
  let chroma = match frame.cur.p.layout {
    Rav1dPixelLayout::I400 => luma,
    Rav1dPixelLayout::I420 => i420,
    Rav1dPixelLayout::I422 => i422,
    Rav1dPixelLayout::I444 => i444,
  };

  let (max_ytx, uvtx, masks) = match (
    coded.skip,
    (lossless || luma == TxfmSize::S4x4).then_some(()),
    header.txfm_mode,
  ) {
    (0, Some(()), mode) => {
      match mode {
        Rav1dTxfmMode::Switchable => update_transform_context::<32>(
          &mut neighbors,
          block.local,
          extent,
          [TxfmSize::S4x4; 2],
        )?,
        Rav1dTxfmMode::Only4x4 | Rav1dTxfmMode::Largest => {}
      }

      (TxfmSize::S4x4, TxfmSize::S4x4, [0; 2])
    }
    (0, None, Rav1dTxfmMode::Switchable) => {
      let rect = Rectangle {
        origin: Point {
          x: task.b.x,
          y: task.b.y,
        },
        size: Size {
          width: i32::from(width),
          height: i32::from(height),
        },
      };
      let frame_extent = Size {
        width: frame.bw,
        height: frame.bh,
      };
      let masks = read_variable_partitions(tile, &mut neighbors, frame_extent, rect, luma)?;

      match {
        let Ok(active_trace) = debug_block_info(frame, task.b);
        active_trace
      } {
        BlockTrace::Enabled => {
          let [root_mask, child_mask] = masks;
          println!(
            "Post-vartxtree[{root_mask}/{child_mask}]: r={}",
            tile.msac.rng
          );
        }
        BlockTrace::Disabled => {}
      }

      (luma, chroma, masks)
    }
    (1.., _, Rav1dTxfmMode::Switchable) => {
      let checked_value = square_transform(log_height)?;
      let values = [checked_value, {
        let checked_value_3 = square_transform(log_width)?;
        checked_value_3
      }];
      update_transform_context::<32>(&mut neighbors, block.local, extent, values)?;
      (luma, chroma, [0; 2])
    }
    (_, _, Rav1dTxfmMode::Only4x4 | Rav1dTxfmMode::Largest) => (luma, chroma, [0; 2]),
  };

  let [root_mask, tx_split1] = masks;

  match root_mask & !0x33 {
    0 => {}
    _ => return Err(EINVAL),
  }

  let tx_split0 = u8::try_from(root_mask).map_err(|_too_many_root_splits| EINVAL)?;

  Ok(VarTx {
    uvtx,
    max_ytx,
    tx_split0,
    tx_split1,
  })
}

fn get_prev_frame_segid(
  frame_hdr: &Rav1dFrameHeader,
  previous_map: Option<&DisjointMutSlice<SegmentId>>,
  block: Rectangle<i32>,
  stride: i64,
) -> Rav1dResult<SegmentId> {
  let map = match previous_map {
    Some(previous) => previous,
    None => return Ok(SegmentId::default()),
  };

  match frame_hdr.primary_ref_frame {
    RAV1D_PRIMARY_REF_NONE => return Err(EINVAL),
    _ => {}
  }

  let origin = Point {
    x: {
      let checked_value_8 = u64::try_from(block.origin.x).map_err(|_negative_column| EINVAL)?;
      checked_value_8
    },
    y: {
      let checked_value_9 = u64::try_from(block.origin.y).map_err(|_negative_row| EINVAL)?;
      checked_value_9
    },
  };
  let width = u64::try_from(block.size.width).map_err(|_negative_width| EINVAL)?;
  let height = u64::try_from(block.size.height).map_err(|_negative_height| EINVAL)?;
  let row_stride = u64::try_from(stride).map_err(|_negative_stride| EINVAL)?;
  let Ok(lowest_segment) = SegmentId::min();
  let Ok(mut lowest_seen) = SegmentId::max();
  let Ok(map_length) = console_core_number_conversion::fixed_index(map.len());
  let row_end = origin.y.checked_add(height).ok_or(EINVAL)?;

  for row in origin.y..row_end {
    let start = row
      .checked_mul(row_stride)
      .and_then(|base| base.checked_add(origin.x))
      .ok_or(EINVAL)?;
    let end = start.checked_add(width).ok_or(EINVAL)?;

    match end.cmp(&map_length) {
      cmp::Ordering::Less | cmp::Ordering::Equal => {}
      cmp::Ordering::Greater => return Err(EINVAL),
    }

    let Ok(window) = console_core_number_conversion::indices(start..end);
    lowest_seen = map
      .index(window)
      .iter()
      .copied()
      .fold(lowest_seen, cmp::min);

    match (lowest_seen == lowest_segment).then_some(()) {
      Some(()) => break,
      None => {}
    }
  }

  Ok(lowest_seen)
}

#[inline]
fn splat_oneref_mv(
  c: &Rav1dContext,
  t: &Rav1dTaskContext,
  rf: &RefMvsFrame,
  bs: BlockSize,
  inter: &Av1BlockInter,
  extent: Size<i32>,
) -> Rav1dResult {
  let mode = inter.inter_mode;
  let tmpl = Aligned(RefMvsBlock {
    mv: RefMvsMvPair {
      mv: [
        match inter.nd.one_d.mv {
          [_explicit_element, ..] => _explicit_element,
        },
        Mv::ZERO,
      ],
    },
    r#ref: RefMvsRefPair {
      r#ref: [
        i8::wrapping_add(
          match inter.r#ref {
            [element, ..] => element,
          },
          1,
        ),
        match inter.interintra_type.map(|_| 0) {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => -1,
        },
      ],
    },
    bs,
    mf: u8::from(mode == GLOBALMV && cmp::min(extent.width, extent.height) >= 2)
      | u8::wrapping_mul(u8::from(mode == NEWMV), 2),
  });

  c.dsp.refmvs.splat_mv.call(rf, &t.rt, &tmpl, t.b, extent)
}

#[inline]
fn splat_intrabc_mv(
  c: &Rav1dContext,
  t: &Rav1dTaskContext,
  rf: &RefMvsFrame,
  bs: BlockSize,
  r#ref: Mv,
  extent: Size<i32>,
) -> Rav1dResult {
  let tmpl = Aligned(RefMvsBlock {
    mv: RefMvsMvPair {
      mv: [r#ref, Mv::ZERO],
    },
    r#ref: RefMvsRefPair { r#ref: [0, -1] },
    bs,
    mf: 0,
  });
  c.dsp.refmvs.splat_mv.call(rf, &t.rt, &tmpl, t.b, extent)
}

#[inline]
fn splat_tworef_mv(
  c: &Rav1dContext,
  t: &Rav1dTaskContext,
  rf: &RefMvsFrame,
  bs: BlockSize,
  inter: &Av1BlockInter,
  extent: Size<i32>,
) -> Rav1dResult {
  assert!(extent.width >= 2 && extent.height >= 2);
  let mode = inter.inter_mode;
  let tmpl = Aligned(RefMvsBlock {
    mv: RefMvsMvPair {
      mv: inter.nd.one_d.mv,
    },
    r#ref: RefMvsRefPair {
      r#ref: [
        i8::wrapping_add(
          match inter.r#ref {
            [element, ..] => element,
          },
          1,
        ),
        i8::wrapping_add(
          match inter.r#ref {
            [_, element, ..] => element,
          },
          1,
        ),
      ],
    },
    bs,
    mf: u8::from(mode == GLOBALMV_GLOBALMV)
      | u8::wrapping_mul(
        u8::from(
          i32::wrapping_shl(1, {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(mode);
            numeric_value
          }) & 0xbc
            != 0,
        ),
        2,
      ),
  });
  c.dsp.refmvs.splat_mv.call(rf, &t.rt, &tmpl, t.b, extent)
}

#[inline]
fn splat_intraref(
  c: &Rav1dContext,
  t: &Rav1dTaskContext,
  rf: &RefMvsFrame,
  bs: BlockSize,
  extent: Size<i32>,
) -> Rav1dResult {
  let tmpl = Aligned(RefMvsBlock {
    mv: RefMvsMvPair {
      mv: [Mv::INVALID, Mv::ZERO],
    },
    r#ref: RefMvsRefPair { r#ref: [0, -1] },
    bs,
    mf: 0,
  });
  c.dsp.refmvs.splat_mv.call(rf, &t.rt, &tmpl, t.b, extent)
}

struct MotionVerticalExtent {
  start: c_int,
  height: c_int,
  subsampling: c_int,
}

fn mc_lowest_px(
  dst: &mut c_int,
  MotionVerticalExtent {
    start: by4,
    height: bh4,
    subsampling: ss_ver,
  }: MotionVerticalExtent,
  mvy: i16,
  smp: &ScalableMotionParams,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let Ok(mvy_2) = console_core_number_conversion::wrapped::<_, i32>(mvy);

    let v_mul = i32::wrapping_shr(4, {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
      numeric_value
    });

    match ::core::primitive::bool::then_some(smp.scale == 0, ()) {
      ::core::option::Option::Some(()) => {
        let my = i32::wrapping_shr(mvy_2, {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_add(3, ss_ver));
          numeric_value
        });
        let dy = mvy_2
          & i32::wrapping_shr(15, {
            let Ok(numeric_value) =
              console_core_number_conversion::wrapped::<_, u32>(i32::from(ss_ver == 0));
            numeric_value
          });
        *dst = cmp::max(
          *dst,
          i32::wrapping_add(
            i32::wrapping_add(i32::wrapping_mul(i32::wrapping_add(by4, bh4), v_mul), my),
            i32::wrapping_mul(4, i32::from(dy != 0)),
          ),
        );
      }
      ::core::option::Option::None => {
        let mut y = i32::wrapping_add(
          i32::wrapping_shl(i32::wrapping_mul(by4, v_mul), 4_u32),
          i32::wrapping_mul(
            mvy_2,
            i32::wrapping_shl(1, {
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped::<_, u32>(i32::from(ss_ver == 0));
              numeric_value
            }),
          ),
        );
        let tmp = i64::wrapping_add(
          i64::wrapping_mul(
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i64>(y);
              numeric_value
            },
            {
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped::<_, i64>(smp.scale);
              numeric_value
            },
          ),
          {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i64>(
              i32::wrapping_mul(i32::wrapping_sub(smp.scale, 0x4000), 8),
            );
            numeric_value
          },
        );
        y = i32::wrapping_add(
          apply_sign64(
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(
                i64::wrapping_shr(i64::wrapping_add(tmp.abs(), 128), 8_u32),
              );
              numeric_value
            },
            tmp,
          ),
          32,
        );
        let bottom = i32::wrapping_add(
          i32::wrapping_add(
            i32::wrapping_shr(
              i32::wrapping_add(
                y,
                i32::wrapping_mul(
                  i32::wrapping_sub(i32::wrapping_mul(bh4, v_mul), 1),
                  smp.step,
                ),
              ),
              10_u32,
            ),
            1,
          ),
          4,
        );
        *dst = cmp::max(*dst, bottom);
      }
    };
  };

  Ok(())
}

fn affine_lowest_px(
  task: &Rav1dTaskContext,
  bottom: &mut i32,
  &[width, height, ..]: &[u8; 4],
  motion: &Rav1dWarpedMotionParams,
  subsampling: Point<u32>,
) -> Rav1dResult {
  let width_pixels = i32::from(width).wrapping_mul(4_i32.wrapping_shr(subsampling.x));
  let height_pixels = i32::from(height).wrapping_mul(4_i32.wrapping_shr(subsampling.y));

  match (width_pixels, height_pixels) {
    (8.., 8..) => {}
    (..=7, _) | (_, ..=7) => return Err(EINVAL),
  }

  match (width_pixels & 7) | (height_pixels & 7) {
    0 => {}
    ..=-1 | 1.. => return Err(EINVAL),
  }

  let [_, offset_y, _, _, yx, yy] = motion.matrix;
  let row = task
    .b
    .y
    .wrapping_mul(4)
    .wrapping_add(height_pixels.wrapping_sub(4).wrapping_shl(subsampling.y));
  let vertical_motion = i64::from(yy)
    .wrapping_mul(i64::from(row))
    .wrapping_add(i64::from(offset_y));

  for x in [0_i32, width_pixels.wrapping_sub(8)] {
    let column = task
      .b
      .x
      .wrapping_mul(4)
      .wrapping_add(x.wrapping_add(4).wrapping_shl(subsampling.x));
    let reference_y = i64::from(yx)
      .wrapping_mul(i64::from(column))
      .wrapping_add(vertical_motion)
      .wrapping_shr(subsampling.y)
      .wrapping_shr(16);
    let Ok(reference_row) = console_core_number_conversion::wrapped::<_, i32>(reference_y);
    *bottom = (*bottom).max(reference_row.wrapping_add(8));
  }

  Ok(())
}

fn affine_lowest_px_luma(
  task: &Rav1dTaskContext,
  bottom: &mut i32,
  dimensions: &[u8; 4],
  motion: &Rav1dWarpedMotionParams,
) -> Rav1dResult {
  affine_lowest_px(task, bottom, dimensions, motion, Point { x: 0, y: 0 })
}

fn affine_lowest_px_chroma(
  task: &Rav1dTaskContext,
  layout: Rav1dPixelLayout,
  bottom: &mut i32,
  dimensions: &[u8; 4],
  motion: &Rav1dWarpedMotionParams,
) -> Rav1dResult {
  let subsampling = match layout {
    Rav1dPixelLayout::I400 => return Err(EINVAL),
    Rav1dPixelLayout::I420 => Point { x: 1, y: 1 },
    Rav1dPixelLayout::I422 => Point { x: 1, y: 0 },
    Rav1dPixelLayout::I444 => Point { x: 0, y: 0 },
  };
  affine_lowest_px(task, bottom, dimensions, motion, subsampling)
}

#[derive(Clone, Copy)]
enum OverlapPlane {
  Luma,
  Chroma,
}

struct OverlapBlock {
  visible: Size<i32>,
  dimensions: [u8; 4],
  plane: OverlapPlane,
}

fn track_overlap_neighbor(
  dst: &mut [[c_int; 2]; 7],
  scale: &[[ScalableMotionParams; 2]; 7],
  plane: OverlapPlane,
  vertical_subsampling: i32,
  coverage: core::ops::Range<i32>,
  block: RefMvsBlock,
) -> Rav1dResult {
  let [reference, _] = block.r#ref.r#ref;
  let ordinal = u8::try_from(reference.wrapping_sub(1)).map_err(|_invalid_reference| EINVAL)?;
  let Ok(reference_index) = console_core_number_conversion::index(ordinal);
  let [luma, chroma] = dst.get_mut(reference_index).ok_or(EINVAL)?;
  let [_, vertical] = scale.get(reference_index).ok_or(EINVAL)?;
  let [motion, _] = block.mv.mv;
  let destination = match plane {
    OverlapPlane::Luma => luma,
    OverlapPlane::Chroma => chroma,
  };
  let Ok(()) = mc_lowest_px(
    destination,
    MotionVerticalExtent {
      start: coverage.start,
      height: coverage.end.wrapping_sub(coverage.start),
      subsampling: vertical_subsampling,
    },
    motion.y,
    vertical,
  );
  Ok(())
}

fn obmc_lowest_px(
  f: &Rav1dFrameData,
  t: &Rav1dTaskContext,
  dst: &mut [[c_int; 2]; 7],
  OverlapBlock {
    visible,
    dimensions: [width, height, horizontal_limit, vertical_limit],
    plane,
  }: OverlapBlock,
) -> Rav1dResult {
  match { ::core::primitive::bool::then_some(t.b.x & 1 != 0 || t.b.y & 1 != 0, ()) } {
    ::core::option::Option::Some(()) => {
      return Err(EINVAL);
    }
    ::core::option::Option::None => {}
  }

  let tile = f.tile(t.tile_index)?;
  let origin = Point { x: t.b.x, y: t.b.y };
  let grid = WarpNeighborGrid {
    blocks: &f.rf.r,
    tile: &t.rt,
    origin,
  };
  let layout = f.cur.p.layout;
  let block_width = i32::from(width);
  let block_height = i32::from(height);
  let (horizontal_shift, vertical_shift) = match plane {
    OverlapPlane::Luma => (0, 0),
    OverlapPlane::Chroma => (
      u32::from(layout != Rav1dPixelLayout::I444),
      u32::from(layout == Rav1dPixelLayout::I420),
    ),
  };
  let Ok(vertical_subsampling) =
    console_core_number_conversion::wrapped::<_, i32>(vertical_shift);
  let fits_above = match plane {
    OverlapPlane::Luma => true,
    OverlapPlane::Chroma => {
      block_width
        .wrapping_mul(4_i32.wrapping_shr(horizontal_shift))
        .wrapping_add(block_height.wrapping_mul(4_i32.wrapping_shr(vertical_shift)))
        >= 16
    }
  };

  match { ::core::primitive::bool::then_some(t.b.y > tile.tiling.row_start && fits_above, ()) } {
    ::core::option::Option::Some(()) => {
      let count_limit = horizontal_limit.min(4);
      let overlap_height = block_height
        .min(16)
        .wrapping_shr(1)
        .wrapping_mul(3)
        .wrapping_add(3)
        .wrapping_shr(2);
      let mut distance = 0_i32;
      let mut count = 0_u8;

      for _ in 0..visible.width {
        match {
          ::core::primitive::bool::then_some(
            distance >= visible.width || count >= count_limit,
            (),
          )
        } {
          ::core::option::Option::Some(()) => {
            break;
          }
          ::core::option::Option::None => {}
        }

        let block = grid.sample(Point {
          x: distance.wrapping_add(1),
          y: -1,
        })?;
        let [reference, _] = block.r#ref.r#ref;

        match { ::core::primitive::bool::then_some(reference > 0, ()) } {
          ::core::option::Option::Some(()) => {
            let start = t.b.y;
            track_overlap_neighbor(
              dst,
              &f.svc,
              plane,
              vertical_subsampling,
              start..start.wrapping_add(overlap_height),
              block,
            )?;
            count = count.wrapping_add(1);
          }
          ::core::option::Option::None => {}
        }

        let Ok(&[neighbor_width, ..]) = block.bs.dimensions();
        distance = distance.wrapping_add(i32::from(neighbor_width).max(2));
      }
    }
    ::core::option::Option::None => {}
  }

  match { ::core::primitive::bool::then_some(t.b.x > tile.tiling.col_start, ()) } {
    ::core::option::Option::Some(()) => {
      let count_limit = vertical_limit.min(4);
      let mut distance = 0_i32;
      let mut count = 0_u8;

      for _ in 0..visible.height {
        match {
          ::core::primitive::bool::then_some(
            distance >= visible.height || count >= count_limit,
            (),
          )
        } {
          ::core::option::Option::Some(()) => {
            break;
          }
          ::core::option::Option::None => {}
        }

        let block = grid.sample(Point {
          x: -1,
          y: distance.wrapping_add(1),
        })?;
        let [reference, _] = block.r#ref.r#ref;
        let Ok(&[_, neighbor_height, ..]) = block.bs.dimensions();
        let neighbor_height_2 = i32::from(neighbor_height);

        match reference {
          1.. => {
            let start = t.b.y.wrapping_add(distance);
            let overlap_height = iclip(neighbor_height_2, 2, block_height);
            track_overlap_neighbor(
              dst,
              &f.svc,
              plane,
              vertical_subsampling,
              start..start.wrapping_add(overlap_height),
              block,
            )?;
            count = count.wrapping_add(1);
          }
          _ => {}
        }

        distance = distance.wrapping_add(neighbor_height_2.max(2));
      }
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn cdef_quadrant(block: Bxy) -> Result<u8, console_core_never::Never> {
  let column = (block.x & 16).wrapping_shr(4);
  let row = (block.y & 16).wrapping_shr(3);
  console_core_number_conversion::wrapped::<_, u8>(column.wrapping_add(row))
}

fn read_block_cdef(
  f: &Rav1dFrameData,
  t: &Rav1dTaskContext,
  msac: &mut MsacContext,
  size: Size<i32>,
) -> Rav1dResult {
  let Ok(q) = cdef_quadrant(t.b);
  let sequence = f.seq_hdr.as_ref().ok_or(EINVAL)?;
  let frame = f.frame_hdr.as_ref().ok_or(EINVAL)?;
  let filter = f.loop_filter_mask(t.lf_mask)?;
  let indices = &filter.cdef_idx;
  let quadrant = match sequence.sb128 {
    0 => t.cur_sb_cdef_idx,
    _ => t.cur_sb_cdef_idx.wrapping_add(q),
  };
  let Ok(current) = console_core_number_conversion::index(quadrant);
  let current_value = indices.get(current).ok_or(EINVAL)?;
  let Ok(value) = current_value.get();

  match value {
    -1 => {}
    _ => return Ok(()),
  }

  let Ok(strength) = rav1d_msac_decode_bools(msac, frame.cdef.n_bits);
  let Ok(decoded) = console_core_number_conversion::wrapped::<_, i8>(strength);

  let updates = [
    (0u8, true),
    (1, size.width > 16),
    (2, size.height > 16),
    (3, size.width == 32 && size.height == 32),
  ];
  for (delta, update) in updates {
    match update.then_some(()) {
      None => continue,
      Some(()) => {
        let Ok(position) = console_core_number_conversion::index(quadrant.wrapping_add(delta));
        let entry = indices.get(position).ok_or(EINVAL)?;
        let Ok(()) = entry.set(decoded);
      }
    }
  }

  match {
    let Ok(active_trace) = debug_block_info(f, t.b);
    active_trace
  } {
    BlockTrace::Enabled => {
      let Ok(position) = console_core_number_conversion::index(t.cur_sb_cdef_idx);
      let entry = indices.get(position).ok_or(EINVAL)?;
      let Ok(index) = entry.get();
      console_core_logging::trace!("Post-cdef_idx[{index}]: r={}", msac.rng);
    }
    BlockTrace::Disabled => {}
  }

  Ok(())
}

fn reset_superblock_cdef(
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
  level: BlockLevel,
) -> Rav1dResult {
  let filter = f.loop_filter_mask(t.lf_mask)?;

  match level {
    BlockLevel::Bl128x128 => {
      for entry in &filter.cdef_idx {
        let Ok(()) = entry.set(-1);
      }

      t.cur_sb_cdef_idx = 0;
    }
    BlockLevel::Bl64x64 | BlockLevel::Bl32x32 | BlockLevel::Bl16x16 | BlockLevel::Bl8x8 => {
      let Ok(quadrant) = cdef_quadrant(t.b);
      let Ok(position) = console_core_number_conversion::index(quadrant);
      let entry = filter.cdef_idx.get(position).ok_or(EINVAL)?;
      t.cur_sb_cdef_idx = quadrant;
      let Ok(()) = entry.set(-1);
    }
  }

  Ok(())
}

fn write_block_segments(
  f: &Rav1dFrameData,
  block: Rectangle<i32>,
  segment: SegmentId,
) -> Rav1dResult {
  let Rectangle { origin, size } = block;
  let row = u64::try_from(origin.y).map_err(|_outside_segment_map| EINVAL)?;
  let column = u64::try_from(origin.x).map_err(|_outside_segment_map| EINVAL)?;
  let width = u64::try_from(size.width).map_err(|_invalid_width| EINVAL)?;
  let height = u64::try_from(size.height).map_err(|_invalid_height| EINVAL)?;
  let stride = u64::from(f.b4_stride);
  let end_column = column.checked_add(width).ok_or(EINVAL)?;
  let end_row = row.checked_add(height).ok_or(EINVAL)?;
  let cur_segmap = f.cur_segmap.as_ref().ok_or(EINVAL)?;
  let map = &cur_segmap.inner;
  let Ok(length) = console_core_number_conversion::fixed_index(map.len());
  let end = end_row
    .saturating_sub(1)
    .checked_mul(stride)
    .and_then(|start| start.checked_add(end_column))
    .ok_or(EINVAL)?;

  match (end_column <= stride && end <= length).then_some(()) {
    Some(()) => {}
    None => return Err(EINVAL),
  }

  let Ok(count) = console_core_number_conversion::index(width);

  for y in row..end_row {
    let offset = y.wrapping_mul(stride).wrapping_add(column);
    let Ok(start) = console_core_number_conversion::index(offset);
    let mut cells = map.index_mut((start.., ..count));
    let Ok(()) = crate::src::ctx::small_memset::<_, 32, false>(&mut cells, segment);
  }

  Ok(())
}

struct ChromaScaleSigns {
  current: u8,
  opposite: u8,
}

fn read_chroma_scale(
  tile: &mut Rav1dTileStateContext,
  ChromaScaleSigns { current, opposite }: ChromaScaleSigns,
) -> Result<i8, ()> {
  match current {
    0 => return Ok(0),
    _ => {}
  }

  let context = u8::from(current == 2)
    .wrapping_mul(3)
    .wrapping_add(opposite);
  let Ok(table) = console_core_number_conversion::index(context);
  let cdf = tile.cdf.m.cfl_alpha.get_mut(table).ok_or(())?;
  let Ok(symbol) = rav1d_msac_decode_symbol_adapt16(&mut tile.msac, cdf, 15);
  let Ok(magnitude) = console_core_number_conversion::wrapped::<_, i8>(symbol.wrapping_add(1));
  let scale = match current {
    1 => magnitude.wrapping_neg(),
    _ => magnitude,
  };
  Ok(scale)
}

fn bottom_motion_span(
  task: &Rav1dTaskContext,
  block: Size<c_int>,
) -> Rav1dResult<std::ops::Range<u64>> {
  let width = u64::try_from(block.width).map_err(|_| EINVAL)?;
  let height = u64::try_from(block.height).map_err(|_| EINVAL)?;
  let x = u64::try_from(task.b.x).map_err(|_| EINVAL)?;
  let ring_row = u64::from(task.b.y.cast_unsigned() & 31);
  let bottom = ring_row
    .wrapping_add(5)
    .wrapping_add(height)
    .saturating_sub(1);
  let Ok(row) = console_core_number_conversion::index(bottom);
  let row_offset = task.rt.r.get(row).copied().ok_or(EINVAL)?;
  let start = row_offset.checked_add(x).ok_or(EINVAL)?;
  let end = start.checked_add(width).ok_or(EINVAL)?;
  Ok(start..end)
}

enum SegmentPrediction {
  Open,
  Forced,
}

fn segment_prediction(
  segment: Option<&Rav1dSegmentationData>,
) -> Result<SegmentPrediction, console_core_never::Never> {
  Ok(match segment {
    None
    | Some(Rav1dSegmentationData {
      globalmv: 0,
      r#ref: -1,
      skip: 0,
      ..
    }) => SegmentPrediction::Open,
    Some(_) => SegmentPrediction::Forced,
  })
}

enum WarpAvailability {
  Disabled,
  Allowed,
}

fn read_motion_mode(
  msac: &mut MsacContext,
  probabilities: &mut CdfModeInterContext,
  block: BlockSize,
  warp: WarpAvailability,
) -> Rav1dResult<MotionMode> {
  let Ok(ordinal) = block.ordinal();
  let Ok(position) = console_core_number_conversion::index(ordinal);

  match warp {
    WarpAvailability::Allowed => {
      let table = probabilities.motion_mode.get_mut(position).ok_or(EINVAL)?;
      let Ok(code) = rav1d_msac_decode_symbol_adapt4(msac, table, 2);

      match code {
        0 => Ok(MotionMode::Translation),
        1 => Ok(MotionMode::Obmc),
        2 => Ok(MotionMode::Warp),
        _ => Err(EINVAL),
      }
    }
    WarpAvailability::Disabled => {
      let table = probabilities.obmc.get_mut(position).ok_or(EINVAL)?;
      let Ok(code) = rav1d_msac_decode_bool_adapt(msac, table);

      match code {
        EntropyBit::Zero => Ok(MotionMode::Translation),
        EntropyBit::One => Ok(MotionMode::Obmc),
      }
    }
  }
}

fn capture_block_warp(params: &Rav1dWarpedMotionParams) -> Rav1dResult<BlockWarp> {
  match params.r#type {
    Rav1dWarpedMotionType::Identity
    | Rav1dWarpedMotionType::Translation
    | Rav1dWarpedMotionType::RotZoom => Ok(BlockWarp::Identity),
    Rav1dWarpedMotionType::Affine => {
      let [_, _, xx, xy, yx, yy] = params.matrix;
      let source = [xx.wrapping_sub(0x10000), xy, yx, yy.wrapping_sub(0x10000)];
      let mut coefficients = [0; 4];

      for (parameter, coefficient) in source.into_iter().zip(&mut coefficients) {
        match parameter {
          -8192..=8191 => {
            let Ok(packed) = console_core_number_conversion::wrapped::<_, i16>(parameter);
            *coefficient = packed;
          }
          ..=-8193 | 8192.. => return Err(EINVAL),
        }
      }

      Ok(BlockWarp::Affine(coefficients))
    }
  }
}

struct SignAbs(i32);

impl std::fmt::Display for SignAbs {
  fn fmt(&self, out: &mut std::fmt::Formatter) -> std::fmt::Result {
    let sign = match ::core::primitive::bool::then_some(self.0 < 0, ()) {
      ::core::option::Option::Some(()) => "-",
      ::core::option::Option::None => " ",
    };
    write!(out, "{}{:x}", sign, self.0.abs())
  }
}

struct ReconstructionBlock {
  size: BlockSize,
  edge_flags: EdgeFlags,
  frame_type: Rav1dFrameType,
  luma: Rectangle<i32>,
  chroma: Option<Rectangle<i32>>,
}

fn trace_motion_warp(
  frame: &Rav1dFrameData,
  task: &Rav1dTaskContext,
  motion: Mv,
) -> Result<(), console_core_never::Never> {
  let Ok(trace) = debug_block_info(frame, task.b);

  match trace {
    BlockTrace::Disabled => {}
    BlockTrace::Enabled => {
      let [x, y, xx, xy, yx, yy] = task.warpmv.matrix.map(SignAbs);
      let alpha = SignAbs(i32::from(task.warpmv.alpha()));
      let beta = SignAbs(i32::from(task.warpmv.beta()));
      let gamma = SignAbs(i32::from(task.warpmv.gamma()));
      let delta = SignAbs(i32::from(task.warpmv.delta()));
      println!(
        "[ {x} {y} {xx}\n  {xy} {yx} {yy} ]\nalpha={alpha}, beta={beta}, gamma={gamma}, deta={delta}, mv=y:{},x:{}",
        motion.y, motion.x,
      );
    }
  }

  Ok(())
}

fn restore_block_warp(
  frame: &Rav1dFrameData,
  task: &mut Rav1dTaskContext,
  inter: &Av1BlockInter,
  dimensions: Size<i32>,
) -> Result<(), console_core_never::Never> {
  let Ok(two_d) = inter.nd.two_d();
  let Ok(warp) = two_d.warp();

  match warp {
    BlockWarp::Identity => task.warpmv.r#type = Rav1dWarpedMotionType::Identity,
    BlockWarp::Affine([xx, xy, yx, yy]) => {
      task.warpmv.r#type = Rav1dWarpedMotionType::Affine;
      let [x, y, ..] = task.warpmv.matrix;
      task.warpmv.matrix = [
        x,
        y,
        i32::from(xx).wrapping_add(0x10000),
        i32::from(xy),
        i32::from(yx),
        i32::from(yy).wrapping_add(0x10000),
      ];
      let area = Rectangle {
        origin: Point {
          x: task.b.x,
          y: task.b.y,
        },
        size: dimensions,
      };
      let Ok(()) = rav1d_set_affine_mv2d(area, two_d.mv2d, &mut task.warpmv);
      let Ok(WarpValidity::Usable | WarpValidity::Rejected) =
        rav1d_get_shear_params(&task.warpmv);
      let Ok(()) = trace_motion_warp(frame, task, two_d.mv2d);
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum ReconstructedMotion {
  Intra,
  Inter { reference: i8, motion: Mv },
}

impl ReconstructedMotion {
  fn install(
    self,
    block: &mut RefMvsBlock,
    size: BlockSize,
  ) -> Result<(), console_core_never::Never> {
    let [reference, ..] = &mut block.r#ref.r#ref;

    match self {
      Self::Intra => *reference = 0,
      Self::Inter {
        reference: selected,
        motion,
      } => {
        *reference = selected.wrapping_add(1);
        let [first, ..] = &mut block.mv.mv;
        *first = motion;
      }
    }

    block.bs = size;
    Ok(())
  }
}

fn write_reconstructed_motion_edges(
  frame: &Rav1dFrameData,
  task: &Rav1dTaskContext,
  size: BlockSize,
  extent: Size<i32>,
  motion: ReconstructedMotion,
) -> Result<(), ()> {
  let span = bottom_motion_span(task, extent).map_err(|_invalid_motion_row| ())?;
  let Ok(columns) = console_core_number_conversion::indices(span);
  let mut bottom = frame.rf.r.index_mut(columns);

  for cell in &mut *bottom {
    let Ok(()) = motion.install(cell, size);
  }

  drop(bottom);
  let first = (task.b.y & 31).wrapping_add(5);
  let Ok(first_index) = console_core_number_conversion::wrapped_index(first);
  let Ok(height) = console_core_number_conversion::wrapped_index(extent.height);
  let rows = task.rt.r.get(first_index..).ok_or(())?;
  let right = rows.get(..height.wrapping_sub(1)).ok_or(())?;
  let Ok(x) = console_core_number_conversion::wrapped::<_, u64>(task.b.x);
  let Ok(width) = console_core_number_conversion::wrapped::<_, u64>(extent.width);

  for &row_base in right {
    let address = row_base.wrapping_add(x).wrapping_add(width).wrapping_sub(1);
    let Ok(offset) = console_core_number_conversion::wrapped_index(address);
    let mut cell = frame.rf.r.index_mut(offset);
    let Ok(()) = motion.install(&mut cell, size);
  }

  Ok(())
}

fn reconstruct_decoded_block(
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
  ta: &BlockContext,
  b: &Av1Block,
  geometry: ReconstructionBlock,
) -> Result<(), ()> {
  let ReconstructionBlock {
    size: bs,
    edge_flags: intra_edge_flags,
    frame_type,
    luma:
      Rectangle {
        origin: Point { x: bx4, y: by4 },
        size: Size {
          width: bw4,
          height: bh4,
        },
      },
    chroma,
  } = geometry;
  let Ok(bd_fn) = f.bd_fn();

  let extent = Size {
    width: bw4,
    height: bh4,
  };
  let Ok(left_rows) = console_core_number_conversion::wrapped_index(bh4);
  let Ok(left_start) = console_core_number_conversion::wrapped_index(by4);
  let Ok(above_columns) = console_core_number_conversion::wrapped_index(bw4);
  let Ok(above_start) = console_core_number_conversion::wrapped_index(bx4);

  let (motion, uv_mode) = match &b.ii {
    Av1BlockIntraInter::Intra(intra) => {
      (bd_fn.recon_b_intra)(f, t, None, bs, intra_edge_flags, b, intra)
        .map_err(|_invalid_reconstruction| ())?;
      let mode = match (intra.y_mode == FILTER_PRED).then_some(()) {
        Some(()) => DC_PRED,
        None => intra.y_mode,
      };
      case_set_al! {
        <32, false>
        l: (&mut t.l, left_rows, left_start),
        a: (ta, above_columns, above_start),
        mode = (mode, mode),
        intra = (1, 1),
      }

      (ReconstructedMotion::Intra, intra.uv_mode)
    }
    Av1BlockIntraInter::Inter(inter) => {
      match (frame_type.is_inter_or_switch()
        && inter.comp_type.is_none()
        && inter.motion_mode == MotionMode::Warp)
        .then_some(())
      {
        Some(()) => {
          let Ok(()) = restore_block_warp(f, t, inter, extent);
        }
        None => {}
      }

      (bd_fn.recon_b_inter)(f, t, None, bs, b, inter)?;
      let Ok(filter_index) = console_core_number_conversion::index(
        crate::src::enum_map::EnumKey::ordinal(inter.filter2d),
      );
      let &[horizontal, vertical] = dav1d_filter_dir.get(filter_index).ok_or(())?;
      case_set_al! {
        <32, false>
        l: (&mut t.l, left_rows, left_start),
        a: (ta, above_columns, above_start),
        filter[0] = (horizontal, horizontal),
        filter[1] = (vertical, vertical),
        intra = (0, 0),
      }

      let [reference, ..] = inter.r#ref;
      let [motion, ..] = inter.nd.one_d.mv;
      (ReconstructedMotion::Inter { reference, motion }, DC_PRED)
    }
  };

  match frame_type.is_inter_or_switch().then_some(()) {
    Some(()) => write_reconstructed_motion_edges(f, t, bs, extent, motion)?,
    None => {}
  }

  match chroma {
    Some(Rectangle {
      origin: Point { x, y },
      size: Size { width, height },
    }) => {
      let Ok(row_count) = console_core_number_conversion::wrapped_index(height);
      let Ok(column_count) = console_core_number_conversion::wrapped_index(width);
      let Ok(left_row) = console_core_number_conversion::wrapped_index(y);
      let Ok(above_column) = console_core_number_conversion::wrapped_index(x);
      case_set_al! {
        <32, false>
        l: (&mut t.l, row_count, left_row),
        a: (ta, column_count, above_column),
        uvmode = (uv_mode, uv_mode),
      }
    }
    None => {}
  }

  Ok(())
}

enum DrlStart {
  NewMotion,
  NearReference,
}

fn decode_drl_index(
  tile: &mut Rav1dTileStateContext,
  candidates: &[crate::src::refmvs::RefMvsCandidate; 8],
  available: u8,
  mode: DrlStart,
) -> Result<DrlProximity, ()> {
  let (mut selected, tested) = match mode {
    DrlStart::NewMotion => (
      DrlProximity::Nearest,
      [(0_u8, DrlProximity::Nearer), (1, DrlProximity::Near)],
    ),
    DrlStart::NearReference => (
      DrlProximity::Nearer,
      [(1_u8, DrlProximity::Near), (2, DrlProximity::Nearish)],
    ),
  };

  for (reference, next) in tested {
    match (available > reference.saturating_add(1)).then_some(()) {
      Some(()) => {}
      None => break,
    }

    let context = get_drl_context(candidates, reference).map_err(|_invalid_context| ())?;
    let Ok(position) = console_core_number_conversion::wrapped_index(context);
    let cdf = tile.cdf.mi.drl_bit.get_mut(position).ok_or(())?;
    let Ok(decision) = rav1d_msac_decode_bool_adapt(&mut tile.msac, cdf);

    match decision {
      EntropyBit::One => selected = next,
      EntropyBit::Zero => break,
    }
  }

  Ok(selected)
}

#[derive(Clone, Copy)]
struct BlockDecodeGeometry {
  dimensions: &'static [u8; 4],
  luma: Rectangle<i32>,
  visible_luma: Size<i32>,
  chroma: Rectangle<i32>,
  visible_chroma: Size<i32>,
  subsampling: Point<i32>,
  has_chroma: bool,
}

impl BlockDecodeGeometry {
  fn new(
    task: &Rav1dTaskContext,
    frame: &Rav1dFrameData,
    block: BlockSize,
  ) -> Result<Self, console_core_never::Never> {
    let Ok(dimensions) = block.dimensions();
    let &[columns, rows, ..] = dimensions;
    let source = task.b;
    let layout = frame.cur.p.layout;
    let horizontal = layout != Rav1dPixelLayout::I444;
    let vertical = layout == Rav1dPixelLayout::I420;
    let subsampling = Point {
      x: i32::from(horizontal),
      y: i32::from(vertical),
    };
    let origin = Point {
      x: source.x & 31,
      y: source.y & 31,
    };
    let width = i32::from(columns);
    let height = i32::from(rows);
    let size = Size { width, height };
    let visible = Size {
      width: width.min(frame.bw.wrapping_sub(source.x)),
      height: height.min(frame.bh.wrapping_sub(source.y)),
    };
    let x_shift = u32::from(horizontal);
    let y_shift = u32::from(vertical);
    let chroma_origin = Point {
      x: origin.x.wrapping_shr(x_shift),
      y: origin.y.wrapping_shr(y_shift),
    };
    let chroma = Rectangle {
      origin: chroma_origin,
      size: Size {
        width: width.wrapping_add(subsampling.x).wrapping_shr(x_shift),
        height: height.wrapping_add(subsampling.y).wrapping_shr(y_shift),
      },
    };
    let visible_chroma = Size {
      width: visible
        .width
        .wrapping_add(subsampling.x)
        .wrapping_shr(x_shift),
      height: visible
        .height
        .wrapping_add(subsampling.y)
        .wrapping_shr(y_shift),
    };
    let has_chroma = layout != Rav1dPixelLayout::I400
      && (width > subsampling.x || source.x & 1 != 0)
      && (height > subsampling.y || source.y & 1 != 0);

    Ok(Self {
      dimensions,
      luma: Rectangle { origin, size },
      visible_luma: visible,
      chroma,
      visible_chroma,
      subsampling,
      has_chroma,
    })
  }
}

#[derive(Clone, Copy)]
enum SegmentOrigin {
  Signalled,
  Inherited,
}

struct BlockSyntaxContext<'frame> {
  decoder: &'frame Rav1dContext,
  frame: &'frame Rav1dFrameData,
  frame_header: &'frame Rav1dFrameHeader,
  sequence: &'frame Rav1dSequenceHeader,
  tile: &'frame Rav1dTileState,
  above: &'frame BlockContext,
  geometry: BlockDecodeGeometry,
  block: BlockSize,
  edges: EdgeFlags,
  neighbors: NeighborAvailability,
  segment: Option<&'frame Rav1dSegmentationData>,
  segment_origin: SegmentOrigin,
}

struct IntraDirection {
  mode: u8,
  angle: i8,
}

struct IntraChroma {
  direction: IntraDirection,
  alpha: [i8; 2],
}

enum SegmentCoding {
  Lossless,
  Quantized,
}

fn intra_segment_coding(
  frame: &Rav1dFrameHeader,
  segment: SegmentId,
) -> Result<SegmentCoding, ()> {
  let Ok(number) = segment.get();
  let Ok(index) = console_core_number_conversion::index(number);
  let lossless = frame.segmentation.lossless.get(index).copied().ok_or(())?;

  Ok(match lossless.then_some(()) {
    Some(()) => SegmentCoding::Lossless,
    None => SegmentCoding::Quantized,
  })
}

fn read_intra_angle(
  entropy: &mut Rav1dTileStateContext,
  mode: u8,
  &[_, _, width_log, height_log]: &[u8; 4],
) -> Result<i8, ()> {
  let directional =
    width_log.wrapping_add(height_log) >= 2 && (VERT_PRED..=VERT_LEFT_PRED).contains(&mode);

  match directional.then_some(()) {
    Some(()) => {
      let Ok(index) = console_core_number_conversion::index(mode.wrapping_sub(VERT_PRED));
      let cdf = entropy.cdf.m.angle_delta.get_mut(index).ok_or(())?;
      let Ok(symbol) = rav1d_msac_decode_symbol_adapt8(&mut entropy.msac, cdf, 6);
      let Ok(angle) = console_core_number_conversion::wrapped::<_, i8>(symbol);
      Ok(angle.wrapping_sub(3))
    }
    None => Ok(0),
  }
}

fn read_keyframe_luma_context(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
) -> Result<[u8; 2], ()> {
  let Point { x, y } = syntax.geometry.luma.origin;
  let Ok(above_index) = console_core_number_conversion::wrapped_index(x);
  let Ok(left_index) = console_core_number_conversion::wrapped_index(y);
  let above = *syntax.above.mode.index(above_index);
  let left = task.l.mode.get_mut().get(left_index).copied().ok_or(())?;
  let Ok(above_mode) = console_core_number_conversion::index(above);
  let Ok(left_mode) = console_core_number_conversion::index(left);
  let above_context = dav1d_intra_mode_context
    .get(above_mode)
    .copied()
    .ok_or(())?;
  let left_context = dav1d_intra_mode_context.get(left_mode).copied().ok_or(())?;

  Ok([above_context, left_context])
}

fn read_luma_intra_direction(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
) -> Result<IntraDirection, ()> {
  let cdf = match syntax.frame_header.frame_type {
    Rav1dFrameType::Inter | Rav1dFrameType::Switch => {
      let Ok(block) = syntax.block.ordinal();
      let Ok(index) = console_core_number_conversion::index(block);
      let context = dav1d_ymode_size_context.get(index).copied().ok_or(())?;
      let Ok(context_index) = console_core_number_conversion::index(context);
      entropy.cdf.mi.y_mode.get_mut(context_index).ok_or(())?
    }
    Rav1dFrameType::Key | Rav1dFrameType::Intra => {
      let [above, left] = read_keyframe_luma_context(syntax, task)?;
      let Ok(above_index) = console_core_number_conversion::index(above);
      let Ok(left_index) = console_core_number_conversion::index(left);
      let row = entropy.cdf.kfym.get_mut(above_index).ok_or(())?;
      row.get_mut(left_index).ok_or(())?
    }
  };
  let Ok(mode_count) = console_core_number_conversion::wrapped::<_, u8>(N_INTRA_PRED_MODES);
  let Ok(mode) =
    rav1d_msac_decode_symbol_adapt16(&mut entropy.msac, cdf, mode_count.wrapping_sub(1));
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match trace {
    BlockTrace::Enabled => println!("Post-ymode[{mode}]: r={}", entropy.msac.rng),
    BlockTrace::Disabled => {}
  }

  let angle = read_intra_angle(entropy, mode, syntax.geometry.dimensions)?;

  Ok(IntraDirection { mode, angle })
}

fn read_intra_chroma_alpha(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
) -> Result<[i8; 2], ()> {
  let Ok(symbol) =
    rav1d_msac_decode_symbol_adapt8(&mut entropy.msac, &mut *entropy.cdf.m.cfl_sign, 7);
  let sign = symbol.wrapping_add(1);
  let sign_u = sign.wrapping_div(3);
  let sign_v = sign.wrapping_sub(sign_u.wrapping_mul(3));
  let alpha_u = read_chroma_scale(
    entropy,
    ChromaScaleSigns {
      current: sign_u,
      opposite: sign_v,
    },
  )?;
  let alpha_v = read_chroma_scale(
    entropy,
    ChromaScaleSigns {
      current: sign_v,
      opposite: sign_u,
    },
  )?;
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match trace {
    BlockTrace::Enabled => {
      console_core_logging::trace!("Post-uvalphas[{alpha_u}/{alpha_v}]: r={}", entropy.msac.rng);
    }
    BlockTrace::Disabled => {}
  }

  Ok([alpha_u, alpha_v])
}

fn read_chroma_intra_direction(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
  segment: SegmentId,
  luma_mode: u8,
) -> Result<IntraChroma, ()> {
  match syntax.geometry.has_chroma.then_some(()) {
    Some(()) => {}
    None => {
      return Ok(IntraChroma {
        direction: IntraDirection { mode: 0, angle: 0 },
        alpha: [0; 2],
      });
    }
  }

  let coding = intra_segment_coding(syntax.frame_header, segment)?;
  let cfl_allowed = match coding {
    SegmentCoding::Lossless => {
      syntax.geometry.chroma.size
        == (Size {
          width: 1,
          height: 1,
        })
    }
    SegmentCoding::Quantized => {
      let Ok(ordinal) = syntax.block.ordinal();
      cfl_allowed_mask & 1u32.wrapping_shl(u32::from(ordinal)) != 0
    }
  };
  let Ok(cfl_index) = console_core_number_conversion::index(u8::from(cfl_allowed));
  let Ok(luma_index) = console_core_number_conversion::index(luma_mode);
  let row = entropy.cdf.m.uv_mode.get_mut(cfl_index).ok_or(())?;
  let cdf = row.get_mut(luma_index).ok_or(())?;
  let Ok(count) = console_core_number_conversion::wrapped::<_, u8>(N_UV_INTRA_PRED_MODES);
  let last = count.wrapping_sub(1).wrapping_sub(u8::from(!cfl_allowed));
  let Ok(mode) = rav1d_msac_decode_symbol_adapt16(&mut entropy.msac, cdf, last);
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match trace {
    BlockTrace::Enabled => println!("Post-uvmode[{mode}]: r={}", entropy.msac.rng),
    BlockTrace::Disabled => {}
  }

  let (angle, alpha) = match (mode == CFL_PRED).then_some(()) {
    Some(()) => {
      let factors = read_intra_chroma_alpha(syntax, task, entropy)?;
      (0, factors)
    }
    None => {
      let direction = read_intra_angle(entropy, mode, syntax.geometry.dimensions)?;
      (direction, [0; 2])
    }
  };

  Ok(IntraChroma {
    direction: IntraDirection { mode, angle },
    alpha,
  })
}

fn read_luma_palette_size(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
  block: PaletteRead,
) -> Result<u8, ()> {
  let above = checked_index(block.position.x).map_err(|_invalid_neighbor| ())?;
  let left = checked_index(block.position.y).map_err(|_invalid_neighbor| ())?;
  let above_size = *syntax.above.pal_sz.index(above);
  let &left_size = task.l.pal_sz.get_mut().get(left).ok_or(())?;
  let neighbors = u8::from(above_size > 0).wrapping_add(u8::from(left_size > 0));
  let Ok(block_size) = console_core_number_conversion::index(block.size_context);
  let Ok(context) = console_core_number_conversion::index(neighbors);
  let row = entropy.cdf.m.pal_y.get_mut(block_size).ok_or(())?;
  let probability = row.get_mut(context).ok_or(())?;
  let Ok(selected) = rav1d_msac_decode_bool_adapt(&mut entropy.msac, probability);
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match trace {
    BlockTrace::Enabled => println!(
      "Post-y_pal[{}]: r={}",
      selected == EntropyBit::One,
      entropy.msac.rng
    ),
    BlockTrace::Disabled => {}
  }

  match selected {
    EntropyBit::One => {
      let Ok(dsp) = syntax.frame.bd_fn();
      (dsp.read_pal_y)(task, syntax.frame, entropy, block).map_err(|_invalid_palette| ())
    }
    EntropyBit::Zero => Ok(0),
  }
}

fn read_chroma_palette_size(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
  block: PaletteRead,
  luma_size: u8,
) -> Result<u8, ()> {
  let Ok(context) = console_core_number_conversion::index(u8::from(luma_size > 0));
  let probability = entropy.cdf.m.pal_uv.get_mut(context).ok_or(())?;
  let Ok(selected) = rav1d_msac_decode_bool_adapt(&mut entropy.msac, probability);
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match trace {
    BlockTrace::Enabled => println!(
      "Post-uv_pal[{}]: r={}",
      selected == EntropyBit::One,
      entropy.msac.rng
    ),
    BlockTrace::Disabled => {}
  }

  match selected {
    EntropyBit::One => {
      let Ok(dsp) = syntax.frame.bd_fn();
      (dsp.read_pal_uv)(task, syntax.frame, entropy, block).map_err(|_invalid_palette| ())
    }
    EntropyBit::Zero => Ok(0),
  }
}

fn read_intra_palette_sizes(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
  [luma_mode, chroma_mode]: [u8; 2],
) -> Result<[u8; 2], ()> {
  let luma = syntax.geometry.luma;
  let eligible = syntax.frame_header.allow_screen_content_tools
    && luma.size.width.max(luma.size.height) <= 16
    && luma.size.width.wrapping_add(luma.size.height) >= 4;

  match eligible.then_some(()) {
    Some(()) => {}
    None => return Ok([0; 2]),
  }

  let &[_, _, width_log, height_log] = syntax.geometry.dimensions;
  let size_context = width_log.wrapping_add(height_log).wrapping_sub(2);
  let y = match (luma_mode == DC_PRED).then_some(()) {
    Some(()) => read_luma_palette_size(
      syntax,
      task,
      entropy,
      PaletteRead {
        size_context,
        position: luma.origin,
      },
    )?,
    None => 0,
  };
  let uv = match (syntax.geometry.has_chroma && chroma_mode == DC_PRED).then_some(()) {
    Some(()) => read_chroma_palette_size(
      syntax,
      task,
      entropy,
      PaletteRead {
        size_context,
        position: luma.origin,
      },
      y,
    )?,
    None => 0,
  };

  Ok([y, uv])
}

fn select_filtered_intra_direction(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
  original: IntraDirection,
  palette: u8,
) -> Result<IntraDirection, ()> {
  let &[_, _, width_log, height_log] = syntax.geometry.dimensions;
  let enabled = original.mode == DC_PRED
    && palette == 0
    && width_log.max(height_log) <= 3
    && syntax.sequence.filter_intra != 0;

  match enabled.then_some(()) {
    Some(()) => {}
    None => return Ok(original),
  }

  let Ok(block) = syntax.block.ordinal();
  let Ok(index) = console_core_number_conversion::index(block);
  let probability = entropy.cdf.m.use_filter_intra.get_mut(index).ok_or(())?;
  let Ok(selected) = rav1d_msac_decode_bool_adapt(&mut entropy.msac, probability);
  let direction = match selected {
    EntropyBit::One => {
      let Ok(filter) =
        rav1d_msac_decode_symbol_adapt8(&mut entropy.msac, &mut *entropy.cdf.m.filter_intra, 4);
      let Ok(angle) = console_core_number_conversion::wrapped::<_, i8>(filter);
      IntraDirection {
        mode: FILTER_PRED,
        angle,
      }
    }
    EntropyBit::Zero => original,
  };
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match trace {
    BlockTrace::Enabled => println!(
      "Post-filterintramode[{}/{}]: r={}",
      direction.mode, direction.angle, entropy.msac.rng
    ),
    BlockTrace::Disabled => {}
  }

  Ok(direction)
}

struct IntraPalettePlane {
  cells: PaletteCellBlock,
  entries: u8,
  plane: PalettePlane,
}

fn read_intra_palette_plane(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
  IntraPalettePlane {
    cells,
    entries,
    plane,
  }: IntraPalettePlane,
) -> Result<(), ()> {
  let scratch = task
    .scratch
    .inter_intra_mut()
    .map_err(|_invalid_scratch| ())?;
  let mut frame_output = match (task.frame_thread.pass != 0).then_some(()) {
    Some(()) => {
      let Ok(pass) = console_core_number_conversion::wrapped_index(task.frame_thread.pass & 1);
      let buffer = syntax.tile.frame_thread.get(pass).ok_or(())?;
      let samples = cells
        .block
        .width
        .wrapping_mul(cells.block.height)
        .wrapping_mul(8);
      let Ok(length) = console_core_number_conversion::wrapped::<_, u32>(samples);
      let Ok(start) = buffer
        .pal_idx
        .get_update(|offset| offset.wrapping_add(length));
      let Ok(first) = console_core_number_conversion::index(start);
      let Ok(count) = console_core_number_conversion::index(length);
      Some(
        syntax
          .frame
          .frame_thread
          .pal_idx
          .index_mut((first.., ..count)),
      )
    }
    None => None,
  };
  let output = match (frame_output.as_deref_mut(), plane) {
    (Some(buffer), _) => PaletteOutput::Separate(buffer),
    (None, PalettePlane::Luma) => PaletteOutput::Separate(&mut scratch.pal_idx_y),
    (None, PalettePlane::Chroma) => PaletteOutput::InPlace,
  };
  read_pal_indices(
    entropy,
    &syntax.decoder.dsp.pal,
    PaletteIndexInput {
      staging: &mut scratch.pal_idx_uv,
      packed: output,
      cells,
      entries,
      plane,
    },
  )
  .map_err(|_invalid_palette| ())?;
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match (trace, plane) {
    (BlockTrace::Enabled, PalettePlane::Luma) => {
      println!("Post-y-pal-indices: r={}", entropy.msac.rng)
    }
    (BlockTrace::Enabled, PalettePlane::Chroma) => {
      println!("Post-uv-pal-indices: r={}", entropy.msac.rng)
    }
    (BlockTrace::Disabled, _) => {}
  }

  Ok(())
}

fn read_intra_palette_indices(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
  [y, uv]: [u8; 2],
) -> Result<(), ()> {
  for (plane, block, visible, entries) in [
    (
      PalettePlane::Luma,
      syntax.geometry.luma.size,
      syntax.geometry.visible_luma,
      y,
    ),
    (
      PalettePlane::Chroma,
      syntax.geometry.chroma.size,
      syntax.geometry.visible_chroma,
      uv,
    ),
  ] {
    match entries {
      0 => {}
      _ => read_intra_palette_plane(
        syntax,
        task,
        entropy,
        IntraPalettePlane {
          cells: PaletteCellBlock { block, visible },
          entries,
          plane,
        },
      )?,
    }
  }

  Ok(())
}

fn read_intra_transform_depth(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
  info: &TxfmInfo,
) -> Result<u8, ()> {
  let Point { x, y } = syntax.geometry.luma.origin;
  let context = get_tx_ctx(
    BlockNeighbors {
      above: syntax.above,
      left: &task.l,
      location: Bxy { x, y },
      availability: syntax.neighbors,
    },
    info,
  )
  .map_err(|_invalid_context| ())?;
  let Ok(level_index) = console_core_number_conversion::index(info.max.wrapping_sub(1));
  let Ok(context_index) = console_core_number_conversion::wrapped_index(context);
  let row = entropy.cdf.m.txsz.get_mut(level_index).ok_or(())?;
  let cdf = row.get_mut(context_index).ok_or(())?;
  let Ok(depth) = rav1d_msac_decode_symbol_adapt4(&mut entropy.msac, cdf, info.max.min(2));

  Ok(depth)
}

fn read_intra_transform(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  block: &mut Av1Block,
  entropy: &mut Rav1dTileStateContext,
) -> Result<TxfmSize, ()> {
  let coding = intra_segment_coding(syntax.frame_header, block.seg_id)?;

  match coding {
    SegmentCoding::Lossless => {
      block.uvtx = TxfmSize::S4x4;
      return Ok(TxfmSize::S4x4);
    }
    SegmentCoding::Quantized => {}
  }

  let Ok(ordinal) = syntax.block.ordinal();
  let Ok(index) = console_core_number_conversion::index(ordinal);
  let allowed = dav1d_max_txfm_size_for_bs.get(index).ok_or(())?;
  let mut transform = allowed.first().copied().ok_or(())?;
  let Ok(chroma_index) =
    console_core_number_conversion::index(syntax.frame.cur.p.layout.into_rav1d());
  let chroma = allowed.get(chroma_index).copied().ok_or(())?;
  block.uvtx = chroma;
  let mut info = transform_info(transform).map_err(|_invalid_transform| ())?;
  let Ok(minimum) = TxfmSize::S4x4.ordinal();

  match (syntax.frame_header.txfm_mode == Rav1dTxfmMode::Switchable && info.max > minimum)
    .then_some(())
  {
    Some(()) => {
      let depth = read_intra_transform_depth(syntax, task, entropy, info)?;

      for _ in 0..depth {
        transform = info.sub;
        let next = transform_info(transform).map_err(|_invalid_transform| ())?;
        info = next;
      }
    }
    None => {}
  }

  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match trace {
    BlockTrace::Enabled => println!("Post-tx[{transform:?}]: r={}", entropy.msac.rng),
    BlockTrace::Disabled => {}
  }

  Ok(transform)
}

fn apply_intra_loopfilter(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  block: &Av1Block,
  transform: TxfmSize,
) -> Result<(), ()> {
  match syntax.frame_header.loopfilter.level_y {
    [0, 0] => return Ok(()),
    _ => {}
  }

  let frame = syntax.frame;
  let tile = syntax.tile;
  let Ok(level_source) = tile.lflvl.get();
  let local;
  let levels = match level_source {
    TileStateRef::Frame => &frame.lf.lvl,
    TileStateRef::Local => {
      let retained = tile.lflvlmem.try_read().ok_or(())?;
      local = retained;
      &*local
    }
  };
  let mask = frame
    .loop_filter_mask(task.lf_mask)
    .map_err(|_invalid_mask| ())?;
  let Ok(segment) = block.seg_id.get();
  let Ok(segment_slot) = console_core_number_conversion::index(segment);
  let segment_levels = levels.get(segment_slot).ok_or(())?;
  let luma = syntax.geometry.luma;
  let chroma = syntax.geometry.chroma;
  let Ok(above_x) = console_core_number_conversion::wrapped_index(luma.origin.x);
  let Ok(above_width) = console_core_number_conversion::wrapped_index(luma.size.width);
  let Ok(left_y) = console_core_number_conversion::wrapped_index(luma.origin.y);
  let Ok(left_height) = console_core_number_conversion::wrapped_index(luma.size.height);
  let mut above_luma = syntax.above.tx_lpf_y.index_mut((above_x.., ..above_width));
  let left_luma = task
    .l
    .tx_lpf_y
    .get_mut()
    .get_mut(left_y..)
    .and_then(|rows| rows.get_mut(..left_height))
    .ok_or(())?;
  let mut above_chroma;
  let chroma_edges = match syntax.geometry.has_chroma.then_some(()) {
    Some(()) => {
      let Ok(chroma_x) = console_core_number_conversion::wrapped_index(chroma.origin.x);
      let Ok(chroma_width) = console_core_number_conversion::wrapped_index(chroma.size.width);
      let Ok(chroma_y) = console_core_number_conversion::wrapped_index(chroma.origin.y);
      let Ok(chroma_height) = console_core_number_conversion::wrapped_index(chroma.size.height);
      above_chroma = syntax
        .above
        .tx_lpf_uv
        .index_mut((chroma_x.., ..chroma_width));
      let left_chroma = task
        .l
        .tx_lpf_uv
        .get_mut()
        .get_mut(chroma_y..)
        .and_then(|rows| rows.get_mut(..chroma_height))
        .ok_or(())?;
      Some((&mut *above_chroma, left_chroma))
    }
    None => None,
  };
  let filtered = create_lf_mask(
    FilterMaskBlock {
      mask,
      level_cache: &frame.lf.level,
      stride: i64::from(frame.b4_stride),
      levels: segment_levels,
      origin: task.b,
      frame: Size {
        width: frame.w4,
        height: frame.h4,
      },
      size: syntax.block,
      ytx: transform,
      uvtx: block.uvtx,
      layout: frame.cur.p.layout,
      luma: EdgeNeighbors {
        above: &mut above_luma,
        left: left_luma,
      },
      chroma: chroma_edges,
    },
    FilterMaskPrediction::Intra,
  );

  match filtered {
    Ok(()) => Ok(()),
    Err(fault) => {
      console_core_logging::debug!("Could not create an AV1 intra filter mask: {fault:?}");
      Err(())
    }
  }
}

fn write_intra_luma_context(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  block: &Av1Block,
  intra: &Av1BlockIntra,
  transform: &TxfmInfo,
) -> Result<(), ()> {
  let above = syntax.above;
  let luma = syntax.geometry.luma;
  let Ok(left_start) = console_core_number_conversion::wrapped_index(luma.origin.y);
  let Ok(left_count) = console_core_number_conversion::wrapped_index(luma.size.height);
  let Ok(above_start) = console_core_number_conversion::wrapped_index(luma.origin.x);
  let Ok(above_count) = console_core_number_conversion::wrapped_index(luma.size.width);
  let Ok(height_index) = console_core_number_conversion::index(transform.lh);
  let Ok(width_index) = console_core_number_conversion::index(transform.lw);
  let tx_height = TxfmSize::from_repr(height_index).ok_or(())?;
  let tx_width = TxfmSize::from_repr(width_index).ok_or(())?;
  let Ok(neighbor_height) = console_core_number_conversion::wrapped::<_, i8>(transform.lh);
  let Ok(neighbor_width) = console_core_number_conversion::wrapped::<_, i8>(transform.lw);
  let y_mode = match (intra.y_mode == FILTER_PRED).then_some(()) {
    Some(()) => DC_PRED,
    None => intra.y_mode,
  };
  let [y_palette, chroma_palette] = intra.pal_sz;
  let uv_palette = match syntax.geometry.has_chroma.then_some(chroma_palette) {
    Some(entries) => entries,
    None => 0,
  };
  let predicted_segment = u8::from(matches!(syntax.segment_origin, SegmentOrigin::Inherited));

  case_set_al! {
    <32, false>
    l: (&mut task.l, left_count, left_start)
      also |case| {
        let [_, chroma] = &mut task.pal_sz_uv;
        let Ok(()) = case.set(chroma, uv_palette);
      },
    a: (above, above_count, above_start)
      also |case| {
        let [chroma, _] = &mut task.pal_sz_uv;
        let Ok(()) = case.set(chroma, uv_palette);
      },
    tx_intra = (neighbor_height, neighbor_width),
    tx = (tx_height, tx_width),
    mode = (y_mode, y_mode),
    pal_sz = (y_palette, y_palette),
    seg_pred = (predicted_segment, predicted_segment),
    skip_mode = (0, 0),
    intra = (1, 1),
    skip = (block.skip, block.skip),
  }

  match syntax.frame_header.frame_type {
    Rav1dFrameType::Key | Rav1dFrameType::Intra => {}
    Rav1dFrameType::Inter | Rav1dFrameType::Switch => {
      let filter = Rav1dFilterMode::N_SWITCHABLE_FILTERS;
      case_set_al! {
        <32, false>
        l: (&mut task.l, left_count, left_start),
        a: (above, above_count, above_start),
        comp_type = (None, None),
        r#ref[0] = (-1, -1),
        r#ref[1] = (-1, -1),
        filter[0] = (filter, filter),
        filter[1] = (filter, filter),
      }
    }
  }

  Ok(())
}

fn copy_intra_palette(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  entries: u8,
  plane: PalettePlane,
) -> Result<(), ()> {
  match entries {
    0 => Ok(()),
    _ => {
      let Ok(dsp) = syntax.frame.bd_fn();
      (dsp.copy_pal_block)(
        task,
        syntax.frame,
        PaletteCopy {
          region: syntax.geometry.luma,
          plane,
        },
      )
      .map_err(|_invalid_palette| ())
    }
  }
}

fn update_intra_contexts(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  block: &Av1Block,
  intra: &Av1BlockIntra,
  transform: &TxfmInfo,
) -> Result<(), ()> {
  write_intra_luma_context(syntax, task, block, intra, transform)?;
  let [y_palette, uv_palette] = intra.pal_sz;
  copy_intra_palette(syntax, task, y_palette, PalettePlane::Luma)?;

  match syntax.geometry.has_chroma.then_some(()) {
    Some(()) => {
      let chroma = syntax.geometry.chroma;
      let Ok(left_start) = console_core_number_conversion::wrapped_index(chroma.origin.y);
      let Ok(left_count) = console_core_number_conversion::wrapped_index(chroma.size.height);
      let Ok(above_start) = console_core_number_conversion::wrapped_index(chroma.origin.x);
      let Ok(above_count) = console_core_number_conversion::wrapped_index(chroma.size.width);
      case_set_al! {
        <32, false>
        l: (&mut task.l, left_count, left_start),
        a: (syntax.above, above_count, above_start),
        uvmode = (intra.uv_mode, intra.uv_mode),
      }
      copy_intra_palette(syntax, task, uv_palette, PalettePlane::Chroma)?;
    }
    None => {}
  }

  match (syntax.frame_header.frame_type.is_inter_or_switch() || syntax.frame_header.allow_intrabc)
    .then_some(())
  {
    Some(()) => splat_intraref(
      syntax.decoder,
      task,
      &syntax.frame.rf,
      syntax.block,
      syntax.geometry.luma.size,
    )
    .map_err(|_invalid_motion| ()),
    None => Ok(()),
  }
}

fn decode_intra_syntax(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  block: &mut Av1Block,
  entropy: &mut Rav1dTileStateContext,
) -> Result<(), ()> {
  let luma = read_luma_intra_direction(syntax, task, entropy)?;
  let chroma = read_chroma_intra_direction(syntax, task, entropy, block.seg_id, luma.mode)?;
  let palettes =
    read_intra_palette_sizes(syntax, task, entropy, [luma.mode, chroma.direction.mode])?;
  let [luma_palette, _] = palettes;
  let direction = select_filtered_intra_direction(syntax, task, entropy, luma, luma_palette)?;
  read_intra_palette_indices(syntax, task, entropy, palettes)?;
  let transform = read_intra_transform(syntax, task, block, entropy)?;
  let info = transform_info(transform).map_err(|_invalid_transform| ())?;
  let intra = Av1BlockIntra {
    y_mode: direction.mode,
    uv_mode: chroma.direction.mode,
    tx: transform,
    pal_sz: palettes,
    y_angle: direction.angle,
    uv_angle: chroma.direction.angle,
    cfl_alpha: chroma.alpha,
  };
  block.ii = Av1BlockIntraInter::Intra(intra);
  let Ok(dsp) = syntax.frame.bd_fn();

  match task.frame_thread.pass {
    1 => (dsp.read_coef_blocks)(syntax.frame, task, entropy, syntax.block, block)
      .map_err(|_invalid_coefficients| ())?,
    _ => (dsp.recon_b_intra)(
      syntax.frame,
      task,
      Some(entropy),
      syntax.block,
      syntax.edges,
      block,
      &intra,
    )
    .map_err(|_invalid_reconstruction| ())?,
  }

  apply_intra_loopfilter(syntax, task, block, transform)?;
  update_intra_contexts(syntax, task, block, &intra, info)
}

struct IntrabcBorders {
  top_left: Point<i32>,
  right: i32,
}

fn intrabc_tile_borders(
  syntax: &BlockSyntaxContext<'_>,
) -> Result<IntrabcBorders, console_core_never::Never> {
  let tile = &syntax.tile.tiling;
  let geometry = syntax.geometry;
  let Size { width, height } = geometry.luma.size;
  let extra_left = geometry.has_chroma && width < 2 && geometry.subsampling.x != 0;
  let extra_top = geometry.has_chroma && height < 2 && geometry.subsampling.y != 0;
  let pad = width.wrapping_sub(1);
  let right = (tile.col_end.wrapping_add(pad) & !pad).wrapping_mul(4);
  let top_left = Point {
    x: tile
      .col_start
      .wrapping_mul(4)
      .wrapping_add(i32::from(extra_left).wrapping_mul(4)),
    y: tile
      .row_start
      .wrapping_mul(4)
      .wrapping_add(i32::from(extra_top).wrapping_mul(4)),
  };

  Ok(IntrabcBorders { top_left, right })
}

fn fit_intrabc_source(
  mut source: Rectangle<i32>,
  tile: IntrabcBorders,
  superblock: Rectangle<i32>,
) -> Result<Rectangle<i32>, ()> {
  let source_right = source.origin.x.wrapping_add(source.size.width);
  source.origin.x = match (
    (source.origin.x < tile.top_left.x).then_some(()),
    (source_right > tile.right).then_some(()),
  ) {
    (Some(()), _) => tile.top_left.x,
    (None, Some(())) => source
      .origin
      .x
      .wrapping_sub(source_right.wrapping_sub(tile.right)),
    (None, None) => source.origin.x,
  };
  source.origin.y = source.origin.y.max(tile.top_left.y);
  let right = source.origin.x.wrapping_add(source.size.width);
  let bottom = source.origin.y.wrapping_add(source.size.height);

  match (bottom > superblock.origin.y && right > superblock.origin.x).then_some(()) {
    Some(()) => {
      let vertical_overlap = bottom.wrapping_sub(superblock.origin.y);
      let horizontal_overlap = right.wrapping_sub(superblock.origin.x);
      let room_above = source.origin.y.wrapping_sub(tile.top_left.y);
      let room_left = source.origin.x.wrapping_sub(tile.top_left.x);

      match (
        (room_above >= vertical_overlap).then_some(()),
        (room_left >= horizontal_overlap).then_some(()),
      ) {
        (Some(()), _) => source.origin.y = source.origin.y.wrapping_sub(vertical_overlap),
        (None, Some(())) => source.origin.x = source.origin.x.wrapping_sub(horizontal_overlap),
        (None, None) => {}
      }
    }
    None => {}
  }

  let superblock_bottom = superblock.origin.y.wrapping_add(superblock.size.height);
  let bottom_after_shift = source.origin.y.wrapping_add(source.size.height);

  match (bottom_after_shift > superblock_bottom).then_some(()) {
    Some(()) => {
      source.origin.y = source
        .origin
        .y
        .wrapping_sub(bottom_after_shift.wrapping_sub(superblock_bottom))
    }
    None => {}
  }

  let crosses_current = source.origin.y.wrapping_add(source.size.height) > superblock.origin.y
    && source.origin.x.wrapping_add(source.size.width) > superblock.origin.x;

  match crosses_current.then_some(()) {
    Some(()) => Err(()),
    None => Ok(source),
  }
}

fn correct_intrabc_motion(
  syntax: &BlockSyntaxContext<'_>,
  position: Bxy,
  motion: Mv,
) -> Result<Mv, ()> {
  let destination = Point {
    x: position.x.wrapping_mul(4),
    y: position.y.wrapping_mul(4),
  };
  let luma = syntax.geometry.luma.size;
  let source = Rectangle {
    origin: Point {
      x: destination
        .x
        .wrapping_add(i32::from(motion.x).wrapping_shr(3)),
      y: destination
        .y
        .wrapping_add(i32::from(motion.y).wrapping_shr(3)),
    },
    size: Size {
      width: luma.width.wrapping_mul(4),
      height: luma.height.wrapping_mul(4),
    },
  };
  let sb128 = u32::from(syntax.sequence.sb128);
  let address_shift = 4_u32.wrapping_add(sb128);
  let pixel_shift = 6_u32.wrapping_add(sb128);
  let span = 1_i32.wrapping_shl(pixel_shift);
  let position_in_pixels = |coordinate: i32| {
    coordinate
      .wrapping_shr(address_shift)
      .wrapping_shl(pixel_shift)
  };
  let superblock = Rectangle {
    origin: Point {
      x: position_in_pixels(position.x),
      y: position_in_pixels(position.y),
    },
    size: Size {
      width: span,
      height: span,
    },
  };
  let Ok(tile) = intrabc_tile_borders(syntax);
  let corrected = fit_intrabc_source(source, tile, superblock)?;
  let Ok(x) = console_core_number_conversion::wrapped::<_, i16>(
    corrected
      .origin
      .x
      .wrapping_sub(destination.x)
      .wrapping_mul(8),
  );
  let Ok(y) = console_core_number_conversion::wrapped::<_, i16>(
    corrected
      .origin
      .y
      .wrapping_sub(destination.y)
      .wrapping_mul(8),
  );

  Ok(Mv { x, y })
}

fn read_intrabc_motion(
  syntax: &BlockSyntaxContext<'_>,
  t: &Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
) -> Result<Mv, ()> {
  let mut candidates = [Default::default(); 8];
  let mut candidate_context = 0;
  let _count = rav1d_refmvs_find(
    &t.rt,
    &syntax.frame.rf,
    &mut candidates,
    &mut candidate_context,
    crate::src::refmvs::RefMvsSearch {
      references: crate::src::refmvs::RefMvsRefPair::from([0, -1]),
      block: syntax.block,
      edges: syntax.edges,
      origin: Point { x: t.b.x, y: t.b.y },
    },
    syntax.frame_header,
  )
  .map_err(|_invalid_motion_neighbors| ())?;
  let [first, second, ..] = candidates;
  let [nearest, _] = first.mv.mv;
  let [next, _] = second.mv.mv;
  let mut predicted = match (
    (nearest != Mv::ZERO).then_some(()),
    (next != Mv::ZERO).then_some(()),
  ) {
    (Some(()), _) => nearest,
    (None, Some(())) => next,
    (None, None) => {
      let sb128 = u32::from(syntax.sequence.sb128);
      let superblock_before = t.b.y.wrapping_sub(16_i32.wrapping_shl(sb128));
      let upwards = 512_i32.wrapping_shl(sb128).wrapping_neg();

      match (superblock_before < syntax.tile.tiling.row_start).then_some(()) {
        Some(()) => {
          let Ok(x) =
            console_core_number_conversion::wrapped::<_, i16>(upwards.wrapping_sub(2048));
          Mv { x, y: 0 }
        }
        None => {
          let Ok(y) = console_core_number_conversion::wrapped::<_, i16>(upwards);
          Mv { x: 0, y }
        }
      }
    }
  };

  read_mv_residual(entropy, &mut predicted, ResidualPrecision::Integer)
    .map_err(|_invalid_motion_residual| ())?;
  let corrected = correct_intrabc_motion(syntax, t.b, predicted)?;
  let Ok(trace) = debug_block_info(syntax.frame, t.b);

  match trace {
    BlockTrace::Enabled => {
      console_core_logging::trace!(
        "Post-dmv[{}/{},ref={}/{}|{}/{}]: r={}",
        corrected.y,
        corrected.x,
        predicted.y,
        predicted.x,
        nearest.y,
        nearest.x,
        entropy.msac.rng,
      );
    }
    BlockTrace::Disabled => {}
  }

  Ok(corrected)
}

fn decode_intrabc_syntax(
  syntax: &BlockSyntaxContext<'_>,
  t: &mut Rav1dTaskContext,
  b: &mut Av1Block,
  entropy: &mut Rav1dTileStateContext,
) -> Result<(), ()> {
  let motion = read_intrabc_motion(syntax, t, entropy)?;
  reconstruct_intrabc(syntax, t, b, entropy, motion)?;
  let Ok(()) = update_intrabc_context(syntax, t, b);
  Ok(())
}

fn reconstruct_intrabc(
  syntax: &BlockSyntaxContext<'_>,
  t: &mut Rav1dTaskContext,
  b: &mut Av1Block,
  entropy: &mut Rav1dTileStateContext,
  motion: Mv,
) -> Result<(), ()> {
  let frame = syntax.frame;
  let block = syntax.block;
  let Ok(bd_fn) = frame.bd_fn();
  let Point { x, y } = syntax.geometry.luma.origin;
  let transforms = read_vartx_tree(
    t,
    frame,
    entropy,
    b,
    VariableTransformBlock {
      size: block,
      local: Bxy { x, y },
    },
  )
  .map_err(|_invalid_transform_tree| ())?;
  let VarTx {
    uvtx,
    max_ytx,
    tx_split0,
    tx_split1,
  } = transforms;
  let filter2d = match (t.frame_thread.pass == 1).then_some(()) {
    Some(()) => Filter2d::Bilinear,
    None => Filter2d::default(),
  };
  b.uvtx = uvtx;
  let inter = Av1BlockInter {
    nd: Av1BlockInterNd::from(Av1BlockInter1d {
      mv: [motion, Mv::default()],
      ..Av1BlockInter1d::default()
    }),
    comp_type: Default::default(),
    inter_mode: Default::default(),
    motion_mode: Default::default(),
    drl_idx: Default::default(),
    r#ref: Default::default(),
    max_ytx,
    filter2d,
    interintra_type: Default::default(),
    tx_split0,
    tx_split1,
  };
  b.ii = Av1BlockIntraInter::Inter(inter);

  match (t.frame_thread.pass == 1).then_some(()) {
    Some(()) => {
      (bd_fn.read_coef_blocks)(frame, t, entropy, block, b).map_err(|_invalid_coefficients| ())?
    }
    None => (bd_fn.recon_b_inter)(frame, t, Some(entropy), block, b, &inter)?,
  }

  splat_intrabc_mv(
    syntax.decoder,
    t,
    &frame.rf,
    block,
    motion,
    syntax.geometry.luma.size,
  )
  .map_err(|_invalid_reference_map| ())
}

fn update_intrabc_context(
  syntax: &BlockSyntaxContext<'_>,
  t: &mut Rav1dTaskContext,
  b: &Av1Block,
) -> Result<(), console_core_never::Never> {
  let ta = syntax.above;
  let b_dim = syntax.geometry.dimensions;
  let has_chroma = syntax.geometry.has_chroma;
  let seg_pred = matches!(syntax.segment_origin, SegmentOrigin::Inherited);
  let Point { x: bx4, y: by4 } = syntax.geometry.luma.origin;
  let Size {
    width: bw4,
    height: bh4,
  } = syntax.geometry.luma.size;
  let Point { x: cbx4, y: cby4 } = syntax.geometry.chroma.origin;
  let Size {
    width: cbw4,
    height: cbh4,
  } = syntax.geometry.chroma.size;

  case_set_al! {
      <32, false>

      l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4); numeric_value })
          also |case| { ({ let Ok(explicit_value) = case.set({ let [_, _explicit_part, ..] = &mut t.pal_sz_uv; _explicit_part }, 0); explicit_value }); },
      a: (ta, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx4); numeric_value })
          also |case| { ({ let Ok(explicit_value) = case.set({ let [_explicit_part, ..] = &mut t.pal_sz_uv; _explicit_part }, 0); explicit_value }); },
      tx_intra = ({
        let &[_, _, _, height] = b_dim;
        let Ok(height_log) = console_core_number_conversion::wrapped::<_, i8>(height);
        height_log
      }, {
        let &[_, _, width, _] = b_dim;
        let Ok(width_log) = console_core_number_conversion::wrapped::<_, i8>(width);
        width_log
      }),
      mode = (DC_PRED, DC_PRED),
      pal_sz = (0, 0),
      seg_pred = (<::core::primitive::u8 as ::core::convert::From<::core::primitive::bool>>::from(seg_pred), <::core::primitive::u8 as ::core::convert::From<::core::primitive::bool>>::from(seg_pred)),
      skip_mode = (0, 0),
      intra = (0, 0),
      skip = (b.skip, b.skip),
  }

  match ::core::primitive::bool::then_some(has_chroma, ()) {
    ::core::option::Option::Some(()) => {
      case_set_al! {
          <32, false>
          l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbh4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cby4); numeric_value }),
          a: (ta, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbw4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbx4); numeric_value }),
          uvmode = (DC_PRED, DC_PRED),
      }
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

struct InterPredictionSyntax {
  nd: Av1BlockInterNd,
  comp_type: Option<CompInterType>,
  inter_mode: u8,
  motion_mode: MotionMode,
  drl_idx: DrlProximity,
  r#ref: [i8; 2],
  interintra_type: Option<InterIntraType>,
  needs_filter: SwitchableFilter,
}

enum SwitchableFilter {
  Read,
  Regular,
}

impl From<bool> for SwitchableFilter {
  fn from(read: bool) -> Self {
    match read {
      true => Self::Read,
      false => Self::Regular,
    }
  }
}

fn read_skip_motion(
  syntax: &BlockSyntaxContext<'_>,
  t: &Rav1dTaskContext,
) -> Result<InterPredictionSyntax, ()> {
  let r#ref = syntax.frame_header.skip_mode.refs.map(|raw| {
    let Ok(reference) = console_core_number_conversion::wrapped::<_, i8>(raw);
    reference
  });
  let mut mvstack = [Default::default(); 8];
  let mut context = 0;
  let _candidate_count = rav1d_refmvs_find(
    &t.rt,
    &syntax.frame.rf,
    &mut mvstack,
    &mut context,
    crate::src::refmvs::RefMvsSearch {
      references: RefMvsRefPair::from(r#ref.map(|reference| reference.wrapping_add(1))),
      block: syntax.block,
      edges: syntax.edges,
      origin: Point { x: t.b.x, y: t.b.y },
    },
    syntax.frame_header,
  )
  .map_err(|_invalid_neighbors| ())?;

  let [candidate, ..] = mvstack;
  let [mut first, mut second] = candidate.mv.mv;
  let Ok(()) = fix_mv_precision(syntax.frame_header, &mut first);
  let Ok(()) = fix_mv_precision(syntax.frame_header, &mut second);
  let Ok(trace) = debug_block_info(syntax.frame, t.b);

  match trace {
    BlockTrace::Enabled => {
      let [first_ref, second_ref] = r#ref;
      println!(
        "Post-skipmodeblock[mv=1:y={},x={},2:y={},x={},refs={}+{}",
        first.y, first.x, second.y, second.x, first_ref, second_ref,
      );
    }
    BlockTrace::Disabled => {}
  }

  Ok(InterPredictionSyntax {
    nd: Av1BlockInterNd::from(Av1BlockInter1d {
      mv: [first, second],
      ..Default::default()
    }),
    comp_type: Some(CompInterType::Avg),
    inter_mode: NEARESTMV_NEARESTMV,
    motion_mode: Default::default(),
    drl_idx: DrlProximity::Nearest,
    r#ref,
    interintra_type: None,
    needs_filter: SwitchableFilter::Regular,
  })
}

fn reference_bit(
  entropy: &mut MsacContext,
  table: &mut [[u16; 2]],
  context: u8,
) -> Result<EntropyBit, ()> {
  let Ok(position) = console_core_number_conversion::index(context);
  let cdf = table.get_mut(position).ok_or(())?;
  let Ok(bit) = rav1d_msac_decode_bool_adapt(entropy, cdf);
  Ok(bit)
}

fn reference_branch(
  entropy: &mut MsacContext,
  table: &mut [[[u16; 2]; 3]],
  counts: &ReferenceCounts,
  (level, context): (u8, ReferenceContext),
) -> Result<EntropyBit, ()> {
  let Ok(position) = console_core_number_conversion::index(level);
  let row = table.get_mut(position).ok_or(())?;
  let Ok(context_index) = counts.context(context);
  reference_bit(entropy, row, context_index)
}

fn read_compound_opposite(
  tile: &mut Rav1dTileStateContext,
  counts: &ReferenceCounts,
) -> Result<[i8; 2], ()> {
  use ReferenceContext::{BackwardGroup, FarForward, ForwardGroup, NearBackward, NearForward};
  let forward = &mut *tile.cdf.mi.comp_fwd_ref;
  let group = reference_branch(&mut tile.msac, forward, counts, (0, ForwardGroup))?;
  let (part, start) = match group {
    EntropyBit::Zero => ((1, NearForward), 0_i8),
    EntropyBit::One => ((2, FarForward), 2_i8),
  };
  let near = reference_branch(&mut tile.msac, forward, counts, part)?;
  let first = match near {
    EntropyBit::Zero => start,
    EntropyBit::One => start.wrapping_add(1),
  };

  let backward = &mut *tile.cdf.mi.comp_bwd_ref;
  let backward_group = reference_branch(&mut tile.msac, backward, counts, (0, BackwardGroup))?;
  let second = match backward_group {
    EntropyBit::One => 6,
    EntropyBit::Zero => {
      let near_backward = reference_branch(&mut tile.msac, backward, counts, (1, NearBackward))?;

      match near_backward {
        EntropyBit::Zero => 4,
        EntropyBit::One => 5,
      }
    }
  };

  Ok([first, second])
}

fn read_compound_same(
  tile: &mut Rav1dTileStateContext,
  counts: &ReferenceCounts,
) -> Result<[i8; 2], ()> {
  use ReferenceContext::{Direction, FarForward, UnidirectionalPair};
  let table = &mut *tile.cdf.mi.comp_uni_ref;
  let backward = reference_branch(&mut tile.msac, table, counts, (0, Direction))?;

  match backward {
    EntropyBit::One => return Ok([4, 6]),
    EntropyBit::Zero => {}
  }

  let group = reference_branch(&mut tile.msac, table, counts, (1, UnidirectionalPair))?;

  match group {
    EntropyBit::Zero => Ok([0, 1]),
    EntropyBit::One => {
      let far = reference_branch(&mut tile.msac, table, counts, (2, FarForward))?;

      match far {
        EntropyBit::Zero => Ok([0, 2]),
        EntropyBit::One => Ok([0, 3]),
      }
    }
  }
}

fn read_compound_references(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  tile: &mut Rav1dTileStateContext,
) -> Result<[i8; 2], ()> {
  let Point { x, y } = syntax.geometry.luma.origin;
  let neighbors = || BlockNeighbors {
    above: syntax.above,
    left: &task.l,
    location: Bxy { x, y },
    availability: syntax.neighbors,
  };
  let counts = ReferenceCounts::from_neighbors(neighbors()).map_err(|_invalid_reference| ())?;
  let context = get_comp_dir_ctx(neighbors()).map_err(|_invalid_context| ())?;
  let direction = reference_bit(&mut tile.msac, &mut *tile.cdf.mi.comp_dir, context)?;

  match direction {
    EntropyBit::One => read_compound_opposite(tile, &counts),
    EntropyBit::Zero => read_compound_same(tile, &counts),
  }
}

enum CompoundBlend {
  Average,
  Weighted,
  Segment(EntropyBit),
  Wedge { index: u8, sign: EntropyBit },
}

impl CompoundBlend {
  fn parts(self) -> Result<(CompInterType, bool, u8), console_core_never::Never> {
    Ok(match self {
      Self::Average => (CompInterType::Avg, false, 0),
      Self::Weighted => (CompInterType::WeightedAvg, false, 0),
      Self::Segment(sign) => (CompInterType::Seg, sign == EntropyBit::One, 0),
      Self::Wedge { index, sign } => (CompInterType::Wedge, sign == EntropyBit::One, index),
    })
  }
}

fn wedge_context(block: BlockSize) -> Result<u8, ()> {
  let Ok(ordinal) = block.ordinal();
  let Ok(position) = console_core_number_conversion::index(ordinal);
  dav1d_wedge_ctx_lut.get(position).copied().ok_or(())
}

fn read_wedge_index(tile: &mut Rav1dTileStateContext, context: u8) -> Result<u8, ()> {
  let Ok(position) = console_core_number_conversion::index(context);
  let cdf = tile.cdf.mi.wedge_idx.get_mut(position).ok_or(())?;
  let Ok(symbol) = rav1d_msac_decode_symbol_adapt16(&mut tile.msac, cdf, 15);
  let Ok(value) = console_core_number_conversion::wrapped::<_, u8>(symbol);
  Ok(value)
}

fn read_joint_compound(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  tile: &mut Rav1dTileStateContext,
  references: [i8; 2],
) -> Result<CompoundBlend, ()> {
  match syntax.sequence.jnt_comp {
    0 => return Ok(CompoundBlend::Average),
    _ => {}
  }

  let frame = syntax.frame;
  let reference_stamp = |slot| -> Result<u32, ()> {
    let Ok(position) = console_core_number_conversion::wrapped_index(slot);
    let reference = frame.refp.get(position).ok_or(())?;
    let header = reference.p.frame_hdr.as_ref().ok_or(())?;
    Ok(u32::from(header.frame_offset))
  };
  let [first_reference, second_reference] = references;
  let first_hint = reference_stamp(first_reference)?;
  let second_hint = reference_stamp(second_reference)?;
  let current_header = frame.cur.frame_hdr.as_ref().ok_or(())?;
  let hints = CompoundOrderHints {
    width: syntax.sequence.order_hint_n_bits,
    current: u32::from(current_header.frame_offset),
    reference_hints: [first_hint, second_hint],
  };
  let Point { x, y } = syntax.geometry.luma.origin;
  let above = syntax.above;
  let left = &task.l;
  let context = get_jnt_comp_ctx(
    hints,
    BlockNeighbors {
      above,
      left,
      location: Bxy { x, y },
      availability: syntax.neighbors,
    },
  )
  .map_err(|_invalid_context| ())?;
  let choice = reference_bit(&mut tile.msac, &mut *tile.cdf.mi.jnt_comp, context)?;
  let Ok(trace) = debug_block_info(frame, task.b);

  match trace {
    BlockTrace::Enabled => {
      let Ok(column) = console_core_number_conversion::wrapped_index(x);
      let Ok(row) = console_core_number_conversion::wrapped_index(y);
      let [above_first, ..] = &above.r#ref;
      let [left_first, ..] = &left.r#ref;
      println!(
        "Post-jnt_comp[{},ctx={}[ac:{:?},ar:{},lc:{:?},lr:{}]]: r={}",
        choice == EntropyBit::One,
        context,
        *above.comp_type.index(column),
        *above_first.index(column),
        *left.comp_type.index(row),
        *left_first.index(row),
        tile.msac.rng,
      );
    }
    BlockTrace::Disabled => {}
  }

  Ok(match choice {
    EntropyBit::One => CompoundBlend::Average,
    EntropyBit::Zero => CompoundBlend::Weighted,
  })
}

fn read_masked_compound(
  syntax: &BlockSyntaxContext<'_>,
  origin: Bxy,
  tile: &mut Rav1dTileStateContext,
) -> Result<CompoundBlend, ()> {
  let Ok(ordinal) = syntax.block.ordinal();
  let wedge =
    match (wedge_allowed_mask & 1_u32.wrapping_shl(u32::from(ordinal)) != 0).then_some(()) {
      Some(()) => {
        let context = wedge_context(syntax.block)?;
        let kind = reference_bit(&mut tile.msac, &mut *tile.cdf.mi.wedge_comp, context)?;

        match kind {
          EntropyBit::One => None,
          EntropyBit::Zero => {
            let index = read_wedge_index(tile, context)?;
            Some(index)
          }
        }
      }
      None => None,
    };
  let Ok(sign) = rav1d_msac_decode_bool_equi(&mut tile.msac);
  let blend = match wedge {
    Some(index) => CompoundBlend::Wedge { index, sign },
    None => CompoundBlend::Segment(sign),
  };
  let Ok(trace) = debug_block_info(syntax.frame, origin);

  match trace {
    BlockTrace::Enabled => {
      let index = match wedge {
        Some(value) => value,
        None => 0,
      };
      println!(
        "Post-seg/wedge[{},wedge_idx={},sign={}]: r={}",
        wedge.is_some(),
        index,
        sign == EntropyBit::One,
        tile.msac.rng,
      );
    }
    BlockTrace::Disabled => {}
  }

  Ok(blend)
}

fn read_compound_blend(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  tile: &mut Rav1dTileStateContext,
  references: [i8; 2],
) -> Result<CompoundBlend, ()> {
  match syntax.sequence.masked_compound {
    0 => return read_joint_compound(syntax, task, tile, references),
    _ => {}
  }

  let Point { x, y } = syntax.geometry.luma.origin;
  let mask_context = get_mask_comp_ctx(BlockNeighbors {
    above: syntax.above,
    left: &task.l,
    location: Bxy { x, y },
    availability: syntax.neighbors,
  })
  .map_err(|_invalid_context| ())?;
  let choice = reference_bit(&mut tile.msac, &mut *tile.cdf.mi.mask_comp, mask_context)?;
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match trace {
    BlockTrace::Enabled => println!(
      "Post-segwedge_vs_jntavg[{},ctx={}]: r={}",
      choice == EntropyBit::One,
      mask_context,
      tile.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  match choice {
    EntropyBit::Zero => read_joint_compound(syntax, task, tile, references),
    EntropyBit::One => read_masked_compound(syntax, task.b, tile),
  }
}

fn compound_motion_candidates(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  references: [i8; 2],
) -> Result<([crate::src::refmvs::RefMvsCandidate; 8], u8, i32), ()> {
  let mut candidates = [Default::default(); 8];
  let mut context = 0;
  let count = rav1d_refmvs_find(
    &task.rt,
    &syntax.frame.rf,
    &mut candidates,
    &mut context,
    crate::src::refmvs::RefMvsSearch {
      references: RefMvsRefPair::from(references.map(|reference| reference.wrapping_add(1))),
      block: syntax.block,
      edges: syntax.edges,
      origin: Point {
        x: task.b.x,
        y: task.b.y,
      },
    },
    syntax.frame_header,
  )
  .map_err(|_invalid_neighbors| ())?;

  Ok((candidates, count, context))
}

fn read_compound_mode(
  tile: &mut Rav1dTileStateContext,
  candidates: &BlockMotionCandidates<'_>,
  trace: BlockTrace,
) -> Result<u8, ()> {
  let Ok(position) = console_core_number_conversion::wrapped_index(candidates.context);
  let cdf = tile.cdf.mi.comp_inter_mode.get_mut(position).ok_or(())?;
  let ceiling = N_COMP_INTER_PRED_MODES.wrapping_sub(1);
  let Ok(mode) = rav1d_msac_decode_symbol_adapt8(&mut tile.msac, cdf, ceiling);

  match trace {
    BlockTrace::Enabled => println!(
      "Post-compintermode[{},ctx={},n_mvs={}]: r={}",
      mode, candidates.context, candidates.count, tile.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  Ok(mode)
}

fn select_compound_candidate(
  tile: &mut Rav1dTileStateContext,
  candidates: &BlockMotionCandidates<'_>,
  mode: u8,
  component_modes: [u8; 2],
  trace: BlockTrace,
) -> Result<DrlProximity, ()> {
  let starting_at = match (mode, component_modes) {
    (NEWMV_NEWMV, _) => (DrlStart::NewMotion, 1),
    (_, [NEARMV, _] | [_, NEARMV]) => (DrlStart::NearReference, 2),
    _ => return Ok(DrlProximity::Nearest),
  };
  let (start, threshold) = starting_at;
  let selection = decode_drl_index(tile, candidates.stack, candidates.count, start)?;

  match (trace, (candidates.count > threshold).then_some(())) {
    (BlockTrace::Enabled, Some(())) => println!(
      "Post-drlidx[{:?},n_mvs={}]: r={}",
      selection, candidates.count, tile.msac.rng,
    ),
    (BlockTrace::Enabled, None) | (BlockTrace::Disabled, _) => {}
  }

  Ok(selection)
}

struct CompoundMotionSelection {
  modes: [u8; 2],
  references: [i8; 2],
  initial: [Mv; 2],
  filter: bool,
}

fn decode_compound_vectors(
  syntax: &BlockSyntaxContext<'_>,
  tile: &mut Rav1dTileStateContext,
  origin: Bxy,
  selection: CompoundMotionSelection,
) -> Result<([Mv; 2], SwitchableFilter), ()> {
  let CompoundMotionSelection {
    modes,
    references,
    initial,
    mut filter,
  } = selection;
  let header = syntax.frame_header;
  let precision = match (header.hp, header.force_integer_mv) {
    (false, true) => ResidualPrecision::Integer,
    (false, false) | (true, true) => ResidualPrecision::QuarterPel,
    (true, false) => ResidualPrecision::EighthPel,
  };
  let mut vectors = [Mv::default(); 2];

  for (((output, mode), slot), mut vector) in
    vectors.iter_mut().zip(modes).zip(references).zip(initial)
  {
    *output = match mode {
      NEARMV | NEARESTMV => {
        let Ok(()) = fix_mv_precision(header, &mut vector);
        vector
      }
      GLOBALMV => {
        let global = global_motion_vector(syntax, origin, slot)?;
        filter |= global.filter;
        global.motion
      }
      NEWMV => {
        read_mv_residual(tile, &mut vector, precision).map_err(|_invalid_residual| ())?;
        vector
      }
      _ => return Err(()),
    };
  }

  Ok((vectors, SwitchableFilter::from(filter)))
}

fn read_compound_motion(
  syntax: &BlockSyntaxContext<'_>,
  t: &mut Rav1dTaskContext,
  tile: &mut Rav1dTileStateContext,
) -> Result<InterPredictionSyntax, ()> {
  let references = read_compound_references(syntax, t, tile)?;
  let Ok(trace) = debug_block_info(syntax.frame, t.b);

  match trace {
    BlockTrace::Enabled => {
      let [first, second] = references;
      println!("Post-refs[{}/{}]: r={}", first, second, tile.msac.rng);
    }
    BlockTrace::Disabled => {}
  }

  let (stack, count, context) = compound_motion_candidates(syntax, t, references)?;
  let candidates = BlockMotionCandidates {
    stack: &stack,
    count,
    context,
  };
  let inter_mode = read_compound_mode(tile, &candidates, trace)?;
  let Ok(mode_index) = console_core_number_conversion::wrapped_index(inter_mode);
  let &modes = dav1d_comp_inter_pred_modes.get(mode_index).ok_or(())?;
  let drl_idx = select_compound_candidate(tile, &candidates, inter_mode, modes, trace)?;
  let Ok(rank) = drl_idx.ordinal();
  let Ok(position) = console_core_number_conversion::index(rank);
  let predictor = stack.get(position).ok_or(())?;
  let size = syntax.geometry.luma.size;
  let selection = CompoundMotionSelection {
    modes,
    references,
    initial: predictor.mv.mv,
    filter: size.width.min(size.height) == 1 || inter_mode != GLOBALMV_GLOBALMV,
  };
  let (vectors, needs_filter) = decode_compound_vectors(syntax, tile, t.b, selection)?;

  match trace {
    BlockTrace::Enabled => {
      let [first, second] = vectors;
      println!(
        "Post-residual_mv[1:y={},x={},2:y={},x={}]: r={}",
        first.y, first.x, second.y, second.x, tile.msac.rng,
      );
    }
    BlockTrace::Disabled => {}
  }

  let blend = read_compound_blend(syntax, t, tile, references)?;
  let Ok((comp_type, mask_sign, wedge_idx)) = blend.parts();
  let data = Av1BlockInter1d {
    mv: vectors,
    wedge_idx,
    mask_sign: u8::from(mask_sign),
    ..Default::default()
  };

  Ok(InterPredictionSyntax {
    nd: Av1BlockInterNd::from(data),
    comp_type: Some(comp_type),
    inter_mode,
    motion_mode: Default::default(),
    drl_idx,
    r#ref: references,
    interintra_type: None,
    needs_filter,
  })
}

fn read_single_reference(
  tile: &mut Rav1dTileStateContext,
  counts: &ReferenceCounts,
) -> Result<i8, ()> {
  use ReferenceContext::{
    BackwardGroup, Direction, FarForward, ForwardGroup, NearBackward, NearForward,
  };
  let table = &mut *tile.cdf.mi.r#ref;
  let direction = reference_branch(&mut tile.msac, table, counts, (0, Direction))?;
  let ((level, context), start) = match direction {
    EntropyBit::One => {
      let group = reference_branch(&mut tile.msac, table, counts, (1, BackwardGroup))?;

      match group {
        EntropyBit::One => return Ok(6),
        EntropyBit::Zero => ((5, NearBackward), 4_i8),
      }
    }
    EntropyBit::Zero => {
      let group = reference_branch(&mut tile.msac, table, counts, (2, ForwardGroup))?;

      match group {
        EntropyBit::One => ((4, FarForward), 2_i8),
        EntropyBit::Zero => ((3, NearForward), 0_i8),
      }
    }
  };
  let choice = reference_branch(&mut tile.msac, table, counts, (level, context))?;

  Ok(match choice {
    EntropyBit::Zero => start,
    EntropyBit::One => start.wrapping_add(1),
  })
}

struct SingleMotionVector {
  mode: u8,
  motion: Mv,
  reference: DrlProximity,
  filter: bool,
}

struct BlockMotionCandidates<'frame> {
  stack: &'frame [crate::src::refmvs::RefMvsCandidate; 8],
  count: u8,
  context: i32,
}

fn first_candidate_motion(
  stack: &[crate::src::refmvs::RefMvsCandidate; 8],
  choice: DrlProximity,
) -> Result<Mv, ()> {
  let Ok(ordinal) = choice.ordinal();
  let Ok(position) = console_core_number_conversion::index(ordinal);
  let candidate = stack.get(position).ok_or(())?;
  let [motion, _paired] = candidate.mv.mv;
  Ok(motion)
}

fn read_new_motion_vector(
  syntax: &BlockSyntaxContext<'_>,
  tile: &mut Rav1dTileStateContext,
  origin: Bxy,
  candidates: &BlockMotionCandidates<'_>,
) -> Result<SingleMotionVector, ()> {
  let reference = decode_drl_index(
    tile,
    candidates.stack,
    candidates.count,
    DrlStart::NewMotion,
  )?;
  let mut motion = first_candidate_motion(candidates.stack, reference)?;
  let header = syntax.frame_header;

  match candidates.count {
    0 | 1 => {
      let Ok(()) = fix_mv_precision(header, &mut motion);
    }
    2.. => {}
  }

  let Ok(trace) = debug_block_info(syntax.frame, origin);

  match trace {
    BlockTrace::Enabled => println!(
      "Post-intermode[{},drl={:?}]: r={}",
      NEWMV, reference, tile.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  let precision = match (header.hp, header.force_integer_mv) {
    (false, true) => ResidualPrecision::Integer,
    (false, false) | (true, true) => ResidualPrecision::QuarterPel,
    (true, false) => ResidualPrecision::EighthPel,
  };
  read_mv_residual(tile, &mut motion, precision).map_err(|_invalid_motion_residual| ())?;

  match trace {
    BlockTrace::Enabled => println!(
      "Post-residualmv[mv=y:{},x:{}]: r={}",
      motion.y, motion.x, tile.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  Ok(SingleMotionVector {
    mode: NEWMV,
    motion,
    reference,
    filter: true,
  })
}

fn read_reference_motion_vector(
  syntax: &BlockSyntaxContext<'_>,
  tile: &mut Rav1dTileStateContext,
  candidates: &BlockMotionCandidates<'_>,
) -> Result<SingleMotionVector, ()> {
  let Ok(context) =
    console_core_number_conversion::wrapped::<_, u8>(candidates.context.wrapping_shr(4) & 15);
  let nearby = reference_bit(&mut tile.msac, &mut *tile.cdf.mi.refmv_mode, context)?;
  let (mode, reference) = match nearby {
    EntropyBit::Zero => (NEARESTMV, DrlProximity::Nearest),
    EntropyBit::One => {
      let selected = decode_drl_index(
        tile,
        candidates.stack,
        candidates.count,
        DrlStart::NearReference,
      )?;
      (NEARMV, selected)
    }
  };
  let mut motion = first_candidate_motion(candidates.stack, reference)?;

  match reference {
    DrlProximity::Nearest | DrlProximity::Nearer => {
      let Ok(()) = fix_mv_precision(syntax.frame_header, &mut motion);
    }
    DrlProximity::Near | DrlProximity::Nearish => {}
  }

  Ok(SingleMotionVector {
    mode,
    motion,
    reference,
    filter: true,
  })
}

fn global_motion_vector(
  syntax: &BlockSyntaxContext<'_>,
  origin: Bxy,
  reference: i8,
) -> Result<SingleMotionVector, ()> {
  let Ok(position) = console_core_number_conversion::wrapped_index(reference);
  let header = syntax.frame_header;
  let warp = header.gmv.get(position).ok_or(())?;
  let size = syntax.geometry.luma.size;
  let Ok(motion) = get_gmv_2d(
    warp,
    Rectangle {
      origin: Point {
        x: origin.x,
        y: origin.y,
      },
      size,
    },
    header,
  );
  let filter =
    size.width.min(size.height) == 1 || warp.r#type == Rav1dWarpedMotionType::Translation;

  Ok(SingleMotionVector {
    mode: GLOBALMV,
    motion,
    reference: DrlProximity::Nearest,
    filter,
  })
}

fn read_single_motion_vector(
  syntax: &BlockSyntaxContext<'_>,
  tile: &mut Rav1dTileStateContext,
  origin: Bxy,
  reference: i8,
  candidates: BlockMotionCandidates<'_>,
) -> Result<SingleMotionVector, ()> {
  let force_global = syntax
    .segment
    .is_some_and(|segment| segment.skip != 0 || segment.globalmv != 0);
  let choice = match force_global.then_some(()) {
    Some(()) => {
      let selected = global_motion_vector(syntax, origin, reference)?;
      selected
    }
    None => {
      let Ok(new_context) =
        console_core_number_conversion::wrapped::<_, u8>(candidates.context & 7);
      let new_mode = reference_bit(&mut tile.msac, &mut *tile.cdf.mi.newmv_mode, new_context)?;

      match new_mode {
        EntropyBit::Zero => return read_new_motion_vector(syntax, tile, origin, &candidates),
        EntropyBit::One => {}
      }

      let Ok(global_context) =
        console_core_number_conversion::wrapped::<_, u8>(candidates.context.wrapping_shr(3) & 1);
      let global = reference_bit(
        &mut tile.msac,
        &mut *tile.cdf.mi.globalmv_mode,
        global_context,
      )?;

      match global {
        EntropyBit::Zero => {
          let selected = global_motion_vector(syntax, origin, reference)?;
          selected
        }
        EntropyBit::One => {
          let selected = read_reference_motion_vector(syntax, tile, &candidates)?;
          selected
        }
      }
    }
  };
  let Ok(trace) = debug_block_info(syntax.frame, origin);

  match trace {
    BlockTrace::Enabled => println!(
      "Post-intermode[{},drl={:?},mv=y:{},x:{},n_mvs={}]: r={}",
      choice.mode,
      choice.reference,
      choice.motion.y,
      choice.motion.x,
      candidates.count,
      tile.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  Ok(choice)
}

enum InterIntraSyntax {
  Off,
  Blend(InterIntraPredMode),
  Wedge { mode: InterIntraPredMode, index: u8 },
}

impl InterIntraSyntax {
  fn parts(
    self,
  ) -> Result<(InterIntraPredMode, Option<InterIntraType>, u8), console_core_never::Never> {
    Ok(match self {
      Self::Off => (InterIntraPredMode::default(), None, 0),
      Self::Blend(mode) => (mode, Some(InterIntraType::Blend), 0),
      Self::Wedge { mode, index } => (mode, Some(InterIntraType::Wedge), index),
    })
  }
}

fn read_interintra_syntax(
  syntax: &BlockSyntaxContext<'_>,
  tile: &mut Rav1dTileStateContext,
) -> Result<InterIntraSyntax, ()> {
  let Ok(ordinal) = syntax.block.ordinal();
  let Ok(size_index) = console_core_number_conversion::index(ordinal);
  let &group = dav1d_ymode_size_context.get(size_index).ok_or(())?;

  match (syntax.sequence.inter_intra != 0
    && interintra_allowed_mask & 1_u32.wrapping_shl(u32::from(ordinal)) != 0)
    .then_some(())
  {
    Some(()) => {}
    None => return Ok(InterIntraSyntax::Off),
  }

  let present = reference_bit(&mut tile.msac, &mut *tile.cdf.mi.interintra, group)?;

  match present {
    EntropyBit::One => {}
    EntropyBit::Zero => return Ok(InterIntraSyntax::Off),
  }

  let Ok(context) = console_core_number_conversion::index(group);
  let cdf = tile.cdf.mi.interintra_mode.get_mut(context).ok_or(())?;
  let Ok(count) = console_core_number_conversion::wrapped::<_, u8>(InterIntraPredMode::COUNT);
  let Ok(symbol) = rav1d_msac_decode_symbol_adapt4(&mut tile.msac, cdf, count.wrapping_sub(1));
  let Ok(position) = console_core_number_conversion::wrapped_index(symbol);
  let mode = InterIntraPredMode::from_repr(position).ok_or(())?;
  let wedge = wedge_context(syntax.block)?;
  let selected = reference_bit(&mut tile.msac, &mut *tile.cdf.mi.interintra_wedge, wedge)?;

  match selected {
    EntropyBit::Zero => Ok(InterIntraSyntax::Blend(mode)),
    EntropyBit::One => {
      let index = read_wedge_index(tile, wedge)?;
      Ok(InterIntraSyntax::Wedge { mode, index })
    }
  }
}

fn select_single_reference(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
) -> Result<i8, ()> {
  match syntax.segment {
    Some(segment) => {
      match (segment.r#ref > 0).then_some(()) {
        Some(()) => {
          let Ok(reference) = console_core_number_conversion::wrapped::<_, i8>(segment.r#ref);
          return Ok(reference.wrapping_sub(1));
        }
        None => {}
      }

      match (segment.globalmv != 0 || segment.skip != 0).then_some(()) {
        Some(()) => return Ok(0),
        None => {}
      }
    }
    None => {}
  }

  let Point { x, y } = syntax.geometry.luma.origin;
  let references = ReferenceCounts::from_neighbors(BlockNeighbors {
    above: syntax.above,
    left: &task.l,
    location: Bxy { x, y },
    availability: syntax.neighbors,
  })
  .map_err(|_invalid_reference| ())?;
  let selected = read_single_reference(entropy, &references)?;
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match trace {
    BlockTrace::Enabled => println!("Post-ref[{selected}]: r={}", entropy.msac.rng),
    BlockTrace::Disabled => {}
  }

  Ok(selected)
}

fn single_reference_vector(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
  selected: i8,
) -> Result<SingleMotionVector, ()> {
  let mut stack = [Default::default(); 8];
  let mut context = 0;
  let count = rav1d_refmvs_find(
    &task.rt,
    &syntax.frame.rf,
    &mut stack,
    &mut context,
    crate::src::refmvs::RefMvsSearch {
      references: RefMvsRefPair {
        r#ref: [selected.wrapping_add(1), -1],
      },
      block: syntax.block,
      edges: syntax.edges,
      origin: Point {
        x: task.b.x,
        y: task.b.y,
      },
    },
    syntax.frame_header,
  )
  .map_err(|_invalid_motion_neighbors| ())?;
  let candidates = BlockMotionCandidates {
    stack: &stack,
    count,
    context,
  };

  read_single_motion_vector(syntax, entropy, task.b, selected, candidates)
}

fn trace_single_interintra(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  entropy: &MsacContext,
  (mode, kind, wedge): (InterIntraPredMode, Option<InterIntraType>, u8),
) -> Result<(), console_core_never::Never> {
  let Ok(trace) = debug_block_info(syntax.frame, task.b);
  let Ok(block) = syntax.block.ordinal();
  let permitted = syntax.sequence.inter_intra != 0
    && interintra_allowed_mask & 1u32.wrapping_shl(u32::from(block)) != 0;

  match (trace, permitted.then_some(())) {
    (BlockTrace::Enabled, Some(())) => {
      println!(
        "Post-interintra[t={kind:?},m={mode:?},w={wedge}]: r={}",
        entropy.rng
      );
    }
    (BlockTrace::Enabled, None) | (BlockTrace::Disabled, _) => {}
  }

  Ok(())
}

fn overlap_neighbors(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
) -> Result<OddNeighborPrediction, ()> {
  let Point { x, y } = syntax.geometry.luma.origin;
  let visible = syntax.geometry.visible_luma;

  match syntax.neighbors {
    NeighborAvailability::Left | NeighborAvailability::Both => {
      let Ok(first) = console_core_number_conversion::wrapped_index(y);
      let Ok(end) = console_core_number_conversion::wrapped_index(y.wrapping_add(visible.height));
      let modes = task.l.intra.get_mut().get(first..end).ok_or(())?;
      let Ok(prediction) = odd_neighbor_prediction(modes);

      match prediction {
        OddNeighborPrediction::HasInter => return Ok(prediction),
        OddNeighborPrediction::IntraOnly => {}
      }
    }
    NeighborAvailability::Top | NeighborAvailability::Neither => {}
  }

  match syntax.neighbors {
    NeighborAvailability::Top | NeighborAvailability::Both => {
      let Ok(first) = console_core_number_conversion::wrapped_index(x);
      let Ok(end) = console_core_number_conversion::wrapped_index(x.wrapping_add(visible.width));
      let modes = syntax.above.intra.index(first..end);
      let Ok(prediction) = odd_neighbor_prediction(&modes);
      Ok(prediction)
    }
    NeighborAvailability::Left | NeighborAvailability::Neither => {
      Ok(OddNeighborPrediction::IntraOnly)
    }
  }
}

enum SingleMotionMode {
  Translation,
  Obmc,
  LiveWarp,
  StoredWarp(BlockWarp),
}

struct SingleModeInput {
  selected: i8,
  vector: Mv,
  inter_mode: u8,
  interintra: Option<InterIntraType>,
}

fn derive_single_warp(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  vector: Mv,
  masks: [u64; 2],
) -> Result<SingleMotionMode, ()> {
  let grid = WarpNeighborGrid {
    blocks: &syntax.frame.rf.r,
    tile: &task.rt,
    origin: Point {
      x: task.b.x,
      y: task.b.y,
    },
  };
  let derived = derive_warpmv(
    &grid,
    syntax.geometry.luma.size,
    masks,
    vector,
    task.warpmv.clone(),
  )
  .map_err(|_invalid_warp_sample| ())?;
  task.warpmv = derived;
  let Ok(()) = trace_motion_warp(syntax.frame, task, vector);

  match task.frame_thread.pass {
    0 => Ok(SingleMotionMode::LiveWarp),
    _ => {
      let recorded = capture_block_warp(&task.warpmv).map_err(|_invalid_warp| ())?;
      Ok(SingleMotionMode::StoredWarp(recorded))
    }
  }
}

fn choose_single_motion_mode(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
  input: SingleModeInput,
) -> Result<SingleMotionMode, ()> {
  let header = syntax.frame_header;
  let block = syntax.geometry.luma.size;
  let signalled = header.switchable_motion_mode != 0
    && input.interintra.is_none()
    && block.width.min(block.height) >= 2;

  match signalled.then_some(()) {
    Some(()) => {}
    None => return Ok(SingleMotionMode::Translation),
  }

  let Ok(index) = console_core_number_conversion::wrapped_index(input.selected);

  match (!header.force_integer_mv && input.inter_mode == GLOBALMV).then_some(()) {
    Some(()) => {
      let global = header.gmv.get(index).ok_or(())?;

      match (global.r#type > Rav1dWarpedMotionType::Translation).then_some(()) {
        Some(()) => return Ok(SingleMotionMode::Translation),
        None => {}
      }
    }
    None => {}
  }

  let neighbors = overlap_neighbors(syntax, task)?;

  match neighbors {
    OddNeighborPrediction::IntraOnly => return Ok(SingleMotionMode::Translation),
    OddNeighborPrediction::HasInter => {}
  }

  let masks = find_matching_ref(
    syntax.frame,
    task,
    WarpNeighborQuery {
      block,
      visible: syntax.geometry.visible_luma,
      availability: syntax.neighbors,
      edge_flags: syntax.edges,
      reference: input.selected,
    },
  )
  .map_err(|_invalid_warp_neighbor| ())?;
  let [luma_scale, _] = syntax.frame.svc.get(index).ok_or(())?;
  let [above, left] = masks;
  let allowed = luma_scale.scale == 0
    && !header.force_integer_mv
    && header.warp_motion != 0
    && (above | left) != 0;
  let availability = match allowed.then_some(()) {
    Some(()) => WarpAvailability::Allowed,
    None => WarpAvailability::Disabled,
  };
  let mode = read_motion_mode(
    &mut entropy.msac,
    &mut entropy.cdf.mi,
    syntax.block,
    availability,
  )
  .map_err(|_invalid_motion_mode| ())?;
  let decoded = match mode {
    MotionMode::Translation => SingleMotionMode::Translation,
    MotionMode::Obmc => SingleMotionMode::Obmc,
    MotionMode::Warp => derive_single_warp(syntax, task, input.vector, masks)?,
  };
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match trace {
    BlockTrace::Enabled => println!(
      "Post-motionmode[{mode:?}]: r={} [mask: 0x{above:x}/0x{left:x}]",
      entropy.msac.rng
    ),
    BlockTrace::Disabled => {}
  }

  Ok(decoded)
}

fn read_single_motion(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  entropy: &mut Rav1dTileStateContext,
) -> Result<InterPredictionSyntax, ()> {
  let selected = select_single_reference(syntax, task, entropy)?;
  let SingleMotionVector {
    mode: inter_mode,
    motion,
    reference: drl_idx,
    filter,
  } = single_reference_vector(syntax, task, entropy, selected)?;
  let blending = read_interintra_syntax(syntax, entropy)?;
  let Ok(parts @ (interintra_mode, interintra_type, wedge_idx)) = blending.parts();
  let Ok(()) = trace_single_interintra(syntax, task, &entropy.msac, parts);
  let mode = choose_single_motion_mode(
    syntax,
    task,
    entropy,
    SingleModeInput {
      selected,
      vector: motion,
      inter_mode,
      interintra: interintra_type,
    },
  )?;
  let (motion_mode, warp, needs_filter) = match mode {
    SingleMotionMode::Translation => (
      MotionMode::Translation,
      None,
      SwitchableFilter::from(filter),
    ),
    SingleMotionMode::Obmc => (MotionMode::Obmc, None, SwitchableFilter::from(filter)),
    SingleMotionMode::LiveWarp => (MotionMode::Warp, None, SwitchableFilter::Regular),
    SingleMotionMode::StoredWarp(recorded) => {
      (MotionMode::Warp, Some(recorded), SwitchableFilter::Regular)
    }
  };
  let nd = match warp {
    None => Av1BlockInterNd::from(Av1BlockInter1d {
      mv: [motion, Default::default()],
      wedge_idx,
      interintra_mode: crate::src::levels::MaskedInterIntraPredMode::from(interintra_mode),
      ..Default::default()
    }),
    Some(recorded) => {
      let Ok(parameters) = Av1BlockInter2d::from_warp(motion, recorded);
      Av1BlockInterNd::from(parameters)
    }
  };

  Ok(InterPredictionSyntax {
    nd,
    comp_type: None,
    inter_mode,
    motion_mode,
    drl_idx,
    r#ref: [selected, -1],
    interintra_type,
    needs_filter,
  })
}

fn filter_symbol(
  tile: &mut Rav1dTileStateContext,
  axis: FilterAxis,
  context: u8,
) -> Result<Rav1dFilterMode, ()> {
  let [first, second] = &mut *tile.cdf.mi.filter;
  let table = match axis {
    FilterAxis::Vertical => first,
    FilterAxis::Horizontal => second,
  };
  let Ok(position) = console_core_number_conversion::index(context);
  let cdf = table.get_mut(position).ok_or(())?;
  let ceiling = u8::from(Rav1dFilterMode::N_SWITCHABLE_FILTERS).wrapping_sub(1);
  let Ok(symbol) = rav1d_msac_decode_symbol_adapt4(&mut tile.msac, cdf, ceiling);
  let Ok(selection) = console_core_number_conversion::wrapped_index(symbol);
  Rav1dFilterMode::from_repr(selection).ok_or(())
}

fn read_filter_pair(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  tile: &mut Rav1dTileStateContext,
  prediction: &InterPredictionSyntax,
) -> Result<[Rav1dFilterMode; 2], ()> {
  let Point { x, y } = syntax.geometry.luma.origin;
  let neighbors = || BlockNeighbors {
    above: syntax.above,
    left: &task.l,
    location: Bxy { x, y },
    availability: syntax.neighbors,
  };
  let arity = match prediction.comp_type {
    Some(_) => ReferenceArity::Compound,
    None => ReferenceArity::Single,
  };
  let [reference, _partner] = prediction.r#ref;
  let context = get_filter_ctx(neighbors(), arity, FilterAxis::Vertical, reference)
    .map_err(|_invalid_context| ())?;
  let first = filter_symbol(tile, FilterAxis::Vertical, context)?;
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match syntax.sequence.dual_filter {
    0 => {
      match trace {
        BlockTrace::Enabled => println!(
          "Post-subpel_filter[{:?},ctx={}]: r={}",
          first, context, tile.msac.rng,
        ),
        BlockTrace::Disabled => {}
      }

      Ok([first; 2])
    }
    _ => {
      let second_context = get_filter_ctx(neighbors(), arity, FilterAxis::Horizontal, reference)
        .map_err(|_invalid_context| ())?;

      match trace {
        BlockTrace::Enabled => println!(
          "Post-subpel_filter1[{:?},ctx={}]: r={}",
          first, context, tile.msac.rng,
        ),
        BlockTrace::Disabled => {}
      }

      let second = filter_symbol(tile, FilterAxis::Horizontal, second_context)?;

      match trace {
        BlockTrace::Enabled => println!(
          "Post-subpel_filter2[{:?},ctx={}]: r={}",
          second, second_context, tile.msac.rng,
        ),
        BlockTrace::Disabled => {}
      }

      Ok([first, second])
    }
  }
}

fn read_block_filter(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  tile: &mut Rav1dTileStateContext,
  prediction: &InterPredictionSyntax,
) -> Result<(Filter2d, [Rav1dFilterMode; 2]), ()> {
  let frame_mode = syntax.frame_header.subpel_filter_mode;
  let modes = match frame_mode {
    Rav1dFilterMode::Switchable => match prediction.needs_filter {
      SwitchableFilter::Read => {
        let pair = read_filter_pair(syntax, task, tile, prediction)?;
        pair
      }
      SwitchableFilter::Regular => [Rav1dFilterMode::Regular8Tap; 2],
    },
    fixed @ (Rav1dFilterMode::Regular8Tap
    | Rav1dFilterMode::Smooth8Tap
    | Rav1dFilterMode::Sharp8Tap
    | Rav1dFilterMode::Bilinear) => [fixed; 2],
  };
  let [horizontal, vertical] = modes;
  let Ok(vertical_index) = console_core_number_conversion::index(u8::from(vertical));
  let Ok(horizontal_index) = console_core_number_conversion::index(u8::from(horizontal));
  let direction = dav1d_filter_2d.get(vertical_index).ok_or(())?;
  let &kernel = direction.get(horizontal_index).ok_or(())?;
  Ok((kernel, modes))
}

fn inter_filter_prediction<'splits>(
  block: &Av1Block,
  inter: &Av1BlockInter,
  split_masks: &'splits [u16; 2],
) -> Result<FilterMaskPrediction<'splits>, console_core_never::Never> {
  let [reference, _partner] = inter.r#ref;
  let global = match inter.comp_type {
    Some(_) => GLOBALMV_GLOBALMV,
    None => GLOBALMV,
  };
  let motion = match (inter.inter_mode == global).then_some(()) {
    Some(()) => FilterMaskMotion::Global,
    None => FilterMaskMotion::Other,
  };
  let scan = match block.skip {
    0 => EdgeScan::AllTransforms,
    _ => EdgeScan::Boundary,
  };
  Ok(FilterMaskPrediction::Inter {
    reference: reference.wrapping_add(1),
    motion,
    scan,
    split_masks,
  })
}

fn publish_inter_loopfilter(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  block: &Av1Block,
  inter: &Av1BlockInter,
) -> Result<(), ()> {
  let header = syntax.frame_header;

  match header.loopfilter.level_y {
    [0, 0] => return Ok(()),
    _ => {}
  }

  let Ok(segment) = block.seg_id.get();
  let Ok(segment_index) = console_core_number_conversion::index(segment);
  let &lossless = header.segmentation.lossless.get(segment_index).ok_or(())?;
  let [ytx, uvtx] = match lossless.then_some(()) {
    Some(()) => [TxfmSize::S4x4; 2],
    None => [inter.max_ytx, block.uvtx],
  };
  let frame = syntax.frame;
  let Ok(level_source) = syntax.tile.lflvl.get();
  let local_levels;
  let all_levels = match level_source {
    TileStateRef::Frame => &frame.lf.lvl,
    TileStateRef::Local => {
      let acquired = syntax.tile.lflvlmem.try_read().ok_or(())?;
      local_levels = acquired;
      &*local_levels
    }
  };
  let levels = all_levels.get(segment_index).ok_or(())?;
  let mask = frame
    .loop_filter_mask(task.lf_mask)
    .map_err(|_invalid_mask| ())?;
  let above = syntax.above;
  let Rectangle { origin, size } = syntax.geometry.luma;
  let Ok(x) = console_core_number_conversion::wrapped_index(origin.x);
  let Ok(y) = console_core_number_conversion::wrapped_index(origin.y);
  let Ok(width) = console_core_number_conversion::wrapped_index(size.width);
  let Ok(height) = console_core_number_conversion::wrapped_index(size.height);
  let mut above_luma = above.tx_lpf_y.index_mut((x.., ..width));
  let left_luma = task
    .l
    .tx_lpf_y
    .get_mut()
    .get_mut(y..)
    .and_then(|rows| rows.get_mut(..height))
    .ok_or(())?;
  let mut above_chroma;
  let chroma = match syntax.geometry.has_chroma.then_some(()) {
    Some(()) => {
      let Rectangle {
        origin: chroma_origin,
        size: chroma_size,
      } = syntax.geometry.chroma;
      let Ok(column) = console_core_number_conversion::wrapped_index(chroma_origin.x);
      let Ok(row) = console_core_number_conversion::wrapped_index(chroma_origin.y);
      let Ok(columns) = console_core_number_conversion::wrapped_index(chroma_size.width);
      let Ok(rows) = console_core_number_conversion::wrapped_index(chroma_size.height);
      above_chroma = above.tx_lpf_uv.index_mut((column.., ..columns));
      let left = task
        .l
        .tx_lpf_uv
        .get_mut()
        .get_mut(row..)
        .and_then(|available| available.get_mut(..rows))
        .ok_or(())?;
      Some((&mut *above_chroma, left))
    }
    None => None,
  };
  let split_masks = [u16::from(inter.tx_split0), inter.tx_split1];
  let Ok(prediction) = inter_filter_prediction(block, inter, &split_masks);
  let filter = FilterMaskBlock {
    mask,
    level_cache: &frame.lf.level,
    stride: i64::from(frame.b4_stride),
    levels,
    origin: task.b,
    frame: Size {
      width: frame.w4,
      height: frame.h4,
    },
    size: syntax.block,
    ytx,
    uvtx,
    layout: frame.cur.p.layout,
    luma: EdgeNeighbors {
      above: &mut above_luma,
      left: left_luma,
    },
    chroma,
  };
  create_lf_mask(filter, prediction).map_err(|error| {
    console_core_logging::debug!("Could not create an AV1 inter filter mask: {error:?}");
  })
}

fn select_inter_syntax(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  block: &Av1Block,
  tile: &mut Rav1dTileStateContext,
) -> Result<InterPredictionSyntax, ()> {
  match block.skip_mode {
    0 => {}
    _ => return read_skip_motion(syntax, task),
  }

  let size = syntax.geometry.luma.size;
  let switchable = matches!(
    segment_prediction(syntax.segment),
    Ok(SegmentPrediction::Open)
  ) && syntax.frame_header.switchable_comp_refs != 0
    && size.width.min(size.height) > 1;

  match switchable.then_some(()) {
    Some(()) => {}
    None => return read_single_motion(syntax, task, tile),
  }

  let Point { x, y } = syntax.geometry.luma.origin;
  let context = get_comp_ctx(BlockNeighbors {
    above: syntax.above,
    left: &task.l,
    location: Bxy { x, y },
    availability: syntax.neighbors,
  })
  .map_err(|_invalid_context| ())?;
  let compound = reference_bit(&mut tile.msac, &mut *tile.cdf.mi.comp, context)?;
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match trace {
    BlockTrace::Enabled => println!(
      "Post-compflag[{}]: r={}",
      compound == EntropyBit::One,
      tile.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  match compound {
    EntropyBit::One => read_compound_motion(syntax, task, tile),
    EntropyBit::Zero => read_single_motion(syntax, task, tile),
  }
}

fn publish_inter_neighbors(
  syntax: &BlockSyntaxContext<'_>,
  t: &mut Rav1dTaskContext,
  b: &Av1Block,
  inter: &Av1BlockInter,
  filter: [Rav1dFilterMode; 2],
) -> Result<(), console_core_never::Never> {
  let ta = syntax.above;
  let b_dim = syntax.geometry.dimensions;
  let has_chroma = syntax.geometry.has_chroma;
  let seg_pred = matches!(syntax.segment_origin, SegmentOrigin::Inherited);
  let Point { x: bx4, y: by4 } = syntax.geometry.luma.origin;
  let Size {
    width: bw4,
    height: bh4,
  } = syntax.geometry.luma.size;
  let Point { x: cbx4, y: cby4 } = syntax.geometry.chroma.origin;
  let Size {
    width: cbw4,
    height: cbh4,
  } = syntax.geometry.chroma.size;
  let Av1BlockInter {
    comp_type,
    inter_mode,
    r#ref,
    ..
  } = *inter;

  case_set_al! {
      <32, false>

      l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4); numeric_value })
          also |case| { ({ let Ok(explicit_value) = case.set({ let [_, _explicit_part, ..] = &mut t.pal_sz_uv; _explicit_part }, 0); explicit_value }); },
      a: (ta, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx4); numeric_value })
          also |case| { ({ let Ok(explicit_value) = case.set({ let [_explicit_part, ..] = &mut t.pal_sz_uv; _explicit_part }, 0); explicit_value }); },
      seg_pred = (<::core::primitive::u8 as ::core::convert::From<::core::primitive::bool>>::from(seg_pred), <::core::primitive::u8 as ::core::convert::From<::core::primitive::bool>>::from(seg_pred)),
      skip_mode = (b.skip_mode, b.skip_mode),
      intra = (0, 0),
      skip = (b.skip, b.skip),
      pal_sz = (0, 0),
      tx_intra = ({
        let &[_, _, _, height] = b_dim;
        let Ok(height_log) = console_core_number_conversion::wrapped::<_, i8>(height);
        height_log
      }, {
        let &[_, _, width, _] = b_dim;
        let Ok(width_log) = console_core_number_conversion::wrapped::<_, i8>(width);
        width_log
      }),
      comp_type = (comp_type, comp_type),
      filter[0] = (match filter { [_explicit_element, ..] => _explicit_element }, match filter { [_explicit_element, ..] => _explicit_element }),
      filter[1] = (match filter { [_, _explicit_element, ..] => _explicit_element }, match filter { [_, _explicit_element, ..] => _explicit_element }),
      mode = (inter_mode, inter_mode),
      r#ref[0] = (match r#ref { [element, ..] => element }, match r#ref { [element, ..] => element }),
      r#ref[1] = (match r#ref { [_, element, ..] => element }, match r#ref { [_, element, ..] => element }),
  }

  match ::core::primitive::bool::then_some(has_chroma, ()) {
    ::core::option::Option::Some(()) => {
      case_set_al! {
          <32, false>
          l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbh4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cby4); numeric_value }),
          a: (ta, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbw4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbx4); numeric_value }),
          uvmode = (DC_PRED, DC_PRED),
      }
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn decode_inter_syntax(
  syntax: &BlockSyntaxContext<'_>,
  t: &mut Rav1dTaskContext,
  b: &mut Av1Block,
  ts_c_2: &mut Rav1dTileStateContext,
) -> Result<(), ()> {
  let f = syntax.frame;
  let bs = syntax.block;
  let Ok(bd_fn) = f.bd_fn();
  let prediction = select_inter_syntax(syntax, t, b, ts_c_2)?;
  let (filter2d, filter) = read_block_filter(syntax, t, ts_c_2, &prediction)?;
  let InterPredictionSyntax {
    nd,
    comp_type,
    inter_mode,
    motion_mode,
    drl_idx,
    r#ref,
    interintra_type,
    ..
  } = prediction;
  let Point { x, y } = syntax.geometry.luma.origin;
  let VarTx {
    uvtx,
    max_ytx,
    tx_split0,
    tx_split1,
  } = read_vartx_tree(
    t,
    f,
    ts_c_2,
    b,
    VariableTransformBlock {
      size: bs,
      local: Bxy { x, y },
    },
  )
  .map_err(|_invalid_transform_tree| ())?;
  b.uvtx = uvtx;
  let inter = Av1BlockInter {
    nd,
    comp_type,
    inter_mode,
    motion_mode,
    drl_idx,
    r#ref,
    max_ytx,
    filter2d,
    interintra_type,
    tx_split0,
    tx_split1,
  };
  b.ii = Av1BlockIntraInter::Inter(inter);

  match t.frame_thread.pass {
    1 => {
      (bd_fn.read_coef_blocks)(f, t, ts_c_2, bs, b).map_err(|_invalid_coefficients| ())?;
    }
    _ => {
      (bd_fn.recon_b_inter)(f, t, Some(ts_c_2), bs, b, &inter)?;
    }
  }

  publish_inter_loopfilter(syntax, t, b, &inter)?;
  let size = syntax.geometry.luma.size;
  let decoder = syntax.decoder;

  match comp_type {
    Some(_) => {
      splat_tworef_mv(decoder, t, &f.rf, bs, &inter, size).map_err(|_invalid_motion| ())?;
    }
    None => {
      splat_oneref_mv(decoder, t, &f.rf, bs, &inter, size).map_err(|_invalid_motion| ())?;
    }
  }

  let Ok(()) = publish_inter_neighbors(syntax, t, b, &inter, filter);
  Ok(())
}

enum BlockPredictionForm {
  Intra,
  IntraCopy,
  Inter,
}

struct BlockSyntaxPrefix<'frame> {
  prediction: BlockPredictionForm,
  segment: Option<&'frame Rav1dSegmentationData>,
  segment_origin: SegmentOrigin,
}

fn read_block_skip_modes(
  syntax: &BlockSyntaxContext<'_>,
  t: &mut Rav1dTaskContext,
  b: &mut Av1Block,
  ts_c_2: &mut Rav1dTileStateContext,
  seg: Option<&Rav1dSegmentationData>,
) -> Result<(), ()> {
  let f = syntax.frame;
  let ta = syntax.above;
  let frame_hdr = syntax.frame_header;
  let Point { x: bx4, y: by4 } = syntax.geometry.luma.origin;
  let Size {
    width: bw4,
    height: bh4,
  } = syntax.geometry.luma.size;

  match ::core::primitive::bool::then_some(
    matches!(segment_prediction(seg), Ok(SegmentPrediction::Open))
      && frame_hdr.skip_mode.enabled != 0
      && cmp::min(bw4, bh4) > 1,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let smctx = u8::wrapping_add(
        *ta.skip_mode.index({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx4);
          numeric_value
        }),
        *match <[_]>::get(&**t.l.skip_mode.get_mut(), {
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4);
          numeric_value
        }) {
          ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
          ::core::option::Option::None => return ::core::result::Result::Err(()),
        },
      );
      b.skip_mode = u8::from(matches!(
        rav1d_msac_decode_bool_adapt(
          &mut ts_c_2.msac,
          &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.skip_mode, {
            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(smctx);
            numeric_value
          }) {
            ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
            ::core::option::Option::None => return ::core::result::Result::Err(()),
          },
        ),
        Ok(EntropyBit::One)
      ));

      match {
        let Ok(active_trace) = debug_block_info(f, t.b);
        active_trace
      } {
        BlockTrace::Enabled => {
          println!("Post-skipmode[{}]: r={}", b.skip_mode, ts_c_2.msac.rng);
        }
        BlockTrace::Disabled => {}
      }
    }
    ::core::option::Option::None => {
      b.skip_mode = 0;
    }
  }

  match ::core::primitive::bool::then_some(
    b.skip_mode != 0
      || match seg.map(|seg_11| seg_11.skip != 0) {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => false,
      },
    (),
  ) {
    ::core::option::Option::Some(()) => {
      b.skip = 1;
    }
    ::core::option::Option::None => {
      let sctx = u8::wrapping_add(
        *ta.skip.index({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx4);
          numeric_value
        }),
        *match <[_]>::get(&**t.l.skip.get_mut(), {
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4);
          numeric_value
        }) {
          ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
          ::core::option::Option::None => return ::core::result::Result::Err(()),
        },
      );
      b.skip = u8::from(matches!(
        rav1d_msac_decode_bool_adapt(
          &mut ts_c_2.msac,
          &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.m.skip, {
            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(sctx);
            numeric_value
          }) {
            ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
            ::core::option::Option::None => return ::core::result::Result::Err(()),
          },
        ),
        Ok(EntropyBit::One)
      ));

      match {
        let Ok(active_trace) = debug_block_info(f, t.b);
        active_trace
      } {
        BlockTrace::Enabled => {
          println!("Post-skip[{}]: r={}", b.skip, ts_c_2.msac.rng);
        }
        BlockTrace::Disabled => {}
      }
    }
  }

  Ok(())
}

fn read_scaled_delta(
  entropy: &mut MsacContext,
  cdf: &mut [u16; 4],
  shift: u8,
) -> Result<i32, console_core_never::Never> {
  let Ok(symbol) = rav1d_msac_decode_symbol_adapt4(entropy, cdf, 3);
  let magnitude = match symbol {
    3 => {
      let Ok(prefix) = rav1d_msac_decode_bools(entropy, 3);
      let Ok(bits) = console_core_number_conversion::wrapped::<_, u8>(prefix);
      let count = bits.wrapping_add(1);
      let Ok(payload) = rav1d_msac_decode_bools(entropy, count);
      let value = payload
        .wrapping_add(1)
        .wrapping_add(1_u32.wrapping_shl(u32::from(count)));
      let Ok(signed) = console_core_number_conversion::wrapped::<_, i32>(value);
      signed
    }
    _ => i32::from(symbol),
  };
  let signed = match magnitude {
    0 => return Ok(0),
    _ => match rav1d_msac_decode_bool_equi(entropy) {
      Ok(EntropyBit::Zero) => magnitude,
      Ok(EntropyBit::One) => magnitude.wrapping_neg(),
    },
  };
  Ok(signed.wrapping_mul(1_i32.wrapping_shl(u32::from(shift))))
}

fn read_loopfilter_deltas(
  syntax: &BlockSyntaxContext<'_>,
  origin: Bxy,
  tile: &mut Rav1dTileStateContext,
) -> Result<(), ()> {
  let options = &syntax.frame_header.delta.lf;

  match options.present {
    0 => return Ok(()),
    _ => {}
  }

  let count = match (options.multi, syntax.frame.cur.p.layout) {
    (0, _) => 1_u8,
    (_, Rav1dPixelLayout::I400) => 2,
    (_, Rav1dPixelLayout::I420 | Rav1dPixelLayout::I422 | Rav1dPixelLayout::I444) => 4,
  };
  let Ok(mut levels) = syntax.tile.last_delta_lf.get();
  let Ok(trace) = debug_block_info(syntax.frame, origin);

  for (slot, level) in (0..count).zip(&mut levels) {
    let probability = slot.wrapping_add(options.multi);
    let Ok(index) = console_core_number_conversion::index(probability);
    let cdf = tile.cdf.m.delta_lf.get_mut(index).ok_or(())?;
    let Ok(delta) = read_scaled_delta(&mut tile.msac, cdf, options.res_log2);
    *level = clip(i32::from(*level).wrapping_add(delta), -63, 63);

    match trace {
      BlockTrace::Enabled => println!("Post-delta_lf[{}:{}]: r={}", slot, delta, tile.msac.rng),
      BlockTrace::Disabled => {}
    }
  }

  let Ok(()) = syntax.tile.last_delta_lf.set(levels);
  Ok(())
}

fn update_delta_tables(
  syntax: &BlockSyntaxContext<'_>,
  previous_qidx: u8,
  previous_lf: [i8; 4],
) -> Result<(), ()> {
  let tile = syntax.tile;
  let header = syntax.frame_header;
  let Ok(quantizer) = tile.last_qidx.get();

  match (quantizer == header.quant.yac, quantizer != previous_qidx) {
    (true, _) => {
      let Ok(()) = tile.dq.set(TileStateRef::Frame);
    }
    (false, true) => {
      init_quant_tables(syntax.sequence, header, quantizer, &tile.dqmem)
        .map_err(|_invalid_quantizer| ())?;
      let Ok(()) = tile.dq.set(TileStateRef::Local);
    }
    (false, false) => {}
  }

  let Ok(deltas) = tile.last_delta_lf.get();

  match (deltas == [0; 4], deltas != previous_lf) {
    (true, _) => {
      let Ok(()) = tile.lflvl.set(TileStateRef::Frame);
    }
    (false, true) => {
      {
        let mut levels = tile.lflvlmem.try_write().ok_or(())?;
        let Ok(()) = rav1d_calc_lf_values(&mut levels, header, &deltas);
      }

      let Ok(()) = tile.lflvl.set(TileStateRef::Local);
    }
    (false, false) => {}
  }

  Ok(())
}

fn read_block_delta(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  block: &Av1Block,
  tile: &mut Rav1dTileStateContext,
) -> Result<(), ()> {
  let (superblock, alignment) = match syntax.sequence.sb128 {
    0 => (BlockSize::Bs64x64, 15),
    _ => (BlockSize::Bs128x128, 31),
  };

  match ((task.b.x | task.b.y) & alignment == 0).then_some(()) {
    Some(()) => {}
    None => return Ok(()),
  }

  let Ok(previous_qidx) = syntax.tile.last_qidx.get();
  let options = &syntax.frame_header.delta.q;
  let may_update = options.present != 0 && (syntax.block != superblock || block.skip == 0);
  let Ok(previous_lf) = syntax.tile.last_delta_lf.get();

  match may_update.then_some(()) {
    Some(()) => {
      let Ok(delta) =
        read_scaled_delta(&mut tile.msac, &mut *tile.cdf.m.delta_q, options.res_log2);
      let quantizer = clip(i32::from(previous_qidx).wrapping_add(delta), 1, 255);
      let Ok(()) = syntax.tile.last_qidx.set(quantizer);
      let Ok(trace) = debug_block_info(syntax.frame, task.b);

      match trace {
        BlockTrace::Enabled => println!(
          "Post-delta_q[{}->{}]: r={}",
          delta, quantizer, tile.msac.rng,
        ),
        BlockTrace::Disabled => {}
      }

      read_loopfilter_deltas(syntax, task.b, tile)?;
    }
    None => {}
  }

  update_delta_tables(syntax, previous_qidx, previous_lf)
}

struct SegmentHeader<'frame> {
  features: Option<&'frame Rav1dSegmentationData>,
  origin: SegmentOrigin,
}

#[derive(Clone, Copy)]
enum SegmentSyntax {
  ReadDelta,
  SpatialPrediction,
}

fn previous_block_segment(syntax: &BlockSyntaxContext<'_>, origin: Bxy) -> Result<SegmentId, ()> {
  let frame = syntax.frame;
  let map = frame.prev_segmap.as_ref().map(|prior| prior.inner.as_ref());
  let area = Rectangle {
    origin: Point {
      x: origin.x,
      y: origin.y,
    },
    size: syntax.geometry.visible_luma,
  };
  get_prev_frame_segid(syntax.frame_header, map, area, i64::from(frame.b4_stride))
    .map_err(|_previous_segment| ())
}

fn segment_features(
  header: &Rav1dFrameHeader,
  segment: SegmentId,
) -> Result<&Rav1dSegmentationData, ()> {
  let Ok(id) = segment.get();
  let Ok(position) = console_core_number_conversion::index(id);
  header.segmentation.seg_data.d.get(position).ok_or(())
}

fn read_temporal_segment_bit(
  syntax: &BlockSyntaxContext<'_>,
  task: &mut Rav1dTaskContext,
  tile: &mut Rav1dTileStateContext,
) -> Result<EntropyBit, ()> {
  match syntax.frame_header.segmentation.temporal {
    0 => return Ok(EntropyBit::Zero),
    _ => {}
  }

  let Point { x, y } = syntax.geometry.luma.origin;
  let Ok(column) = console_core_number_conversion::wrapped_index(x);
  let Ok(row) = console_core_number_conversion::wrapped_index(y);
  let above = syntax.above.seg_pred.index(column);
  let &left = task.l.seg_pred.get_mut().get(row).ok_or(())?;
  reference_bit(
    &mut tile.msac,
    &mut *tile.cdf.mi.seg_pred,
    above.wrapping_add(left),
  )
}

fn read_segment_residual(
  syntax: &BlockSyntaxContext<'_>,
  tile: &mut Rav1dTileStateContext,
  predicted: SegmentId,
  context: u8,
) -> Result<SegmentId, ()> {
  let Ok(position) = console_core_number_conversion::index(context);
  let cdf = tile.cdf.m.seg_id.get_mut(position).ok_or(())?;
  let Ok(count) = console_core_number_conversion::wrapped::<_, u8>(SegmentId::COUNT);
  let Ok(difference) =
    rav1d_msac_decode_symbol_adapt8(&mut tile.msac, cdf, count.wrapping_sub(1));
  let last = syntax.frame_header.segmentation.seg_data.last_active_segid;
  let limit = last.wrapping_add(1).cast_unsigned();
  let Ok(value) = neg_deinterleave(SegmentRemap { difference, limit }, predicted);

  match (syntax.decoder.strictness >= Strictness::Strict && value > last.max(0).cast_unsigned())
    .then_some(())
  {
    Some(()) => return Err(()),
    None => {}
  }

  let id = match (last >= 0 && value > last.cast_unsigned()).then_some(()) {
    Some(()) => 0,
    None => value,
  };
  let Ok(segment) = SegmentId::new(id);
  let Ok(minimum) = SegmentId::min();
  Ok(match segment {
    Some(decoded) => decoded,
    None => minimum,
  })
}

fn read_updated_segment<'frame>(
  syntax: &BlockSyntaxContext<'frame>,
  task: &mut Rav1dTaskContext,
  block: &mut Av1Block,
  tile: &mut Rav1dTileStateContext,
  coding: SegmentSyntax,
) -> Result<SegmentHeader<'frame>, ()> {
  let temporal = match coding {
    SegmentSyntax::ReadDelta => {
      let bit = read_temporal_segment_bit(syntax, task, tile)?;
      bit
    }
    SegmentSyntax::SpatialPrediction => EntropyBit::Zero,
  };
  let (id, origin) = match temporal {
    EntropyBit::One => {
      let previous = previous_block_segment(syntax, task.b)?;
      (previous, SegmentOrigin::Inherited)
    }
    EntropyBit::Zero => {
      let map = syntax.frame.cur_segmap.as_ref().ok_or(())?;
      let pitch = u64::from(syntax.frame.b4_stride);
      let (predicted, context) = get_cur_frame_segid(task.b, syntax.neighbors, &map.inner, pitch)
        .map_err(|_invalid_segment_map| ())?;
      let segment = match coding {
        SegmentSyntax::ReadDelta => {
          let decoded = read_segment_residual(syntax, tile, predicted, context)?;
          decoded
        }
        SegmentSyntax::SpatialPrediction => predicted,
      };
      (segment, SegmentOrigin::Signalled)
    }
  };
  block.seg_id = id;
  let features = segment_features(syntax.frame_header, id)?;
  Ok(SegmentHeader {
    features: Some(features),
    origin,
  })
}

fn read_before_skip<'frame>(
  syntax: &BlockSyntaxContext<'frame>,
  task: &mut Rav1dTaskContext,
  block: &mut Av1Block,
  tile: &mut Rav1dTileStateContext,
) -> Result<SegmentHeader<'frame>, ()> {
  let segmentation = &syntax.frame_header.segmentation;
  let absent = SegmentHeader {
    features: None,
    origin: SegmentOrigin::Signalled,
  };

  match (
    segmentation.enabled,
    segmentation.update_map,
    segmentation.seg_data.preskip,
  ) {
    (0, _, _) => {
      block.seg_id = Default::default();
      Ok(absent)
    }
    (_, 0, _) => {
      let previous = previous_block_segment(syntax, task.b)?;
      block.seg_id = previous;
      let features = segment_features(syntax.frame_header, previous)?;
      Ok(SegmentHeader {
        features: Some(features),
        origin: SegmentOrigin::Signalled,
      })
    }
    (_, _, 0) => Ok(absent),
    (_, _, _) => {
      let header = read_updated_segment(syntax, task, block, tile, SegmentSyntax::ReadDelta)?;
      let Ok(trace) = debug_block_info(syntax.frame, task.b);

      match trace {
        BlockTrace::Enabled => {
          println!("Post-segid[preskip;{}]: r={}", block.seg_id, tile.msac.rng)
        }
        BlockTrace::Disabled => {}
      }

      Ok(header)
    }
  }
}

fn read_after_skip<'frame>(
  syntax: &BlockSyntaxContext<'frame>,
  task: &mut Rav1dTaskContext,
  block: &mut Av1Block,
  tile: &mut Rav1dTileStateContext,
  before: SegmentHeader<'frame>,
) -> Result<SegmentHeader<'frame>, ()> {
  let segmentation = &syntax.frame_header.segmentation;

  match (
    segmentation.enabled,
    segmentation.update_map,
    segmentation.seg_data.preskip,
  ) {
    (0, _, _) | (_, 0, _) | (_, _, 1..) => return Ok(before),
    (_, _, 0) => {}
  }

  let coding = match block.skip {
    0 => SegmentSyntax::ReadDelta,
    _ => SegmentSyntax::SpatialPrediction,
  };
  let header = read_updated_segment(syntax, task, block, tile, coding)?;
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match trace {
    BlockTrace::Enabled => println!("Post-segid[postskip;{}]: r={}", block.seg_id, tile.msac.rng),
    BlockTrace::Disabled => {}
  }

  Ok(header)
}

fn read_prediction_form(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  block: &Av1Block,
  tile: &mut Rav1dTileStateContext,
  segment: Option<&Rav1dSegmentationData>,
) -> Result<BlockPredictionForm, ()> {
  let header = syntax.frame_header;

  match block.skip_mode {
    0 => {}
    _ => {
      return Ok(match header.frame_type.is_key_or_intra().then_some(()) {
        Some(()) => BlockPredictionForm::IntraCopy,
        None => BlockPredictionForm::Inter,
      });
    }
  }

  match header.frame_type.is_inter_or_switch().then_some(()) {
    Some(()) => read_inter_or_intra(syntax, task, tile, segment),
    None => {
      match header.allow_intrabc.then_some(()) {
        Some(()) => {}
        None => return Ok(BlockPredictionForm::Intra),
      }

      let Ok(copy) = rav1d_msac_decode_bool_adapt(&mut tile.msac, &mut *tile.cdf.m.intrabc);
      let Ok(trace) = debug_block_info(syntax.frame, task.b);

      match trace {
        BlockTrace::Enabled => println!(
          "Post-intrabcflag[{}]: r={}",
          copy == EntropyBit::Zero,
          tile.msac.rng,
        ),
        BlockTrace::Disabled => {}
      }

      Ok(match copy {
        EntropyBit::Zero => BlockPredictionForm::Intra,
        EntropyBit::One => BlockPredictionForm::IntraCopy,
      })
    }
  }
}

fn read_inter_or_intra(
  syntax: &BlockSyntaxContext<'_>,
  task: &Rav1dTaskContext,
  tile: &mut Rav1dTileStateContext,
  segment: Option<&Rav1dSegmentationData>,
) -> Result<BlockPredictionForm, ()> {
  match segment.filter(|features| features.r#ref >= 0 || features.globalmv != 0) {
    Some(features) => {
      return Ok(match features.r#ref {
        0 => BlockPredictionForm::Intra,
        _ => BlockPredictionForm::Inter,
      });
    }
    None => {}
  }

  let Point { x, y } = syntax.geometry.luma.origin;
  let context = get_intra_ctx(BlockNeighbors {
    above: syntax.above,
    left: &task.l,
    location: Bxy { x, y },
    availability: syntax.neighbors,
  })
  .map_err(|_invalid_context| ())?;
  let inter = reference_bit(&mut tile.msac, &mut *tile.cdf.mi.intra, context)?;
  let Ok(trace) = debug_block_info(syntax.frame, task.b);

  match trace {
    BlockTrace::Enabled => println!(
      "Post-intra[{}]: r={}",
      inter == EntropyBit::Zero,
      tile.msac.rng,
    ),
    BlockTrace::Disabled => {}
  }

  Ok(match inter {
    EntropyBit::Zero => BlockPredictionForm::Intra,
    EntropyBit::One => BlockPredictionForm::Inter,
  })
}

fn read_block_prefix<'frame>(
  syntax: &BlockSyntaxContext<'frame>,
  t: &mut Rav1dTaskContext,
  b: &mut Av1Block,
  tile: &mut Rav1dTileStateContext,
) -> Result<BlockSyntaxPrefix<'frame>, ()> {
  let before = read_before_skip(syntax, t, b, tile)?;
  read_block_skip_modes(syntax, t, b, tile, before.features)?;
  let decoded = read_after_skip(syntax, t, b, tile, before)?;

  match b.skip {
    0 => {
      read_block_cdef(syntax.frame, t, &mut tile.msac, syntax.geometry.luma.size)
        .map_err(|_invalid_cdef| ())?;
    }
    _ => {}
  }

  read_block_delta(syntax, t, b, tile)?;
  let prediction = read_prediction_form(syntax, t, b, tile, decoded.features)?;
  Ok(BlockSyntaxPrefix {
    prediction,
    segment: decoded.features,
    segment_origin: decoded.origin,
  })
}

#[derive(Clone, Copy)]
struct TrackedChroma {
  dimensions: Size<i32>,
  vertical_subsampling: i32,
}

struct ReferenceTrackingGeometry<'a> {
  dimensions: &'a [u8; 4],
  visible: Size<i32>,
  chroma: Option<TrackedChroma>,
}

struct ReferenceTracking<'a> {
  frame: &'a Rav1dFrameData,
  header: &'a Rav1dFrameHeader,
  task: &'a Rav1dTaskContext,
  inter: &'a Av1BlockInter,
  geometry: ReferenceTrackingGeometry<'a>,
}

#[derive(Clone, Copy)]
enum TrackedPlane {
  Luma,
  Chroma(TrackedChroma),
}

enum ReferenceProjection {
  Translated,
  Warped,
}

impl ReferenceTracking<'_> {
  fn affine_motion(
    &self,
    reference: u8,
    count: ReferenceArity,
    plane: TrackedPlane,
  ) -> Rav1dResult<Option<&Rav1dWarpedMotionParams>> {
    let &[width, height, ..] = self.geometry.dimensions;
    let allowed_size = match (count, plane) {
      (ReferenceArity::Single, TrackedPlane::Luma) => width.min(height) > 1,
      (ReferenceArity::Compound, TrackedPlane::Luma) => true,
      (_, TrackedPlane::Chroma(chroma)) => {
        chroma.dimensions.width.min(chroma.dimensions.height) > 1
      }
    };

    match allowed_size.then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    let global_mode = match count {
      ReferenceArity::Single => GLOBALMV,
      ReferenceArity::Compound => GLOBALMV_GLOBALMV,
    };
    let Ok(index) = console_core_number_conversion::index(reference);
    let global_warp = match (self.inter.inter_mode == global_mode).then_some(()) {
      Some(()) => {
        let &allowed = self.frame.gmv_warp_allowed.get(index).ok_or(EINVAL)?;
        allowed != 0
      }
      None => false,
    };
    let local_warp = matches!(count, ReferenceArity::Single)
      && self.inter.motion_mode == MotionMode::Warp
      && self.task.warpmv.r#type > Rav1dWarpedMotionType::Translation;

    match (global_warp || local_warp).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    match (count, self.inter.motion_mode) {
      (ReferenceArity::Single, MotionMode::Warp) => Ok(Some(&self.task.warpmv)),
      (ReferenceArity::Single, MotionMode::Translation | MotionMode::Obmc)
      | (ReferenceArity::Compound, _) => {
        let global = self.header.gmv.get(index).ok_or(EINVAL)?;
        Ok(Some(global))
      }
    }
  }

  fn translated_extent(
    &self,
    count: ReferenceArity,
    plane: TrackedPlane,
  ) -> Result<MotionVerticalExtent, console_core_never::Never> {
    let &[_, luma_height, ..] = self.geometry.dimensions;
    let height = i32::from(luma_height);
    let start = self.task.b.y;
    Ok(match (count, plane) {
      (_, TrackedPlane::Luma) => MotionVerticalExtent {
        start,
        height,
        subsampling: 0,
      },
      (ReferenceArity::Compound, TrackedPlane::Chroma(chroma)) => MotionVerticalExtent {
        start,
        height,
        subsampling: chroma.vertical_subsampling,
      },
      (ReferenceArity::Single, TrackedPlane::Chroma(chroma)) => {
        let subsampling = chroma.vertical_subsampling;
        MotionVerticalExtent {
          start: start & !subsampling,
          height: height.wrapping_shl(u32::from(height == subsampling)),
          subsampling,
        }
      }
    })
  }

  fn record(
    &self,
    lowest: &mut [[c_int; 2]; 7],
    reference: i8,
    motion: Mv,
    count: ReferenceArity,
    plane: TrackedPlane,
  ) -> Rav1dResult<ReferenceProjection> {
    let ordinal = u8::try_from(reference).map_err(|_negative_reference| EINVAL)?;
    let Ok(index) = console_core_number_conversion::index(ordinal);
    let [luma, chroma] = lowest.get_mut(index).ok_or(EINVAL)?;
    let affine = self.affine_motion(ordinal, count, plane)?;

    match (affine, plane) {
      (Some(parameters), TrackedPlane::Luma) => {
        affine_lowest_px_luma(self.task, luma, self.geometry.dimensions, parameters)?;
        Ok(ReferenceProjection::Warped)
      }
      (Some(parameters), TrackedPlane::Chroma(_)) => {
        affine_lowest_px_chroma(
          self.task,
          self.frame.cur.p.layout,
          chroma,
          self.geometry.dimensions,
          parameters,
        )?;
        Ok(ReferenceProjection::Warped)
      }
      (None, _) => {
        let [_, scale] = self.frame.svc.get(index).ok_or(EINVAL)?;
        let dst = match plane {
          TrackedPlane::Luma => luma,
          TrackedPlane::Chroma(_) => chroma,
        };
        let Ok(extent) = self.translated_extent(count, plane);
        let Ok(()) = mc_lowest_px(dst, extent, motion.y, scale);
        Ok(ReferenceProjection::Translated)
      }
    }
  }

  fn track_overlap(
    &self,
    lowest: &mut [[c_int; 2]; 7],
    projection: ReferenceProjection,
    plane: OverlapPlane,
  ) -> Rav1dResult {
    match (projection, self.inter.motion_mode) {
      (ReferenceProjection::Translated, MotionMode::Obmc) => {
        let region = OverlapBlock {
          visible: self.geometry.visible,
          dimensions: *self.geometry.dimensions,
          plane,
        };
        obmc_lowest_px(self.frame, self.task, lowest, region)
      }
      (ReferenceProjection::Warped, _)
      | (ReferenceProjection::Translated, MotionMode::Translation | MotionMode::Warp) => Ok(()),
    }
  }

  fn track_shared_chroma(
    &self,
    lowest: &mut [[c_int; 2]; 7],
    prediction: ChromaNeighborPrediction,
    chroma: TrackedChroma,
  ) -> Rav1dResult {
    let Ok(index) = console_core_number_conversion::index(prediction.reference);
    let [_, destination] = lowest.get_mut(index).ok_or(EINVAL)?;
    let [_, scale] = self.frame.svc.get(index).ok_or(EINVAL)?;
    let &[_, height, ..] = self.geometry.dimensions;
    let extent = MotionVerticalExtent {
      start: prediction.position.y,
      height: i32::from(height),
      subsampling: chroma.vertical_subsampling,
    };
    let Ok(()) = mc_lowest_px(destination, extent, prediction.vector.y, scale);
    Ok(())
  }

  fn track_single_chroma(
    &self,
    lowest: &mut [[c_int; 2]; 7],
    reference: i8,
    motion: Mv,
    chroma: TrackedChroma,
  ) -> Rav1dResult {
    let &[width, height, ..] = self.geometry.dimensions;
    let luma = Size {
      width: i32::from(width),
      height: i32::from(height),
    };
    let shared = self.frame.chroma_motion_neighbors(self.task, luma)?;

    match shared {
      Some(neighbors) => {
        for neighbor in [neighbors.diagonal, neighbors.left, neighbors.above]
          .into_iter()
          .flatten()
        {
          self.track_shared_chroma(lowest, neighbor, chroma)?;
        }

        let ordinal = u8::try_from(reference).map_err(|_negative_reference| EINVAL)?;
        let local = ChromaNeighborPrediction {
          position: self.task.b,
          vector: motion,
          reference: ordinal,
        };
        self.track_shared_chroma(lowest, local, chroma)
      }
      None => {
        let plane = TrackedPlane::Chroma(chroma);
        let projection = self.record(lowest, reference, motion, ReferenceArity::Single, plane)?;
        self.track_overlap(lowest, projection, OverlapPlane::Chroma)
      }
    }
  }

  fn update(&self, lowest: &mut [[c_int; 2]; 7]) -> Rav1dResult {
    let vectors = self.inter.nd.one_d.mv;
    let references = self.inter.r#ref;

    match self.inter.comp_type {
      None => {
        let [reference, _] = references;
        let [motion, _] = vectors;
        let projection = self.record(
          lowest,
          reference,
          motion,
          ReferenceArity::Single,
          TrackedPlane::Luma,
        )?;
        self.track_overlap(lowest, projection, OverlapPlane::Luma)?;

        match self.geometry.chroma {
          Some(chroma) => self.track_single_chroma(lowest, reference, motion, chroma),
          None => Ok(()),
        }
      }
      Some(_) => {
        for (reference, motion) in references.into_iter().zip(vectors) {
          let _projection = self.record(
            lowest,
            reference,
            motion,
            ReferenceArity::Compound,
            TrackedPlane::Luma,
          )?;
        }

        match self.geometry.chroma {
          Some(chroma) => {
            for (reference, motion) in references.into_iter().zip(vectors) {
              let _projection = self.record(
                lowest,
                reference,
                motion,
                ReferenceArity::Compound,
                TrackedPlane::Chroma(chroma),
              )?;
            }
          }
          None => {}
        }

        Ok(())
      }
    }
  }
}

fn record_block_reference_rows(
  tracking: ReferenceTracking<'_>,
  tile: &Rav1dTileState,
) -> Rav1dResult {
  let shift =
    u32::try_from(tracking.frame.sb_shift).map_err(|_invalid_superblock_shift| EINVAL)?;
  let row = tracking
    .task
    .b
    .y
    .wrapping_sub(tile.tiling.row_start)
    .wrapping_shr(shift);
  let position = tracking.frame.lowest_pixel_position(tile, row)?;
  let Ok(index) = console_core_number_conversion::index(position);
  let mut lowest = tracking.frame.lowest_pixel_mem.index_mut(index);
  tracking.update(&mut lowest)
}

fn record_nonskipped_block(
  frame: &Rav1dFrameData,
  task: &Rav1dTaskContext,
  rectangle: Rectangle<i32>,
) -> Rav1dResult {
  let x = u32::try_from(rectangle.origin.x).map_err(|_outside_mask| EINVAL)?;
  let y = u32::try_from(rectangle.origin.y).map_err(|_outside_mask| EINVAL)?;
  let width = u32::try_from(rectangle.size.width).map_err(|_outside_mask| EINVAL)?;
  let height = u32::try_from(rectangle.size.height).map_err(|_outside_mask| EINVAL)?;

  match width {
    1..=32 => {}
    0 | 33.. => return Err(EINVAL),
  }

  let start = y.wrapping_shr(1);
  let end = start.checked_add(height.div_ceil(2)).ok_or(EINVAL)?;
  let Ok(range) = console_core_number_conversion::indices(start..end);
  let filter = frame.loop_filter_mask(task.lf_mask)?;
  let rows = filter.noskip_mask.get(range).ok_or(EINVAL)?;
  let bits = u32::MAX
    .wrapping_shr(32_u32.wrapping_sub(width))
    .wrapping_shl(x & 15);
  let Ok(mask) = console_core_number_conversion::wrapped::<_, u16>(bits);

  for [left, right] in rows {
    let column = match x & 16 {
      0 => left,
      _ => right,
    };
    let Ok(()) = column.update(|previous| previous | mask);

    match width {
      32 => {
        let Ok(()) = right.update(|previous| previous | mask);
      }
      _ => {}
    }
  }

  Ok(())
}

fn record_decoded_block_references(
  frame: &Rav1dFrameData,
  task: &Rav1dTaskContext,
  tile: &Rav1dTileState,
  header: &Rav1dFrameHeader,
  block: &Av1Block,
  geometry: BlockDecodeGeometry,
) -> Rav1dResult {
  let track =
    (task.frame_thread.pass == 1 && header.frame_type.is_inter_or_switch()).then_some(());
  let inter = match (&block.ii, track) {
    (Av1BlockIntraInter::Inter(inter), Some(())) => inter,
    (Av1BlockIntraInter::Intra(_), _) | (Av1BlockIntraInter::Inter(_), None) => return Ok(()),
  };
  let chroma = geometry.has_chroma.then_some(TrackedChroma {
    dimensions: geometry.chroma.size,
    vertical_subsampling: geometry.subsampling.y,
  });
  let tracking = ReferenceTracking {
    frame,
    header,
    task,
    inter,
    geometry: ReferenceTrackingGeometry {
      dimensions: geometry.dimensions,
      visible: geometry.visible_luma,
      chroma,
    },
  };
  record_block_reference_rows(tracking, tile)
}

fn decode_b(
  c: &Rav1dContext,
  t: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
  pass: &mut FrameThreadPassState,
  bl: BlockLevel,
  bs: BlockSize,
  bp: BlockPartition,
  intra_edge_flags: EdgeFlags,
) -> Result<(), ()> {
  let seq_hdr = f.seq_hdr().map_err(|_missing_header| ())?;
  let mut b_mem = Av1Block::default();
  let b = match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
    ::core::option::Option::Some(()) => {
      let block = f.coded_block_position(t.b).map_err(|_invalid_block| ())?;
      let Ok(index) = console_core_number_conversion::index(block);
      &mut *f.frame_thread.b.index_mut(index)
    }
    ::core::option::Option::None => &mut b_mem,
  };

  let ts = f.tile(t.tile_index).map_err(|_invalid_tile| ())?;
  let ta = f
    .above_context(t.above_index)
    .map_err(|_invalid_context| ())?;
  let Ok(geometry) = BlockDecodeGeometry::new(t, f, bs);
  let have_left = t.b.x > ts.tiling.col_start;
  let have_top = t.b.y > ts.tiling.row_start;
  let neighbors = NeighborAvailability::from((have_top, have_left));
  let header = f.frame_hdr().map_err(|_missing_header| ())?;
  let frame_type = header.frame_type;

  let ts_c = match pass {
    FrameThreadPassState::First(first_pass) => first_pass,
    FrameThreadPassState::Second => {
      let replay = ReconstructionBlock {
        size: bs,
        edge_flags: intra_edge_flags,
        frame_type,
        luma: geometry.luma,
        chroma: geometry.has_chroma.then_some(geometry.chroma),
      };
      return reconstruct_decoded_block(f, t, ta, b, replay);
    }
  };

  let ts_c_2 = &mut **ts_c;

  b.bl = bl;
  b.bp = bp;
  let Ok(block_code) = bs.ordinal();
  b.bs = block_code;

  let mut syntax = BlockSyntaxContext {
    decoder: c,
    frame: f,
    frame_header: header,
    sequence: seq_hdr,
    tile: ts,
    above: ta,
    geometry,
    block: bs,
    edges: intra_edge_flags,
    neighbors,
    segment: None,
    segment_origin: SegmentOrigin::Signalled,
  };
  let prefix = read_block_prefix(&syntax, t, b, ts_c_2)?;
  syntax.segment = prefix.segment;
  syntax.segment_origin = prefix.segment_origin;

  let decoded = match prefix.prediction {
    BlockPredictionForm::Intra => decode_intra_syntax(&syntax, t, b, ts_c_2),
    BlockPredictionForm::IntraCopy => decode_intrabc_syntax(&syntax, t, b, ts_c_2),
    BlockPredictionForm::Inter => decode_inter_syntax(&syntax, t, b, ts_c_2),
  };
  decoded?;

  match (header.segmentation.enabled != 0 && header.segmentation.update_map != 0).then_some(()) {
    Some(()) => {
      let block = Rectangle {
        origin: Point { x: t.b.x, y: t.b.y },
        size: geometry.luma.size,
      };
      write_block_segments(f, block, b.seg_id).map_err(|_invalid_segment_map| ())?;
    }
    None => {}
  }

  match b.skip {
    0 => record_nonskipped_block(f, t, geometry.luma).map_err(|_invalid_mask| ())?,
    _ => {}
  }

  record_decoded_block_references(f, t, ts, header, b, geometry)
    .map_err(|_invalid_reference_footprint| ())
}

enum FrameThreadPassState<'a> {
  First(&'a mut Rav1dTileStateContext),
  Second,
}

#[derive(Clone, Copy)]
struct PartitionCursor {
  origin: Bxy,
  level: BlockLevel,
  edge: EdgeIndex,
}

enum PartitionCoverage {
  TopLeft,
  TopRow,
  LeftColumn,
  AllQuadrants,
}

struct SplitPartition {
  origin: Bxy,
  child_level: BlockLevel,
  step: i32,
  edges: [EdgeIndex; 4],
  coverage: PartitionCoverage,
}

impl SplitPartition {
  fn children(self) -> Result<[Option<PartitionCursor>; 4], console_core_never::Never> {
    let [top_left, top_right, bottom_left, bottom_right] = self.edges;
    let Bxy { x, y } = self.origin;
    let right = x.wrapping_add(self.step);
    let bottom = y.wrapping_add(self.step);
    let children = match self.coverage {
      PartitionCoverage::TopLeft => [Some((x, y, top_left)), None, None, None],
      PartitionCoverage::TopRow => [
        Some((x, y, top_left)),
        Some((right, y, top_right)),
        None,
        None,
      ],
      PartitionCoverage::LeftColumn => [
        Some((x, y, top_left)),
        Some((x, bottom, bottom_left)),
        None,
        None,
      ],
      PartitionCoverage::AllQuadrants => [
        Some((x, y, top_left)),
        Some((right, y, top_right)),
        Some((x, bottom, bottom_left)),
        Some((right, bottom, bottom_right)),
      ],
    };
    Ok(children.map(|child| {
      child.map(|(child_x, child_y, edge)| PartitionCursor {
        origin: Bxy {
          x: child_x,
          y: child_y,
        },
        level: self.child_level,
        edge,
      })
    }))
  }
}

enum PartitionStep {
  Complete,
  Split(SplitPartition),
}

const PARTITION_SPLIT_COUNT: u8 = 4;
const PARTITION_STACK_CAPACITY: u8 = 1u8.wrapping_add(3u8.wrapping_mul(PARTITION_SPLIT_COUNT));
const PARTITION_NODE_LIMIT: u32 = 1u32
  .wrapping_add(4)
  .wrapping_add(16)
  .wrapping_add(64)
  .wrapping_add(256);

fn decode_sb(
  c: &Rav1dContext,
  t: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
  pass: &mut FrameThreadPassState,
  level: BlockLevel,
  edge: EdgeIndex,
) -> Result<(), ()> {
  let origin = t.b;
  let root = PartitionCursor {
    origin,
    level,
    edge,
  };
  let mut stack = [None;
    match console_core_number_conversion::const_u8_index(PARTITION_STACK_CAPACITY) {
      Ok(capacity) => capacity,
    }];
  let [first, ..] = &mut stack;
  *first = Some(root);
  let mut size = 1u8;

  for _node in 0..PARTITION_NODE_LIMIT {
    let last = match size.checked_sub(1) {
      Some(last) => last,
      None => break,
    };
    let Ok(slot) = console_core_number_conversion::index(last);
    let entry = stack.get_mut(slot).ok_or(())?;
    let next = entry.take().ok_or(())?;
    size = last;
    t.b = next.origin;
    let step = decode_partition(c, t, f, pass, next)?;

    match step {
      PartitionStep::Complete => {}
      PartitionStep::Split(branch) => {
        let Ok(children) = branch.children();

        for child in children.into_iter().rev().flatten() {
          let Ok(child_slot) = console_core_number_conversion::index(size);
          let sibling = stack.get_mut(child_slot).ok_or(())?;
          *sibling = Some(child);
          size = size.wrapping_add(1);
        }
      }
    }
  }

  match size {
    0 => {
      t.b = origin;
      Ok(())
    }
    _ => Err(()),
  }
}

fn partition_cdf(
  cdf: &mut crate::src::cdf::CdfContext,
  level: BlockLevel,
  context: u8,
) -> Result<&mut [u16; 16], ()> {
  let Ok(ordinal) = level.ordinal();
  let Ok(level_index) = console_core_number_conversion::index(ordinal);
  let Ok(context_index) = console_core_number_conversion::index(context);
  let row = cdf.m.partition.get_mut(level_index).ok_or(())?;
  row.get_mut(context_index).ok_or(())
}

fn partition_block_sizes(
  level: BlockLevel,
  partition: BlockPartition,
) -> Result<[BlockSize; 2], ()> {
  let Ok(level_number) = level.ordinal();
  let Ok(partition_number) = partition.ordinal();
  let Ok(level_index) = console_core_number_conversion::index(level_number);
  let Ok(partition_index) = console_core_number_conversion::index(partition_number);
  let row = dav1d_block_sizes.get(level_index).ok_or(())?;
  row.get(partition_index).copied().ok_or(())
}

struct PartitionLayout {
  cursor: PartitionCursor,
  partition: BlockPartition,
  superblock: SuperblockSize,
  half: i32,
}

fn align_subblock_coefficients(tile: &Rav1dTileState, task: &Rav1dTaskContext) -> Result<(), ()> {
  match (cfg!(target_arch = "x86_64") && task.frame_thread.pass != 0).then_some(()) {
    Some(()) => {}
    None => return Ok(()),
  }

  let Ok(pass) = console_core_number_conversion::wrapped_index(task.frame_thread.pass & 1);
  let storage = tile.frame_thread.get(pass).ok_or(())?;
  let Ok(()) = storage.cf.update(|count| count.wrapping_add(31) & !31);
  Ok(())
}

fn decode_partition_layout(
  c: &Rav1dContext,
  t: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
  pass: &mut FrameThreadPassState,
  layout: PartitionLayout,
  tile: &Rav1dTileState,
) -> Result<PartitionStep, ()> {
  let PartitionLayout {
    cursor: PartitionCursor {
      origin,
      level: bl,
      edge: edge_index,
    },
    partition: bp,
    superblock: superblock_size,
    half: hsz,
  } = layout;
  let intra_edge = &IntraEdges::DEFAULT;
  let [primary_size, secondary_size] = partition_block_sizes(bl, bp)?;

  match bp {
    BlockPartition::None => {
      let node = intra_edge
        .node(superblock_size, edge_index)
        .map_err(|_edge| ())?;
      decode_b(c, t, f, pass, bl, primary_size, bp, node.o)?;
    }
    BlockPartition::H => {
      let node = intra_edge
        .node(superblock_size, edge_index)
        .map_err(|_edge| ())?;
      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        match node.h {
          [_explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        match node.h {
          [_, _explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
      };
    }
    BlockPartition::V => {
      let node = intra_edge
        .node(superblock_size, edge_index)
        .map_err(|_edge| ())?;
      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        match node.v {
          [_explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        match node.v {
          [_, _explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
      };
    }
    BlockPartition::Split => {
      let Ok(decrease) = bl.decrease();

      match decrease {
        None => {
          let tip = intra_edge
            .tip(superblock_size, edge_index)
            .map_err(|_edge| ())?;
          assert!(hsz == 1);
          decode_b(
            c,
            t,
            f,
            pass,
            bl,
            BlockSize::Bs4x4,
            bp,
            EdgeFlags::ALL_TR_AND_BL,
          )?;
          let tl_filter = t.tl_4x4_filter;

          {
            let numeric_amount = 1;
            let numeric_target = &mut (t.b.x);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };

          decode_b(
            c,
            t,
            f,
            pass,
            bl,
            BlockSize::Bs4x4,
            bp,
            match tip.split {
              [_explicit_element, ..] => _explicit_element,
            },
          )?;

          {
            let numeric_amount = 1;
            let numeric_target = &mut (t.b.x);
            *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
          };

          {
            let numeric_amount = 1;
            let numeric_target = &mut (t.b.y);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };

          decode_b(
            c,
            t,
            f,
            pass,
            bl,
            BlockSize::Bs4x4,
            bp,
            match tip.split {
              [_, _explicit_element, ..] => _explicit_element,
            },
          )?;

          {
            let numeric_amount = 1;
            let numeric_target = &mut (t.b.x);
            *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
          };

          t.tl_4x4_filter = tl_filter;
          decode_b(
            c,
            t,
            f,
            pass,
            bl,
            BlockSize::Bs4x4,
            bp,
            match tip.split {
              [_, _, _explicit_element, ..] => _explicit_element,
            },
          )?;

          {
            let numeric_amount = 1;
            let numeric_target = &mut (t.b.x);
            *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
          };

          {
            let numeric_amount = 1;
            let numeric_target = &mut (t.b.y);
            *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
          };

          align_subblock_coefficients(tile, t)?;
        }
        Some(next_bl) => {
          let branch = intra_edge
            .branch(superblock_size, edge_index)
            .map_err(|_edge| ())?;
          return Ok(PartitionStep::Split(SplitPartition {
            origin,
            child_level: next_bl,
            step: hsz,
            edges: branch.split,
            coverage: PartitionCoverage::AllQuadrants,
          }));
        }
      }
    }
    BlockPartition::TopSplit => {
      let node = intra_edge
        .node(superblock_size, edge_index)
        .map_err(|_edge| ())?;
      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        EdgeFlags::ALL_TR_AND_BL,
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        match node.v {
          [_, _explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
      };

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        secondary_size,
        bp,
        match node.h {
          [_, _explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
      };
    }
    BlockPartition::BottomSplit => {
      let node = intra_edge
        .node(superblock_size, edge_index)
        .map_err(|_edge| ())?;
      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        match node.h {
          [_explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        secondary_size,
        bp,
        match node.v {
          [_explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(c, t, f, pass, bl, secondary_size, bp, EdgeFlags::EMPTY)?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
      };

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
      };
    }
    BlockPartition::LeftSplit => {
      let node = intra_edge
        .node(superblock_size, edge_index)
        .map_err(|_edge| ())?;
      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        EdgeFlags::ALL_TR_AND_BL,
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        match node.h {
          [_, _explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
      };

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        secondary_size,
        bp,
        match node.v {
          [_, _explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
      };
    }
    BlockPartition::RightSplit => {
      let node = intra_edge
        .node(superblock_size, edge_index)
        .map_err(|_edge| ())?;
      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        match node.v {
          [_explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        secondary_size,
        bp,
        match node.h {
          [_explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(c, t, f, pass, bl, secondary_size, bp, EdgeFlags::EMPTY)?;

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
      };

      {
        let numeric_amount = hsz;
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
      };
    }
    BlockPartition::H4 => {
      let branch = intra_edge
        .branch(superblock_size, edge_index)
        .map_err(|_edge| ())?;
      let node = &branch.node;
      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        match node.h {
          [_explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = i32::wrapping_shr(hsz, 1_u32);
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(c, t, f, pass, bl, primary_size, bp, branch.h4)?;

      {
        let numeric_amount = i32::wrapping_shr(hsz, 1_u32);
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        EdgeFlags::ALL_LEFT_HAS_BOTTOM,
      )?;

      {
        let numeric_amount = i32::wrapping_shr(hsz, 1_u32);
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      match ::core::primitive::bool::then_some(t.b.y < f.bh, ()) {
        ::core::option::Option::Some(()) => {
          decode_b(
            c,
            t,
            f,
            pass,
            bl,
            primary_size,
            bp,
            match node.h {
              [_, _explicit_element, ..] => _explicit_element,
            },
          )?;
        }
        ::core::option::Option::None => {}
      }

      {
        let numeric_amount = i32::wrapping_shr(i32::wrapping_mul(hsz, 3), 1_u32);
        let numeric_target = &mut (t.b.y);
        *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
      };
    }
    BlockPartition::V4 => {
      let branch = intra_edge
        .branch(superblock_size, edge_index)
        .map_err(|_edge| ())?;
      let node = &branch.node;
      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        match node.v {
          [_explicit_element, ..] => _explicit_element,
        },
      )?;

      {
        let numeric_amount = i32::wrapping_shr(hsz, 1_u32);
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(c, t, f, pass, bl, primary_size, bp, branch.v4)?;

      {
        let numeric_amount = i32::wrapping_shr(hsz, 1_u32);
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      decode_b(
        c,
        t,
        f,
        pass,
        bl,
        primary_size,
        bp,
        EdgeFlags::ALL_TOP_HAS_RIGHT,
      )?;

      {
        let numeric_amount = i32::wrapping_shr(hsz, 1_u32);
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };

      match ::core::primitive::bool::then_some(t.b.x < f.bw, ()) {
        ::core::option::Option::Some(()) => {
          decode_b(
            c,
            t,
            f,
            pass,
            bl,
            primary_size,
            bp,
            match node.v {
              [_, _explicit_element, ..] => _explicit_element,
            },
          )?;
        }
        ::core::option::Option::None => {}
      }

      {
        let numeric_amount = i32::wrapping_shr(i32::wrapping_mul(hsz, 3), 1_u32);
        let numeric_target = &mut (t.b.x);
        *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
      };
    }
  }

  Ok(PartitionStep::Complete)
}

enum BoundaryDecision {
  Split,
  Unsplit,
}

struct PartitionBoundary {
  level: BlockLevel,
  edge: PartitionEdge,
  frame_offset: u8,
}

fn read_partition_boundary(
  frame: &Rav1dFrameData,
  task: &Rav1dTaskContext,
  input: Option<(u8, &mut Rav1dTileStateContext)>,
  boundary: PartitionBoundary,
) -> Result<BoundaryDecision, ()> {
  let (context, entropy) = match input {
    Some(source) => source,
    None => {
      let position = frame
        .coded_block_position(task.b)
        .map_err(|_invalid_block| ())?;
      let Ok(index) = console_core_number_conversion::index(position);
      let block = frame.frame_thread.b.index(index);

      return Ok(match (block.bl != boundary.level).then_some(()) {
        Some(()) => BoundaryDecision::Split,
        None => BoundaryDecision::Unsplit,
      });
    }
  };

  let cdf = partition_cdf(&mut entropy.cdf, boundary.level, context)?;
  let Ok(probability) = gather_partition_probability(cdf, boundary.level, boundary.edge);
  let Ok(bit) = rav1d_msac_decode_bool(&mut entropy.msac, probability);
  let position = match (bit, boundary.edge, frame.cur.p.layout) {
    (EntropyBit::One, _, _) => BlockPartition::Split,
    (EntropyBit::Zero, PartitionEdge::Top, _) => BlockPartition::H,
    (EntropyBit::Zero, PartitionEdge::Left, Rav1dPixelLayout::I422) => return Err(()),
    (
      EntropyBit::Zero,
      PartitionEdge::Left,
      Rav1dPixelLayout::I400 | Rav1dPixelLayout::I420 | Rav1dPixelLayout::I444,
    ) => BlockPartition::V,
  };
  let Ok(trace) = debug_block_info(frame, task.b);

  match trace {
    BlockTrace::Disabled => {}
    BlockTrace::Enabled => {
      println!(
        "poc={},y={},x={},bl={:?},ctx={},bp={:?}: r={}",
        boundary.frame_offset,
        task.b.y,
        task.b.x,
        boundary.level,
        context,
        position,
        entropy.msac.rng,
      );
    }
  }

  Ok(match bit {
    EntropyBit::One => BoundaryDecision::Split,
    EntropyBit::Zero => BoundaryDecision::Unsplit,
  })
}

fn read_partition_symbol(
  frame: &Rav1dFrameData,
  task: &Rav1dTaskContext,
  input: Option<(u8, &mut Rav1dTileStateContext)>,
  level: BlockLevel,
  frame_offset: u8,
) -> Result<BlockPartition, ()> {
  let (context, entropy) = match input {
    Some(source) => source,
    None => {
      let position = frame
        .coded_block_position(task.b)
        .map_err(|_invalid_block| ())?;
      let Ok(index) = console_core_number_conversion::index(position);
      let block = frame.frame_thread.b.index(index);
      return Ok(match (block.bl == level).then_some(()) {
        Some(()) => block.bp,
        None => BlockPartition::Split,
      });
    }
  };

  let cdf = partition_cdf(&mut entropy.cdf, level, context)?;
  let Ok(level_number) = level.ordinal();
  let Ok(level_index) = console_core_number_conversion::index(level_number);
  let &symbols = dav1d_partition_type_count.get(level_index).ok_or(())?;
  let Ok(symbol) = rav1d_msac_decode_symbol_adapt16(&mut entropy.msac, cdf, symbols);
  let Ok(symbol_index) = console_core_number_conversion::wrapped_index(symbol);
  let partition = BlockPartition::from_repr(symbol_index).ok_or(())?;

  let prohibited = frame.cur.p.layout == Rav1dPixelLayout::I422
    && matches!(
      partition,
      BlockPartition::V
        | BlockPartition::V4
        | BlockPartition::LeftSplit
        | BlockPartition::RightSplit
    );

  match prohibited.then_some(()) {
    Some(()) => return Err(()),
    None => {}
  }

  let Ok(trace) = debug_block_info(frame, task.b);

  match trace {
    BlockTrace::Disabled => {}
    BlockTrace::Enabled => {
      println!(
        "poc={},y={},x={},bl={:?},ctx={},bp={:?}: r={}",
        frame_offset, task.b.y, task.b.x, level, context, partition, entropy.msac.rng,
      );
    }
  }

  Ok(partition)
}

fn decode_partition(
  c: &Rav1dContext,
  t: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
  pass: &mut FrameThreadPassState,
  cursor: PartitionCursor,
) -> Result<PartitionStep, ()> {
  let PartitionCursor {
    origin,
    level: bl,
    edge: edge_index,
  } = cursor;
  let ts = &*match f.tile(t.tile_index) {
    Ok(indexed_value) => indexed_value,
    Err(_invalid_slot) => return ::core::result::Result::Err(()),
  };
  let hsz = i32::wrapping_shr(16, {
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>({
      let Ok(ordinal) = bl.ordinal();
      ordinal
    });
    numeric_value
  });
  let have_h_split = f.bw > i32::wrapping_add(t.b.x, hsz);
  let have_v_split = f.bh > i32::wrapping_add(t.b.y, hsz);

  let seq_hdr = f.seq_hdr().map_err(|_missing_header| ())?;
  let superblock_size = match seq_hdr.sb128 {
    0 => SuperblockSize::Pixels64,
    _ => SuperblockSize::Pixels128,
  };
  let intra_edge = &IntraEdges::DEFAULT;

  match ::core::primitive::bool::then_some(!have_h_split && !have_v_split, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(bl_decrease) = bl.decrease();
      let next_bl = bl_decrease.ok_or(())?;

      let intra_edge_branch = intra_edge
        .branch(superblock_size, edge_index)
        .map_err(|_edge| ())?;
      return Ok(PartitionStep::Split(SplitPartition {
        origin,
        child_level: next_bl,
        step: hsz,
        edges: intra_edge_branch.split,
        coverage: PartitionCoverage::TopLeft,
      }));
    }
    ::core::option::Option::None => {}
  }

  let f_frame_hdr = f.frame_hdr.as_ref().ok_or(())?;
  let frame_hdr = &***f_frame_hdr;

  let bp;
  let mut bx8 = 0;
  let mut by8 = 0;
  let ctx = match pass {
    FrameThreadPassState::First(ts_c) => {
      match ::core::primitive::bool::then_some(false && bl == BlockLevel::Bl64x64, ()) {
        ::core::option::Option::Some(()) => {
          println!(
            "poc={},y={},x={},bl={:?},r={}",
            frame_hdr.frame_offset, t.b.y, t.b.x, bl, ts_c.msac.rng,
          );
        }
        ::core::option::Option::None => {}
      }

      bx8 = i32::wrapping_shr(t.b.x & 31, 1_u32);
      by8 = i32::wrapping_shr(t.b.y & 31, 1_u32);

      let ta = &*match f.above_context(t.above_index) {
        Ok(indexed_value) => indexed_value,
        Err(_invalid_slot) => return ::core::result::Result::Err(()),
      };
      let partition_ctx = get_partition_ctx(
        BlockNeighbors {
          above: ta,
          left: &t.l,
          location: Bxy { x: bx8, y: by8 },
          availability: NeighborAvailability::Both,
        },
        bl,
      )
      .map_err(|_invalid_context| ())?;

      Some((partition_ctx, &mut **ts_c))
    }
    FrameThreadPassState::Second => None,
  };

  match ::core::primitive::bool::then_some(have_h_split && have_v_split, ()) {
    ::core::option::Option::Some(()) => {
      let parsed = read_partition_symbol(f, t, ctx, bl, frame_hdr.frame_offset)?;
      bp = parsed;

      let layout = PartitionLayout {
        cursor: PartitionCursor {
          origin,
          level: bl,
          edge: edge_index,
        },
        partition: bp,
        superblock: superblock_size,
        half: hsz,
      };
      let decoded = decode_partition_layout(c, t, f, pass, layout, ts)?;

      match decoded {
        PartitionStep::Complete => {}
        PartitionStep::Split(children) => return Ok(PartitionStep::Split(children)),
      }
    }
    ::core::option::Option::None => match ::core::primitive::bool::then_some(have_h_split, ()) {
      ::core::option::Option::Some(()) => {
        let boundary = read_partition_boundary(
          f,
          t,
          ctx,
          PartitionBoundary {
            level: bl,
            edge: PartitionEdge::Top,
            frame_offset: frame_hdr.frame_offset,
          },
        )?;

        let Ok(bl_decrease) = bl.decrease();
        let next_bl = bl_decrease.ok_or(())?;

        match boundary {
          BoundaryDecision::Split => {
            let branch = intra_edge
              .branch(superblock_size, edge_index)
              .map_err(|_edge| ())?;
            return Ok(PartitionStep::Split(SplitPartition {
              origin,
              child_level: next_bl,
              step: hsz,
              edges: branch.split,
              coverage: PartitionCoverage::TopRow,
            }));
          }
          BoundaryDecision::Unsplit => {
            let node = intra_edge
              .node(superblock_size, edge_index)
              .map_err(|_edge| ())?;
            bp = BlockPartition::H;
            let [primary_size, _] = partition_block_sizes(bl, bp)?;
            let [flags, ..] = node.h;
            decode_b(c, t, f, pass, bl, primary_size, bp, flags)?;
          }
        }
      }
      ::core::option::Option::None => {
        assert!(have_v_split);
        let boundary = read_partition_boundary(
          f,
          t,
          ctx,
          PartitionBoundary {
            level: bl,
            edge: PartitionEdge::Left,
            frame_offset: frame_hdr.frame_offset,
          },
        )?;

        let Ok(bl_decrease) = bl.decrease();
        let next_bl = bl_decrease.ok_or(())?;

        match boundary {
          BoundaryDecision::Split => {
            let branch = intra_edge
              .branch(superblock_size, edge_index)
              .map_err(|_edge| ())?;
            return Ok(PartitionStep::Split(SplitPartition {
              origin,
              child_level: next_bl,
              step: hsz,
              edges: branch.split,
              coverage: PartitionCoverage::LeftColumn,
            }));
          }
          BoundaryDecision::Unsplit => {
            let node = intra_edge
              .node(superblock_size, edge_index)
              .map_err(|_edge| ())?;
            bp = BlockPartition::V;
            let [primary_size, _] = partition_block_sizes(bl, bp)?;
            let [flags, ..] = node.v;
            decode_b(c, t, f, pass, bl, primary_size, bp, flags)?;
          }
        }
      }
    },
  }

  match ::core::primitive::bool::then_some(
    matches!(pass, FrameThreadPassState::First(_))
      && (bp != BlockPartition::Split || bl == BlockLevel::Bl8x8),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let above = f.above_context(t.above_index).map_err(|_invalid_slot| ())?;
      let Ok(level) = bl.ordinal();
      let Ok(partition) = bp.ordinal();
      let Ok(level_index) = console_core_number_conversion::index(level);
      let Ok(partition_index) = console_core_number_conversion::index(partition);
      let [horizontal, vertical] = &dav1d_al_part_ctx;
      let horizontal_level = horizontal.get(level_index).ok_or(())?;
      let vertical_level = vertical.get(level_index).ok_or(())?;
      let horizontal_context = horizontal_level.get(partition_index).copied().ok_or(())?;
      let vertical_context = vertical_level.get(partition_index).copied().ok_or(())?;
      let Ok(run) = console_core_number_conversion::wrapped_index(hsz);
      let Ok(left_start) = console_core_number_conversion::wrapped_index(by8);
      let Ok(above_start) = console_core_number_conversion::wrapped_index(bx8);
      case_set_al! {
          <16, false>
          l: (&mut t.l, run, left_start),
          a: (above, run, above_start),
          partition = (vertical_context, horizontal_context),
      }
    }
    ::core::option::Option::None => {}
  }

  Ok(PartitionStep::Complete)
}

enum ContextInitialization {
  PredictionOnly,
  Full,
}

fn reset_context(
  ctx: &mut BlockContext,
  frame: Rav1dFrameType,
  update: ContextInitialization,
) -> Result<(), console_core_never::Never> {
  ctx.intra.get_mut().fill(u8::from(frame.is_key_or_intra()));
  ctx.uvmode.get_mut().fill(DC_PRED);

  match frame {
    Rav1dFrameType::Key | Rav1dFrameType::Intra => ctx.mode.get_mut().fill(DC_PRED),
    Rav1dFrameType::Inter | Rav1dFrameType::Switch => {}
  }

  match update {
    ContextInitialization::PredictionOnly => return Ok(()),
    ContextInitialization::Full => {}
  }

  ctx.partition.get_mut().fill(0);
  ctx.skip.get_mut().fill(0);
  ctx.skip_mode.get_mut().fill(0);
  ctx.tx_lpf_y.get_mut().fill(2);
  ctx.tx_lpf_uv.get_mut().fill(1);
  ctx.tx_intra.get_mut().fill(-1);
  ctx.tx.get_mut().fill(TxfmSize::S64x64);

  match frame {
    Rav1dFrameType::Key | Rav1dFrameType::Intra => {}
    Rav1dFrameType::Inter | Rav1dFrameType::Switch => {
      for reference in &mut ctx.r#ref {
        reference.get_mut().fill(-1);
      }

      ctx.comp_type.get_mut().fill(None);
      ctx.mode.get_mut().fill(NEARESTMV);
    }
  }

  ctx.lcoef.get_mut().fill(0x40);

  for coefficient in &mut ctx.ccoef {
    coefficient.get_mut().fill(0x40);
  }

  for filter in &mut ctx.filter {
    filter.get_mut().fill(Rav1dFilterMode::N_SWITCHABLE_FILTERS);
  }

  ctx.seg_pred.get_mut().fill(0);
  ctx.pal_sz.get_mut().fill(0);
  Ok(())
}

static ss_size_mul: enum_map_ty!(Rav1dPixelLayout, [u8; 2]) = {
  let monochrome = [4, 4];
  let chroma_420 = [6, 5];
  let chroma_422 = [8, 6];
  let chroma_444 = [12, 8];
  let Ok(multipliers) = EnumMap::new([monochrome, chroma_420, chroma_422, chroma_444]);
  multipliers
};

fn adjacent_tile_boundaries(starts: &[u16], tile: u32) -> Rav1dResult<[i32; 2]> {
  let Ok(index) = console_core_number_conversion::index(tile);
  let &[first, next] = starts
    .get(index..)
    .and_then(<[_]>::first_chunk::<2>)
    .ok_or(EINVAL)?;
  Ok([i32::from(first), i32::from(next)])
}

struct SetupTileArguments {
  bitdepth_max: i32,
  sb_shift: i32,
  frame: Size<i32>,
  superblock_widths: SuperblockWidths,
  tile: Point<u32>,
  palette_start: u32,
}

struct SuperblockWidths {
  scaled: i32,
  coded: i32,
}

struct TileSetupFrame<'a> {
  sequence: &'a Rav1dSequenceHeader,
  header: &'a Rav1dFrameHeader,
  picture: &'a Rav1dPicture,
  buffers: &'a Rav1dFrameContextFrameThread,
  restoration: &'a mut Rav1dFrameContextLf,
}

struct TileEntropyInput<'a> {
  initial_cdf: &'a CdfThreadContext,
  payload: CArc<[u8]>,
}

fn set_tile_buffer_offsets(
  tile: &Rav1dTileState,
  frame: &TileSetupFrame<'_>,
  settings: &SetupTileArguments,
) -> Rav1dResult {
  let &[coefficient_scale, palette_scale] = ss_size_mul.checked(frame.picture.p.layout)?;
  let palette = match frame.buffers.pal_idx.is_empty().then_some(()) {
    Some(()) => 0,
    None => settings
      .palette_start
      .wrapping_mul(u32::from(palette_scale))
      .wrapping_div(8),
  };
  let coding = settings
    .palette_start
    .wrapping_mul(u32::from(coefficient_scale));
  let blocks = match frame.buffers.cbi.is_empty().then_some(()) {
    Some(()) => 0,
    None => coding.wrapping_div(64),
  };
  let coefficients = match frame.buffers.cf.is_empty().then_some(()) {
    Some(()) => 0,
    None => {
      let bpc = BPC::from_bitdepth_max(settings.bitdepth_max);
      bpc.coef_stride(coding.wrapping_shr(u32::from(frame.sequence.hbd == 0)))
    }
  };

  for pass in &tile.frame_thread {
    let Ok(()) = pass.pal_idx.set(palette);
    let Ok(()) = pass.cbi_idx.set(blocks);
    let Ok(()) = pass.cf.set(coefficients);
  }

  Ok(())
}

struct TileSuperblockBoundaries {
  rows: [i32; 2],
  columns: [i32; 2],
}

impl TileSuperblockBoundaries {
  fn from_header(header: &Rav1dFrameHeader, position: Point<u32>) -> Rav1dResult<Self> {
    let columns = adjacent_tile_boundaries(&header.tiling.col_start_sb, position.x)?;
    let rows = adjacent_tile_boundaries(&header.tiling.row_start_sb, position.y)?;
    Ok(Self { rows, columns })
  }

  fn tiling(&self, settings: &SetupTileArguments) -> Rav1dResult<Rav1dTileStateTiling> {
    let col = i32::try_from(settings.tile.x).map_err(|_outside_columns| EINVAL)?;
    let row = i32::try_from(settings.tile.y).map_err(|_outside_rows| EINVAL)?;
    let Ok(shift) = console_core_number_conversion::wrapped::<_, u32>(settings.sb_shift);
    let [first_column, last_column] = self.columns;
    let [first_row, last_row] = self.rows;
    Ok(Rav1dTileStateTiling {
      col,
      row,
      col_start: first_column.wrapping_shl(shift),
      col_end: last_column.wrapping_shl(shift).min(settings.frame.width),
      row_start: first_row.wrapping_shl(shift),
      row_end: last_row.wrapping_shl(shift).min(settings.frame.height),
    })
  }
}

struct TileRestorationPosition {
  superblock: i32,
  quadrant: i32,
}

enum TileRestorationColumns {
  Coded,
  Upscaled { width_blocks: i32, denominator: i32 },
}

struct TileRestorationLayout {
  origin: TileRestorationPosition,
  column_start: i32,
  unit_size: [u8; 2],
  pixel_layout: Rav1dPixelLayout,
  columns: TileRestorationColumns,
}

impl TileRestorationLayout {
  fn position(
    &self,
    plane: u8,
  ) -> Result<Option<TileRestorationPosition>, console_core_never::Never> {
    match self.columns {
      TileRestorationColumns::Coded => Ok(Some(TileRestorationPosition {
        superblock: self.origin.superblock,
        quadrant: self.origin.quadrant,
      })),
      TileRestorationColumns::Upscaled {
        width_blocks,
        denominator,
      } => {
        let horizontal_subsampling =
          u8::from(plane != 0 && self.pixel_layout != Rav1dPixelLayout::I444);
        let [luma, chroma] = self.unit_size;
        let unit = match plane {
          0 => luma,
          _ => chroma,
        };
        let round = 8i32.wrapping_shl(u32::from(unit)).wrapping_sub(1);
        let column = self
          .column_start
          .wrapping_mul(4)
          .wrapping_mul(denominator)
          .wrapping_shr(u32::from(horizontal_subsampling))
          .wrapping_add(round)
          .wrapping_shr(u32::from(unit.wrapping_add(3)));
        let x = column.wrapping_shl(u32::from(unit.wrapping_add(horizontal_subsampling)));
        let block = x.wrapping_shr(7);

        match block.cmp(&width_blocks) {
          cmp::Ordering::Less => Ok(Some(TileRestorationPosition {
            superblock: self.origin.superblock.wrapping_add(block),
            quadrant: self.origin.quadrant.wrapping_add((x & 64).wrapping_shr(6)),
          })),
          cmp::Ordering::Equal | cmp::Ordering::Greater => Ok(None),
        }
      }
    }
  }
}

fn reset_tile_restoration(
  units: &mut [Av1RestorationUnit; 3],
  filter: &mut Rav1dFrameContextLf,
  layout: TileRestorationLayout,
) -> Rav1dResult {
  let planes = filter.restore_planes.bits();

  for (plane, previous) in (0u8..).zip(units) {
    match planes.wrapping_shr(u32::from(plane)) & 1 {
      0 => continue,
      _ => {}
    }

    let Ok(placement) = layout.position(plane);
    let TileRestorationPosition {
      superblock,
      quadrant,
    } = match placement {
      Some(position) => position,
      None => continue,
    };
    let block = console_core_number_conversion::checked_index(superblock)
      .map_err(|_outside_mask| EINVAL)?;
    let index =
      console_core_number_conversion::checked_index(quadrant).map_err(|_outside_unit| EINVAL)?;
    let Ok(plane_index) = console_core_number_conversion::index(plane);
    let mask = filter.lr_mask.get_mut(block).ok_or(EINVAL)?;
    let plane_units = mask.lr.get_mut(plane_index).ok_or(EINVAL)?;
    let reference = plane_units.get_mut(index).ok_or(EINVAL)?;
    let unit = reference.get_mut();
    *unit = Av1RestorationUnit {
      filter_v: [3, -7, 15],
      filter_h: [3, -7, 15],
      sgr_weights: [-32, 31],
      ..*unit
    };
    *previous = *unit;
  }

  Ok(())
}

fn setup_tile(
  decoder: &Rav1dContext,
  tile: &mut Rav1dTileState,
  frame: TileSetupFrame<'_>,
  settings: SetupTileArguments,
  input: TileEntropyInput<'_>,
) -> Rav1dResult {
  let initial_cdf = rav1d_cdf_thread_copy(input.initial_cdf)?;
  let boundaries = TileSuperblockBoundaries::from_header(frame.header, settings.tile)?;
  set_tile_buffer_offsets(tile, &frame, &settings)?;
  let mut context = tile.context.try_lock().ok_or(EINVAL)?;
  context.cdf = initial_cdf;
  tile.last_qidx = RelaxedAtomic::from(frame.header.quant.yac);
  tile.last_delta_lf = Default::default();
  let updates = match frame.header.disable_cdf_update {
    0 => crate::src::msac::CdfUpdates::Allowed,
    _ => crate::src::msac::CdfUpdates::Frozen,
  };
  let Ok(entropy) = MsacContext::new(input.payload, updates, &decoder.dsp.msac);
  context.msac = entropy;
  let tiling = boundaries.tiling(&settings)?;
  tile.tiling = tiling;
  let [coded, restored] = frame.header.size.width;
  let [column_start, _] = boundaries.columns;
  let sb128_row = tile.tiling.row_start.wrapping_shr(5);
  let quadrant_y = (tile.tiling.row_start & 16).wrapping_shr(3);
  let (origin, columns) = match coded.cmp(&restored) {
    cmp::Ordering::Equal => {
      let sb128_col = column_start.wrapping_shr(u32::from(frame.sequence.sb128 == 0));
      let origin = TileRestorationPosition {
        superblock: sb128_row
          .wrapping_mul(settings.superblock_widths.coded)
          .wrapping_add(sb128_col),
        quadrant: quadrant_y.wrapping_add((tile.tiling.col_start & 16).wrapping_shr(4)),
      };
      (origin, TileRestorationColumns::Coded)
    }
    cmp::Ordering::Less | cmp::Ordering::Greater => {
      let width_blocks = settings.superblock_widths.scaled;
      let origin = TileRestorationPosition {
        superblock: sb128_row.wrapping_mul(width_blocks),
        quadrant: quadrant_y,
      };
      let denominator = i32::from(frame.header.size.super_res.width_scale_denominator);
      (
        origin,
        TileRestorationColumns::Upscaled {
          width_blocks,
          denominator,
        },
      )
    }
  };
  let restoration = TileRestorationLayout {
    origin,
    column_start: tile.tiling.col_start,
    unit_size: frame.header.restoration.unit_size,
    pixel_layout: frame.picture.p.layout,
    columns,
  };
  reset_tile_restoration(tile.lr_ref.get_mut(), frame.restoration, restoration)?;

  match decoder.tc.len() {
    0 | 1 => {}
    2.. => {
      let [first_row, _] = boundaries.rows;
      tile.progress.fill_with(|| AtomicI32::new(first_row));
    }
  }

  Ok(())
}

fn read_optional_restoration_type(
  entropy: &mut MsacContext,
  probabilities: &mut [u16; 2],
  enabled: Rav1dRestorationType,
) -> Result<Rav1dRestorationType, console_core_never::Never> {
  let Ok(use_filter) = rav1d_msac_decode_bool_adapt(entropy, probabilities);

  Ok(match use_filter {
    EntropyBit::One => enabled,
    EntropyBit::Zero => Rav1dRestorationType::None,
  })
}

fn read_restoration_info(
  ts: &Rav1dTileState,
  lr: &mut Av1RestorationUnit,
  plane: u8,
  frame_type: Rav1dRestorationType,
  debug_block_info: BlockTrace,
) -> Rav1dResult {
  let Ok(plane_index) = console_core_number_conversion::index(plane);
  let mut context = ts.context.try_lock().ok_or(EINVAL)?;
  let references = ts.lr_ref.try_read().ok_or(EINVAL)?;
  let &reference = references.get(plane_index).ok_or(EINVAL)?;
  drop(references);
  let Rav1dTileStateContext { msac, cdf, .. } = &mut *context;

  lr.r#type = match frame_type {
    Rav1dRestorationType::Switchable => {
      let Ok(filter) = rav1d_msac_decode_symbol_adapt4(msac, &mut *cdf.m.restore_switchable, 2);

      match filter {
        0 => Rav1dRestorationType::None,
        2 => Rav1dRestorationType::SgrProj(SgrIdx::I0),
        _ => Rav1dRestorationType::Wiener,
      }
    }
    Rav1dRestorationType::Wiener => {
      let Ok(selected) =
        read_optional_restoration_type(msac, &mut cdf.m.restore_wiener, frame_type);
      selected
    }
    Rav1dRestorationType::SgrProj(_) => {
      let Ok(selected) =
        read_optional_restoration_type(msac, &mut cdf.m.restore_sgrproj, frame_type);
      selected
    }
    Rav1dRestorationType::None => return Err(EINVAL),
  };

  struct RestorationSubexpression {
    reference: i8,
    order: u8,
    adjustment: i8,
  }

  fn read_subexpression(
    msac: &mut MsacContext,
    RestorationSubexpression {
      reference,
      order,
      adjustment,
    }: RestorationSubexpression,
  ) -> Result<i8, console_core_never::Never> {
    let Ok(base) =
      console_core_number_conversion::wrapped::<_, u32>(reference.wrapping_add(adjustment));
    let Ok(symbol) = rav1d_msac_decode_subexp(
      msac,
      SubexpParameters {
        reference: base,
        order,
      },
    );
    let Ok(value) = console_core_number_conversion::wrapped::<_, i8>(symbol);
    Ok(value.wrapping_sub(adjustment))
  }

  match lr.r#type {
    Rav1dRestorationType::Wiener => {
      for (filter, [outer_reference, middle_reference, inner_reference]) in
        [&mut lr.filter_v, &mut lr.filter_h]
          .into_iter()
          .zip([reference.filter_v, reference.filter_h])
      {
        let outer = match plane {
          0 => {
            let Ok(value) = read_subexpression(
              msac,
              RestorationSubexpression {
                reference: outer_reference,
                order: 1,
                adjustment: 5,
              },
            );
            value
          }
          _ => 0,
        };
        let Ok(middle) = read_subexpression(
          msac,
          RestorationSubexpression {
            reference: middle_reference,
            order: 2,
            adjustment: 23,
          },
        );
        let Ok(inner) = read_subexpression(
          msac,
          RestorationSubexpression {
            reference: inner_reference,
            order: 3,
            adjustment: 17,
          },
        );
        *filter = [outer, middle, inner];
      }

      lr.sgr_weights = reference.sgr_weights;
      let mut references_2 = ts.lr_ref.try_write().ok_or(EINVAL)?;

      let saved = references_2.get_mut(plane_index).ok_or(EINVAL)?;
      *saved = *lr;

      match { ::core::primitive::bool::then_some(debug_block_info == BlockTrace::Enabled, ()) } {
        ::core::option::Option::Some(()) => {
          let [v0, v1, v2] = lr.filter_v;
          let [h0, h1, h2] = lr.filter_h;
          console_core_logging::trace!(
            "Post-lr_wiener[pl={plane},v[{v0},{v1},{v2}],h[{h0},{h1},{h2}]]: r={}",
            msac.rng,
          );
        }
        ::core::option::Option::None => {}
      }
    }
    Rav1dRestorationType::SgrProj(_) => {
      let Ok(raw_index) = rav1d_msac_decode_bools(msac, 4);
      let Ok(parameter_index) = console_core_number_conversion::index(raw_index);
      let sgr_index = SgrIdx::from_repr(parameter_index).ok_or(EINVAL)?;
      let &[outer_scale, inner_scale] = dav1d_sgr_params.get(parameter_index).ok_or(EINVAL)?;
      lr.r#type = Rav1dRestorationType::SgrProj(sgr_index);
      let [outer_reference, inner_reference] = reference.sgr_weights;
      let outer = match outer_scale {
        0 => 0,
        _ => {
          let Ok(value) = read_subexpression(
            msac,
            RestorationSubexpression {
              reference: outer_reference,
              order: 4,
              adjustment: 96,
            },
          );
          value
        }
      };
      let inner = match inner_scale {
        0 => 95,
        _ => {
          let Ok(value) = read_subexpression(
            msac,
            RestorationSubexpression {
              reference: inner_reference,
              order: 4,
              adjustment: 32,
            },
          );
          value
        }
      };
      lr.sgr_weights = [outer, inner];
      lr.filter_v = reference.filter_v;
      lr.filter_h = reference.filter_h;
      let mut references_3 = ts.lr_ref.try_write().ok_or(EINVAL)?;

      let saved = references_3.get_mut(plane_index).ok_or(EINVAL)?;
      *saved = *lr;

      match { ::core::primitive::bool::then_some(debug_block_info == BlockTrace::Enabled, ()) } {
        ::core::option::Option::Some(()) => {
          console_core_logging::trace!(
            "Post-lr_sgrproj[pl={plane},idx={sgr_index},w[{outer},{inner}]]: r={}",
            msac.rng,
          );
        }
        ::core::option::Option::None => {}
      }
    }
    Rav1dRestorationType::None | Rav1dRestorationType::Switchable => {}
  }

  Ok(())
}

fn check_trailing_bits_after_symbol_coder(msac: &MsacContext) -> Result<(), ()> {
  let trailing_bits = msac.cnt.wrapping_add(14).wrapping_neg();

  match trailing_bits.cmp(&0) {
    core::cmp::Ordering::Greater => return Err(()),
    core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {}
  }

  let displacement = trailing_bits
    .wrapping_add(7)
    .wrapping_shr(3)
    .wrapping_sub(1);
  let bytes = msac
    .trailing_bytes(displacement)
    .map_err(|_invalid_offset| ())?;
  let (&first, padding) = bytes.split_first().ok_or(())?;
  let Ok(shift) =
    console_core_number_conversion::wrapped::<_, u32>(trailing_bits.wrapping_sub(1) & 7);
  let pattern = 128u8.wrapping_shr(shift);
  let significant_bits = pattern.wrapping_add(pattern.wrapping_sub(1));

  match (first & significant_bits == pattern && padding.iter().all(|&byte| byte == 0))
    .then_some(())
  {
    Some(()) => Ok(()),
    None => Err(()),
  }
}

struct RestorationGrid {
  plane: u8,
  kind: Rav1dRestorationType,
  unit_shift: u8,
  horizontal: u8,
}

impl RestorationGrid {
  fn unit_size(&self) -> Result<i32, console_core_never::Never> {
    Ok(1_i32.wrapping_shl(u32::from(self.unit_shift)))
  }

  fn read_unit(
    &self,
    frame: &Rav1dFrameData,
    tile: &Rav1dTileState,
    block: Bxy,
    address: Point<i32>,
  ) -> Result<(), ()> {
    let Ok(superblock) = console_core_number_conversion::wrapped_index(address.x);
    let Ok(unit) = console_core_number_conversion::wrapped_index(address.y);
    let Ok(plane) = console_core_number_conversion::index(self.plane);
    let mask = frame.lf.lr_mask.get(superblock).ok_or(())?;
    let row = mask.lr.get(plane).ok_or(())?;
    let mut output = row.get(unit).and_then(|one| one.try_write()).ok_or(())?;
    let Ok(trace) = debug_block_info(frame, block);
    read_restoration_info(tile, &mut output, self.plane, self.kind, trace)
      .map_err(|_invalid_restoration| ())
  }

  fn read_scaled(
    &self,
    frame: &Rav1dFrameData,
    tile: &Rav1dTileState,
    block: Bxy,
    denominator: i32,
  ) -> Result<(), ()> {
    let Ok(unit_size) = self.unit_size();
    let horizontal = u32::from(self.horizontal);
    let unit_shift = u32::from(self.unit_shift);
    let width = frame
      .sr_cur
      .p
      .p
      .w
      .wrapping_add(i32::from(self.horizontal))
      .wrapping_shr(horizontal);
    let half = unit_size.wrapping_shr(1);
    let count = width.wrapping_add(half).wrapping_shr(unit_shift).max(1);
    let rounding = unit_size.wrapping_mul(8).wrapping_sub(1);
    let shift = u32::from(self.unit_shift.wrapping_add(3));
    let scaled = |column: i32| {
      column
        .wrapping_mul(4)
        .wrapping_mul(denominator)
        .wrapping_shr(horizontal)
        .wrapping_add(rounding)
        .wrapping_shr(shift)
    };
    let first = scaled(block.x);
    let end = scaled(block.x.wrapping_add(frame.sb_step)).min(count);
    let row_offset = block.y.wrapping_shr(5).wrapping_mul(frame.sr_sb128w);
    let row_unit = (block.y & 16).wrapping_shr(3);
    let pixel_shift = u32::from(self.unit_shift.wrapping_add(self.horizontal));

    for column in first..end {
      let x = column.wrapping_shl(pixel_shift);
      let address = Point {
        x: row_offset.wrapping_add(x.wrapping_shr(7)),
        y: row_unit.wrapping_add((x & 64).wrapping_shr(6)),
      };
      self.read_unit(frame, tile, block, address)?;
    }

    Ok(())
  }

  fn read_unscaled(
    &self,
    frame: &Rav1dFrameData,
    tile: &Rav1dTileState,
    block: Bxy,
  ) -> Result<(), ()> {
    let Ok(unit_size) = self.unit_size();
    let horizontal = u32::from(self.horizontal);
    let column = block.x.wrapping_mul(4).wrapping_shr(horizontal);
    let width = frame
      .cur
      .p
      .w
      .wrapping_add(i32::from(self.horizontal))
      .wrapping_shr(horizontal);
    let half = unit_size.wrapping_shr(1);

    match (column & unit_size.wrapping_sub(1) != 0
      || (column != 0 && column.wrapping_add(half) > width))
      .then_some(())
    {
      Some(()) => return Ok(()),
      None => {}
    }

    let address = Point {
      x: block
        .y
        .wrapping_shr(5)
        .wrapping_mul(frame.sr_sb128w)
        .wrapping_add(block.x.wrapping_shr(5)),
      y: (block.y & 16)
        .wrapping_shr(3)
        .wrapping_add((block.x & 16).wrapping_shr(4)),
    };
    self.read_unit(frame, tile, block, address)
  }
}

fn decode_superblock_restoration(
  frame: &Rav1dFrameData,
  task: &Rav1dTaskContext,
  tile: &Rav1dTileState,
) -> Result<(), ()> {
  let header = frame.frame_hdr().map_err(|_missing_header| ())?;
  let [width, upscaled] = header.size.width;
  let denominator = i32::from(header.size.super_res.width_scale_denominator);
  let [luma_unit, chroma_unit] = header.restoration.unit_size;

  for plane in 0u8..3 {
    match (frame
      .lf
      .restore_planes
      .bits()
      .wrapping_shr(u32::from(plane))
      & 1
      == 0)
      .then_some(())
    {
      Some(()) => continue,
      None => {}
    }

    let is_chroma = plane != 0;
    let vertical = i32::from(is_chroma && frame.cur.p.layout == Rav1dPixelLayout::I420);
    let horizontal = u8::from(is_chroma && frame.cur.p.layout != Rav1dPixelLayout::I444);
    let Ok(vertical_shift) = console_core_number_conversion::wrapped::<_, u32>(vertical);
    let unit_shift = match is_chroma.then_some(()) {
      Some(()) => chroma_unit,
      None => luma_unit,
    };
    let row = task.b.y.wrapping_mul(4).wrapping_shr(vertical_shift);
    let height = frame
      .cur
      .p
      .h
      .wrapping_add(vertical)
      .wrapping_shr(vertical_shift);
    let size = 1_i32.wrapping_shl(u32::from(unit_shift));

    match (row & size.wrapping_sub(1) != 0
      || (row != 0 && row.wrapping_add(size.wrapping_shr(1)) > height))
      .then_some(())
    {
      Some(()) => continue,
      None => {}
    }

    let Ok(index) = console_core_number_conversion::index(plane);
    let kind = header.restoration.r#type.get(index).copied().ok_or(())?;
    let grid = RestorationGrid {
      plane,
      kind,
      unit_shift,
      horizontal,
    };

    match width.cmp(&upscaled) {
      cmp::Ordering::Equal => grid.read_unscaled(frame, tile, task.b)?,
      cmp::Ordering::Less | cmp::Ordering::Greater => {
        grid.read_scaled(frame, tile, task.b, denominator)?
      }
    }
  }

  Ok(())
}

fn prepare_tile_row(
  c: &Rav1dContext,
  t: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
  ts: &Rav1dTileState,
  frame_hdr: &Rav1dFrameHeader,
) -> Result<(), ()> {
  match ::core::primitive::bool::then_some(
    frame_hdr.frame_type.is_inter_or_switch() || frame_hdr.allow_intrabc,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(row_shift) = console_core_number_conversion::wrapped::<_, u32>(f.sb_shift);
      let reference_motion = rav1d_refmvs_tile_sbrow_init(
        &f.rf,
        SpatialMvRegion {
          columns: RefmvsTileRange {
            start: ts.tiling.col_start,
            end: ts.tiling.col_end,
          },
          rows: RefmvsTileRange {
            start: ts.tiling.row_start,
            end: ts.tiling.row_end,
          },
        },
        MotionTileRow {
          superblock_row: t.b.y.wrapping_shr(row_shift),
          tile_row: ts.tiling.row,
          pass: match t.frame_thread.pass {
            2 => MotionRowPass::Second,
            _ => MotionRowPass::Primary,
          },
        },
      )
      .map_err(|_invalid_motion_row| ())?;
      t.rt = reference_motion;
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    frame_hdr.frame_type.is_inter_or_switch() && c.fc.len() > 1,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let sby = i32::wrapping_shr(i32::wrapping_sub(t.b.y, ts.tiling.row_start), {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(f.sb_shift);
        numeric_value
      });
      let position = f
        .lowest_pixel_position(ts, sby)
        .map_err(|_outside_tile| ())?;
      let Ok(address) = console_core_number_conversion::index(position);
      *f.lowest_pixel_mem.index_mut(address) = [[i32::MIN; 2]; 7];
    }
    ::core::option::Option::None => {}
  }

  let initialization = match t.frame_thread.pass {
    2 => ContextInitialization::PredictionOnly,
    _ => ContextInitialization::Full,
  };
  let Ok(()) = reset_context(&mut t.l, frame_hdr.frame_type, initialization);

  crate::src::owned_recon::arm_sbrow(f, t).map_err(|_invalid_band| ())?;

  Ok(())
}

fn store_tile_right_edge(
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
  tile_col: i32,
) -> Result<(), ()> {
  let checked_value_4 = u64::try_from(f.bh).map_err(|_invalid_height| ())?;
  let padded_height = checked_value_4.wrapping_add(31) & !31;
  let column = u64::try_from(tile_col).map_err(|_invalid_tile| ())?;
  let row = u64::try_from(t.b.y).map_err(|_invalid_row| ())?;
  let edge_step = u64::try_from(f.sb_step).map_err(|_invalid_superblock| ())?;
  let vertical = u32::from(f.cur.p.layout == Rav1dPixelLayout::I420);
  let edge_range = |shift| {
    let first = (row & 16).wrapping_shr(shift);
    let end = first.wrapping_add(edge_step.wrapping_shr(shift));
    first..end
  };
  let Ok(y_range) = console_core_number_conversion::indices(edge_range(0));
  let Ok(uv_range) = console_core_number_conversion::indices(edge_range(vertical));
  let y_values = t.l.tx_lpf_y.get_mut().get(y_range).ok_or(())?;
  let uv_values = t.l.tx_lpf_uv.get_mut().get(uv_range).ok_or(())?;
  let y_offset = padded_height.wrapping_mul(column).wrapping_add(row);
  let uv_offset = padded_height
    .wrapping_shr(vertical)
    .wrapping_mul(column)
    .wrapping_add(row.wrapping_shr(vertical));
  f.lf
    .tx_lpf_right_edge
    .store(RightEdgeRows {
      luma: RightEdgePixels {
        start: y_offset,
        values: y_values,
      },
      chroma: RightEdgePixels {
        start: uv_offset,
        values: uv_values,
      },
    })
    .map_err(|_invalid_edge| ())?;

  Ok(())
}

fn validate_tile_row(
  strictness: Strictness,
  f: &Rav1dFrameData,
  t: &Rav1dTaskContext,
  ts: &Rav1dTileState,
) -> Result<(), ()> {
  {
    let context = ts.context.try_lock().ok_or(())?;
    context.msac.check().map_err(|_invalid_entropy_table| ())?;

    match context.msac.cnt {
      ..=-15 => return Err(()),
      -14.. => {}
    }
  }

  match (strictness >= Strictness::Strict).then_some(()) {
    None => return Ok(()),
    Some(()) => {}
  }

  let header = f.frame_hdr().map_err(|_missing_header| ())?;
  let Ok(next_tile) =
    console_core_number_conversion::wrapped_index(ts.tiling.row.wrapping_add(1));
  let end = header
    .tiling
    .row_start_sb
    .get(next_tile)
    .copied()
    .ok_or(())?;
  let Ok(shift) = console_core_number_conversion::wrapped::<_, u32>(f.sb_shift);
  let next = t.b.y.wrapping_shr(shift).wrapping_add(1);

  match next.cmp(&i32::from(end)) {
    cmp::Ordering::Less => Ok(()),
    cmp::Ordering::Equal | cmp::Ordering::Greater => {
      let context = ts.context.try_lock().ok_or(())?;
      check_trailing_bits_after_symbol_coder(&context.msac)
    }
  }
}

pub(crate) fn rav1d_decode_tile_sbrow(
  c: &Rav1dContext,
  t: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
) -> Result<(), ()> {
  let sequence = f.seq_hdr().map_err(|_missing_header| ())?;
  let root_bl = match sequence.sb128 {
    0 => BlockLevel::Bl64x64,
    _ => BlockLevel::Bl128x128,
  };
  let ts = f.tile(t.tile_index).map_err(|_missing_tile| ())?;
  let sb_step = f.sb_step;
  let superblock_step = match sb_step {
    16 => 16u32,
    32 => 32,
    _ => return Err(()),
  };
  let Ok(step) = console_core_number_conversion::index(superblock_step);
  let tile_row = ts.tiling.row;
  let tile_col = ts.tiling.col;
  let frame_hdr = f.frame_hdr().map_err(|_missing_header| ())?;
  let Ok(column) = console_core_number_conversion::wrapped_index(tile_col);
  let col_sb_start = frame_hdr
    .tiling
    .col_start_sb
    .get(column)
    .copied()
    .ok_or(())?;
  let col_sb128_start = i32::from(col_sb_start).wrapping_shr(u32::from(sequence.sb128 == 0));

  prepare_tile_row(c, t, f, ts, frame_hdr)?;

  match ::core::primitive::bool::then_some(t.frame_thread.pass == 2, ()) {
    ::core::option::Option::Some(()) => {
      let off_2pass = match ::core::primitive::bool::then_some(c.tc.len() > 1, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(rows) = console_core_number_conversion::wrapped::<_, i32>(frame_hdr.tiling.rows);

          i32::wrapping_mul(f.sb128w, rows)
        }
        ::core::option::Option::None => 0,
      };
      let context = off_2pass
        .wrapping_add(col_sb128_start)
        .wrapping_add(tile_row.wrapping_mul(f.sb128w));
      let checked_value = u32::try_from(context).map_err(|_invalid_above_context| ())?;
      t.above_index = checked_value;

      for bx in (ts.tiling.col_start..ts.tiling.col_end).step_by(step.max(1)) {
        t.b.x = bx;

        match ::core::primitive::bool::then_some(c.flush.load(Ordering::Acquire), ()) {
          ::core::option::Option::Some(()) => {
            return Err(());
          }
          ::core::option::Option::None => {}
        }

        let Ok(root) = EdgeIndex::root();
        decode_sb(c, t, f, &mut FrameThreadPassState::Second, root_bl, root)?;

        match ::core::primitive::bool::then_some(t.b.x & 16 != 0 || sequence.sb128 != 0, ()) {
          ::core::option::Option::Some(()) => {
            t.above_index = t.above_index.wrapping_add(1);
          }
          ::core::option::Option::None => {}
        }
      }

      let Ok(bd_fn) = f.bd_fn();
      (bd_fn.backup_ipred_edge)(f, t).map_err(|_invalid_band| ())?;
      let Ok(f_bd_fn) = f.bd_fn();
      (f_bd_fn.stitch_recon_band)(f, t).map_err(|_invalid_band| ())?;
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(c.tc.len() > 1 && frame_hdr.use_ref_frame_mvs != 0, ())
  {
    ::core::option::Option::Some(()) => {
      c.dsp
        .refmvs
        .load_tmvs
        .call(
          &f.rf,
          &f.ref_mvs,
          ts.tiling.row,
          TemporalMvRegion {
            columns: ts.tiling.col_start.wrapping_shr(1)..ts.tiling.col_end.wrapping_shr(1),
            rows: t.b.y.wrapping_shr(1)..t.b.y.wrapping_add(sb_step).wrapping_shr(1),
          },
        )
        .map_err(|_invalid_motion| ())?;
    }
    ::core::option::Option::None => {}
  }

  *{
    let [_, _explicit_part, ..] = &mut t.pal_sz_uv;
    _explicit_part
  } = Default::default();
  let sb128y = i32::wrapping_shr(t.b.y, 5_u32);
  let context = col_sb128_start.wrapping_add(tile_row.wrapping_mul(f.sb128w));
  let checked_value_2 = u32::try_from(context).map_err(|_invalid_above_context| ())?;
  t.above_index = checked_value_2;
  let mask_position = sb128y.wrapping_mul(f.sb128w).wrapping_add(col_sb128_start);
  let checked_value_3 = u32::try_from(mask_position).map_err(|_invalid_mask| ())?;
  t.lf_mask = Some(checked_value_3);

  for bx in (ts.tiling.col_start..ts.tiling.col_end).step_by(step.max(1)) {
    t.b.x = bx;

    match ::core::primitive::bool::then_some(c.flush.load(Ordering::Acquire), ()) {
      ::core::option::Option::Some(()) => {
        return Err(());
      }
      ::core::option::Option::None => {}
    }

    reset_superblock_cdef(f, t, root_bl).map_err(|_invalid_cdef| ())?;

    decode_superblock_restoration(f, t, ts)?;

    {
      let tile = f.tile(t.tile_index).map_err(|_invalid_slot| ())?;
      let mut context_2 = tile.context.try_lock().ok_or(())?;
      let Ok(root_edge) = EdgeIndex::root();
      let mut pass = FrameThreadPassState::First(&mut context_2);
      decode_sb(c, t, f, &mut pass, root_bl, root_edge)?;
    }

    match ::core::primitive::bool::then_some(t.b.x & 16 != 0 || sequence.sb128 != 0, ()) {
      ::core::option::Option::Some(()) => {
        t.above_index = t.above_index.wrapping_add(1);

        t.lf_mask = t.lf_mask.map(|i| i.wrapping_add(1));
      }
      ::core::option::Option::None => {}
    }
  }

  match ::core::primitive::bool::then_some(
    sequence.ref_frame_mvs != 0 && c.tc.len() > 1 && frame_hdr.frame_type.is_inter_or_switch(),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      c.dsp
        .refmvs
        .save_tmvs
        .call(
          &t.rt,
          &f.rf,
          &f.mvs,
          TemporalMvRegion {
            columns: ts.tiling.col_start.wrapping_shr(1)..ts.tiling.col_end.wrapping_shr(1),
            rows: t.b.y.wrapping_shr(1)..t.b.y.wrapping_add(sb_step).wrapping_shr(1),
          },
        )
        .map_err(|_invalid_motion| ())?;
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(t.frame_thread.pass != 1, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(bd_fn) = f.bd_fn();
      (bd_fn.backup_ipred_edge)(f, t).map_err(|_invalid_band| ())?;
    }
    ::core::option::Option::None => {}
  }

  let Ok(bd_fn) = f.bd_fn();
  (bd_fn.stitch_recon_band)(f, t).map_err(|_invalid_band| ())?;

  store_tile_right_edge(f, t, tile_col)?;
  validate_tile_row(c.strictness, f, t, ts)
}

#[derive(Clone, Copy)]
enum ExtraCdefLines {
  Required,
  Shared,
}

struct FilterLineBuffer {
  base: u64,
  pitch: i64,
  rows: u64,
}

impl FilterLineBuffer {
  fn offset(&self, row: u64) -> Rav1dResult<u64> {
    let physical_row = match self.pitch.cmp(&0) {
      std::cmp::Ordering::Less => {
        let last = self.rows.checked_sub(1).ok_or(EINVAL)?;
        last.checked_sub(row).ok_or(EINVAL)?
      }
      std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => row,
    };
    let displacement = self
      .pitch
      .unsigned_abs()
      .checked_mul(physical_row)
      .ok_or(EINVAL)?;
    self.base.checked_add(displacement).ok_or(EINVAL)
  }

  fn end(&self) -> Rav1dResult<u64> {
    let length = self
      .pitch
      .unsigned_abs()
      .checked_mul(self.rows)
      .ok_or(EINVAL)?;
    self.base.checked_add(length).ok_or(EINVAL)
  }
}

struct FilterLineLayout {
  bitdepth: BPC,
  byte_pitches: [i64; 2],
  rows: u64,
  extra_cdef: ExtraCdefLines,
}

fn filter_buffer_size(pitches: [i64; 2], rows: u64, padding: u16) -> Rav1dResult<u64> {
  let [luma, chroma] = pitches;
  let doubled_chroma = chroma.unsigned_abs().checked_mul(2).ok_or(ENOMEM)?;
  let row_bytes = luma
    .unsigned_abs()
    .checked_add(doubled_chroma)
    .ok_or(ENOMEM)?;
  let size = row_bytes.checked_mul(rows).ok_or(ENOMEM)?;
  size.checked_add(u64::from(padding)).ok_or(ENOMEM)
}

fn configure_cdef_lines(lf: &mut Rav1dFrameContextLf, layout: FilterLineLayout) -> Rav1dResult {
  let [y_bytes, uv_bytes] = layout.byte_pitches;
  let [y_pitch, uv_pitch] =
    [y_bytes, uv_bytes].map(|byte_pitch| layout.bitdepth.pxstride(byte_pitch));
  let rows = layout.rows;
  let chroma_rows = rows.checked_mul(2).ok_or(ENOMEM)?;
  let storage_rows = match layout.extra_cdef {
    ExtraCdefLines::Required => chroma_rows,
    ExtraCdefLines::Shared => rows,
  };
  let buffer_bytes = filter_buffer_size(layout.byte_pitches, storage_rows, 64)?;
  let buffer_len = console_core_number_conversion::checked_index(buffer_bytes)
    .map_err(|_outside_address_space| ENOMEM)?;
  lf.cdef_line_buf
    .try_resize(buffer_len, 0)
    .map_err(|_allocation| ENOMEM)?;

  let luma = FilterLineBuffer {
    base: layout.bitdepth.pxstride(32_u64),
    pitch: y_pitch,
    rows,
  };
  let chroma_base = luma.end()?;
  let chroma = FilterLineBuffer {
    base: chroma_base,
    pitch: uv_pitch,
    rows: chroma_rows,
  };
  let y0_offset = luma.offset(0)?;
  let y1_offset = luma.offset(2)?;
  let u0_offset = chroma.offset(0)?;
  let v0_offset = chroma.offset(2)?;
  let u1_offset = chroma.offset(4)?;
  let v1_offset = chroma.offset(6)?;
  lf.cdef_line = [
    [y0_offset, u0_offset, v0_offset],
    [y1_offset, u1_offset, v1_offset],
  ];

  match layout.extra_cdef {
    ExtraCdefLines::Shared => {}
    ExtraCdefLines::Required => {
      let next_luma = chroma.end()?;
      let y = FilterLineBuffer {
        base: next_luma,
        ..luma
      };
      let next_chroma = y.end()?;
      let [u, v] = separate_chroma_lines(FilterLineBuffer {
        base: next_chroma,
        pitch: uv_pitch,
        rows,
      })?;
      let y_offset = y.offset(0)?;
      lf.cdef_lpf_line = [y_offset, u, v];
    }
  }

  Ok(())
}

fn separate_chroma_lines(cb: FilterLineBuffer) -> Rav1dResult<[u64; 2]> {
  let cr_base = cb.end()?;
  let cr = FilterLineBuffer {
    base: cr_base,
    ..cb
  };
  let u = cb.offset(0)?;
  let v = cr.offset(0)?;
  Ok([u, v])
}

fn configure_restoration_lines(
  lf: &mut Rav1dFrameContextLf,
  bitdepth: BPC,
  byte_pitches: [i64; 2],
  rows: u64,
) -> Rav1dResult {
  let buffer_bytes = filter_buffer_size(byte_pitches, rows, 128)?;
  let buffer_len = console_core_number_conversion::checked_index(buffer_bytes)
    .map_err(|_outside_address_space| ENOMEM)?;
  lf.lr_line_buf
    .try_resize(buffer_len, 0)
    .map_err(|_allocation| ENOMEM)?;

  let [y_pitch, uv_pitch] = byte_pitches.map(|bytes| bitdepth.pxstride(bytes));
  let luma = FilterLineBuffer {
    base: bitdepth.pxstride(64_u64),
    pitch: y_pitch,
    rows,
  };
  let chroma_base = luma.end()?;
  let [u, v] = separate_chroma_lines(FilterLineBuffer {
    base: chroma_base,
    pitch: uv_pitch,
    rows,
  })?;
  let y = luma.offset(0)?;
  lf.lr_lpf_line = [y, u, v];
  Ok(())
}

fn compound_distance_weight([left, right]: [u8; 2]) -> Result<u8, console_core_never::Never> {
  use cmp::Ordering::{Equal, Greater, Less};

  let nearer = left.min(right);
  let farther = left.max(right).wrapping_mul(2);
  let ratios = [3, 5, 7].map(|near_factor| nearer.wrapping_mul(near_factor).cmp(&farther));
  let [first, second] = match ratios {
    [Greater, _, _] => [9, 7],
    [Equal | Less, Greater, _] => [11, 5],
    [Equal | Less, Equal | Less, Greater] => [12, 4],
    [Equal | Less, Equal | Less, Equal | Less] => [13, 3],
  };

  Ok(match left.cmp(&right) {
    Greater => first,
    Equal | Less => second,
  })
}

fn assign_compound_weights(frame: &mut Rav1dFrameData, order_hint_bits: u8) -> Rav1dResult {
  let header = frame.cur.frame_hdr.as_ref().ok_or(EINVAL)?;
  let Ok(current) = console_core_number_conversion::wrapped::<_, i32>(header.frame_offset);
  let distances = frame.refp.each_ref().map(|reference| {
    let offset = match reference.p.frame_hdr.as_ref() {
      Some(reference_header) => reference_header.frame_offset,
      None => 0,
    };
    let Ok(target) = console_core_number_conversion::wrapped::<_, i32>(offset);
    let Ok(difference) = get_poc_diff(
      order_hint_bits,
      OrderHintComparison {
        target,
        origin: current,
      },
    );
    let Ok(distance) =
      console_core_number_conversion::wrapped::<_, u8>(difference.unsigned_abs().min(31));
    distance
  });

  for (i, (weights, first)) in frame.jnt_weights.iter_mut().zip(distances).enumerate() {
    for (weight, second) in weights.iter_mut().zip(distances).skip(i.wrapping_add(1)) {
      let Ok(value) = compound_distance_weight([second, first]);
      *weight = value;
    }
  }

  Ok(())
}

fn mark_tile_row_starts(
  markers: &mut Vec<u8>,
  superblock_rows: i32,
  tiling: &Rav1dFrameHeaderTiling,
) -> Rav1dResult {
  let height = console_core_number_conversion::checked_index(superblock_rows)
    .map_err(|_invalid_height| EINVAL)?;
  markers.try_resize(height, 0).map_err(|_allocate| ENOMEM)?;
  let mut sby = 0u32;

  for tile_row in 0..tiling.rows {
    let Ok(position) = console_core_number_conversion::index(sby);
    let first = markers.get_mut(position).ok_or(EINVAL)?;
    *first = tile_row;
    sby = sby.wrapping_add(1);
    let Ok(next_tile) =
      console_core_number_conversion::index(u32::from(tile_row).wrapping_add(1));
    let limit = tiling.row_start_sb.get(next_tile).copied().ok_or(EINVAL)?;

    for y in sby..u32::from(limit) {
      let Ok(row) = console_core_number_conversion::index(y);
      let marker = markers.get_mut(row).ok_or(EINVAL)?;
      *marker = 0;
      sby = y.wrapping_add(1);
    }
  }

  Ok(())
}

fn initialize_frame_tile_tables(c: &Rav1dContext, f_2: &mut Rav1dFrameData) -> Rav1dResult {
  let header = f_2.frame_hdr.as_ref().ok_or(EINVAL)?;
  let tiles = u32::from(header.tiling.cols).wrapping_mul(u32::from(header.tiling.rows));
  let workers = u32::try_from(c.tc.len()).map_err(|_too_many_workers| EINVAL)?;
  let parallelism = crate::src::internal::ScratchParallelism { workers, tiles };
  let Ok(()) = f_2.configure_scratch_parallelism(parallelism);

  let frame_hdr = f_2.frame_hdr.as_deref().ok_or(EINVAL)?;
  mark_tile_row_starts(&mut f_2.lf.start_of_tile_row, f_2.sbh, &frame_hdr.tiling)?;

  let Ok(frame_hdr_cols) =
    console_core_number_conversion::wrapped::<_, i32>(frame_hdr.tiling.cols);
  let Ok(frame_hdr_rows) =
    console_core_number_conversion::wrapped::<_, i32>(frame_hdr.tiling.rows);
  let n_ts = i32::wrapping_mul(frame_hdr_cols, frame_hdr_rows);

  match ::core::primitive::bool::then_some(c.fc.len() > 1, ()) {
    ::core::option::Option::Some(()) => {
      f_2
        .frame_thread
        .tile_start_off
        .try_resize(
          {
            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(n_ts);
            numeric_value
          },
          0,
        )
        .map_err(|_| ENOMEM)?;
    }
    ::core::option::Option::None => {}
  }

  f_2
    .ts
    .try_resize_with(
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(n_ts);
        numeric_value
      },
      Default::default,
    )
    .map_err(|_| ENOMEM)?;

  let a_sz = i32::wrapping_mul(
    i32::wrapping_mul(f_2.sb128w, {
      let Ok(numeric_value) =
        console_core_number_conversion::wrapped::<_, i32>(frame_hdr.tiling.rows);
      numeric_value
    }),
    i32::wrapping_add(1, i32::from(c.fc.len() > 1 && c.tc.len() > 1)),
  );
  f_2
    .a
    .try_resize_with(
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(a_sz);
        numeric_value
      },
      Default::default,
    )
    .map_err(|_| ENOMEM)?;

  Ok(())
}

fn initialize_frame_thread_storage(c: &Rav1dContext, f_2: &mut Rav1dFrameData) -> Rav1dResult {
  let header = f_2.frame_hdr.as_deref().ok_or(EINVAL)?;
  let frame_hdr = &**header;
  let num_sb128 = i32::wrapping_mul(f_2.sb128w, f_2.sb128h);
  let &[coefficient_scale, palette_scale] = ss_size_mul.checked(f_2.cur.p.layout)?;
  let f_2_seq_hdr = f_2.seq_hdr.as_ref().ok_or(EINVAL)?;
  let seq_hdr = &***f_2_seq_hdr;
  let bit_depth_shift = u32::from(seq_hdr.hbd != 0);

  match ::core::primitive::bool::then_some(c.fc.len() > 1, ()) {
    ::core::option::Option::Some(()) => {
      let checked_value = u32::try_from(f_2.sb_step).map_err(|_invalid_superblock| EINVAL)?;
      let sb_step4 = checked_value.wrapping_mul(4);
      let frame_width = u32::try_from(f_2.sb128w).map_err(|_invalid_width| EINVAL)?;
      let columns = u32::from(frame_hdr.tiling.cols);

      for tile_row in 0..u32::from(frame_hdr.tiling.rows) {
        let [first, end] = adjacent_tile_boundaries(&frame_hdr.tiling.row_start_sb, tile_row)?;
        let first_row = u32::try_from(first).map_err(|_invalid_tile| EINVAL)?;
        let rows = u32::try_from(end.wrapping_sub(first)).map_err(|_invalid_tile| EINVAL)?;
        let row_offset = first_row
          .wrapping_mul(sb_step4)
          .wrapping_mul(frame_width)
          .wrapping_mul(128);
        let block_rows = rows.wrapping_mul(sb_step4);

        for tile_column in 0..columns {
          let Ok(column) = console_core_number_conversion::index(tile_column);
          let &first_column = frame_hdr.tiling.col_start_sb.get(column).ok_or(EINVAL)?;
          let position = tile_row.wrapping_mul(columns).wrapping_add(tile_column);
          let Ok(tile) = console_core_number_conversion::index(position);
          let start = f_2
            .frame_thread
            .tile_start_off
            .get_mut(tile)
            .ok_or(EINVAL)?;
          let column_offset = block_rows
            .wrapping_mul(u32::from(first_column))
            .wrapping_mul(sb_step4);
          *start = row_offset.wrapping_add(column_offset);
        }
      }

      let tile_width = u32::from(frame_hdr.tiling.cols);
      let superblock_rows = u64::try_from(f_2.sbh).map_err(|_invalid_height| EINVAL)?;
      let map_size = superblock_rows.wrapping_mul(u64::from(tile_width));
      let map_length = console_core_number_conversion::checked_index(map_size)
        .map_err(|_outside_allocation| ENOMEM)?;
      f_2
        .lowest_pixel_mem
        .try_resize(map_length, Default::default())
        .map_err(|_allocation| ENOMEM)?;
      let mut lowest_pixel_offset = 0_u64;

      for tile_row in 0..u32::from(frame_hdr.tiling.rows) {
        let [start, end] = adjacent_tile_boundaries(&frame_hdr.tiling.row_start_sb, tile_row)?;
        let height = u64::try_from(end.wrapping_sub(start)).map_err(|_invalid_height| EINVAL)?;

        for tile_col in 0..tile_width {
          let position = tile_row.wrapping_mul(tile_width).wrapping_add(tile_col);
          let Ok(index) = console_core_number_conversion::index(position);
          let tile = f_2.ts.get_mut(index).ok_or(EINVAL)?;
          tile.lowest_pixel = lowest_pixel_offset;
          let checked_value_2 = lowest_pixel_offset.checked_add(height).ok_or(EINVAL)?;
          lowest_pixel_offset = checked_value_2;
        }
      }

      let cbi_sz = num_sb128.wrapping_mul(i32::from(coefficient_scale));
      f_2
        .frame_thread
        .cbi
        .try_resize_with(
          ({
            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbi_sz);
            numeric_value
          })
          .wrapping_mul(32)
          .wrapping_mul(32)
          .wrapping_div(4),
          || RelaxedAtomic::from(CodedBlockInfo::INITIAL),
        )
        .map_err(|_| ENOMEM)?;

      let cf_sz = cbi_sz.wrapping_shl(bit_depth_shift);
      f_2
        .frame_thread
        .cf
        .try_resize(
          ({
            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cf_sz);
            numeric_value
          })
          .wrapping_mul(128)
          .wrapping_mul(128)
          .wrapping_div(2),
          0,
        )
        .map_err(|_| ENOMEM)?;

      match ::core::primitive::bool::then_some(frame_hdr.allow_screen_content_tools, ()) {
        ::core::option::Option::Some(()) => {
          let superblocks = u64::try_from(num_sb128).map_err(|_invalid_palette_grid| EINVAL)?;
          let palette_entries = superblocks.wrapping_mul(256).wrapping_shl(bit_depth_shift);
          f_2.frame_thread.pal.try_resize(palette_entries)?;

          let pal_idx_sz = num_sb128.wrapping_mul(i32::from(palette_scale));
          f_2
            .frame_thread
            .pal_idx
            .try_resize(
              ({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(pal_idx_sz);
                numeric_value
              })
              .wrapping_mul(128)
              .wrapping_mul(128)
              .wrapping_div(8),
              Default::default(),
            )
            .map_err(|_| ENOMEM)?;
        }
        ::core::option::Option::None => match f_2.frame_thread.pal.pixels() {
          Ok(crate::src::internal::PalettePixels::Present) => {
            let _ = mem::take(&mut f_2.frame_thread.pal);
            let _ = mem::take(&mut f_2.frame_thread.pal_idx);
          }
          Ok(crate::src::internal::PalettePixels::Empty) => {}
        },
      }
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn initialize_frame_filters(c: &Rav1dContext, f_2: &mut Rav1dFrameData) -> Rav1dResult {
  let header = f_2.frame_hdr.as_deref().ok_or(EINVAL)?;
  let frame_hdr = &**header;
  let seq_hdr = f_2.seq_hdr.as_deref().ok_or(EINVAL)?;
  let bitdepth = BPC::from_bitdepth_max(f_2.bitdepth_max);
  let superblock_rows = u64::try_from(f_2.sbh).map_err(|_invalid_height| EINVAL)?;
  let cdef_rows = superblock_rows.checked_mul(4).ok_or(ENOMEM)?;
  let [original_width, scaled_width] = frame_hdr.size.width;
  let workers_2 = c.tc.len().cmp(&1);
  let extra_cdef = match (workers_2, original_width.cmp(&scaled_width)) {
    (std::cmp::Ordering::Greater, std::cmp::Ordering::Less | std::cmp::Ordering::Greater) => {
      ExtraCdefLines::Required
    }
    _ => ExtraCdefLines::Shared,
  };
  let pitch = |stride| {
    let Ok(fixed) = console_core_number_conversion::fixed_offset(stride);
    fixed
  };
  let cdef_layout = FilterLineLayout {
    bitdepth,
    byte_pitches: f_2.cur.stride.map(pitch),
    rows: cdef_rows,
    extra_cdef,
  };
  configure_cdef_lines(&mut f_2.lf, cdef_layout)?;
  let restoration_rows = match workers_2 {
    std::cmp::Ordering::Greater => cdef_rows.wrapping_shl(u32::from(seq_hdr.sb128)),
    std::cmp::Ordering::Equal | std::cmp::Ordering::Less => 12,
  };
  let restored_pitch = f_2.sr_cur.p.stride.map(pitch);
  configure_restoration_lines(&mut f_2.lf, bitdepth, restored_pitch, restoration_rows)?;

  initialize_loop_filter_masks(c, f_2)
}

fn initialize_loop_filter_masks(c: &Rav1dContext, f_2: &mut Rav1dFrameData) -> Rav1dResult {
  let num_sb128 = f_2.sb128w.wrapping_mul(f_2.sb128h);
  let frame_hdr = f_2.frame_hdr.as_deref().ok_or(EINVAL)?;
  f_2.lf.mask.clear();
  f_2
    .lf
    .mask
    .try_resize_with(
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(num_sb128);
        numeric_value
      },
      Default::default,
    )
    .map_err(|_| ENOMEM)?;

  {
    const MAX_SUPERBLOCKS_PER_AXIS: u64 = 65_536 / 128;
    const MAX_LEVEL_ENTRIES: u64 = MAX_SUPERBLOCKS_PER_AXIS
      .wrapping_mul(MAX_SUPERBLOCKS_PER_AXIS)
      .wrapping_mul(4096)
      .wrapping_add(3);
    let superblocks = u64::try_from(num_sb128).map_err(|_invalid_block_count| EINVAL)?;
    let length = superblocks.wrapping_mul(4096).wrapping_add(3);

    match length.cmp(&MAX_LEVEL_ENTRIES) {
      cmp::Ordering::Greater => return Err(EINVAL),
      cmp::Ordering::Equal | cmp::Ordering::Less => {}
    }

    let level_len = console_core_number_conversion::checked_index(length)
      .map_err(|_outside_allocation| ENOMEM)?;

    f_2
      .lf
      .level
      .try_resize_with(level_len, || AtomicU8::new(0))
      .map_err(|_allocate| ENOMEM)?;
  }

  match ::core::primitive::bool::then_some(c.fc.len() > 1, ()) {
    ::core::option::Option::Some(()) => {
      f_2
        .frame_thread
        .b
        .try_resize_with(
          ({
            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(num_sb128);
            numeric_value
          })
          .wrapping_mul(32)
          .wrapping_mul(32),
          Default::default,
        )
        .map_err(|_| ENOMEM)?;
    }
    ::core::option::Option::None => {}
  }

  f_2.sr_sb128w = i32::wrapping_shr(i32::wrapping_add(f_2.sr_cur.p.p.w, 127), 7_u32);
  let lr_mask_sz = i32::wrapping_mul(f_2.sr_sb128w, f_2.sb128h);
  f_2
    .lf
    .lr_mask
    .try_resize_with(
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(lr_mask_sz);
        numeric_value
      },
      Default::default,
    )
    .map_err(|_| ENOMEM)?;
  f_2.lf.restore_planes = LrRestorePlanes::from_bits_truncate(
    frame_hdr
      .restoration
      .r#type
      .iter()
      .enumerate()
      .map(|(i, &r#type)| {
        u8::wrapping_shl(u8::from(r#type != Rav1dRestorationType::None), {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(i);
          numeric_value
        })
      })
      .sum::<u8>(),
  );

  match ::core::primitive::bool::then_some(
    frame_hdr.loopfilter.sharpness != f_2.lf.last_sharpness,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(()) = rav1d_calc_eih(&mut *f_2.lf.lim_lut, frame_hdr.loopfilter.sharpness);
      f_2.lf.last_sharpness = frame_hdr.loopfilter.sharpness;
    }
    ::core::option::Option::None => {}
  }

  let Ok(()) = rav1d_calc_lf_values(&mut f_2.lf.lvl, &frame_hdr, &[0, 0, 0, 0]);

  Ok(())
}

fn initialize_frame_edge_storage(c: &Rav1dContext, f_2: &mut Rav1dFrameData) -> Rav1dResult {
  let header = f_2.frame_hdr.as_deref().ok_or(EINVAL)?;
  let frame_hdr = &**header;
  let seq_hdr = f_2.seq_hdr.as_deref().ok_or(EINVAL)?;
  let bitdepth = BPC::from_bitdepth_max(f_2.bitdepth_max);
  let luma_columns = u64::try_from(f_2.sb128w).map_err(|_invalid_width| EINVAL)?;
  let superblock_rows_2 = u64::try_from(f_2.sbh).map_err(|_invalid_height| EINVAL)?;
  let blocks = luma_columns.checked_mul(superblock_rows_2).ok_or(ENOMEM)?;
  let plane_samples = blocks.checked_mul(128).ok_or(ENOMEM)?;
  let bytes_per_sample = 1_u64.wrapping_add(u64::from(seq_hdr.hbd != 0));
  let plane_bytes = plane_samples.checked_mul(bytes_per_sample).ok_or(ENOMEM)?;
  let edge_bytes = plane_bytes.checked_mul(3).ok_or(ENOMEM)?;
  let allocation = console_core_number_conversion::checked_index(edge_bytes)
    .map_err(|_outside_allocation| ENOMEM)?;
  f_2
    .ipred_edge
    .try_resize(allocation, 0)
    .map_err(|_allocation| ENOMEM)?;
  f_2.ipred_edge_off = bitdepth.pxstride(plane_bytes);

  let edge_rows = u64::try_from(f_2.sb128h).map_err(|_invalid_height| EINVAL)?;
  let edge_capacity = edge_rows.wrapping_mul(u64::from(frame_hdr.tiling.cols));
  f_2.lf.tx_lpf_right_edge.resize(edge_capacity)?;

  match ::core::primitive::bool::then_some(
    frame_hdr.frame_type.is_inter_or_switch() || frame_hdr.allow_intrabc,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(length) = console_core_number_conversion::wrapped::<_, u32>(c.tc.len());
      let Ok(c_length) = console_core_number_conversion::wrapped::<_, u32>(c.fc.len());
      rav1d_refmvs_init_frame(
        &mut f_2.rf,
        seq_hdr,
        frame_hdr,
        &f_2.refpoc,
        &f_2.refrefpoc,
        &f_2.ref_mvs,
        crate::src::refmvs::RefMvsParallelism {
          tile_threads: length,
          frame_threads: c_length,
        },
      )?;
    }
    ::core::option::Option::None => {}
  }

  init_quant_tables(seq_hdr, frame_hdr, frame_hdr.quant.yac, &f_2.dq)?;

  init_quant_matrices(&frame_hdr.quant, &mut f_2.qm)?;

  match frame_hdr.switchable_comp_refs {
    0 => {}
    _ => {
      assign_compound_weights(f_2, seq_hdr.order_hint_n_bits)?;
    }
  }

  Ok(())
}

pub(crate) fn rav1d_decode_frame_init(c: &Rav1dContext, fc: &Rav1dFrameContext) -> Rav1dResult {
  let mut frame = fc.data.try_write().ok_or(EINVAL)?;
  initialize_frame_tile_tables(c, &mut frame)?;
  initialize_frame_thread_storage(c, &mut frame)?;
  initialize_frame_filters(c, &mut frame)?;
  initialize_frame_edge_storage(c, &mut frame)?;
  let Ok(()) = crate::src::owned_recon::frame_setup(c, &mut frame);

  Ok(())
}

fn split_tile_payload(
  data: CArc<[u8]>,
  size_bytes: Option<u8>,
) -> Rav1dResult<(CArc<[u8]>, CArc<[u8]>)> {
  let (payload, size) = match size_bytes {
    None => {
      let Ok(remaining) = console_core_number_conversion::fixed_index(data.len());
      (data, remaining)
    }
    Some(width @ 1..=4) => {
      let (prefix, remaining) = CArc::split_at(data, u64::from(width))?;
      let mut encoded = [0; 4];
      let destination = encoded.get_mut(..prefix.len()).ok_or(EINVAL)?;

      for (byte, &encoded_byte) in destination.iter_mut().zip(prefix.iter()) {
        *byte = encoded_byte;
      }

      let size = u64::from(u32::from_le_bytes(encoded)).wrapping_add(1);
      (remaining, size)
    }
    Some(_) => return Err(EINVAL),
  };

  CArc::split_at(payload, size)
}

#[cfg(test)]
mod tile_payload_tests {
  use super::*;
  use crate::src::c_box::CBox;

  fn owned(bytes: &[u8]) -> Rav1dResult<CArc<[u8]>> {
    let Ok(boxed) = CBox::from_box(Box::from(bytes));
    CArc::wrap(boxed)
  }

  #[test]
  fn tile_sizes_are_little_endian_and_relative_to_the_remaining_view() -> Rav1dResult {
    let payload = owned(&[1, 0, 9, 8, 7, 6])?;
    let (first, rest) = split_tile_payload(payload, Some(2))?;
    let (last, empty) = split_tile_payload(rest, None)?;
    assert_eq!(&*first, &[9, 8]);
    assert_eq!(&*last, &[7, 6]);
    assert!(empty.is_empty());

    Ok(())
  }

  #[test]
  fn incomplete_tile_sizes_and_payloads_return_decode_errors() -> Rav1dResult {
    for (width, bytes) in [
      (2, [1].as_slice()),
      (1, [2, 7].as_slice()),
      (4, [0xff, 0xff, 0xff, 0xff].as_slice()),
      (0, [].as_slice()),
      (5, [0, 0, 0, 0, 0].as_slice()),
    ] {
      let result = split_tile_payload(
        {
          let checked_value_4 = owned(bytes)?;
          checked_value_4
        },
        Some(width),
      );
      assert_eq!(result.map(|_| ()), Err(EINVAL));
    }

    Ok(())
  }
}

fn initialize_above_contexts(
  context: &Rav1dContext,
  above: &mut [BlockContext],
  frame_type: Rav1dFrameType,
  contexts_per_pass: u64,
) -> Rav1dResult {
  match context.tc.len() {
    0 | 1 => return Ok(()),
    _ => {}
  }

  let threading = match context.fc.len() {
    0 | 1 => CdfThreading::Serial,
    _ => CdfThreading::ParallelFrames,
  };
  let passes = match threading {
    CdfThreading::Serial => 1,
    CdfThreading::ParallelFrames => 2,
  };
  let total = contexts_per_pass.wrapping_mul(passes);
  let Ok(context_count) = console_core_number_conversion::index(total);
  let contexts = above.get_mut(..context_count).ok_or(EINVAL)?;

  for (position, above_context) in (0_u64..).zip(contexts) {
    let initialization = match (&threading, position.cmp(&contexts_per_pass)) {
      (CdfThreading::ParallelFrames, cmp::Ordering::Equal | cmp::Ordering::Greater) => {
        ContextInitialization::PredictionOnly
      }
      (CdfThreading::ParallelFrames, cmp::Ordering::Less) | (CdfThreading::Serial, _) => {
        ContextInitialization::Full
      }
    };
    let Ok(()) = reset_context(above_context, frame_type, initialization);
  }

  Ok(())
}

pub(crate) fn rav1d_decode_frame_init_cdf(
  c: &Rav1dContext,
  fc: &Rav1dFrameContext,
  f: &mut Rav1dFrameData,
  in_cdf: &CdfThreadContext,
) -> Rav1dResult {
  let frame_hdr = f.frame_hdr.as_deref().ok_or(EINVAL)?;

  match ::core::primitive::bool::then_some(frame_hdr.refresh_context != 0, ()) {
    ::core::option::Option::Some(()) => {
      let initialized_cdf = rav1d_cdf_thread_copy(in_cdf)?;
      let mut output_cdf = f.out_cdf.cdf_write()?;
      *output_cdf = initialized_cdf;
    }
    ::core::option::Option::None => {}
  }

  let uses_2pass = c.fc.len() > 1;

  let tiling = &frame_hdr.tiling;

  let rows = u64::from(tiling.rows);
  let cols = u32::from(tiling.cols);
  let sb128_width = u64::try_from(f.sb128w).map_err(|_invalid_width| EINVAL)?;
  let contexts_per_pass = sb128_width.checked_mul(rows).ok_or(EINVAL)?;

  let mut tile_position = Point { x: 0u32, y: 0u32 };
  let Ok(update) = console_core_number_conversion::wrapped_index(tiling.update);
  let Ok(()) = fc.task_thread.update_set.set(false);

  for tile in &f.tiles {
    let start = checked_index(tile.hdr.start).map_err(|_invalid_start| EINVAL)?;
    let end = checked_index(tile.hdr.end).map_err(|_invalid_end| EINVAL)?;

    let mut data = tile.data.data.clone().ok_or(EINVAL)?;

    let last = checked_index(i64::from(tile.hdr.end).wrapping_add(1))
      .map_err(|_invalid_tile_span| EINVAL)?;
    let slots = f.ts.get_mut(..last).ok_or(EINVAL)?;
    let starts = match uses_2pass.then_some(()) {
      Some(()) => f.frame_thread.tile_start_off.get(..last).ok_or(EINVAL)?,
      None => &[],
    };
    let offsets = starts.iter().copied().chain(iter::repeat_n(0, last));

    for (j, (ts, tile_start_off)) in slots.iter_mut().zip(offsets).enumerate().skip(start) {
      let size_bytes = (j != end).then_some(tiling.n_bytes);
      let (cur_data, rest_data) = split_tile_payload(data, size_bytes)?;
      let seq_hdr = f.seq_hdr.as_ref().ok_or(EINVAL)?;
      setup_tile(
        c,
        ts,
        TileSetupFrame {
          sequence: &***seq_hdr,
          header: frame_hdr,
          picture: &f.cur,
          buffers: &f.frame_thread,
          restoration: &mut f.lf,
        },
        SetupTileArguments {
          bitdepth_max: f.bitdepth_max,
          sb_shift: f.sb_shift,
          frame: Size {
            width: f.bw,
            height: f.bh,
          },
          superblock_widths: SuperblockWidths {
            scaled: f.sr_sb128w,
            coded: f.sb128w,
          },
          tile: tile_position,
          palette_start: tile_start_off,
        },
        TileEntropyInput {
          initial_cdf: in_cdf,
          payload: cur_data,
        },
      )?;

      tile_position.x = tile_position.x.wrapping_add(1);

      match ::core::primitive::bool::then_some(tile_position.x == cols, ()) {
        ::core::option::Option::Some(()) => {
          tile_position.x = 0;
          tile_position.y = tile_position.y.wrapping_add(1);
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(j == update && frame_hdr.refresh_context != 0, ())
      {
        ::core::option::Option::Some(()) => {
          let Ok(()) = fc.task_thread.update_set.set(true);
        }
        ::core::option::Option::None => {}
      }

      data = rest_data;
    }
  }

  initialize_above_contexts(c, &mut f.a, frame_hdr.frame_type, contexts_per_pass)
}

struct FrameTileRow {
  tile_row: u32,
  superblock_row: i32,
  columns: u32,
}

fn decode_frame_row(
  c: &Rav1dContext,
  task: &mut Rav1dTaskContext,
  frame: &Rav1dFrameData,
  row: FrameTileRow,
) -> Rav1dResult {
  let sequence = frame.seq_hdr()?;
  let header = frame.frame_hdr()?;
  let shift = u32::from(sequence.sb128).wrapping_add(4);
  task.b.y = row.superblock_row.wrapping_shl(shift);
  let row_end = task.b.y.wrapping_add(frame.sb_step).wrapping_shr(1);
  let Ok(tile_row) = console_core_number_conversion::wrapped::<_, i32>(row.tile_row);

  match header.use_ref_frame_mvs {
    0 => {}
    _ => {
      c.dsp.refmvs.load_tmvs.call(
        &frame.rf,
        &frame.ref_mvs,
        tile_row,
        TemporalMvRegion {
          columns: 0..frame.bw.wrapping_shr(1),
          rows: task.b.y.wrapping_shr(1)..row_end,
        },
      )?;
    }
  }

  for column in 0..row.columns {
    task.tile_index = row.tile_row.wrapping_mul(row.columns).wrapping_add(column);
    rav1d_decode_tile_sbrow(c, task, frame).map_err(|()| EINVAL)?;
  }

  match header.frame_type.is_inter_or_switch().then_some(()) {
    Some(()) => {
      c.dsp.refmvs.save_tmvs.call(
        &task.rt,
        &frame.rf,
        &frame.mvs,
        TemporalMvRegion {
          columns: 0..frame.bw.wrapping_shr(1),
          rows: task.b.y.wrapping_shr(1)..row_end,
        },
      )?;
    }
    None => {}
  }

  let Ok(backend) = frame.bd_fn();
  (backend.filter_sbrow)(c, frame, task, row.superblock_row)
}

fn rav1d_decode_frame_main(c: &Rav1dContext, f: &mut Rav1dFrameData) -> Rav1dResult {
  let task_slot = match c.tc.as_ref() {
    [single] => single,
    [] | [_, _, ..] => return Err(EINVAL),
  };
  let thread = match &task_slot.task {
    Rav1dContextTaskType::Single(task) => task,
    Rav1dContextTaskType::Worker => return Err(EINVAL),
  };
  let mut task = thread.lock();
  let header = f.frame_hdr()?;
  let row_starts = header.tiling.row_start_sb;
  let rows = u32::from(header.tiling.rows);
  let columns = u32::from(header.tiling.cols);
  let frame_type = header.frame_type;
  let context_width = u64::try_from(f.sb128w).map_err(|_invalid_width| EINVAL)?;
  let context_count = context_width.wrapping_mul(u64::from(rows));
  let Ok(context_end) = console_core_number_conversion::index(context_count);
  let contexts = f.a.get_mut(..context_end).ok_or(EINVAL)?;

  for context in contexts {
    let Ok(()) = reset_context(context, frame_type, ContextInitialization::Full);
  }

  let Ok(row_boundaries) = console_core_number_conversion::index(rows.wrapping_add(1));
  let boundaries = row_starts.get(..row_boundaries).ok_or(EINVAL)?;
  let Ok(stop) = c.stop_token();

  for (tile_row, row_bounds) in (0_u32..).zip(boundaries.windows(2)) {
    let [start, end] = <[u16; 2]>::try_from(row_bounds).map_err(|_incomplete_row| EINVAL)?;
    let frame_end = i32::from(end).min(f.sbh);

    for superblock_row in i32::from(start)..frame_end {
      match &stop {
        Some(token) => token.check().map_err(|_stop_requested| ECANCELED)?,
        None => {}
      }

      let row = FrameTileRow {
        tile_row,
        superblock_row,
        columns,
      };
      decode_frame_row(c, &mut task, f, row)?;
    }
  }

  Ok(())
}

fn check_frame_reference_progress(frame: &Rav1dFrameData) -> Rav1dResult {
  for reference in &frame.refp {
    let Ok(counters) = reference.progress();

    match (&reference.p.frame_hdr, counters) {
      (None, _) => {}
      (Some(_), Some([_, reconstruction])) => {
        let position = reconstruction.load(Ordering::SeqCst);

        match position {
          FRAME_ERROR => return Err(EINVAL),
          _ => {}
        }
      }
      (Some(_), None) => return Err(EINVAL),
    }
  }

  Ok(())
}

fn finalize_frame_cdf(
  frame: &mut Rav1dFrameData,
  outcome: Rav1dResult,
) -> Result<(), console_core_never::Never> {
  match frame
    .frame_hdr
    .as_ref()
    .map(|header| header.refresh_context)
  {
    None | Some(0) => {}
    Some(_) => {
      let Ok(progress) = frame.out_cdf.progress();
      let Ok(tile_error) = console_core_number_conversion::wrapped::<_, u32>(TILE_ERROR);
      let position = match outcome {
        Ok(()) => 1,
        Err(_decode_error) => tile_error,
      };

      match progress {
        Some(counter) => counter.store(position, Ordering::SeqCst),
        None => {}
      }

      drop(mem::take(&mut frame.out_cdf));
    }
  }

  Ok(())
}

pub(crate) fn rav1d_decode_frame_exit(
  c: &Rav1dContext,
  fc: &Rav1dFrameContext,
  outcome: Rav1dResult,
) -> Rav1dResult {
  let tasks = &fc.task_thread;
  let mut frame = fc.data.write();

  match &frame.sr_cur.p.data {
    Some(_) => tasks.error.store(0, Ordering::Relaxed),
    None => {}
  }

  let checked = match (outcome, c.fc.len(), c.strictness >= Strictness::Strict) {
    (Err(error), 2.., _) => {
      frame.frame_thread.cf.get_mut().fill_with(Default::default);
      Err(error)
    }
    (Ok(()), 2.., true) => match check_frame_reference_progress(&frame) {
      Ok(()) => Ok(()),
      Err(error) => {
        tasks.error.store(1, Ordering::SeqCst);

        let Ok(counters) = frame.sr_cur.progress();

        match counters {
          Some([_, reconstruction]) => reconstruction.store(FRAME_ERROR, Ordering::SeqCst),
          None => {}
        }

        Err(error)
      }
    },
    (known, _, _) => known,
  };

  drop(mem::take(&mut frame.refp));
  drop(mem::take(&mut frame.ref_mvs));
  drop(mem::take(&mut frame.cur));
  drop(mem::take(&mut frame.sr_cur));
  drop(mem::take(&mut *fc.in_cdf.write()));
  let Ok(()) = finalize_frame_cdf(&mut frame, checked);
  drop(mem::take(&mut frame.cur_segmap));
  drop(mem::take(&mut frame.prev_segmap));
  drop(mem::take(&mut frame.mvs));
  drop(mem::take(&mut frame.seq_hdr));
  drop(mem::take(&mut frame.frame_hdr));
  frame.tiles.clear();
  *tasks.retval.lock() = match checked {
    Ok(()) => None,
    Err(error) => Some(error),
  };
  tasks.finished.store(true, Ordering::SeqCst);
  checked
}

pub(crate) fn rav1d_publish_tile_cdf(
  f: &Rav1dFrameData,
  frame_hdr: &Rav1dFrameHeader,
) -> Rav1dResult {
  let mut output_cdf = f.out_cdf.cdf_write()?;
  let Ok(update_tile) = console_core_number_conversion::index(frame_hdr.tiling.update);
  let tile = f.ts.get(update_tile).ok_or(EINVAL)?;
  let state = tile.context.try_lock().ok_or(EINVAL)?;
  let Ok(()) = rav1d_cdf_thread_update(frame_hdr, &mut output_cdf, &state.cdf);
  Ok(())
}

fn recover_frame_workers(
  context: &Rav1dContext,
  frame: &Rav1dFrameContext,
  mut scheduler: MutexGuard<'_, ()>,
) -> Result<(), console_core_never::Never> {
  let control = &context.task_thread;
  context.flush.store(true, Ordering::SeqCst);
  control.cond.notify_all();

  for worker in context.tc.iter() {
    scheduler = wait_while(&worker.thread_data.cond, scheduler, |_state| {
      matches!(worker.flush_state(), Ok(FlushState::Waiting))
    });
  }

  let Ok(()) = frame.task_thread.tasks.clear();
  control.first.store(0, Ordering::SeqCst);
  let Ok(frame_count) = console_core_number_conversion::wrapped::<_, u32>(context.fc.len());
  let Ok(()) = control.cur.set(frame_count);
  control.reset_task_cur.store(None, Ordering::SeqCst);
  control.cond_signaled.store(0, Ordering::SeqCst);
  context.flush.store(false, Ordering::SeqCst);
  drop(scheduler);
  Ok(())
}

fn await_frame_reconstruction(
  context: &Rav1dContext,
  frame: &Rav1dFrameContext,
  queued: Rav1dResult,
) -> Rav1dResult {
  let control = &context.task_thread;
  let tasks = &frame.task_thread;
  let [reconstruction, _entropy] = &tasks.done;
  let waiting = control.lock.lock();
  control.cond.notify_one();
  queued?;

  let pending = || {
    reconstruction.load(Ordering::SeqCst) == 0 || tasks.task_counter.load(Ordering::SeqCst) > 0
  };
  let scheduler = wait_while(&tasks.cond, waiting, |_state| {
    !control.panicked.load(Ordering::SeqCst) && pending()
  });

  match pending().then_some(()) {
    Some(()) => {
      let Ok(()) = recover_frame_workers(context, frame, scheduler);
      Err(EGeneric)
    }
    None => {
      drop(scheduler);
      let result = tasks.retval.try_lock().ok_or(EINVAL)?;

      match *result {
        Some(error) => Err(error),
        None => Ok(()),
      }
    }
  }
}

fn reconstruct_frame(context: &Rav1dContext, frame: &Rav1dFrameContext) -> Rav1dResult {
  let mut data = frame.data.try_write().ok_or(EINVAL)?;

  {
    let input = frame.in_cdf()?;
    rav1d_decode_frame_init_cdf(context, frame, &mut data, &input)?;
  }

  match context.tc.len() {
    0 => Err(EINVAL),
    1 => {
      rav1d_decode_frame_main(context, &mut data)?;
      let header = data.frame_hdr()?;
      let Ok(update_set) = frame.task_thread.update_set.get();

      match (header.refresh_context, update_set) {
        (0, _) | (_, false) => Ok(()),
        (_, true) => rav1d_publish_tile_cdf(&data, header),
      }
    }
    2.. => {
      let queued = rav1d_task_create_tile_sbrow(frame, &data, TileQueueStage::Reconstruction);
      drop(data);
      await_frame_reconstruction(context, frame, queued)
    }
  }
}

pub(crate) fn rav1d_decode_frame(c: &Rav1dContext, fc: &Rav1dFrameContext) -> Rav1dResult {
  match c.fc.len() {
    1 => {}
    0 | 2.. => return Err(EINVAL),
  }

  let result = match rav1d_decode_frame_init(c, fc) {
    Ok(()) => reconstruct_frame(c, fc),
    Err(error) => Err(error),
  };
  rav1d_decode_frame_exit(c, fc, result)
}

#[derive(Clone, Copy)]
struct ResolutionScale {
  input: i32,
  output: i32,
}

fn get_upscale_x0(
  ResolutionScale { input, output }: ResolutionScale,
  step: i32,
) -> Rav1dResult<i32> {
  let difference = output.wrapping_sub(input);
  let error = output
    .wrapping_mul(step)
    .wrapping_sub(input.wrapping_shl(14));
  let offset = difference
    .wrapping_shl(13)
    .wrapping_neg()
    .wrapping_add(output.wrapping_shr(1));
  let prediction = offset.checked_div(output).ok_or(EINVAL)?;
  let position = prediction
    .wrapping_add(128)
    .wrapping_sub(error.wrapping_div(2));
  Ok(position & 0x3fff)
}

fn scale_factor(ResolutionScale { input, output }: ResolutionScale) -> Rav1dResult<i32> {
  let scaled = input.wrapping_shl(14).wrapping_add(output.wrapping_shr(1));
  scaled.checked_div(output).ok_or(EINVAL)
}

fn discard_submitted_frame(
  fc: &Rav1dFrameContext,
  f: &mut Rav1dFrameData,
  out: &mut Rav1dThreadPicture,
  cached_error_props: &mut Rav1dDataProps,
  m: &Rav1dDataProps,
) -> core::result::Result<(), console_core_never::Never> {
  {
    fc.task_thread.error.store(1, Ordering::Relaxed);
    drop(mem::take(&mut *fc.in_cdf.write()));

    match ::core::primitive::bool::then_some(
      f.frame_hdr
        .as_ref()
        .is_some_and(|hdr| hdr.refresh_context != 0),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let _ = mem::take(&mut f.out_cdf);
      }
      ::core::option::Option::None => {}
    }

    for (reference, motion) in f.refp.iter_mut().zip(&mut f.ref_mvs) {
      match &reference.p.frame_hdr {
        Some(_) => drop(mem::take(reference)),
        None => {}
      }

      drop(mem::take(motion));
    }

    let _ = mem::take(out);
    let _ = mem::take(&mut f.cur);
    let _ = mem::take(&mut f.sr_cur);
    let _ = mem::take(&mut f.mvs);
    let _ = mem::take(&mut f.seq_hdr);
    let _ = mem::take(&mut f.frame_hdr);
    *cached_error_props = m.clone();

    f.tiles.clear();
    fc.task_thread.finished.store(true, Ordering::SeqCst);
  };

  Ok(())
}

fn configure_picture_scaling(frame: &mut Rav1dFrameData) -> Rav1dResult {
  let luma = ResolutionScale {
    input: frame.cur.p.w,
    output: frame.sr_cur.p.p.w,
  };
  let horizontal = i32::from(frame.cur.p.layout != Rav1dPixelLayout::I444);
  let Ok(shift) = console_core_number_conversion::wrapped::<_, u32>(horizontal);
  let chroma = ResolutionScale {
    input: luma.input.wrapping_add(horizontal).wrapping_shr(shift),
    output: luma.output.wrapping_add(horizontal).wrapping_shr(shift),
  };
  let luma_step = scale_factor(luma)?;
  let chroma_step = scale_factor(chroma)?;
  let luma_start = get_upscale_x0(luma, luma_step)?;
  let chroma_start = get_upscale_x0(chroma, chroma_step)?;
  frame.resize_step = [luma_step, chroma_step];
  frame.resize_start = [luma_start, chroma_start];

  Ok(())
}

fn initialize_submission_geometry(
  c: &Rav1dContext,
  fc: &Rav1dFrameContext,
  frame: &mut Rav1dFrameData,
) -> Rav1dResult<crate::src::internal::ScratchParallelism> {
  let header = frame.frame_hdr()?;
  let [width, _scaled] = header.size.width;
  let height = header.size.height;
  let tiles = u32::from(header.tiling.cols).wrapping_mul(u32::from(header.tiling.rows));
  let sequence = frame.seq_hdr()?;
  let block_shift = u32::from(sequence.sb128);
  frame.w4 = width.wrapping_add(3).wrapping_shr(2);
  frame.h4 = height.wrapping_add(3).wrapping_shr(2);
  frame.bw = width.wrapping_add(7).wrapping_shr(3).wrapping_shl(1);
  frame.bh = height.wrapping_add(7).wrapping_shr(3).wrapping_shl(1);
  frame.sb128w = frame.bw.wrapping_add(31).wrapping_shr(5);
  frame.sb128h = frame.bh.wrapping_add(31).wrapping_shr(5);
  let shift = 4u32.wrapping_add(block_shift);
  let Ok(signed_shift) = console_core_number_conversion::wrapped::<_, i32>(shift);
  frame.sb_shift = signed_shift;
  frame.sb_step = 16_i32.wrapping_shl(block_shift);
  frame.sbh = frame
    .bh
    .wrapping_add(frame.sb_step)
    .wrapping_sub(1)
    .wrapping_shr(shift);
  let padded_width = frame.bw.wrapping_add(31) & !31;
  let stride = u32::try_from(padded_width).map_err(|_negative_stride| EINVAL)?;
  frame.b4_stride = stride;
  let bit_depth = u32::from(frame.cur.p.bpc);
  frame.bitdepth_max = 1_i32.wrapping_shl(bit_depth).wrapping_sub(1);
  fc.task_thread.error.store(0, Ordering::Relaxed);
  let workers = u32::try_from(c.tc.len()).map_err(|_too_many_workers| EINVAL)?;
  let Ok(tile_count) = console_core_number_conversion::wrapped::<_, i32>(tiles);
  let passes_shift = u32::from(c.fc.len() > 1);
  let tasks = tile_count
    .wrapping_add(frame.sbh)
    .wrapping_shl(passes_shift);
  fc.task_thread.task_counter.store(tasks, Ordering::SeqCst);

  Ok(crate::src::internal::ScratchParallelism { workers, tiles })
}

fn refreshed_references<'a>(
  flags: u8,
  references: &'a mut [crate::src::internal::Rav1dContextRefs; 8],
  cdfs: &'a mut [CdfThreadContext; 8],
) -> Result<
  impl Iterator<
    Item = (
      &'a mut crate::src::internal::Rav1dContextRefs,
      &'a mut CdfThreadContext,
    ),
  >,
  console_core_never::Never,
> {
  Ok(
    (0u32..8)
      .zip(references.iter_mut().zip(cdfs))
      .filter_map(move |(bit, slot)| (flags & 1u8.wrapping_shl(bit) != 0).then_some(slot)),
  )
}

fn publish_reference_frames(
  context: &Rav1dFrameContext,
  frame: &Rav1dFrameData,
  references: &mut [crate::src::internal::Rav1dContextRefs; 8],
  cdfs: &mut [CdfThreadContext; 8],
) -> Rav1dResult {
  let header = frame.frame_hdr()?;
  let Ok(refreshed) = refreshed_references(header.refresh_frame_flags, references, cdfs);

  for (reference, cdf) in refreshed {
    match reference.p.p.frame_hdr.as_ref() {
      Some(_) => drop(mem::take(&mut reference.p)),
      None => {}
    }

    reference.p = frame.sr_cur.clone();
    *cdf = match (header.refresh_context != 0).then_some(()) {
      Some(()) => frame.out_cdf.clone(),
      None => {
        let current = context.in_cdf()?;
        current.clone()
      }
    };
    reference.segmap = frame.cur_segmap.clone();
    drop(mem::take(&mut reference.refmvs));
    reference.refmvs = (!header.allow_intrabc).then(|| frame.mvs.clone()).flatten();
    reference.refpoc = frame.refpoc;
  }

  Ok(())
}

fn clear_reference_frames(
  flags: u8,
  references: &mut [crate::src::internal::Rav1dContextRefs; 8],
  cdfs: &mut [CdfThreadContext; 8],
) -> Result<(), console_core_never::Never> {
  let Ok(refreshed) = refreshed_references(flags, references, cdfs);

  for (reference, cdf) in refreshed {
    match reference.p.p.frame_hdr.as_ref() {
      Some(_) => drop(mem::take(&mut reference.p)),
      None => {}
    }

    drop(mem::take(cdf));
    drop(reference.segmap.take());
    drop(reference.refmvs.take());
  }

  Ok(())
}

fn initialize_submission_cdf(
  context: &Rav1dFrameContext,
  header: &Rav1dFrameHeader,
  previous: &[CdfThreadContext; 8],
) -> Rav1dResult {
  let initial = match (header.primary_ref_frame == RAV1D_PRIMARY_REF_NONE).then_some(()) {
    Some(()) => {
      let Ok(provided) = rav1d_cdf_thread_init_static(header.quant.yac);
      provided
    }
    None => {
      let Ok(position) = console_core_number_conversion::index(header.primary_ref_frame);
      let &reference = header.refidx.get(position).ok_or(EINVAL)?;
      let Ok(index) = console_core_number_conversion::wrapped_index(reference);
      let source = previous.get(index).ok_or(EINVAL)?;
      source.clone()
    }
  };
  let mut storage = context.in_cdf.try_write().ok_or(EINVAL)?;
  *storage = initial;

  Ok(())
}

fn motion_grid(size: Size<i32>) -> Result<Size<i32>, console_core_never::Never> {
  Ok(Size {
    width: size.width.wrapping_add(7).wrapping_shr(3).wrapping_shl(1),
    height: size.height.wrapping_add(7).wrapping_shr(3).wrapping_shl(1),
  })
}

fn initialize_reference_motion(
  frame: &mut Rav1dFrameData,
  references: &[crate::src::internal::Rav1dContextRefs; 8],
  coded_widths: &[i32; 7],
  parallelism: &crate::src::internal::ScratchParallelism,
) -> Rav1dResult {
  let header = frame.frame_hdr.as_deref().ok_or(EINVAL)?;

  match (header.frame_type.is_inter_or_switch() || header.allow_intrabc).then_some(()) {
    Some(()) => {}
    None => {
      frame.mvs = None;
      frame.ref_mvs.fill_with(Default::default);
      return Ok(());
    }
  }

  let superblock_rows = u64::try_from(frame.sb128h).map_err(|_negative_height| EINVAL)?;
  let length = superblock_rows
    .wrapping_mul(16)
    .wrapping_mul(u64::from(frame.b4_stride.wrapping_shr(1)));
  let motion = crate::src::disjoint_mut::dm_arc_try_new(length, Default::default(), parallelism)?;
  frame.mvs = Some(motion);

  match header.allow_intrabc.then_some(()) {
    Some(()) => frame.refpoc.fill(0),
    None => {
      for (reference, output) in frame.refp.iter().zip(&mut frame.refpoc) {
        let reference_header = reference.p.frame_hdr.as_deref().ok_or(EINVAL)?;
        let Ok(frame_offset) =
          console_core_number_conversion::wrapped::<_, u32>(reference_header.frame_offset);
        *output = frame_offset;
      }
    }
  }

  match header.use_ref_frame_mvs {
    0 => {
      frame.ref_mvs.fill_with(Default::default);
      return Ok(());
    }
    _ => {}
  }

  let current = Size {
    width: frame.bw,
    height: frame.bh,
  };

  for position in 0u32..7 {
    let Ok(slot) = console_core_number_conversion::index(position);
    let &reference = header.refidx.get(slot).ok_or(EINVAL)?;
    let Ok(index) = console_core_number_conversion::wrapped_index(reference);
    let &width = coded_widths.get(slot).ok_or(EINVAL)?;
    let picture = frame.refp.get(slot).ok_or(EINVAL)?;
    let Ok(grid) = motion_grid(Size {
      width,
      height: picture.p.p.h,
    });
    let output = frame.ref_mvs.get_mut(slot).ok_or(EINVAL)?;
    *output = match (grid == current).then_some(()) {
      Some(()) => {
        let source = references.get(index).ok_or(EINVAL)?;
        source.refmvs.clone()
      }
      None => None,
    };
    let source = references.get(index).ok_or(EINVAL)?;
    let positions = frame.refrefpoc.get_mut(slot).ok_or(EINVAL)?;
    *positions = source.refpoc;
  }

  Ok(())
}

fn initialize_segment_maps(
  frame: &mut Rav1dFrameData,
  references: &[crate::src::internal::Rav1dContextRefs; 8],
  coded_widths: &[i32; 7],
  parallelism: &crate::src::internal::ScratchParallelism,
) -> Rav1dResult {
  let header = frame.frame_hdr.as_deref().ok_or(EINVAL)?;

  match header.segmentation.enabled {
    0 => {
      frame.cur_segmap = None;
      frame.prev_segmap = None;
      return Ok(());
    }
    _ => {}
  }

  frame.prev_segmap = None;

  match (header.segmentation.temporal != 0 || header.segmentation.update_map == 0).then_some(()) {
    Some(()) => {
      let Ok(slot) = console_core_number_conversion::index(header.primary_ref_frame);
      let &width = coded_widths.get(slot).ok_or(EINVAL)?;
      let picture = frame.refp.get(slot).ok_or(EINVAL)?;
      let Ok(grid) = motion_grid(Size {
        width,
        height: picture.p.p.h,
      });

      match (grid
        == (Size {
          width: frame.bw,
          height: frame.bh,
        }))
      .then_some(())
      {
        Some(()) => {
          let &reference = header.refidx.get(slot).ok_or(EINVAL)?;
          let Ok(index) = console_core_number_conversion::wrapped_index(reference);
          let source = references.get(index).ok_or(EINVAL)?;
          frame.prev_segmap = source.segmap.clone();
        }
        None => {}
      }
    }
    None => {}
  }

  let current = match (
    (header.segmentation.update_map != 0).then_some(()),
    &frame.prev_segmap,
  ) {
    (None, Some(previous)) => previous.clone(),
    (Some(()), _) | (None, None) => {
      let rows = u64::try_from(frame.sb128h).map_err(|_negative_height| EINVAL)?;
      let length = u64::from(frame.b4_stride)
        .wrapping_mul(32)
        .wrapping_mul(rows);
      let allocation =
        crate::src::disjoint_mut::dm_arc_try_new(length, Default::default(), parallelism)?;
      allocation
    }
  };
  frame.cur_segmap = Some(current);

  Ok(())
}

enum ReferenceSize {
  Suitable,
  OutsideLimits,
}

fn reference_fits_frame(
  reference: Size<i32>,
  coded: Size<i32>,
) -> Result<ReferenceSize, console_core_never::Never> {
  Ok(
    match (coded.width.wrapping_mul(2) >= reference.width
      && coded.height.wrapping_mul(2) >= reference.height
      && coded.width <= reference.width.wrapping_mul(16)
      && coded.height <= reference.height.wrapping_mul(16))
    .then_some(())
    {
      Some(()) => ReferenceSize::Suitable,
      None => ReferenceSize::OutsideLimits,
    },
  )
}

fn configure_reference_scale(
  [horizontal, vertical]: &mut [ScalableMotionParams; 2],
  reference: Size<i32>,
  coded: Size<i32>,
) -> Rav1dResult {
  match (reference == coded).then_some(()) {
    Some(()) => {
      vertical.scale = 0;
      horizontal.scale = 0;
    }
    None => {
      let input_scale = scale_factor(ResolutionScale {
        input: reference.width,
        output: coded.width,
      })?;
      horizontal.scale = input_scale;
      let row_scale = scale_factor(ResolutionScale {
        input: reference.height,
        output: coded.height,
      })?;
      vertical.scale = row_scale;
      horizontal.step = horizontal.scale.wrapping_add(8).wrapping_shr(4);
      vertical.step = vertical.scale.wrapping_add(8).wrapping_shr(4);
    }
  }

  Ok(())
}

fn launch_submitted_frame(
  context: &Rav1dContext,
  frame: &Rav1dFrameContext,
  state: &mut Rav1dState,
  refreshed: u8,
) -> Rav1dResult {
  match context.fc.len() {
    1 => {
      let decoded = rav1d_decode_frame(context, frame);

      match decoded {
        Ok(()) => Ok(()),
        Err(fault) => {
          drop(mem::take(&mut state.out));
          let Ok(()) = clear_reference_frames(refreshed, &mut state.refs, &mut state.cdf);
          let mut data = frame.data.try_write().ok_or(EINVAL)?;
          let Ok(()) = discard_submitted_frame(
            frame,
            &mut data,
            &mut state.out,
            &mut state.cached_error_props,
            &state.in_0.m,
          );
          Err(fault)
        }
      }
    }
    _ => match rav1d_task_frame_init(context, frame) {
      Ok(()) => Ok(()),
      Err(fault) => rav1d_decode_frame_exit(context, frame, Err(fault)),
    },
  }
}

enum PrimaryReference {
  NotRequired,
  Ready,
  MissingPicture,
}

fn inspect_primary_reference(
  header: &Rav1dFrameHeader,
  references: &[crate::src::internal::Rav1dContextRefs; 8],
) -> Rav1dResult<PrimaryReference> {
  match header.primary_ref_frame {
    RAV1D_PRIMARY_REF_NONE => return Ok(PrimaryReference::NotRequired),
    _ => {}
  }

  let Ok(position) = console_core_number_conversion::index(header.primary_ref_frame);
  let &reference = header.refidx.get(position).ok_or(EINVAL)?;
  let Ok(index) = console_core_number_conversion::wrapped_index(reference);
  let primary = references.get(index).ok_or(EINVAL)?;

  Ok(match primary.p.p.data.as_ref() {
    Some(_) => PrimaryReference::Ready,
    None => PrimaryReference::MissingPicture,
  })
}

pub fn rav1d_submit_frame(c: &Rav1dContext, state: &mut Rav1dState) -> Rav1dResult {
  let (fc, out, _task_thread_lock) = match ::core::primitive::bool::then_some(c.fc.len() > 1, ())
  {
    ::core::option::Option::Some(()) => {
      let FrameReservation {
        frame,
        index,
        scheduler,
      } = reserve_delayed_frame(c, state)?;
      let Ok(index_2) = console_core_number_conversion::index(index);
      let delayed = state
        .frame_thread
        .out_delayed
        .get_mut(index_2)
        .ok_or(EINVAL)?;
      (frame, delayed, Some(scheduler))
    }
    ::core::option::Option::None => (
      &*match <[_]>::get(&*c.fc, 0) {
        ::core::option::Option::Some(indexed_value_3) => indexed_value_3,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
        }
      },
      &mut state.out,
      None,
    ),
  };

  let mut frame_guard = fc.data.try_write().ok_or(EINVAL)?;
  let f = &mut *frame_guard;
  let seq_hdr = state.seq_hdr.as_ref().ok_or(EINVAL)?;
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "a pending frame retains the shared stable Rust and C sequence-header representations for its output picture"
    )
  )]
  let retained_sequence = seq_hdr.clone();
  f.seq_hdr = Some(retained_sequence);
  f.frame_hdr = mem::take(&mut state.frame_hdr);

  let bpc = u8::wrapping_add(8, u8::wrapping_mul(2, seq_hdr.hbd));
  let Ok(get) = Rav1dBitDepthDSPContext::get(bpc);

  match get {
    Some(dsp) => f.dsp = dsp,
    None => {
      writeln!(c.logger, "Compiled without support for {bpc}-bit decoding",);
      let Ok(()) =
        discard_submitted_frame(fc, f, out, &mut state.cached_error_props, &state.in_0.m);
      return Err(ENOPROTOOPT);
    }
  };

  let mut ref_coded_width = <[i32; 7]>::default();
  let frame_hdr = f.frame_hdr.as_deref().ok_or(EINVAL)?;

  let Ok(frame_hdr_cols) = console_core_number_conversion::wrapped_index(frame_hdr.tiling.cols);
  let Ok(frame_hdr_rows) = console_core_number_conversion::wrapped_index(frame_hdr.tiling.rows);
  rav1d_disjoint_mut::set_tile_concurrency(frame_hdr_cols.wrapping_mul(frame_hdr_rows));

  match frame_hdr.frame_type {
    Rav1dFrameType::Key | Rav1dFrameType::Intra => {}
    Rav1dFrameType::Inter | Rav1dFrameType::Switch => {
      let primary = inspect_primary_reference(frame_hdr, &state.refs)?;

      match primary {
        PrimaryReference::NotRequired | PrimaryReference::Ready => {}
        PrimaryReference::MissingPicture => {
          let Ok(()) =
            discard_submitted_frame(fc, f, out, &mut state.cached_error_props, &state.in_0.m);
          return Err(EINVAL);
        }
      }

      let [width, _scaled] = frame_hdr.size.width;
      let dimensions = Size {
        width,
        height: frame_hdr.size.height,
      };

      for (position, &reference) in (0u32..7).zip(&frame_hdr.refidx) {
        let Ok(index) = console_core_number_conversion::wrapped_index(reference);
        let source = state.refs.get(index).ok_or(EINVAL)?;
        let parameters = &source.p.p.p;
        let reference_size = Size {
          width: parameters.w,
          height: parameters.h,
        };
        let Ok(fits) = reference_fits_frame(reference_size, dimensions);

        match (
          fits,
          (source.p.p.data.is_some()
            && seq_hdr.layout == parameters.layout
            && bpc == parameters.bpc)
            .then_some(()),
        ) {
          (ReferenceSize::Suitable, Some(())) => {}
          (ReferenceSize::OutsideLimits, _) | (ReferenceSize::Suitable, None) => {
            let Ok(()) =
              discard_submitted_frame(fc, f, out, &mut state.cached_error_props, &state.in_0.m);
            return Err(EINVAL);
          }
        }

        let Ok(slot) = console_core_number_conversion::index(position);
        let destination = f.refp.get_mut(slot).ok_or(EINVAL)?;
        *destination = source.p.clone();
        let reference_header = source.p.p.frame_hdr.as_deref().ok_or(EINVAL)?;
        let [coded, _upscaled] = reference_header.size.width;
        let recorded_width = ref_coded_width.get_mut(slot).ok_or(EINVAL)?;
        *recorded_width = coded;

        let scaling = f.svc.get_mut(slot).ok_or(EINVAL)?;
        configure_reference_scale(scaling, reference_size, dimensions)?;
        let warp = frame_hdr.gmv.get(slot).ok_or(EINVAL)?;
        let [horizontal, _vertical] = scaling;
        let allowed = f.gmv_warp_allowed.get_mut(slot).ok_or(EINVAL)?;
        *allowed = u8::from(
          warp.r#type > Rav1dWarpedMotionType::Translation
            && !frame_hdr.force_integer_mv
            && rav1d_get_shear_params(warp) == Ok(WarpValidity::Usable)
            && horizontal.scale == 0,
        );
      }
    }
  }

  initialize_submission_cdf(fc, frame_hdr, &state.cdf)?;

  match ::core::primitive::bool::then_some(frame_hdr.refresh_context != 0, ()) {
    ::core::option::Option::Some(()) => {
      let threading = match c.fc.len() {
        0 | 1 => CdfThreading::Serial,
        _ => CdfThreading::ParallelFrames,
      };
      let res = rav1d_cdf_thread_alloc(threading);

      match res {
        Err(e) => {
          let Ok(()) =
            discard_submitted_frame(fc, f, out, &mut state.cached_error_props, &state.in_0.m);
          return Err(e);
        }
        Ok(res_2) => {
          f.out_cdf = res_2;
        }
      }
    }
    ::core::option::Option::None => {}
  }

  f.tiles.clear();
  mem::swap(&mut f.tiles, &mut state.tiles);
  fc.task_thread
    .finished
    .store(f.tiles.is_empty(), Ordering::SeqCst);

  let itut_t35 = mem::take(&mut state.itut_t35);
  let res = rav1d_thread_picture_alloc(
    c,
    f,
    ThreadPictureRequest {
      content_light: state.content_light.clone(),
      mastering_display: state.mastering_display.clone(),
      max_spatial_id: state.max_spatial_id,
      frame_flags: &mut state.frame_flags,
      bit_depth: bpc,
      itut_t35,
    },
  );

  match res {
    Err(fault) => {
      let Ok(()) =
        discard_submitted_frame(fc, f, out, &mut state.cached_error_props, &state.in_0.m);
      return Err(fault);
    }
    Ok(()) => {}
  }

  let header = f.frame_hdr()?;
  let [coded_width, display_width] = header.size.width;
  let show_frame = header.show_frame;

  match coded_width.cmp(&display_width) {
    cmp::Ordering::Equal => f.cur = f.sr_cur.p.clone(),
    cmp::Ordering::Less | cmp::Ordering::Greater => {
      let allocated = rav1d_picture_alloc_copy(&c.logger, &mut f.cur, coded_width, &f.sr_cur.p);

      match allocated {
        Ok(()) => {}
        Err(fault) => {
          let Ok(()) =
            discard_submitted_frame(fc, f, out, &mut state.cached_error_props, &state.in_0.m);
          return Err(fault);
        }
      }

      configure_picture_scaling(f)?;
    }
  }

  match ::core::primitive::bool::then_some(c.fc.len() == 1, ()) {
    ::core::option::Option::Some(()) => match ::core::primitive::bool::then_some(
      show_frame != 0
        || c.frame_visibility == crate::src::internal::FrameVisibilityPolicy::IncludeInvisible,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        *out = f.sr_cur.clone();
        state.event_flags |=
          <crate::include::dav1d::dav1d::Rav1dEventFlags as ::core::convert::From<
            crate::src::picture::PictureFlags,
          >>::from(f.sr_cur.flags);
      }
      ::core::option::Option::None => {}
    },
    ::core::option::Option::None => {
      *out = f.sr_cur.clone();
    }
  }

  let parallelism = initialize_submission_geometry(c, fc, f)?;
  initialize_reference_motion(f, &state.refs, &ref_coded_width, &parallelism)?;
  initialize_segment_maps(f, &state.refs, &ref_coded_width, &parallelism)?;
  let submitted_header = f.frame_hdr()?;
  let refresh_frame_flags = submitted_header.refresh_frame_flags;
  publish_reference_frames(fc, f, &mut state.refs, &mut state.cdf)?;

  drop(frame_guard);

  launch_submitted_frame(c, fc, state, refresh_frame_flags)
}
