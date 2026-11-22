//! Fixed-point numbers: a signed integer counting a power-of-two fraction of one.
//!
//! OpenType writes six of them, named for how many bits are whole and how many are fraction --
//! 2.14 for normalized coordinates, 16.16 for most of the tables, 26.6 for hinting, 4.12 and 6.10
//! for the colour tables, and 48.16 for the sum of variation deltas, which can run past what 16.16
//! holds. The arithmetic wraps rather than saturating, because `FreeType`'s does and a glyph has to
//! come out the same here as it does there; division by zero answers the largest magnitude with the
//! quotient's sign, which is `FreeType`'s answer too.
//!
//! A float is taken in by rounding half away from zero and is held at the ends of the range, which
//! is what the cast upstream wrote did, and given back exactly: every value a type can hold is a
//! value its float can say. A narrowing between two of them keeps the low bits, as the casts it
//! replaces did, and is written as the low bytes of the wider one so the width it becomes is
//! named at the site.
//!
//! What is printed is the shortest decimal that reads back as the same value: the fewest
//! fractional digits whose nearest decimal is strictly within half a step of it, as `fontTools`'
//! `fixedToStr` does, with no point at all for a whole number. Five places always suffice, because
//! the finest step is 2^-16 and 10^5 is past 2^16; the products the search makes stay under 2^82,
//! so they are written to wrap rather than to saturate. Asked for a precision, a value prints as
//! its float does.
//!
//! The six types share one definition, so it is written once as a macro; the body is held to the
//! rules as though the lints could see inside it, which they mostly cannot.

use console_core_never::Never;
use console_core_number_conversion::{
  Float, fitted, index, narrow_f32, toward_zero_i32, toward_zero_i64,
};
use core::cmp::Ordering;
use core::fmt::{self, Write};
use core::num::NonZeroU128;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

const fn low_i16(value: i32) -> Result<i16, Never> {
  let [_, _, high, low] = value.to_be_bytes();

  Ok(i16::from_be_bytes([high, low]))
}

const fn low_i32(value: i64) -> Result<i32, Never> {
  let [_, _, _, _, first, second, third, fourth] = value.to_be_bytes();

  Ok(i32::from_be_bytes([first, second, third, fourth]))
}

const fn low_u32(value: u64) -> Result<u32, Never> {
  let [_, _, _, _, first, second, third, fourth] = value.to_be_bytes();

  Ok(u32::from_be_bytes([first, second, third, fourth]))
}

fn i16_in_f32(value: i16) -> Result<f32, Never> {
  Ok(f32::from(value))
}

fn i16_in_f64(value: i16) -> Result<f64, Never> {
  Ok(f64::from(value))
}

fn i32_in_f64(value: i32) -> Result<f64, Never> {
  Ok(f64::from(value))
}

