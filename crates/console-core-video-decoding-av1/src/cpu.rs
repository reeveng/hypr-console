#![forbid(unsafe_code)]
use crate::src::const_fn::const_for;
use std::ffi::c_uint;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "bitflags generates its primitive capability query methods"
  )
)]
mod capabilities {
  use bitflags::bitflags;
  use std::ffi::c_uint;

  #[cfg(not(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "arm",
    target_arch = "aarch64",
    target_arch = "riscv32",
    target_arch = "riscv64",
  )))]
  bitflags! {
      #[derive(Clone, Copy)]
      pub struct CpuFlags: c_uint {}
  }

  #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
  bitflags! {
      #[derive(Clone, Copy)]
      pub struct CpuFlags: c_uint {
          const SSE2 = 1 << 0;
          const SSSE3 = 1 << 1;
          const SSE41 = 1 << 2;
          const AVX2 = 1 << 3;

          const AVX512ICL = 1 << 4;

          const SLOW_GATHER = 1 << 5;
      }
  }

  #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
  bitflags! {
      #[derive(Clone, Copy)]
      pub struct CpuFlags: c_uint {
          const NEON = 1 << 0;
          const DOTPROD = 1 << 1;
          const I8MM = 1 << 2;
          const SVE = 1 << 3;
          const SVE2 = 1 << 4;
      }
  }

  #[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
  bitflags! {
      #[derive(Clone, Copy)]
      pub struct CpuFlags: c_uint {
          const V = 1 << 0;
      }
  }
}

pub(crate) use capabilities::CpuFlags;

impl CpuFlags {
  pub const fn compile_time_detect() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let individual_flags = [
        #[cfg(target_feature = "sse2")]
        CpuFlags::SSE2,
        #[cfg(target_feature = "sse3")]
        CpuFlags::SSSE3,
        #[cfg(target_feature = "sse4.1")]
        CpuFlags::SSE41,
        #[cfg(target_feature = "avx2")]
        CpuFlags::AVX2,
        #[cfg(all(
          target_feature = "avx512f",
          target_feature = "avx512cd",
          target_feature = "avx512bw",
          target_feature = "avx512dq",
          target_feature = "avx512vl",
          target_feature = "avx512vnni",
          target_feature = "avx512ifma",
          target_feature = "avx512vbmi",
          target_feature = "avx512vbmi2",
          target_feature = "avx512vpopcntdq",
          target_feature = "avx512bitalg",
          target_feature = "gfni",
          target_feature = "vaes",
          target_feature = "vpclmulqdq",
        ))]
        CpuFlags::AVX512ICL,
        #[cfg(target_feature = "neon")]
        CpuFlags::NEON,
        #[cfg(target_feature = "i8mm")]
        CpuFlags::I8MM,
        #[cfg(target_feature = "dotprod")]
        CpuFlags::DOTPROD,
        #[cfg(target_feature = "sve")]
        CpuFlags::SVE,
        #[cfg(target_feature = "sve2")]
        CpuFlags::SVE2,
        #[cfg(target_feature = "v")]
        CpuFlags::V,
      ];

      let mut combined_flags = Self::empty();
      const_for!(i in 0..individual_flags.len() => {
          combined_flags = combined_flags.union(individual_flags[i]);
      });
      combined_flags
    })
  }

  pub fn run_time_detect() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let mut flags = Self::empty();

      #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
      {
        flags.set(Self::SSE2, is_x86_feature_detected!("sse2"));
        flags.set(Self::SSSE3, is_x86_feature_detected!("ssse3"));
        flags.set(Self::SSE41, is_x86_feature_detected!("sse4.1"));
        flags.set(Self::AVX2, is_x86_feature_detected!("avx2"));

        flags.set(
          Self::AVX512ICL,
          is_x86_feature_detected!("avx512f")
            && is_x86_feature_detected!("avx512cd")
            && is_x86_feature_detected!("avx512bw")
            && is_x86_feature_detected!("avx512dq")
            && is_x86_feature_detected!("avx512vl")
            && is_x86_feature_detected!("avx512vnni")
            && is_x86_feature_detected!("avx512ifma")
            && is_x86_feature_detected!("avx512vbmi")
            && is_x86_feature_detected!("avx512vbmi2")
            && is_x86_feature_detected!("avx512vpopcntdq")
            && is_x86_feature_detected!("avx512bitalg")
            && is_x86_feature_detected!("gfni")
            && is_x86_feature_detected!("vaes")
            && is_x86_feature_detected!("vpclmulqdq"),
        );
      }

      #[cfg(target_arch = "arm")]
      {
        flags.set(Self::NEON, std::arch::is_arm_feature_detected!("neon"));
        flags.set(
          Self::DOTPROD,
          std::arch::is_arm_feature_detected!("dotprod"),
        );
        flags.set(Self::I8MM, std::arch::is_arm_feature_detected!("i8mm"));
      }

      #[cfg(target_arch = "aarch64")]
      {
        flags.set(Self::NEON, std::arch::is_aarch64_feature_detected!("neon"));
        flags.set(
          Self::DOTPROD,
          std::arch::is_aarch64_feature_detected!("dotprod"),
        );
        flags.set(Self::I8MM, std::arch::is_aarch64_feature_detected!("i8mm"));
        flags.set(Self::SVE, std::arch::is_aarch64_feature_detected!("sve"));
        flags.set(Self::SVE2, std::arch::is_aarch64_feature_detected!("sve2"));
      }

      #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
      flags.set(Self::V, std::arch::is_riscv_feature_detected!("v"));

      flags
    })
  }
}

