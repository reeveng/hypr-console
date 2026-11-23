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
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::headers::Rav1dRestorationType;
use crate::include::dav1d::headers::Rav1dSequenceHeader;
use crate::include::dav1d::headers::Rav1dTxfmMode;
use crate::include::dav1d::headers::Rav1dWarpedMotionParams;
use crate::include::dav1d::headers::Rav1dWarpedMotionType;
use crate::include::dav1d::headers::SgrIdx;
use crate::include::dav1d::picture::Rav1dPicture;
use crate::src::align::AlignedVec64;
use crate::src::c_arc::CArc;
use crate::src::cdf::CdfMvComponent;
use crate::src::cdf::CdfMvContext;
use crate::src::cdf::CdfThreadContext;
use crate::src::cdf::CdfThreading;
use crate::src::cdf::rav1d_cdf_thread_alloc;
use crate::src::cdf::rav1d_cdf_thread_copy;
use crate::src::cdf::rav1d_cdf_thread_init_static;
use crate::src::cdf::rav1d_cdf_thread_update;
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
use crate::src::extensions::OptionError as _;
use crate::src::internal::Bxy;
use crate::src::internal::ChromaNeighborPrediction;
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
use crate::src::internal::Rav1dTileStateContext;
use crate::src::internal::ScalableMotionParams;
use crate::src::internal::TileStateRef;
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
use console_core_pixel_buffers::RowLayout;
use console_core_synchronization::nonpoison::unpoison;
#[allow(non_camel_case_types)]
use std::array;
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

fn neg_deinterleave(
  diff: u8,
  r#ref: SegmentId,
  max: u8,
) -> core::result::Result<u8, console_core_never::Never> {
  Ok({
    let Ok(ref_2) = console_core_number_conversion::wrapped::<_, u8>({
      let Ok(explicit_value) = r#ref.get();
      explicit_value
    });

    match ::core::primitive::bool::then_some(ref_2 == 0, ()) {
      ::core::option::Option::Some(()) => diff,
      ::core::option::Option::None => {
        match ::core::primitive::bool::then_some(u8::wrapping_add(ref_2, 1) >= max, ()) {
          ::core::option::Option::Some(()) => max.wrapping_sub(u8::wrapping_add(diff, 1)),
          ::core::option::Option::None => {
            match ::core::primitive::bool::then_some(u8::wrapping_mul(2, ref_2) < max, ()) {
              ::core::option::Option::Some(()) => {
                match ::core::primitive::bool::then_some(diff <= u8::wrapping_mul(2, ref_2), ()) {
                  ::core::option::Option::Some(()) => {
                    match ::core::primitive::bool::then_some(diff & 1 != 0, ()) {
                      ::core::option::Option::Some(()) => u8::wrapping_add(
                        ref_2,
                        u8::wrapping_shr(u8::wrapping_add(diff, 1), 1_u32),
                      ),
                      ::core::option::Option::None => {
                        u8::wrapping_sub(ref_2, u8::wrapping_shr(diff, 1_u32))
                      }
                    }
                  }
                  ::core::option::Option::None => diff,
                }
              }
              ::core::option::Option::None => {
                match ::core::primitive::bool::then_some(
                  diff <= u8::wrapping_mul(2, u8::wrapping_sub(u8::wrapping_sub(max, ref_2), 1)),
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    match ::core::primitive::bool::then_some(diff & 1 != 0, ()) {
                      ::core::option::Option::Some(()) => u8::wrapping_add(
                        ref_2,
                        u8::wrapping_shr(u8::wrapping_add(diff, 1), 1_u32),
                      ),
                      ::core::option::Option::None => {
                        u8::wrapping_sub(ref_2, u8::wrapping_shr(diff, 1_u32))
                      }
                    }
                  }
                  ::core::option::Option::None => max.wrapping_sub(u8::wrapping_add(diff, 1)),
                }
              }
            }
          }
        }
      }
    }
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
      let tile = f.tile(t.tile_index).ok_or(EINVAL)?;

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
  let above = frame.above_context(task.above_index).ok_or(EINVAL)?;
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

      match debug_block_info!(frame, task.b) {
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

fn mc_lowest_px(
  dst: &mut c_int,
  by4: c_int,
  bh4: c_int,
  mvy: i16,
  ss_ver: c_int,
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

#[inline(always)]
fn affine_lowest_px(
  t: &Rav1dTaskContext,
  dst: &mut c_int,
  b_dim: &[u8; 4],
  wmp: &Rav1dWarpedMotionParams,
  ss_ver: c_int,
  ss_hor: c_int,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let &[width, height, ..] = b_dim;

    let h_mul = i32::wrapping_shr(4, {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
      numeric_value
    });
    let v_mul = i32::wrapping_shr(4, {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
      numeric_value
    });
    assert!(
      i32::wrapping_mul(i32::from(width), h_mul) & 7 == 0
        && i32::wrapping_mul(i32::from(height), v_mul) & 7 == 0
    );
    let mat = &wmp.matrix;
    let y = i32::wrapping_sub(i32::wrapping_mul(i32::from(height), v_mul), 8);
    let src_y = i32::wrapping_add(
      i32::wrapping_mul(t.b.y, 4),
      i32::wrapping_shl(i32::wrapping_add(y, 4), {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
        numeric_value
      }),
    );
    let mat5_y = i64::wrapping_add(
      i64::wrapping_mul(
        {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i64>(match mat {
            &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
          });
          numeric_value
        },
        {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i64>(src_y);
          numeric_value
        },
      ),
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i64>(match mat {
          &[_, element, ..] => element,
        });
        numeric_value
      },
    );
    let mut x = 0;

    while x < i32::wrapping_mul(i32::from(width), h_mul) {
      let src_x = i32::wrapping_add(
        i32::wrapping_mul(t.b.x, 4),
        i32::wrapping_shl(i32::wrapping_add(x, 4), {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
          numeric_value
        }),
      );
      let mvy = i64::wrapping_shr(
        i64::wrapping_add(
          i64::wrapping_mul(
            {
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped::<_, i64>(match mat {
                  &[_, _, _, _, _explicit_element, ..] => _explicit_element,
                });
              numeric_value
            },
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i64>(src_x);
              numeric_value
            },
          ),
          mat5_y,
        ),
        {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
          numeric_value
        },
      );
      let dy = i32::wrapping_sub(
        {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, i32>(i64::wrapping_shr(mvy, 16_u32));
          numeric_value
        },
        4,
      );

      *dst = cmp::max(*dst, i32::wrapping_add(i32::wrapping_add(dy, 4), 8));
      {
        let numeric_amount = cmp::max(
          8,
          i32::wrapping_sub(i32::wrapping_mul(i32::from(width), h_mul), 8),
        );
        let numeric_target = &mut (x);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };
    }
  };

  Ok(())
}

#[inline(never)]
fn affine_lowest_px_luma(
  t: &Rav1dTaskContext,
  dst: &mut c_int,
  b_dim: &[u8; 4],
  wmp: &Rav1dWarpedMotionParams,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let Ok(()) = affine_lowest_px(t, dst, b_dim, wmp, 0, 0);
  };

  Ok(())
}

