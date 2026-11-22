use super::{f32x8, i32x8};

cfg_if::cfg_if! {
    if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
        #[cfg(target_arch = "x86")]
        use core::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::*;

        use bytemuck::cast;

        #[derive(Clone, Copy, Debug)]
        #[repr(C, align(32))]
        pub struct u32x8(__m256i);
    } else {
        use super::u32x4;

        #[derive(Clone, Copy, Debug)]
        #[repr(C, align(32))]
        pub struct u32x8(u32x4, u32x4);
    }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(explicit058_contain_unsafe, reason = "all selected lane storage layouts have a valid zero bit pattern in every lane")
)]
unsafe impl bytemuck::Zeroable for u32x8 {}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(explicit058_contain_unsafe, reason = "each selected layout is one register, a fixed lane array, or adjacent equal-width lane blocks without padding")
)]
unsafe impl bytemuck::Pod for u32x8 {}

impl Default for u32x8 {
  fn default() -> Self {
    let Ok(splat) = Self::splat(0);

    splat
  }
}

impl u32x8 {
  pub fn splat(n: u32) -> core::result::Result<Self, console_core_never::Never> {
    Ok(bytemuck::cast([n, n, n, n, n, n, n, n]))
  }

  pub fn to_i32x8_bitcast(self) -> core::result::Result<i32x8, console_core_never::Never> {
    Ok(bytemuck::cast(self))
  }

  pub fn to_f32x8_bitcast(self) -> core::result::Result<f32x8, console_core_never::Never> {
    Ok(bytemuck::cast(self))
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
              Self(self.0.cmp_eq(rhs.0), self.1.cmp_eq(rhs.1))
          }
      }
    })
  }

  pub fn shl<const RHS: i32>(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
         if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
              let shift: __m128i = cast([({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u64>(RHS); numeric_value }), 0]);
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX2 enabled.
                  unsafe { _mm256_sll_epi32(self.0, shift) },
              )
          } else {
              Self(self.0.shl::<RHS>(), self.1.shl::<RHS>())
          }
      }
    })
  }

  pub fn shr<const RHS: i32>(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
              let shift: __m128i = cast([({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u64>(RHS); numeric_value }), 0]);
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX2 enabled.
                  unsafe { _mm256_srl_epi32(self.0, shift) },
              )
          } else {
              Self(self.0.shr::<RHS>(), self.1.shr::<RHS>())
          }
      }
    })
  }
}

impl core::ops::Not for u32x8 {
  type Output = Self;

  fn not(self) -> Self {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx2"))] {
            let all_bits = cast([u32::MAX; 8]);
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with AVX2 enabled.
                unsafe { _mm256_xor_si256(self.0, all_bits) },
            )
        } else {
            Self(!self.0, !self.1)
        }
    }
  }
}

impl core::ops::Add for u32x8 {
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

impl core::ops::BitAnd for u32x8 {
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

impl core::ops::BitOr for u32x8 {
  type Output = Self;

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
