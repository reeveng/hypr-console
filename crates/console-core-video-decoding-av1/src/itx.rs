#![forbid(unsafe_code)]

use crate::include::common::bitdepth::AsPrimitive;
use crate::include::common::bitdepth::BitDepth;
use crate::include::common::intops::iclip;
use crate::src::cpu::CpuFlags;
use crate::src::error::{Rav1dError, Rav1dResult};
use crate::src::itx_1d::rav1d_inv_dct4_1d_c;
use crate::src::itx_1d::rav1d_inv_dct8_1d_c;
use crate::src::itx_1d::rav1d_inv_dct16_1d_c;
use crate::src::itx_1d::rav1d_inv_dct32_1d_c;
use crate::src::itx_1d::rav1d_inv_dct64_1d_c;
use crate::src::itx_1d::rav1d_inv_identity4_1d_c;
use crate::src::itx_1d::rav1d_inv_identity8_1d_c;
use crate::src::itx_1d::rav1d_inv_identity16_1d_c;
use crate::src::itx_1d::rav1d_inv_identity32_1d_c;
use crate::src::itx_1d::rav1d_inv_wht4_1d_c;
use crate::src::itx_1d::{inverse_adst4, inverse_adst8, inverse_adst16};
use crate::src::levels::ADST_ADST;
use crate::src::levels::ADST_DCT;
use crate::src::levels::ADST_FLIPADST;
use crate::src::levels::DCT_ADST;
use crate::src::levels::DCT_DCT;
use crate::src::levels::DCT_FLIPADST;
use crate::src::levels::FLIPADST_ADST;
use crate::src::levels::FLIPADST_DCT;
use crate::src::levels::FLIPADST_FLIPADST;
use crate::src::levels::H_ADST;
use crate::src::levels::H_DCT;
use crate::src::levels::H_FLIPADST;
use crate::src::levels::IDTX;
use crate::src::levels::TxfmSize;
use crate::src::levels::TxfmType;
use crate::src::levels::V_ADST;
use crate::src::levels::V_DCT;
use crate::src::levels::V_FLIPADST;
use crate::src::levels::WHT_WHT;
use crate::src::owned_recon::ReconDst;

use console_core_geometry::Size;
use std::num::NonZeroU32;

#[derive(Clone, Copy)]
pub(super) struct CoefficientBounds {
  pub(super) lower: i32,
  pub(super) upper: i32,
}

pub(super) type Itx1dFn =
  fn(c: &mut [i32], stride: NonZeroU32, bounds: CoefficientBounds) -> Rav1dResult;

#[derive(Clone, Copy)]
enum DcShortcut {
  Available,
  Unavailable,
}

struct TransformPlan {
  size: Size<u32>,
  shift: u8,
  row: Itx1dFn,
  column: Itx1dFn,
  dc_shortcut: DcShortcut,
}

#[inline(never)]
fn inv_txfm_add<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  coeff: &mut [BD::Coef],
  eob: i32,
  plan: TransformPlan,
  bd: BD,
  tmp: &mut [i32],
) -> Rav1dResult {
  inv_txfm_add_body(dst, coeff, eob, plan, bd, tmp)
}