fn i64_in_f64(value: i64) -> Result<f64, Never> {
  value.float()
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the input is a count of fixed-point steps already scaled for storage, independent of the caller's unit"
  )
)]
fn f32_toward_i16(value: f32) -> Result<i16, Never> {
  let Ok(wide) = toward_zero_i32(f64::from(value));

  fitted::<i32, i16>(wide)
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the input is a count of fixed-point steps already scaled for storage, independent of the caller's unit"
  )
)]
fn f64_toward_i16(value: f64) -> Result<i16, Never> {
  let Ok(wide) = toward_zero_i32(value);

  fitted::<i32, i16>(wide)
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "six fixed-point widths share their constants, bit operations and byte order, and one definition keeps the six from drifting apart"
  )
)]
macro_rules! fixed_impl {
  ($name:ident, $bits:literal, $fract_bits:literal, $ty:ty) => {
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    #[cfg_attr(
      feature = "bytemuck",
      derive(bytemuck::AnyBitPattern, bytemuck::NoUninit)
    )]
    #[repr(transparent)]
    pub struct $name($ty);

    impl $name {
      pub const MIN: Self = Self(<$ty>::MIN);

      pub const MAX: Self = Self(<$ty>::MAX);

      pub const EPSILON: Self = Self(1);

      pub const ZERO: Self = Self(0);

      pub const ONE: Self = Self(1 << $fract_bits);

      pub const NEG_ONE: Self = Self(!0 << $fract_bits);

      const INT_MASK: $ty = !0 << $fract_bits;

      const ROUND: $ty = 1 << ($fract_bits - 1);

      const FRACT_BITS: u32 = $fract_bits;

      #[inline(always)]
      pub const fn from_bits(bits: $ty) -> Result<Self, Never> {
        Ok(Self(bits))
      }

      #[inline(always)]
      pub const fn to_bits(self) -> Result<$ty, Never> {
        Ok(self.0)
      }

      #[inline(always)]
      pub const fn round(self) -> Result<Self, Never> {
        Ok(Self(self.0.wrapping_add(Self::ROUND) & Self::INT_MASK))
      }

      #[inline(always)]
      pub const fn abs(self) -> Result<Self, Never> {
        Ok(Self(self.0.wrapping_abs()))
      }

      #[inline(always)]
      pub const fn floor(self) -> Result<Self, Never> {
        Ok(Self(self.0 & Self::INT_MASK))
      }

      #[inline(always)]
      pub const fn fract(self) -> Result<Self, Never> {
        Ok(Self(self.0 & !Self::INT_MASK))
      }

      #[inline(always)]
      pub const fn wrapping_add(self, other: Self) -> Result<Self, Never> {
        Ok(Self(self.0.wrapping_add(other.0)))
      }

      #[inline(always)]
      pub const fn saturating_add(self, other: Self) -> Result<Self, Never> {
        Ok(Self(self.0.saturating_add(other.0)))
      }

      #[inline(always)]
      pub const fn checked_add(self, other: Self) -> Result<Option<Self>, Never> {
        Ok(match self.0.checked_add(other.0) {
          Some(inner) => Some(Self(inner)),
          None => None,
        })
      }

      #[inline(always)]
      pub const fn wrapping_sub(self, other: Self) -> Result<Self, Never> {
        Ok(Self(self.0.wrapping_sub(other.0)))
      }

      #[inline(always)]
      pub const fn saturating_sub(self, other: Self) -> Result<Self, Never> {
        Ok(Self(self.0.saturating_sub(other.0)))
      }

      #[inline(always)]
      pub const fn to_be_bytes(self) -> Result<[u8; $bits / 8], Never> {
        Ok(self.0.to_be_bytes())
      }
    }

    impl Add for $name {
      type Output = Self;

      #[inline(always)]
      fn add(self, other: Self) -> Self {
        Self(self.0.wrapping_add(other.0))
      }
    }

    impl AddAssign for $name {
      #[inline(always)]
      fn add_assign(&mut self, other: Self) {
        *self = *self + other;
      }
    }

    impl Sub for $name {
      type Output = Self;

      #[inline(always)]
      fn sub(self, other: Self) -> Self {
        Self(self.0.wrapping_sub(other.0))
      }
    }

    impl SubAssign for $name {
      #[inline(always)]
      fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
      }
    }

    impl Neg for $name {
      type Output = Self;

      #[inline(always)]
      fn neg(self) -> Self {
        Self(self.0.wrapping_neg())
      }
    }
  };
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "nine pairings of a fixed-point width with a float share one rounding rule, and the five that print and serialize share one way of doing it"
  )
)]
macro_rules! float_conv {
  ($name:ident, $to:ident, $from:ident, $ty:ty, $widen:path, $narrow:path) => {
    float_conv!($name, $to, $from, $ty, $widen, $narrow, no_fmt);

    impl fmt::Display for $name {
      fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match f.precision() {
          Some(_) => {
            let Ok(float) = self.$to();

            fmt::Display::fmt(&float, f)
          }
          None => fmt_shortest(i64::from(self.0), Self::FRACT_BITS, f),
        }
      }
    }

    impl fmt::Debug for $name {
      fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
      }
    }

    #[cfg(feature = "serde")]
    impl ::serde::Serialize for $name {
      fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
      where
        S: ::serde::Serializer,
      {
        let Ok(float) = self.$to();

        ::serde::Serialize::serialize(&float, serializer)
      }
    }

    #[cfg(feature = "serde")]
    impl<'de> ::serde::Deserialize<'de> for $name {
      fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
      where
        D: ::serde::Deserializer<'de>,
      {
        let float = <$ty as ::serde::Deserialize>::deserialize(deserializer)?;
        let Ok(held) = $name::$from(float);

        Ok(held)
      }
    }
  };
  ($name:ident, $to:ident, $from:ident, $ty:ty, $widen:path, $narrow:path, no_fmt) => {
    impl $name {
      #[cfg_attr(
        dylint_lib = "explicit067_no_unit_less_quantity",
        expect(
          explicit067_no_unit_less_quantity,
          reason = "this conversion changes the numeric representation of any quantity without assigning it a unit"
        )
      )]
      #[inline(always)]
      pub fn $from(x: $ty) -> Result<Self, Never> {
        let half_step: $ty = 0.5;
        let half = half_step.copysign(x);
        let Ok(one) = $widen(Self::ONE.0);
        let Ok(bits) = $narrow(x * one + half);

        Ok(Self(bits))
      }

      #[inline(always)]
      pub fn $to(self) -> Result<$ty, Never> {
        let Ok(int) = $widen((self.0 & Self::INT_MASK).wrapping_shr(Self::FRACT_BITS));
        let Ok(fract) = $widen(self.0 & !Self::INT_MASK);
        let Ok(one) = $widen(Self::ONE.0);

        Ok(int + fract / one)
      }
    }
  };
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Closeness {
  WithinHalfAStep,
  Further,
}

