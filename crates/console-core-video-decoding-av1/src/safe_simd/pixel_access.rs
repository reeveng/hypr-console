#![allow(
  clippy::undocumented_unsafe_blocks,
  reason = "the shared unchecked SIMD macros expand inside kernels whose validated tile or row layout establishes the pointer extent"
)]
//! Safe pixel access helpers and SIMD load/store macros for SIMD modules.
//!
//! All slice access functions use bounds-checked indexing unconditionally.
//! The `unchecked` feature only controls DisjointMut tracking overhead,
//! not slice bounds checking.
//!
//! # SIMD Load/Store Macros
//!
//! These macros provide a clean, unified API for SIMD memory access that
//! switches between safe and unchecked implementations:
//!
//! ```ignore
//! // x86_64 AVX2: load/store 256 bits (32 bytes)
//! let v = loadu_256!(&src[off..off+32], [u8; 32]);
//! storeu_256!(&mut dst[off..off+32], [u8; 32], v);
//!
//! // x86_64 SSE2: load/store 128 bits (16 bytes)
//! let v = loadu_128!(&src[off..off+16], [u8; 16]);
//! storeu_128!(&mut dst[off..off+16], [u8; 16], v);
//!
//! // Direct array reference (no conversion needed):
//! let v = loadu_256!(&arr);  // arr: [u8; 32]
//! storeu_256!(&mut arr, v);  // arr: [u8; 32]
//! ```
//!
//! When `unchecked` is **off** (default):
//! - Uses `archmage::intrinsics` for memory access (safe, bounds-checked)
//! - Compatible with `#![forbid(unsafe_code)]` in calling modules
//!
//! When `unchecked` is **on**:
//! - Uses raw `core::arch` intrinsics with pointer access (no bounds checks)
//! - `debug_assert!` still validates in debug builds

#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Ref};

pub struct FlexSlice<'a, T>(pub &'a [T]);

pub struct FlexSliceMut<'a, T>(pub &'a mut [T]);

impl<'a, T> FlexSlice<'a, T> {
  #[inline(always)]
  pub fn as_slice(&self) -> core::result::Result<&'a [T], console_core_never::Never> {
    Ok(self.0)
  }

  #[inline(always)]
  pub fn iter(
    &self,
  ) -> core::result::Result<core::slice::Iter<'a, T>, console_core_never::Never> {
    Ok(self.0.iter())
  }
}

impl<'a, T> FlexSliceMut<'a, T> {
  #[inline(always)]
  pub fn as_slice(&self) -> core::result::Result<&[T], console_core_never::Never> {
    Ok(self.0)
  }

  #[inline(always)]
  pub fn as_mut_slice(&mut self) -> core::result::Result<&mut [T], console_core_never::Never> {
    Ok(self.0)
  }

  #[inline(always)]
  pub fn iter(
    &self,
  ) -> core::result::Result<core::slice::Iter<'_, T>, console_core_never::Never> {
    Ok(self.0.iter())
  }

  #[inline(always)]
  pub fn iter_mut(
    &mut self,
  ) -> core::result::Result<core::slice::IterMut<'_, T>, console_core_never::Never> {
    Ok(self.0.iter_mut())
  }

  #[inline(always)]
  pub fn flex(&self) -> core::result::Result<FlexSlice<'_, T>, console_core_never::Never> {
    Ok(FlexSlice(self.0))
  }
}

impl<'a, T> core::ops::Deref for FlexSlice<'a, T> {
  type Target = [T];

  #[inline(always)]
  fn deref(&self) -> &[T] {
    self.0
  }
}

impl<'a, T> core::ops::Deref for FlexSliceMut<'a, T> {
  type Target = [T];

  #[inline(always)]
  fn deref(&self) -> &[T] {
    self.0
  }
}

impl<'a, T> core::ops::DerefMut for FlexSliceMut<'a, T> {
  #[inline(always)]
  fn deref_mut(&mut self) -> &mut [T] {
    self.0
  }
}