#[inline(always)]
fn inv_txfm_add_body<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  coeff: &mut [BD::Coef],
  eob: i32,
  plan: TransformPlan,
  bd: BD,
  tmp: &mut [i32],
) -> Rav1dResult {
  let Size { width, height } = plan.size;
  let bitdepth_max = bd.bitdepth_max().as_::<i32>();
  let col_stride = NonZeroU32::new(width).ok_or(Rav1dError::EINVAL)?;
  let shift = u32::from(plan.shift);
  let is_rect2 = width.wrapping_mul(2) == height || height.wrapping_mul(2) == width;
  let rnd = 1_i32.wrapping_shl(shift).wrapping_shr(1);
  let Ok(w) = console_core_number_conversion::index(width);
  let Ok(h) = console_core_number_conversion::index(height);
  let Ok(native_stride) =
    ::console_core_number_conversion::fixed_offset(match dst.pixel_stride::<BD>() {
      ::core::result::Result::Ok(native_stride) => native_stride,
    });
  let Ok(pxstride) = console_core_number_conversion::fixed_offset(
    match ::console_core_number_conversion::wrapped_offset(native_stride) {
      ::core::result::Result::Ok(native_position__) => native_position__,
    },
  );

  let dc_threshold = match plan.dc_shortcut {
    DcShortcut::Available => 1_i32,
    DcShortcut::Unavailable => 0_i32,
  };

  match ::core::primitive::bool::then_some(eob < dc_threshold, ()) {
    ::core::option::Option::Some(()) => {
      let coefficient = coeff.first_mut().ok_or(Rav1dError::EINVAL)?;
      let mut dc = (*coefficient).as_::<i32>();
      *coefficient = BD::Coef::from(0_i16);

      dc = match ::core::primitive::bool::then_some(is_rect2, ()) {
        ::core::option::Option::Some(()) => {
          dc.wrapping_mul(181).wrapping_add(128).wrapping_shr(8)
        }
        ::core::option::Option::None => dc,
      };

      dc = dc.wrapping_mul(181).wrapping_add(128).wrapping_shr(8);
      dc = dc.wrapping_add(rnd).wrapping_shr(shift);
      dc = dc
        .wrapping_mul(181)
        .wrapping_add(128)
        .wrapping_add(2048)
        .wrapping_shr(12);

      for y in 0..h {
        let Ok(row_offset) = console_core_number_conversion::wrapped_offset(y);
        let Ok(mut row) = dst.at(row_offset.wrapping_mul(
          match ::console_core_number_conversion::wrapped_offset(pxstride) {
            ::core::result::Result::Ok(native_position_) => native_position_,
          },
        ));
        let Ok(mut pixels) = row.slice_mut::<BD>(w);

        for pixel in pixels.iter_mut() {
          *pixel = bd.iclip_pixel((*pixel).as_::<i32>().wrapping_add(dc));
        }
      }

      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  let coefficient_height = NonZeroU32::new(height.min(32)).ok_or(Rav1dError::EINVAL)?;
  let active_height = coefficient_height.get();
  let active_width = width.min(32);
  let Ok(sh) = console_core_number_conversion::index(active_height);
  let active = active_width
    .checked_mul(active_height)
    .ok_or(Rav1dError::ERANGE)?;
  let area = width.checked_mul(height).ok_or(Rav1dError::ERANGE)?;
  let Ok(coefficient_count) = console_core_number_conversion::index(active);
  let Ok(scratch_count) = console_core_number_conversion::index(area);
  let coeff_2 = coeff
    .get_mut(..coefficient_count)
    .ok_or(Rav1dError::EINVAL)?;
  let tmp_2 = tmp.get_mut(..scratch_count).ok_or(Rav1dError::EINVAL)?;

  let (row_clip_min, col_clip_min) = match BD::BITDEPTH {
    8 => (i32::from(i16::MIN), i32::from(i16::MIN)),
    _ => (
      (!bitdepth_max).wrapping_shl(7),
      (!bitdepth_max).wrapping_shl(5),
    ),
  };
  let row_clip_max = !row_clip_min;
  let col_clip_max = !col_clip_min;

  for (y, row) in tmp_2.chunks_exact_mut(w.max(1)).take(sh).enumerate() {
    for (pixel, column) in row.iter_mut().zip(coeff_2.chunks_exact(sh.max(1))) {
      let coefficient = column.get(y).ok_or(Rav1dError::EINVAL)?;
      let sample = (*coefficient).as_::<i32>();
      *pixel = match ::core::primitive::bool::then_some(is_rect2, ()) {
        ::core::option::Option::Some(()) => {
          sample.wrapping_mul(181).wrapping_add(128).wrapping_shr(8)
        }
        ::core::option::Option::None => sample,
      };
    }

    (plan.row)(
      row,
      NonZeroU32::MIN,
      CoefficientBounds {
        lower: row_clip_min,
        upper: row_clip_max,
      },
    )?;
  }

  coeff_2.fill(BD::Coef::from(0_i16));

  for value in tmp_2.iter_mut().take(w.wrapping_mul(sh)) {
    *value = iclip(
      value.wrapping_add(rnd).wrapping_shr(shift),
      col_clip_min,
      col_clip_max,
    );
  }

  for x in 0..w {
    let column = tmp_2.get_mut(x..).ok_or(Rav1dError::EINVAL)?;
    (plan.column)(
      column,
      col_stride,
      CoefficientBounds {
        lower: col_clip_min,
        upper: col_clip_max,
      },
    )?;
  }

  for (y, source) in tmp_2.chunks_exact(w.max(1)).enumerate() {
    let Ok(row_offset) = console_core_number_conversion::wrapped_offset(y);
    let Ok(mut row) = dst.at(row_offset.wrapping_mul(
      match ::console_core_number_conversion::wrapped_offset(pxstride) {
        ::core::result::Result::Ok(native_position_) => native_position_,
      },
    ));
    let Ok(mut pixels) = row.slice_mut::<BD>(w);

    for (pixel, &coefficient) in pixels.iter_mut().zip(source) {
      let residual = coefficient.wrapping_add(8).wrapping_shr(4);
      *pixel = bd.iclip_pixel((*pixel).as_::<i32>().wrapping_add(residual));
    }
  }

  Ok(())
}

fn inv_txfm_add_rust<const W: usize, const H: usize, const TYPE: TxfmType, BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  coeff: &mut [BD::Coef],
  eob: i32,
  bd: BD,
) -> Rav1dResult {
  let shift = match (W, H) {
    (4, 4) => 0,
    (4, 8) => 0,
    (4, 16) => 1,
    (8, 4) => 0,
    (8, 8) => 1,
    (8, 16) => 1,
    (8, 32) => 2,
    (16, 4) => 1,
    (16, 8) => 1,
    (16, 16) => 2,
    (16, 32) => 1,
    (16, 64) => 2,
    (32, 8) => 2,
    (32, 16) => 1,
    (32, 32) => 2,
    (32, 64) => 1,
    (64, 16) => 2,
    (64, 32) => 1,
    (64, 64) => 2,
    _ => return Err(Rav1dError::EINVAL),
  };
  let dc_shortcut = match TYPE {
    DCT_DCT => DcShortcut::Available,
    _ => DcShortcut::Unavailable,
  };

  enum Type {
    Identity,
    Dct,
    Adst,
    FlipAdst,
  }

  use Type::*;

  let (second, first) = match TYPE {
    IDTX => (Identity, Identity),
    DCT_DCT => (Dct, Dct),
    ADST_DCT => (Adst, Dct),
    FLIPADST_DCT => (FlipAdst, Dct),
    H_DCT => (Identity, Dct),
    DCT_ADST => (Dct, Adst),
    ADST_ADST => (Adst, Adst),
    FLIPADST_ADST => (FlipAdst, Adst),
    DCT_FLIPADST => (Dct, FlipAdst),
    ADST_FLIPADST => (Adst, FlipAdst),
    FLIPADST_FLIPADST => (FlipAdst, FlipAdst),
    V_DCT => (Dct, Identity),
    H_ADST => (Identity, Adst),
    H_FLIPADST => (Identity, FlipAdst),
    V_ADST => (Adst, Identity),
    V_FLIPADST => (FlipAdst, Identity),
    WHT_WHT => {
      return match (W, H) {
        (4, 4) => inv_txfm_add_wht_wht_4x4_rust(dst, coeff, bd),
        (_, _) => Err(Rav1dError::EINVAL),
      };
    }
    _ => return Err(Rav1dError::EINVAL),
  };

  fn resolve_1d_fn(r#type: Type, n: u32) -> Rav1dResult<Itx1dFn> {
    let transform: Itx1dFn = match (r#type, n) {
      (Identity, 4) => rav1d_inv_identity4_1d_c,
      (Identity, 8) => rav1d_inv_identity8_1d_c,
      (Identity, 16) => rav1d_inv_identity16_1d_c,
      (Identity, 32) => rav1d_inv_identity32_1d_c,
      (Dct, 4) => rav1d_inv_dct4_1d_c,
      (Dct, 8) => rav1d_inv_dct8_1d_c,
      (Dct, 16) => rav1d_inv_dct16_1d_c,
      (Dct, 32) => rav1d_inv_dct32_1d_c,
      (Dct, 64) => rav1d_inv_dct64_1d_c,
      (Adst, 4) => inverse_adst4::<false>,
      (Adst, 8) => inverse_adst8::<false>,
      (Adst, 16) => inverse_adst16::<false>,
      (FlipAdst, 4) => inverse_adst4::<true>,
      (FlipAdst, 8) => inverse_adst8::<true>,
      (FlipAdst, 16) => inverse_adst16::<true>,
      _ => return Err(Rav1dError::EINVAL),
    };
    Ok(transform)
  }

  let width = u32::try_from(W).map_err(|_overflow| Rav1dError::ERANGE)?;
  let height = u32::try_from(H).map_err(|_overflow| Rav1dError::ERANGE)?;
  let row = resolve_1d_fn(first, width)?;
  let column = resolve_1d_fn(second, height)?;
  let plan = TransformPlan {
    size: Size { width, height },
    shift,
    row,
    column,
    dc_shortcut,
  };
  let mut tmp = [[0i32; W]; H];

  match (width, height, BD::BITDEPTH) {
    (8, 8, 8) => inv_txfm_add_body(dst, coeff, eob, plan, bd, tmp.as_flattened_mut()),
    (_, _, _) => inv_txfm_add(dst, coeff, eob, plan, bd, tmp.as_flattened_mut()),
  }
}

#[cfg(test)]
pub(crate) fn itxfm_add_scalar_fallback<BD: BitDepth>(
  size_ordinal: u8,
  tx_type: TxfmType,
  dst: &mut ReconDst<'_>,
  coeff: &mut [BD::Coef],
  eob: i32,
  bd: BD,
) -> Rav1dResult {
  let Ok(index) = console_core_number_conversion::index(size_ordinal);
  let tx_size_2 = TxfmSize::from_repr(index).ok_or(Rav1dError::EINVAL)?;
  check_transform::<BD>(tx_size_2, tx_type, dst, eob)?;
  add_scalar::<BD>(tx_size_2, tx_type, dst, coeff, eob, bd)
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "each dispatch arm uses the same call with its own compile-time dimensions and transform kind"
  )
)]
macro_rules! scalar_transform_call {
  ($w:literal, $h:literal, $ty:expr, ($dst:expr, $coeff:expr, $eob:expr, $bd:expr)) => {
    inv_txfm_add_rust::<$w, $h, { $ty }, _>($dst, $coeff, $eob, $bd)
  };
}
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "nine small transform dimensions share the sixteen compile-time transform kinds"
  )
)]
macro_rules! dispatch_type_16 {
  ($w:literal, $h:literal, $kind:expr, $arguments:tt) => {
    match $kind {
      DCT_DCT => scalar_transform_call!($w, $h, DCT_DCT, $arguments),
      IDTX => scalar_transform_call!($w, $h, IDTX, $arguments),
      DCT_ADST => scalar_transform_call!($w, $h, DCT_ADST, $arguments),
      ADST_DCT => scalar_transform_call!($w, $h, ADST_DCT, $arguments),
      ADST_ADST => scalar_transform_call!($w, $h, ADST_ADST, $arguments),
      DCT_FLIPADST => scalar_transform_call!($w, $h, DCT_FLIPADST, $arguments),
      FLIPADST_DCT => scalar_transform_call!($w, $h, FLIPADST_DCT, $arguments),
      FLIPADST_FLIPADST => scalar_transform_call!($w, $h, FLIPADST_FLIPADST, $arguments),
      ADST_FLIPADST => scalar_transform_call!($w, $h, ADST_FLIPADST, $arguments),
      FLIPADST_ADST => scalar_transform_call!($w, $h, FLIPADST_ADST, $arguments),
      H_DCT => scalar_transform_call!($w, $h, H_DCT, $arguments),
      V_DCT => scalar_transform_call!($w, $h, V_DCT, $arguments),
      H_ADST => scalar_transform_call!($w, $h, H_ADST, $arguments),
      V_ADST => scalar_transform_call!($w, $h, V_ADST, $arguments),
      H_FLIPADST => scalar_transform_call!($w, $h, H_FLIPADST, $arguments),
      V_FLIPADST => scalar_transform_call!($w, $h, V_FLIPADST, $arguments),
      _ => Err(Rav1dError::EINVAL),
    }
  };
}
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "five dimensions share the twelve kinds allowed when either length is 32 pixels"
  )
)]
macro_rules! dispatch_type_12 {
  ($w:literal, $h:literal, $kind:expr, $arguments:tt) => {
    match $kind {
      DCT_DCT => scalar_transform_call!($w, $h, DCT_DCT, $arguments),
      IDTX => scalar_transform_call!($w, $h, IDTX, $arguments),
      DCT_ADST => scalar_transform_call!($w, $h, DCT_ADST, $arguments),
      ADST_DCT => scalar_transform_call!($w, $h, ADST_DCT, $arguments),
      ADST_ADST => scalar_transform_call!($w, $h, ADST_ADST, $arguments),
      DCT_FLIPADST => scalar_transform_call!($w, $h, DCT_FLIPADST, $arguments),
      FLIPADST_DCT => scalar_transform_call!($w, $h, FLIPADST_DCT, $arguments),
      FLIPADST_FLIPADST => scalar_transform_call!($w, $h, FLIPADST_FLIPADST, $arguments),
      ADST_FLIPADST => scalar_transform_call!($w, $h, ADST_FLIPADST, $arguments),
      FLIPADST_ADST => scalar_transform_call!($w, $h, FLIPADST_ADST, $arguments),
      H_DCT => scalar_transform_call!($w, $h, H_DCT, $arguments),
      V_DCT => scalar_transform_call!($w, $h, V_DCT, $arguments),
      _ => Err(Rav1dError::EINVAL),
    }
  };
}
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "four large rectangular dimensions share the DCT and identity instantiations"
  )
)]
macro_rules! dispatch_type_2 {
  ($w:literal, $h:literal, $kind:expr, $arguments:tt) => {
    match $kind {
      DCT_DCT => scalar_transform_call!($w, $h, DCT_DCT, $arguments),
      IDTX => scalar_transform_call!($w, $h, IDTX, $arguments),
      _ => Err(Rav1dError::EINVAL),
    }
  };
}

