use bytemuck::cast;

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use console_core_paths::NoStdFloat;

use super::i32x4;

cfg_if::cfg_if! {
    if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
        #[cfg(target_arch = "x86")]
        use core::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::*;

        #[derive(Clone, Copy, Debug)]
        #[repr(C, align(16))]
        pub struct f32x4(__m128);
    } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
        use core::arch::wasm32::*;

        #[derive(Clone, Copy, Debug)]
        #[repr(transparent)]
        pub struct f32x4(v128);
    } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
        use core::arch::aarch64::*;

        #[derive(Clone, Copy, Debug)]
        #[repr(C, align(16))]
        pub struct f32x4(float32x4_t);
    } else {
        #[derive(Clone, Copy, Debug)]
        #[repr(C, align(16))]
        pub struct f32x4([f32; 4]);
    }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(explicit058_contain_unsafe, reason = "all selected lane storage layouts have a valid zero bit pattern in every lane")
)]
unsafe impl bytemuck::Zeroable for f32x4 {}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(explicit058_contain_unsafe, reason = "each selected layout is one register, a fixed lane array, or adjacent equal-width lane blocks without padding")
)]
unsafe impl bytemuck::Pod for f32x4 {}

impl Default for f32x4 {
  fn default() -> Self {
    let Ok(splat) = Self::splat(0.0);

    splat
  }
}