pub trait Flex<T> {
  fn flex(&self) -> FlexSlice<'_, T>;

  fn flex_mut(&mut self) -> FlexSliceMut<'_, T>;
}

impl<T> Flex<T> for [T] {
  #[inline(always)]
  fn flex(&self) -> FlexSlice<'_, T> {
    FlexSlice(self)
  }

  #[inline(always)]
  fn flex_mut(&mut self) -> FlexSliceMut<'_, T> {
    FlexSliceMut(self)
  }
}

#[inline(always)]
pub fn reinterpret_slice<Src: IntoBytes + Immutable, Dst: FromBytes + KnownLayout + Immutable>(
  src: &[Src],
) -> core::result::Result<Option<&[Dst]>, console_core_never::Never> {
  Ok(match Ref::<&[u8], [Dst]>::from_bytes(src.as_bytes()) {
    Ok(view) => Some(Ref::into_ref(view)),
    Err(_incompatible_layout) => None,
  })
}

#[inline(always)]
pub fn reinterpret_ref<Src: IntoBytes + Immutable, Dst: FromBytes + KnownLayout + Immutable>(
  src: &Src,
) -> core::result::Result<Option<&Dst>, console_core_never::Never> {
  Ok(match Ref::<&[u8], Dst>::from_bytes(src.as_bytes()) {
    Ok(view) => Some(Ref::into_ref(view)),
    Err(_incompatible_layout) => None,
  })
}

#[cfg(all(target_arch = "x86_64", not(feature = "unchecked")))]
#[inline]
#[cfg_attr(
  dylint_lib = "explicit004_no_panic",
  expect(
    explicit004_no_panic,
    reason = "the SIMD slice-form macro requires exactly one complete lane and returns the intrinsic vector or unit directly"
  )
)]
pub(crate) fn simd_lane_view<Width, Slice>(
  slice: Slice,
) -> Result<Width, console_core_never::Never>
where
  Slice: TryInto<Width, Error: core::fmt::Debug>,
{
  Ok(slice.try_into().unwrap())
}

#[cfg(target_arch = "x86_64")]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "retains the fixed-array and slice forms at the caller’s active SIMD target features and selects the checked or unchecked implementation at compile time"
  )
)]
macro_rules! loadu_256 {
  ($src:expr) => {{
    #[cfg(not(feature = "unchecked"))]
    {
      archmage::intrinsics::x86_64::_mm256_loadu_si256($src)
    }
    #[cfg(feature = "unchecked")]
    {
      #[allow(unsafe_code)]
      unsafe {
        core::arch::x86_64::_mm256_loadu_si256(core::ptr::from_ref($src).cast())
      }
    }
  }};
  ($slice:expr, $T:ty) => {{
    #[cfg(not(feature = "unchecked"))]
    {
      let Ok(lane) = $crate::src::safe_simd::pixel_access::simd_lane_view::<&$T, _>($slice);
      archmage::intrinsics::x86_64::_mm256_loadu_si256::<$T>(lane)
    }
    #[cfg(feature = "unchecked")]
    {
      let __s = $slice;
      debug_assert!(core::mem::size_of_val(__s) >= 32);
      #[allow(unsafe_code)]
      unsafe {
        core::arch::x86_64::_mm256_loadu_si256(__s.as_ptr().cast())
      }
    }
  }};
}
#[cfg(target_arch = "x86_64")]
pub(crate) use loadu_256;