#[inline(never)]
fn affine_lowest_px_chroma(
  t: &Rav1dTaskContext,
  layout: Rav1dPixelLayout,
  dst: &mut c_int,
  b_dim: &[u8; 4],
  wmp: &Rav1dWarpedMotionParams,
) -> core::result::Result<(), console_core_never::Never> {
  {
    assert!(layout != Rav1dPixelLayout::I400);

    match ::core::primitive::bool::then_some(layout == Rav1dPixelLayout::I444, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = affine_lowest_px_luma(t, dst, b_dim, wmp);
      }
      ::core::option::Option::None => {
        let Ok(()) = affine_lowest_px(
          t,
          dst,
          b_dim,
          wmp,
          i32::from(layout & Rav1dPixelLayout::I420),
          1,
        );
      }
    };
  };

  Ok(())
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
    coverage.start,
    coverage.end.wrapping_sub(coverage.start),
    motion.y,
    vertical_subsampling,
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

  let tile = f.tile(t.tile_index).ok_or(EINVAL)?;
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

        if reference > 0 {
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

  match debug_block_info!(f, t.b) {
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
    _ => {
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
  let map = &f.cur_segmap.as_ref().ok_or(EINVAL)?.inner;
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
  let seq_hdr = &***f.seq_hdr.as_ref().ok_or(())?;
  use std::fmt;

  struct SignAbs(i32);

  impl fmt::Display for SignAbs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
      let sign = match ::core::primitive::bool::then_some(self.0 < 0, ()) {
        ::core::option::Option::Some(()) => "-",
        ::core::option::Option::None => " ",
      };
      write!(f, "{}{:x}", sign, self.0.abs())
    }
  }

  let mut b_mem = Av1Block::default();
  let b = match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
    ::core::option::Option::Some(()) => {
      let block = f.coded_block_position(t.b).map_err(|_invalid_block| ())?;
      let Ok(index) = console_core_number_conversion::index(block);
      &mut *f.frame_thread.b.index_mut(index)
    }
    ::core::option::Option::None => &mut b_mem,
  };

  let ts = &*match f.tile(t.tile_index) {
    ::core::option::Option::Some(indexed_value) => indexed_value,
    ::core::option::Option::None => return ::core::result::Result::Err(()),
  };
  let ta = &*match f.above_context(t.above_index) {
    ::core::option::Option::Some(indexed_value) => indexed_value,
    ::core::option::Option::None => return ::core::result::Result::Err(()),
  };
  let Ok(bd_fn) = f.bd_fn();
  let Ok(b_dim) = bs.dimensions();
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
  let Ok(bw4) = console_core_number_conversion::wrapped::<_, i32>(match b_dim {
    &[element, ..] => element,
  });
  let Ok(bh4) = console_core_number_conversion::wrapped::<_, i32>(match b_dim {
    &[_, element, ..] => element,
  });
  let w4 = cmp::min(bw4, i32::wrapping_sub(f.bw, t.b.x));
  let h4 = cmp::min(bh4, i32::wrapping_sub(f.bh, t.b.y));
  let cbw4 = i32::wrapping_shr(i32::wrapping_add(bw4, ss_hor), {
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
    numeric_value
  });
  let cbh4 = i32::wrapping_shr(i32::wrapping_add(bh4, ss_ver), {
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
    numeric_value
  });
  let have_left = t.b.x > ts.tiling.col_start;
  let have_top = t.b.y > ts.tiling.row_start;
  let neighbors = NeighborAvailability::from((have_top, have_left));
  let has_chroma = f.cur.p.layout != Rav1dPixelLayout::I400
    && (bw4 > ss_hor || t.b.x & 1 != 0)
    && (bh4 > ss_ver || t.b.y & 1 != 0);
  let frame_type = f.frame_hdr.as_ref().ok_or(())?.frame_type;

  let FrameThreadPassState::First(ts_c) = pass else {
    match &b.ii {
      Av1BlockIntraInter::Intra(intra) => {
        (bd_fn.recon_b_intra)(f, t, None, bs, intra_edge_flags, b, intra).map_err(|_| ())?;

        let y_mode = intra.y_mode;
        let y_mode_nofilt = match ::core::primitive::bool::then_some(y_mode == FILTER_PRED, ()) {
          ::core::option::Option::Some(()) => DC_PRED,
          ::core::option::Option::None => y_mode,
        };
        case_set_al! {
            <32, false>
            l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4); numeric_value }),
            a: (ta, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx4); numeric_value }),
            mode = (y_mode_nofilt, y_mode_nofilt),
            intra = (1, 1),
        }

        match ::core::primitive::bool::then_some(frame_type.is_inter_or_switch(), ()) {
          ::core::option::Option::Some(()) => {
            let ri = (::core::num::Wrapping({
              let offset = *match <[_]>::get(
                &t.rt.r,
                (({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t.b.y);
                  numeric_value
                }) & 31)
                  .wrapping_add(5)
                  .wrapping_add({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4);
                    numeric_value
                  })
                  .wrapping_sub(1),
              ) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              };
              let Ok(row) = console_core_number_conversion::wrapped_index(offset);
              row
            }) + ::core::num::Wrapping({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t.b.x);
              numeric_value
            }))
            .0;
            let r = &mut *f.rf.r.index_mut(
              ri..ri.wrapping_add({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4);
                numeric_value
              }),
            );

            for block in r {
              *{
                let [_explicit_part, ..] = &mut block.r#ref.r#ref;
                _explicit_part
              } = 0;
              block.bs = bs;
            }

            let rr = &*match <[_]>::get(
              &(*match <[_]>::get(
                &t.rt.r,
                (({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t.b.y);
                  numeric_value
                }) & 31)
                  .wrapping_add(5)..,
              ) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              }),
              ..({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4);
                numeric_value
              })
              .wrapping_sub(1),
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            };

            for &row_base in rr {
              let Ok(row) = console_core_number_conversion::wrapped_index(row_base);
              let block = &mut f.rf.r.index_mut(
                (::core::num::Wrapping(row)
                  + ::core::num::Wrapping({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t.b.x);
                    numeric_value
                  }))
                .0
                .wrapping_add({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4);
                  numeric_value
                })
                .wrapping_sub(1),
              );
              *{
                let [_explicit_part, ..] = &mut block.r#ref.r#ref;
                _explicit_part
              } = 0;
              block.bs = bs;
            }
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(has_chroma, ()) {
          ::core::option::Option::Some(()) => {
            case_set_al! {
                <32, false>
                l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbh4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cby4); numeric_value }),
                a: (ta, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbw4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbx4); numeric_value }),
                uvmode = (intra.uv_mode, intra.uv_mode),
            }
          }
          ::core::option::Option::None => {}
        }
      }
      Av1BlockIntraInter::Inter(inter) => {
        match ::core::primitive::bool::then_some(
          frame_type.is_inter_or_switch()
            && inter.comp_type.is_none()
            && inter.motion_mode == MotionMode::Warp,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            let Ok(two_d) = inter.nd.two_d();

            match ::core::primitive::bool::then_some(
              (match two_d.matrix {
                [element, ..] => element,
              }) == i16::MIN,
              (),
            ) {
              ::core::option::Option::Some(()) => {
                t.warpmv.r#type = Rav1dWarpedMotionType::Identity;
              }
              ::core::option::Option::None => {
                t.warpmv.r#type = Rav1dWarpedMotionType::Affine;
                let Ok(two_d_matrix) =
                  console_core_number_conversion::wrapped::<_, i32>(match two_d.matrix {
                    [element, ..] => element,
                  });
                *{
                  let [_, _, _explicit_part, ..] = &mut t.warpmv.matrix;
                  _explicit_part
                } = i32::wrapping_add(two_d_matrix, 0x10000);
                *{
                  let [_, _, _, _explicit_part, ..] = &mut t.warpmv.matrix;
                  _explicit_part
                } = {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(match two_d.matrix {
                      [_, element, ..] => element,
                    });
                  numeric_value
                };
                *{
                  let [_, _, _, _, _explicit_part, ..] = &mut t.warpmv.matrix;
                  _explicit_part
                } = {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(match two_d.matrix {
                      [_, _, element, ..] => element,
                    });
                  numeric_value
                };
                *{
                  let [_, _, _, _, _, _explicit_part, ..] = &mut t.warpmv.matrix;
                  _explicit_part
                } = i32::wrapping_add(
                  {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, i32>(match two_d.matrix {
                        [_, _, _, element, ..] => element,
                      });
                    numeric_value
                  },
                  0x10000,
                );
                let block = Rectangle {
                  origin: Point { x: t.b.x, y: t.b.y },
                  size: Size {
                    width: bw4,
                    height: bh4,
                  },
                };
                let Ok(()) = rav1d_set_affine_mv2d(block, two_d.mv2d, &mut t.warpmv);
                let Ok(WarpValidity::Usable | WarpValidity::Rejected) =
                  rav1d_get_shear_params(&t.warpmv);

                match debug_block_info!(f, t.b) {
                  BlockTrace::Enabled => {
                    println!(
                      "[ {} {} {}\n  {} {} {} ]\n\
                            alpha={}, beta={}, gamma={}, deta={}, mv=y:{},x:{}",
                      SignAbs(match t.warpmv.matrix {
                        [element, ..] => element,
                      }),
                      SignAbs(match t.warpmv.matrix {
                        [_, element, ..] => element,
                      }),
                      SignAbs(match t.warpmv.matrix {
                        [_, _, element, ..] => element,
                      }),
                      SignAbs(match t.warpmv.matrix {
                        [_, _, _, element, ..] => element,
                      }),
                      SignAbs(match t.warpmv.matrix {
                        [_, _, _, _, _explicit_element, ..] => _explicit_element,
                      }),
                      SignAbs(match t.warpmv.matrix {
                        [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
                      }),
                      SignAbs(<::core::primitive::i32 as ::core::convert::From<
                        ::core::primitive::i16,
                      >>::from(t.warpmv.alpha())),
                      SignAbs(<::core::primitive::i32 as ::core::convert::From<
                        ::core::primitive::i16,
                      >>::from(t.warpmv.beta())),
                      SignAbs(<::core::primitive::i32 as ::core::convert::From<
                        ::core::primitive::i16,
                      >>::from(t.warpmv.gamma())),
                      SignAbs(<::core::primitive::i32 as ::core::convert::From<
                        ::core::primitive::i16,
                      >>::from(t.warpmv.delta())),
                      two_d.mv2d.y,
                      two_d.mv2d.x,
                    );
                  }
                  BlockTrace::Disabled => {}
                }
              }
            }
          }
          ::core::option::Option::None => {}
        }

        (bd_fn.recon_b_inter)(f, t, None, bs, b, inter)?;

        let filter = &*match <[_]>::get(&dav1d_filter_dir, {
          let Ok(table_index) = console_core_number_conversion::index(
            crate::src::enum_map::EnumKey::ordinal(inter.filter2d),
          );
          table_index
        }) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => return ::core::result::Result::Err(()),
        };
        case_set_al! {
            <32, false>
            l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4); numeric_value }),
            a: (ta, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx4); numeric_value }),
            filter[0] = (match filter { &[_explicit_element, ..] => _explicit_element }, match filter { &[_explicit_element, ..] => _explicit_element }),
            filter[1] = (match filter { &[_, _explicit_element, ..] => _explicit_element }, match filter { &[_, _explicit_element, ..] => _explicit_element }),
            intra = (0, 0),
        }

        match ::core::primitive::bool::then_some(frame_type.is_inter_or_switch(), ()) {
          ::core::option::Option::Some(()) => {
            let ri = (::core::num::Wrapping({
              let offset = *match <[_]>::get(
                &t.rt.r,
                (({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t.b.y);
                  numeric_value
                }) & 31)
                  .wrapping_add(5)
                  .wrapping_add({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4);
                    numeric_value
                  })
                  .wrapping_sub(1),
              ) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              };
              let Ok(row) = console_core_number_conversion::wrapped_index(offset);
              row
            }) + ::core::num::Wrapping({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t.b.x);
              numeric_value
            }))
            .0;
            let r = &mut *f.rf.r.index_mut(
              ri..ri.wrapping_add({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4);
                numeric_value
              }),
            );

            for block in r {
              *{
                let [_explicit_part, ..] = &mut block.r#ref.r#ref;
                _explicit_part
              } = i8::wrapping_add(
                match inter.r#ref {
                  [element, ..] => element,
                },
                1,
              );
              *{
                let [_explicit_part, ..] = &mut block.mv.mv;
                _explicit_part
              } = match inter.nd.one_d.mv {
                [_explicit_element, ..] => _explicit_element,
              };
              block.bs = bs;
            }

            let rr = &*match <[_]>::get(
              &(*match <[_]>::get(
                &t.rt.r,
                (({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t.b.y);
                  numeric_value
                }) & 31)
                  .wrapping_add(5)..,
              ) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              }),
              ..({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4);
                numeric_value
              })
              .wrapping_sub(1),
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            };

            for &row_base in rr {
              let Ok(row) = console_core_number_conversion::wrapped_index(row_base);
              let block = &mut f.rf.r.index_mut(
                (::core::num::Wrapping(row)
                  + ::core::num::Wrapping({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(t.b.x);
                    numeric_value
                  }))
                .0
                .wrapping_add({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4);
                  numeric_value
                })
                .wrapping_sub(1),
              );
              *{
                let [_explicit_part, ..] = &mut block.r#ref.r#ref;
                _explicit_part
              } = i8::wrapping_add(
                match inter.r#ref {
                  [element, ..] => element,
                },
                1,
              );
              *{
                let [_explicit_part, ..] = &mut block.mv.mv;
                _explicit_part
              } = match inter.nd.one_d.mv {
                [_explicit_element, ..] => _explicit_element,
              };
              block.bs = bs;
            }
          }
          ::core::option::Option::None => {}
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
      }
    }

    return Ok(());
  };

  let ts_c_2 = &mut **ts_c;

  let cw4 = i32::wrapping_shr(i32::wrapping_add(w4, ss_hor), {
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
    numeric_value
  });
  let ch4 = i32::wrapping_shr(i32::wrapping_add(h4, ss_ver), {
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
    numeric_value
  });

  b.bl = bl;
  b.bp = bp;
  let Ok(block_code) = bs.ordinal();
  b.bs = block_code;

  let mut seg = None;
  let mut seg_pred = false;
  let frame_hdr: &Rav1dFrameHeader = f.frame_hdr.as_ref().ok_or(())?;
  let previous_block = Rectangle {
    origin: Point { x: t.b.x, y: t.b.y },
    size: Size {
      width: w4,
      height: h4,
    },
  };
  let previous_segment = || {
    let pitch = i64::from(f.b4_stride);
    let previous_map = f
      .prev_segmap
      .as_ref()
      .map(|previous| previous.inner.as_ref());
    get_prev_frame_segid(frame_hdr, previous_map, previous_block, pitch)
  };

  match ::core::primitive::bool::then_some(frame_hdr.segmentation.enabled != 0, ()) {
    ::core::option::Option::Some(()) => {
      match ::core::primitive::bool::then_some(frame_hdr.segmentation.update_map == 0, ()) {
        ::core::option::Option::Some(()) => {
          let checked_value_3 = previous_segment().map_err(|_previous_segment| ())?;
          b.seg_id = checked_value_3;
          let Ok(b_seg_id) = b.seg_id.get();
          seg = Some(&*match <[_]>::get(
            &frame_hdr.segmentation.seg_data.d,
            match ::console_core_number_conversion::index(b_seg_id) {
              ::core::result::Result::Ok(native_position__) => native_position__,
            },
          ) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => return ::core::result::Result::Err(()),
          });
        }
        ::core::option::Option::None => match ::core::primitive::bool::then_some(
          frame_hdr.segmentation.seg_data.preskip != 0,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            match ::core::primitive::bool::then_some(
              frame_hdr.segmentation.temporal != 0 && {
                let index = u8::wrapping_add(
                  *ta.seg_pred.index({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx4);
                    numeric_value
                  }),
                  *match <[_]>::get(&**t.l.seg_pred.get_mut(), {
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4);
                    numeric_value
                  }) {
                    ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                );
                seg_pred = matches!(
                  rav1d_msac_decode_bool_adapt(
                    &mut ts_c_2.msac,
                    &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.seg_pred, {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped_index(index);
                      numeric_value
                    }) {
                      ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                  ),
                  Ok(EntropyBit::One)
                );
                seg_pred
              },
              (),
            ) {
              ::core::option::Option::Some(()) => {
                let checked_value_4 = previous_segment().map_err(|_previous_segment| ())?;
                b.seg_id = checked_value_4;
              }
              ::core::option::Option::None => {
                let segment_map = f.cur_segmap.as_ref().ok_or(())?;
                let segment_stride = u64::from(f.b4_stride);
                let (pred_seg_id, seg_ctx) =
                  get_cur_frame_segid(t.b, neighbors, &segment_map.inner, segment_stride)
                    .map_err(|_invalid_segment_map| ())?;
                let Ok(diff) = rav1d_msac_decode_symbol_adapt8(
                  &mut ts_c_2.msac,
                  &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.m.seg_id, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped_index(seg_ctx);
                    numeric_value
                  }) {
                    ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                  u8::wrapping_sub(
                    {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u8>(SegmentId::COUNT);
                      numeric_value
                    },
                    1,
                  ),
                );
                let last_active_segid = frame_hdr.segmentation.seg_data.last_active_segid;
                let Ok(last_active_seg_id_plus1) = console_core_number_conversion::wrapped::<_, u8>(
                  i8::wrapping_add(last_active_segid, 1),
                );
                let Ok(mut seg_id) = neg_deinterleave(
                  {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u8>(diff);
                    numeric_value
                  },
                  pred_seg_id,
                  last_active_seg_id_plus1,
                );

                match ::core::primitive::bool::then_some(
                  c.strictness >= Strictness::Strict
                    && seg_id
                      > ({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(
                          last_active_segid.max(0),
                        );
                        numeric_value
                      }),
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    return Err(());
                  }
                  ::core::option::Option::None => {}
                }

                match ::core::primitive::bool::then_some(
                  last_active_segid >= 0
                    && seg_id
                      > ({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u8>(last_active_segid);
                        numeric_value
                      }),
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    seg_id = 0;
                  }
                  ::core::option::Option::None => {}
                }

                let Ok(segment_id) = SegmentId::new(seg_id);
                b.seg_id = segment_id.unwrap_or_default();
              }
            }

            match debug_block_info!(f, t.b) {
              BlockTrace::Enabled => {
                println!("Post-segid[preskip;{}]: r={}", b.seg_id, ts_c_2.msac.rng);
              }
              BlockTrace::Disabled => {}
            }

            let Ok(b_seg_id) = b.seg_id.get();
            seg = Some(&*match <[_]>::get(
              &frame_hdr.segmentation.seg_data.d,
              match ::console_core_number_conversion::index(b_seg_id) {
                ::core::result::Result::Ok(native_position__) => native_position__,
              },
            ) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            });
          }
          ::core::option::Option::None => {}
        },
      }
    }
    ::core::option::Option::None => {
      b.seg_id = Default::default();
    }
  }

  match ::core::primitive::bool::then_some(
    seg
      .map(|seg_10| seg_10.globalmv == 0 && seg_10.r#ref == -1 && seg_10.skip == 0)
      .unwrap_or(true)
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

      match debug_block_info!(f, t.b) {
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

      match debug_block_info!(f, t.b) {
        BlockTrace::Enabled => {
          println!("Post-skip[{}]: r={}", b.skip, ts_c_2.msac.rng);
        }
        BlockTrace::Disabled => {}
      }
    }
  }

  match ::core::primitive::bool::then_some(
    frame_hdr.segmentation.enabled != 0
      && frame_hdr.segmentation.update_map != 0
      && frame_hdr.segmentation.seg_data.preskip == 0,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      match ::core::primitive::bool::then_some(
        b.skip == 0 && frame_hdr.segmentation.temporal != 0 && {
          let index = u8::wrapping_add(
            *ta.seg_pred.index({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx4);
              numeric_value
            }),
            *match <[_]>::get(&**t.l.seg_pred.get_mut(), {
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4);
              numeric_value
            }) {
              ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            },
          );
          seg_pred = matches!(
            rav1d_msac_decode_bool_adapt(
              &mut ts_c_2.msac,
              &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.seg_pred, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(index);
                numeric_value
              }) {
                ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              },
            ),
            Ok(EntropyBit::One)
          );
          seg_pred
        },
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let checked_value_7 = previous_segment().map_err(|_previous_segment| ())?;
          b.seg_id = checked_value_7;
        }
        ::core::option::Option::None => {
          let segment_map = f.cur_segmap.as_ref().ok_or(())?;
          let segment_stride = u64::from(f.b4_stride);
          let (pred_seg_id, seg_ctx) =
            get_cur_frame_segid(t.b, neighbors, &segment_map.inner, segment_stride)
              .map_err(|_invalid_segment_map| ())?;
          b.seg_id = match ::core::primitive::bool::then_some(b.skip != 0, ()) {
            ::core::option::Option::Some(()) => pred_seg_id,
            ::core::option::Option::None => {
              let Ok(diff) = rav1d_msac_decode_symbol_adapt8(
                &mut ts_c_2.msac,
                &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.m.seg_id, {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(seg_ctx);
                  numeric_value
                }) {
                  ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                },
                u8::wrapping_sub(
                  {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u8>(SegmentId::COUNT);
                    numeric_value
                  },
                  1,
                ),
              );
              let last_active_segid = frame_hdr.segmentation.seg_data.last_active_segid;
              let Ok(last_active_seg_id_plus1) = console_core_number_conversion::wrapped::<_, u8>(
                i8::wrapping_add(last_active_segid, 1),
              );
              let Ok(mut seg_id) = neg_deinterleave(
                {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(diff);
                  numeric_value
                },
                pred_seg_id,
                last_active_seg_id_plus1,
              );

              match ::core::primitive::bool::then_some(
                c.strictness >= Strictness::Strict
                  && seg_id
                    > ({
                      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(
                        last_active_segid.max(0),
                      );
                      numeric_value
                    }),
                (),
              ) {
                ::core::option::Option::Some(()) => {
                  return Err(());
                }
                ::core::option::Option::None => {}
              }

              match ::core::primitive::bool::then_some(
                last_active_segid >= 0
                  && seg_id
                    > ({
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u8>(last_active_segid);
                      numeric_value
                    }),
                (),
              ) {
                ::core::option::Option::Some(()) => {
                  seg_id = 0;
                }
                ::core::option::Option::None => {}
              }

              let Ok(segment_id) = SegmentId::new(seg_id);

              segment_id.unwrap_or_default()
            }
          };
        }
      }

      let Ok(b_seg_id) = b.seg_id.get();
      seg = Some(&*match <[_]>::get(
        &frame_hdr.segmentation.seg_data.d,
        match ::console_core_number_conversion::index(b_seg_id) {
          ::core::result::Result::Ok(native_position__) => native_position__,
        },
      ) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => return ::core::result::Result::Err(()),
      });

      match debug_block_info!(f, t.b) {
        BlockTrace::Enabled => {
          println!("Post-segid[postskip;{}]: r={}", b.seg_id, ts_c_2.msac.rng);
        }
        BlockTrace::Disabled => {}
      }
    }
    ::core::option::Option::None => {}
  }

  match (b.skip == 0).then_some(()) {
    Some(()) => {
      let size = Size {
        width: bw4,
        height: bh4,
      };
      read_block_cdef(f, t, &mut ts_c_2.msac, size).map_err(|_invalid_cdef| ())?;
    }
    None => {}
  }

  let not_sb128 = i32::from(seq_hdr.sb128 == 0);

  match ::core::primitive::bool::then_some(
    t.b.x
      & i32::wrapping_shr(31, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(not_sb128);
        numeric_value
      })
      == 0
      && t.b.y
        & i32::wrapping_shr(31, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(not_sb128);
          numeric_value
        })
        == 0,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(prev_qidx) = ts.last_qidx.get();
      let have_delta_q = frame_hdr.delta.q.present != 0
        && (bs
          != (match ::core::primitive::bool::then_some(seq_hdr.sb128 != 0, ()) {
            ::core::option::Option::Some(()) => BlockSize::Bs128x128,
            ::core::option::Option::None => BlockSize::Bs64x64,
          })
          || b.skip == 0);

      let Ok(prev_delta_lf) = ts.last_delta_lf.get();

      match ::core::primitive::bool::then_some(have_delta_q, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(mut delta_q) = console_core_number_conversion::wrapped::<_, i32>({
            let Ok(explicit_value) =
              rav1d_msac_decode_symbol_adapt4(&mut ts_c_2.msac, &mut *ts_c_2.cdf.m.delta_q, 3);
            explicit_value
          });

          match ::core::primitive::bool::then_some(delta_q == 3, ()) {
            ::core::option::Option::Some(()) => {
              let n_bits = u8::wrapping_add(1, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>({
                  let Ok(explicit_value) = rav1d_msac_decode_bools(&mut ts_c_2.msac, 3);
                  explicit_value
                });
                numeric_value
              });
              delta_q = {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(u32::wrapping_add(
                    u32::wrapping_add(
                      {
                        let Ok(explicit_value) =
                          rav1d_msac_decode_bools(&mut ts_c_2.msac, n_bits);
                        explicit_value
                      },
                      1,
                    ),
                    u32::wrapping_shl(1, {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u32>(n_bits);
                      numeric_value
                    }),
                  ));
                numeric_value
              };
            }
            ::core::option::Option::None => {}
          }

          match ::core::primitive::bool::then_some(delta_q != 0, ()) {
            ::core::option::Option::Some(()) => {
              match ::core::primitive::bool::then_some(
                matches!(
                  rav1d_msac_decode_bool_equi(&mut ts_c_2.msac),
                  Ok(EntropyBit::One)
                ),
                (),
              ) {
                ::core::option::Option::Some(()) => {
                  delta_q = (delta_q).wrapping_neg();
                }
                ::core::option::Option::None => {}
              }

              {
                let Ok(res_log2) =
                  console_core_number_conversion::wrapped::<_, u32>(frame_hdr.delta.q.res_log2);
                let numeric_amount = i32::wrapping_shl(1, res_log2);
                let numeric_target = &mut (delta_q);
                *numeric_target = i32::wrapping_mul(*numeric_target, numeric_amount);
              };
            }
            ::core::option::Option::None => {}
          }

          let Ok(ts_last_qidx) = ts.last_qidx.get();
          let last_qidx = clip(
            i32::wrapping_add(
              {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(ts_last_qidx);
                numeric_value
              },
              delta_q,
            ),
            1,
            255,
          );
          let Ok(()) = ts.last_qidx.set(last_qidx);

          match ::core::primitive::bool::then_some(
            have_delta_q && debug_block_info!(f, t.b) == BlockTrace::Enabled,
            (),
          ) {
            ::core::option::Option::Some(()) => {
              println!(
                "Post-delta_q[{}->{}]: r={}",
                delta_q, last_qidx, ts_c_2.msac.rng
              );
            }
            ::core::option::Option::None => {}
          }

          match ::core::primitive::bool::then_some(frame_hdr.delta.lf.present != 0, ()) {
            ::core::option::Option::Some(()) => {
              let n_lfs =
                match ::core::primitive::bool::then_some(frame_hdr.delta.lf.multi != 0, ()) {
                  ::core::option::Option::Some(()) => match ::core::primitive::bool::then_some(
                    f.cur.p.layout != Rav1dPixelLayout::I400,
                    (),
                  ) {
                    ::core::option::Option::Some(()) => 4,
                    ::core::option::Option::None => 2,
                  },
                  ::core::option::Option::None => 1,
                };

              let Ok(mut last_delta_lf) = ts.last_delta_lf.get();

              for i in 0..({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(n_lfs);
                numeric_value
              }) {
                let Ok(multi) =
                  console_core_number_conversion::wrapped_index(frame_hdr.delta.lf.multi);
                let ::core::result::Result::Ok(delta_lf_index) =
                  ::console_core_number_conversion::fixed_index(i.wrapping_add(multi));
                let Ok(mut delta_lf) = console_core_number_conversion::wrapped::<_, i32>({
                  let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt4(
                    &mut ts_c_2.msac,
                    &mut *match <[_]>::get_mut(
                      &mut *ts_c_2.cdf.m.delta_lf,
                      match ::console_core_number_conversion::index(delta_lf_index) {
                        ::core::result::Result::Ok(native_position_) => native_position_,
                      },
                    ) {
                      ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                    3,
                  );
                  explicit_value
                });

                match ::core::primitive::bool::then_some(delta_lf == 3, ()) {
                  ::core::option::Option::Some(()) => {
                    let n_bits = u8::wrapping_add(1, {
                      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>({
                        let Ok(explicit_value) = rav1d_msac_decode_bools(&mut ts_c_2.msac, 3);
                        explicit_value
                      });
                      numeric_value
                    });
                    delta_lf = {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, i32>(u32::wrapping_add(
                          u32::wrapping_add(
                            {
                              let Ok(explicit_value) =
                                rav1d_msac_decode_bools(&mut ts_c_2.msac, n_bits);
                              explicit_value
                            },
                            1,
                          ),
                          u32::wrapping_shl(1, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, u32>(n_bits);
                            numeric_value
                          }),
                        ));
                      numeric_value
                    };
                  }
                  ::core::option::Option::None => {}
                }

                match ::core::primitive::bool::then_some(delta_lf != 0, ()) {
                  ::core::option::Option::Some(()) => {
                    match ::core::primitive::bool::then_some(
                      matches!(
                        rav1d_msac_decode_bool_equi(&mut ts_c_2.msac),
                        Ok(EntropyBit::One)
                      ),
                      (),
                    ) {
                      ::core::option::Option::Some(()) => {
                        delta_lf = (delta_lf).wrapping_neg();
                      }
                      ::core::option::Option::None => {}
                    }

                    {
                      let Ok(res_log2) = console_core_number_conversion::wrapped::<_, u32>(
                        frame_hdr.delta.lf.res_log2,
                      );
                      let numeric_amount = i32::wrapping_shl(1, res_log2);
                      let numeric_target = &mut (delta_lf);
                      *numeric_target = i32::wrapping_mul(*numeric_target, numeric_amount);
                    };
                  }
                  ::core::option::Option::None => {}
                }

                *match <[_]>::get_mut(&mut last_delta_lf, i) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                } = clip(
                  i32::wrapping_add(
                    {
                      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(
                        *match <[_]>::get(&last_delta_lf, i) {
                          ::core::option::Option::Some(indexed_value) => indexed_value,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        },
                      );
                      numeric_value
                    },
                    delta_lf,
                  ),
                  -63,
                  63,
                );

                match ::core::primitive::bool::then_some(
                  have_delta_q && debug_block_info!(f, t.b) == BlockTrace::Enabled,
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    println!("Post-delta_lf[{}:{}]: r={}", i, delta_lf, ts_c_2.msac.rng);
                  }
                  ::core::option::Option::None => {}
                }
              }

              let Ok(()) = ts.last_delta_lf.set(last_delta_lf);
            }
            ::core::option::Option::None => {}
          }
        }
        ::core::option::Option::None => {}
      }

      let Ok(last_qidx) = ts.last_qidx.get();

      match ::core::primitive::bool::then_some(last_qidx == frame_hdr.quant.yac, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(()) = ts.dq.set(TileStateRef::Frame);
        }
        ::core::option::Option::None => {
          match ::core::primitive::bool::then_some(last_qidx != prev_qidx, ()) {
            ::core::option::Option::Some(()) => {
              init_quant_tables(seq_hdr, frame_hdr, last_qidx, &ts.dqmem)
                .map_err(|_invalid_quantizer| ())?;
              let Ok(()) = ts.dq.set(TileStateRef::Local);
            }
            ::core::option::Option::None => {}
          }
        }
      }

      let Ok(last_delta_lf) = ts.last_delta_lf.get();

      match ::core::primitive::bool::then_some(last_delta_lf == [0, 0, 0, 0], ()) {
        ::core::option::Option::Some(()) => {
          let Ok(()) = ts.lflvl.set(TileStateRef::Frame);
        }
        ::core::option::Option::None => {
          match ::core::primitive::bool::then_some(last_delta_lf != prev_delta_lf, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(()) = rav1d_calc_lf_values(
                &mut (*ts.lflvlmem.try_write().ok_or(())?),
                frame_hdr,
                &last_delta_lf,
              );
              let Ok(()) = ts.lflvl.set(TileStateRef::Local);
            }
            ::core::option::Option::None => {}
          }
        }
      }
    }
    ::core::option::Option::None => {}
  }

  let intra = match ::core::primitive::bool::then_some(b.skip_mode != 0, ()) {
    ::core::option::Option::Some(()) => false,
    ::core::option::Option::None => {
      match ::core::primitive::bool::then_some(frame_hdr.frame_type.is_inter_or_switch(), ()) {
        ::core::option::Option::Some(()) => {
          match seg.filter(|seg_2| seg_2.r#ref >= 0 || seg_2.globalmv != 0) {
            Some(seg_3) => seg_3.r#ref == 0,
            None => {
              let ictx = get_intra_ctx(BlockNeighbors {
                above: ta,
                left: &t.l,
                location: Bxy { x: bx4, y: by4 },
                availability: neighbors,
              })
              .map_err(|_invalid_context| ())?;
              let intra = !matches!(
                rav1d_msac_decode_bool_adapt(
                  &mut ts_c_2.msac,
                  &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.intra, {
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(ictx);
                    numeric_value
                  }) {
                    ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                ),
                Ok(EntropyBit::One)
              );

              match debug_block_info!(f, t.b) {
                BlockTrace::Enabled => {
                  println!("Post-intra[{}]: r={}", intra, ts_c_2.msac.rng);
                }
                BlockTrace::Disabled => {}
              }

              intra
            }
          }
        }
        ::core::option::Option::None => {
          match ::core::primitive::bool::then_some(frame_hdr.allow_intrabc, ()) {
            ::core::option::Option::Some(()) => {
              let intra = !matches!(
                rav1d_msac_decode_bool_adapt(&mut ts_c_2.msac, &mut *ts_c_2.cdf.m.intrabc),
                Ok(EntropyBit::One)
              );

              match debug_block_info!(f, t.b) {
                BlockTrace::Enabled => {
                  println!("Post-intrabcflag[{}]: r={}", intra, ts_c_2.msac.rng);
                }
                BlockTrace::Disabled => {}
              }

              intra
            }
            ::core::option::Option::None => true,
          }
        }
      }
    }
  };

  match ::core::primitive::bool::then_some(intra, ()) {
    ::core::option::Option::Some(()) => {
      let ymode_cdf =
        match ::core::primitive::bool::then_some(frame_hdr.frame_type.is_inter_or_switch(), ()) {
          ::core::option::Option::Some(()) => {
            &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.y_mode, {
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                *match <[_]>::get(&dav1d_ymode_size_context, {
                  let Ok(table_index) = console_core_number_conversion::index({
                    let Ok(ordinal) = bs.ordinal();
                    ordinal
                  });
                  table_index
                }) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                },
              );
              numeric_value
            }) {
              ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            }
          }
          ::core::option::Option::None => {
            let Ok(ta_index) = console_core_number_conversion::wrapped_index(*ta.mode.index({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx4);
              numeric_value
            }));

            &mut *match <[_]>::get_mut(
              &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.kfym, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                  *match <[_]>::get(&dav1d_intra_mode_context, ta_index) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                );
                numeric_value
              }) {
                ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              },
              {
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                  *match <[_]>::get(&dav1d_intra_mode_context, {
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                      *match <[_]>::get(&**t.l.mode.get_mut(), {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(by4);
                        numeric_value
                      }) {
                        ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                    );
                    numeric_value
                  }) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                );
                numeric_value
              },
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            }
          }
        };
      let Ok(y_mode) = rav1d_msac_decode_symbol_adapt16(
        &mut ts_c_2.msac,
        ymode_cdf,
        u8::wrapping_sub(
          {
            let Ok(numeric_value) =
              console_core_number_conversion::wrapped::<_, u8>(N_INTRA_PRED_MODES);
            numeric_value
          },
          1,
        ),
      );

      match debug_block_info!(f, t.b) {
        BlockTrace::Enabled => {
          println!("Post-ymode[{}]: r={}", y_mode, ts_c_2.msac.rng);
        }
        BlockTrace::Disabled => {}
      }

      let y_angle = match ::core::primitive::bool::then_some(
        u8::wrapping_add(
          match b_dim {
            &[_, _, element, ..] => element,
          },
          match b_dim {
            &[_, _, _, element, ..] => element,
          },
        ) >= 2
          && (VERT_PRED..=VERT_LEFT_PRED).contains(&y_mode),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let acdf = &mut *match <[_]>::get_mut(
            &mut *ts_c_2.cdf.m.angle_delta,
            ({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(y_mode);
              numeric_value
            })
            .wrapping_sub({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(VERT_PRED);
              numeric_value
            }),
          ) {
            ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
            ::core::option::Option::None => return ::core::result::Result::Err(()),
          };
          let Ok(angle) = rav1d_msac_decode_symbol_adapt8(&mut ts_c_2.msac, acdf, 6);
          i8::wrapping_sub(
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i8>(angle);
              numeric_value
            },
            3,
          )
        }
        ::core::option::Option::None => 0,
      };

      let uv_mode;
      let uv_angle;
      let cfl_alpha;

      match ::core::primitive::bool::then_some(has_chroma, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(b_seg_id) = b.seg_id.get();
          let cfl_allowed = match ::core::primitive::bool::then_some(
            *match <[_]>::get(
              &frame_hdr.segmentation.lossless,
              match ::console_core_number_conversion::index(b_seg_id) {
                ::core::result::Result::Ok(native_position__) => native_position__,
              },
            ) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            },
            (),
          ) {
            ::core::option::Option::Some(()) => cbw4 == 1 && cbh4 == 1,
            ::core::option::Option::None => {
              (cfl_allowed_mask
                & u32::wrapping_shl(1, {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>({
                    let Ok(ordinal) = bs.ordinal();
                    ordinal
                  });
                  numeric_value
                }))
                != 0
            }
          };
          let uvmode_cdf = &mut *match <[_]>::get_mut(
            &mut *match <[_]>::get_mut(
              &mut *ts_c_2.cdf.m.uv_mode,
              match ::console_core_number_conversion::index::<::core::primitive::u8>(
                ::core::primitive::u8::from(cfl_allowed),
              ) {
                ::core::result::Result::Ok(converted_position) => converted_position,
              },
            ) {
              ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            },
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(y_mode);
              numeric_value
            },
          ) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => return ::core::result::Result::Err(()),
          };
          uv_mode = {
            let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt16(
              &mut ts_c_2.msac,
              uvmode_cdf,
              u8::wrapping_sub(
                u8::wrapping_sub(
                  {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u8>(N_UV_INTRA_PRED_MODES);
                    numeric_value
                  },
                  1,
                ),
                u8::from(!cfl_allowed),
              ),
            );
            explicit_value
          };

          match debug_block_info!(f, t.b) {
            BlockTrace::Enabled => {
              println!("Post-uvmode[{}]: r={}", uv_mode, ts_c_2.msac.rng);
            }
            BlockTrace::Disabled => {}
          }

          match ::core::primitive::bool::then_some(uv_mode == CFL_PRED, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(symbol) =
                rav1d_msac_decode_symbol_adapt8(&mut ts_c_2.msac, &mut *ts_c_2.cdf.m.cfl_sign, 7);
              let sign = symbol.wrapping_add(1);
              let sign_u = sign.wrapping_div(3);
              let sign_v = sign.wrapping_sub(sign_u.wrapping_mul(3));
              let alpha_u = read_chroma_scale(
                ts_c_2,
                ChromaScaleSigns {
                  current: sign_u,
                  opposite: sign_v,
                },
              )?;
              let alpha_v = read_chroma_scale(
                ts_c_2,
                ChromaScaleSigns {
                  current: sign_v,
                  opposite: sign_u,
                },
              )?;
              cfl_alpha = [alpha_u, alpha_v];

              match debug_block_info!(f, t.b) {
                BlockTrace::Enabled => {
                  console_core_logging::trace!(
                    "Post-uvalphas[{alpha_u}/{alpha_v}]: r={}",
                    ts_c_2.msac.rng
                  );
                }
                BlockTrace::Disabled => {}
              }

              uv_angle = 0;
            }
            ::core::option::Option::None => match ::core::primitive::bool::then_some(
              u8::wrapping_add(
                match b_dim {
                  &[_, _, element, ..] => element,
                },
                match b_dim {
                  &[_, _, _, element, ..] => element,
                },
              ) >= 2
                && (VERT_PRED..=VERT_LEFT_PRED).contains(&uv_mode),
              (),
            ) {
              ::core::option::Option::Some(()) => {
                let acdf = &mut *match <[_]>::get_mut(
                  &mut *ts_c_2.cdf.m.angle_delta,
                  ({
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped_index(uv_mode);
                    numeric_value
                  })
                  .wrapping_sub({
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped_index(VERT_PRED);
                    numeric_value
                  }),
                ) {
                  ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                };
                let Ok(angle) = rav1d_msac_decode_symbol_adapt8(&mut ts_c_2.msac, acdf, 6);
                uv_angle = i8::wrapping_sub(
                  {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, i8>(angle);
                    numeric_value
                  },
                  3,
                );
                cfl_alpha = Default::default();
              }
              ::core::option::Option::None => {
                uv_angle = 0;
                cfl_alpha = Default::default();
              }
            },
          }
        }
        ::core::option::Option::None => {
          uv_mode = Default::default();
          uv_angle = Default::default();
          cfl_alpha = Default::default();
        }
      }

      let mut pal_sz = [0; 2];

      match ::core::primitive::bool::then_some(
        frame_hdr.allow_screen_content_tools
          && cmp::max(bw4, bh4) <= 16
          && i32::wrapping_add(bw4, bh4) >= 4,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let sz_ctx = u8::wrapping_sub(
            u8::wrapping_add(
              match b_dim {
                &[_, _, element, ..] => element,
              },
              match b_dim {
                &[_, _, _, element, ..] => element,
              },
            ),
            2,
          );

          match ::core::primitive::bool::then_some(y_mode == DC_PRED, ()) {
            ::core::option::Option::Some(()) => {
              let above = console_core_number_conversion::checked_index(bx4)
                .map_err(|_invalid_neighbor| ())?;
              let left = console_core_number_conversion::checked_index(by4)
                .map_err(|_invalid_neighbor| ())?;
              let above_size = *ta.pal_sz.index(above);
              let &left_size = t.l.pal_sz.get_mut().get(left).ok_or(())?;
              let pal_ctx = u8::from(above_size > 0).wrapping_add(u8::from(left_size > 0));
              let use_y_pal = matches!(
                rav1d_msac_decode_bool_adapt(
                  &mut ts_c_2.msac,
                  &mut *match <[_]>::get_mut(
                    &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.m.pal_y, {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped_index(sz_ctx);
                      numeric_value
                    }) {
                      ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                    match ::console_core_number_conversion::index(pal_ctx) {
                      ::core::result::Result::Ok(native_position_) => native_position_,
                    },
                  ) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                ),
                Ok(EntropyBit::One)
              );

              match debug_block_info!(f, t.b) {
                BlockTrace::Enabled => {
                  println!("Post-y_pal[{}]: r={}", use_y_pal, ts_c_2.msac.rng);
                }
                BlockTrace::Disabled => {}
              }

              match ::core::primitive::bool::then_some(use_y_pal, ()) {
                ::core::option::Option::Some(()) => {
                  let checked_value = (bd_fn.read_pal_y)(
                    t,
                    f,
                    ts_c_2,
                    PaletteRead {
                      size_context: sz_ctx,
                      position: Point { x: bx4, y: by4 },
                    },
                  )
                  .map_err(|_invalid_palette| ())?;
                  *{
                    let [_explicit_part, ..] = &mut pal_sz;
                    _explicit_part
                  } = checked_value;
                }
                ::core::option::Option::None => {}
              }
            }
            ::core::option::Option::None => {}
          }

          match ::core::primitive::bool::then_some(has_chroma && uv_mode == DC_PRED, ()) {
            ::core::option::Option::Some(()) => {
              let pal_ctx = (match pal_sz {
                [element, ..] => element,
              }) > 0;
              let use_uv_pal = matches!(
                rav1d_msac_decode_bool_adapt(
                  &mut ts_c_2.msac,
                  &mut *match <[_]>::get_mut(
                    &mut *ts_c_2.cdf.m.pal_uv,
                    match ::console_core_number_conversion::index::<::core::primitive::u8>(
                      ::core::primitive::u8::from(pal_ctx),
                    ) {
                      ::core::result::Result::Ok(converted_position) => converted_position,
                    },
                  ) {
                    ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                ),
                Ok(EntropyBit::One)
              );

              match debug_block_info!(f, t.b) {
                BlockTrace::Enabled => {
                  println!("Post-uv_pal[{}]: r={}", use_uv_pal, ts_c_2.msac.rng);
                }
                BlockTrace::Disabled => {}
              }

              match ::core::primitive::bool::then_some(use_uv_pal, ()) {
                ::core::option::Option::Some(()) => {
                  let checked_value_2 = (bd_fn.read_pal_uv)(
                    t,
                    f,
                    ts_c_2,
                    PaletteRead {
                      size_context: sz_ctx,
                      position: Point { x: bx4, y: by4 },
                    },
                  )
                  .map_err(|_invalid_palette| ())?;
                  *{
                    let [_, _explicit_part, ..] = &mut pal_sz;
                    _explicit_part
                  } = checked_value_2;
                }
                ::core::option::Option::None => {}
              }
            }
            ::core::option::Option::None => {}
          }
        }
        ::core::option::Option::None => {}
      }

      let pal_sz = pal_sz;

      let mut y_mode_3 = y_mode;
      let mut y_angle_3 = y_angle;
      let seq_hdr_2 = f.seq_hdr().map_err(|_missing_header| ())?;

      match ::core::primitive::bool::then_some(
        y_mode_3 == DC_PRED
          && (match pal_sz {
            [element, ..] => element,
          }) == 0
          && cmp::max(
            match b_dim {
              &[_, _, element, ..] => element,
            },
            match b_dim {
              &[_, _, _, element, ..] => element,
            },
          ) <= 3
          && seq_hdr_2.filter_intra != 0,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let is_filter = matches!(
            rav1d_msac_decode_bool_adapt(
              &mut ts_c_2.msac,
              &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.m.use_filter_intra, {
                let Ok(table_index) = console_core_number_conversion::index({
                  let Ok(ordinal) = bs.ordinal();
                  ordinal
                });
                table_index
              }) {
                ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              },
            ),
            Ok(EntropyBit::One)
          );

          match ::core::primitive::bool::then_some(is_filter, ()) {
            ::core::option::Option::Some(()) => {
              y_mode_3 = {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u8>(FILTER_PRED);
                numeric_value
              };
              y_angle_3 = {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i8>({
                  let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt8(
                    &mut ts_c_2.msac,
                    &mut *ts_c_2.cdf.m.filter_intra,
                    4,
                  );
                  explicit_value
                });
                numeric_value
              };
            }
            ::core::option::Option::None => {}
          }

          match debug_block_info!(f, t.b) {
            BlockTrace::Enabled => {
              println!(
                "Post-filterintramode[{}/{}]: r={}",
                y_mode_3, y_angle_3, ts_c_2.msac.rng,
              );
            }
            BlockTrace::Disabled => {}
          }
        }
        ::core::option::Option::None => {}
      }

      let y_mode_2 = y_mode_3;
      let y_angle_2 = y_angle_3;

      match ::core::primitive::bool::then_some(
        (match pal_sz {
          [element, ..] => element,
        }) != 0,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let scratch = t.scratch.inter_intra_mut().map_err(|_invalid_scratch| ())?;
          let pal_idx = match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
            ::core::option::Option::Some(()) => {
              let p = t.frame_thread.pass & 1;
              let frame_thread = &*match <[_]>::get(&ts.frame_thread, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(p);
                numeric_value
              }) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              };
              let Ok(len) = console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_mul(
                i32::wrapping_mul(bw4, bh4),
                8,
              ));
              let Ok(pal_idx) = frame_thread
                .pal_idx
                .get_update(|i| u32::wrapping_add(i, len));
              &mut *f.frame_thread.pal_idx.index_mut((
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
            ::core::option::Option::None => &mut scratch.pal_idx_y,
          };
          let [entries, _] = pal_sz;
          read_pal_indices(
            ts_c_2,
            &c.dsp.pal,
            PaletteIndexInput {
              staging: &mut scratch.pal_idx_uv,
              packed: PaletteOutput::Separate(pal_idx),
              cells: PaletteCellBlock {
                visible: Size {
                  width: w4,
                  height: h4,
                },
                block: Size {
                  width: bw4,
                  height: bh4,
                },
              },
              entries,
              plane: PalettePlane::Luma,
            },
          )
          .map_err(|_palette| ())?;

          match debug_block_info!(f, t.b) {
            BlockTrace::Enabled => {
              println!("Post-y-pal-indices: r={}", ts_c_2.msac.rng);
            }
            BlockTrace::Disabled => {}
          }
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(
        has_chroma
          && (match pal_sz {
            [_, element, ..] => element,
          }) != 0,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let scratch = t.scratch.inter_intra_mut().map_err(|_invalid_scratch| ())?;
          let mut pal_idx = match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ())
          {
            ::core::option::Option::Some(()) => {
              let p = t.frame_thread.pass & 1;
              let frame_thread = &*match <[_]>::get(&ts.frame_thread, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(p);
                numeric_value
              }) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              };
              let Ok(len) = console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_mul(
                i32::wrapping_mul(cbw4, cbh4),
                8,
              ));
              let Ok(pal_idx) = frame_thread
                .pal_idx
                .get_update(|i| u32::wrapping_add(i, len));
              Some(f.frame_thread.pal_idx.index_mut((
                ({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(pal_idx);
                  numeric_value
                })..,
                ..({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(len);
                  numeric_value
                }),
              )))
            }
            ::core::option::Option::None => None,
          };
          let packed = match pal_idx.as_deref_mut() {
            Some(output) => PaletteOutput::Separate(output),
            None => PaletteOutput::InPlace,
          };
          let [_, entries] = pal_sz;
          read_pal_indices(
            ts_c_2,
            &c.dsp.pal,
            PaletteIndexInput {
              staging: &mut scratch.pal_idx_uv,
              packed,
              cells: PaletteCellBlock {
                visible: Size {
                  width: cw4,
                  height: ch4,
                },
                block: Size {
                  width: cbw4,
                  height: cbh4,
                },
              },
              entries,
              plane: PalettePlane::Chroma,
            },
          )
          .map_err(|_palette| ())?;

          match debug_block_info!(f, t.b) {
            BlockTrace::Enabled => {
              println!("Post-uv-pal-indices: r={}", ts_c_2.msac.rng);
            }
            BlockTrace::Disabled => {}
          }
        }
        ::core::option::Option::None => {}
      }

      let frame_hdr_2 = f.frame_hdr().map_err(|_missing_header| ())?;

      let Ok(b_seg_id) = b.seg_id.get();
      let tx = match ::core::primitive::bool::then_some(
        *match <[_]>::get(
          &frame_hdr_2.segmentation.lossless,
          match ::console_core_number_conversion::index(b_seg_id) {
            ::core::result::Result::Ok(native_position__) => native_position__,
          },
        ) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => return ::core::result::Result::Err(()),
        },
        (),
      ) {
        ::core::option::Option::Some(()) => {
          b.uvtx = TxfmSize::S4x4;
          b.uvtx
        }
        ::core::option::Option::None => {
          let mut tx = *match <[_]>::get(
            &(*match <[_]>::get(&dav1d_max_txfm_size_for_bs, {
              let Ok(table_index) = console_core_number_conversion::index({
                let Ok(ordinal) = bs.ordinal();
                ordinal
              });
              table_index
            }) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            }),
            0,
          ) {
            ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
            ::core::option::Option::None => return ::core::result::Result::Err(()),
          };
          b.uvtx = *match <[_]>::get(
            &(*match <[_]>::get(&dav1d_max_txfm_size_for_bs, {
              let Ok(table_index) = console_core_number_conversion::index({
                let Ok(ordinal) = bs.ordinal();
                ordinal
              });
              table_index
            }) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            }),
            {
              let Ok(table_index) =
                console_core_number_conversion::index(f.cur.p.layout.into_rav1d());
              table_index
            },
          ) {
            ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
            ::core::option::Option::None => return ::core::result::Result::Err(()),
          };
          let mut t_dim = &*match <[_]>::get(&dav1d_txfm_dimensions, {
            let Ok(table_index) = console_core_number_conversion::index({
              let Ok(ordinal) = tx.ordinal();
              ordinal
            });
            table_index
          }) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => return ::core::result::Result::Err(()),
          };

          match ::core::primitive::bool::then_some(
            frame_hdr_2.txfm_mode == Rav1dTxfmMode::Switchable
              && t_dim.max > {
                let Ok(ordinal) = TxfmSize::S4x4.ordinal();
                ordinal
              },
            (),
          ) {
            ::core::option::Option::Some(()) => {
              let tctx = get_tx_ctx(
                BlockNeighbors {
                  above: ta,
                  left: &t.l,
                  location: Bxy { x: bx4, y: by4 },
                  availability: neighbors,
                },
                t_dim,
              )
              .map_err(|_invalid_context| ())?;
              let tx_cdf = &mut *match <[_]>::get_mut(
                &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.m.txsz, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped_index(u8::wrapping_sub(t_dim.max, 1));
                  numeric_value
                }) {
                  ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                },
                {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(tctx);
                  numeric_value
                },
              ) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              };
              let Ok(depth) = console_core_number_conversion::wrapped::<_, i32>({
                let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt4(
                  &mut ts_c_2.msac,
                  tx_cdf,
                  cmp::min(t_dim.max, 2),
                );
                explicit_value
              });

              for _ in 0..depth {
                tx = t_dim.sub;
                t_dim = &*match <[_]>::get(&dav1d_txfm_dimensions, {
                  let Ok(table_index) = console_core_number_conversion::index({
                    let Ok(ordinal) = tx.ordinal();
                    ordinal
                  });
                  table_index
                }) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                };
              }
            }
            ::core::option::Option::None => {}
          }

          match debug_block_info!(f, t.b) {
            BlockTrace::Enabled => {
              println!("Post-tx[{:?}]: r={}", tx, ts_c_2.msac.rng);
            }
            BlockTrace::Disabled => {}
          }

          tx
        }
      };
      let t_dim = &*match <[_]>::get(&dav1d_txfm_dimensions, {
        let Ok(table_index) = console_core_number_conversion::index({
          let Ok(ordinal) = tx.ordinal();
          ordinal
        });
        table_index
      }) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => return ::core::result::Result::Err(()),
      };

      let intra = Av1BlockIntra {
        y_mode: y_mode_2,
        uv_mode,
        tx,
        pal_sz,
        y_angle: y_angle_2,
        uv_angle,
        cfl_alpha,
      };
      b.ii = Av1BlockIntraInter::Intra(intra);

      match ::core::primitive::bool::then_some(t.frame_thread.pass == 1, ()) {
        ::core::option::Option::Some(()) => {
          (bd_fn.read_coef_blocks)(f, t, ts_c_2, bs, b).map_err(|_| ())?;
        }
        ::core::option::Option::None => {
          (bd_fn.recon_b_intra)(f, t, Some(ts_c_2), bs, intra_edge_flags, b, &intra)
            .map_err(|_| ())?;
        }
      }

      let f_frame_hdr = f.frame_hdr().map_err(|_missing_header| ())?;

      match ::core::primitive::bool::then_some(f_frame_hdr.loopfilter.level_y != [0, 0], ()) {
        ::core::option::Option::Some(()) => {
          let Ok(get) = ts.lflvl.get();
          let lflvl = match get {
            TileStateRef::Frame => &f.lf.lvl,
            TileStateRef::Local => &*ts.lflvlmem.try_read().ok_or(())?,
          };
          let mut a_uv_guard;
          let l_uv_slice;

          match create_lf_mask(
            FilterMaskBlock {
              mask: {
                let checked_value_5 =
                  f.loop_filter_mask(t.lf_mask).map_err(|_invalid_mask| ())?;
                checked_value_5
              },
              level_cache: &f.lf.level,
              stride: i64::from(f.b4_stride),
              levels: {
                let Ok(segment) = b.seg_id.get();
                lflvl
                  .get(match ::console_core_number_conversion::index(segment) {
                    ::core::result::Result::Ok(native_position___) => native_position___,
                  })
                  .ok_or(())?
              },
              origin: t.b,
              frame: console_core_geometry::Size {
                width: f.w4,
                height: f.h4,
              },
              size: bs,
              ytx: tx,
              uvtx: b.uvtx,
              layout: f.cur.p.layout,
              luma: EdgeNeighbors {
                above: &mut ta.tx_lpf_y.index_mut((
                  ({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx4);
                    numeric_value
                  })..,
                  ..({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4);
                    numeric_value
                  }),
                )),
                left: &mut *match <[_]>::get_mut(
                  &mut *match <[_]>::get_mut(
                    &mut **t.l.tx_lpf_y.get_mut(),
                    ({
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4);
                      numeric_value
                    })..,
                  ) {
                    ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                  ..({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4);
                    numeric_value
                  }),
                ) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                },
              },
              chroma: match ::core::primitive::bool::then_some(has_chroma, ()) {
                ::core::option::Option::Some(()) => {
                  a_uv_guard = ta.tx_lpf_uv.index_mut((
                    ({
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbx4);
                      numeric_value
                    })..,
                    ..({
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbw4);
                      numeric_value
                    }),
                  ));
                  l_uv_slice = &mut *match <[_]>::get_mut(
                    &mut *match <[_]>::get_mut(
                      &mut **t.l.tx_lpf_uv.get_mut(),
                      ({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(cby4);
                        numeric_value
                      })..,
                    ) {
                      ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                    ..({
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbh4);
                      numeric_value
                    }),
                  ) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  };
                  Some((&mut a_uv_guard, l_uv_slice))
                }
                ::core::option::Option::None => None,
              },
            },
            FilterMaskPrediction::Intra,
          ) {
            Ok(()) => {}
            Err(error) => {
              console_core_logging::debug!(
                "Could not create an AV1 intra filter mask: {error:?}"
              );
              return Err(());
            }
          }
        }

        ::core::option::Option::None => {}
      }

      let y_mode_nofilt = match ::core::primitive::bool::then_some(y_mode_2 == FILTER_PRED, ()) {
        ::core::option::Option::Some(()) => DC_PRED,
        ::core::option::Option::None => y_mode_2,
      };
      let is_inter_or_switch = ({
        let explicit_value = f.frame_hdr().map_err(|_missing_header| ())?;
        explicit_value
      })
      .frame_type
      .is_inter_or_switch();

      let Ok(tx_height_index) = console_core_number_conversion::index(t_dim.lh);
      let Ok(tx_width_index) = console_core_number_conversion::index(t_dim.lw);
      let tx_height = TxfmSize::from_repr(tx_height_index).ok_or(())?;
      let tx_width = TxfmSize::from_repr(tx_width_index).ok_or(())?;

      case_set_al! {
          <32, false>

          l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4); numeric_value })

              also |case| { ({ let Ok(explicit_value) = case.set({ let [_, _explicit_part, ..] = &mut t.pal_sz_uv; _explicit_part },

              match has_chroma {
                  true => { match pal_sz { [_, _explicit_element, ..] => _explicit_element } },

                  false => { 0 },
              }); explicit_value }); },

          a: (ta, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx4); numeric_value })

              also |case| { ({ let Ok(explicit_value) = case.set({ let [_explicit_part, ..] = &mut t.pal_sz_uv; _explicit_part },

              match has_chroma {
                  true => { match pal_sz { [_, _explicit_element, ..] => _explicit_element } },

                  false => { 0 },
              }); explicit_value }); },

          tx_intra = ({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i8>(t_dim.lh); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i8>(t_dim.lw); numeric_value }),
          tx = (tx_height, tx_width),
          mode = (y_mode_nofilt, y_mode_nofilt),
          pal_sz = (match pal_sz { [element, ..] => element }, match pal_sz { [element, ..] => element }),
          seg_pred = (<::core::primitive::u8 as ::core::convert::From<::core::primitive::bool>>::from(seg_pred), <::core::primitive::u8 as ::core::convert::From<::core::primitive::bool>>::from(seg_pred)),
          skip_mode = (0, 0),
          intra = (1, 1),
          skip = (b.skip, b.skip),
      }

      match ::core::primitive::bool::then_some(is_inter_or_switch, ()) {
        ::core::option::Option::Some(()) => {
          case_set_al! {
              <32, false>
              l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4); numeric_value }),
              a: (ta, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bw4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx4); numeric_value }),
              comp_type = (None, None),
              r#ref[0] = (-1, -1),
              r#ref[1] = (-1, -1),
              filter[0] = (
                  Rav1dFilterMode::N_SWITCHABLE_FILTERS,
                  Rav1dFilterMode::N_SWITCHABLE_FILTERS
              ),
              filter[1] = (
                  Rav1dFilterMode::N_SWITCHABLE_FILTERS,
                  Rav1dFilterMode::N_SWITCHABLE_FILTERS
              ),
          }
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(
        (match pal_sz {
          [element, ..] => element,
        }) != 0,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          (bd_fn.copy_pal_block)(
            t,
            f,
            PaletteCopy {
              region: Rectangle {
                origin: Point { x: bx4, y: by4 },
                size: Size {
                  width: bw4,
                  height: bh4,
                },
              },
              plane: PalettePlane::Luma,
            },
          )
          .map_err(|_invalid_palette| ())?;
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(has_chroma, ()) {
        ::core::option::Option::Some(()) => {
          case_set_al! {
              <32, false>
              l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbh4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cby4); numeric_value }),
              a: (ta, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbw4); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cbx4); numeric_value }),
              uvmode = (uv_mode, uv_mode),
          }

          match ::core::primitive::bool::then_some(
            (match pal_sz {
              [_, element, ..] => element,
            }) != 0,
            (),
          ) {
            ::core::option::Option::Some(()) => {
              (bd_fn.copy_pal_block)(
                t,
                f,
                PaletteCopy {
                  region: Rectangle {
                    origin: Point { x: bx4, y: by4 },
                    size: Size {
                      width: bw4,
                      height: bh4,
                    },
                  },
                  plane: PalettePlane::Chroma,
                },
              )
              .map_err(|_invalid_palette| ())?;
            }
            ::core::option::Option::None => {}
          }
        }
        ::core::option::Option::None => {}
      }

      let frame_hdr_3 = f.frame_hdr().map_err(|_missing_header| ())?;

      match ::core::primitive::bool::then_some(
        frame_hdr_3.frame_type.is_inter_or_switch() || frame_hdr_3.allow_intrabc,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          splat_intraref(
            c,
            t,
            &f.rf,
            bs,
            Size {
              width: bw4,
              height: bh4,
            },
          )
          .map_err(|_| ())?;
        }
        ::core::option::Option::None => {}
      }
    }
    ::core::option::Option::None => {
      match ::core::primitive::bool::then_some(frame_hdr.frame_type.is_key_or_intra(), ()) {
        ::core::option::Option::Some(()) => {
          let mut mvstack = [Default::default(); 8];
          let mut ctx = 0;
          let _candidate_count = rav1d_refmvs_find(
            &t.rt,
            &f.rf,
            &mut mvstack,
            &mut ctx,
            crate::src::refmvs::RefMvsSearch {
              references: <crate::src::refmvs::RefMvsRefPair as ::core::convert::From<
                [::core::primitive::i8; 2],
              >>::from([0, -1]),
              block: bs,
              edges: intra_edge_flags,
              origin: Point { x: t.b.x, y: t.b.y },
            },
            frame_hdr,
          )
          .map_err(|_invalid_motion_neighbors| ())?;

          let mut r#ref = match ::core::primitive::bool::then_some(
            *match <[_]>::get(
              &(*match <[_]>::get(&mvstack, 0) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              })
              .mv
              .mv,
              0,
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            } != Mv::ZERO,
            (),
          ) {
            ::core::option::Option::Some(()) => *match <[_]>::get(
              &(*match <[_]>::get(&mvstack, 0) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              })
              .mv
              .mv,
              0,
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            },
            ::core::option::Option::None => match ::core::primitive::bool::then_some(
              *match <[_]>::get(
                &(*match <[_]>::get(&mvstack, 1) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                })
                .mv
                .mv,
                0,
              ) {
                ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              } != Mv::ZERO,
              (),
            ) {
              ::core::option::Option::Some(()) => *match <[_]>::get(
                &(*match <[_]>::get(&mvstack, 1) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                })
                .mv
                .mv,
                0,
              ) {
                ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              },
              ::core::option::Option::None => {
                let Ok(sb128) = console_core_number_conversion::wrapped::<_, u32>(seq_hdr.sb128);

                match ::core::primitive::bool::then_some(
                  i32::wrapping_sub(t.b.y, i32::wrapping_shl(16, sb128)) < ts.tiling.row_start,
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    let Ok(seq_hdr_sb128) =
                      console_core_number_conversion::wrapped::<_, u32>(seq_hdr.sb128);
                    let Ok(x) =
                      console_core_number_conversion::wrapped::<_, i16>(i32::wrapping_sub(
                        (i32::wrapping_shl(512, seq_hdr_sb128)).wrapping_neg(),
                        2048,
                      ));

                    Mv { y: 0, x }
                  }
                  ::core::option::Option::None => {
                    let Ok(seq_hdr_sb128) =
                      console_core_number_conversion::wrapped::<_, u32>(seq_hdr.sb128);
                    let Ok(y) = console_core_number_conversion::wrapped::<_, i16>(
                      (i32::wrapping_shl(512, seq_hdr_sb128)).wrapping_neg(),
                    );

                    Mv { y, x: 0 }
                  }
                }
              }
            },
          };

          read_mv_residual(ts_c_2, &mut r#ref, ResidualPrecision::Integer)
            .map_err(|_invalid_motion_residual| ())?;

          let mut border_left = i32::wrapping_mul(ts.tiling.col_start, 4);
          let mut border_top = i32::wrapping_mul(ts.tiling.row_start, 4);

          match ::core::primitive::bool::then_some(has_chroma, ()) {
            ::core::option::Option::Some(()) => {
              match ::core::primitive::bool::then_some(bw4 < 2 && ss_hor != 0, ()) {
                ::core::option::Option::Some(()) => {
                  {
                    let numeric_amount = 4;
                    let numeric_target = &mut (border_left);
                    *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                  };
                }
                ::core::option::Option::None => {}
              }

              match ::core::primitive::bool::then_some(bh4 < 2 && ss_ver != 0, ()) {
                ::core::option::Option::Some(()) => {
                  {
                    let numeric_amount = 4;
                    let numeric_target = &mut (border_top);
                    *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                  };
                }
                ::core::option::Option::None => {}
              }
            }
            ::core::option::Option::None => {}
          }

          let mut src_left = i32::wrapping_add(
            i32::wrapping_mul(t.b.x, 4),
            i32::wrapping_shr(
              {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(r#ref.x);
                numeric_value
              },
              3_u32,
            ),
          );
          let mut src_top = i32::wrapping_add(
            i32::wrapping_mul(t.b.y, 4),
            i32::wrapping_shr(
              {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, i32>(r#ref.y);
                numeric_value
              },
              3_u32,
            ),
          );
          let mut src_right = i32::wrapping_add(src_left, i32::wrapping_mul(bw4, 4));
          let mut src_bottom = i32::wrapping_add(src_top, i32::wrapping_mul(bh4, 4));
          let border_right = i32::wrapping_mul(
            i32::wrapping_add(ts.tiling.col_end, i32::wrapping_sub(bw4, 1))
              & !i32::wrapping_sub(bw4, 1),
            4,
          );

          match ::core::primitive::bool::then_some(src_left < border_left, ()) {
            ::core::option::Option::Some(()) => {
              {
                let numeric_amount = i32::wrapping_sub(border_left, src_left);
                let numeric_target = &mut (src_right);
                *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
              };

              {
                let numeric_amount = i32::wrapping_sub(border_left, src_left);
                let numeric_target = &mut (src_left);
                *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
              };
            }
            ::core::option::Option::None => {
              match ::core::primitive::bool::then_some(src_right > border_right, ()) {
                ::core::option::Option::Some(()) => {
                  {
                    let numeric_amount = i32::wrapping_sub(src_right, border_right);
                    let numeric_target = &mut (src_left);
                    *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
                  };

                  {
                    let numeric_amount = i32::wrapping_sub(src_right, border_right);
                    let numeric_target = &mut (src_right);
                    *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
                  };
                }
                ::core::option::Option::None => {}
              }
            }
          }

          match ::core::primitive::bool::then_some(src_top < border_top, ()) {
            ::core::option::Option::Some(()) => {
              {
                let numeric_amount = i32::wrapping_sub(border_top, src_top);
                let numeric_target = &mut (src_bottom);
                *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
              };

              {
                let numeric_amount = i32::wrapping_sub(border_top, src_top);
                let numeric_target = &mut (src_top);
                *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
              };
            }
            ::core::option::Option::None => {}
          }

          let sbx = i32::wrapping_shl(
            i32::wrapping_shr(t.b.x, {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                u8::wrapping_add(4, seq_hdr.sb128),
              );
              numeric_value
            }),
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                u8::wrapping_add(6, seq_hdr.sb128),
              );
              numeric_value
            },
          );
          let sby = i32::wrapping_shl(
            i32::wrapping_shr(t.b.y, {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                u8::wrapping_add(4, seq_hdr.sb128),
              );
              numeric_value
            }),
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                u8::wrapping_add(6, seq_hdr.sb128),
              );
              numeric_value
            },
          );
          let sb_size = i32::wrapping_shl(1, {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
              u8::wrapping_add(6, seq_hdr.sb128),
            );
            numeric_value
          });

          match ::core::primitive::bool::then_some(src_bottom > sby && src_right > sbx, ()) {
            ::core::option::Option::Some(()) => {
              match ::core::primitive::bool::then_some(
                i32::wrapping_sub(src_top, border_top) >= i32::wrapping_sub(src_bottom, sby),
                (),
              ) {
                ::core::option::Option::Some(()) => {
                  {
                    let numeric_amount = i32::wrapping_sub(src_bottom, sby);
                    let numeric_target = &mut (src_top);
                    *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
                  };

                  {
                    let numeric_amount = i32::wrapping_sub(src_bottom, sby);
                    let numeric_target = &mut (src_bottom);
                    *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
                  };
                }
                ::core::option::Option::None => match ::core::primitive::bool::then_some(
                  i32::wrapping_sub(src_left, border_left) >= i32::wrapping_sub(src_right, sbx),
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    {
                      let numeric_amount = i32::wrapping_sub(src_right, sbx);
                      let numeric_target = &mut (src_left);
                      *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
                    };

                    {
                      let numeric_amount = i32::wrapping_sub(src_right, sbx);
                      let numeric_target = &mut (src_right);
                      *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
                    };
                  }
                  ::core::option::Option::None => {}
                },
              }
            }
            ::core::option::Option::None => {}
          }

          match ::core::primitive::bool::then_some(
            src_bottom > i32::wrapping_add(sby, sb_size),
            (),
          ) {
            ::core::option::Option::Some(()) => {
              {
                let numeric_amount =
                  i32::wrapping_sub(src_bottom, i32::wrapping_add(sby, sb_size));
                let numeric_target = &mut (src_top);
                *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
              };

              {
                let numeric_amount =
                  i32::wrapping_sub(src_bottom, i32::wrapping_add(sby, sb_size));
                let numeric_target = &mut (src_bottom);
                *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
              };
            }
            ::core::option::Option::None => {}
          }

          match ::core::primitive::bool::then_some(src_bottom > sby && src_right > sbx, ()) {
            ::core::option::Option::Some(()) => {
              return Err(());
            }
            ::core::option::Option::None => {}
          }

          let prev_ref = r#ref;
          let Ok(x) = console_core_number_conversion::wrapped::<_, i16>(i32::wrapping_mul(
            i32::wrapping_sub(src_left, i32::wrapping_mul(t.b.x, 4)),
            8,
          ));
          let Ok(y) = console_core_number_conversion::wrapped::<_, i16>(i32::wrapping_mul(
            i32::wrapping_sub(src_top, i32::wrapping_mul(t.b.y, 4)),
            8,
          ));
          let ref_2 = Mv { x, y };

          match debug_block_info!(f, t.b) {
            BlockTrace::Enabled => {
              println!(
                "Post-dmv[{}/{},ref={}/{}|{}/{}]: r={}",
                ref_2.y,
                ref_2.x,
                prev_ref.y,
                prev_ref.x,
                (*match <[_]>::get(
                  &(*match <[_]>::get(&mvstack, 0) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  })
                  .mv
                  .mv,
                  0
                ) {
                  ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                })
                .y,
                (*match <[_]>::get(
                  &(*match <[_]>::get(&mvstack, 0) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  })
                  .mv
                  .mv,
                  0
                ) {
                  ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                })
                .x,
                ts_c_2.msac.rng,
              );
            }
            BlockTrace::Disabled => {}
          }

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
              local: Bxy { x: bx4, y: by4 },
            },
          )
          .map_err(|_invalid_transform_tree| ())?;

          let filter2d = match ::core::primitive::bool::then_some(t.frame_thread.pass == 1, ()) {
            ::core::option::Option::Some(()) => Filter2d::Bilinear,
            ::core::option::Option::None => Default::default(),
          };

          b.uvtx = uvtx;
          let inter = Av1BlockInter {
            nd: <crate::src::levels::Av1BlockInterNd as ::core::convert::From<
              crate::src::levels::Av1BlockInter1d,
            >>::from(Av1BlockInter1d {
              mv: [ref_2, Default::default()],
              ..Default::default()
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

          match ::core::primitive::bool::then_some(t.frame_thread.pass == 1, ()) {
            ::core::option::Option::Some(()) => {
              (bd_fn.read_coef_blocks)(f, t, ts_c_2, bs, b).map_err(|_| ())?;
            }
            ::core::option::Option::None => {
              (bd_fn.recon_b_inter)(f, t, Some(ts_c_2), bs, b, &inter)?;
            }
          }

          splat_intrabc_mv(
            c,
            t,
            &f.rf,
            bs,
            ref_2,
            Size {
              width: bw4,
              height: bh4,
            },
          )
          .map_err(|_| ())?;

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
        }
        ::core::option::Option::None => {
          let residual_precision = match (frame_hdr.hp, frame_hdr.force_integer_mv) {
            (false, true) => ResidualPrecision::Integer,
            (false, false) | (true, true) => ResidualPrecision::QuarterPel,
            (true, false) => ResidualPrecision::EighthPel,
          };

          let mut has_subpel_filter;

          let is_comp = match ::core::primitive::bool::then_some(b.skip_mode != 0, ()) {
            ::core::option::Option::Some(()) => true,
            ::core::option::Option::None => match ::core::primitive::bool::then_some(
              seg
                .map(|seg_4| seg_4.r#ref == -1 && seg_4.globalmv == 0 && seg_4.skip == 0)
                .unwrap_or(true)
                && frame_hdr.switchable_comp_refs != 0
                && cmp::min(bw4, bh4) > 1,
              (),
            ) {
              ::core::option::Option::Some(()) => {
                let ctx = get_comp_ctx(BlockNeighbors {
                  above: ta,
                  left: &t.l,
                  location: Bxy { x: bx4, y: by4 },
                  availability: neighbors,
                })
                .map_err(|_invalid_context| ())?;
                let is_comp = matches!(
                  rav1d_msac_decode_bool_adapt(
                    &mut ts_c_2.msac,
                    &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.comp, {
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(ctx);
                      numeric_value
                    }) {
                      ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                  ),
                  Ok(EntropyBit::One)
                );

                match debug_block_info!(f, t.b) {
                  BlockTrace::Enabled => {
                    println!("Post-compflag[{}]: r={}", is_comp, ts_c_2.msac.rng);
                  }
                  BlockTrace::Disabled => {}
                }

                is_comp
              }
              ::core::option::Option::None => false,
            },
          };

          struct Inter {
            nd: Av1BlockInterNd,
            comp_type: Option<CompInterType>,
            inter_mode: u8,
            motion_mode: MotionMode,
            drl_idx: DrlProximity,
            r#ref: [i8; 2],
            interintra_type: Option<InterIntraType>,
          }

          let Inter {
            nd,
            comp_type,
            inter_mode,
            motion_mode,
            drl_idx,
            r#ref,
            interintra_type,
          } = match ::core::primitive::bool::then_some(b.skip_mode != 0, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(refs) = console_core_number_conversion::wrapped::<_, i8>(
                match frame_hdr.skip_mode.refs {
                  [element, ..] => element,
                },
              );
              let Ok(frame_hdr_refs) = console_core_number_conversion::wrapped::<_, i8>(
                match frame_hdr.skip_mode.refs {
                  [_, element, ..] => element,
                },
              );
              let r#ref = [refs, frame_hdr_refs];
              let comp_type = CompInterType::Avg;
              let inter_mode = NEARESTMV_NEARESTMV;
              let drl_idx = DrlProximity::Nearest;
              has_subpel_filter = false;

              let mut mvstack = [Default::default(); 8];
              let mut ctx = 0;
              let _candidate_count = rav1d_refmvs_find(
                &t.rt,
                &f.rf,
                &mut mvstack,
                &mut ctx,
                crate::src::refmvs::RefMvsSearch {
                  references: <crate::src::refmvs::RefMvsRefPair as ::core::convert::From<
                    [::core::primitive::i8; 2],
                  >>::from([
                    i8::wrapping_add(
                      match r#ref {
                        [element, ..] => element,
                      },
                      1,
                    ),
                    i8::wrapping_add(
                      match r#ref {
                        [_, element, ..] => element,
                      },
                      1,
                    ),
                  ]),
                  block: bs,
                  edges: intra_edge_flags,
                  origin: Point { x: t.b.x, y: t.b.y },
                },
                frame_hdr,
              )
              .map_err(|_invalid_motion_neighbors| ())?;

              let mut mv1d = (*match <[_]>::get(&mvstack, 0) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              })
              .mv
              .mv;
              let Ok(()) = fix_mv_precision(frame_hdr, {
                let [_explicit_part, ..] = &mut mv1d;
                _explicit_part
              });
              let Ok(()) = fix_mv_precision(frame_hdr, {
                let [_, _explicit_part, ..] = &mut mv1d;
                _explicit_part
              });
              let mv1d_2 = mv1d;

              match debug_block_info!(f, t.b) {
                BlockTrace::Enabled => {
                  println!(
                    "Post-skipmodeblock[mv=1:y={},x={},2:y={},x={},refs={}+{}",
                    (*match <[_]>::get(&mv1d_2, 0) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    })
                    .y,
                    (*match <[_]>::get(&mv1d_2, 0) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    })
                    .x,
                    (*match <[_]>::get(&mv1d_2, 1) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    })
                    .y,
                    (*match <[_]>::get(&mv1d_2, 1) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    })
                    .x,
                    *match <[_]>::get(&r#ref, 0) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                    *match <[_]>::get(&r#ref, 1) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                  );
                }
                BlockTrace::Disabled => {}
              }

              Inter {
                nd: <crate::src::levels::Av1BlockInterNd as ::core::convert::From<
                  crate::src::levels::Av1BlockInter1d,
                >>::from(Av1BlockInter1d {
                  mv: mv1d_2,
                  ..Default::default()
                }),
                comp_type: Some(comp_type),
                inter_mode,
                motion_mode: Default::default(),
                drl_idx,
                r#ref,
                interintra_type: None,
              }
            }
            ::core::option::Option::None => match ::core::primitive::bool::then_some(is_comp, ())
            {
              ::core::option::Option::Some(()) => {
                let reference_counts = ReferenceCounts::from_neighbors(BlockNeighbors {
                  above: ta,
                  left: &t.l,
                  location: Bxy { x: bx4, y: by4 },
                  availability: neighbors,
                })
                .map_err(|_invalid_reference| ())?;
                let dir_ctx = get_comp_dir_ctx(BlockNeighbors {
                  above: ta,
                  left: &t.l,
                  location: Bxy { x: bx4, y: by4 },
                  availability: neighbors,
                })
                .map_err(|_invalid_context| ())?;
                let r#ref = match ::core::primitive::bool::then_some(
                  matches!(
                    rav1d_msac_decode_bool_adapt(
                      &mut ts_c_2.msac,
                      &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.comp_dir, {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(dir_ctx);
                        numeric_value
                      }) {
                        ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                    ),
                    Ok(EntropyBit::One)
                  ),
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    let Ok(ctx1) = reference_counts.context(ReferenceContext::ForwardGroup);
                    let ref0 = match ::core::primitive::bool::then_some(
                      matches!(
                        rav1d_msac_decode_bool_adapt(
                          &mut ts_c_2.msac,
                          &mut *match <[_]>::get_mut(
                            &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.comp_fwd_ref, 0) {
                              ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                              ::core::option::Option::None =>
                                return ::core::result::Result::Err(()),
                            },
                            {
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped_index(ctx1);
                              numeric_value
                            },
                          ) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None =>
                              return ::core::result::Result::Err(()),
                          },
                        ),
                        Ok(EntropyBit::One)
                      ),
                      (),
                    ) {
                      ::core::option::Option::Some(()) => {
                        let Ok(ctx2) = reference_counts.context(ReferenceContext::FarForward);
                        i8::wrapping_add(
                          2,
                          i8::from(matches!(
                            rav1d_msac_decode_bool_adapt(
                              &mut ts_c_2.msac,
                              &mut *match <[_]>::get_mut(
                                &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.comp_fwd_ref, 2) {
                                  ::core::option::Option::Some(indexed_value_4) =>
                                    indexed_value_4,
                                  ::core::option::Option::None => {
                                    return ::core::result::Result::Err(());
                                  }
                                },
                                {
                                  let Ok(numeric_value) =
                                    console_core_number_conversion::wrapped_index(ctx2);
                                  numeric_value
                                },
                              ) {
                                ::core::option::Option::Some(indexed_value) => indexed_value,
                                ::core::option::Option::None => {
                                  return ::core::result::Result::Err(());
                                }
                              },
                            ),
                            Ok(EntropyBit::One)
                          )),
                        )
                      }
                      ::core::option::Option::None => {
                        let Ok(ctx2) = reference_counts.context(ReferenceContext::NearForward);
                        i8::from(matches!(
                          rav1d_msac_decode_bool_adapt(
                            &mut ts_c_2.msac,
                            &mut *match <[_]>::get_mut(
                              &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.comp_fwd_ref, 1) {
                                ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                                ::core::option::Option::None => {
                                  return ::core::result::Result::Err(());
                                }
                              },
                              {
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(ctx2);
                                numeric_value
                              },
                            ) {
                              ::core::option::Option::Some(indexed_value) => indexed_value,
                              ::core::option::Option::None =>
                                return ::core::result::Result::Err(()),
                            },
                          ),
                          Ok(EntropyBit::One)
                        ))
                      }
                    };

                    let Ok(ctx3) = reference_counts.context(ReferenceContext::BackwardGroup);
                    let ref1 = match ::core::primitive::bool::then_some(
                      matches!(
                        rav1d_msac_decode_bool_adapt(
                          &mut ts_c_2.msac,
                          &mut *match <[_]>::get_mut(
                            &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.comp_bwd_ref, 0) {
                              ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                              ::core::option::Option::None =>
                                return ::core::result::Result::Err(()),
                            },
                            {
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped_index(ctx3);
                              numeric_value
                            },
                          ) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None =>
                              return ::core::result::Result::Err(()),
                          },
                        ),
                        Ok(EntropyBit::One)
                      ),
                      (),
                    ) {
                      ::core::option::Option::Some(()) => 6,
                      ::core::option::Option::None => {
                        let Ok(ctx4) = reference_counts.context(ReferenceContext::NearBackward);
                        i8::wrapping_add(
                          4,
                          i8::from(matches!(
                            rav1d_msac_decode_bool_adapt(
                              &mut ts_c_2.msac,
                              &mut *match <[_]>::get_mut(
                                &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.comp_bwd_ref, 1) {
                                  ::core::option::Option::Some(indexed_value_4) =>
                                    indexed_value_4,
                                  ::core::option::Option::None => {
                                    return ::core::result::Result::Err(());
                                  }
                                },
                                {
                                  let Ok(numeric_value) =
                                    console_core_number_conversion::wrapped_index(ctx4);
                                  numeric_value
                                },
                              ) {
                                ::core::option::Option::Some(indexed_value) => indexed_value,
                                ::core::option::Option::None => {
                                  return ::core::result::Result::Err(());
                                }
                              },
                            ),
                            Ok(EntropyBit::One)
                          )),
                        )
                      }
                    };

                    [ref0, ref1]
                  }
                  ::core::option::Option::None => {
                    let Ok(uctx_p) = reference_counts.context(ReferenceContext::Direction);

                    match ::core::primitive::bool::then_some(
                      matches!(
                        rav1d_msac_decode_bool_adapt(
                          &mut ts_c_2.msac,
                          &mut *match <[_]>::get_mut(
                            &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.comp_uni_ref, 0) {
                              ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                              ::core::option::Option::None =>
                                return ::core::result::Result::Err(()),
                            },
                            {
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped_index(uctx_p);
                              numeric_value
                            },
                          ) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None =>
                              return ::core::result::Result::Err(()),
                          },
                        ),
                        Ok(EntropyBit::One)
                      ),
                      (),
                    ) {
                      ::core::option::Option::Some(()) => [4, 6],
                      ::core::option::Option::None => {
                        let Ok(uctx_p1) =
                          reference_counts.context(ReferenceContext::UnidirectionalPair);
                        let mut r#ref = [
                          0,
                          i8::wrapping_add(
                            1,
                            i8::from(matches!(
                              rav1d_msac_decode_bool_adapt(
                                &mut ts_c_2.msac,
                                &mut *match <[_]>::get_mut(
                                  &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.comp_uni_ref, 1)
                                  {
                                    ::core::option::Option::Some(indexed_value_4) =>
                                      indexed_value_4,
                                    ::core::option::Option::None => {
                                      return ::core::result::Result::Err(());
                                    }
                                  },
                                  {
                                    let Ok(numeric_value) =
                                      console_core_number_conversion::wrapped_index(uctx_p1);
                                    numeric_value
                                  },
                                ) {
                                  ::core::option::Option::Some(indexed_value) => indexed_value,
                                  ::core::option::Option::None => {
                                    return ::core::result::Result::Err(());
                                  }
                                },
                              ),
                              Ok(EntropyBit::One)
                            )),
                          ),
                        ];

                        match ::core::primitive::bool::then_some(
                          (match r#ref {
                            [_, element, ..] => element,
                          }) == 2,
                          (),
                        ) {
                          ::core::option::Option::Some(()) => {
                            let Ok(uctx_p2) =
                              reference_counts.context(ReferenceContext::FarForward);

                            {
                              let numeric_amount = i8::from(matches!(
                                rav1d_msac_decode_bool_adapt(
                                  &mut ts_c_2.msac,
                                  &mut *match <[_]>::get_mut(
                                    &mut *match <[_]>::get_mut(
                                      &mut *ts_c_2.cdf.mi.comp_uni_ref,
                                      2
                                    ) {
                                      ::core::option::Option::Some(indexed_value_4) => {
                                        indexed_value_4
                                      }
                                      ::core::option::Option::None => {
                                        return ::core::result::Result::Err(());
                                      }
                                    },
                                    {
                                      let Ok(numeric_value) =
                                        console_core_number_conversion::wrapped_index(uctx_p2);
                                      numeric_value
                                    },
                                  ) {
                                    ::core::option::Option::Some(indexed_value) => indexed_value,
                                    ::core::option::Option::None => {
                                      return ::core::result::Result::Err(());
                                    }
                                  },
                                ),
                                Ok(EntropyBit::One)
                              ));
                              let numeric_target = {
                                let [_, _explicit_part, ..] = &mut r#ref;
                                _explicit_part
                              };
                              *numeric_target = i8::wrapping_add(*numeric_target, numeric_amount);
                            };
                          }
                          ::core::option::Option::None => {}
                        }

                        r#ref
                      }
                    }
                  }
                };

                match debug_block_info!(f, t.b) {
                  BlockTrace::Enabled => {
                    println!(
                      "Post-refs[{}/{}]: r={}",
                      *match <[_]>::get(&r#ref, 0) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                      *match <[_]>::get(&r#ref, 1) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                      ts_c_2.msac.rng,
                    );
                  }
                  BlockTrace::Disabled => {}
                }

                let mut mvstack = [Default::default(); 8];
                let mut ctx = 0;
                let n_mvs = rav1d_refmvs_find(
                  &t.rt,
                  &f.rf,
                  &mut mvstack,
                  &mut ctx,
                  crate::src::refmvs::RefMvsSearch {
                    references: <crate::src::refmvs::RefMvsRefPair as ::core::convert::From<
                      [::core::primitive::i8; 2],
                    >>::from([
                      i8::wrapping_add(
                        match r#ref {
                          [element, ..] => element,
                        },
                        1,
                      ),
                      i8::wrapping_add(
                        match r#ref {
                          [_, element, ..] => element,
                        },
                        1,
                      ),
                    ]),
                    block: bs,
                    edges: intra_edge_flags,
                    origin: Point { x: t.b.x, y: t.b.y },
                  },
                  frame_hdr,
                )
                .map_err(|_invalid_motion_neighbors| ())?;

                let Ok(inter_mode) = rav1d_msac_decode_symbol_adapt8(
                  &mut ts_c_2.msac,
                  &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.comp_inter_mode, {
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(ctx);
                    numeric_value
                  }) {
                    ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                  u8::wrapping_sub(
                    {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u8>(N_COMP_INTER_PRED_MODES);
                      numeric_value
                    },
                    1,
                  ),
                );

                match debug_block_info!(f, t.b) {
                  BlockTrace::Enabled => {
                    println!(
                      "Post-compintermode[{},ctx={},n_mvs={}]: r={}",
                      inter_mode, ctx, n_mvs, ts_c_2.msac.rng,
                    );
                  }
                  BlockTrace::Disabled => {}
                }

                let im = &*match <[_]>::get(&dav1d_comp_inter_pred_modes, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped_index(inter_mode);
                  numeric_value
                }) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                };
                let mut drl_idx = DrlProximity::Nearest;

                match ::core::primitive::bool::then_some(inter_mode == NEWMV_NEWMV, ()) {
                  ::core::option::Option::Some(()) => {
                    match ::core::primitive::bool::then_some(n_mvs > 1, ()) {
                      ::core::option::Option::Some(()) => {
                        let drl_ctx_v1 =
                          get_drl_context(&mvstack, 0).map_err(|_invalid_context| ())?;

                        match ::core::primitive::bool::then_some(
                          matches!(
                            rav1d_msac_decode_bool_adapt(
                              &mut ts_c_2.msac,
                              &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.drl_bit, {
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(drl_ctx_v1);
                                numeric_value
                              }) {
                                ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                                ::core::option::Option::None =>
                                  return ::core::result::Result::Err(()),
                              },
                            ),
                            Ok(EntropyBit::One)
                          ),
                          (),
                        ) {
                          ::core::option::Option::Some(()) => {
                            drl_idx = DrlProximity::Nearer;

                            match ::core::primitive::bool::then_some(n_mvs > 2, ()) {
                              ::core::option::Option::Some(()) => {
                                let drl_ctx_v2 =
                                  get_drl_context(&mvstack, 1).map_err(|_invalid_context| ())?;

                                match ::core::primitive::bool::then_some(
                                  matches!(
                                    rav1d_msac_decode_bool_adapt(
                                      &mut ts_c_2.msac,
                                      &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.drl_bit, {
                                        let Ok(numeric_value) =
                                          console_core_number_conversion::wrapped_index(
                                            drl_ctx_v2,
                                          );
                                        numeric_value
                                      }) {
                                        ::core::option::Option::Some(indexed_value_4) =>
                                          indexed_value_4,
                                        ::core::option::Option::None => {
                                          return ::core::result::Result::Err(());
                                        }
                                      },
                                    ),
                                    Ok(EntropyBit::One)
                                  ),
                                  (),
                                ) {
                                  ::core::option::Option::Some(()) => {
                                    drl_idx = DrlProximity::Near;
                                  }
                                  ::core::option::Option::None => {}
                                }
                              }
                              ::core::option::Option::None => {}
                            }
                          }
                          ::core::option::Option::None => {}
                        }

                        match debug_block_info!(f, t.b) {
                          BlockTrace::Enabled => {
                            println!(
                              "Post-drlidx[{:?},n_mvs={}]: r={}",
                              drl_idx, n_mvs, ts_c_2.msac.rng,
                            );
                          }
                          BlockTrace::Disabled => {}
                        }
                      }
                      ::core::option::Option::None => {}
                    }
                  }
                  ::core::option::Option::None => match ::core::primitive::bool::then_some(
                    (match im {
                      &[element, ..] => element,
                    }) == NEARMV
                      || (match im {
                        &[_, element, ..] => element,
                      }) == NEARMV,
                    (),
                  ) {
                    ::core::option::Option::Some(()) => {
                      drl_idx = DrlProximity::Nearer;

                      match ::core::primitive::bool::then_some(n_mvs > 2, ()) {
                        ::core::option::Option::Some(()) => {
                          let drl_ctx_v2 =
                            get_drl_context(&mvstack, 1).map_err(|_invalid_context| ())?;

                          match ::core::primitive::bool::then_some(
                            matches!(
                              rav1d_msac_decode_bool_adapt(
                                &mut ts_c_2.msac,
                                &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.drl_bit, {
                                  let Ok(numeric_value) =
                                    console_core_number_conversion::wrapped_index(drl_ctx_v2);
                                  numeric_value
                                }) {
                                  ::core::option::Option::Some(indexed_value_4) =>
                                    indexed_value_4,
                                  ::core::option::Option::None => {
                                    return ::core::result::Result::Err(());
                                  }
                                },
                              ),
                              Ok(EntropyBit::One)
                            ),
                            (),
                          ) {
                            ::core::option::Option::Some(()) => {
                              drl_idx = DrlProximity::Near;

                              match ::core::primitive::bool::then_some(n_mvs > 3, ()) {
                                ::core::option::Option::Some(()) => {
                                  let drl_ctx_v3 = get_drl_context(&mvstack, 2)
                                    .map_err(|_invalid_context| ())?;

                                  match ::core::primitive::bool::then_some(
                                    matches!(
                                      rav1d_msac_decode_bool_adapt(
                                        &mut ts_c_2.msac,
                                        &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.drl_bit, {
                                          let Ok(numeric_value) =
                                            console_core_number_conversion::wrapped_index(
                                              drl_ctx_v3,
                                            );
                                          numeric_value
                                        }) {
                                          ::core::option::Option::Some(indexed_value_4) => {
                                            indexed_value_4
                                          }
                                          ::core::option::Option::None => {
                                            return ::core::result::Result::Err(());
                                          }
                                        },
                                      ),
                                      Ok(EntropyBit::One)
                                    ),
                                    (),
                                  ) {
                                    ::core::option::Option::Some(()) => {
                                      drl_idx = DrlProximity::Nearish;
                                    }
                                    ::core::option::Option::None => {}
                                  }
                                }
                                ::core::option::Option::None => {}
                              }
                            }
                            ::core::option::Option::None => {}
                          }

                          match debug_block_info!(f, t.b) {
                            BlockTrace::Enabled => {
                              println!(
                                "Post-drlidx[{:?},n_mvs={}]: r={}",
                                drl_idx, n_mvs, ts_c_2.msac.rng,
                              );
                            }
                            BlockTrace::Disabled => {}
                          }
                        }
                        ::core::option::Option::None => {}
                      }
                    }
                    ::core::option::Option::None => {}
                  },
                }

                let drl_idx_2 = drl_idx;

                has_subpel_filter = cmp::min(bw4, bh4) == 1 || inter_mode != GLOBALMV_GLOBALMV;
                let Ok(candidate_rank) = drl_idx_2.ordinal();
                let Ok(candidate_index) = console_core_number_conversion::index(candidate_rank);
                let predictors = match mvstack.get(candidate_index) {
                  Some(candidate) => candidate.mv.mv.map(Some),
                  None => [None; 2],
                };
                let mut mv1d = [Mv::default(); 2];

                for (((motion, mode), slot), predictor) in
                  mv1d.iter_mut().zip(*im).zip(r#ref).zip(predictors)
                {
                  *motion = match mode {
                    NEARMV | NEARESTMV => {
                      let mut vector = predictor.ok_or(())?;
                      let Ok(()) = fix_mv_precision(frame_hdr, &mut vector);
                      vector
                    }
                    GLOBALMV => {
                      let Ok(reference_index) =
                        console_core_number_conversion::wrapped_index(slot);
                      let global = frame_hdr.gmv.get(reference_index).ok_or(())?;
                      has_subpel_filter |= global.r#type == Rav1dWarpedMotionType::Translation;
                      let block = Rectangle {
                        origin: Point { x: t.b.x, y: t.b.y },
                        size: Size {
                          width: bw4,
                          height: bh4,
                        },
                      };
                      let Ok(vector) = get_gmv_2d(global, block, frame_hdr);
                      vector
                    }
                    NEWMV => {
                      let mut vector = predictor.ok_or(())?;
                      read_mv_residual(ts_c_2, &mut vector, residual_precision)
                        .map_err(|_invalid_motion_residual| ())?;
                      vector
                    }
                    _ => return Err(()),
                  };
                }

                match debug_block_info!(f, t.b) {
                  BlockTrace::Enabled => {
                    println!(
                      "Post-residual_mv[1:y={},x={},2:y={},x={}]: r={}",
                      (*match <[_]>::get(&mv1d, 0) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      })
                      .y,
                      (*match <[_]>::get(&mv1d, 0) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      })
                      .x,
                      (*match <[_]>::get(&mv1d, 1) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      })
                      .y,
                      (*match <[_]>::get(&mv1d, 1) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      })
                      .x,
                      ts_c_2.msac.rng,
                    );
                  }
                  BlockTrace::Disabled => {}
                }

                let is_segwedge;

                match ::core::primitive::bool::then_some(seq_hdr.masked_compound != 0, ()) {
                  ::core::option::Option::Some(()) => {
                    let mask_ctx = get_mask_comp_ctx(BlockNeighbors {
                      above: ta,
                      left: &t.l,
                      location: Bxy { x: bx4, y: by4 },
                      availability: neighbors,
                    })
                    .map_err(|_invalid_context| ())?;
                    is_segwedge = matches!(
                      rav1d_msac_decode_bool_adapt(
                        &mut ts_c_2.msac,
                        &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.mask_comp, {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(mask_ctx);
                          numeric_value
                        }) {
                          ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        },
                      ),
                      Ok(EntropyBit::One)
                    );

                    match debug_block_info!(f, t.b) {
                      BlockTrace::Enabled => {
                        println!(
                          "Post-segwedge_vs_jntavg[{},ctx={}]: r={}",
                          is_segwedge, mask_ctx, ts_c_2.msac.rng,
                        );
                      }
                      BlockTrace::Disabled => {}
                    }
                  }
                  ::core::option::Option::None => {
                    is_segwedge = false;
                  }
                }

                let comp_type;
                let mask_sign;
                let mut wedge_idx = Default::default();

                match ::core::primitive::bool::then_some(!is_segwedge, ()) {
                  ::core::option::Option::Some(()) => {
                    match ::core::primitive::bool::then_some(seq_hdr.jnt_comp != 0, ()) {
                      ::core::option::Option::Some(()) => {
                        let reference_stamp = |slot| -> Result<u32, ()> {
                          let Ok(position) = console_core_number_conversion::wrapped_index(slot);
                          let reference = f.refp.get(position).ok_or(())?;
                          let header = reference.p.frame_hdr.as_ref().ok_or(())?;
                          Ok(u32::from(header.frame_offset))
                        };
                        let [first_reference, second_reference] = r#ref;
                        let first_hint = reference_stamp(first_reference)?;
                        let second_hint = reference_stamp(second_reference)?;
                        let current_header = f.cur.frame_hdr.as_ref().ok_or(())?;
                        let hints = CompoundOrderHints {
                          width: seq_hdr.order_hint_n_bits,
                          current: u32::from(current_header.frame_offset),
                          reference_hints: [first_hint, second_hint],
                        };
                        let jnt_ctx = get_jnt_comp_ctx(
                          hints,
                          BlockNeighbors {
                            above: ta,
                            left: &t.l,
                            location: Bxy { x: bx4, y: by4 },
                            availability: neighbors,
                          },
                        )
                        .map_err(|_invalid_context| ())?;
                        comp_type = match ::core::primitive::bool::then_some(
                          matches!(
                            rav1d_msac_decode_bool_adapt(
                              &mut ts_c_2.msac,
                              &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.jnt_comp, {
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(jnt_ctx);
                                numeric_value
                              }) {
                                ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                                ::core::option::Option::None =>
                                  return ::core::result::Result::Err(()),
                              },
                            ),
                            Ok(EntropyBit::One)
                          ),
                          (),
                        ) {
                          ::core::option::Option::Some(()) => CompInterType::Avg,
                          ::core::option::Option::None => CompInterType::WeightedAvg,
                        };

                        match debug_block_info!(f, t.b) {
                          BlockTrace::Enabled => {
                            let a = ta;
                            let l = &t.l;
                            println!(
                              "Post-jnt_comp[{},ctx={}[ac:{:?},ar:{},lc:{:?},lr:{}]]: r={}",
                              comp_type == CompInterType::Avg,
                              jnt_ctx,
                              *a.comp_type.index({
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(bx4);
                                numeric_value
                              }),
                              *(*match <[_]>::get(&a.r#ref, 0) {
                                ::core::option::Option::Some(indexed_value) => indexed_value,
                                ::core::option::Option::None =>
                                  return ::core::result::Result::Err(()),
                              })
                              .index({
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(bx4);
                                numeric_value
                              }),
                              *l.comp_type.index({
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(by4);
                                numeric_value
                              }),
                              *(*match <[_]>::get(&l.r#ref, 0) {
                                ::core::option::Option::Some(indexed_value) => indexed_value,
                                ::core::option::Option::None =>
                                  return ::core::result::Result::Err(()),
                              })
                              .index({
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(by4);
                                numeric_value
                              }),
                              ts_c_2.msac.rng,
                            );
                          }
                          BlockTrace::Disabled => {}
                        }
                      }
                      ::core::option::Option::None => {
                        comp_type = CompInterType::Avg;
                      }
                    }

                    mask_sign = Default::default();
                  }
                  ::core::option::Option::None => {
                    comp_type = match ::core::primitive::bool::then_some(
                      wedge_allowed_mask
                        & u32::wrapping_shl(1, {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped::<_, u32>({
                              let Ok(ordinal) = bs.ordinal();
                              ordinal
                            });
                          numeric_value
                        })
                        != 0,
                      (),
                    ) {
                      ::core::option::Option::Some(()) => {
                        let Ok(ctx_2) = console_core_number_conversion::wrapped_index(
                          *match <[_]>::get(&dav1d_wedge_ctx_lut, {
                            let Ok(table_index) = console_core_number_conversion::index({
                              let Ok(ordinal) = bs.ordinal();
                              ordinal
                            });
                            table_index
                          }) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None => {
                              return ::core::result::Result::Err(());
                            }
                          },
                        );
                        let comp_type_2 = match ::core::primitive::bool::then_some(
                          matches!(
                            rav1d_msac_decode_bool_adapt(
                              &mut ts_c_2.msac,
                              &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.wedge_comp, ctx_2) {
                                ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                                ::core::option::Option::None =>
                                  return ::core::result::Result::Err(()),
                              },
                            ),
                            Ok(EntropyBit::One)
                          ),
                          (),
                        ) {
                          ::core::option::Option::Some(()) => CompInterType::Seg,
                          ::core::option::Option::None => CompInterType::Wedge,
                        };

                        match ::core::primitive::bool::then_some(
                          comp_type_2 == CompInterType::Wedge,
                          (),
                        ) {
                          ::core::option::Option::Some(()) => {
                            wedge_idx = {
                              let Ok(numeric_value) = console_core_number_conversion::wrapped::<
                                _,
                                u8,
                              >({
                                let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt16(
                                  &mut ts_c_2.msac,
                                  &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.wedge_idx, ctx_2)
                                  {
                                    ::core::option::Option::Some(indexed_value_4) => {
                                      indexed_value_4
                                    }
                                    ::core::option::Option::None => {
                                      return ::core::result::Result::Err(());
                                    }
                                  },
                                  15,
                                );
                                explicit_value
                              });
                              numeric_value
                            };
                          }
                          ::core::option::Option::None => {}
                        }

                        comp_type_2
                      }
                      ::core::option::Option::None => CompInterType::Seg,
                    };

                    mask_sign = matches!(
                      rav1d_msac_decode_bool_equi(&mut ts_c_2.msac),
                      Ok(EntropyBit::One)
                    );

                    match debug_block_info!(f, t.b) {
                      BlockTrace::Enabled => {
                        println!(
                          "Post-seg/wedge[{},wedge_idx={},sign={}]: r={}",
                          comp_type == CompInterType::Wedge,
                          wedge_idx,
                          mask_sign,
                          ts_c_2.msac.rng,
                        );
                      }
                      BlockTrace::Disabled => {}
                    }
                  }
                }

                let wedge_idx_2 = wedge_idx;

                Inter {
                  nd: <crate::src::levels::Av1BlockInterNd as ::core::convert::From<
                    crate::src::levels::Av1BlockInter1d,
                  >>::from(Av1BlockInter1d {
                    mv: mv1d,
                    wedge_idx: wedge_idx_2,
                    mask_sign: u8::from(mask_sign),
                    ..Default::default()
                  }),
                  comp_type: Some(comp_type),
                  inter_mode,
                  motion_mode: Default::default(),
                  drl_idx: drl_idx_2,
                  r#ref,
                  interintra_type: None,
                }
              }
              ::core::option::Option::None => {
                let ref0 = match seg.filter(|seg_5| seg_5.r#ref > 0) {
                  Some(seg_6) => i8::wrapping_sub(
                    {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, i8>(seg_6.r#ref);
                      numeric_value
                    },
                    1,
                  ),
                  None => match ::core::primitive::bool::then_some(
                    seg
                      .filter(|seg_7| seg_7.globalmv != 0 || seg_7.skip != 0)
                      .is_some(),
                    (),
                  ) {
                    ::core::option::Option::Some(()) => 0,
                    ::core::option::Option::None => {
                      let reference_counts = ReferenceCounts::from_neighbors(BlockNeighbors {
                        above: ta,
                        left: &t.l,
                        location: Bxy { x: bx4, y: by4 },
                        availability: neighbors,
                      })
                      .map_err(|_invalid_reference| ())?;
                      let Ok(ctx1) = reference_counts.context(ReferenceContext::Direction);
                      let ref0 = match ::core::primitive::bool::then_some(
                        matches!(
                          rav1d_msac_decode_bool_adapt(
                            &mut ts_c_2.msac,
                            &mut *match <[_]>::get_mut(
                              &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.r#ref, 0) {
                                ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                                ::core::option::Option::None =>
                                  return ::core::result::Result::Err(()),
                              },
                              {
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(ctx1);
                                numeric_value
                              },
                            ) {
                              ::core::option::Option::Some(indexed_value) => indexed_value,
                              ::core::option::Option::None =>
                                return ::core::result::Result::Err(()),
                            },
                          ),
                          Ok(EntropyBit::One)
                        ),
                        (),
                      ) {
                        ::core::option::Option::Some(()) => {
                          let Ok(ctx2) =
                            reference_counts.context(ReferenceContext::BackwardGroup);

                          match ::core::primitive::bool::then_some(
                            matches!(
                              rav1d_msac_decode_bool_adapt(
                                &mut ts_c_2.msac,
                                &mut *match <[_]>::get_mut(
                                  &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.r#ref, 1) {
                                    ::core::option::Option::Some(indexed_value_4) =>
                                      indexed_value_4,
                                    ::core::option::Option::None => {
                                      return ::core::result::Result::Err(());
                                    }
                                  },
                                  {
                                    let Ok(numeric_value) =
                                      console_core_number_conversion::wrapped_index(ctx2);
                                    numeric_value
                                  },
                                ) {
                                  ::core::option::Option::Some(indexed_value) => indexed_value,
                                  ::core::option::Option::None => {
                                    return ::core::result::Result::Err(());
                                  }
                                },
                              ),
                              Ok(EntropyBit::One)
                            ),
                            (),
                          ) {
                            ::core::option::Option::Some(()) => 6,
                            ::core::option::Option::None => {
                              let Ok(ctx3) =
                                reference_counts.context(ReferenceContext::NearBackward);
                              i8::wrapping_add(
                                4,
                                i8::from(matches!(
                                  rav1d_msac_decode_bool_adapt(
                                    &mut ts_c_2.msac,
                                    &mut *match <[_]>::get_mut(
                                      &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.r#ref, 5) {
                                        ::core::option::Option::Some(indexed_value_4) => {
                                          indexed_value_4
                                        }
                                        ::core::option::Option::None => {
                                          return ::core::result::Result::Err(());
                                        }
                                      },
                                      {
                                        let Ok(numeric_value) =
                                          console_core_number_conversion::wrapped_index(ctx3);
                                        numeric_value
                                      },
                                    ) {
                                      ::core::option::Option::Some(indexed_value) =>
                                        indexed_value,
                                      ::core::option::Option::None => {
                                        return ::core::result::Result::Err(());
                                      }
                                    },
                                  ),
                                  Ok(EntropyBit::One)
                                )),
                              )
                            }
                          }
                        }
                        ::core::option::Option::None => {
                          let Ok(ctx2) = reference_counts.context(ReferenceContext::ForwardGroup);

                          match ::core::primitive::bool::then_some(
                            matches!(
                              rav1d_msac_decode_bool_adapt(
                                &mut ts_c_2.msac,
                                &mut *match <[_]>::get_mut(
                                  &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.r#ref, 2) {
                                    ::core::option::Option::Some(indexed_value_4) =>
                                      indexed_value_4,
                                    ::core::option::Option::None => {
                                      return ::core::result::Result::Err(());
                                    }
                                  },
                                  {
                                    let Ok(numeric_value) =
                                      console_core_number_conversion::wrapped_index(ctx2);
                                    numeric_value
                                  },
                                ) {
                                  ::core::option::Option::Some(indexed_value) => indexed_value,
                                  ::core::option::Option::None => {
                                    return ::core::result::Result::Err(());
                                  }
                                },
                              ),
                              Ok(EntropyBit::One)
                            ),
                            (),
                          ) {
                            ::core::option::Option::Some(()) => {
                              let Ok(ctx3) =
                                reference_counts.context(ReferenceContext::FarForward);
                              i8::wrapping_add(
                                2,
                                i8::from(matches!(
                                  rav1d_msac_decode_bool_adapt(
                                    &mut ts_c_2.msac,
                                    &mut *match <[_]>::get_mut(
                                      &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.r#ref, 4) {
                                        ::core::option::Option::Some(indexed_value_4) => {
                                          indexed_value_4
                                        }
                                        ::core::option::Option::None => {
                                          return ::core::result::Result::Err(());
                                        }
                                      },
                                      {
                                        let Ok(numeric_value) =
                                          console_core_number_conversion::wrapped_index(ctx3);
                                        numeric_value
                                      },
                                    ) {
                                      ::core::option::Option::Some(indexed_value) =>
                                        indexed_value,
                                      ::core::option::Option::None => {
                                        return ::core::result::Result::Err(());
                                      }
                                    },
                                  ),
                                  Ok(EntropyBit::One)
                                )),
                              )
                            }
                            ::core::option::Option::None => {
                              let Ok(ctx3) =
                                reference_counts.context(ReferenceContext::NearForward);
                              i8::from(matches!(
                                rav1d_msac_decode_bool_adapt(
                                  &mut ts_c_2.msac,
                                  &mut *match <[_]>::get_mut(
                                    &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.r#ref, 3) {
                                      ::core::option::Option::Some(indexed_value_4) => {
                                        indexed_value_4
                                      }
                                      ::core::option::Option::None => {
                                        return ::core::result::Result::Err(());
                                      }
                                    },
                                    {
                                      let Ok(numeric_value) =
                                        console_core_number_conversion::wrapped_index(ctx3);
                                      numeric_value
                                    },
                                  ) {
                                    ::core::option::Option::Some(indexed_value) => indexed_value,
                                    ::core::option::Option::None => {
                                      return ::core::result::Result::Err(());
                                    }
                                  },
                                ),
                                Ok(EntropyBit::One)
                              ))
                            }
                          }
                        }
                      };

                      match debug_block_info!(f, t.b) {
                        BlockTrace::Enabled => {
                          println!("Post-ref[{}]: r={}", ref0, ts_c_2.msac.rng);
                        }
                        BlockTrace::Disabled => {}
                      }

                      ref0
                    }
                  },
                };
                let r#ref = [ref0, -1];

                let mut mvstack = [Default::default(); 8];
                let mut ctx = 0;
                let n_mvs = rav1d_refmvs_find(
                  &t.rt,
                  &f.rf,
                  &mut mvstack,
                  &mut ctx,
                  crate::src::refmvs::RefMvsSearch {
                    references: RefMvsRefPair {
                      r#ref: [
                        i8::wrapping_add(
                          match r#ref {
                            [element, ..] => element,
                          },
                          1,
                        ),
                        -1,
                      ],
                    },
                    block: bs,
                    edges: intra_edge_flags,
                    origin: Point { x: t.b.x, y: t.b.y },
                  },
                  frame_hdr,
                )
                .map_err(|_invalid_motion_neighbors| ())?;

                let inter_mode;
                let mut mv1d0;
                let mut drl_idx;

                match ::core::primitive::bool::then_some(
                  (match seg.map(|seg_8| seg_8.skip != 0 || seg_8.globalmv != 0) {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => false,
                  }) || matches!(
                    rav1d_msac_decode_bool_adapt(
                      &mut ts_c_2.msac,
                      &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.newmv_mode, {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(ctx & 7);
                        numeric_value
                      }) {
                        ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                    ),
                    Ok(EntropyBit::One)
                  ),
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    match ::core::primitive::bool::then_some(
                      (match seg.map(|seg_9| seg_9.skip != 0 || seg_9.globalmv != 0) {
                        ::core::option::Option::Some(option_value) => option_value,
                        ::core::option::Option::None => false,
                      }) || !matches!(
                        rav1d_msac_decode_bool_adapt(
                          &mut ts_c_2.msac,
                          &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.globalmv_mode, {
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              i32::wrapping_shr(ctx, 3_u32) & 1,
                            );
                            numeric_value
                          }) {
                            ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                            ::core::option::Option::None =>
                              return ::core::result::Result::Err(()),
                          },
                        ),
                        Ok(EntropyBit::One)
                      ),
                      (),
                    ) {
                      ::core::option::Option::Some(()) => {
                        inter_mode = GLOBALMV;
                        let Ok(gmv_2d) = get_gmv_2d(
                          &*match <[_]>::get(&frame_hdr.gmv, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_index(match r#ref {
                                [element, ..] => element,
                              });
                            numeric_value
                          }) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None => {
                              return ::core::result::Result::Err(());
                            }
                          },
                          Rectangle {
                            origin: Point { x: t.b.x, y: t.b.y },
                            size: Size {
                              width: bw4,
                              height: bh4,
                            },
                          },
                          frame_hdr,
                        );
                        mv1d0 = gmv_2d;
                        has_subpel_filter = cmp::min(bw4, bh4) == 1
                          || (*match <[_]>::get(&frame_hdr.gmv, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_index(match r#ref {
                                [element, ..] => element,
                              });
                            numeric_value
                          }) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None => {
                              return ::core::result::Result::Err(());
                            }
                          })
                          .r#type
                            == Rav1dWarpedMotionType::Translation;

                        drl_idx = Default::default();
                      }
                      ::core::option::Option::None => {
                        has_subpel_filter = true;

                        match ::core::primitive::bool::then_some(
                          matches!(
                            rav1d_msac_decode_bool_adapt(
                              &mut ts_c_2.msac,
                              &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.refmv_mode, {
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(
                                    i32::wrapping_shr(ctx, 4_u32) & 15,
                                  );
                                numeric_value
                              }) {
                                ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                                ::core::option::Option::None =>
                                  return ::core::result::Result::Err(()),
                              },
                            ),
                            Ok(EntropyBit::One)
                          ),
                          (),
                        ) {
                          ::core::option::Option::Some(()) => {
                            inter_mode = NEARMV;
                            drl_idx = DrlProximity::Nearer;

                            match ::core::primitive::bool::then_some(n_mvs > 2, ()) {
                              ::core::option::Option::Some(()) => {
                                let drl_ctx_v2 =
                                  get_drl_context(&mvstack, 1).map_err(|_invalid_context| ())?;

                                match ::core::primitive::bool::then_some(
                                  matches!(
                                    rav1d_msac_decode_bool_adapt(
                                      &mut ts_c_2.msac,
                                      &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.drl_bit, {
                                        let Ok(numeric_value) =
                                          console_core_number_conversion::wrapped_index(
                                            drl_ctx_v2,
                                          );
                                        numeric_value
                                      }) {
                                        ::core::option::Option::Some(indexed_value_4) => {
                                          indexed_value_4
                                        }
                                        ::core::option::Option::None => {
                                          return ::core::result::Result::Err(());
                                        }
                                      },
                                    ),
                                    Ok(EntropyBit::One)
                                  ),
                                  (),
                                ) {
                                  ::core::option::Option::Some(()) => {
                                    drl_idx = DrlProximity::Near;

                                    match ::core::primitive::bool::then_some(n_mvs > 3, ()) {
                                      ::core::option::Option::Some(()) => {
                                        let drl_ctx_v3 = get_drl_context(&mvstack, 2)
                                          .map_err(|_invalid_context| ())?;

                                        match ::core::primitive::bool::then_some(
                                          matches!(
                                            rav1d_msac_decode_bool_adapt(
                                              &mut ts_c_2.msac,
                                              &mut *match <[_]>::get_mut(
                                                &mut *ts_c_2.cdf.mi.drl_bit,
                                                {
                                                  let Ok(numeric_value) =
                                                    console_core_number_conversion::wrapped_index(
                                                      drl_ctx_v3,
                                                    );
                                                  numeric_value
                                                }
                                              ) {
                                                ::core::option::Option::Some(indexed_value_4) => {
                                                  indexed_value_4
                                                }
                                                ::core::option::Option::None => {
                                                  return ::core::result::Result::Err(());
                                                }
                                              },
                                            ),
                                            Ok(EntropyBit::One)
                                          ),
                                          (),
                                        ) {
                                          ::core::option::Option::Some(()) => {
                                            drl_idx = DrlProximity::Nearish;
                                          }
                                          ::core::option::Option::None => {}
                                        }
                                      }
                                      ::core::option::Option::None => {}
                                    }
                                  }
                                  ::core::option::Option::None => {}
                                }
                              }
                              ::core::option::Option::None => {}
                            }
                          }
                          ::core::option::Option::None => {
                            inter_mode = NEARESTMV;
                            drl_idx = DrlProximity::Nearest;
                          }
                        }

                        mv1d0 = *match <[_]>::get(
                          &(*match <[_]>::get(&mvstack, {
                            let Ok(table_index) = console_core_number_conversion::index({
                              let Ok(ordinal) = drl_idx.ordinal();
                              ordinal
                            });
                            table_index
                          }) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None => {
                              return ::core::result::Result::Err(());
                            }
                          })
                          .mv
                          .mv,
                          0,
                        ) {
                          ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        };

                        match ::core::primitive::bool::then_some(drl_idx < DrlProximity::Near, ())
                        {
                          ::core::option::Option::Some(()) => {
                            let Ok(()) = fix_mv_precision(frame_hdr, &mut mv1d0);
                          }
                          ::core::option::Option::None => {}
                        }
                      }
                    }

                    match debug_block_info!(f, t.b) {
                      BlockTrace::Enabled => {
                        println!(
                          "Post-intermode[{},drl={:?},mv=y:{},x:{},n_mvs={}]: r={}",
                          inter_mode, drl_idx, mv1d0.y, mv1d0.x, n_mvs, ts_c_2.msac.rng,
                        );
                      }
                      BlockTrace::Disabled => {}
                    }
                  }
                  ::core::option::Option::None => {
                    has_subpel_filter = true;
                    inter_mode = NEWMV;
                    drl_idx = DrlProximity::Nearest;

                    match ::core::primitive::bool::then_some(n_mvs > 1, ()) {
                      ::core::option::Option::Some(()) => {
                        let drl_ctx_v1 =
                          get_drl_context(&mvstack, 0).map_err(|_invalid_context| ())?;

                        match ::core::primitive::bool::then_some(
                          matches!(
                            rav1d_msac_decode_bool_adapt(
                              &mut ts_c_2.msac,
                              &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.drl_bit, {
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(drl_ctx_v1);
                                numeric_value
                              }) {
                                ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                                ::core::option::Option::None =>
                                  return ::core::result::Result::Err(()),
                              },
                            ),
                            Ok(EntropyBit::One)
                          ),
                          (),
                        ) {
                          ::core::option::Option::Some(()) => {
                            drl_idx = DrlProximity::Nearer;

                            match ::core::primitive::bool::then_some(n_mvs > 2, ()) {
                              ::core::option::Option::Some(()) => {
                                let drl_ctx_v2 =
                                  get_drl_context(&mvstack, 1).map_err(|_invalid_context| ())?;

                                match ::core::primitive::bool::then_some(
                                  matches!(
                                    rav1d_msac_decode_bool_adapt(
                                      &mut ts_c_2.msac,
                                      &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.drl_bit, {
                                        let Ok(numeric_value) =
                                          console_core_number_conversion::wrapped_index(
                                            drl_ctx_v2,
                                          );
                                        numeric_value
                                      }) {
                                        ::core::option::Option::Some(indexed_value_4) => {
                                          indexed_value_4
                                        }
                                        ::core::option::Option::None => {
                                          return ::core::result::Result::Err(());
                                        }
                                      },
                                    ),
                                    Ok(EntropyBit::One)
                                  ),
                                  (),
                                ) {
                                  ::core::option::Option::Some(()) => {
                                    drl_idx = DrlProximity::Near;
                                  }
                                  ::core::option::Option::None => {}
                                }
                              }
                              ::core::option::Option::None => {}
                            }
                          }
                          ::core::option::Option::None => {}
                        }
                      }
                      ::core::option::Option::None => {}
                    }

                    match ::core::primitive::bool::then_some(n_mvs > 1, ()) {
                      ::core::option::Option::Some(()) => {
                        mv1d0 = *match <[_]>::get(
                          &(*match <[_]>::get(&mvstack, {
                            let Ok(table_index) = console_core_number_conversion::index({
                              let Ok(ordinal) = drl_idx.ordinal();
                              ordinal
                            });
                            table_index
                          }) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None => {
                              return ::core::result::Result::Err(());
                            }
                          })
                          .mv
                          .mv,
                          0,
                        ) {
                          ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        };
                      }
                      ::core::option::Option::None => {
                        assert_eq!(drl_idx, DrlProximity::Nearest);
                        mv1d0 = *match <[_]>::get(
                          &(*match <[_]>::get(&mvstack, 0) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None => {
                              return ::core::result::Result::Err(());
                            }
                          })
                          .mv
                          .mv,
                          0,
                        ) {
                          ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        };
                        let Ok(()) = fix_mv_precision(frame_hdr, &mut mv1d0);
                      }
                    }

                    match debug_block_info!(f, t.b) {
                      BlockTrace::Enabled => {
                        println!(
                          "Post-intermode[{},drl={:?}]: r={}",
                          inter_mode, drl_idx, ts_c_2.msac.rng,
                        );
                      }
                      BlockTrace::Disabled => {}
                    }

                    read_mv_residual(ts_c_2, &mut mv1d0, residual_precision)
                      .map_err(|_invalid_motion_residual| ())?;

                    match debug_block_info!(f, t.b) {
                      BlockTrace::Enabled => {
                        println!(
                          "Post-residualmv[mv=y:{},x:{}]: r={}",
                          mv1d0.y, mv1d0.x, ts_c_2.msac.rng,
                        );
                      }
                      BlockTrace::Disabled => {}
                    }
                  }
                }

                let drl_idx_3 = drl_idx;
                let mv1d0_2 = mv1d0;

                let interintra_mode;
                let interintra_type;
                let mut wedge_idx = Default::default();
                let Ok(ii_sz_grp) = console_core_number_conversion::wrapped::<_, i32>(
                  *match <[_]>::get(&dav1d_ymode_size_context, {
                    let Ok(table_index) = console_core_number_conversion::index({
                      let Ok(ordinal) = bs.ordinal();
                      ordinal
                    });
                    table_index
                  }) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                );

                match ::core::primitive::bool::then_some(
                  seq_hdr.inter_intra != 0
                    && interintra_allowed_mask
                      & u32::wrapping_shl(1, {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>({
                            let Ok(ordinal) = bs.ordinal();
                            ordinal
                          });
                        numeric_value
                      })
                      != 0
                    && matches!(
                      rav1d_msac_decode_bool_adapt(
                        &mut ts_c_2.msac,
                        &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.interintra, {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(ii_sz_grp);
                          numeric_value
                        }) {
                          ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        },
                      ),
                      Ok(EntropyBit::One)
                    ),
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    interintra_mode = InterIntraPredMode::from_repr({
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index({
                        let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt4(
                          &mut ts_c_2.msac,
                          &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.interintra_mode, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_index(ii_sz_grp);
                            numeric_value
                          }) {
                            ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                            ::core::option::Option::None => {
                              return ::core::result::Result::Err(());
                            }
                          },
                          u8::wrapping_sub(
                            {
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped::<_, u8>(
                                  InterIntraPredMode::COUNT,
                                );
                              numeric_value
                            },
                            1,
                          ),
                        );
                        explicit_value
                      });
                      numeric_value
                    })
                    .ok_or(())?;
                    let Ok(wedge_ctx) = console_core_number_conversion::wrapped::<_, i32>(
                      *match <[_]>::get(&dav1d_wedge_ctx_lut, {
                        let Ok(table_index) = console_core_number_conversion::index({
                          let Ok(ordinal) = bs.ordinal();
                          ordinal
                        });
                        table_index
                      }) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                    );
                    let ii_type = match ::core::primitive::bool::then_some(
                      matches!(
                        rav1d_msac_decode_bool_adapt(
                          &mut ts_c_2.msac,
                          &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.interintra_wedge, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_index(wedge_ctx);
                            numeric_value
                          }) {
                            ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                            ::core::option::Option::None =>
                              return ::core::result::Result::Err(()),
                          },
                        ),
                        Ok(EntropyBit::One)
                      ),
                      (),
                    ) {
                      ::core::option::Option::Some(()) => InterIntraType::Wedge,
                      ::core::option::Option::None => InterIntraType::Blend,
                    };
                    interintra_type = Some(ii_type);

                    match ::core::primitive::bool::then_some(ii_type == InterIntraType::Wedge, ())
                    {
                      ::core::option::Option::Some(()) => {
                        wedge_idx = {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped::<_, u8>({
                              let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt16(
                                &mut ts_c_2.msac,
                                &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.wedge_idx, {
                                  let Ok(numeric_value) =
                                    console_core_number_conversion::wrapped_index(wedge_ctx);
                                  numeric_value
                                }) {
                                  ::core::option::Option::Some(indexed_value_4) => {
                                    indexed_value_4
                                  }
                                  ::core::option::Option::None => {
                                    return ::core::result::Result::Err(());
                                  }
                                },
                                15,
                              );
                              explicit_value
                            });
                          numeric_value
                        };
                      }
                      ::core::option::Option::None => {}
                    }
                  }
                  ::core::option::Option::None => {
                    interintra_mode = Default::default();
                    interintra_type = None;
                  }
                }

                let wedge_idx_3 = wedge_idx;

                match ::core::primitive::bool::then_some(
                  debug_block_info!(f, t.b) == BlockTrace::Enabled
                    && seq_hdr.inter_intra != 0
                    && interintra_allowed_mask
                      & u32::wrapping_shl(1, {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>({
                            let Ok(ordinal) = bs.ordinal();
                            ordinal
                          });
                        numeric_value
                      })
                      != 0,
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    println!(
                      "Post-interintra[t={:?},m={:?},w={}]: r={}",
                      interintra_type, interintra_mode, wedge_idx_3, ts_c_2.msac.rng,
                    );
                  }
                  ::core::option::Option::None => {}
                }

                let motion_mode;
                let mut matrix = None;

                match ::core::primitive::bool::then_some(
                  frame_hdr.switchable_motion_mode != 0
                    && interintra_type.is_none()
                    && cmp::min(bw4, bh4) >= 2
                    && !(!frame_hdr.force_integer_mv
                      && inter_mode == GLOBALMV
                      && (*match <[_]>::get(&frame_hdr.gmv, {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(match r#ref {
                            [element, ..] => element,
                          });
                        numeric_value
                      }) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      })
                      .r#type
                        > Rav1dWarpedMotionType::Translation)
                    && (have_left
                      && matches!(
                        odd_neighbor_prediction(&*match <[_]>::get(
                          &**t.l.intra.get_mut(),
                          ({
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_index(by4);
                            numeric_value
                          })..({
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              i32::wrapping_add(by4, h4),
                            );
                            numeric_value
                          }),
                        ) {
                          ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        }),
                        Ok(OddNeighborPrediction::HasInter)
                      )
                      || have_top
                        && matches!(
                          odd_neighbor_prediction(&ta.intra.index(
                            ({
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped_index(bx4);
                              numeric_value
                            })..({
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped_index(i32::wrapping_add(
                                  bx4, w4,
                                ));
                              numeric_value
                            }),
                          )),
                          Ok(OddNeighborPrediction::HasInter)
                        )),
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    let [reference, _other] = r#ref;
                    let mask = find_matching_ref(
                      f,
                      t,
                      WarpNeighborQuery {
                        block: Size {
                          width: bw4,
                          height: bh4,
                        },
                        visible: Size {
                          width: w4,
                          height: h4,
                        },
                        availability: NeighborAvailability::from((have_top, have_left)),
                        edge_flags: intra_edge_flags,
                        reference,
                      },
                    )
                    .map_err(|_invalid_warp_neighbor| ())?;
                    let allow_warp = i32::from(
                      (*match <[_]>::get(
                        &(*match <[_]>::get(&f.svc, {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(match r#ref {
                              [element, ..] => element,
                            });
                          numeric_value
                        }) {
                          ::core::option::Option::Some(indexed_value) => indexed_value,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        }),
                        0,
                      ) {
                        ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      })
                      .scale
                        == 0
                        && !frame_hdr.force_integer_mv
                        && frame_hdr.warp_motion != 0
                        && (match mask {
                          [element, ..] => element,
                        }) | (match mask {
                          [_, element, ..] => element,
                        }) != 0,
                    );

                    motion_mode = MotionMode::from_repr(
                      match ::core::primitive::bool::then_some(allow_warp != 0, ()) {
                        ::core::option::Option::Some(()) => {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index({
                              let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt4(
                                &mut ts_c_2.msac,
                                &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.motion_mode, {
                                  let Ok(table_index) = console_core_number_conversion::index({
                                    let Ok(ordinal) = bs.ordinal();
                                    ordinal
                                  });
                                  table_index
                                }) {
                                  ::core::option::Option::Some(indexed_value_4) => {
                                    indexed_value_4
                                  }
                                  ::core::option::Option::None => {
                                    return ::core::result::Result::Err(());
                                  }
                                },
                                2,
                              );
                              explicit_value
                            });
                          numeric_value
                        }
                        ::core::option::Option::None => {
                          match ::console_core_number_conversion::index::<::core::primitive::u8>(
                            ::core::primitive::u8::from(matches!(
                              rav1d_msac_decode_bool_adapt(
                                &mut ts_c_2.msac,
                                &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.obmc, {
                                  let Ok(table_index) = console_core_number_conversion::index({
                                    let Ok(ordinal) = bs.ordinal();
                                    ordinal
                                  });
                                  table_index
                                }) {
                                  ::core::option::Option::Some(indexed_value_4) =>
                                    indexed_value_4,
                                  ::core::option::Option::None => {
                                    return ::core::result::Result::Err(());
                                  }
                                },
                              ),
                              Ok(EntropyBit::One)
                            )),
                          ) {
                            ::core::result::Result::Ok(converted_position) => converted_position,
                          }
                        }
                      },
                    )
                    .ok_or(())?;

                    match ::core::primitive::bool::then_some(motion_mode == MotionMode::Warp, ())
                    {
                      ::core::option::Option::Some(()) => {
                        has_subpel_filter = false;
                        let grid = WarpNeighborGrid {
                          blocks: &f.rf.r,
                          tile: &t.rt,
                          origin: Point { x: t.b.x, y: t.b.y },
                        };
                        t.warpmv = derive_warpmv(
                          &grid,
                          Size {
                            width: bw4,
                            height: bh4,
                          },
                          mask,
                          mv1d0_2,
                          t.warpmv.clone(),
                        )
                        .map_err(|_invalid_warp_sample| ())?;

                        match debug_block_info!(f, t.b) {
                          BlockTrace::Enabled => {
                            println!(
                              "[ {} {} {}\n  {} {} {} ]\n\
                            alpha={}, beta={}, gamma={}, deta={}, mv=y:{},x:{}",
                              SignAbs(match t.warpmv.matrix {
                                [element, ..] => element,
                              }),
                              SignAbs(match t.warpmv.matrix {
                                [_, element, ..] => element,
                              }),
                              SignAbs(match t.warpmv.matrix {
                                [_, _, element, ..] => element,
                              }),
                              SignAbs(match t.warpmv.matrix {
                                [_, _, _, element, ..] => element,
                              }),
                              SignAbs(match t.warpmv.matrix {
                                [_, _, _, _, _explicit_element, ..] => _explicit_element,
                              }),
                              SignAbs(match t.warpmv.matrix {
                                [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
                              }),
                              SignAbs(<::core::primitive::i32 as ::core::convert::From<
                                ::core::primitive::i16,
                              >>::from(t.warpmv.alpha())),
                              SignAbs(<::core::primitive::i32 as ::core::convert::From<
                                ::core::primitive::i16,
                              >>::from(t.warpmv.beta())),
                              SignAbs(<::core::primitive::i32 as ::core::convert::From<
                                ::core::primitive::i16,
                              >>::from(t.warpmv.gamma())),
                              SignAbs(<::core::primitive::i32 as ::core::convert::From<
                                ::core::primitive::i16,
                              >>::from(t.warpmv.delta())),
                              mv1d0_2.y,
                              mv1d0_2.x,
                            );
                          }
                          BlockTrace::Disabled => {}
                        }

                        match ::core::primitive::bool::then_some(t.frame_thread.pass != 0, ()) {
                          ::core::option::Option::Some(()) => {
                            matrix = Some(
                              match ::core::primitive::bool::then_some(
                                t.warpmv.r#type == Rav1dWarpedMotionType::Affine,
                                (),
                              ) {
                                ::core::option::Option::Some(()) => [
                                  i32::wrapping_sub(
                                    match t.warpmv.matrix {
                                      [_, _, element, ..] => element,
                                    },
                                    0x10000,
                                  ),
                                  match t.warpmv.matrix {
                                    [_, _, _, _explicit_element, ..] => _explicit_element,
                                  },
                                  match t.warpmv.matrix {
                                    [_, _, _, _, _explicit_element, ..] => _explicit_element,
                                  },
                                  i32::wrapping_sub(
                                    match t.warpmv.matrix {
                                      [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
                                    },
                                    0x10000,
                                  ),
                                ]
                                .map(|coef| {
                                  let Ok(coef) =
                                    console_core_number_conversion::wrapped::<_, i16>(coef);

                                  debug_assert!(coef < i16::wrapping_shl(1, 13_u32));
                                  debug_assert!(
                                    coef >= (i16::wrapping_shl(1, 13_u32)).wrapping_neg()
                                  );
                                  coef
                                }),
                                ::core::option::Option::None => [i16::MIN, 0, 0, 0],
                              },
                            );
                          }
                          ::core::option::Option::None => {}
                        }
                      }
                      ::core::option::Option::None => {}
                    }

                    match debug_block_info!(f, t.b) {
                      BlockTrace::Enabled => {
                        println!(
                          "Post-motionmode[{:?}]: r={} [mask: 0x{:x}/0x{:x}]",
                          motion_mode,
                          ts_c_2.msac.rng,
                          *match <[_]>::get(&mask, 0) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None =>
                              return ::core::result::Result::Err(()),
                          },
                          *match <[_]>::get(&mask, 1) {
                            ::core::option::Option::Some(indexed_value) => indexed_value,
                            ::core::option::Option::None =>
                              return ::core::result::Result::Err(()),
                          },
                        );
                      }
                      BlockTrace::Disabled => {}
                    }
                  }
                  ::core::option::Option::None => {
                    motion_mode = MotionMode::Translation;
                  }
                }

                let matrix_2 = matrix;

                Inter {
                  nd: match matrix_2 {
                    None => <crate::src::levels::Av1BlockInterNd as ::core::convert::From<
                      crate::src::levels::Av1BlockInter1d,
                    >>::from(Av1BlockInter1d {
                      mv: [mv1d0_2, Default::default()],
                      wedge_idx: wedge_idx_3,
                      interintra_mode:
                        <crate::src::levels::MaskedInterIntraPredMode as ::core::convert::From<
                          crate::src::levels::InterIntraPredMode,
                        >>::from(interintra_mode),
                      ..Default::default()
                    }),
                    Some(matrix_3) => {
                      <crate::src::levels::Av1BlockInterNd as ::core::convert::From<
                        crate::src::levels::Av1BlockInter2d,
                      >>::from(Av1BlockInter2d {
                        mv2d: mv1d0_2,
                        matrix: matrix_3,
                      })
                    }
                  },
                  comp_type: None,
                  inter_mode,
                  motion_mode,
                  drl_idx: drl_idx_3,
                  r#ref,
                  interintra_type,
                }
              }
            },
          };

          let filter = match ::core::primitive::bool::then_some(
            frame_hdr.subpel_filter_mode == Rav1dFilterMode::Switchable,
            (),
          ) {
            ::core::option::Option::Some(()) => {
              match ::core::primitive::bool::then_some(has_subpel_filter, ()) {
                ::core::option::Option::Some(()) => {
                  let references = match comp_type {
                    Some(_) => ReferenceArity::Compound,
                    None => ReferenceArity::Single,
                  };
                  let [filter_reference, _other_reference] = r#ref;
                  let ctx1 = get_filter_ctx(
                    BlockNeighbors {
                      above: ta,
                      left: &t.l,
                      location: Bxy { x: bx4, y: by4 },
                      availability: neighbors,
                    },
                    references,
                    FilterAxis::Vertical,
                    filter_reference,
                  )
                  .map_err(|_invalid_context| ())?;
                  let filter0 = Rav1dFilterMode::from_repr({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index({
                      let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt4(
                        &mut ts_c_2.msac,
                        &mut *match <[_]>::get_mut(
                          &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.filter, 0) {
                            ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                            ::core::option::Option::None => {
                              return ::core::result::Result::Err(());
                            }
                          },
                          {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_index(ctx1);
                            numeric_value
                          },
                        ) {
                          ::core::option::Option::Some(indexed_value) => indexed_value,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        },
                        u8::wrapping_sub(u8::from(Rav1dFilterMode::N_SWITCHABLE_FILTERS), 1),
                      );
                      explicit_value
                    });
                    numeric_value
                  })
                  .ok_or(())?;

                  match ::core::primitive::bool::then_some(seq_hdr.dual_filter != 0, ()) {
                    ::core::option::Option::Some(()) => {
                      let ctx2 = get_filter_ctx(
                        BlockNeighbors {
                          above: ta,
                          left: &t.l,
                          location: Bxy { x: bx4, y: by4 },
                          availability: neighbors,
                        },
                        references,
                        FilterAxis::Horizontal,
                        filter_reference,
                      )
                      .map_err(|_invalid_context| ())?;

                      match debug_block_info!(f, t.b) {
                        BlockTrace::Enabled => {
                          println!(
                            "Post-subpel_filter1[{:?},ctx={}]: r={}",
                            filter0, ctx1, ts_c_2.msac.rng,
                          );
                        }
                        BlockTrace::Disabled => {}
                      }

                      let filter1 = Rav1dFilterMode::from_repr({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped_index({
                          let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt4(
                            &mut ts_c_2.msac,
                            &mut *match <[_]>::get_mut(
                              &mut *match <[_]>::get_mut(&mut *ts_c_2.cdf.mi.filter, 1) {
                                ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                                ::core::option::Option::None => {
                                  return ::core::result::Result::Err(());
                                }
                              },
                              {
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped_index(ctx2);
                                numeric_value
                              },
                            ) {
                              ::core::option::Option::Some(indexed_value) => indexed_value,
                              ::core::option::Option::None => {
                                return ::core::result::Result::Err(());
                              }
                            },
                            u8::wrapping_sub(u8::from(Rav1dFilterMode::N_SWITCHABLE_FILTERS), 1),
                          );
                          explicit_value
                        });
                        numeric_value
                      })
                      .ok_or(())?;

                      match debug_block_info!(f, t.b) {
                        BlockTrace::Enabled => {
                          println!(
                            "Post-subpel_filter2[{:?},ctx={}]: r={}",
                            filter1, ctx2, ts_c_2.msac.rng,
                          );
                        }
                        BlockTrace::Disabled => {}
                      }

                      [filter0, filter1]
                    }
                    ::core::option::Option::None => {
                      match debug_block_info!(f, t.b) {
                        BlockTrace::Enabled => {
                          println!(
                            "Post-subpel_filter[{:?},ctx={}]: r={}",
                            filter0, ctx1, ts_c_2.msac.rng
                          );
                        }
                        BlockTrace::Disabled => {}
                      }

                      [filter0; 2]
                    }
                  }
                }
                ::core::option::Option::None => [Rav1dFilterMode::Regular8Tap; 2],
              }
            }
            ::core::option::Option::None => [frame_hdr.subpel_filter_mode; 2],
          };
          let [horizontal, vertical] = filter;
          let Ok(vertical_index) = console_core_number_conversion::index(u8::from(vertical));
          let Ok(horizontal_index) = console_core_number_conversion::index(u8::from(horizontal));
          let direction = dav1d_filter_2d.get(vertical_index).ok_or(())?;
          let &filter2d = direction.get(horizontal_index).ok_or(())?;

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
              local: Bxy { x: bx4, y: by4 },
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

          match ::core::primitive::bool::then_some(t.frame_thread.pass == 1, ()) {
            ::core::option::Option::Some(()) => {
              (bd_fn.read_coef_blocks)(f, t, ts_c_2, bs, b).map_err(|_| ())?;
            }
            ::core::option::Option::None => {
              (bd_fn.recon_b_inter)(f, t, Some(ts_c_2), bs, b, &inter)?;
            }
          }

          let frame_hdr_4 = f.frame_hdr().map_err(|_missing_header| ())?;

          match ::core::primitive::bool::then_some(frame_hdr_4.loopfilter.level_y != [0, 0], ()) {
            ::core::option::Option::Some(()) => {
              let is_globalmv = i32::from(
                inter_mode
                  == match ::core::primitive::bool::then_some(is_comp, ()) {
                    ::core::option::Option::Some(()) => GLOBALMV_GLOBALMV,
                    ::core::option::Option::None => GLOBALMV,
                  },
              );
              let tx_split = [
                ({
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u16>(tx_split0);
                  numeric_value
                }),
                tx_split1,
              ];
              let mut ytx = max_ytx;
              let mut uvtx_2 = b.uvtx;

              let Ok(b_seg_id) = b.seg_id.get();

              match ::core::primitive::bool::then_some(
                *match <[_]>::get(
                  &frame_hdr_4.segmentation.lossless,
                  match ::console_core_number_conversion::index(b_seg_id) {
                    ::core::result::Result::Ok(native_position__) => native_position__,
                  },
                ) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                },
                (),
              ) {
                ::core::option::Option::Some(()) => {
                  ytx = TxfmSize::S4x4;
                  uvtx_2 = TxfmSize::S4x4;
                }
                ::core::option::Option::None => {}
              }

              let Ok(get) = ts.lflvl.get();
              let lflvl = match get {
                TileStateRef::Frame => &f.lf.lvl,
                TileStateRef::Local => &*ts.lflvlmem.try_read().ok_or(())?,
              };
              let mut a_uv_guard;
              let l_uv_slice;

              match create_lf_mask(
                FilterMaskBlock {
                  mask: {
                    let checked_value_6 =
                      f.loop_filter_mask(t.lf_mask).map_err(|_invalid_mask| ())?;
                    checked_value_6
                  },
                  level_cache: &f.lf.level,
                  stride: i64::from(f.b4_stride),
                  levels: {
                    let Ok(segment) = b.seg_id.get();
                    lflvl
                      .get(match ::console_core_number_conversion::index(segment) {
                        ::core::result::Result::Ok(native_position___) => native_position___,
                      })
                      .ok_or(())?
                  },
                  origin: t.b,
                  frame: console_core_geometry::Size {
                    width: f.w4,
                    height: f.h4,
                  },
                  size: bs,
                  ytx: ytx,
                  uvtx: uvtx_2,
                  layout: f.cur.p.layout,
                  luma: EdgeNeighbors {
                    above: &mut ta.tx_lpf_y.index_mut((
                      ({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(bx4);
                        numeric_value
                      })..,
                      ..({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(bw4);
                        numeric_value
                      }),
                    )),
                    left: &mut *match <[_]>::get_mut(
                      &mut *match <[_]>::get_mut(
                        &mut **t.l.tx_lpf_y.get_mut(),
                        ({
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(by4);
                          numeric_value
                        })..,
                      ) {
                        ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                      ..({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_index(bh4);
                        numeric_value
                      }),
                    ) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                  },
                  chroma: match ::core::primitive::bool::then_some(has_chroma, ()) {
                    ::core::option::Option::Some(()) => {
                      a_uv_guard = ta.tx_lpf_uv.index_mut((
                        ({
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(cbx4);
                          numeric_value
                        })..,
                        ..({
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(cbw4);
                          numeric_value
                        }),
                      ));
                      l_uv_slice = &mut *match <[_]>::get_mut(
                        &mut *match <[_]>::get_mut(
                          &mut **t.l.tx_lpf_uv.get_mut(),
                          ({
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_index(cby4);
                            numeric_value
                          })..,
                        ) {
                          ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        },
                        ..({
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(cbh4);
                          numeric_value
                        }),
                      ) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      };
                      Some((&mut *a_uv_guard, l_uv_slice))
                    }
                    ::core::option::Option::None => None,
                  },
                },
                FilterMaskPrediction::Inter {
                  reference: match r#ref {
                    [first, ..] => first.wrapping_add(1),
                  },
                  motion: match is_globalmv {
                    0 => FilterMaskMotion::Other,
                    _ => FilterMaskMotion::Global,
                  },
                  scan: match b.skip {
                    0 => EdgeScan::AllTransforms,
                    _ => EdgeScan::Boundary,
                  },
                  split_masks: &tx_split,
                },
              ) {
                Ok(()) => {}
                Err(error) => {
                  console_core_logging::debug!(
                    "Could not create an AV1 inter filter mask: {error:?}"
                  );
                  return Err(());
                }
              }
            }

            ::core::option::Option::None => {}
          }

          match ::core::primitive::bool::then_some(is_comp, ()) {
            ::core::option::Option::Some(()) => {
              splat_tworef_mv(
                c,
                t,
                &f.rf,
                bs,
                &inter,
                Size {
                  width: bw4,
                  height: bh4,
                },
              )
              .map_err(|_| ())?;
            }
            ::core::option::Option::None => {
              splat_oneref_mv(
                c,
                t,
                &f.rf,
                bs,
                &inter,
                Size {
                  width: bw4,
                  height: bh4,
                },
              )
              .map_err(|_| ())?;
            }
          }

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
        }
      }
    }
  }

  let frame_header = f.frame_hdr.as_ref().ok_or(())?;

  match (frame_header.segmentation.enabled != 0 && frame_header.segmentation.update_map != 0)
    .then_some(())
  {
    Some(()) => {
      let block = Rectangle {
        origin: Point { x: t.b.x, y: t.b.y },
        size: Size {
          width: bw4,
          height: bh4,
        },
      };
      write_block_segments(f, block, b.seg_id).map_err(|_invalid_segment_map| ())?;
    }
    None => {}
  }

  match ::core::primitive::bool::then_some(b.skip == 0, ()) {
    ::core::option::Option::Some(()) => {
      let mask = u32::wrapping_shl(
        u32::wrapping_shr(!0u32, {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_sub(32, bw4));
          numeric_value
        }),
        {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(bx4 & 15);
          numeric_value
        },
      );
      let bx_idx = i32::wrapping_shr(bx4 & 16, 4_u32);

      for noskip_mask in &*match <[_]>::get(
        &(*match <[_]>::get(
          &f.loop_filter_mask(t.lf_mask)
            .map_err(|_invalid_mask| ())?
            .noskip_mask,
          ({
            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by4);
            numeric_value
          })
          .wrapping_shr(1_u32)..,
        ) {
          ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
          ::core::option::Option::None => return ::core::result::Result::Err(()),
        }),
        ..({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bh4);
          numeric_value
        })
        .div_ceil(2),
      ) {
        ::core::option::Option::Some(indexed_value_3) => indexed_value_3,
        ::core::option::Option::None => return ::core::result::Result::Err(()),
      } {
        let Ok(()) = (*match <[_]>::get(&*noskip_mask, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx_idx);
          numeric_value
        }) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => return ::core::result::Result::Err(()),
        })
        .update(|it| {
          it | ({
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(mask);
            numeric_value
          })
        });

        match ::core::primitive::bool::then_some(bw4 == 32, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(()) = (*match <[_]>::get(&*noskip_mask, 1) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            })
            .update(|it| {
              it | ({
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(mask);
                numeric_value
              })
            });
          }
          ::core::option::Option::None => {}
        }
      }
    }
    ::core::option::Option::None => {}
  }

  match &b.ii {
    Av1BlockIntraInter::Inter(inter)
      if t.frame_thread.pass == 1 && frame_header.frame_type.is_inter_or_switch() =>
    {
      let sby = i32::wrapping_shr(i32::wrapping_sub(t.b.y, ts.tiling.row_start), {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(f.sb_shift);
        numeric_value
      });
      let position = f
        .lowest_pixel_position(ts, sby)
        .map_err(|_outside_tile| ())?;
      let Ok(address) = console_core_number_conversion::index(position);
      let mut lowest_px = f.lowest_pixel_mem.index_mut(address);

      match ::core::primitive::bool::then_some(inter.comp_type.is_none(), ()) {
        ::core::option::Option::Some(()) => {
          match ::core::primitive::bool::then_some(
            cmp::min(bw4, bh4) > 1
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
                  && t.warpmv.r#type > Rav1dWarpedMotionType::Translation),
            (),
          ) {
            ::core::option::Option::Some(()) => {
              let Ok(()) = affine_lowest_px_luma(
                t,
                &mut *match <[_]>::get_mut(
                  &mut *match <[_]>::get_mut(&mut *lowest_px, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped_index(match inter.r#ref {
                        [element, ..] => element,
                      });
                    numeric_value
                  }) {
                    ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                  0,
                ) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                },
                b_dim,
                match ::core::primitive::bool::then_some(
                  inter.motion_mode == MotionMode::Warp,
                  (),
                ) {
                  ::core::option::Option::Some(()) => &t.warpmv,
                  ::core::option::Option::None => &*match <[_]>::get(&frame_header.gmv, {
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
              );
            }
            ::core::option::Option::None => {
              let Ok(()) = mc_lowest_px(
                &mut *match <[_]>::get_mut(
                  &mut *match <[_]>::get_mut(&mut *lowest_px, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped_index(match inter.r#ref {
                        [element, ..] => element,
                      });
                    numeric_value
                  }) {
                    ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                  0,
                ) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                },
                t.b.y,
                bh4,
                (*match <[_]>::get(&inter.nd.one_d.mv, 0) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                })
                .y,
                0,
                &*match <[_]>::get(
                  &(*match <[_]>::get(&f.svc, {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped_index(match inter.r#ref {
                        [element, ..] => element,
                      });
                    numeric_value
                  }) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  }),
                  1,
                ) {
                  ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                },
              );

              match ::core::primitive::bool::then_some(inter.motion_mode == MotionMode::Obmc, ())
              {
                ::core::option::Option::Some(()) => {
                  obmc_lowest_px(
                    f,
                    t,
                    &mut lowest_px,
                    OverlapBlock {
                      visible: Size {
                        width: w4,
                        height: h4,
                      },
                      dimensions: *b_dim,
                      plane: OverlapPlane::Luma,
                    },
                  )
                  .map_err(|_invalid_overlap| ())?;
                }
                ::core::option::Option::None => {}
              }
            }
          }

          match ::core::primitive::bool::then_some(has_chroma, ()) {
            ::core::option::Option::Some(()) => {
              let extent = Size {
                width: bw4,
                height: bh4,
              };
              match f
                .chroma_motion_neighbors(t, extent)
                .map_err(|_invalid_neighbor| ())?
              {
                Some(neighbors) => {
                  let update_lowest =
                    |lowest: &mut [[c_int; 2]; 7], prediction: ChromaNeighborPrediction| {
                      let Ok(reference) =
                        console_core_number_conversion::index(prediction.reference);
                      let [_, chroma] = lowest.get_mut(reference).ok_or(())?;
                      let [_, scale] = f.svc.get(reference).ok_or(())?;
                      let Ok(()) = mc_lowest_px(
                        chroma,
                        prediction.position.y,
                        bh4,
                        prediction.vector.y,
                        ss_ver,
                        scale,
                      );
                      Ok(())
                    };

                  for neighbor in [neighbors.diagonal, neighbors.left, neighbors.above]
                    .into_iter()
                    .flatten()
                  {
                    update_lowest(&mut lowest_px, neighbor)?;
                  }
                  let [reference, _] = inter.r#ref;
                  let [vector, _] = inter.nd.one_d.mv;
                  let reference = u8::try_from(reference).map_err(|_invalid_reference| ())?;
                  update_lowest(
                    &mut lowest_px,
                    ChromaNeighborPrediction {
                      position: t.b,
                      vector,
                      reference,
                    },
                  )?;
                }
                ::core::option::Option::None => match ::core::primitive::bool::then_some(
                  cmp::min(cbw4, cbh4) > 1
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
                        && t.warpmv.r#type > Rav1dWarpedMotionType::Translation),
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    let Ok(()) = affine_lowest_px_chroma(
                      t,
                      f.cur.p.layout,
                      &mut *match <[_]>::get_mut(
                        &mut *match <[_]>::get_mut(&mut *lowest_px, {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(match inter.r#ref {
                              [element, ..] => element,
                            });
                          numeric_value
                        }) {
                          ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        },
                        1,
                      ) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                      b_dim,
                      match ::core::primitive::bool::then_some(
                        inter.motion_mode == MotionMode::Warp,
                        (),
                      ) {
                        ::core::option::Option::Some(()) => &t.warpmv,
                        ::core::option::Option::None => &*match <[_]>::get(&frame_header.gmv, {
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
                    );
                  }
                  ::core::option::Option::None => {
                    let Ok(()) = mc_lowest_px(
                      &mut *match <[_]>::get_mut(
                        &mut *match <[_]>::get_mut(&mut *lowest_px, {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(match inter.r#ref {
                              [element, ..] => element,
                            });
                          numeric_value
                        }) {
                          ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        },
                        1,
                      ) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                      t.b.y & !ss_ver,
                      i32::wrapping_shl(bh4, {
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                          i32::from(bh4 == ss_ver),
                        );
                        numeric_value
                      }),
                      (*match <[_]>::get(&inter.nd.one_d.mv, 0) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      })
                      .y,
                      ss_ver,
                      &*match <[_]>::get(
                        &(*match <[_]>::get(&f.svc, {
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_index(match inter.r#ref {
                              [element, ..] => element,
                            });
                          numeric_value
                        }) {
                          ::core::option::Option::Some(indexed_value) => indexed_value,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        }),
                        1,
                      ) {
                        ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                    );

                    match ::core::primitive::bool::then_some(
                      inter.motion_mode == MotionMode::Obmc,
                      (),
                    ) {
                      ::core::option::Option::Some(()) => {
                        obmc_lowest_px(
                          f,
                          t,
                          &mut lowest_px,
                          OverlapBlock {
                            visible: Size {
                              width: w4,
                              height: h4,
                            },
                            dimensions: *b_dim,
                            plane: OverlapPlane::Chroma,
                          },
                        )
                        .map_err(|_invalid_overlap| ())?;
                      }
                      ::core::option::Option::None => {}
                    }
                  }
                },
              }
            }
            ::core::option::Option::None => {}
          }
        }
        ::core::option::Option::None => {
          let refmvs = || {
            std::iter::zip(inter.r#ref, inter.nd.one_d.mv).map(|(r#ref, mv)| {
              (
                ({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(r#ref);
                  numeric_value
                }),
                mv,
              )
            })
          };

          for (r#ref, mv) in refmvs() {
            match ::core::primitive::bool::then_some(
              inter.inter_mode == GLOBALMV_GLOBALMV
                && *match <[_]>::get(&f.gmv_warp_allowed, r#ref) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                } != 0,
              (),
            ) {
              ::core::option::Option::Some(()) => {
                let Ok(()) = affine_lowest_px_luma(
                  t,
                  &mut *match <[_]>::get_mut(
                    &mut *match <[_]>::get_mut(&mut *lowest_px, r#ref) {
                      ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                    0,
                  ) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                  b_dim,
                  &*match <[_]>::get(&frame_header.gmv, r#ref) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                );
              }
              ::core::option::Option::None => {
                let Ok(()) = mc_lowest_px(
                  &mut *match <[_]>::get_mut(
                    &mut *match <[_]>::get_mut(&mut *lowest_px, r#ref) {
                      ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                    0,
                  ) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                  t.b.y,
                  bh4,
                  mv.y,
                  0,
                  &*match <[_]>::get(
                    &(*match <[_]>::get(&f.svc, r#ref) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    }),
                    1,
                  ) {
                    ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                );
              }
            }
          }

          for (r#ref, mv) in refmvs() {
            match ::core::primitive::bool::then_some(
              inter.inter_mode == GLOBALMV_GLOBALMV
                && *match <[_]>::get(&f.gmv_warp_allowed, r#ref) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                } != 0,
              (),
            ) {
              ::core::option::Option::Some(()) => {
                let Ok(()) = affine_lowest_px_luma(
                  t,
                  &mut *match <[_]>::get_mut(
                    &mut *match <[_]>::get_mut(&mut *lowest_px, r#ref) {
                      ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                    0,
                  ) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                  b_dim,
                  &*match <[_]>::get(&frame_header.gmv, r#ref) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                );
              }
              ::core::option::Option::None => {
                let Ok(()) = mc_lowest_px(
                  &mut *match <[_]>::get_mut(
                    &mut *match <[_]>::get_mut(&mut *lowest_px, r#ref) {
                      ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    },
                    0,
                  ) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                  t.b.y,
                  bh4,
                  mv.y,
                  0,
                  &*match <[_]>::get(
                    &(*match <[_]>::get(&f.svc, r#ref) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    }),
                    1,
                  ) {
                    ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  },
                );
              }
            }
          }

          match ::core::primitive::bool::then_some(has_chroma, ()) {
            ::core::option::Option::Some(()) => {
              for (r#ref, mv) in refmvs() {
                match ::core::primitive::bool::then_some(
                  inter.inter_mode == GLOBALMV_GLOBALMV
                    && cmp::min(cbw4, cbh4) > 1
                    && *match <[_]>::get(&f.gmv_warp_allowed, r#ref) {
                      ::core::option::Option::Some(indexed_value) => indexed_value,
                      ::core::option::Option::None => return ::core::result::Result::Err(()),
                    } != 0,
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    let Ok(()) = affine_lowest_px_chroma(
                      t,
                      f.cur.p.layout,
                      &mut *match <[_]>::get_mut(
                        &mut *match <[_]>::get_mut(&mut *lowest_px, r#ref) {
                          ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        },
                        1,
                      ) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                      b_dim,
                      &*match <[_]>::get(&frame_header.gmv, r#ref) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                    );
                  }
                  ::core::option::Option::None => {
                    let Ok(()) = mc_lowest_px(
                      &mut *match <[_]>::get_mut(
                        &mut *match <[_]>::get_mut(&mut *lowest_px, r#ref) {
                          ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        },
                        1,
                      ) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                      t.b.y,
                      bh4,
                      mv.y,
                      ss_ver,
                      &*match <[_]>::get(
                        &(*match <[_]>::get(&f.svc, r#ref) {
                          ::core::option::Option::Some(indexed_value) => indexed_value,
                          ::core::option::Option::None => return ::core::result::Result::Err(()),
                        }),
                        1,
                      ) {
                        ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                        ::core::option::Option::None => return ::core::result::Result::Err(()),
                      },
                    );
                  }
                }
              }
            }
            ::core::option::Option::None => {}
          }
        }
      }
    }
    Av1BlockIntraInter::Intra(_) | Av1BlockIntraInter::Inter(_) => {}
  }

  Ok(())
}

enum FrameThreadPassState<'a> {
  First(&'a mut Rav1dTileStateContext),
  Second,
}

fn decode_sb(
  c: &Rav1dContext,
  t: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
  pass: &mut FrameThreadPassState,
  bl: BlockLevel,
  edge_index: EdgeIndex,
) -> Result<(), ()> {
  let ts = &*match f.tile(t.tile_index) {
    ::core::option::Option::Some(indexed_value) => indexed_value,
    ::core::option::Option::None => return ::core::result::Result::Err(()),
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
      return decode_sb(
        c,
        t,
        f,
        pass,
        next_bl,
        match intra_edge_branch.split {
          [_explicit_element, ..] => _explicit_element,
        },
      );
    }
    ::core::option::Option::None => {}
  }

  let frame_hdr = &***f.frame_hdr.as_ref().ok_or(())?;

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
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => return ::core::result::Result::Err(()),
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
      match ctx {
        Some((ctx_2, ts_c)) => {
          let pc = &mut *match <[_]>::get_mut(
            &mut *match <[_]>::get_mut(&mut *ts_c.cdf.m.partition, {
              let Ok(table_index) = console_core_number_conversion::index({
                let Ok(ordinal) = bl.ordinal();
                ordinal
              });
              table_index
            }) {
              ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            },
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(ctx_2);
              numeric_value
            },
          ) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => return ::core::result::Result::Err(()),
          };
          bp = BlockPartition::from_repr({
            let Ok(numeric_value) = console_core_number_conversion::wrapped_index({
              let Ok(explicit_value) = rav1d_msac_decode_symbol_adapt16(
                &mut ts_c.msac,
                pc,
                *match <[_]>::get(&dav1d_partition_type_count, {
                  let Ok(table_index) = console_core_number_conversion::index({
                    let Ok(ordinal) = bl.ordinal();
                    ordinal
                  });
                  table_index
                }) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                },
              );
              explicit_value
            });
            numeric_value
          })
          .ok_or(())?;

          match ::core::primitive::bool::then_some(
            f.cur.p.layout == Rav1dPixelLayout::I422
              && matches!(
                bp,
                BlockPartition::V
                  | BlockPartition::V4
                  | BlockPartition::LeftSplit
                  | BlockPartition::RightSplit
              ),
            (),
          ) {
            ::core::option::Option::Some(()) => {
              return Err(());
            }
            ::core::option::Option::None => {}
          }

          match debug_block_info!(f, t.b) {
            BlockTrace::Enabled => {
              println!(
                "poc={},y={},x={},bl={:?},ctx={},bp={:?}: r={}",
                frame_hdr.frame_offset, t.b.y, t.b.x, bl, ctx_2, bp, ts_c.msac.rng,
              );
            }
            BlockTrace::Disabled => {}
          }
        }
        ::core::option::Option::None => {
          let block = f.coded_block_position(t.b).map_err(|_invalid_block| ())?;
          let Ok(index) = console_core_number_conversion::index(block);
          let b = f.frame_thread.b.index(index);
          bp = match ::core::primitive::bool::then_some(b.bl == bl, ()) {
            ::core::option::Option::Some(()) => b.bp,
            ::core::option::Option::None => BlockPartition::Split,
          };
        }
      }

      let b = &*match <[_]>::get(
        &(*match <[_]>::get(&dav1d_block_sizes, {
          let Ok(table_index) = console_core_number_conversion::index({
            let Ok(ordinal) = bl.ordinal();
            ordinal
          });
          table_index
        }) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => return ::core::result::Result::Err(()),
        }),
        {
          let Ok(table_index) = console_core_number_conversion::index({
            let Ok(ordinal) = bp.ordinal();
            ordinal
          });
          table_index
        },
      ) {
        ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
        ::core::option::Option::None => return ::core::result::Result::Err(()),
      };

      match bp {
        BlockPartition::None => {
          let node = intra_edge
            .node(superblock_size, edge_index)
            .map_err(|_edge| ())?;
          decode_b(
            c,
            t,
            f,
            pass,
            bl,
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
            bp,
            node.o,
          )?;
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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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

              match ::core::primitive::bool::then_some(
                cfg!(target_arch = "x86_64") && t.frame_thread.pass != 0,
                (),
              ) {
                ::core::option::Option::Some(()) => {
                  let Ok(p) =
                    console_core_number_conversion::wrapped_index(t.frame_thread.pass & 1);
                  let Ok(()) = (*match <[_]>::get(&ts.frame_thread, p) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  })
                  .cf
                  .update(|cf| u32::wrapping_add(cf, 31) & !31);
                }
                ::core::option::Option::None => {}
              }
            }
            Some(next_bl) => {
              let branch = intra_edge
                .branch(superblock_size, edge_index)
                .map_err(|_edge| ())?;
              decode_sb(
                c,
                t,
                f,
                pass,
                next_bl,
                match branch.split {
                  [_explicit_element, ..] => _explicit_element,
                },
              )?;

              {
                let numeric_amount = hsz;
                let numeric_target = &mut (t.b.x);
                *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
              };

              decode_sb(
                c,
                t,
                f,
                pass,
                next_bl,
                match branch.split {
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

              decode_sb(
                c,
                t,
                f,
                pass,
                next_bl,
                match branch.split {
                  [_, _, _explicit_element, ..] => _explicit_element,
                },
              )?;

              {
                let numeric_amount = hsz;
                let numeric_target = &mut (t.b.x);
                *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
              };

              decode_sb(
                c,
                t,
                f,
                pass,
                next_bl,
                match branch.split {
                  [_, _, _, _explicit_element, ..] => _explicit_element,
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
                *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
              };
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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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
            match b {
              &[_, _explicit_element, ..] => _explicit_element,
            },
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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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
            match b {
              &[_, _explicit_element, ..] => _explicit_element,
            },
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
            match b {
              &[_, _explicit_element, ..] => _explicit_element,
            },
            bp,
            EdgeFlags::EMPTY,
          )?;

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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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
            match b {
              &[_, _explicit_element, ..] => _explicit_element,
            },
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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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
            match b {
              &[_, _explicit_element, ..] => _explicit_element,
            },
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
            match b {
              &[_, _explicit_element, ..] => _explicit_element,
            },
            bp,
            EdgeFlags::EMPTY,
          )?;

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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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

          decode_b(
            c,
            t,
            f,
            pass,
            bl,
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
            bp,
            branch.h4,
          )?;

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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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
                match b {
                  &[_explicit_element, ..] => _explicit_element,
                },
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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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

          decode_b(
            c,
            t,
            f,
            pass,
            bl,
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
            bp,
            branch.v4,
          )?;

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
            match b {
              &[_explicit_element, ..] => _explicit_element,
            },
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
                match b {
                  &[_explicit_element, ..] => _explicit_element,
                },
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
    }
    ::core::option::Option::None => match ::core::primitive::bool::then_some(have_h_split, ()) {
      ::core::option::Option::Some(()) => {
        let is_split;

        match ctx {
          Some((ctx_3, ts_c)) => {
            let pc = &mut *match <[_]>::get_mut(
              &mut *match <[_]>::get_mut(&mut *ts_c.cdf.m.partition, {
                let Ok(table_index) = console_core_number_conversion::index({
                  let Ok(ordinal) = bl.ordinal();
                  ordinal
                });
                table_index
              }) {
                ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              },
              {
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(ctx_3);
                numeric_value
              },
            ) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            };
            is_split = matches!(
              rav1d_msac_decode_bool(&mut ts_c.msac, {
                let Ok(explicit_value) = gather_partition_probability(pc, bl, PartitionEdge::Top);
                explicit_value
              }),
              Ok(EntropyBit::One)
            );

            match debug_block_info!(f, t.b) {
              BlockTrace::Enabled => {
                println!(
                  "poc={},y={},x={},bl={:?},ctx={},bp={:?}: r={}",
                  frame_hdr.frame_offset,
                  t.b.y,
                  t.b.x,
                  bl,
                  ctx_3,
                  match is_split {
                    true => {
                      BlockPartition::Split
                    }
                    false => {
                      BlockPartition::H
                    }
                  },
                  ts_c.msac.rng,
                );
              }
              BlockTrace::Disabled => {}
            }
          }
          ::core::option::Option::None => {
            let block = f.coded_block_position(t.b).map_err(|_invalid_block| ())?;
            let Ok(index) = console_core_number_conversion::index(block);
            let b = &f.frame_thread.b.index(index);
            is_split = b.bl != bl;
          }
        }

        let Ok(bl_decrease) = bl.decrease();
        let next_bl = bl_decrease.ok_or(())?;

        match ::core::primitive::bool::then_some(is_split, ()) {
          ::core::option::Option::Some(()) => {
            let branch = intra_edge
              .branch(superblock_size, edge_index)
              .map_err(|_edge| ())?;
            bp = BlockPartition::Split;
            decode_sb(
              c,
              t,
              f,
              pass,
              next_bl,
              match branch.split {
                [_explicit_element, ..] => _explicit_element,
              },
            )?;

            {
              let numeric_amount = hsz;
              let numeric_target = &mut (t.b.x);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };

            decode_sb(
              c,
              t,
              f,
              pass,
              next_bl,
              match branch.split {
                [_, _explicit_element, ..] => _explicit_element,
              },
            )?;

            {
              let numeric_amount = hsz;
              let numeric_target = &mut (t.b.x);
              *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
            };
          }
          ::core::option::Option::None => {
            let node = intra_edge
              .node(superblock_size, edge_index)
              .map_err(|_edge| ())?;
            bp = BlockPartition::H;
            decode_b(
              c,
              t,
              f,
              pass,
              bl,
              *match <[_]>::get(
                &(*match <[_]>::get(
                  &(*match <[_]>::get(&dav1d_block_sizes, {
                    let Ok(table_index) = console_core_number_conversion::index({
                      let Ok(ordinal) = bl.ordinal();
                      ordinal
                    });
                    table_index
                  }) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  }),
                  {
                    let Ok(table_index) = console_core_number_conversion::index({
                      let Ok(ordinal) = bp.ordinal();
                      ordinal
                    });
                    table_index
                  },
                ) {
                  ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                }),
                0,
              ) {
                ::core::option::Option::Some(indexed_value_3) => indexed_value_3,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              },
              bp,
              match node.h {
                [_explicit_element, ..] => _explicit_element,
              },
            )?;
          }
        }
      }
      ::core::option::Option::None => {
        assert!(have_v_split);
        let is_split;

        match ctx {
          Some((ctx_4, ts_c)) => {
            let pc = &mut *match <[_]>::get_mut(
              &mut *match <[_]>::get_mut(&mut *ts_c.cdf.m.partition, {
                let Ok(table_index) = console_core_number_conversion::index({
                  let Ok(ordinal) = bl.ordinal();
                  ordinal
                });
                table_index
              }) {
                ::core::option::Option::Some(indexed_value_4) => indexed_value_4,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              },
              {
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(ctx_4);
                numeric_value
              },
            ) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            };
            is_split = matches!(
              rav1d_msac_decode_bool(&mut ts_c.msac, {
                let Ok(explicit_value) =
                  gather_partition_probability(pc, bl, PartitionEdge::Left);
                explicit_value
              }),
              Ok(EntropyBit::One)
            );

            match ::core::primitive::bool::then_some(
              f.cur.p.layout == Rav1dPixelLayout::I422 && !is_split,
              (),
            ) {
              ::core::option::Option::Some(()) => {
                return Err(());
              }
              ::core::option::Option::None => {}
            }

            match debug_block_info!(f, t.b) {
              BlockTrace::Enabled => {
                println!(
                  "poc={},y={},x={},bl={:?},ctx={},bp={:?}: r={}",
                  frame_hdr.frame_offset,
                  t.b.y,
                  t.b.x,
                  bl,
                  ctx_4,
                  match is_split {
                    true => {
                      BlockPartition::Split
                    }
                    false => {
                      BlockPartition::V
                    }
                  },
                  ts_c.msac.rng,
                );
              }
              BlockTrace::Disabled => {}
            }
          }
          ::core::option::Option::None => {
            let block = f.coded_block_position(t.b).map_err(|_invalid_block| ())?;
            let Ok(index) = console_core_number_conversion::index(block);
            let b = &f.frame_thread.b.index(index);
            is_split = b.bl != bl;
          }
        }

        let Ok(bl_decrease) = bl.decrease();
        let next_bl = bl_decrease.ok_or(())?;

        match ::core::primitive::bool::then_some(is_split, ()) {
          ::core::option::Option::Some(()) => {
            let branch = intra_edge
              .branch(superblock_size, edge_index)
              .map_err(|_edge| ())?;
            bp = BlockPartition::Split;
            decode_sb(
              c,
              t,
              f,
              pass,
              next_bl,
              match branch.split {
                [_explicit_element, ..] => _explicit_element,
              },
            )?;

            {
              let numeric_amount = hsz;
              let numeric_target = &mut (t.b.y);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };

            decode_sb(
              c,
              t,
              f,
              pass,
              next_bl,
              match branch.split {
                [_, _, _explicit_element, ..] => _explicit_element,
              },
            )?;

            {
              let numeric_amount = hsz;
              let numeric_target = &mut (t.b.y);
              *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
            };
          }
          ::core::option::Option::None => {
            let node = intra_edge
              .node(superblock_size, edge_index)
              .map_err(|_edge| ())?;
            bp = BlockPartition::V;
            decode_b(
              c,
              t,
              f,
              pass,
              bl,
              *match <[_]>::get(
                &(*match <[_]>::get(
                  &(*match <[_]>::get(&dav1d_block_sizes, {
                    let Ok(table_index) = console_core_number_conversion::index({
                      let Ok(ordinal) = bl.ordinal();
                      ordinal
                    });
                    table_index
                  }) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => return ::core::result::Result::Err(()),
                  }),
                  {
                    let Ok(table_index) = console_core_number_conversion::index({
                      let Ok(ordinal) = bp.ordinal();
                      ordinal
                    });
                    table_index
                  },
                ) {
                  ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                }),
                0,
              ) {
                ::core::option::Option::Some(indexed_value_3) => indexed_value_3,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              },
              bp,
              match node.v {
                [_explicit_element, ..] => _explicit_element,
              },
            )?;
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
      let above = f.above_context(t.above_index).ok_or(())?;
      case_set_al! {
          <16, false>
          l: (&mut t.l, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(hsz); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(by8); numeric_value }),
          a: (above, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(hsz); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bx8); numeric_value }),
          partition = (
              (*match &dav1d_al_part_ctx { [_, element, ..] => element })[{ let Ok(table_index) = console_core_number_conversion::index({ let Ok(ordinal) = bl.ordinal(); ordinal }); table_index }][{ let Ok(table_index) = console_core_number_conversion::index({ let Ok(ordinal) = bp.ordinal(); ordinal }); table_index }],
              (*match &dav1d_al_part_ctx { [element, ..] => element })[{ let Ok(table_index) = console_core_number_conversion::index({ let Ok(ordinal) = bl.ordinal(); ordinal }); table_index }][{ let Ok(table_index) = console_core_number_conversion::index({ let Ok(ordinal) = bp.ordinal(); ordinal }); table_index }]
          ),
      }
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn reset_context(
  ctx: &mut BlockContext,
  keyframe: bool,
  pass: c_int,
) -> core::result::Result<(), console_core_never::Never> {
  {
    ctx
      .intra
      .get_mut()
      .fill(<::core::primitive::u8 as ::core::convert::From<
        ::core::primitive::bool,
      >>::from(keyframe));
    ctx.uvmode.get_mut().fill(DC_PRED);

    match ::core::primitive::bool::then_some(keyframe, ()) {
      ::core::option::Option::Some(()) => {
        ctx.mode.get_mut().fill(DC_PRED);
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(pass == 2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    ctx.partition.get_mut().fill(0);
    ctx.skip.get_mut().fill(0);
    ctx.skip_mode.get_mut().fill(0);
    ctx.tx_lpf_y.get_mut().fill(2);
    ctx.tx_lpf_uv.get_mut().fill(1);
    ctx.tx_intra.get_mut().fill(-1);
    ctx.tx.get_mut().fill(TxfmSize::S64x64);

    match ::core::primitive::bool::then_some(!keyframe, ()) {
      ::core::option::Option::Some(()) => {
        for r#ref in &mut ctx.r#ref {
          r#ref.get_mut().fill(-1);
        }

        ctx.comp_type.get_mut().fill(None);
        ctx.mode.get_mut().fill(NEARESTMV);
      }
      ::core::option::Option::None => {}
    }

    ctx.lcoef.get_mut().fill(0x40);

    for ccoef in &mut ctx.ccoef {
      ccoef.get_mut().fill(0x40);
    }

    for filter in &mut ctx.filter {
      filter.get_mut().fill(Rav1dFilterMode::N_SWITCHABLE_FILTERS);
    }

    ctx.seg_pred.get_mut().fill(0);
    ctx.pal_sz.get_mut().fill(0);
  };

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

fn setup_tile(
  c: &Rav1dContext,
  ts: &mut Rav1dTileState,
  seq_hdr: &Rav1dSequenceHeader,
  frame_hdr: &Rav1dFrameHeader,
  SetupTileArguments {
    bitdepth_max,
    sb_shift,
  }: SetupTileArguments,
  cur: &Rav1dPicture,
  bw: i32,
  bh: i32,
  frame_thread: &Rav1dFrameContextFrameThread,
  sr_sb128w: i32,
  sb128w: i32,
  lf: &mut Rav1dFrameContextLf,
  in_cdf: &CdfThreadContext,
  data: CArc<[u8]>,
  tile_row: u32,
  tile_col: u32,
  tile_start_off: u32,
) -> Rav1dResult {
  let initialized_cdf = rav1d_cdf_thread_copy(in_cdf)?;

  {
    let columns = adjacent_tile_boundaries(&frame_hdr.tiling.col_start_sb, tile_col)?;
    let rows = adjacent_tile_boundaries(&frame_hdr.tiling.row_start_sb, tile_row)?;
    let [col_sb_start, col_sb_end] = columns;
    let [row_sb_start, row_sb_end] = rows;
    let col_sb128_start = col_sb_start.wrapping_shr(u32::from(seq_hdr.sb128 == 0));
    let &[coefficient_scale, palette_scale] = ss_size_mul.checked(cur.p.layout)?;

    for pass in &ts.frame_thread {
      let palette_offset = match frame_thread.pal_idx.is_empty().then_some(()) {
        Some(()) => 0,
        None => tile_start_off
          .wrapping_mul(u32::from(palette_scale))
          .wrapping_div(8),
      };
      let coding_offset = tile_start_off.wrapping_mul(u32::from(coefficient_scale));
      let block_index = match frame_thread.cbi.is_empty().then_some(()) {
        Some(()) => 0,
        None => coding_offset.wrapping_div(64),
      };
      let coefficients = match frame_thread.cf.is_empty().then_some(()) {
        Some(()) => 0,
        None => {
          let bpc = BPC::from_bitdepth_max(bitdepth_max);
          bpc.coef_stride(coding_offset.wrapping_shr(u32::from(seq_hdr.hbd == 0)))
        }
      };
      let Ok(()) = pass.pal_idx.set(palette_offset);
      let Ok(()) = pass.cbi_idx.set(block_index);
      let Ok(()) = pass.cf.set(coefficients);
    }

    let mut tile_context = ts.context.try_lock().ok_or(EINVAL)?;
    let ts_c = &mut *tile_context;
    ts_c.cdf = initialized_cdf;
    ts.last_qidx = <crate::src::relaxed_atomic::RelaxedAtomic<::core::primitive::u8> as ::core::convert::From<::core::primitive::u8>>::from(frame_hdr.quant.yac);
    ts.last_delta_lf = Default::default();

    let updates = match frame_hdr.disable_cdf_update {
      0 => crate::src::msac::CdfUpdates::Allowed,
      _ => crate::src::msac::CdfUpdates::Frozen,
    };
    let Ok(msac_context) = MsacContext::new(data, updates, &c.dsp.msac);
    ts_c.msac = msac_context;

    ts.tiling.row = {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(tile_row);
      numeric_value
    };
    ts.tiling.col = {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(tile_col);
      numeric_value
    };
    ts.tiling.col_start = i32::wrapping_shl(col_sb_start, {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(sb_shift);
      numeric_value
    });
    ts.tiling.col_end = cmp::min(
      i32::wrapping_shl(col_sb_end, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(sb_shift);
        numeric_value
      }),
      bw,
    );
    ts.tiling.row_start = i32::wrapping_shl(row_sb_start, {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(sb_shift);
      numeric_value
    });
    ts.tiling.row_end = cmp::min(
      i32::wrapping_shl(row_sb_end, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(sb_shift);
        numeric_value
      }),
      bh,
    );
    let diff_width = (match frame_hdr.size.width {
      [element, ..] => element,
    }) != (match frame_hdr.size.width {
      [_, element, ..] => element,
    });

    let (sb_idx, unit_idx) = match ::core::primitive::bool::then_some(diff_width, ()) {
      ::core::option::Option::Some(()) => (
        i32::wrapping_mul(i32::wrapping_shr(ts.tiling.row_start, 5_u32), sr_sb128w),
        i32::wrapping_shr(ts.tiling.row_start & 16, 3_u32),
      ),
      ::core::option::Option::None => (
        i32::wrapping_add(
          i32::wrapping_mul(i32::wrapping_shr(ts.tiling.row_start, 5_u32), sb128w),
          col_sb128_start,
        ),
        i32::wrapping_add(
          i32::wrapping_shr(ts.tiling.row_start & 16, 3_u32),
          i32::wrapping_shr(ts.tiling.col_start & 16, 4_u32),
        ),
      ),
    };

    let [luma_unit, chroma_unit] = frame_hdr.restoration.unit_size;
    let plane_mask = lf.restore_planes.bits();

    for (plane, previous) in (0u8..).zip(ts.lr_ref.get_mut()) {
      match plane_mask.wrapping_shr(u32::from(plane)) & 1 {
        0 => continue,
        _ => {}
      }

      let (superblock, quadrant) = match diff_width.then_some(()) {
        Some(()) => {
          let horizontal_subsampling =
            u8::from(plane != 0 && cur.p.layout != Rav1dPixelLayout::I444);
          let size = match plane {
            0 => luma_unit,
            _ => chroma_unit,
          };
          let denominator = i32::from(frame_hdr.size.super_res.width_scale_denominator);
          let round = 8i32.wrapping_shl(u32::from(size)).wrapping_sub(1);
          let scaled_column = ts
            .tiling
            .col_start
            .wrapping_mul(4)
            .wrapping_mul(denominator)
            .wrapping_shr(u32::from(horizontal_subsampling))
            .wrapping_add(round)
            .wrapping_shr(u32::from(size.wrapping_add(3)));
          let x =
            scaled_column.wrapping_shl(u32::from(size.wrapping_add(horizontal_subsampling)));
          let block_x = x.wrapping_shr(7);

          match (block_x >= sr_sb128w).then_some(()) {
            Some(()) => continue,
            None => {}
          }

          (
            sb_idx.wrapping_add(block_x),
            unit_idx.wrapping_add((x & 64).wrapping_shr(6)),
          )
        }
        None => (sb_idx, unit_idx),
      };

      let block_index = console_core_number_conversion::checked_index(superblock)
        .map_err(|_outside_mask| EINVAL)?;
      let unit_index = console_core_number_conversion::checked_index(quadrant)
        .map_err(|_outside_unit| EINVAL)?;
      let Ok(plane_index) = console_core_number_conversion::index(plane);
      let mask = lf.lr_mask.get_mut(block_index).ok_or(EINVAL)?;
      let units = mask.lr.get_mut(plane_index).ok_or(EINVAL)?;
      let reference = units.get_mut(unit_index).ok_or(EINVAL)?;
      let unit = reference.get_mut();
      *unit = Av1RestorationUnit {
        filter_v: [3, -7, 15],
        filter_h: [3, -7, 15],
        sgr_weights: [-32, 31],
        ..*unit
      };
      *previous = *unit;
    }

    match ::core::primitive::bool::then_some(c.tc.len() > 1, ()) {
      ::core::option::Option::Some(()) => {
        ts.progress.fill_with(|| AtomicI32::new(row_sb_start));
      }
      ::core::option::Option::None => {}
    }
  };

  Ok(())
}

struct SetupTileArguments {
  bitdepth_max: i32,
  sb_shift: i32,
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
    others => {
      let probability = match others {
        Rav1dRestorationType::Wiener => &mut *cdf.m.restore_wiener,
        _ => &mut *cdf.m.restore_sgrproj,
      };
      let selected = matches!(
        rav1d_msac_decode_bool_adapt(msac, probability),
        Ok(EntropyBit::One)
      );

      match ::core::primitive::bool::then_some(selected, ()) {
        ::core::option::Option::Some(()) => others,
        ::core::option::Option::None => Rav1dRestorationType::None,
      }
    }
  };

  fn read_subexpression(
    msac: &mut MsacContext,
    reference: i8,
    order: u8,
    adjustment: i8,
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
            let Ok(value) = read_subexpression(msac, outer_reference, 1, 5);
            value
          }
          _ => 0,
        };
        let Ok(middle) = read_subexpression(msac, middle_reference, 2, 23);
        let Ok(inner) = read_subexpression(msac, inner_reference, 3, 17);
        *filter = [outer, middle, inner];
      }

      lr.sgr_weights = reference.sgr_weights;
      let mut references_2 = ts.lr_ref.try_write().ok_or(EINVAL)?;

      *references_2.get_mut(plane_index).ok_or(EINVAL)? = *lr;
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
          let Ok(value) = read_subexpression(msac, outer_reference, 4, 96);
          value
        }
      };
      let inner = match inner_scale {
        0 => 95,
        _ => {
          let Ok(value) = read_subexpression(msac, inner_reference, 4, 32);
          value
        }
      };
      lr.sgr_weights = [outer, inner];
      lr.filter_v = reference.filter_v;
      lr.filter_h = reference.filter_h;
      let mut references_3 = ts.lr_ref.try_write().ok_or(EINVAL)?;

      *references_3.get_mut(plane_index).ok_or(EINVAL)? = *lr;
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

pub(crate) fn rav1d_decode_tile_sbrow(
  c: &Rav1dContext,
  t: &mut Rav1dTaskContext,
  f: &Rav1dFrameData,
) -> Result<(), ()> {
  let seq_hdr = &***f.seq_hdr.as_ref().ok_or(())?;
  let root_bl = match ::core::primitive::bool::then_some(seq_hdr.sb128 != 0, ()) {
    ::core::option::Option::Some(()) => BlockLevel::Bl128x128,
    ::core::option::Option::None => BlockLevel::Bl64x64,
  };
  let ts = &*match f.tile(t.tile_index) {
    ::core::option::Option::Some(indexed_value) => indexed_value,
    ::core::option::Option::None => return ::core::result::Result::Err(()),
  };
  let sb_step = f.sb_step;
  let tile_row = ts.tiling.row;
  let tile_col = ts.tiling.col;
  let f_frame_hdr = f.frame_hdr.as_ref().ok_or(())?;
  let frame_hdr = &***f_frame_hdr;
  let Ok(col_sb_start) = console_core_number_conversion::wrapped::<_, i32>(*match <[_]>::get(
    &frame_hdr.tiling.col_start_sb,
    {
      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(tile_col);
      numeric_value
    },
  ) {
    ::core::option::Option::Some(indexed_value) => indexed_value,
    ::core::option::Option::None => return ::core::result::Result::Err(()),
  });
  let col_sb128_start = i32::wrapping_shr(col_sb_start, {
    let Ok(numeric_value) =
      console_core_number_conversion::wrapped::<_, u32>(i32::from(seq_hdr.sb128 == 0));
    numeric_value
  });

  match ::core::primitive::bool::then_some(
    frame_hdr.frame_type.is_inter_or_switch() || frame_hdr.allow_intrabc,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(row_shift) = console_core_number_conversion::wrapped::<_, u32>(f.sb_shift);
      t.rt = rav1d_refmvs_tile_sbrow_init(
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

  let Ok(()) = reset_context(
    &mut t.l,
    frame_hdr.frame_type.is_key_or_intra(),
    t.frame_thread.pass,
  );

  crate::src::owned_recon::arm_sbrow(f, t).map_err(|_invalid_band| ())?;

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

      for bx in (ts.tiling.col_start..ts.tiling.col_end).step_by({
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(sb_step);
        numeric_value
      }) {
        t.b.x = bx;

        match ::core::primitive::bool::then_some(c.flush.load(Ordering::Acquire), ()) {
          ::core::option::Option::Some(()) => {
            return Err(());
          }
          ::core::option::Option::None => {}
        }

        let Ok(root) = EdgeIndex::root();
        decode_sb(c, t, f, &mut FrameThreadPassState::Second, root_bl, root)?;

        match ::core::primitive::bool::then_some(
          t.b.x & 16 != 0
            || ({
              let explicit_value = f.seq_hdr().map_err(|_missing_header| ())?;
              explicit_value
            })
            .sb128
              != 0,
          (),
        ) {
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

  for bx in (ts.tiling.col_start..ts.tiling.col_end).step_by({
    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(sb_step);
    numeric_value
  }) {
    t.b.x = bx;

    match ::core::primitive::bool::then_some(c.flush.load(Ordering::Acquire), ()) {
      ::core::option::Option::Some(()) => {
        return Err(());
      }
      ::core::option::Option::None => {}
    }

    reset_superblock_cdef(f, t, root_bl).map_err(|_invalid_cdef| ())?;

    let frame_hdr_2 = &***f.frame_hdr.as_ref().ok_or(())?;

    'over_p_2: for plane in 0_u8..3 {
      let Ok(p) = console_core_number_conversion::index(plane);

      match ::core::primitive::bool::then_some(
        u8::wrapping_shr(f.lf.restore_planes.bits(), {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(plane);
          numeric_value
        }) & 1
          == 0,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          continue 'over_p_2;
        }
        ::core::option::Option::None => {}
      }

      let ss_ver = i32::from(p != 0 && f.cur.p.layout == Rav1dPixelLayout::I420);
      let ss_hor = i32::from(p != 0 && f.cur.p.layout != Rav1dPixelLayout::I444);
      let unit_size_log2 = *match <[_]>::get(
        &frame_hdr_2.restoration.unit_size,
        match ::console_core_number_conversion::index::<::core::primitive::u8>(
          ::core::primitive::u8::from(p != 0),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        },
      ) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => return ::core::result::Result::Err(()),
      };
      let y = i32::wrapping_shr(i32::wrapping_mul(t.b.y, 4), {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
        numeric_value
      });
      let h = i32::wrapping_shr(i32::wrapping_add(f.cur.p.h, ss_ver), {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_ver);
        numeric_value
      });

      let unit_size = i32::wrapping_shl(1, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(unit_size_log2);
        numeric_value
      });
      let Ok(mask) =
        console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_sub(unit_size, 1));

      match ::core::primitive::bool::then_some(
        ({
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(y);
          numeric_value
        }) & mask
          != 0,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          continue 'over_p_2;
        }
        ::core::option::Option::None => {}
      }

      let half_unit = i32::wrapping_shr(unit_size, 1_u32);

      match ::core::primitive::bool::then_some(y != 0 && i32::wrapping_add(y, half_unit) > h, ())
      {
        ::core::option::Option::Some(()) => {
          continue 'over_p_2;
        }
        ::core::option::Option::None => {}
      }

      let frame_type = *match <[_]>::get(&frame_hdr_2.restoration.r#type, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(p);
        numeric_value
      }) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => return ::core::result::Result::Err(()),
      };

      match ::core::primitive::bool::then_some(
        (match frame_hdr_2.size.width {
          [element, ..] => element,
        }) != (match frame_hdr_2.size.width {
          [_, element, ..] => element,
        }),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let w = i32::wrapping_shr(i32::wrapping_add(f.sr_cur.p.p.w, ss_hor), {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
            numeric_value
          });
          let n_units = cmp::max(
            1,
            i32::wrapping_shr(i32::wrapping_add(w, half_unit), {
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped::<_, u32>(unit_size_log2);
              numeric_value
            }),
          );

          let Ok(d) = console_core_number_conversion::wrapped::<_, i32>(
            frame_hdr_2.size.super_res.width_scale_denominator,
          );
          let rnd = i32::wrapping_sub(i32::wrapping_mul(unit_size, 8), 1);
          let shift = u8::wrapping_add(unit_size_log2, 3);
          let x0 = i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_shr(i32::wrapping_mul(i32::wrapping_mul(4, t.b.x), d), {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                numeric_value
              }),
              rnd,
            ),
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(shift);
              numeric_value
            },
          );
          let x1 = i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_shr(
                i32::wrapping_mul(i32::wrapping_mul(4, i32::wrapping_add(t.b.x, sb_step)), d),
                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ss_hor);
                  numeric_value
                },
              ),
              rnd,
            ),
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(shift);
              numeric_value
            },
          );

          for x in x0..cmp::min(x1, n_units) {
            let px_x = i32::wrapping_shl(x, {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                u8::wrapping_add(unit_size_log2, {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u8>(ss_hor);
                  numeric_value
                }),
              );
              numeric_value
            });
            let sb_idx = i32::wrapping_add(
              i32::wrapping_mul(i32::wrapping_shr(t.b.y, 5_u32), f.sr_sb128w),
              i32::wrapping_shr(px_x, 7_u32),
            );
            let unit_idx = i32::wrapping_add(
              i32::wrapping_shr(t.b.y & 16, 3_u32),
              i32::wrapping_shr(px_x & 64, 6_u32),
            );
            let mut lr = (*match <[_]>::get(
              &(*match <[_]>::get(
                &(*match <[_]>::get(&f.lf.lr_mask, {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_index(sb_idx);
                  numeric_value
                }) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => return ::core::result::Result::Err(()),
                })
                .lr,
                p,
              ) {
                ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              }),
              {
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(unit_idx);
                numeric_value
              },
            ) {
              ::core::option::Option::Some(indexed_value_3) => indexed_value_3,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            })
            .try_write()
            .ok_or(())?;

            read_restoration_info(ts, &mut lr, plane, frame_type, debug_block_info!(f, t.b))
              .map_err(|_invalid_restoration| ())?;
          }
        }
        ::core::option::Option::None => {
          let x = i32::wrapping_shr(i32::wrapping_mul(4, t.b.x), {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
            numeric_value
          });

          match ::core::primitive::bool::then_some(
            ({
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(x);
              numeric_value
            }) & mask
              != 0,
            (),
          ) {
            ::core::option::Option::Some(()) => {
              continue 'over_p_2;
            }
            ::core::option::Option::None => {}
          }

          let w = i32::wrapping_shr(i32::wrapping_add(f.cur.p.w, ss_hor), {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
            numeric_value
          });

          match ::core::primitive::bool::then_some(
            x != 0 && i32::wrapping_add(x, half_unit) > w,
            (),
          ) {
            ::core::option::Option::Some(()) => {
              continue 'over_p_2;
            }
            ::core::option::Option::None => {}
          }

          let sb_idx = i32::wrapping_add(
            i32::wrapping_mul(i32::wrapping_shr(t.b.y, 5_u32), f.sr_sb128w),
            i32::wrapping_shr(t.b.x, 5_u32),
          );
          let unit_idx = i32::wrapping_add(
            i32::wrapping_shr(t.b.y & 16, 3_u32),
            i32::wrapping_shr(t.b.x & 16, 4_u32),
          );
          let mut lr = (*match <[_]>::get(
            &(*match <[_]>::get(
              &(*match <[_]>::get(&f.lf.lr_mask, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(sb_idx);
                numeric_value
              }) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => return ::core::result::Result::Err(()),
              })
              .lr,
              p,
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => return ::core::result::Result::Err(()),
            }),
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(unit_idx);
              numeric_value
            },
          ) {
            ::core::option::Option::Some(indexed_value_3) => indexed_value_3,
            ::core::option::Option::None => return ::core::result::Result::Err(()),
          })
          .try_write()
          .ok_or(())?;

          read_restoration_info(ts, &mut lr, plane, frame_type, debug_block_info!(f, t.b))
            .map_err(|_invalid_restoration| ())?;
        }
      }
    }

    {
      let tile = f.tile(t.tile_index).ok_or(())?;
      let mut context_2 = tile.context.try_lock().ok_or(())?;
      let Ok(root_edge) = EdgeIndex::root();
      let mut pass = FrameThreadPassState::First(&mut context_2);
      decode_sb(c, t, f, &mut pass, root_bl, root_edge)?;
    }

    match ::core::primitive::bool::then_some(
      t.b.x & 16 != 0
        || ({
          let explicit_value = f.seq_hdr().map_err(|_missing_header| ())?;
          explicit_value
        })
        .sb128
          != 0,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        t.above_index = t.above_index.wrapping_add(1);

        t.lf_mask = t.lf_mask.map(|i| i.wrapping_add(1));
      }
      ::core::option::Option::None => {}
    }
  }

  let f_seq_hdr = f.seq_hdr().map_err(|_missing_header| ())?;

  match ::core::primitive::bool::then_some(
    f_seq_hdr.ref_frame_mvs != 0
      && c.tc.len() > 1
      && ({
        let explicit_value = f.frame_hdr().map_err(|_missing_header| ())?;
        explicit_value
      })
      .frame_type
      .is_inter_or_switch(),
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

  let checked_value_4 = u64::try_from(f.bh).map_err(|_invalid_height| ())?;
  let padded_height = checked_value_4.wrapping_add(31) & !31;
  let column = u64::try_from(tile_col).map_err(|_invalid_tile| ())?;
  let row = u64::try_from(t.b.y).map_err(|_invalid_row| ())?;
  let step = u64::try_from(sb_step).map_err(|_invalid_superblock| ())?;
  let vertical = u32::from(f.cur.p.layout == Rav1dPixelLayout::I420);
  let edge_range = |shift| {
    let first = (row & 16).wrapping_shr(shift);
    let end = first.wrapping_add(step.wrapping_shr(shift));
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

  {
    let tile_context = ts.context.try_lock().ok_or(())?;
    tile_context
      .msac
      .check()
      .map_err(|_invalid_entropy_table| ())?;

    match (tile_context.msac.cnt <= -15).then_some(()) {
      Some(()) => return Err(()),
      None => {}
    }
  }

  match ::core::primitive::bool::then_some(
    c.strict_std_compliance
      && i32::wrapping_add(
        i32::wrapping_shr(t.b.y, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(f.sb_shift);
          numeric_value
        }),
        1,
      ) >= <::core::primitive::i32 as ::core::convert::From<::core::primitive::u16>>::from(
        *match <[_]>::get(
          &({
            let explicit_value = f.frame_hdr().map_err(|_missing_header| ())?;
            explicit_value
          })
          .tiling
          .row_start_sb,
          ({
            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(tile_row);
            numeric_value
          })
          .wrapping_add(1),
        ) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => return ::core::result::Result::Err(()),
        },
      ),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return check_trailing_bits_after_symbol_coder(&ts.context.try_lock().ok_or(())?.msac);
    }
    ::core::option::Option::None => {}
  }

  Ok(())
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
      let [u, v] = separate_chroma_lines(next_chroma, uv_pitch, rows)?;
      let y_offset = y.offset(0)?;
      lf.cdef_lpf_line = [y_offset, u, v];
    }
  }

  Ok(())
}

