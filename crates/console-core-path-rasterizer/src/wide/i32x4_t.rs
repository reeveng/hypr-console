use bytemuck::cast;

use super::f32x4;

cfg_if::cfg_if! {
    if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
        #[cfg(target_arch = "x86")]
        use core::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::*;

        #[derive(Clone, Copy, Debug)]
        #[repr(C, align(16))]
        pub struct i32x4(pub __m128i);
    } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
        use core::arch::wasm32::*;

        #[derive(Clone, Copy, Debug)]
        #[repr(C, align(16))]
        pub struct i32x4(pub v128);
    } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
        use core::arch::aarch64::*;

        #[derive(Clone, Copy, Debug)]
        #[repr(C, align(16))]
        pub struct i32x4(pub int32x4_t);
    } else {
        #[derive(Clone, Copy, Debug)]
        #[repr(C, align(16))]
        pub struct i32x4([i32; 4]);
    }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(explicit058_contain_unsafe, reason = "all selected lane storage layouts have a valid zero bit pattern in every lane")
)]
unsafe impl bytemuck::Zeroable for i32x4 {}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(explicit058_contain_unsafe, reason = "each selected layout is one register, a fixed lane array, or adjacent equal-width lane blocks without padding")
)]
unsafe impl bytemuck::Pod for i32x4 {}

impl Default for i32x4 {
  fn default() -> Self {
    let Ok(splat) = Self::splat(0);

    splat
  }
}

impl i32x4 {
  pub fn splat(n: i32) -> core::result::Result<Self, console_core_never::Never> {
    Ok(cast([n, n, n, n]))
  }

  pub fn blend(self, t: Self, f: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse4.1"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE4.1 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE4.1 enabled.
                  unsafe { _mm_blendv_epi8(f.0, t.0, self.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "relaxed-simd"))] {
              Self(i32x4_relaxed_laneselect(t.0, f.0, self.0))
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(v128_bitselect(t.0, f.0, self.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vbslq_s32(cast(self.0), t.0, f.0) },
              )
          } else {
              let Ok(blended) = super::generic_bit_blend(self, t, f);

              blended
          }
      }
    })
  }

  pub fn cmp_eq(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              cast(Self(cast(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_cmpeq_epi32(self.0, rhs.0) },
              )))
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(i32x4_eq(self.0, rhs.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { cast(vceqq_s32(self.0, rhs.0)) },
              )
          } else {
              Self([
                  if self.0[0] == rhs.0[0] { -1 } else { 0 },
                  if self.0[1] == rhs.0[1] { -1 } else { 0 },
                  if self.0[2] == rhs.0[2] { -1 } else { 0 },
                  if self.0[3] == rhs.0[3] { -1 } else { 0 },
              ])
          }
      }
    })
  }

  pub fn cmp_gt(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              cast(Self(cast(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_cmpgt_epi32(self.0, rhs.0) },
              )))
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(i32x4_gt(self.0, rhs.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { cast(vcgtq_s32(self.0, rhs.0)) },
              )
          } else {
              Self([
                  if self.0[0] > rhs.0[0] { -1 } else { 0 },
                  if self.0[1] > rhs.0[1] { -1 } else { 0 },
                  if self.0[2] > rhs.0[2] { -1 } else { 0 },
                  if self.0[3] > rhs.0[3] { -1 } else { 0 },
              ])
          }
      }
    })
  }

  pub fn cmp_lt(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              cast(Self(cast(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_cmplt_epi32(self.0, rhs.0) },
              )))
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(i32x4_lt(self.0, rhs.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { cast(vcltq_s32(self.0, rhs.0)) },
              )
          } else {
              Self([
                  if self.0[0] < rhs.0[0] { -1 } else { 0 },
                  if self.0[1] < rhs.0[1] { -1 } else { 0 },
                  if self.0[2] < rhs.0[2] { -1 } else { 0 },
                  if self.0[3] < rhs.0[3] { -1 } else { 0 },
              ])
          }
      }
    })
  }

  pub fn to_f32x4(self) -> core::result::Result<f32x4, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              cast(Self(cast(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_cvtepi32_ps(self.0) },
              )))
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              cast(Self(f32x4_convert_i32x4(self.0)))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              cast(Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { cast(vcvtq_f32_s32(self.0)) },
              ))
          } else {
              let arr: [i32; 4] = cast(self);
              cast([
                  arr[0] as f32,
                  arr[1] as f32,
                  arr[2] as f32,
                  arr[3] as f32,
              ])
          }
      }
    })
  }

  pub fn to_f32x4_bitcast(self) -> core::result::Result<f32x4, console_core_never::Never> {
    Ok(bytemuck::cast(self))
  }
}

