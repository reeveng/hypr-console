use alloc::vec::Vec;

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "Rust array lengths and slice chunk const parameters have native width"
  )
)]
pub(crate) const SIMD_WIDTH: usize = 8;

#[cfg(feature = "simd")]
mod inner {
  use super::SIMD_WIDTH;
  use core::ops::{Add, AddAssign, DivAssign, Mul, MulAssign, Sub, SubAssign};
  use fearless_simd::{SimdBase, SimdFloat};

  pub(crate) use fearless_simd::{Level, Simd, dispatch};

  #[derive(Copy, Clone)]
  #[repr(C, align(32))]
  pub(crate) struct Float32x8<S: Simd> {
    inner: fearless_simd::f32x8<S>,
  }

  impl<S: Simd> Float32x8<S> {
    #[inline(always)]
    pub(crate) fn from_slice(
      simd: S,
      slice: &[f32; SIMD_WIDTH],
    ) -> core::result::Result<Self, console_core_never::Never> {
      Ok(Self {
        inner: fearless_simd::f32x8::from_slice(simd, slice),
      })
    }

    #[inline(always)]
    pub(crate) fn splat(
      simd: S,
      super::Sample(value): super::Sample,
    ) -> core::result::Result<Self, console_core_never::Never> {
      Ok(Self {
        inner: fearless_simd::f32x8::splat(simd, value),
      })
    }

    #[inline(always)]
    pub(crate) fn mul_add(
      self,
      mul: Self,
      addend: Self,
    ) -> core::result::Result<Self, console_core_never::Never> {
      Ok(Self {
        inner: self.inner.mul_add(mul.inner, addend.inner),
      })
    }

    #[inline(always)]
    pub(crate) fn floor(self) -> core::result::Result<Self, console_core_never::Never> {
      Ok(Self {
        inner: self.inner.floor(),
      })
    }

    #[inline(always)]
    pub(crate) fn store(
      self,
      slice: &mut [f32; SIMD_WIDTH],
    ) -> core::result::Result<(), console_core_never::Never> {
      {
        self.inner.store_slice(slice);
      };

      Ok(())
    }

    #[inline(always)]
    pub(crate) fn zip_low(
      self,
      other: Self,
    ) -> core::result::Result<Self, console_core_never::Never> {
      Ok(Self {
        inner: self.inner.zip_low(other.inner),
      })
    }

    #[inline(always)]
    pub(crate) fn zip_high(
      self,
      other: Self,
    ) -> core::result::Result<Self, console_core_never::Never> {
      Ok(Self {
        inner: self.inner.zip_high(other.inner),
      })
    }

    #[inline(always)]
    pub(crate) fn min(
      self,
      other: Self,
    ) -> core::result::Result<Self, console_core_never::Never> {
      Ok(Self {
        inner: self.inner.min(other.inner),
      })
    }

    #[inline(always)]
    pub(crate) fn max(
      self,
      other: Self,
    ) -> core::result::Result<Self, console_core_never::Never> {
      Ok(Self {
        inner: self.inner.max(other.inner),
      })
    }
  }

  impl<S: Simd> Add for Float32x8<S> {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
      Self {
        inner: self.inner + rhs.inner,
      }
    }
  }

  impl<S: Simd> Sub for Float32x8<S> {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
      Self {
        inner: self.inner - rhs.inner,
      }
    }
  }

  impl<S: Simd> Mul for Float32x8<S> {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
      Self {
        inner: self.inner * rhs.inner,
      }
    }
  }

  impl<S: Simd> Add<f32> for Float32x8<S> {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: f32) -> Self {
      Self {
        inner: self.inner + rhs,
      }
    }
  }

  impl<S: Simd> Mul<f32> for Float32x8<S> {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: f32) -> Self {
      Self {
        inner: self.inner * rhs,
      }
    }
  }

  impl<S: Simd> AddAssign for Float32x8<S> {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
      self.inner = self.inner + rhs.inner;
    }
  }

  impl<S: Simd> SubAssign for Float32x8<S> {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: Self) {
      self.inner = self.inner - rhs.inner;
    }
  }

  impl<S: Simd> MulAssign<f32> for Float32x8<S> {
    #[inline(always)]
    fn mul_assign(&mut self, rhs: f32) {
      self.inner = self.inner * rhs;
    }
  }

  impl<S: Simd> DivAssign<f32> for Float32x8<S> {
    #[inline(always)]
    fn div_assign(&mut self, rhs: f32) {
      self.inner = self.inner / rhs;
    }
  }
}

#[cfg(not(feature = "simd"))]
mod inner {
  use super::SIMD_WIDTH;
  use core::marker::PhantomData;
  use core::ops::{Add, AddAssign, DivAssign, Mul, MulAssign, Sub, SubAssign};