fn separate_chroma_lines(base: u64, pitch: i64, rows: u64) -> Rav1dResult<[u64; 2]> {
  let cb = FilterLineBuffer { base, pitch, rows };
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
  let [u, v] = separate_chroma_lines(chroma_base, uv_pitch, rows)?;
  let y = luma.offset(0)?;
  lf.lr_lpf_line = [y, u, v];
  Ok(())
}

pub(crate) fn rav1d_decode_frame_init(c: &Rav1dContext, fc: &Rav1dFrameContext) -> Rav1dResult {
  let mut f = fc.data.try_write().ok_or(EINVAL)?;
  let f_2 = &mut *f;

  let header = f_2.frame_hdr.as_ref().ok_or(EINVAL)?;
  let tiles = u32::from(header.tiling.cols).wrapping_mul(u32::from(header.tiling.rows));
  let workers = u32::try_from(c.tc.len()).map_err(|_too_many_workers| EINVAL)?;
  let parallelism = crate::src::internal::ScratchParallelism { workers, tiles };
  let Ok(()) = f_2.configure_scratch_parallelism(parallelism);

  f_2
    .lf
    .start_of_tile_row
    .try_resize(
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(f_2.sbh);
        numeric_value
      },
      0,
    )
    .map_err(|_| ENOMEM)?;

  let f_2_frame_hdr = f_2.frame_hdr.as_ref().ok_or(EINVAL)?;
  let frame_hdr = &***f_2_frame_hdr;
  let mut sby = 0;

  for tile_row in 0..frame_hdr.tiling.rows {
    *match <[_]>::get_mut(&mut f_2.lf.start_of_tile_row, {
      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(sby);
      numeric_value
    }) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
      }
    } = tile_row;

    {
      let numeric_amount = 1;
      let numeric_target = &mut (sby);
      *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
    };

    while sby
      < ({
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(
          *match <[_]>::get(&frame_hdr.tiling.row_start_sb, {
            let Ok(numeric_value) =
              console_core_number_conversion::wrapped_index(u8::wrapping_add(tile_row, 1));
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
    {
      *match <[_]>::get_mut(&mut f_2.lf.start_of_tile_row, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(sby);
        numeric_value
      }) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
        }
      } = 0;

      {
        let numeric_amount = 1;
        let numeric_target = &mut (sby);
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
      };
    }
  }

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

  let num_sb128 = i32::wrapping_mul(f_2.sb128w, f_2.sb128h);
  let size_mul = &ss_size_mul[f_2.cur.p.layout];
  let f_2_seq_hdr = f_2.seq_hdr.as_ref().ok_or(EINVAL)?;
  let seq_hdr = &***f_2_seq_hdr;
  let hbd = i32::from(seq_hdr.hbd != 0);

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

      let cbi_sz = i32::wrapping_mul(num_sb128, {
        let Ok(numeric_value) =
          console_core_number_conversion::wrapped::<_, i32>(match size_mul {
            &[element, ..] => element,
          });
        numeric_value
      });
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
          Default::default,
        )
        .map_err(|_| ENOMEM)?;

      let cf_sz = i32::wrapping_shl(
        i32::wrapping_mul(num_sb128, {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, i32>(match size_mul {
              &[element, ..] => element,
            });
          numeric_value
        }),
        {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(hbd);
          numeric_value
        },
      );
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
          f_2
            .frame_thread
            .pal
            .try_resize(
              ({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(num_sb128);
                numeric_value
              })
              .wrapping_mul(16)
              .wrapping_mul(16)
              .wrapping_shl({
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(hbd);
                numeric_value
              }),
            )
            .map_err(|_| ENOMEM)?;

          let pal_idx_sz = i32::wrapping_mul(num_sb128, {
            let Ok(numeric_value) =
              console_core_number_conversion::wrapped::<_, i32>(match size_mul {
                &[_, element, ..] => element,
              });
            numeric_value
          });
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
          crate::src::internal::PalettePixels::Present => {
            let _ = mem::take(&mut f_2.frame_thread.pal);
            let _ = mem::take(&mut f_2.frame_thread.pal_idx);
          }
          crate::src::internal::PalettePixels::Empty => {}
        },
      }
    }
    ::core::option::Option::None => {}
  }

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
    let superblocks = u64::try_from(num_sb128).map_err(|_invalid_block_count| EINVAL)?;
    let length = superblocks.wrapping_mul(4096).wrapping_add(3);
    let level_len = console_core_number_conversion::checked_index(length)
      .map_err(|_outside_allocation| ENOMEM)?;
    f_2
      .lf
      .level
      .try_reserve(level_len.saturating_sub(f_2.lf.level.len()))
      .map_err(|_| ENOMEM)?;
    f_2.lf.level.resize_with(level_len, || AtomicU8::new(0));
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

  match ::core::primitive::bool::then_some(frame_hdr.switchable_comp_refs != 0, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(cur_frame_offset) = console_core_number_conversion::wrapped::<_, i32>(
        f_2.cur.frame_hdr.as_ref().ok_or(EINVAL)?.frame_offset,
      );
      let ref_pocs: [_; 7] =
        array::from_fn(
          |i| match f_2.refp[i].p.frame_hdr.as_ref().map(|h| h.frame_offset) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 0,
          },
        );

      for (i, &poc_i) in ref_pocs.iter().enumerate() {
        for (j, &poc_j) in ref_pocs.iter().enumerate().skip(i.wrapping_add(1)) {
          let d = [poc_j, poc_i].map(|poc| {
            let Ok(ref_poc) = console_core_number_conversion::wrapped::<_, i32>(poc);
            let Ok(poc_diff) = get_poc_diff(
              seq_hdr.order_hint_n_bits,
              OrderHintComparison {
                target: ref_poc,
                origin: cur_frame_offset,
              },
            );

            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(cmp::min(
                poc_diff.unsigned_abs(),
                31,
              ));
              numeric_value
            }
          });
          let order = (match d {
            [element, ..] => element,
          }) <= (match d {
            [_, element, ..] => element,
          });

          static quant_dist_weight: [[u8; 2]; 3] = [[2, 3], [2, 5], [2, 7]];
          static quant_dist_lookup_table: [[u8; 2]; 4] = [[9, 7], [11, 5], [12, 4], [13, 3]];

          let ::core::result::Result::Ok(k) = ::console_core_number_conversion::fixed_index(
            quant_dist_weight
              .into_iter()
              .position(|weight| {
                let c_2 = [order, !order].map(|order_2| {
                  weight[match ::console_core_number_conversion::index::<::core::primitive::u8>(
                    ::core::primitive::u8::from(order_2),
                  ) {
                    ::core::result::Result::Ok(converted_position) => converted_position,
                  }]
                });
                let dc: [_; 2] = array::from_fn(|i_2| u8::wrapping_mul(d[i_2], c_2[i_2]));
                !order
                  && (match dc {
                    [element, ..] => element,
                  }) < (match dc {
                    [_, element, ..] => element,
                  })
                  || order
                    && (match dc {
                      [element, ..] => element,
                    }) > (match dc {
                      [_, element, ..] => element,
                    })
              })
              .unwrap_or(quant_dist_weight.len()),
          );

          *match <[_]>::get_mut(
            &mut (*match <[_]>::get_mut(&mut f_2.jnt_weights, i) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            }),
            j,
          ) {
            ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
            }
          } = *match <[_]>::get(
            &(*match <[_]>::get(
              &quant_dist_lookup_table,
              match ::console_core_number_conversion::index(k) {
                ::core::result::Result::Ok(native_position_) => native_position_,
              },
            ) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            }),
            match ::console_core_number_conversion::index::<::core::primitive::u8>(
              ::core::primitive::u8::from(order),
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            },
          ) {
            ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
            }
          };
        }
      }
    }
    ::core::option::Option::None => {}
  }

  let Ok(()) = crate::src::owned_recon::frame_setup(c, f_2);

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
      destination.copy_from_slice(&prefix);
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
    let (first, rest) = split_tile_payload(owned(&[1, 0, 9, 8, 7, 6])?, Some(2))?;
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