#[derive(Clone, Copy)]
struct Exact {
  magnitude: u128,
  fract_bits: u32,
}

#[derive(Clone, Copy)]
struct Decimal {
  digits: u128,
  places: u32,
  closeness: Closeness,
}

const MOST_PLACES: u32 = 5;

const TEN: NonZeroU128 = match NonZeroU128::new(10) {
  Some(ten) => ten,
  None => NonZeroU128::MIN,
};

impl Exact {
  fn nearest(self, places: u32) -> Result<Decimal, Never> {
    let pow = 10u128.wrapping_pow(places);
    let one = 1u128.wrapping_shl(self.fract_bits);
    let scaled = self.magnitude.wrapping_mul(pow);
    let digits = scaled
      .wrapping_mul(2)
      .wrapping_add(one)
      .wrapping_shr(self.fract_bits.wrapping_add(1));
    let off = digits.wrapping_mul(one).abs_diff(scaled).wrapping_mul(2);

    Ok(Decimal {
      digits,
      places,
      closeness: match off.cmp(&pow) {
        Ordering::Less => Closeness::WithinHalfAStep,
        Ordering::Equal | Ordering::Greater => Closeness::Further,
      },
    })
  }

  fn shortest(self) -> Result<Decimal, Never> {
    let Ok(finest) = self.nearest(MOST_PLACES);
    let found = (0..MOST_PLACES).find_map(|places| {
      let Ok(decimal) = self.nearest(places);

      match decimal.closeness {
        Closeness::WithinHalfAStep => Some(decimal),
        Closeness::Further => None,
      }
    });

    Ok(match found {
      Some(decimal) => decimal,
      None => finest,
    })
  }
}

struct Written {
  held: [u8; 32],
  length: u32,
}

impl fmt::Write for Written {
  fn write_str(&mut self, said: &str) -> fmt::Result {
    let Ok(added) = fitted::<_, u32>(said.len());
    let end = self.length.checked_add(added).ok_or(fmt::Error)?;
    let Ok(start) = index(self.length);
    let Ok(stop) = index(end);
    let room = self.held.get_mut(start..stop).ok_or(fmt::Error)?;

    for (slot, byte) in room.iter_mut().zip(said.bytes()) {
      *slot = byte;
    }

    self.length = end;

    Ok(())
  }
}

fn fmt_shortest(raw: i64, fract_bits: u32, f: &mut fmt::Formatter) -> fmt::Result {
  let exact = Exact {
    magnitude: u128::from(raw.unsigned_abs()),
    fract_bits,
  };
  let Ok(decimal) = exact.shortest();
  let pow = TEN.saturating_pow(decimal.places);
  let mut written = Written {
    held: [0; 32],
    length: 0,
  };

  match decimal.places {
    0 => {
      write!(written, "{}", decimal.digits)?;
    }
    places => {
      let Ok(width) = index(places);

      write!(
        written,
        "{}.{:0width$}",
        decimal.digits / pow,
        decimal.digits % pow
      )?;
    }
  }

  let Ok(length) = index(written.length);
  let text = written.held.get(..length).ok_or(fmt::Error)?;
  let shown = core::str::from_utf8(text).map_err(|_not_text| fmt::Error)?;

  f.pad_integral(raw >= 0, "", shown)
}

fixed_impl!(F2Dot14, 16, 14, i16);
fixed_impl!(F4Dot12, 16, 12, i16);
fixed_impl!(F6Dot10, 16, 10, i16);
fixed_impl!(Fixed, 32, 16, i32);
fixed_impl!(F26Dot6, 32, 6, i32);
fixed_impl!(F48Dot16, 64, 16, i64);