#[cfg(target_arch = "x86_64")]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "retains the fixed-array and slice forms at the caller’s active SIMD target features and selects the checked or unchecked implementation at compile time"
  )
)]
macro_rules! storeu_256 {
  ($dst:expr, $val:expr) => {{
    #[cfg(not(feature = "unchecked"))]
    {
      archmage::intrinsics::x86_64::_mm256_storeu_si256($dst, $val)
    }
    #[cfg(feature = "unchecked")]
    {
      #[allow(unsafe_code)]
      unsafe {
        core::arch::x86_64::_mm256_storeu_si256(core::ptr::from_mut($dst).cast(), $val)
      }
    }
  }};
  ($slice:expr, $T:ty, $val:expr) => {{
    #[cfg(not(feature = "unchecked"))]
    {
      let Ok(lane) = $crate::src::safe_simd::pixel_access::simd_lane_view::<&mut $T, _>($slice);
      archmage::intrinsics::x86_64::_mm256_storeu_si256::<$T>(lane, $val)
    }
    #[cfg(feature = "unchecked")]
    {
      let __s = $slice;
      debug_assert!(core::mem::size_of_val(__s) >= 32);
      #[allow(unsafe_code)]
      unsafe {
        core::arch::x86_64::_mm256_storeu_si256(__s.as_mut_ptr().cast(), $val)
      }
    }
  }};
}
#[cfg(target_arch = "x86_64")]
pub(crate) use storeu_256;

#[cfg(target_arch = "x86_64")]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "retains the fixed-array and slice forms at the caller’s active SIMD target features and selects the checked or unchecked implementation at compile time"
  )
)]
macro_rules! loadu_512 {
  ($src:expr) => {{
    #[cfg(not(feature = "unchecked"))]
    {
      archmage::intrinsics::x86_64::_mm512_loadu_si512($src)
    }
    #[cfg(feature = "unchecked")]
    {
      #[allow(unsafe_code)]
      unsafe {
        core::arch::x86_64::_mm512_loadu_si512(core::ptr::from_ref($src).cast())
      }
    }
  }};
  ($slice:expr, $T:ty) => {{
    #[cfg(not(feature = "unchecked"))]
    {
      let Ok(lane) = $crate::src::safe_simd::pixel_access::simd_lane_view::<&$T, _>($slice);
      archmage::intrinsics::x86_64::_mm512_loadu_si512::<$T>(lane)
    }
    #[cfg(feature = "unchecked")]
    {
      let __s = $slice;
      debug_assert!(core::mem::size_of_val(__s) >= 64);
      #[allow(unsafe_code)]
      unsafe {
        core::arch::x86_64::_mm512_loadu_si512(__s.as_ptr().cast())
      }
    }
  }};
}
#[cfg(target_arch = "x86_64")]
pub(crate) use loadu_512;

#[cfg(target_arch = "x86_64")]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "retains the fixed-array and slice forms at the caller’s active SIMD target features and selects the checked or unchecked implementation at compile time"
  )
)]
macro_rules! storeu_512 {
  ($dst:expr, $val:expr) => {{
    #[cfg(not(feature = "unchecked"))]
    {
      archmage::intrinsics::x86_64::_mm512_storeu_si512($dst, $val)
    }
    #[cfg(feature = "unchecked")]
    {
      #[allow(unsafe_code)]
      unsafe {
        core::arch::x86_64::_mm512_storeu_si512(core::ptr::from_mut($dst).cast(), $val)
      }
    }
  }};
  ($slice:expr, $T:ty, $val:expr) => {{
    #[cfg(not(feature = "unchecked"))]
    {
      let Ok(lane) = $crate::src::safe_simd::pixel_access::simd_lane_view::<&mut $T, _>($slice);
      archmage::intrinsics::x86_64::_mm512_storeu_si512::<$T>(lane, $val)
    }
    #[cfg(feature = "unchecked")]
    {
      let __s = $slice;
      debug_assert!(core::mem::size_of_val(__s) >= 64);
      #[allow(unsafe_code)]
      unsafe {
        core::arch::x86_64::_mm512_storeu_si512(__s.as_mut_ptr().cast(), $val)
      }
    }
  }};
}
#[cfg(target_arch = "x86_64")]
pub(crate) use storeu_512;