fn add_scalar<BD: BitDepth>(
  tx_size: TxfmSize,
  tx_type: TxfmType,
  dst: &mut ReconDst<'_>,
  coeff: &mut [BD::Coef],
  eob: i32,
  bd: BD,
) -> Rav1dResult {
  use TxfmSize::*;

  match tx_size {
    S4x4 => match tx_type {
      WHT_WHT => scalar_transform_call!(4, 4, WHT_WHT, (dst, coeff, eob, bd)),
      _ => dispatch_type_16!(4, 4, tx_type, (dst, coeff, eob, bd)),
    },

    R4x8 => dispatch_type_16!(4, 8, tx_type, (dst, coeff, eob, bd)),
    R8x4 => dispatch_type_16!(8, 4, tx_type, (dst, coeff, eob, bd)),
    S8x8 => dispatch_type_16!(8, 8, tx_type, (dst, coeff, eob, bd)),
    R4x16 => dispatch_type_16!(4, 16, tx_type, (dst, coeff, eob, bd)),
    R16x4 => dispatch_type_16!(16, 4, tx_type, (dst, coeff, eob, bd)),
    R8x16 => dispatch_type_16!(8, 16, tx_type, (dst, coeff, eob, bd)),
    R16x8 => dispatch_type_16!(16, 8, tx_type, (dst, coeff, eob, bd)),
    S16x16 => dispatch_type_16!(16, 16, tx_type, (dst, coeff, eob, bd)),

    R8x32 => dispatch_type_12!(8, 32, tx_type, (dst, coeff, eob, bd)),
    R32x8 => dispatch_type_12!(32, 8, tx_type, (dst, coeff, eob, bd)),
    R16x32 => dispatch_type_12!(16, 32, tx_type, (dst, coeff, eob, bd)),
    R32x16 => dispatch_type_12!(32, 16, tx_type, (dst, coeff, eob, bd)),
    S32x32 => dispatch_type_12!(32, 32, tx_type, (dst, coeff, eob, bd)),

    R16x64 => dispatch_type_2!(16, 64, tx_type, (dst, coeff, eob, bd)),
    R64x16 => dispatch_type_2!(64, 16, tx_type, (dst, coeff, eob, bd)),
    R32x64 => dispatch_type_2!(32, 64, tx_type, (dst, coeff, eob, bd)),
    R64x32 => dispatch_type_2!(64, 32, tx_type, (dst, coeff, eob, bd)),

    S64x64 => match tx_type {
      DCT_DCT => scalar_transform_call!(64, 64, DCT_DCT, (dst, coeff, eob, bd)),
      _ => Err(Rav1dError::EINVAL),
    },
  }
}