float_conv!(F2Dot14, to_f32, from_f32, f32, i16_in_f32, f32_toward_i16);
float_conv!(F4Dot12, to_f32, from_f32, f32, i16_in_f32, f32_toward_i16);
float_conv!(F6Dot10, to_f32, from_f32, f32, i16_in_f32, f32_toward_i16);
float_conv!(F48Dot16, to_f64, from_f64, f64, i64_in_f64, toward_zero_i64);
float_conv!(
  F2Dot14,
  to_f64,
  from_f64,
  f64,
  i16_in_f64,
  f64_toward_i16,
  no_fmt
);
float_conv!(
  F4Dot12,
  to_f64,
  from_f64,
  f64,
  i16_in_f64,
  f64_toward_i16,
  no_fmt
);
float_conv!(
  F6Dot10,
  to_f64,
  from_f64,
  f64,
  i16_in_f64,
  f64_toward_i16,
  no_fmt
);
float_conv!(Fixed, to_f64, from_f64, f64, i32_in_f64, toward_zero_i32);
float_conv!(F26Dot6, to_f64, from_f64, f64, i32_in_f64, toward_zero_i32);

crate::raw::newtype_scalar!(F2Dot14, [u8; 2]);
crate::raw::newtype_scalar!(F4Dot12, [u8; 2]);
crate::raw::newtype_scalar!(F6Dot10, [u8; 2]);
crate::raw::newtype_scalar!(Fixed, [u8; 4]);

impl Fixed {
  #[inline]
  pub fn mul_div(&self, a: Self, b: Self) -> Result<Self, Never> {
    let su = u64::from(self.0.unsigned_abs());
    let au = u64::from(a.0.unsigned_abs());
    let bu = u64::from(b.0.unsigned_abs());
    let quotient = match su
      .wrapping_mul(au)
      .wrapping_add(bu.wrapping_shr(1))
      .checked_div(bu)
    {
      Some(quotient) => quotient,
      None => 0x7FFF_FFFF,
    };
    let Ok(narrowed) = low_u32(quotient);
    let signed = narrowed.cast_signed();

    Ok(Self(match self.0 ^ a.0 ^ b.0 {
      ..0 => signed.wrapping_neg(),
      0.. => signed,
    }))
  }

  #[inline(always)]
  pub const fn from_i32(i: i32) -> Result<Self, Never> {
    Ok(Self(i.wrapping_shl(16)))
  }

  #[inline(always)]
  pub const fn to_i32(self) -> Result<i32, Never> {
    Ok(self.0.wrapping_add(0x8000).wrapping_shr(16))
  }

  #[inline(always)]
  pub const fn to_f26dot6(self) -> Result<F26Dot6, Never> {
    Ok(F26Dot6(self.0.wrapping_add(0x200).wrapping_shr(10)))
  }

  #[inline(always)]
  pub const fn to_f2dot14(self) -> Result<F2Dot14, Never> {
    let Ok(narrowed) = low_i16(self.0.wrapping_add(2).wrapping_shr(2));

    Ok(F2Dot14(narrowed))
  }

  #[inline(always)]
  pub fn to_f32(self) -> Result<f32, Never> {
    const SCALE_FACTOR: f32 = 1.0 / 65536.0;

    let Ok(near) = narrow_f32(f64::from(self.0));

    Ok(near * SCALE_FACTOR)
  }

  #[inline(always)]
  pub fn to_f48dot16(self) -> Result<F48Dot16, Never> {
    Ok(F48Dot16(i64::from(self.0)))
  }

  #[inline(always)]
  pub fn apply_delta(self, delta: F48Dot16) -> Result<f32, Never> {
    let Ok(base) = self.to_f32();
    let Ok(quanta) = delta.to_f64();
    let Ok(moved) = narrow_f32(quanta / 65536.0);

    Ok(base + moved)
  }

  #[inline(always)]
  pub fn mul_i32(self, value: i32) -> Result<F48Dot16, Never> {
    Ok(F48Dot16(i64::from(self.0).wrapping_mul(i64::from(value))))
  }
}

impl Mul for Fixed {
  type Output = Self;

  #[inline(always)]
  fn mul(self, other: Self) -> Self::Output {
    let ab = i64::from(self.0).wrapping_mul(i64::from(other.0));
    let rounded = ab
      .wrapping_add(0x8000)
      .wrapping_sub(i64::from(ab < 0))
      .wrapping_shr(16);
    let Ok(narrowed) = low_i32(rounded);

    Self(narrowed)
  }
}

impl Div for Fixed {
  type Output = Self;

  fn div(self, other: Self) -> Self {
    let Ok(quotient) = divided(Dividend {
      value: self.0,
      other: other.0,
      fract_bits: 16,
    });

    Self(quotient)
  }
}

impl Mul for F26Dot6 {
  type Output = Self;

