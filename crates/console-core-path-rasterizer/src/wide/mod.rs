mod f32x16_t;
mod f32x4_t;
mod f32x8_t;
mod i32x4_t;
mod i32x8_t;
mod u16x16_t;
#[cfg(not(all(feature = "simd", target_feature = "avx2")))]
mod u32x4_t;
mod u32x8_t;

pub use f32x4_t::f32x4;
pub use f32x8_t::f32x8;
pub use f32x16_t::f32x16;
pub use i32x4_t::i32x4;
pub use i32x8_t::i32x8;
pub use u16x16_t::u16x16;
#[cfg(not(all(feature = "simd", target_feature = "avx2")))]
pub use u32x4_t::u32x4;
pub use u32x8_t::u32x8;

#[cfg(not(any(
  all(feature = "simd", target_feature = "sse4.1"),
  all(feature = "simd", target_feature = "relaxed-simd"),
  all(feature = "simd", target_feature = "simd128"),
  all(feature = "simd", target_arch = "aarch64", target_feature = "neon"),
)))]
#[inline]
pub fn generic_bit_blend<T>(
  mask: T,
  y: T,
  n: T,
) -> core::result::Result<T, console_core_never::Never>
where
  T: Copy + core::ops::BitXor<Output = T> + core::ops::BitAnd<Output = T>,
{
  Ok(n ^ ((n ^ y) & mask))
}
