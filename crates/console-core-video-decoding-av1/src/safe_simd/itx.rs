//! Safe SIMD implementations for ITX (Inverse Transforms)
#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]
//!
//! ITX is the largest DSP module (~42k asm lines). Strategy:
//! 1. Implement full 2D transforms (not just 1D) for common sizes
//! 2. Process multiple rows/columns in parallel
//! 3. Use in-register transposition
//!
//! Most common transforms: DCT_DCT 4x4, 8x8, 16x16
//!
//! This module is ~23K LOC. To keep it navigable it is split across part
//! files included verbatim via `include!`, in definition order. Everything
//! stays in ONE module so all `macro_rules!` definitions and private
//! functions remain mutually visible exactly as if the file were unsplit —
//! no visibility changes, no macro re-export. `include!` is textual
//! concatenation, so the part files MUST be included in the original source
//! order (macros are defined before use, functions reference each other).

#[cfg(target_arch = "x86_64")]
use crate::src::{
  error::{Rav1dError, Rav1dResult},
  itx::{CoefficientBounds, Itx1dFn},
};

#[cfg(target_arch = "x86_64")]
use crate::src::itx_1d::{
  inv_dct64_1d_x86 as dct64_1d, rav1d_inv_dct4_1d_c as dct4_1d, rav1d_inv_dct8_1d_c as dct8_1d,
  rav1d_inv_dct16_1d_c as dct16_1d, rav1d_inv_dct32_1d_c as dct32_1d,
  rav1d_inv_identity4_1d_c as identity4_1d, rav1d_inv_identity8_1d_c as identity8_1d,
  rav1d_inv_identity16_1d_c as identity16_1d, rav1d_inv_identity32_1d_c as identity32_1d,
};
#[cfg(target_arch = "x86_64")]
use core::num::NonZeroU32;

#[cfg(target_arch = "x86_64")]
const STRIDE_1: NonZeroU32 = NonZeroU32::MIN;
#[cfg(target_arch = "x86_64")]
const STRIDE_4: NonZeroU32 = NonZeroU32::MIN.saturating_add(3);
#[cfg(target_arch = "x86_64")]
const STRIDE_8: NonZeroU32 = NonZeroU32::MIN.saturating_add(7);
#[cfg(target_arch = "x86_64")]
const STRIDE_16: NonZeroU32 = NonZeroU32::MIN.saturating_add(15);
#[cfg(target_arch = "x86_64")]
const STRIDE_32: NonZeroU32 = NonZeroU32::MIN.saturating_add(31);
#[cfg(target_arch = "x86_64")]
const STRIDE_64: NonZeroU32 = NonZeroU32::MIN.saturating_add(63);

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
enum RectangularScale {
  Apply,
  Skip,
}

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
enum AdstRowOrder {
  Normal,
  Reversed,
}

#[cfg(target_arch = "x86_64")]
impl RectangularScale {
  const fn for_transform(
    size: crate::src::levels::TxfmSize,
  ) -> Result<Self, console_core_never::Never> {
    use crate::src::levels::TxfmSize::*;

    Ok(match size {
      R4x8 | R8x4 | R8x16 | R16x8 | R16x32 | R32x16 | R32x64 | R64x32 => Self::Apply,
      S4x4 | S8x8 | S16x16 | S32x32 | S64x64 | R4x16 | R16x4 | R8x32 | R32x8 | R16x64
      | R64x16 => Self::Skip,
    })
  }
}

include!("../../include/safe_simd/itx/part01_macros_and_2d_transforms.rs");
include!("../../include/safe_simd/itx/part02_col_1d_avx512_pmaddwd.rs");
include!("../../include/safe_simd/itx/part03_adst16_and_small_rect.rs");
include!("../../include/safe_simd/itx/part04_rect_idtx_aspect.rs");
include!("../../include/safe_simd/itx/part05_tests.rs");
include!("../../include/safe_simd/itx/part06_adst_flipadst_vh.rs");
include!("../../include/safe_simd/itx/part07_32x32_64x64_dct.rs");
include!("../../include/safe_simd/itx/part08_rect_dct_adst_16bpc.rs");
include!("../../include/safe_simd/itx/part09_identity_hybrid_16bpc.rs");
include!("../../include/safe_simd/itx/part10_dispatch.rs");
include!("../../include/safe_simd/itx/part11_mixed_parity.rs");
