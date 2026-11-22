use crate::math::GammaExponent;
use console_core_never::Never;

use bytemuck::cast;

use super::{i32x8, u32x8};

cfg_if::cfg_if! {
    if #[cfg(all(feature = "simd", target_feature = "avx"))] {
        #[cfg(target_arch = "x86")]
        use core::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::*;

        #[derive(Clone, Copy, Debug)]
        #[repr(C, align(32))]
        pub struct f32x8(__m256);
    } else {
        use super::f32x4;

        #[derive(Clone, Copy, Debug)]
        #[repr(C, align(32))]
        pub struct f32x8(pub f32x4, pub f32x4);
    }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "all selected lane storage layouts have a valid zero bit pattern in every lane"
  )
)]
unsafe impl bytemuck::Zeroable for f32x8 {}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "each selected layout is one register, a fixed lane array, or adjacent equal-width lane blocks without padding"
  )
)]
unsafe impl bytemuck::Pod for f32x8 {}

impl Default for f32x8 {
  fn default() -> Self {
    {
      let Ok(explicit_value) = Self::splat(0.0);
      explicit_value
    }
  }
}

impl f32x8 {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the generic SIMD broadcast preserves the units of its caller's lane value"
    )
  )]
  pub fn splat(n: f32) -> core::result::Result<Self, console_core_never::Never> {
    Ok(cast([n, n, n, n, n, n, n, n]))
  }

  pub fn floor(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              {
                  let Ok(low) = self.0.floor();
                  let Ok(high) = self.1.floor();

                  Self(low, high)
              }
          } else {
              let roundtrip: f32x8 = cast({ let Ok(explicit_value) = ({ let Ok(explicit_value) = self.trunc_int(); explicit_value }).to_f32x8(); explicit_value });
              roundtrip
                  - ({ let Ok(explicit_value) = ({ let Ok(explicit_value) = roundtrip
                      .cmp_gt(self); explicit_value })
                      .blend({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value }, f32x8::default()); explicit_value })
          }
      }
    })
  }

  pub fn fract(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      self
        - ({
          let Ok(explicit_value) = self.floor();
          explicit_value
        })
    })
  }

  pub fn normalize(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      {
        let Ok(explicit_value) = ({
          let Ok(explicit_value) = self.max(f32x8::default());
          explicit_value
        })
        .min({
          let Ok(explicit_value) = f32x8::splat(1.0);
          explicit_value
        });
        explicit_value
      }
    })
  }

  pub fn to_i32x8_bitcast(self) -> core::result::Result<i32x8, console_core_never::Never> {
    Ok(bytemuck::cast(self))
  }

  pub fn to_u32x8_bitcast(self) -> core::result::Result<u32x8, console_core_never::Never> {
    Ok(bytemuck::cast(self))
  }

  pub fn cmp_eq(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_cmp_ps(self.0, rhs.0, _CMP_EQ_OQ) },
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

  pub fn cmp_ne(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {

              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_cmp_ps(self.0, rhs.0, _CMP_NEQ_UQ) },
              )
          } else {
              {
                  let Ok(low) = self.0.cmp_ne(rhs.0);
                  let Ok(high) = self.1.cmp_ne(rhs.1);

                  Self(low, high)
              }
          }
      }
    })
  }

  pub fn cmp_ge(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_cmp_ps(self.0, rhs.0, _CMP_GE_OQ) },
              )
          } else {
              {
                  let Ok(low) = self.0.cmp_ge(rhs.0);
                  let Ok(high) = self.1.cmp_ge(rhs.1);

                  Self(low, high)
              }
          }
      }
    })
  }

  pub fn cmp_gt(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_cmp_ps(self.0, rhs.0, _CMP_GT_OQ) },
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

  pub fn cmp_le(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_cmp_ps(self.0, rhs.0, _CMP_LE_OQ) },
              )
          } else {
              {
                  let Ok(low) = self.0.cmp_le(rhs.0);
                  let Ok(high) = self.1.cmp_le(rhs.1);

                  Self(low, high)
              }
          }
      }
    })
  }

  pub fn cmp_lt(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_cmp_ps(self.0, rhs.0, _CMP_LT_OQ) },
              )
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

  #[inline]
  pub fn blend(self, t: Self, f: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_blendv_ps(f.0, t.0, self.0) },
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

  pub fn abs(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let non_sign_bits = {
        let Ok(explicit_value) = f32x8::splat(f32::from_bits({
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(i32::MAX);
          numeric_value
        }));
        explicit_value
      };
      self & non_sign_bits
    })
  }

  pub fn max(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_max_ps(self.0, rhs.0) },
              )
          } else {
              {
                  let Ok(low) = self.0.max(rhs.0);
                  let Ok(high) = self.1.max(rhs.1);

                  Self(low, high)
              }
          }
      }
    })
  }

  pub fn min(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_min_ps(self.0, rhs.0) },
              )
          } else {
              {
                  let Ok(low) = self.0.min(rhs.0);
                  let Ok(high) = self.1.min(rhs.1);

                  Self(low, high)
              }
          }
      }
    })
  }

  pub fn is_finite(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let shifted_exp_mask = {
        let Ok(explicit_value) = u32x8::splat(0xFF000000);
        explicit_value
      };
      let u: u32x8 = cast(self);
      let shift_u = {
        let Ok(explicit_value) = u.shl::<1>();
        explicit_value
      };
      let out = !({
        let Ok(explicit_value) = (shift_u & shifted_exp_mask).cmp_eq(shifted_exp_mask);
        explicit_value
      });
      cast(out)
    })
  }

  pub fn round(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_round_ps(self.0, _MM_FROUND_NO_EXC | _MM_FROUND_TO_NEAREST_INT) },
              )
          } else {
              {
                  let Ok(low) = self.0.round();
                  let Ok(high) = self.1.round();

                  Self(low, high)
              }
          }
      }
    })
  }

  pub fn round_int(self) -> core::result::Result<i32x8, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              cast(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_cvtps_epi32(self.0) },
              )
          } else {
              {
                  let Ok(low) = self.0.round_int();
                  let Ok(high) = self.1.round_int();

                  i32x8(low, high)
              }
          }
      }
    })
  }

  pub fn trunc_int(self) -> core::result::Result<i32x8, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              cast(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_cvttps_epi32(self.0) },
              )
          } else {
              {
                  let Ok(low) = self.0.trunc_int();
                  let Ok(high) = self.1.trunc_int();

                  i32x8(low, high)
              }
          }
      }
    })
  }

  pub fn recip_fast(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_rcp_ps(self.0) },
              )
          } else {
              {
                  let Ok(low) = self.0.recip_fast();
                  let Ok(high) = self.1.recip_fast();

                  Self(low, high)
              }
          }
      }
    })
  }

  pub fn recip_sqrt(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_rsqrt_ps(self.0) },
              )
          } else {
              {
                  let Ok(low) = self.0.recip_sqrt();
                  let Ok(high) = self.1.recip_sqrt();

                  Self(low, high)
              }
          }
      }
    })
  }

  pub fn sqrt(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "avx"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with AVX enabled.
                  unsafe { _mm256_sqrt_ps(self.0) },
              )
          } else {
              {
                  let Ok(low) = self.0.sqrt();
                  let Ok(high) = self.1.sqrt();

                  Self(low, high)
              }
          }
      }
    })
  }

  pub fn powf(self, GammaExponent(exponent): GammaExponent) -> Result<Self, Never> {
    let Ok(logarithm) = self.approximate_log2();
    let Ok(power) = Self::splat(exponent);
    let Ok(value) = (logarithm * power).approximate_exp2();
    let Ok(zero) = Self::splat(0.0);
    let Ok(one) = Self::splat(1.0);
    let Ok(is_zero) = self.cmp_eq(zero);
    let Ok(is_one) = self.cmp_eq(one);

    (is_zero | is_one).blend(self, value)
  }

  fn approximate_log2(self) -> Result<Self, Never> {
    let Ok(encoded) = self.to_i32x8_bitcast();
    let Ok(exponent_scale) = Self::splat(1.0 / 8_388_608.0);
    let Ok(integer_bits) = encoded.to_f32x8();
    let exponent = integer_bits * exponent_scale;
    let Ok(bits) = self.to_u32x8_bitcast();
    let Ok(significand_bits) = u32x8::splat(0x007f_ffff);
    let Ok(normalized_exponent) = u32x8::splat(0x3f00_0000);
    let Ok(significand) = ((bits & significand_bits) | normalized_exponent).to_f32x8_bitcast();
    let Ok(bias) = Self::splat(124.225_52);
    let Ok(linear) = Self::splat(1.498_030_3);
    let Ok(numerator) = Self::splat(1.725_88);
    let Ok(denominator) = Self::splat(0.352_088_72);

    Ok(exponent - bias - linear * significand - numerator / (denominator + significand))
  }

  fn approximate_exp2(self) -> Result<Self, Never> {
    let Ok(floor) = self.floor();
    let fraction = self - floor;
    let Ok(bias) = Self::splat(121.274_055);
    let Ok(linear) = Self::splat(1.490_129_1);
    let Ok(numerator) = Self::splat(27.728_024);
    let Ok(denominator) = Self::splat(4.842_525_5);
    let Ok(exponent_scale) = Self::splat(8_388_608.0);
    let approx =
      (self + bias - fraction * linear + numerator / (denominator - fraction)) * exponent_scale;
    let Ok(infinity_bits) =
      console_core_number_conversion::nearest_f32_from_u64(u64::from(f32::INFINITY.to_bits()));
    let Ok(maximum) = Self::splat(infinity_bits);
    let Ok(nonnegative) = approx.max(Self::default());
    let Ok(limited) = nonnegative.min(maximum);
    let Ok(rounded) = limited.round_int();

    rounded.to_f32x8_bitcast()
  }
}

