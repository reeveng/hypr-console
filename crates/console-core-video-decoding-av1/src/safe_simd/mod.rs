//! Safe SIMD implementations using Rust intrinsics
//!
//! This module provides safe Rust implementations of the SIMD functions
//! that are normally implemented in hand-written x86/ARM assembly.
//!
//! Used automatically when built without `--features asm`.

#![deny(unsafe_code)]

pub mod partial_simd;
pub mod pixel_access;

#[cfg(test)]
#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "parallel fixture cases must wait for the previous case to restore the process-wide SIMD probe before overriding it"
  )
)]
pub(crate) fn token_test_lock()
-> core::result::Result<std::sync::MutexGuard<'static, ()>, console_core_never::Never> {
  Ok({
    #[cfg_attr(
      dylint_lib = "explicit044_no_ambient_value",
      expect(
        explicit044_no_ambient_value,
        reason = "the test harness runs separate cases in parallel, so the lock lives with the process-wide SIMD probe override it guards"
      )
    )]
    #[cfg_attr(
      dylint_lib = "explicit068_no_global_mutable_state",
      expect(
        explicit068_no_global_mutable_state,
        reason = "a single lock serializes tests that temporarily override the process-wide SIMD probe"
      )
    )]
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    match LOCK.lock() {
      Ok(guard) => guard,
      Err(poisoned) => poisoned.into_inner(),
    }
  })
}

#[cfg(all(
  test,
  any(
    target_arch = "aarch64",
    all(target_arch = "x86_64", feature = "bitdepth_8", not(feature = "c-ffi"))
  )
))]
#[must_use]
pub(crate) fn aligned_plane<T: Copy + Default>(
  src: &[T],
) -> Result<rav1d_disjoint_mut::align::AlignedVec64<T>, console_core_never::Never> {
  let mut plane = rav1d_disjoint_mut::align::AlignedVec64::<T>::new();
  plane.resize(src.len(), T::default());

  for (destination, sample) in plane.iter_mut().zip(src) {
    *destination = *sample;
  }

  Ok(plane)
}

#[cfg(all(test, target_arch = "aarch64"))]
#[doc = "Advances the seeded xorshift64* used by AArch64 parity fixtures."]
pub(super) fn next_fixture_sample(state: &mut u64) -> Result<u64, console_core_never::Never> {
  let mut value = *state;
  value ^= value.wrapping_shr(12);
  value ^= value.wrapping_shl(25);
  value ^= value.wrapping_shr(27);
  *state = value;
  Ok(value.wrapping_mul(0x2545_F491_4F6C_DD1D))
}

pub mod mc;

#[cfg(target_arch = "aarch64")]
pub mod mc_arm;

#[cfg(target_arch = "aarch64")]
pub mod ipred_arm;

#[cfg(target_arch = "aarch64")]
pub mod cdef_arm;

#[cfg(target_arch = "aarch64")]
pub mod loopfilter_arm;

#[cfg(target_arch = "aarch64")]
pub mod looprestoration_arm;

#[cfg(target_arch = "aarch64")]
pub mod itx_arm;

#[cfg(target_arch = "aarch64")]
pub mod itx_arm_hbd;

#[cfg(target_arch = "aarch64")]
mod itx_arm_parity;

#[cfg(target_arch = "aarch64")]
mod mc_arm_prep_parity;

#[cfg(target_arch = "aarch64")]
pub mod itx_arm_neon_16x16;
#[cfg(target_arch = "aarch64")]
pub mod itx_arm_neon_32;
#[cfg(target_arch = "aarch64")]
pub mod itx_arm_neon_4x4;
#[cfg(target_arch = "aarch64")]
pub mod itx_arm_neon_64;
#[cfg(target_arch = "aarch64")]
pub mod itx_arm_neon_8x8;
#[cfg(target_arch = "aarch64")]
pub mod itx_arm_neon_common;
#[cfg(target_arch = "aarch64")]
pub mod itx_arm_neon_large_rect;
#[cfg(target_arch = "aarch64")]
pub mod itx_arm_neon_rect;
#[cfg(target_arch = "aarch64")]
pub mod itx_arm_neon_rect_large;
#[cfg(target_arch = "aarch64")]
pub mod itx_arm_neon_wht;

pub mod cdef;

#[cfg(target_arch = "wasm32")]
pub mod cdef_wasm;

#[cfg(target_arch = "wasm32")]
pub mod mc_wasm;

#[cfg(target_arch = "wasm32")]
pub mod itx_wasm;

#[cfg(target_arch = "x86_64")]
pub mod itx;

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
pub mod loopfilter;

#[cfg(target_arch = "x86_64")]
pub mod looprestoration;

#[cfg(target_arch = "x86_64")]
pub mod ipred;

#[cfg(target_arch = "x86_64")]
pub mod filmgrain;

#[cfg(target_arch = "x86_64")]
pub mod pal;

#[cfg(target_arch = "x86_64")]
pub mod refmvs;

#[cfg(target_arch = "aarch64")]
pub mod filmgrain_arm;

#[cfg(target_arch = "aarch64")]
pub mod refmvs_arm;