impl f32x4 {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(explicit067_no_unit_less_quantity, reason = "the generic SIMD broadcast preserves the units of its caller's lane value")
  )]
  pub fn splat(n: f32) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self::from([n, n, n, n]))
  }

  pub fn floor(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_floor(self.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vrndmq_f32(self.0) },
              )
          } else {
              let roundtrip: f32x4 = cast({ let Ok(explicit_value) = ({ let Ok(explicit_value) = self.trunc_int(); explicit_value }).to_f32x4(); explicit_value });
              roundtrip - ({ let Ok(explicit_value) = ({ let Ok(explicit_value) = roundtrip.cmp_gt(self); explicit_value }).blend({ let Ok(explicit_value) = f32x4::splat(1.0); explicit_value }, f32x4::default()); explicit_value })
          }
      }
    })
  }

  pub fn abs(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_abs(self.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vabsq_f32(self.0) },
              )
          } else {
              let non_sign_bits = { let Ok(explicit_value) = f32x4::splat(f32::from_bits({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(i32::MAX); numeric_value })); explicit_value };
              self & non_sign_bits
          }
      }
    })
  }

  pub fn max(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_max_ps(self.0, rhs.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "relaxed-simd"))] {
              Self(f32x4_relaxed_max(self.0, rhs.0))
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_pmax(self.0, rhs.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vmaxq_f32(self.0, rhs.0) },
              )
          } else {
              let mut lanes = self.0;

              for (left, right) in lanes.iter_mut().zip(rhs.0) {
                  match (*left < right).then_some(()) {
                      Some(()) => *left = right,
                      None => {}
                  }
              }

              Self(lanes)
          }
      }
    })
  }

  pub fn min(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_min_ps(self.0, rhs.0) },
              )
          }  else if #[cfg(all(feature = "simd", target_feature = "relaxed-simd"))] {
              Self(f32x4_relaxed_min(self.0, rhs.0))
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_pmin(self.0, rhs.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vminq_f32(self.0, rhs.0) },
              )
          } else {
              let mut lanes = self.0;

              for (left, right) in lanes.iter_mut().zip(rhs.0) {
                  match (right < *left).then_some(()) {
                      Some(()) => *left = right,
                      None => {}
                  }
              }

              Self(lanes)
          }
      }
    })
  }

  pub fn cmp_eq(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_cmpeq_ps(self.0, rhs.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_eq(self.0, rhs.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(cast(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vceqq_f32(self.0, rhs.0) },
              ))
          } else {
              Self([
                  if self.0[0] == rhs.0[0] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[1] == rhs.0[1] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[2] == rhs.0[2] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[3] == rhs.0[3] { f32::from_bits(u32::MAX) } else { 0.0 },
              ])
          }
      }
    })
  }

  pub fn cmp_ne(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_cmpneq_ps(self.0, rhs.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_ne(self.0, rhs.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(cast(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vmvnq_u32(vceqq_f32(self.0, rhs.0)) },
              ))
          } else {
              Self([
                  if self.0[0] != rhs.0[0] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[1] != rhs.0[1] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[2] != rhs.0[2] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[3] != rhs.0[3] { f32::from_bits(u32::MAX) } else { 0.0 },
              ])
          }
      }
    })
  }

  pub fn cmp_ge(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_cmpge_ps(self.0, rhs.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_ge(self.0, rhs.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(cast(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vcgeq_f32(self.0, rhs.0) },
              ))
          } else {
              Self([
                  if self.0[0] >= rhs.0[0] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[1] >= rhs.0[1] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[2] >= rhs.0[2] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[3] >= rhs.0[3] { f32::from_bits(u32::MAX) } else { 0.0 },
              ])
          }
      }
    })
  }

  pub fn cmp_gt(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_cmpgt_ps(self.0, rhs.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_gt(self.0, rhs.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(cast(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vcgtq_f32(self.0, rhs.0) },
              ))
          } else {
              Self([
                  if self.0[0] > rhs.0[0] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[1] > rhs.0[1] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[2] > rhs.0[2] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[3] > rhs.0[3] { f32::from_bits(u32::MAX) } else { 0.0 },
              ])
          }
      }
    })
  }

  pub fn cmp_le(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_cmple_ps(self.0, rhs.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_le(self.0, rhs.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(cast(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vcleq_f32(self.0, rhs.0) },
              ))
          } else {
              Self([
                  if self.0[0] <= rhs.0[0] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[1] <= rhs.0[1] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[2] <= rhs.0[2] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[3] <= rhs.0[3] { f32::from_bits(u32::MAX) } else { 0.0 },
              ])
          }
      }
    })
  }

  pub fn cmp_lt(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_cmplt_ps(self.0, rhs.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_lt(self.0, rhs.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(cast(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vcltq_f32(self.0, rhs.0) },
              ))
          } else {
              Self([
                  if self.0[0] < rhs.0[0] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[1] < rhs.0[1] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[2] < rhs.0[2] { f32::from_bits(u32::MAX) } else { 0.0 },
                  if self.0[3] < rhs.0[3] { f32::from_bits(u32::MAX) } else { 0.0 },
              ])
          }
      }
    })
  }

  #[inline]
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
                  unsafe { _mm_blendv_ps(f.0, t.0, self.0) },
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
                  unsafe { cast(vbslq_u32( cast(self.0), cast(t.0), cast(f.0))) },
              )
          } else {
              let Ok(blended) = super::generic_bit_blend(self, t, f);

              blended
          }
      }
    })
  }

  pub fn round(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse4.1"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE4.1 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE4.1 enabled.
                  unsafe { _mm_round_ps(self.0, _MM_FROUND_NO_EXC | _MM_FROUND_TO_NEAREST_INT) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_nearest(self.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vrndnq_f32(self.0) },
              )
          } else {
              use super::u32x4;

              let Ok(to_int) = f32x4::splat(1.0 / f32::EPSILON);
              let u: u32x4 = cast(self);
              let Ok(exponent_bits) = u32x4::splat(0xff);
              let e: i32x4 = cast(u.shr::<23>() & exponent_bits);

              let Ok(no_op_magic) = i32x4::splat(::core::primitive::i32::wrapping_add(0x7f, 23));
              let Ok(above_magic) = e.cmp_gt(no_op_magic);
              let Ok(at_magic) = e.cmp_eq(no_op_magic);
              let no_op_mask: f32x4 = cast(above_magic | at_magic);
              let no_op_val: f32x4 = self;

              let Ok(zero_magic) = i32x4::splat(::core::primitive::i32::wrapping_sub(0x7f, 1));
              let Ok(below_zero_magic) = e.cmp_lt(zero_magic);
              let zero_mask: f32x4 = cast(below_zero_magic);
              let Ok(zero) = f32x4::splat(0.0);
              let zero_val: f32x4 = self * zero;

              let Ok(negative) = cast::<u32x4, i32x4>(u).cmp_lt(i32x4::default());
              let neg_bit: f32x4 = cast(negative);
              let Ok(x) = neg_bit.blend(-self, self);
              let y = x + to_int - to_int - x;
              let Ok(half) = f32x4::splat(0.5);
              let Ok(minus_half) = f32x4::splat(-0.5);
              let Ok(minus_one) = f32x4::splat(-1.0);
              let Ok(one) = f32x4::splat(1.0);
              let Ok(above_half) = y.cmp_gt(half);
              let Ok(below_minus_half) = y.cmp_lt(minus_half);
              let Ok(rounded_down) = below_minus_half.blend(y + x + one, y + x);
              let Ok(rounded) = above_half.blend(y + x - minus_one, rounded_down);
              let Ok(signed) = neg_bit.blend(-rounded, rounded);
              let Ok(kept_or_zero) = zero_mask.blend(zero_val, signed);
              let Ok(answer) = no_op_mask.blend(no_op_val, kept_or_zero);

              answer
          }
      }
    })
  }

  pub fn round_int(self) -> core::result::Result<i32x4, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              i32x4(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_cvtps_epi32(self.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "relaxed-simd"))] {
              i32x4(i32x4_relaxed_trunc_f32x4(self.round().0))
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              i32x4(i32x4_trunc_sat_f32x4(self.round().0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              i32x4(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vcvtnq_s32_f32(self.0) },
              )
          } else {
              let Ok(whole) = self.round();
              let rounded: [f32; 4] = cast(whole);
              cast([
                  rounded[0] as i32,
                  rounded[1] as i32,
                  rounded[2] as i32,
                  rounded[3] as i32,
              ])
          }
      }
    })
  }

  pub fn trunc_int(self) -> core::result::Result<i32x4, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              i32x4(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_cvttps_epi32(self.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "relaxed-simd"))] {
              i32x4(i32x4_relaxed_trunc_f32x4(self.0))
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              i32x4(i32x4_trunc_sat_f32x4(self.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              i32x4(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vcvtq_s32_f32(self.0) },
              )
          } else {
              cast([
                  self.0[0] as i32,
                  self.0[1] as i32,
                  self.0[2] as i32,
                  self.0[3] as i32,
              ])
          }
      }
    })
  }

  pub fn recip_fast(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_rcp_ps(self.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_div(f32x4_splat(1.0), self.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              #[cfg_attr(
                dylint_lib = "explicit058_contain_unsafe",
                expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
              )]
              // SAFETY: this branch is compiled with NEON enabled.
              unsafe {
                  let a = vrecpeq_f32(self.0);
                  let a = vmulq_f32(vrecpsq_f32(self.0, a), a);
                  Self(a)
              }
          } else {
              Self::from([
                  1.0 / self.0[0],
                  1.0 / self.0[1],
                  1.0 / self.0[2],
                  1.0 / self.0[3],
              ])
          }
      }
    })
  }

  pub fn recip_sqrt(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_rsqrt_ps(self.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_div(f32x4_splat(1.0), f32x4_sqrt(self.0)))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              #[cfg_attr(
                dylint_lib = "explicit058_contain_unsafe",
                expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
              )]
              // SAFETY: this branch is compiled with NEON enabled.
              unsafe {
                  let a = vrsqrteq_f32(self.0);
                  let a = vmulq_f32(vrsqrtsq_f32(self.0, vmulq_f32(a, a)), a);
                  Self(a)
              }
          } else {
              Self::from([
                  1.0 / self.0[0].sqrt(),
                  1.0 / self.0[1].sqrt(),
                  1.0 / self.0[2].sqrt(),
                  1.0 / self.0[3].sqrt(),
              ])
          }
      }
    })
  }

  pub fn sqrt(self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      cfg_if::cfg_if! {
          if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with SSE2 enabled.
                  unsafe { _mm_sqrt_ps(self.0) },
              )
          } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
              Self(f32x4_sqrt(self.0))
          } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
              Self(
                  #[cfg_attr(
                    dylint_lib = "explicit058_contain_unsafe",
                    expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                  )]
                  // SAFETY: this branch is compiled with NEON enabled.
                  unsafe { vsqrtq_f32(self.0) },
              )
          } else {
              Self::from([
                  self.0[0].sqrt(),
                  self.0[1].sqrt(),
                  self.0[2].sqrt(),
                  self.0[3].sqrt(),
              ])
          }
      }
    })
  }
}