pub(crate) fn rav1d_decode_frame_init_cdf(
  c: &Rav1dContext,
  fc: &Rav1dFrameContext,
  f: &mut Rav1dFrameData,
  in_cdf: &CdfThreadContext,
) -> Rav1dResult {
  let f_frame_hdr = f.frame_hdr.as_ref().ok_or(EINVAL)?;
  let frame_hdr = &***f_frame_hdr;

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

  let mut tile_row = 0u32;
  let mut tile_col = 0u32;
  let Ok(()) = fc.task_thread.update_set.set(false);

  for tile in &f.tiles {
    let start =
      console_core_number_conversion::checked_index(tile.hdr.start).map_err(|_| EINVAL)?;
    let end = console_core_number_conversion::checked_index(tile.hdr.end).map_err(|_| EINVAL)?;

    let mut data = tile.data.data.clone().ok_or(EINVAL)?;

    for (j, (ts, tile_start_off)) in iter::zip(
      &mut *match <[_]>::get_mut(&mut f.ts, ..end.wrapping_add(1)) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
        }
      },
      match ::core::primitive::bool::then_some(uses_2pass, ()) {
        ::core::option::Option::Some(()) => {
          &*match <[_]>::get(&f.frame_thread.tile_start_off, ..end.wrapping_add(1)) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
            }
          }
        }
        ::core::option::Option::None => &[],
      }
      .iter()
      .copied()
      .chain(iter::repeat_n(0, end.wrapping_add(1))),
    )
    .enumerate()
    .skip(start)
    {
      let size_bytes = (j != end).then_some(tiling.n_bytes);
      let (cur_data, rest_data) = split_tile_payload(data, size_bytes)?;
      let seq_hdr = f.seq_hdr.as_ref().ok_or(EINVAL)?;
      setup_tile(
        c,
        ts,
        &***seq_hdr,
        frame_hdr,
        self::SetupTileArguments {
          bitdepth_max: f.bitdepth_max,
          sb_shift: f.sb_shift,
        },
        &f.cur,
        f.bw,
        f.bh,
        &f.frame_thread,
        f.sr_sb128w,
        f.sb128w,
        &mut f.lf,
        in_cdf,
        cur_data,
        tile_row,
        tile_col,
        tile_start_off,
      )?;

      tile_col = tile_col.wrapping_add(1);

      match ::core::primitive::bool::then_some(tile_col == cols, ()) {
        ::core::option::Option::Some(()) => {
          tile_col = 0;

          tile_row = tile_row.wrapping_add(1);
        }
        ::core::option::Option::None => {}
      }

      let Ok(update) = console_core_number_conversion::wrapped_index(tiling.update);

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

  match ::core::primitive::bool::then_some(c.tc.len() > 1, ()) {
    ::core::option::Option::Some(()) => {
      let total = contexts_per_pass.wrapping_mul(u64::from(uses_2pass).wrapping_add(1));
      let Ok(context_count) = console_core_number_conversion::index(total);
      let contexts = f.a.get_mut(..context_count).ok_or(EINVAL)?;

      for (position, context) in (0_u64..).zip(contexts) {
        let pass = match uses_2pass.then_some(()) {
          Some(()) => 1_i32.wrapping_add(i32::from(position >= contexts_per_pass)),
          None => 0,
        };
        let Ok(()) = reset_context(context, frame_hdr.frame_type.is_key_or_intra(), pass);
      }
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn rav1d_decode_frame_main(c: &Rav1dContext, f: &mut Rav1dFrameData) -> Rav1dResult {
  assert!(c.tc.len() == 1);

  let Rav1dContextTaskType::Single(t) = &(*match <[_]>::get(&*c.tc, 0) {
    ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
    ::core::option::Option::None => {
      return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
    }
  })
  .task
  else {
    return Err(EINVAL);
  };

  let mut t_2 = t.lock();

  let frame_hdr = &***f.frame_hdr.as_ref().ok_or(EINVAL)?;

  for ctx in &mut *match <[_]>::get_mut(
    &mut f.a,
    ..({
      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(f.sb128w);
      numeric_value
    })
    .wrapping_mul({
      let Ok(numeric_value) =
        console_core_number_conversion::wrapped_index(frame_hdr.tiling.rows);
      numeric_value
    }),
  ) {
    ::core::option::Option::Some(indexed_value) => indexed_value,
    ::core::option::Option::None => {
      return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
    }
  } {
    let Ok(()) = reset_context(ctx, frame_hdr.frame_type.is_key_or_intra(), 0);
  }

  let Rav1dFrameHeaderTiling { rows, cols, .. } = frame_hdr.tiling;
  let Ok(row_boundaries) = console_core_number_conversion::index(u32::from(rows).wrapping_add(1));
  let columns = u32::from(cols);
  let row_start_sb = frame_hdr.tiling.row_start_sb;

  let Ok(stop) = c.stop_token();

  let boundaries = row_start_sb.get(..row_boundaries).ok_or(EINVAL)?;

  for (tile_row, sbh_start_end) in (0_u32..).zip(boundaries.windows(2)) {
    let [sbh_start, sbh_end] =
      <[u16; 2]>::try_from(sbh_start_end).map_err(|_incomplete_row| EINVAL)?;

    let sbh_end_2 = cmp::min(
      <::core::primitive::i32 as ::core::convert::From<::core::primitive::u16>>::from(sbh_end),
      f.sbh,
    );

    for sby in
      <::core::primitive::i32 as ::core::convert::From<::core::primitive::u16>>::from(sbh_start)
        ..sbh_end_2
    {
      if let Some(stop_2) = &stop
        && stop_2.check().is_err()
      {
        return Err(ECANCELED);
      }

      let f_seq_hdr = f.seq_hdr.as_ref().ok_or(EINVAL)?;
      let seq_hdr = &***f_seq_hdr;
      let frame_hdr_2 = &***f.frame_hdr.as_ref().ok_or(EINVAL)?;
      t_2.b.y = i32::wrapping_shl(sby, {
        let Ok(numeric_value) =
          console_core_number_conversion::wrapped::<_, u32>(u8::wrapping_add(4, seq_hdr.sb128));
        numeric_value
      });
      let by_end = i32::wrapping_shr(i32::wrapping_add(t_2.b.y, f.sb_step), 1_u32);

      match ::core::primitive::bool::then_some(frame_hdr_2.use_ref_frame_mvs != 0, ()) {
        ::core::option::Option::Some(()) => {
          c.dsp.refmvs.load_tmvs.call(
            &f.rf,
            &f.ref_mvs,
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(tile_row);
              numeric_value
            },
            TemporalMvRegion {
              columns: 0..f.bw.wrapping_shr(1),
              rows: t_2.b.y.wrapping_shr(1)..by_end,
            },
          )?;
        }
        ::core::option::Option::None => {}
      }

      for col in 0..columns {
        t_2.tile_index = tile_row.wrapping_mul(columns).wrapping_add(col);
        rav1d_decode_tile_sbrow(c, &mut t_2, f).map_err(|()| EINVAL)?;
      }

      let f_frame_hdr = f.frame_hdr()?;

      match ::core::primitive::bool::then_some(f_frame_hdr.frame_type.is_inter_or_switch(), ()) {
        ::core::option::Option::Some(()) => {
          c.dsp.refmvs.save_tmvs.call(
            &t_2.rt,
            &f.rf,
            &f.mvs,
            TemporalMvRegion {
              columns: 0..f.bw.wrapping_shr(1),
              rows: t_2.b.y.wrapping_shr(1)..by_end,
            },
          )?;
        }
        ::core::option::Option::None => {}
      }

      let Ok(bd_fn) = f.bd_fn();
      (bd_fn.filter_sbrow)(c, f, &mut t_2, sby)?;
    }
  }

  Ok(())
}

pub(crate) fn rav1d_decode_frame_exit(
  c: &Rav1dContext,
  fc: &Rav1dFrameContext,
  mut retval: Rav1dResult,
) -> Rav1dResult {
  let task_thread = &fc.task_thread;

  let mut f = fc.data.write();

  match ::core::primitive::bool::then_some(f.sr_cur.p.data.is_some(), ()) {
    ::core::option::Option::Some(()) => {
      task_thread.error.store(0, Ordering::Relaxed);
    }
    ::core::option::Option::None => {}
  }

  let cf = f.frame_thread.cf.get_mut();

  match ::core::primitive::bool::then_some(c.fc.len() > 1 && retval.is_err(), ()) {
    ::core::option::Option::Some(()) => {
      cf.fill_with(Default::default);
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    retval.is_ok() && c.fc.len() > 1 && c.strict_std_compliance,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      match ::core::primitive::bool::then_some(
        f.refp.iter().any(|rf| {
          rf.p.frame_hdr.is_some()
            && rf.progress.as_ref().unwrap()[1].load(Ordering::SeqCst) == FRAME_ERROR
        }),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          retval = Err(EINVAL);
          task_thread.error.store(1, Ordering::SeqCst);
          (*match <[_]>::get(&**f.sr_cur.progress.as_mut().unwrap(), 1) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
            }
          })
          .store(FRAME_ERROR, Ordering::SeqCst);
        }
        ::core::option::Option::None => {}
      }
    }
    ::core::option::Option::None => {}
  }

  let _ = mem::take(&mut f.refp);
  let _ = mem::take(&mut f.ref_mvs);
  let _ = mem::take(&mut f.cur);
  let _ = mem::take(&mut f.sr_cur);
  let _ = mem::take(&mut *fc.in_cdf.try_write().unwrap());

  match &f.frame_hdr {
    Some(frame_hdr) => {
      match ::core::primitive::bool::then_some(frame_hdr.refresh_context != 0, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(progress) = f.out_cdf.progress();

          match progress {
            Some(progress_2) => {
              progress_2.store(
                match retval {
                  ::core::result::Result::Ok(_) => 1,
                  ::core::result::Result::Err(_) => {
                    let Ok(numeric_value) =
                      console_core_number_conversion::wrapped::<_, u32>(TILE_ERROR);
                    numeric_value
                  }
                },
                Ordering::SeqCst,
              );
            }
            None => {}
          }

          let _ = mem::take(&mut f.out_cdf);
        }
        ::core::option::Option::None => {}
      }
    }
    None => {}
  }

  let _ = mem::take(&mut f.cur_segmap);
  let _ = mem::take(&mut f.prev_segmap);
  let _ = mem::take(&mut f.mvs);
  let _ = mem::take(&mut f.seq_hdr);
  let _ = mem::take(&mut f.frame_hdr);
  f.tiles.clear();
  task_thread.finished.store(true, Ordering::SeqCst);
  *task_thread.retval.try_lock().unwrap() = retval.err();
  retval
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