impl From<[i32; 4]> for i32x4 {
  fn from(v: [i32; 4]) -> Self {
    cast(v)
  }
}

impl From<i32x4> for [i32; 4] {
  fn from(v: i32x4) -> Self {
    cast(v)
  }
}

impl core::ops::Add for i32x4 {
  type Output = Self;

  fn add(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with SSE2 enabled.
                unsafe { _mm_add_epi32(self.0, rhs.0) },
            )
        } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
            Self(i32x4_add(self.0, rhs.0))
        } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with NEON enabled.
                unsafe { vaddq_s32(self.0, rhs.0) },
            )
        } else {
            Self([
                self.0[0].wrapping_add(rhs.0[0]),
                self.0[1].wrapping_add(rhs.0[1]),
                self.0[2].wrapping_add(rhs.0[2]),
                self.0[3].wrapping_add(rhs.0[3]),
            ])
        }
    }
  }
}

impl core::ops::BitAnd for i32x4 {
  type Output = Self;

  fn bitand(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with SSE2 enabled.
                unsafe { _mm_and_si128(self.0, rhs.0) },
            )
        } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
            Self(v128_and(self.0, rhs.0))
        } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with NEON enabled.
                unsafe { vandq_s32(self.0, rhs.0) },
            )
        } else {
            Self([
                self.0[0] & rhs.0[0],
                self.0[1] & rhs.0[1],
                self.0[2] & rhs.0[2],
                self.0[3] & rhs.0[3],
            ])
        }
    }
  }
}

impl core::ops::Mul for i32x4 {
  type Output = Self;

  fn mul(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "sse4.1"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE4.1 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with SSE4.1 enabled.
                unsafe { _mm_mullo_epi32(self.0, rhs.0) },
            )
        } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
            Self(i32x4_mul(self.0, rhs.0))
        } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with NEON enabled.
                unsafe { vmulq_s32(self.0, rhs.0) },
            )
        } else {

            let a: [i32; 4] = cast(self);
            let b: [i32; 4] = cast(rhs);
            Self(cast([
                (*match &a { [element_2, ..] => element_2 }).wrapping_mul(match b { [element, ..] => element }),
                (*match &a { [_, element_2, ..] => element_2 }).wrapping_mul(match b { [_, element, ..] => element }),
                (*match &a { [_, _, element_2, ..] => element_2 }).wrapping_mul(match b { [_, _, element, ..] => element }),
                (*match &a { [_, _, _, element_2, ..] => element_2 }).wrapping_mul(match b { [_, _, _, element, ..] => element }),
            ]))
        }
    }
  }
}

impl core::ops::BitOr for i32x4 {
  type Output = Self;

  #[inline]
  fn bitor(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with SSE2 enabled.
                unsafe { _mm_or_si128(self.0, rhs.0) },
            )
        } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
            Self(v128_or(self.0, rhs.0))
        } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with NEON enabled.
                unsafe { vorrq_s32(self.0, rhs.0) },
            )
        } else {
            Self([
                self.0[0] | rhs.0[0],
                self.0[1] | rhs.0[1],
                self.0[2] | rhs.0[2],
                self.0[3] | rhs.0[3],
            ])
        }
    }
  }
}

impl core::ops::BitXor for i32x4 {
  type Output = Self;

  #[inline]
  fn bitxor(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with SSE2 enabled.
                unsafe { _mm_xor_si128(self.0, rhs.0) },
            )
        } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
            Self(v128_xor(self.0, rhs.0))
        } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with NEON enabled.
                unsafe { veorq_s32(self.0, rhs.0) },
            )
        } else {
            Self([
                self.0[0] ^ rhs.0[0],
                self.0[1] ^ rhs.0[1],
                self.0[2] ^ rhs.0[2],
                self.0[3] ^ rhs.0[3],
            ])
        }
    }
  }
}