impl From<[f32; 4]> for f32x4 {
  fn from(v: [f32; 4]) -> Self {
    cast(v)
  }
}

impl From<f32x4> for [f32; 4] {
  fn from(v: f32x4) -> Self {
    cast(v)
  }
}

impl core::ops::Add for f32x4 {
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
                unsafe { _mm_add_ps(self.0, rhs.0) },
            )
        } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
            Self(f32x4_add(self.0, rhs.0))
        } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with NEON enabled.
                unsafe { vaddq_f32(self.0, rhs.0) },
            )
        } else {
            Self([
                self.0[0] + rhs.0[0],
                self.0[1] + rhs.0[1],
                self.0[2] + rhs.0[2],
                self.0[3] + rhs.0[3],
            ])
        }
    }
  }
}

impl core::ops::AddAssign for f32x4 {
  fn add_assign(&mut self, rhs: f32x4) {
    *self = *self + rhs;
  }
}

impl core::ops::Sub for f32x4 {
  type Output = Self;

  fn sub(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with SSE2 enabled.
                unsafe { _mm_sub_ps(self.0, rhs.0) },
            )
        } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
            Self(f32x4_sub(self.0, rhs.0))
        } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with NEON enabled.
                unsafe { vsubq_f32(self.0, rhs.0) },
            )
        } else {
            Self([
                self.0[0] - rhs.0[0],
                self.0[1] - rhs.0[1],
                self.0[2] - rhs.0[2],
                self.0[3] - rhs.0[3],
            ])
        }
    }
  }
}