fn check_transform<BD: BitDepth>(
  size: TxfmSize,
  kind: TxfmType,
  dst: &ReconDst<'_>,
  eob: i32,
) -> Rav1dResult {
  use TxfmSize::*;

  let supported = match size {
    S4x4 => kind <= WHT_WHT,
    R4x8 | R8x4 | S8x8 | R4x16 | R16x4 | R8x16 | R16x8 | S16x16 => kind < WHT_WHT,
    R8x32 | R32x8 | R16x32 | R32x16 | S32x32 => kind <= H_DCT,
    R16x64 | R64x16 | R32x64 | R64x32 => matches!(kind, DCT_DCT | IDTX),
    S64x64 => kind == DCT_DCT,
  };

  match ::core::primitive::bool::then_some(supported && eob >= 0, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(Rav1dError::EINVAL),
  }

  let Ok(extent) = size.extent();
  let Ok(bounds) = extent.map(u64::from);
  dst.check_block_bounds::<BD>(bounds)
}

pub struct Rav1dInvTxfmDSPContext;

impl Rav1dInvTxfmDSPContext {
  pub(crate) fn add<BD: BitDepth>(
    &self,
    size: TxfmSize,
    kind: TxfmType,
    dst: &mut ReconDst<'_>,
    coeff: &mut [BD::Coef],
    eob: i32,
    bd: BD,
  ) -> Rav1dResult {
    check_transform::<BD>(size, kind, dst, eob)?;
    let Ok(Size { width, height }) = size.extent();
    let full_coefficients = u64::from(width.min(32).wrapping_mul(height.min(32)));
    let Ok(available) = console_core_number_conversion::fixed_index(coeff.len());
    let Ok(ordinal) = size.ordinal();
    let Ok(tx_size) = console_core_number_conversion::index(ordinal);

    let simd_handled =
      match ::core::primitive::bool::then_some(available >= full_coefficients, ()) {
        ::core::option::Option::Some(()) => {
          #[cfg(not(target_arch = "x86_64"))]
          let ::core::result::Result::Ok(tx_type) =
            ::console_core_number_conversion::index::<::core::primitive::u8>(kind);

          #[cfg(target_arch = "x86_64")]
          {
            use crate::src::safe_simd::itx::{TransformDispatch, itxfm_add_dispatch};

            let Ok(dispatch) = itxfm_add_dispatch::<BD>(size, kind, dst, coeff, eob, bd);

            match dispatch {
              TransformDispatch::Applied => true,
              TransformDispatch::Fallback => false,
            }
          }

          #[cfg(target_arch = "aarch64")]
          {
            crate::src::safe_simd::itx_arm::itxfm_add_dispatch::<BD>(
              tx_size, tx_type, dst, coeff, eob, bd,
            )
          }
          #[cfg(target_arch = "wasm32")]
          {
            crate::src::safe_simd::itx_wasm::itxfm_add_dispatch::<BD>(
              tx_size, tx_type, dst, coeff, eob, bd,
            )
          }
          #[cfg(not(any(
            target_arch = "x86_64",
            target_arch = "aarch64",
            target_arch = "wasm32"
          )))]
          {
            let _ = (tx_size, tx_type);
            false
          }
        }
        ::core::option::Option::None => false,
      };
    let Ok(()) = crate::src::ablate::note_itx_shape(tx_size, BD::BITDEPTH, simd_handled);

    match ::core::primitive::bool::then_some(simd_handled, ()) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => add_scalar::<BD>(size, kind, dst, coeff, eob, bd),
    }
  }
}