#[cfg(target_arch = "x86_64")]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "retains the fixed-array and slice forms at the caller’s active SIMD target features and selects the checked or unchecked implementation at compile time"
  )
)]
macro_rules! loadu_128 {
  ($src:expr) => {{
    #[cfg(not(feature = "unchecked"))]
    {
      archmage::intrinsics::x86_64::_mm_loadu_si128($src)
    }
    #[cfg(feature = "unchecked")]
    {
      #[allow(unsafe_code)]
      unsafe {
        core::arch::x86_64::_mm_loadu_si128(core::ptr::from_ref($src).cast())
      }
    }
  }};
  ($slice:expr, $T:ty) => {{
    #[cfg(not(feature = "unchecked"))]
    {
      let Ok(lane) = $crate::src::safe_simd::pixel_access::simd_lane_view::<&$T, _>($slice);
      archmage::intrinsics::x86_64::_mm_loadu_si128::<$T>(lane)
    }
    #[cfg(feature = "unchecked")]
    {
      let __s = $slice;
      debug_assert!(core::mem::size_of_val(__s) >= 16);
      #[allow(unsafe_code)]
      unsafe {
        core::arch::x86_64::_mm_loadu_si128(__s.as_ptr().cast())
      }
    }
  }};
}
#[cfg(target_arch = "x86_64")]
pub(crate) use loadu_128;

#[cfg(target_arch = "x86_64")]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "keeps each eight-byte memory load at the caller’s active SIMD target features and selects the checked or unchecked implementation at compile time"
  )
)]
macro_rules! loadu_64 {
  ($src:expr) => {{
    #[cfg(not(feature = "unchecked"))]
    {
      archmage::intrinsics::x86_64::_mm_loadu_si64($src)
    }
    #[cfg(feature = "unchecked")]
    {
      #[allow(unsafe_code)]
      unsafe {
        core::arch::x86_64::_mm_loadu_si64(core::ptr::from_ref($src).cast())
      }
    }
  }};
}
#[cfg(target_arch = "x86_64")]
pub(crate) use loadu_64;

#[cfg(target_arch = "x86_64")]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "retains the fixed-array and slice forms at the caller’s active SIMD target features and selects the checked or unchecked implementation at compile time"
  )
)]
macro_rules! storeu_128 {
  ($dst:expr, $val:expr) => {{
    #[cfg(not(feature = "unchecked"))]
    {
      archmage::intrinsics::x86_64::_mm_storeu_si128($dst, $val)
    }
    #[cfg(feature = "unchecked")]
    {
      #[allow(unsafe_code)]
      unsafe {
        core::arch::x86_64::_mm_storeu_si128(core::ptr::from_mut($dst).cast(), $val)
      }
    }
  }};
  ($slice:expr, $T:ty, $val:expr) => {{
    #[cfg(not(feature = "unchecked"))]
    {
      let Ok(lane) = $crate::src::safe_simd::pixel_access::simd_lane_view::<&mut $T, _>($slice);
      archmage::intrinsics::x86_64::_mm_storeu_si128::<$T>(lane, $val)
    }
    #[cfg(feature = "unchecked")]
    {
      let __s = $slice;
      debug_assert!(core::mem::size_of_val(__s) >= 16);
      #[allow(unsafe_code)]
      unsafe {
        core::arch::x86_64::_mm_storeu_si128(__s.as_mut_ptr().cast(), $val)
      }
    }
  }};
}
#[cfg(target_arch = "x86_64")]
pub(crate) use storeu_128;

#[cfg(target_arch = "x86_64")]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "loads the four-byte low lane at the caller's active x86 target features, with one slice evaluation"
  )
)]
macro_rules! loadi32 {
  ($src:expr) => {{
    let bytes: &[u8] = $src;
    let val = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    core::arch::x86_64::_mm_cvtsi32_si128(val)
  }};
}
#[cfg(target_arch = "x86_64")]
pub(crate) use loadi32;

