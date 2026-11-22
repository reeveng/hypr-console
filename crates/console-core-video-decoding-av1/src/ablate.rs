//! Per-kernel-family SIMD ablation switch (measurement only, `__ablate` feature).
//!
//! # Why this exists
//!
//! `CpuLevel::Scalar` / `rav1d_set_cpu_flags_mask(0)` does **not** disable the
//! safe-SIMD kernels on aarch64: every `safe_simd/*_arm.rs` dispatcher gates on
//! `archmage::Arm64::summon()` (a compile/run-time CPU token) and never reads
//! `rav1d_cpu_flags_mask`. So there was no way to ask "which kernel family is
//! responsible for this failing conformance vector?".
//!
//! This module adds one: a process-global bitmask of *disabled* families. A
//! disabled dispatcher returns `false` ("not handled") at its entry, so the
//! caller falls through to the generic scalar reference. Ablating family X and
//! re-running the corpus turns correlational evidence ("this kernel logs
//! mismatches") into causal evidence ("these N vectors' MD5s are wrong
//! *because of* X").
//!
//! # Cost when the feature is off
//!
//! [`is_off`] is `#[inline(always)]` and returns a `const false` when
//! `__ablate` is not enabled, so every dispatcher's guard folds away entirely.
//! This is compile-time-gated for the same reason `__simd_test` is: no
//! per-call atomic load on the hot path in a production build.
//!
//! # Scope
//!
//! Measurement infrastructure. Never enable `__ablate` in a shipped build —
//! ablating a family makes the decoder slower, not wrong (the scalar path is
//! the reference), but the switch is process-global and racy by design.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(explicit070_spaced_discriminants, reason = "public measurement consumers use the family numbers as indexes into the activity snapshot array")
)]
pub enum Family {
  Itx = 0,
  McPut = 1,
  McPrep = 2,
  McOther = 3,
  Cdef = 4,
  LoopFilter = 5,
  LoopRestoration = 6,
  IntraPred = 7,
  FilmGrain = 8,
}

impl Family {
  pub const ALL: &'static [Family] = &[
    Family::Itx,
    Family::McPut,
    Family::McPrep,
    Family::McOther,
    Family::Cdef,
    Family::LoopFilter,
    Family::LoopRestoration,
    Family::IntraPred,
    Family::FilmGrain,
  ];

  pub const fn name(self) -> core::result::Result<&'static str, console_core_never::Never> {
    Ok({
      match self {
        Family::Itx => "itx",
        Family::McPut => "mc_put",
        Family::McPrep => "mc_prep",
        Family::McOther => "mc_other",
        Family::Cdef => "cdef",
        Family::LoopFilter => "loopfilter",
        Family::LoopRestoration => "looprestoration",
        Family::IntraPred => "ipred",
        Family::FilmGrain => "filmgrain",
      }
    })
  }

  pub fn from_name(s: &str) -> core::result::Result<Option<Family>, console_core_never::Never> {
    Ok({
      Family::ALL.iter().copied().find(|f| {
        let Ok(name) = f.name();

        name == s
      })
    })
  }
}

pub const ENABLED: bool = false;

pub fn set_disabled(families: &[Family]) -> core::result::Result<(), console_core_never::Never> {
  {
    let _ = families;
  };

  Ok(())
}

#[inline(always)]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(explicit002_infallible_result, reason = "the public measurement dispatch predicate keeps the existing boolean return type")
)]
#[cfg_attr(
  dylint_lib = "explicit007_no_bool_return",
  expect(explicit007_no_bool_return, reason = "the public measurement dispatch predicate keeps the existing boolean return type")
)]
pub fn is_off(_f: Family) -> bool {
  false
}

#[inline(always)]
pub fn note(f: Family, units: u64) -> core::result::Result<(), console_core_never::Never> {
  {
    let _ = (f, units);
  };

  Ok(())
}

pub fn activity_snapshot() -> core::result::Result<[u64; 9], console_core_never::Never> {
  Ok([0u64; 9])
}

pub fn activity_reset() -> core::result::Result<(), console_core_never::Never> {
  {};

  Ok(())
}

#[inline(always)]
#[cfg_attr(
  dylint_lib = "explicit008_no_bool_param",
  expect(explicit008_no_bool_param, reason = "the public measurement hook keeps the boolean it receives from the transform dispatch predicates")
)]
pub fn note_itx_shape(
  tx_size: ::console_core_api_compatibility::NativeSize,
  bitdepth: u8,
  handled: bool,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let _ = (tx_size, bitdepth, handled);
  };

  Ok(())
}

pub fn itx_shape_report() -> core::result::Result<String, console_core_never::Never> {
  Ok(String::from(ITX_CENSUS_HEADER))
}

const ITX_CENSUS_HEADER: &str = "depth\tpath\tshape\tcalls\tcoeff_area\n";

pub fn itx_shape_reset() -> core::result::Result<(), console_core_never::Never> {
  {};

  Ok(())
}