impl core::ops::Mul for f32x4 {
  type Output = Self;

  fn mul(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with SSE2 enabled.
                unsafe { _mm_mul_ps(self.0, rhs.0) },
            )
        } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
            Self(f32x4_mul(self.0, rhs.0))
        } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with NEON enabled.
                unsafe { vmulq_f32(self.0, rhs.0) },
            )
        } else {
            Self([
                self.0[0] * rhs.0[0],
                self.0[1] * rhs.0[1],
                self.0[2] * rhs.0[2],
                self.0[3] * rhs.0[3],
            ])
        }
    }
  }
}

impl core::ops::MulAssign for f32x4 {
  fn mul_assign(&mut self, rhs: f32x4) {
    *self = *self * rhs;
  }
}

impl core::ops::Div for f32x4 {
  type Output = Self;

  fn div(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with SSE2 enabled.
                unsafe { _mm_div_ps(self.0, rhs.0) },
            )
        } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
            Self(f32x4_div(self.0, rhs.0))
        } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with NEON enabled.
                unsafe { vdivq_f32(self.0, rhs.0) },
            )
        } else {
            Self([
                self.0[0] / rhs.0[0],
                self.0[1] / rhs.0[1],
                self.0[2] / rhs.0[2],
                self.0[3] / rhs.0[3],
            ])
        }
    }
  }
}

impl core::ops::BitAnd for f32x4 {
  type Output = Self;