  pub(crate) trait Simd: Copy + Clone {}

  #[derive(Copy, Clone)]
  pub(crate) struct ScalarSimd;

  impl Simd for ScalarSimd {}

  pub(crate) struct Level;

  impl Level {
    #[inline(always)]
    pub(crate) fn new() -> core::result::Result<Self, console_core_never::Never> {
      Ok(Level)
    }
  }

  #[derive(Copy, Clone)]
  #[repr(C, align(32))]
  pub(crate) struct Float32x8<S: Simd> {
    val: [f32; SIMD_WIDTH],
    _marker: PhantomData<S>,
  }

  impl<S: Simd> Float32x8<S> {
    #[inline(always)]
    pub(crate) fn from_slice(
      _simd: S,
      slice: &[f32; SIMD_WIDTH],
    ) -> core::result::Result<Self, console_core_never::Never> {
      let val = *slice;

      Ok(Self {
        val,
        _marker: PhantomData,
      })
    }

    #[inline(always)]
    pub(crate) fn splat(
      _simd: S,
      super::Sample(value): super::Sample,
    ) -> core::result::Result<Self, console_core_never::Never> {
      Ok(Self {
        val: [value; SIMD_WIDTH],
        _marker: PhantomData,
      })
    }

    #[inline(always)]
    pub(crate) fn mul_add(
      self,
      mul: Self,
      addend: Self,
    ) -> core::result::Result<Self, console_core_never::Never> {
      let mut result = [0.0f32; SIMD_WIDTH];

      for (((out, value), factor), offset) in
        result.iter_mut().zip(self.val).zip(mul.val).zip(addend.val)
      {
        let Ok(fused) = super::mul_add(super::MultiplyAdd {
          value: super::Sample(value),
          factor: super::Gain(factor),
          offset: super::Sample(offset),
        });
        *out = fused;
      }

      Ok(Self {
        val: result,
        _marker: PhantomData,
      })
    }

    #[inline(always)]
    pub(crate) fn floor(self) -> core::result::Result<Self, console_core_never::Never> {
      let mut result = [0.0f32; SIMD_WIDTH];

      for (out, value) in result.iter_mut().zip(self.val) {
        let Ok(rounded) = super::floor_f32(super::Sample(value));
        *out = rounded;
      }

      Ok(Self {
        val: result,
        _marker: PhantomData,
      })
    }

    #[inline(always)]
    pub(crate) fn store(
      self,
      slice: &mut [f32; SIMD_WIDTH],
    ) -> core::result::Result<(), console_core_never::Never> {
      {
        *slice = self.val;
      };

      Ok(())
    }

    #[inline(always)]
    pub(crate) fn zip_low(
      self,
      other: Self,
    ) -> core::result::Result<Self, console_core_never::Never> {
      let [a, b, c, d, _, _, _, _] = self.val;
      let [other_a, other_b, other_c, other_d, _, _, _, _] = other.val;

      Ok(Self {
        val: [a, other_a, b, other_b, c, other_c, d, other_d],
        _marker: PhantomData,
      })
    }

    #[inline(always)]
    pub(crate) fn zip_high(
      self,
      other: Self,
    ) -> core::result::Result<Self, console_core_never::Never> {
      let [_, _, _, _, e, f, g, h] = self.val;
      let [_, _, _, _, other_e, other_f, other_g, other_h] = other.val;

      Ok(Self {
        val: [e, other_e, f, other_f, g, other_g, h, other_h],
        _marker: PhantomData,
      })
    }

    #[inline(always)]
    pub(crate) fn min(
      self,
      other: Self,
    ) -> core::result::Result<Self, console_core_never::Never> {
      let mut result = [0.0f32; SIMD_WIDTH];

      for ((out, a), b) in result.iter_mut().zip(self.val).zip(other.val) {
        let Ok(bounded) = super::min_f32([a, b]);
        *out = bounded;
      }

      Ok(Self {
        val: result,
        _marker: PhantomData,
      })
    }

    #[inline(always)]
    pub(crate) fn max(
      self,
      other: Self,
    ) -> core::result::Result<Self, console_core_never::Never> {
      let mut result = [0.0f32; SIMD_WIDTH];

      for ((out, a), b) in result.iter_mut().zip(self.val).zip(other.val) {
        let Ok(bounded) = super::max_f32([a, b]);
        *out = bounded;
      }

      Ok(Self {
        val: result,
        _marker: PhantomData,
      })
    }
  }

  impl<S: Simd> Add for Float32x8<S> {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
      let mut result = [0.0f32; SIMD_WIDTH];