fn detected_cpu_flags() -> Result<CpuFlags, console_core_never::Never> {
  #[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    expect(
      explicit044_no_ambient_value,
      reason = "SIMD detection runs once for the current processor, and every subsequent decoder reads the same immutable capability set"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit068_no_global_mutable_state",
    expect(
      explicit068_no_global_mutable_state,
      reason = "SIMD detection runs once for the current processor, and every subsequent decoder reads the same immutable capability set"
    )
  )]
  static DETECTED: std::sync::OnceLock<CpuFlags> = std::sync::OnceLock::new();

  Ok(*DETECTED.get_or_init(|| {
    let Ok(flags) = CpuFlags::run_time_detect();
    flags
  }))
}

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  expect(
    explicit044_no_ambient_value,
    reason = "the exported dav1d control sets a process-wide mask and the atomic value publishes it to every decoder"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "the exported dav1d control sets a process-wide mask and the atomic value publishes it to every decoder"
  )
)]
static rav1d_cpu_flags_mask: AtomicU32 = AtomicU32::new(!0);

#[inline(always)]
pub(crate) fn rav1d_get_cpu_flags() -> core::result::Result<CpuFlags, console_core_never::Never> {
  let mask = rav1d_cpu_flags_mask.load(Ordering::SeqCst);
  flags_for_mask(mask)
}

pub(crate) fn flags_for_mask(mask: u32) -> Result<CpuFlags, console_core_never::Never> {
  let Ok(detected) = detected_cpu_flags();
  let Ok(compiled) = CpuFlags::compile_time_detect();
  let flags = detected.bits() | compiled.bits();
  Ok(CpuFlags::from_bits_truncate(flags & mask))
}

#[cfg(target_arch = "x86_64")]
enum SimdAvailability {
  Available,
  Unavailable,
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn simd_availability(flag: CpuFlags) -> Result<SimdAvailability, console_core_never::Never> {
  let Ok(flags) = rav1d_get_cpu_flags();

  Ok(match flag.difference(flags).bits() {
    0 => SimdAvailability::Available,
    _ => SimdAvailability::Unavailable,
  })
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(crate) fn summon_avx2()
-> Result<Option<archmage::prelude::Desktop64>, console_core_never::Never> {
  use archmage::SimdToken as _;

  let Ok(availability) = simd_availability(CpuFlags::AVX2);

  Ok(match availability {
    SimdAvailability::Available => archmage::prelude::Desktop64::summon(),
    SimdAvailability::Unavailable => None,
  })
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(crate) fn summon_avx512()
-> Result<Option<archmage::prelude::Server64>, console_core_never::Never> {
  use archmage::SimdToken as _;

  let Ok(availability) = simd_availability(CpuFlags::AVX512ICL);

  Ok(match availability {
    SimdAvailability::Available => archmage::prelude::Server64::summon(),
    SimdAvailability::Unavailable => None,
  })
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(crate) fn summon_avx512x() -> Result<Option<archmage::X64V4xToken>, console_core_never::Never>
{
  use archmage::SimdToken as _;

  let Ok(availability) = simd_availability(CpuFlags::AVX512ICL);

  Ok(match availability {
    SimdAvailability::Available => archmage::X64V4xToken::summon(),
    SimdAvailability::Unavailable => None,
  })
}

#[cfg(target_arch = "wasm32")]
#[inline(always)]
pub(crate) fn summon_wasm128() -> Option<archmage::Wasm128Token> {
  use archmage::SimdToken as _;
  archmage::Wasm128Token::summon()
}

#[cold]
pub(crate) fn rav1d_init_cpu() -> core::result::Result<(), console_core_never::Never> {
  let Ok(_detected) = detected_cpu_flags();

  Ok(())
}

#[cold]
pub fn rav1d_set_cpu_flags_mask(
  mask: c_uint,
) -> core::result::Result<(), console_core_never::Never> {
  {
    rav1d_cpu_flags_mask.store(mask, Ordering::SeqCst);
  };

  Ok(())
}