impl From<[f32; 8]> for f32x8 {
  fn from(v: [f32; 8]) -> Self {
    cast(v)
  }
}

impl From<f32x8> for [f32; 8] {
  fn from(v: f32x8) -> Self {
    cast(v)
  }
}

impl core::ops::Add for f32x8 {
  type Output = Self;

  fn add(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with AVX enabled.
                unsafe { _mm256_add_ps(self.0, rhs.0) },
            )
        } else {
            Self(self.0 + rhs.0, self.1 + rhs.1)
        }
    }
  }
}

impl core::ops::AddAssign for f32x8 {
  fn add_assign(&mut self, rhs: f32x8) {
    *self = *self + rhs;
  }
}

impl core::ops::Sub for f32x8 {
  type Output = Self;

  fn sub(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with AVX enabled.
                unsafe { _mm256_sub_ps(self.0, rhs.0) },
            )
        } else {
            Self(self.0 - rhs.0, self.1 - rhs.1)
        }
    }
  }
}

impl core::ops::Mul for f32x8 {
  type Output = Self;

  fn mul(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with AVX enabled.
                unsafe { _mm256_mul_ps(self.0, rhs.0) },
            )
        } else {
            Self(self.0 * rhs.0, self.1 * rhs.1)
        }
    }
  }
}