fn inv_txfm_add_wht_wht_4x4_rust<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  coeff: &mut [BD::Coef],
  bd: BD,
) -> Rav1dResult {
  let Ok(side) = console_core_number_conversion::index(4u32);
  let stride = NonZeroU32::MIN.saturating_add(3);
  let coeff_2 = coeff.first_chunk_mut::<16>().ok_or(Rav1dError::EINVAL)?;
  let mut tmp = [0; 16];

  for (y, row) in tmp.as_chunks_mut::<4>().0.iter_mut().enumerate() {
    for (slot, column) in row.iter_mut().zip(coeff_2.as_chunks::<4>().0) {
      let sample = column.get(y).ok_or(Rav1dError::EINVAL)?;
      *slot = (*sample).as_::<i32>().wrapping_shr(2);
    }

    rav1d_inv_wht4_1d_c(row, NonZeroU32::MIN)?;
  }

  coeff_2.fill(BD::Coef::from(0_i16));

  for x in 0..side {
    let column = tmp.get_mut(x..).ok_or(Rav1dError::EINVAL)?;
    rav1d_inv_wht4_1d_c(column, stride)?;
  }

  let Ok(pxstride) =
    ::console_core_number_conversion::fixed_offset(match dst.pixel_stride::<BD>() {
      ::core::result::Result::Ok(pxstride) => pxstride,
    });

  for (y, source) in tmp.as_chunks::<4>().0.iter().enumerate() {
    let Ok(row_offset) = console_core_number_conversion::wrapped_offset(y);
    let Ok(mut row) = dst.at(row_offset.wrapping_mul(
      match ::console_core_number_conversion::wrapped_offset(pxstride) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    ));
    let Ok(mut pixels) = row.slice_mut::<BD>(side);

    for (pixel, &residual) in pixels.iter_mut().zip(source) {
      *pixel = bd.iclip_pixel((*pixel).as_::<i32>().wrapping_add(residual));
    }
  }

  Ok(())
}