#[cfg(target_arch = "x86_64")]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "stores the four-byte low lane at the caller's active x86 target features, with one destination and vector evaluation"
  )
)]
macro_rules! storei32 {
  ($dst:expr, $val:expr) => {{
    let val = core::arch::x86_64::_mm_cvtsi128_si32($val);
    let bytes = val.to_le_bytes();
    let dst: &mut [u8] = $dst;
    dst[0] = bytes[0];
    dst[1] = bytes[1];
    dst[2] = bytes[2];
    dst[3] = bytes[3];
  }};
}
#[cfg(target_arch = "x86_64")]
pub(crate) use storei32;

#[cfg(target_arch = "x86_64")]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "loads the eight-byte low lane at the caller's active x86 target features, with one slice evaluation"
  )
)]
macro_rules! loadi64 {
  ($src:expr) => {{
    let bytes: &[u8] = $src;
    let lo = i64::from_le_bytes([
      bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]);
    core::arch::x86_64::_mm_set_epi64x(0, lo)
  }};
}
#[cfg(target_arch = "x86_64")]
pub(crate) use loadi64;

#[cfg(target_arch = "x86_64")]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "stores the eight-byte low lane at the caller's active x86 target features, with one destination and vector evaluation"
  )
)]
macro_rules! storei64 {
  ($dst:expr, $val:expr) => {{
    let val = core::arch::x86_64::_mm_cvtsi128_si64($val);
    let bytes = val.to_le_bytes();
    let dst: &mut [u8] = $dst;
    let lane = &mut dst[..8];

    for (target, sample) in lane.iter_mut().zip(bytes) {
      *target = sample;
    }
  }};
}
#[cfg(target_arch = "x86_64")]
pub(crate) use storei64;

#[cfg(target_arch = "wasm32")]
macro_rules! wasm_load_128 {
  ($src:expr) => {{ archmage::intrinsics::wasm32::v128_load($src) }};
  ($slice:expr, $T:ty) => {{ archmage::intrinsics::wasm32::v128_load::<$T>(($slice).try_into().unwrap()) }};
}
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm_load_128;

#[cfg(target_arch = "wasm32")]
macro_rules! wasm_store_128 {
  ($dst:expr, $val:expr) => {{ archmage::intrinsics::wasm32::v128_store($dst, $val) }};
  ($slice:expr, $T:ty, $val:expr) => {{ archmage::intrinsics::wasm32::v128_store::<$T>(($slice).try_into().unwrap(), $val) }};
}
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm_store_128;

#[cfg(target_arch = "wasm32")]
macro_rules! wasm_loadi32 {
  ($src:expr) => {{
    let bytes: &[u8] = $src;
    let val = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    core::arch::wasm32::i32x4(val, 0, 0, 0)
  }};
}
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm_loadi32;

#[cfg(target_arch = "wasm32")]
macro_rules! wasm_storei32 {
  ($dst:expr, $val:expr) => {{
    let val = core::arch::wasm32::i32x4_extract_lane::<0>($val);
    let bytes = val.to_le_bytes();
    let dst: &mut [u8] = $dst;
    dst[0] = bytes[0];
    dst[1] = bytes[1];
    dst[2] = bytes[2];
    dst[3] = bytes[3];
  }};
}
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm_storei32;

#[cfg(target_arch = "wasm32")]
macro_rules! wasm_loadi64 {
  ($src:expr) => {{
    let bytes: &[u8] = $src;
    let lo = i64::from_le_bytes([
      bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]);
    core::arch::wasm32::i64x2(lo, 0)
  }};
}
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm_loadi64;

#[cfg(target_arch = "wasm32")]
macro_rules! wasm_storei64 {
  ($dst:expr, $val:expr) => {{
    let val = core::arch::wasm32::i64x2_extract_lane::<0>($val);
    let bytes = val.to_le_bytes();
    let dst: &mut [u8] = $dst;
    dst[..8].copy_from_slice(&bytes);
  }};
}
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm_storei64;