impl core::ops::MulAssign for f32x8 {
  fn mul_assign(&mut self, rhs: f32x8) {
    *self = *self * rhs;
  }
}

impl core::ops::Div for f32x8 {
  type Output = Self;

  fn div(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with AVX enabled.
                unsafe { _mm256_div_ps(self.0, rhs.0) },
            )
        } else {
            Self(self.0 / rhs.0, self.1 / rhs.1)
        }
    }
  }
}

impl core::ops::BitAnd for f32x8 {
  type Output = Self;

  #[inline(always)]
  fn bitand(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with AVX enabled.
                unsafe { _mm256_and_ps(self.0, rhs.0) },
            )
        } else {
            Self(self.0 & rhs.0, self.1 & rhs.1)
        }
    }
  }
}

impl core::ops::BitOr for f32x8 {
  type Output = Self;

  #[inline(always)]
  fn bitor(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with AVX enabled.
                unsafe { _mm256_or_ps(self.0, rhs.0) },
            )
        } else {
            Self(self.0 | rhs.0, self.1 | rhs.1)
        }
    }
  }
}

impl core::ops::BitXor for f32x8 {
  type Output = Self;

  #[inline(always)]
  fn bitxor(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with AVX enabled.
                unsafe { _mm256_xor_ps(self.0, rhs.0) },
            )
        } else {
            Self(self.0 ^ rhs.0, self.1 ^ rhs.1)
        }
    }
  }
}

impl core::ops::Neg for f32x8 {
  type Output = Self;

  fn neg(self) -> Self {
    Self::default() - self
  }
}

impl core::ops::Not for f32x8 {
  type Output = Self;

  fn not(self) -> Self {
    let Ok(all_bits) = Self::splat(f32::from_bits(u32::MAX));

    self ^ all_bits
  }
}

impl core::cmp::PartialEq for f32x8 {
  fn eq(&self, rhs: &Self) -> bool {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "avx"))] {
            #[cfg_attr(
              dylint_lib = "explicit058_contain_unsafe",
              expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
            )]
            // SAFETY: this branch is compiled with AVX enabled.
            let mask = unsafe { _mm256_cmp_ps(self.0, rhs.0, _CMP_EQ_OQ) };

            #[cfg_attr(
              dylint_lib = "explicit058_contain_unsafe",
              expect(explicit058_contain_unsafe, reason = "the compile-time branch requires AVX and the intrinsic takes only owned vector registers")
            )]
            // SAFETY: this branch is compiled with AVX enabled.
            unsafe { _mm256_movemask_ps(mask) == 0b1111_1111 }
        } else {
            self.0 == rhs.0 && self.1 == rhs.1
        }
    }
  }
}