  #[inline(always)]
  fn bitand(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with SSE2 enabled.
                unsafe { _mm_and_ps(self.0, rhs.0) },
            )
        } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
            Self(v128_and(self.0, rhs.0))
        } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            Self(cast(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with NEON enabled.
                unsafe { vandq_u32(cast(self.0), cast(rhs.0)) },
            ))
        } else {
            Self([
                f32::from_bits(self.0[0].to_bits() & rhs.0[0].to_bits()),
                f32::from_bits(self.0[1].to_bits() & rhs.0[1].to_bits()),
                f32::from_bits(self.0[2].to_bits() & rhs.0[2].to_bits()),
                f32::from_bits(self.0[3].to_bits() & rhs.0[3].to_bits()),
            ])
        }
    }
  }
}

impl core::ops::BitOr for f32x4 {
  type Output = Self;

  #[inline(always)]
  fn bitor(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with SSE2 enabled.
                unsafe { _mm_or_ps(self.0, rhs.0) },
            )
        } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
            Self(v128_or(self.0, rhs.0))
        } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            Self(cast(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with NEON enabled.
                unsafe { vorrq_u32(cast(self.0), cast(rhs.0)) },
            ))
        } else {
            Self([
                f32::from_bits(self.0[0].to_bits() | rhs.0[0].to_bits()),
                f32::from_bits(self.0[1].to_bits() | rhs.0[1].to_bits()),
                f32::from_bits(self.0[2].to_bits() | rhs.0[2].to_bits()),
                f32::from_bits(self.0[3].to_bits() | rhs.0[3].to_bits()),
            ])
        }
    }
  }
}

impl core::ops::BitXor for f32x4 {
  type Output = Self;

  #[inline(always)]
  fn bitxor(self, rhs: Self) -> Self::Output {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
            Self(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with SSE2 enabled.
                unsafe { _mm_xor_ps(self.0, rhs.0) },
            )
        } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
            Self(v128_xor(self.0, rhs.0))
        } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            Self(cast(
                #[cfg_attr(
                  dylint_lib = "explicit058_contain_unsafe",
                  expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
                )]
                // SAFETY: this branch is compiled with NEON enabled.
                unsafe { veorq_u32(cast(self.0), cast(rhs.0)) },
            ))
        } else {
            Self([
                f32::from_bits(self.0[0].to_bits() ^ rhs.0[0].to_bits()),
                f32::from_bits(self.0[1].to_bits() ^ rhs.0[1].to_bits()),
                f32::from_bits(self.0[2].to_bits() ^ rhs.0[2].to_bits()),
                f32::from_bits(self.0[3].to_bits() ^ rhs.0[3].to_bits()),
            ])
        }
    }
  }
}

impl core::ops::Neg for f32x4 {
  type Output = Self;

  fn neg(self) -> Self {
    Self::default() - self
  }
}

impl core::ops::Not for f32x4 {
  type Output = Self;

  fn not(self) -> Self {
    let Ok(all_bits) = Self::splat(f32::from_bits(u32::MAX));

    self ^ all_bits
  }
}

impl core::cmp::PartialEq for f32x4 {
  fn eq(&self, rhs: &Self) -> bool {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "simd", target_feature = "sse2"))] {
            #[cfg_attr(
              dylint_lib = "explicit058_contain_unsafe",
              expect(explicit058_contain_unsafe, reason = "the compile-time branch requires SSE2 and the intrinsic takes only owned vector registers")
            )]
            // SAFETY: this branch is compiled with SSE2 enabled.
            unsafe { _mm_movemask_ps(_mm_cmpeq_ps(self.0, rhs.0)) == 0b1111 }
        } else if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            #[cfg_attr(
              dylint_lib = "explicit058_contain_unsafe",
              expect(explicit058_contain_unsafe, reason = "the compile-time branch requires NEON and the intrinsic takes only owned vector registers")
            )]
            // SAFETY: this branch is compiled with NEON enabled.
            unsafe { vminvq_u32(vceqq_f32(self.0, rhs.0)) != 0 }
        } else if #[cfg(all(feature = "simd", target_feature = "simd128"))] {
            u32x4_all_true(f32x4_eq(self.0, rhs.0))
        } else {
            self.0 == rhs.0
        }
    }
  }
}