impl Rav1dInvTxfmDSPContext {
  pub const fn default<BD: BitDepth>() -> Result<Self, console_core_never::Never> {
    Ok(Self)
  }

  #[inline(always)]
  const fn init<BD: BitDepth>(
    self,
    flags: CpuFlags,
    bpc: u8,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      {
        let _ = flags;
        let _ = bpc;
        self
      }
    })
  }

  pub const fn new<BD: BitDepth>(
    flags: CpuFlags,
    bpc: u8,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let Ok(rav1d_inv_txfm_dsp_context) = Self::default::<BD>();
      let Ok(init) = rav1d_inv_txfm_dsp_context.init::<BD>(flags, bpc);

      init
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::include::common::bitdepth::BitDepth8;
  use crate::include::dav1d::picture::Rav1dPictureDataComponent;
  use crate::src::with_offset::WithOffset;

  #[repr(align(64))]
  struct TestPixels([u8; 64]);

  #[test]
  fn inverse_transform_rejects_bad_input_before_writing() {
    let mut pixels = TestPixels([42; 64]);
    let picture = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut pixels.0, 4);
    let mut dst = ReconDst::Pic(WithOffset {
      data: &picture,
      offset: 0,
    });
    let mut coefficients = [64_i16; 15];
    let dsp = Rav1dInvTxfmDSPContext;
    let bd = BitDepth8::from_c(255);

    assert_eq!(
      dsp.add(TxfmSize::S4x4, WHT_WHT, &mut dst, &mut coefficients, 1, bd),
      Err(Rav1dError::EINVAL),
    );
    assert_eq!(
      dsp.add(TxfmSize::S4x4, DCT_DCT, &mut dst, &mut coefficients, 1, bd),
      Err(Rav1dError::EINVAL),
    );
    let mut full = [64_i16; 64];
    assert_eq!(
      dsp.add(TxfmSize::S8x8, WHT_WHT, &mut dst, &mut full, 1, bd),
      Err(Rav1dError::EINVAL),
    );
    let mut short_dst = ReconDst::Pic(WithOffset {
      data: &picture,
      offset: 53,
    });
    assert_eq!(
      dsp.add(TxfmSize::S8x8, DCT_DCT, &mut short_dst, &mut full, 0, bd),
      Err(Rav1dError::EINVAL),
    );
    assert_eq!(
      itxfm_add_scalar_fallback::<BitDepth8>(u8::MAX, DCT_DCT, &mut dst, &mut full, 0, bd),
      Err(Rav1dError::EINVAL),
    );
    assert_eq!(
      dsp.add(TxfmSize::S4x4, DCT_DCT, &mut dst, &mut full, -1, bd),
      Err(Rav1dError::EINVAL),
    );
    assert_eq!(coefficients, [64; 15]);
    assert_eq!(full, [64; 64]);
    assert_eq!(&*picture.slice::<BitDepth8, _>(..), &[42; 64]);
  }

  #[test]
  fn inverse_transform_accepts_a_single_dc_coefficient() {
    let mut pixels = TestPixels([42; 64]);
    let picture = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut pixels.0, 4);
    let mut dst = ReconDst::Pic(WithOffset {
      data: &picture,
      offset: 0,
    });
    let mut coefficients = [64_i16];
    let bd = BitDepth8::from_c(255);

    assert_eq!(
      Rav1dInvTxfmDSPContext.add(TxfmSize::S4x4, DCT_DCT, &mut dst, &mut coefficients, 0, bd),
      Ok(()),
    );
    assert_eq!(coefficients, [0]);
    assert_eq!(&*picture.slice::<BitDepth8, _>(..16), &[44; 16]);
    assert_eq!(&*picture.slice::<BitDepth8, _>(16..), &[42; 48]);
  }
}