  #[inline(always)]
  fn mul(self, other: Self) -> Self::Output {
    let ab = i64::from(self.0).wrapping_mul(i64::from(other.0));
    let rounded = ab
      .wrapping_add(32)
      .wrapping_sub(i64::from(ab < 0))
      .wrapping_shr(6);
    let Ok(narrowed) = low_i32(rounded);

    Self(narrowed)
  }
}

impl Div for F26Dot6 {
  type Output = Self;

  fn div(self, other: Self) -> Self {
    let Ok(quotient) = divided(Dividend {
      value: self.0,
      other: other.0,
      fract_bits: 6,
    });

    Self(quotient)
  }
}

#[derive(Clone, Copy)]
struct Dividend {
  value: i32,
  other: i32,
  fract_bits: u32,
}

fn divided(dividend: Dividend) -> Result<i32, Never> {
  let au = u64::from(dividend.value.unsigned_abs());
  let bu = u64::from(dividend.other.unsigned_abs());
  let narrowed = match au
    .wrapping_shl(dividend.fract_bits)
    .wrapping_add(bu.wrapping_shr(1))
    .checked_div(bu)
  {
    Some(quotient) => {
      let Ok(low) = low_u32(quotient);

      low
    }
    None => 0x7FFF_FFFF,
  };
  let signed = narrowed.cast_signed();

  Ok(match dividend.value ^ dividend.other {
    ..0 => signed.wrapping_neg(),
    0.. => signed,
  })
}

impl MulAssign for Fixed {
  #[inline(always)]
  fn mul_assign(&mut self, rhs: Self) {
    *self = *self * rhs;
  }
}

impl DivAssign for Fixed {
  #[inline(always)]
  fn div_assign(&mut self, rhs: Self) {
    *self = *self / rhs;
  }
}

impl MulAssign for F26Dot6 {
  #[inline(always)]
  fn mul_assign(&mut self, rhs: Self) {
    *self = *self * rhs;
  }
}

impl DivAssign for F26Dot6 {
  #[inline(always)]
  fn div_assign(&mut self, rhs: Self) {
    *self = *self / rhs;
  }
}

impl From<i32> for Fixed {
  fn from(value: i32) -> Self {
    let Ok(held) = Self::from_i32(value);

    held
  }
}

impl F26Dot6 {
  #[inline(always)]
  pub const fn from_i32(i: i32) -> Result<Self, Never> {
    Ok(Self(i.wrapping_shl(6)))
  }

  #[inline(always)]
  pub const fn to_i32(self) -> Result<i32, Never> {
    Ok(self.0.wrapping_add(32).wrapping_shr(6))
  }

  #[inline(always)]
  pub fn to_f32(self) -> Result<f32, Never> {
    const SCALE_FACTOR: f32 = 1.0 / 64.0;

    let Ok(near) = narrow_f32(f64::from(self.0));

    Ok(near * SCALE_FACTOR)
  }
}

impl F48Dot16 {
  #[inline(always)]
  pub const fn from_i64(i: i64) -> Result<Self, Never> {
    Ok(Self(i.wrapping_shl(16)))
  }

  #[inline(always)]
  pub fn from_i32(i: i32) -> Result<Self, Never> {
    Ok(Self(i64::from(i).wrapping_shl(16)))
  }

  #[inline(always)]
  pub const fn to_i64(self) -> Result<i64, Never> {
    Ok(self.0.wrapping_add(0x8000).wrapping_shr(16))
  }

  #[inline(always)]
  pub const fn to_i32(self) -> Result<i32, Never> {
    let Ok(whole) = self.to_i64();

    low_i32(whole)
  }

  #[inline(always)]
  pub fn to_f32(self) -> Result<f32, Never> {
    const SCALE_FACTOR: f64 = 1.0 / 65536.0;

    let Ok(wide) = self.0.float();

    narrow_f32(wide * SCALE_FACTOR)
  }

  #[inline(always)]
  pub const fn to_fixed(self) -> Result<Fixed, Never> {
    let Ok(narrowed) = low_i32(self.0);

    Ok(Fixed(narrowed))
  }
}

impl F2Dot14 {
  #[inline(always)]
  pub fn apply_delta(self, delta: F48Dot16) -> Result<f32, Never> {
    let Ok(base) = self.to_f32();
    let Ok(quanta) = delta.to_f64();
    let Ok(moved) = narrow_f32(quanta / 16384.0);

    Ok(base + moved)
  }

  #[inline(always)]
  pub fn to_fixed(self) -> Result<Fixed, Never> {
    Ok(Fixed(i32::from(self.0).wrapping_mul(4)))
  }
}