      for ((out, lhs), rhs_2) in result.iter_mut().zip(self.val).zip(rhs.val) {
        *out = lhs + rhs_2;
      }

      Self {
        val: result,
        _marker: PhantomData,
      }
    }
  }

  impl<S: Simd> Sub for Float32x8<S> {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
      let mut result = [0.0f32; SIMD_WIDTH];

      for ((out, lhs), rhs_2) in result.iter_mut().zip(self.val).zip(rhs.val) {
        *out = lhs - rhs_2;
      }

      Self {
        val: result,
        _marker: PhantomData,
      }
    }
  }

  impl<S: Simd> Mul for Float32x8<S> {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
      let mut result = [0.0f32; SIMD_WIDTH];

      for ((out, lhs), rhs_2) in result.iter_mut().zip(self.val).zip(rhs.val) {
        *out = lhs * rhs_2;
      }

      Self {
        val: result,
        _marker: PhantomData,
      }
    }
  }

  impl<S: Simd> Add<f32> for Float32x8<S> {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: f32) -> Self {
      let mut result = [0.0f32; SIMD_WIDTH];

      for (out, lhs) in result.iter_mut().zip(self.val) {
        *out = lhs + rhs;
      }

      Self {
        val: result,
        _marker: PhantomData,
      }
    }
  }

  impl<S: Simd> Mul<f32> for Float32x8<S> {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: f32) -> Self {
      let mut result = [0.0f32; SIMD_WIDTH];

      for (out, lhs) in result.iter_mut().zip(self.val) {
        *out = lhs * rhs;
      }

      Self {
        val: result,
        _marker: PhantomData,
      }
    }
  }

  impl<S: Simd> AddAssign for Float32x8<S> {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
      for (lhs, rhs_2) in self.val.iter_mut().zip(rhs.val) {
        *lhs += rhs_2;
      }
    }
  }

  impl<S: Simd> SubAssign for Float32x8<S> {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: Self) {
      for (lhs, rhs_2) in self.val.iter_mut().zip(rhs.val) {
        *lhs -= rhs_2;
      }
    }
  }

  impl<S: Simd> MulAssign<f32> for Float32x8<S> {
    #[inline(always)]
    fn mul_assign(&mut self, rhs: f32) {
      for lane in &mut self.val {
        *lane *= rhs;
      }
    }
  }

  impl<S: Simd> DivAssign<f32> for Float32x8<S> {
    #[inline(always)]
    fn div_assign(&mut self, rhs: f32) {
      for lane in &mut self.val {
        *lane /= rhs;
      }
    }
  }

  #[macro_export]
  #[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
      explicit080_macro_budget,
      reason = "provides fearless_simd dispatch syntax when the optional SIMD dependency is unavailable"
    )
  )]
  macro_rules! simd_dispatch {
    ($level:expr, $simd:ident => $body:expr) => {{
      let Ok(_scalar_level) = $level;
      let $simd = $crate::math::ScalarSimd;
      $body
    }};
  }

  pub(crate) use simd_dispatch as dispatch;
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "JPEG 2000 component and wavelet samples use the scale of their encoded component"
  )
)]
pub(crate) struct Sample(pub(crate) f32);

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "color and wavelet gains are dimensionless scale factors"
  )
)]
pub(crate) struct Gain(pub(crate) f32);

pub(crate) struct MultiplyAdd {
  pub(crate) value: Sample,
  pub(crate) factor: Gain,
  pub(crate) offset: Sample,
}

#[inline(always)]
pub(crate) fn mul_add(
  MultiplyAdd {
    value: Sample(value),
    factor: Gain(factor),
    offset: Sample(offset),
  }: MultiplyAdd,
) -> core::result::Result<f32, console_core_never::Never> {
  Ok({
    #[cfg(all(
      feature = "std",
      any(
        all(
          any(target_arch = "x86", target_arch = "x86_64"),
          target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
      )
    ))]
    {
      f32::mul_add(value, factor, offset)
    }

    #[cfg(not(all(
      feature = "std",
      any(
        all(
          any(target_arch = "x86", target_arch = "x86_64"),
          target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
      )
    )))]
    {
      value * factor + offset
    }
  })
}

#[inline(always)]
pub(crate) fn floor_f32(
  Sample(x): Sample,
) -> core::result::Result<f32, console_core_never::Never> {
  Ok({
    #[cfg(feature = "std")]
    {
      x.floor()
    }

    #[cfg(not(feature = "std"))]
    {
      let ::core::result::Result::Ok(xi) = ::console_core_number_conversion::toward_zero_i32(
        <::core::primitive::f64 as ::core::convert::From<::core::primitive::f32>>::from(x),
      );
      let ::core::result::Result::Ok(xf) = ::console_core_number_conversion::nearest_f32_from_i64(
        <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(xi),
      );

      match ::core::primitive::bool::then_some(x < xf, ()) {
        ::core::option::Option::Some(()) => xf - 1.0,
        ::core::option::Option::None => xf,
      }
    }
  })
}

