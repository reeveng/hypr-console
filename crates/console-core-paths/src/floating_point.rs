use crate::scalar::Scalar;

pub use console_core_finite_numbers::{
  FiniteF32, NonZeroPositiveF32, NormalizedF32, PositiveF32,
};

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The numeric adapter takes raw floats into validated types and reads them back"
  )
)]
pub trait StrictNum: Sized + Copy {
  fn new(n: f32) -> Option<Self>;

  fn get(self) -> f32;
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The raw input is a dimensionless ratio"
  )
)]
pub trait StrictNormalized: Sized {
  fn new_clamped(n: f32) -> Self;

  fn new_u8(n: u8) -> Self;

  fn to_u8(&self) -> u8;
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "The four validated float types each use the same two forwarding methods"
  )
)]
macro_rules! strict_num {
  ($kind:ident) => {
    impl StrictNum for $kind {
      #[inline]
      fn new(n: f32) -> Option<Self> {
        let Ok(held) = $kind::of(n);
        held
      }

      #[inline]
      fn get(self) -> f32 {
        f32::from(self)
      }
    }
  };
}

strict_num!(FiniteF32);
strict_num!(PositiveF32);
strict_num!(NonZeroPositiveF32);
strict_num!(NormalizedF32);

impl StrictNormalized for NormalizedF32 {
  #[inline]
  fn new_clamped(n: f32) -> Self {
    let Ok(held) = NormalizedF32::nearest(n);
    held
  }

  #[inline]
  fn new_u8(n: u8) -> Self {
    let Ok(held) = NormalizedF32::nearest(f32::from(n) / 255.0);
    held
  }

  #[inline]
  fn to_u8(&self) -> u8 {
    {
      let Ok(numeric_value) =
        console_core_number_conversion::toward_zero_u8(f64::from(f32::from(*self) * 255.0 + 0.5));
      numeric_value
    }
  }
}

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use crate::NoStdFloat;

pub(crate) const FLOAT_PI: f32 = core::f32::consts::PI;

const MAX_I32_FITS_IN_F32: f32 = 2147483520.0;
const MIN_I32_FITS_IN_F32: f32 = -MAX_I32_FITS_IN_F32;

pub trait SaturateCast<T>: Sized {
  fn saturate_from(n: T) -> Self;
}

impl SaturateCast<f32> for i32 {
  fn saturate_from(mut x: f32) -> Self {
    x = match ::core::primitive::bool::then_some(x < MAX_I32_FITS_IN_F32, ()) {
      ::core::option::Option::Some(()) => x,
      ::core::option::Option::None => MAX_I32_FITS_IN_F32,
    };
    x = match ::core::primitive::bool::then_some(x > MIN_I32_FITS_IN_F32, ()) {
      ::core::option::Option::Some(()) => x,
      ::core::option::Option::None => MIN_I32_FITS_IN_F32,
    };

    {
      let Ok(numeric_value) = console_core_number_conversion::toward_zero_i32(f64::from(x));
      numeric_value
    }
  }
}

impl SaturateCast<f64> for i32 {
  fn saturate_from(mut x: f64) -> Self {
    x = match ::core::primitive::bool::then_some(x < f64::from(i32::MAX), ()) {
      ::core::option::Option::Some(()) => x,
      ::core::option::Option::None => f64::from(i32::MAX),
    };
    x = match ::core::primitive::bool::then_some(x > f64::from(i32::MIN), ()) {
      ::core::option::Option::Some(()) => x,
      ::core::option::Option::None => f64::from(i32::MIN),
    };

    {
      let Ok(numeric_value) = console_core_number_conversion::toward_zero_i32(x);
      numeric_value
    }
  }
}

pub trait SaturateRound<T>: SaturateCast<T> {
  fn saturate_floor(n: T) -> Self;

  fn saturate_ceil(n: T) -> Self;

  fn saturate_round(n: T) -> Self;
}

impl SaturateRound<f32> for i32 {
  fn saturate_floor(x: f32) -> Self {
    Self::saturate_from(x.floor())
  }

  fn saturate_ceil(x: f32) -> Self {
    Self::saturate_from(x.ceil())
  }

  fn saturate_round(x: f32) -> Self {
    Self::saturate_from(x.floor() + 0.5)
  }
}

pub(crate) fn float_bits_in_value_order(
  bits: u32,
) -> core::result::Result<i32, console_core_never::Never> {
  let Ok(signed) = console_core_number_conversion::wrapped::<_, i32>(bits);
  sign_bit_to_2s_compliment(signed)
}

fn sign_bit_to_2s_compliment(mut x: i32) -> core::result::Result<i32, console_core_never::Never> {
  match ::core::primitive::bool::then_some(x < 0, ()) {
    ::core::option::Option::Some(()) => {
      x &= 0x7FFFFFFF;
      x = (x).wrapping_neg();
    }
    ::core::option::Option::None => {}
  }

  Ok(x)
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct NormalizedF32Exclusive(u32);

impl Default for NormalizedF32Exclusive {
  fn default() -> Self {
    Self::HALF
  }
}

impl core::fmt::Debug for NormalizedF32Exclusive {
  fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    formatter
      .debug_tuple("NormalizedF32Exclusive")
      .field(&f32::from_bits(self.0))
      .finish()
  }
}

impl NormalizedF32Exclusive {
  pub const ANY: Self = Self::HALF;
  pub const HALF: Self = Self(0.5_f32.to_bits());

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "This checked constructor takes a raw dimensionless parameter"
    )
  )]
  pub fn new(value: f32) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok((value > 0.0 && value < 1.0).then_some(Self(value.to_bits())))
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The clamp takes a raw dimensionless parameter"
    )
  )]
  pub fn new_bounded(value: f32) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self(
      value.bound(f32::EPSILON, 1.0 - f32::EPSILON).to_bits(),
    ))
  }

  pub fn get(self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(f32::from_bits(self.0))
  }

  pub fn to_normalized(self) -> core::result::Result<NormalizedF32, console_core_never::Never> {
    NormalizedF32::nearest(f32::from_bits(self.0))
  }
}