pub(crate) fn rav1d_decode_frame(c: &Rav1dContext, fc: &Rav1dFrameContext) -> Rav1dResult {
  assert!(c.fc.len() == 1);

  let mut res = rav1d_decode_frame_init(c, fc);

  {
    let mut f = fc.data.try_write().ok_or(EINVAL)?;

    match res {
      ::core::result::Result::Ok(_) => {
        res = match fc.in_cdf() {
          Ok(input) => rav1d_decode_frame_init_cdf(c, fc, &mut f, &input),
          Err(error) => Err(error),
        };
      }
      ::core::result::Result::Err(_) => {}
    }

    match res {
      ::core::result::Result::Ok(_) => {
        match ::core::primitive::bool::then_some(c.tc.len() > 1, ()) {
          ::core::option::Option::Some(()) => {
            res = rav1d_task_create_tile_sbrow(fc, &f, TileQueueStage::Reconstruction);
            drop(f);
            let ttd = &*c.task_thread;
            let mut task_thread_lock = ttd.lock.lock();
            ttd.cond.notify_one();

            match res {
              ::core::result::Result::Ok(_) => {
                while (*match <[_]>::get(&fc.task_thread.done, 0) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                })
                .load(Ordering::SeqCst)
                  == 0
                  || fc.task_thread.task_counter.load(Ordering::SeqCst) > 0
                {
                  match ::core::primitive::bool::then_some(
                    ttd.panicked.load(Ordering::SeqCst),
                    (),
                  ) {
                    ::core::option::Option::Some(()) => {
                      c.flush.store(true, Ordering::SeqCst);
                      ttd.cond.notify_all();

                      for tc in c.tc.iter() {
                        while let crate::src::internal::FlushState::Waiting = tc.flush_state() {
                          task_thread_lock = unpoison(tc.thread_data.cond.wait(task_thread_lock));
                        }
                      }

                      let Ok(()) = fc.task_thread.tasks.clear();
                      ttd.first.store(0, Ordering::SeqCst);
                      let Ok(length) =
                        console_core_number_conversion::wrapped::<_, u32>(c.fc.len());
                      let Ok(()) = ttd.cur.set(length);
                      ttd.reset_task_cur.store(None, Ordering::SeqCst);
                      ttd.cond_signaled.store(0, Ordering::SeqCst);
                      c.flush.store(false, Ordering::SeqCst);
                      res = Err(EGeneric);
                      break;
                    }
                    ::core::option::Option::None => {}
                  }

                  task_thread_lock = unpoison(fc.task_thread.cond.wait(task_thread_lock));
                }
              }
              ::core::result::Result::Err(_) => {}
            }

            drop(task_thread_lock);

            match res {
              ::core::result::Result::Ok(_) => {
                let result = fc.task_thread.retval.try_lock().ok_or(EINVAL)?;
                res = result.err_or(());
              }
              ::core::result::Result::Err(_) => {}
            }
          }
          ::core::option::Option::None => {
            res = rav1d_decode_frame_main(c, &mut f);
            let f_frame_hdr = f.frame_hdr.as_ref().ok_or(EINVAL)?;
            let frame_hdr = &***f_frame_hdr;

            match ::core::primitive::bool::then_some(
              res.is_ok()
                && frame_hdr.refresh_context != 0
                && ({
                  let Ok(explicit_value) = fc.task_thread.update_set.get();
                  explicit_value
                }),
              (),
            ) {
              ::core::option::Option::Some(()) => {
                res = rav1d_publish_tile_cdf(&f, frame_hdr);
              }
              ::core::option::Option::None => {}
            }
          }
        }
      }
      ::core::result::Result::Err(_) => {}
    }
  }

  rav1d_decode_frame_exit(c, fc, res)
}

