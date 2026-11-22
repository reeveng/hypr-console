use bytemuck::cast;

use super::{f32x8, u32x8};

cfg_if::cfg_if! {
    if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
        #[cfg(target_arch = "x86")]
        use core::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::*;

        #[derive(Clone, Copy, Debug)]
        #[repr(C, align(32))]
        pub struct i32x8(__m256i);
    } else {
        use super::i32x4;

        #[derive(Clone, Copy, Debug)]
        #[repr(C, align(32))]
        pub struct i32x8(pub i32x4, pub i32x4);
    }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(explicit058_contain_unsafe, reason = "all selected lane storage layouts have a valid zero bit pattern in every lane")
)]
unsafe impl bytemuck::Zeroable for i32x8 {}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(explicit058_contain_unsafe, reason = "each selected layout is one register, a fixed lane array, or adjacent equal-width lane blocks without padding")
)]
unsafe impl bytemuck::Pod for i32x8 {}

impl Default for i32x8 {
  fn default() -> Self {
    let Ok(splat) = Self::splat(0);

    splat
  }
}

impl i32x8 {
  pub fn splat(n: i32) -> core::result::Result<Self, console_core_never::Never> {
    Ok(cast([n, n, n, n, n, n, n, n]))
  }

  pub fn blend(self, t: Self, f: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX2 enabled.
                  unsafe { _mm256_blendv_epi8(f.0, t.0, self.0) },
              )
          } else {
              {
                  let Ok(low) = self.0.blend(t.0, f.0);
                  let Ok(high) = self.1.blend(t.1, f.1);

                  Self(low, high)
              }
          }
      }
    })
  }

  pub fn cmp_eq(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX2 enabled.
                  unsafe { _mm256_cmpeq_epi32(self.0, rhs.0) },
              )
          } else {
              {
                  let Ok(low) = self.0.cmp_eq(rhs.0);
                  let Ok(high) = self.1.cmp_eq(rhs.1);

                  Self(low, high)
              }
          }
      }
    })
  }

  pub fn cmp_gt(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX2 enabled.
                  unsafe { _mm256_cmpgt_epi32(self.0, rhs.0) },
              )
          } else {
              {
                  let Ok(low) = self.0.cmp_gt(rhs.0);
                  let Ok(high) = self.1.cmp_gt(rhs.1);

                  Self(low, high)
              }
          }
      }
    })
  }

  pub fn cmp_lt(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx2"))] {

              let Ok(greater) = self.cmp_gt(rhs);
              let Ok(all_bits) = Self::splat(-1);

              greater ^ all_bits
          } else {
              {
                  let Ok(low) = self.0.cmp_lt(rhs.0);
                  let Ok(high) = self.1.cmp_lt(rhs.1);

                  Self(low, high)
              }
          }
      }
    })
  }

  pub fn to_f32x8(self) -> core::result::Result<f32x8, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
              cast(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX2 enabled.
                  unsafe { _mm256_cvtepi32_ps(self.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              {
                  let Ok(low) = self.0.to_f32x4();
                  let Ok(high) = self.1.to_f32x4();

                  cast([low, high])
              }
          } else {
              {
                  let Ok(low) = self.0.to_f32x4();
                  let Ok(high) = self.1.to_f32x4();

                  f32x8(low, high)
              }
          }
      }
    })
  }

  pub fn to_u32x8_bitcast(self) -> core::result::Result<u32x8, console_core_never::Never> {
    Ok(bytemuck::cast(self))
  }

  pub fn to_f32x8_bitcast(self) -> core::result::Result<f32x8, console_core_never::Never> {
    Ok(bytemuck::cast(self))
  }
}

impl From<[i32; 8]> for i32x8 {
  fn from(v: [i32; 8]) -> Self {
    cast(v)
  }
}

impl From<i32x8> for [i32; 8] {
  fn from(v: i32x8) -> Self {
    cast(v)
  }
}

impl core::ops::Add for i32x8 {
  type Output = Self;

  fn add(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with AVX2 enabled.
                unsafe { _mm256_add_epi32(self.0, rhs.0) },
            )
        } else {
            Self(self.0 + rhs.0, self.1 + rhs.1)
        }
    }
  }
}

impl core::ops::BitAnd for i32x8 {
  type Output = Self;

  fn bitand(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with AVX2 enabled.
                unsafe { _mm256_and_si256(self.0, rhs.0) },
            )
        } else {
            Self(self.0 & rhs.0, self.1 & rhs.1)
        }
    }
  }
}

impl core::ops::Mul for i32x8 {
  type Output = Self;

  fn mul(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with AVX2 enabled.
                unsafe { _mm256_mullo_epi32(self.0, rhs.0) },
            )
        } else {
            Self(self.0 * rhs.0, self.1 * rhs.1)
        }
    }
  }
}

impl core::ops::BitOr for i32x8 {
  type Output = Self;

  #[inline]
  fn bitor(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with AVX2 enabled.
                unsafe { _mm256_or_si256(self.0, rhs.0) },
            )
        } else {
            Self(self.0 | rhs.0, self.1 | rhs.1)
        }
    }
  }
}

impl core::ops::BitXor for i32x8 {
  type Output = Self;

  #[inline]
  fn bitxor(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with AVX2 enabled.
                unsafe { _mm256_xor_si256(self.0, rhs.0) },
            )
        } else {
            Self(self.0 ^ rhs.0, self.1 ^ rhs.1)
        }
    }
  }
}
