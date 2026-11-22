//! Safe wrappers for partial (64-bit) SIMD load/store operations.
//!
//! These use the safe memory intrinsics re-exported by `archmage`.

#![forbid(unsafe_code)]

#[cfg(target_arch = "x86_64")]
pub use x86_64_aliases::*;

#[cfg(target_arch = "x86_64")]
mod x86_64_aliases {
  use core::arch::x86_64::__m128i;

  #[inline]
  #[target_feature(enable = "sse2")]
  pub fn mm_loadl_epi64(
    src: &[u8; 8],
  ) -> core::result::Result<__m128i, console_core_never::Never> {
    Ok(archmage::intrinsics::x86_64::_mm_loadu_si64(src))
  }

  #[inline]
  #[target_feature(enable = "sse2")]
  pub fn mm_storel_epi64(
    dst: &mut [u8; 8],
    val: __m128i,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      archmage::intrinsics::x86_64::_mm_storeu_si64(dst, val)
    };

    Ok(())
  }
}