fn get_upscale_x0(
  in_w: c_int,
  out_w: c_int,
  step: c_int,
) -> core::result::Result<c_int, console_core_never::Never> {
  Ok({
    let err = i32::wrapping_sub(
      i32::wrapping_mul(out_w, step),
      i32::wrapping_shl(in_w, 14_u32),
    );
    let x0 = i32::wrapping_sub(
      i32::wrapping_add(
        i32::wrapping_add(
          (i32::wrapping_shl(i32::wrapping_sub(out_w, in_w), 13_u32)).wrapping_neg(),
          i32::wrapping_shr(out_w, 1_u32),
        ) / out_w,
        128,
      ),
      ::core::primitive::i32::wrapping_div(err, 2),
    );
    x0 & 0x3fff
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

  let mut f = fc.data.try_write().ok_or(EINVAL)?;
  let seq_hdr = state.seq_hdr.clone().ok_or(EINVAL)?;
  f.seq_hdr = Some(seq_hdr.clone());
  f.frame_hdr = mem::take(&mut state.frame_hdr);

  fn on_error(
    fc: &Rav1dFrameContext,
    f: &mut Rav1dFrameData,
    out: &mut Rav1dThreadPicture,
    cached_error_props: &mut Rav1dDataProps,
    m: &Rav1dDataProps,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      fc.task_thread.error.store(1, Ordering::Relaxed);
      let _ = mem::take(&mut *fc.in_cdf.try_write().unwrap());

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

      for i in 0..7 {
        match ::core::primitive::bool::then_some(f.refp[i].p.frame_hdr.is_some(), ()) {
          ::core::option::Option::Some(()) => {
            let _ = mem::take(&mut f.refp[i]);
          }
          ::core::option::Option::None => {}
        }

        let _ = mem::take(&mut f.ref_mvs[i]);
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

  let bpc = u8::wrapping_add(8, u8::wrapping_mul(2, seq_hdr.hbd));
  let Ok(get) = Rav1dBitDepthDSPContext::get(bpc);

  match get {
    Some(dsp) => f.dsp = dsp,
    None => {
      writeln!(c.logger, "Compiled without support for {bpc}-bit decoding",);
      let Ok(()) = on_error(
        fc,
        &mut f,
        out,
        &mut state.cached_error_props,
        &state.in_0.m,
      );
      return Err(ENOPROTOOPT);
    }
  };

  fn scale_fac(
    ref_sz: i32,
    this_sz: i32,
  ) -> core::result::Result<i32, console_core_never::Never> {
    Ok({
      i32::wrapping_add(
        i32::wrapping_shl(ref_sz, 14_u32),
        i32::wrapping_shr(this_sz, 1_u32),
      ) / this_sz
    })
  }

  let mut ref_coded_width = <[i32; 7]>::default();
  let f_frame_hdr = f.frame_hdr.as_ref().ok_or(EINVAL)?;
  let frame_hdr = f_frame_hdr.clone();

  let Ok(frame_hdr_cols) = console_core_number_conversion::wrapped_index(frame_hdr.tiling.cols);
  let Ok(frame_hdr_rows) = console_core_number_conversion::wrapped_index(frame_hdr.tiling.rows);
  rav1d_disjoint_mut::set_tile_concurrency(frame_hdr_cols.wrapping_mul(frame_hdr_rows));

  match ::core::primitive::bool::then_some(frame_hdr.frame_type.is_inter_or_switch(), ()) {
    ::core::option::Option::Some(()) => {
      match ::core::primitive::bool::then_some(
        frame_hdr.primary_ref_frame != RAV1D_PRIMARY_REF_NONE,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let Ok(pri_ref) =
            console_core_number_conversion::wrapped_index(*match <[_]>::get(&frame_hdr.refidx, {
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped_index(frame_hdr.primary_ref_frame);
              numeric_value
            }) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            });

          match ::core::primitive::bool::then_some(
            (*match <[_]>::get(&state.refs, pri_ref) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            })
            .p
            .p
            .data
            .is_none(),
            (),
          ) {
            ::core::option::Option::Some(()) => {
              let Ok(()) = on_error(
                fc,
                &mut f,
                out,
                &mut state.cached_error_props,
                &state.in_0.m,
              );
              return Err(EINVAL);
            }
            ::core::option::Option::None => {}
          }
        }
        ::core::option::Option::None => {}
      }

      for i in 0..7 {
        let Ok(refidx) = console_core_number_conversion::wrapped_index(*match <[_]>::get(
          &frame_hdr.refidx,
          i,
        ) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        });

        match ::core::primitive::bool::then_some(
          (*match <[_]>::get(&state.refs, refidx) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
            }
          })
          .p
          .p
          .data
          .is_none()
            || i32::wrapping_mul(
              match frame_hdr.size.width {
                [element, ..] => element,
              },
              2,
            ) < (*match <[_]>::get(&state.refs, refidx) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            })
            .p
            .p
            .p
            .w
            || i32::wrapping_mul(frame_hdr.size.height, 2)
              < (*match <[_]>::get(&state.refs, refidx) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              })
              .p
              .p
              .p
              .h
            || (match frame_hdr.size.width {
              [element, ..] => element,
            }) > i32::wrapping_mul(
              (*match <[_]>::get(&state.refs, refidx) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              })
              .p
              .p
              .p
              .w,
              16,
            )
            || frame_hdr.size.height
              > i32::wrapping_mul(
                (*match <[_]>::get(&state.refs, refidx) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                })
                .p
                .p
                .p
                .h,
                16,
              )
            || seq_hdr.layout
              != (*match <[_]>::get(&state.refs, refidx) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              })
              .p
              .p
              .p
              .layout
            || bpc
              != (*match <[_]>::get(&state.refs, refidx) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              })
              .p
              .p
              .p
              .bpc,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            for j in 0..i {
              let _ = mem::take(&mut *match <[_]>::get_mut(&mut f.refp, j) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              });
            }

            let Ok(()) = on_error(
              fc,
              &mut f,
              out,
              &mut state.cached_error_props,
              &state.in_0.m,
            );
            return Err(EINVAL);
          }
          ::core::option::Option::None => {}
        }

        *match <[_]>::get_mut(&mut f.refp, i) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        } = (*match <[_]>::get(&state.refs, refidx) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        })
        .p
        .clone();
        *match <[_]>::get_mut(&mut ref_coded_width, i) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        } = *match <[_]>::get(
          &(*match <[_]>::get(&state.refs, refidx) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
            }
          })
          .p
          .p
          .frame_hdr
          .as_ref()
          .unwrap()
          .size
          .width,
          0,
        ) {
          ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        };

        match ::core::primitive::bool::then_some(
          (match frame_hdr.size.width {
            [element, ..] => element,
          }) != (*match <[_]>::get(&state.refs, refidx) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
            }
          })
          .p
          .p
          .p
          .w || frame_hdr.size.height
            != (*match <[_]>::get(&state.refs, refidx) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            })
            .p
            .p
            .p
            .h,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            (*match <[_]>::get_mut(
              &mut (*match <[_]>::get_mut(&mut f.svc, i) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              }),
              0,
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            })
            .scale = {
              let Ok(explicit_value) = scale_fac(
                (*match <[_]>::get(&state.refs, refidx) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                })
                .p
                .p
                .p
                .w,
                match frame_hdr.size.width {
                  [element, ..] => element,
                },
              );
              explicit_value
            };
            (*match <[_]>::get_mut(
              &mut (*match <[_]>::get_mut(&mut f.svc, i) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              }),
              1,
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            })
            .scale = {
              let Ok(explicit_value) = scale_fac(
                (*match <[_]>::get(&state.refs, refidx) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                })
                .p
                .p
                .p
                .h,
                frame_hdr.size.height,
              );
              explicit_value
            };
            (*match <[_]>::get_mut(
              &mut (*match <[_]>::get_mut(&mut f.svc, i) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              }),
              0,
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            })
            .step = i32::wrapping_shr(
              i32::wrapping_add(
                (*match <[_]>::get(
                  &(*match <[_]>::get(&f.svc, i) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => {
                      return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                    }
                  }),
                  0,
                ) {
                  ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                })
                .scale,
                8,
              ),
              4_u32,
            );
            (*match <[_]>::get_mut(
              &mut (*match <[_]>::get_mut(&mut f.svc, i) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              }),
              1,
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            })
            .step = i32::wrapping_shr(
              i32::wrapping_add(
                (*match <[_]>::get(
                  &(*match <[_]>::get(&f.svc, i) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => {
                      return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                    }
                  }),
                  1,
                ) {
                  ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                })
                .scale,
                8,
              ),
              4_u32,
            );
          }
          ::core::option::Option::None => {
            (*match <[_]>::get_mut(
              &mut (*match <[_]>::get_mut(&mut f.svc, i) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              }),
              1,
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            })
            .scale = 0;
            (*match <[_]>::get_mut(
              &mut (*match <[_]>::get_mut(&mut f.svc, i) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              }),
              0,
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            })
            .scale = (*match <[_]>::get(
              &(*match <[_]>::get(&f.svc, i) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              }),
              1,
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            })
            .scale;
          }
        }

        *match <[_]>::get_mut(&mut f.gmv_warp_allowed, i) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        } = u8::from(
          (*match <[_]>::get(&frame_hdr.gmv, i) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
            }
          })
          .r#type
            > Rav1dWarpedMotionType::Translation
            && !frame_hdr.force_integer_mv
            && rav1d_get_shear_params(&*match <[_]>::get(&frame_hdr.gmv, i) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            }) == Ok(WarpValidity::Usable)
            && (*match <[_]>::get(
              &(*match <[_]>::get(&f.svc, i) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              }),
              0,
            ) {
              ::core::option::Option::Some(indexed_value_2) => indexed_value_2,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            })
            .scale
              == 0,
        );
      }
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    frame_hdr.primary_ref_frame == RAV1D_PRIMARY_REF_NONE,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      *fc.in_cdf.try_write().unwrap() = {
        let Ok(explicit_value) = rav1d_cdf_thread_init_static(frame_hdr.quant.yac);
        explicit_value
      };
    }
    ::core::option::Option::None => {
      let Ok(pri_ref) =
        console_core_number_conversion::wrapped_index(*match <[_]>::get(&frame_hdr.refidx, {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped_index(frame_hdr.primary_ref_frame);
          numeric_value
        }) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        });
      *fc.in_cdf.try_write().unwrap() = (*match <[_]>::get(&state.cdf, pri_ref) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
        }
      })
      .clone();
    }
  }

  match ::core::primitive::bool::then_some(frame_hdr.refresh_context != 0, ()) {
    ::core::option::Option::Some(()) => {
      let threading = match c.fc.len() {
        0 | 1 => CdfThreading::Serial,
        _ => CdfThreading::ParallelFrames,
      };
      let res = rav1d_cdf_thread_alloc(threading);

      match res {
        Err(e) => {
          let Ok(()) = on_error(
            fc,
            &mut f,
            out,
            &mut state.cached_error_props,
            &state.in_0.m,
          );
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
    &mut f,
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
    ::core::result::Result::Err(_) => {
      let Ok(()) = on_error(
        fc,
        &mut f,
        out,
        &mut state.cached_error_props,
        &state.in_0.m,
      );
      return res;
    }
    ::core::result::Result::Ok(_) => {}
  }

  let f_seq_hdr = f.seq_hdr.as_ref().ok_or(EINVAL)?;
  let seq_hdr_2 = f_seq_hdr.clone();
  let checked_value = f.frame_hdr.as_ref().ok_or(EINVAL)?;
  let frame_hdr_2 = checked_value.clone();

  match ::core::primitive::bool::then_some(
    (match frame_hdr_2.size.width {
      [element, ..] => element,
    }) != (match frame_hdr_2.size.width {
      [_, element, ..] => element,
    }),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let f_2 = &mut *f;
      let res_3 = rav1d_picture_alloc_copy(
        &c.logger,
        &mut f_2.cur,
        match frame_hdr_2.size.width {
          [element, ..] => element,
        },
        &f_2.sr_cur.p,
      );

      match res_3 {
        ::core::result::Result::Err(_) => {
          let Ok(()) = on_error(fc, f_2, out, &mut state.cached_error_props, &state.in_0.m);
          return res_3;
        }
        ::core::result::Result::Ok(_) => {}
      }
    }
    ::core::option::Option::None => {
      f.cur = f.sr_cur.p.clone();
    }
  }

  match ::core::primitive::bool::then_some(
    (match frame_hdr_2.size.width {
      [element, ..] => element,
    }) != (match frame_hdr_2.size.width {
      [_, element, ..] => element,
    }),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      *{
        let [_explicit_part, ..] = &mut f.resize_step;
        _explicit_part
      } = {
        let Ok(explicit_value) = scale_fac(f.cur.p.w, f.sr_cur.p.p.w);
        explicit_value
      };
      let ss_hor = i32::from(f.cur.p.layout != Rav1dPixelLayout::I444);
      let in_cw = i32::wrapping_shr(i32::wrapping_add(f.cur.p.w, ss_hor), {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
        numeric_value
      });
      let out_cw = i32::wrapping_shr(i32::wrapping_add(f.sr_cur.p.p.w, ss_hor), {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(ss_hor);
        numeric_value
      });
      *{
        let [_, _explicit_part, ..] = &mut f.resize_step;
        _explicit_part
      } = {
        let Ok(explicit_value) = scale_fac(in_cw, out_cw);
        explicit_value
      };
      let Ok(upscale_x0) = get_upscale_x0(
        f.cur.p.w,
        f.sr_cur.p.p.w,
        match f.resize_step {
          [element, ..] => element,
        },
      );
      *{
        let [_explicit_part, ..] = &mut f.resize_start;
        _explicit_part
      } = upscale_x0;
      *{
        let [_, _explicit_part, ..] = &mut f.resize_start;
        _explicit_part
      } = {
        let Ok(explicit_value) = get_upscale_x0(
          in_cw,
          out_cw,
          match f.resize_step {
            [_, element, ..] => element,
          },
        );
        explicit_value
      };
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(c.fc.len() == 1, ()) {
    ::core::option::Option::Some(()) => match ::core::primitive::bool::then_some(
      frame_hdr_2.show_frame != 0 || c.output_invisible_frames,
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

  f.w4 = i32::wrapping_shr(
    i32::wrapping_add(
      match frame_hdr_2.size.width {
        [element, ..] => element,
      },
      3,
    ),
    2_u32,
  );
  f.h4 = i32::wrapping_shr(i32::wrapping_add(frame_hdr_2.size.height, 3), 2_u32);
  f.bw = i32::wrapping_shl(
    i32::wrapping_shr(
      i32::wrapping_add(
        match frame_hdr_2.size.width {
          [element, ..] => element,
        },
        7,
      ),
      3_u32,
    ),
    1_u32,
  );
  f.bh = i32::wrapping_shl(
    i32::wrapping_shr(i32::wrapping_add(frame_hdr_2.size.height, 7), 3_u32),
    1_u32,
  );
  f.sb128w = i32::wrapping_shr(i32::wrapping_add(f.bw, 31), 5_u32);
  f.sb128h = i32::wrapping_shr(i32::wrapping_add(f.bh, 31), 5_u32);
  let Ok(sb128) = console_core_number_conversion::wrapped::<_, i32>(seq_hdr_2.sb128);
  f.sb_shift = i32::wrapping_add(4, sb128);
  let Ok(seq_hdr_2_sb128) = console_core_number_conversion::wrapped::<_, u32>(seq_hdr_2.sb128);
  f.sb_step = i32::wrapping_shl(16, seq_hdr_2_sb128);
  f.sbh = i32::wrapping_shr(i32::wrapping_sub(i32::wrapping_add(f.bh, f.sb_step), 1), {
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(f.sb_shift);
    numeric_value
  });
  let padded_width = f.bw.wrapping_add(31) & !31;
  let checked_value_2 = u32::try_from(padded_width).map_err(|_negative_stride| EINVAL)?;
  f.b4_stride = checked_value_2;
  let Ok(f_bpc) = console_core_number_conversion::wrapped::<_, u32>(f.cur.p.bpc);
  f.bitdepth_max = i32::wrapping_sub(i32::wrapping_shl(1, f_bpc), 1);
  fc.task_thread.error.store(0, Ordering::Relaxed);
  let uses_2pass = i32::from(c.fc.len() > 1);
  let cols = frame_hdr_2.tiling.cols;
  let rows = frame_hdr_2.tiling.rows;
  fc.task_thread.task_counter.store(
    i32::wrapping_shl(
      i32::wrapping_add(
        i32::wrapping_mul(
          {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(cols);
            numeric_value
          },
          {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(rows);
            numeric_value
          },
        ),
        f.sbh,
      ),
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(uses_2pass);
        numeric_value
      },
    ),
    Ordering::SeqCst,
  );

  match ::core::primitive::bool::then_some(
    frame_hdr_2.frame_type.is_inter_or_switch() || frame_hdr_2.allow_intrabc,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let superblock_rows = u64::try_from(f.sb128h).map_err(|_negative_height| EINVAL)?;
      let mvs_sz = superblock_rows
        .wrapping_mul(16)
        .wrapping_mul(u64::from(f.b4_stride.wrapping_shr(1)));
      let mut mvs = crate::src::disjoint_mut::dm_arc_try_new(mvs_sz, Default::default())?;
      let motion_buffer = std::sync::Arc::get_mut(&mut mvs.inner).ok_or(EGeneric)?;
      motion_buffer.configure_parallelism(
        c.tc.len(),
        ({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cols);
          numeric_value
        })
        .wrapping_mul({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(rows);
          numeric_value
        }),
      );
      f.mvs = Some(mvs);

      match ::core::primitive::bool::then_some(!frame_hdr_2.allow_intrabc, ()) {
        ::core::option::Option::Some(()) => {
          for i in 0..7 {
            let Ok(frame_offset) = console_core_number_conversion::wrapped::<_, u32>(
              (*match <[_]>::get(&f.refp, i) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              })
              .p
              .frame_hdr
              .as_ref()
              .ok_or(EINVAL)?
              .frame_offset,
            );
            *match <[_]>::get_mut(&mut f.refpoc, i) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            } = frame_offset;
          }
        }
        ::core::option::Option::None => {
          f.refpoc.fill(0);
        }
      }

      match ::core::primitive::bool::then_some(frame_hdr_2.use_ref_frame_mvs != 0, ()) {
        ::core::option::Option::Some(()) => {
          for i in 0..7 {
            let Ok(refidx) = console_core_number_conversion::wrapped_index(*match <[_]>::get(
              &frame_hdr_2.refidx,
              i,
            ) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            });
            let ref_w = i32::wrapping_shl(
              i32::wrapping_shr(
                i32::wrapping_add(
                  *match <[_]>::get(&ref_coded_width, i) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => {
                      return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                    }
                  },
                  7,
                ),
                3_u32,
              ),
              1_u32,
            );
            let ref_h = i32::wrapping_shl(
              i32::wrapping_shr(
                i32::wrapping_add(
                  (*match <[_]>::get(&f.refp, i) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => {
                      return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                    }
                  })
                  .p
                  .p
                  .h,
                  7,
                ),
                3_u32,
              ),
              1_u32,
            );

            match ::core::primitive::bool::then_some(ref_w == f.bw && ref_h == f.bh, ()) {
              ::core::option::Option::Some(()) => {
                *match <[_]>::get_mut(&mut f.ref_mvs, i) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                } = (*match <[_]>::get(&state.refs, refidx) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                })
                .refmvs
                .clone();
              }
              ::core::option::Option::None => {
                *match <[_]>::get_mut(&mut f.ref_mvs, i) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                } = None;
              }
            }

            *match <[_]>::get_mut(&mut f.refrefpoc, i) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            } = (*match <[_]>::get(&state.refs, refidx) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            })
            .refpoc;
          }
        }
        ::core::option::Option::None => {
          f.ref_mvs.fill_with(Default::default);
        }
      }
    }
    ::core::option::Option::None => {
      f.mvs = None;
      f.ref_mvs.fill_with(Default::default);
    }
  }

  match ::core::primitive::bool::then_some(frame_hdr_2.segmentation.enabled != 0, ()) {
    ::core::option::Option::Some(()) => {
      f.prev_segmap = None;

      match ::core::primitive::bool::then_some(
        frame_hdr_2.segmentation.temporal != 0 || frame_hdr_2.segmentation.update_map == 0,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let Ok(pri_ref) =
            console_core_number_conversion::wrapped_index(frame_hdr_2.primary_ref_frame);
          assert!(
            pri_ref
              != ({
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped_index(RAV1D_PRIMARY_REF_NONE);
                numeric_value
              })
          );
          let ref_w = i32::wrapping_shl(
            i32::wrapping_shr(
              i32::wrapping_add(
                *match <[_]>::get(&ref_coded_width, pri_ref) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                },
                7,
              ),
              3_u32,
            ),
            1_u32,
          );
          let ref_h = i32::wrapping_shl(
            i32::wrapping_shr(
              i32::wrapping_add(
                (*match <[_]>::get(&f.refp, pri_ref) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                })
                .p
                .p
                .h,
                7,
              ),
              3_u32,
            ),
            1_u32,
          );

          match ::core::primitive::bool::then_some(ref_w == f.bw && ref_h == f.bh, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(frame_hdr_2_refidx) = console_core_number_conversion::wrapped_index(
                *match <[_]>::get(&frame_hdr_2.refidx, pri_ref) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                },
              );
              f.prev_segmap = (*match <[_]>::get(&state.refs, frame_hdr_2_refidx) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              })
              .segmap
              .clone();
            }
            ::core::option::Option::None => {}
          }
        }
        ::core::option::Option::None => {}
      }

      f.cur_segmap = Some(
        match (
          frame_hdr_2.segmentation.update_map != 0,
          f.prev_segmap.as_mut(),
        ) {
          (true, _) | (false, None) => {
            let superblock_rows = u64::try_from(f.sb128h).map_err(|_negative_height| EINVAL)?;
            let segmap_size = u64::from(f.b4_stride)
              .wrapping_mul(32)
              .wrapping_mul(superblock_rows);
            let mut segmap =
              crate::src::disjoint_mut::dm_arc_try_new(segmap_size, Default::default())?;
            let segment_buffer = std::sync::Arc::get_mut(&mut segmap.inner).ok_or(EGeneric)?;
            segment_buffer.configure_parallelism(
              c.tc.len(),
              ({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(cols);
                numeric_value
              })
              .wrapping_mul({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_index(rows);
                numeric_value
              }),
            );
            segmap
          }
          (_, Some(prev_segmap)) => prev_segmap.clone(),
        },
      );
    }
    ::core::option::Option::None => {
      f.cur_segmap = None;
      f.prev_segmap = None;
    }
  }

  let Ok(refresh_frame_flags) =
    console_core_number_conversion::wrapped::<_, u32>(frame_hdr_2.refresh_frame_flags);

  for i in 0..8 {
    match ::core::primitive::bool::then_some(
      refresh_frame_flags
        & u32::wrapping_shl(1, {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(i);
          numeric_value
        })
        != 0,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        match ::core::primitive::bool::then_some(
          (*match <[_]>::get(&state.refs, i) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
            }
          })
          .p
          .p
          .frame_hdr
          .is_some(),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            let _ = mem::take(
              &mut (*match <[_]>::get_mut(&mut state.refs, i) {
                ::core::option::Option::Some(indexed_value) => indexed_value,
                ::core::option::Option::None => {
                  return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                }
              })
              .p,
            );
          }
          ::core::option::Option::None => {}
        }

        (*match <[_]>::get_mut(&mut state.refs, i) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        })
        .p = f.sr_cur.clone();

        match ::core::primitive::bool::then_some(frame_hdr_2.refresh_context != 0, ()) {
          ::core::option::Option::Some(()) => {
            *match <[_]>::get_mut(&mut state.cdf, i) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            } = f.out_cdf.clone();
          }
          ::core::option::Option::None => {
            *match <[_]>::get_mut(&mut state.cdf, i) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            } = fc.in_cdf.try_read().unwrap().clone();
          }
        }

        (*match <[_]>::get_mut(&mut state.refs, i) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        })
        .segmap = f.cur_segmap.clone();
        let _ = mem::take(
          &mut (*match <[_]>::get_mut(&mut state.refs, i) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
            }
          })
          .refmvs,
        );

        match ::core::primitive::bool::then_some(!frame_hdr_2.allow_intrabc, ()) {
          ::core::option::Option::Some(()) => {
            (*match <[_]>::get_mut(&mut state.refs, i) {
              ::core::option::Option::Some(indexed_value) => indexed_value,
              ::core::option::Option::None => {
                return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
              }
            })
            .refmvs = f.mvs.clone();
          }
          ::core::option::Option::None => {}
        }

        (*match <[_]>::get_mut(&mut state.refs, i) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
          }
        })
        .refpoc = f.refpoc;
      }
      ::core::option::Option::None => {}
    }
  }

  drop(f);

  match ::core::primitive::bool::then_some(c.fc.len() == 1, ()) {
    ::core::option::Option::Some(()) => {
      let res_4 = rav1d_decode_frame(c, &fc);

      match res_4 {
        ::core::result::Result::Err(_) => {
          let _ = mem::take(&mut state.out);

          for i in 0..8 {
            match ::core::primitive::bool::then_some(
              refresh_frame_flags
                & u32::wrapping_shl(1, {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(i);
                  numeric_value
                })
                != 0,
              (),
            ) {
              ::core::option::Option::Some(()) => {
                match ::core::primitive::bool::then_some(
                  (*match <[_]>::get(&state.refs, i) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => {
                      return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                    }
                  })
                  .p
                  .p
                  .frame_hdr
                  .is_some(),
                  (),
                ) {
                  ::core::option::Option::Some(()) => {
                    let _ = mem::take(
                      &mut (*match <[_]>::get_mut(&mut state.refs, i) {
                        ::core::option::Option::Some(indexed_value) => indexed_value,
                        ::core::option::Option::None => {
                          return ::core::result::Result::Err(
                            crate::src::error::Rav1dError::EINVAL,
                          );
                        }
                      })
                      .p,
                    );
                  }
                  ::core::option::Option::None => {}
                }

                let _ = mem::take(&mut *match <[_]>::get_mut(&mut state.cdf, i) {
                  ::core::option::Option::Some(indexed_value) => indexed_value,
                  ::core::option::Option::None => {
                    return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                  }
                });
                let _ = mem::take(
                  &mut (*match <[_]>::get_mut(&mut state.refs, i) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => {
                      return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                    }
                  })
                  .segmap,
                );
                let _ = mem::take(
                  &mut (*match <[_]>::get_mut(&mut state.refs, i) {
                    ::core::option::Option::Some(indexed_value) => indexed_value,
                    ::core::option::Option::None => {
                      return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
                    }
                  })
                  .refmvs,
                );
              }
              ::core::option::Option::None => {}
            }
          }

          let mut f_3 = fc.data.try_write().unwrap();
          let Ok(()) = on_error(
            fc,
            &mut f_3,
            &mut state.out,
            &mut state.cached_error_props,
            &state.in_0.m,
          );
          return res_4;
        }
        ::core::result::Result::Ok(_) => {}
      }
    }
    ::core::option::Option::None => match rav1d_task_frame_init(c, fc) {
      Ok(()) => {}
      Err(error) => return rav1d_decode_frame_exit(c, fc, Err(error)),
    },
  }

  Ok(())
}