#[inline(always)]
pub(crate) fn round_f32(
  Sample(x): Sample,
) -> core::result::Result<f32, console_core_never::Never> {
  Ok({
    #[cfg(feature = "std")]
    {
      x.round()
    }

    #[cfg(not(feature = "std"))]
    {
      match ::core::primitive::bool::then_some(x >= 0.0, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(value) = floor_f32(Sample(x + 0.5));
          value
        }
        ::core::option::Option::None => {
          let Ok(value) = floor_f32(Sample(-x + 0.5));
          -value
        }
      }
    }
  })
}

#[inline(always)]
pub(crate) fn pow2i(exp: i32) -> core::result::Result<f32, console_core_never::Never> {
  Ok(match ::core::primitive::bool::then_some(exp >= 0, ()) {
    ::core::option::Option::Some(()) => {
      match console_core_number_conversion::narrow_f32(f64::from(u32::wrapping_shl(
        1_u32,
        match console_core_number_conversion::wrapped::<_, u32>(exp) {
          Ok(shift_bits) => shift_bits,
        },
      ))) {
        Ok(converted) => converted,
      }
    }
    ::core::option::Option::None => {
      1.0
        / (match console_core_number_conversion::narrow_f32(f64::from(u32::wrapping_shl(
          1_u32,
          match console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_neg(exp)) {
            Ok(shift_bits) => shift_bits,
          },
        ))) {
          Ok(converted) => converted,
        })
    }
  })
}

#[inline(always)]
#[cfg(not(feature = "simd"))]
pub(crate) fn min_f32([a, b]: [f32; 2]) -> core::result::Result<f32, console_core_never::Never> {
  Ok({
    #[cfg(feature = "std")]
    {
      a.min(b)
    }

    #[cfg(not(feature = "std"))]
    {
      match ::core::primitive::bool::then_some(a < b, ()) {
        ::core::option::Option::Some(()) => a,
        ::core::option::Option::None => b,
      }
    }
  })
}

#[inline(always)]
#[cfg(not(feature = "simd"))]
pub(crate) fn max_f32([a, b]: [f32; 2]) -> core::result::Result<f32, console_core_never::Never> {
  Ok({
    #[cfg(feature = "std")]
    {
      a.max(b)
    }

    #[cfg(not(feature = "std"))]
    {
      match ::core::primitive::bool::then_some(a > b, ()) {
        ::core::option::Option::Some(()) => a,
        ::core::option::Option::None => b,
      }
    }
  })
}

pub(crate) use inner::*;

#[derive(Debug, Clone)]
pub(crate) struct SimdBuffer {
  data: Vec<f32>,
  original_len: u64,
}

impl SimdBuffer {
  pub(crate) fn new(mut data: Vec<f32>) -> crate::Result<Self> {
    let Ok(original_len) = console_core_number_conversion::fixed_index(data.len());
    let Ok(padded) = Self::padded_length(original_len);
    crate::resize_decoder_buffer(&mut data, padded, crate::DecoderBuffer::Component, 0.0)?;
    Ok(Self { data, original_len })
  }

  pub(crate) fn zeros(original_len: u64) -> crate::Result<Self> {
    let Ok(padded) = Self::padded_length(original_len);
    let mut data = Vec::new();
    crate::resize_decoder_buffer(&mut data, padded, crate::DecoderBuffer::Component, 0.0)?;
    Ok(Self { data, original_len })
  }

  fn padded_length(original_len: u64) -> core::result::Result<u64, console_core_never::Never> {
    let Ok(lanes) = console_core_number_conversion::fixed_index(SIMD_WIDTH);
    let padding = lanes.wrapping_sub(original_len.wrapping_rem(lanes.max(1)));
    Ok(original_len.saturating_add(padding))
  }

  pub(crate) fn samples_mut(
    &mut self,
  ) -> core::result::Result<&mut [f32], console_core_never::Never> {
    Ok(&mut self.data)
  }

  pub(crate) fn truncated(&self) -> crate::Result<&[f32]> {
    let len = console_core_number_conversion::checked_index(self.original_len)
      .map_err(|_overflow| crate::ValidationError::InvalidDimensions)?;
    let data = self
      .data
      .get(..len)
      .ok_or(crate::ValidationError::InvalidDimensions)?;
    Ok(data)
  }
}

impl core::ops::Deref for SimdBuffer {
  type Target = [f32];

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.data
  }
}
