//! Integer conversion that retains the low destination bits.
//!
//! Signed sources extend their sign to 128 bits before narrowing. Destinations
//! reinterpret their low bytes, so negative values and discarded high bits follow
//! the same rule as Rust integer `as`, independently of the numeric range.
//!
//! `wrapped_index` and `wrapped_offset` retain the native-width low bits for a
//! standard-library index or offset. Runtime quantities remain at a fixed width
//! until that boundary. The separate `index` conversion saturates at its bounds.
//!
//! `nearest_f32_from_u64` rounds once to the nearest IEEE-754 single-precision
//! value, with ties to even. Building the representation directly preserves the
//! integer cast's rounding when an intermediate f64 would round twice.

use crate::Sign;
use console_core_never::Never;

enum Rounding {
  Increment,
  Keep,
}

impl From<bool> for Rounding {
  fn from(round_up: bool) -> Self {
    match bool::then_some(round_up, ()) {
      Some(()) => Self::Increment,
      None => Self::Keep,
    }
  }
}

pub trait IntegerBits {
  fn integer_bits(self) -> Result<u128, Never>;
}

pub trait FromIntegerBits: Sized {
  fn from_integer_bits(bits: u128) -> Result<Self, Never>;
}

pub fn wrapped<F: IntegerBits, T: FromIntegerBits>(value: F) -> Result<T, Never> {
  let Ok(bits) = value.integer_bits();

  T::from_integer_bits(bits)
}

impl IntegerBits for u8 {
  fn integer_bits(self) -> Result<u128, Never> {
    Ok(u128::from(self))
  }
}

impl IntegerBits for u16 {
  fn integer_bits(self) -> Result<u128, Never> {
    Ok(u128::from(self))
  }
}

impl IntegerBits for u32 {
  fn integer_bits(self) -> Result<u128, Never> {
    Ok(u128::from(self))
  }
}

impl IntegerBits for u64 {
  fn integer_bits(self) -> Result<u128, Never> {
    Ok(u128::from(self))
  }
}

impl IntegerBits for u128 {
  fn integer_bits(self) -> Result<u128, Never> {
    Ok(self)
  }
}

impl IntegerBits for i8 {
  fn integer_bits(self) -> Result<u128, Never> {
    let signed = i128::from(self);

    Ok(u128::from_le_bytes(signed.to_le_bytes()))
  }
}

impl IntegerBits for i16 {
  fn integer_bits(self) -> Result<u128, Never> {
    let signed = i128::from(self);

    Ok(u128::from_le_bytes(signed.to_le_bytes()))
  }
}

impl IntegerBits for i32 {
  fn integer_bits(self) -> Result<u128, Never> {
    let signed = i128::from(self);

    Ok(u128::from_le_bytes(signed.to_le_bytes()))
  }
}

impl IntegerBits for i64 {
  fn integer_bits(self) -> Result<u128, Never> {
    let signed = i128::from(self);

    Ok(u128::from_le_bytes(signed.to_le_bytes()))
  }
}

impl IntegerBits for i128 {
  fn integer_bits(self) -> Result<u128, Never> {
    Ok(u128::from_le_bytes(self.to_le_bytes()))
  }
}

impl IntegerBits for usize {
  fn integer_bits(self) -> Result<u128, Never> {
    super::fitted::<usize, u128>(self)
  }
}

impl IntegerBits for isize {
  fn integer_bits(self) -> Result<u128, Never> {
    let Ok(signed) = super::fitted::<isize, i128>(self);

    Ok(u128::from_le_bytes(signed.to_le_bytes()))
  }
}

impl FromIntegerBits for u8 {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a]))
  }
}

impl FromIntegerBits for i8 {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a]))
  }
}

impl FromIntegerBits for u16 {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a, b]))
  }
}

impl FromIntegerBits for i16 {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a, b]))
  }
}

impl FromIntegerBits for u32 {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, c, d, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a, b, c, d]))
  }
}

impl FromIntegerBits for i32 {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, c, d, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a, b, c, d]))
  }
}

impl FromIntegerBits for u64 {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, c, d, e, f, g, h, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a, b, c, d, e, f, g, h]))
  }
}

impl FromIntegerBits for i64 {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, c, d, e, f, g, h, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a, b, c, d, e, f, g, h]))
  }
}

impl FromIntegerBits for u128 {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([
      a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p,
    ]))
  }
}

impl FromIntegerBits for i128 {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([
      a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p,
    ]))
  }
}

#[cfg(target_pointer_width = "16")]
impl FromIntegerBits for usize {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a, b]))
  }
}

#[cfg(target_pointer_width = "32")]
impl FromIntegerBits for usize {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, c, d, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a, b, c, d]))
  }
}

#[cfg(target_pointer_width = "64")]
impl FromIntegerBits for usize {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, c, d, e, f, g, h, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a, b, c, d, e, f, g, h]))
  }
}

#[cfg(target_pointer_width = "16")]
impl FromIntegerBits for isize {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a, b]))
  }
}

#[cfg(target_pointer_width = "32")]
impl FromIntegerBits for isize {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, c, d, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a, b, c, d]))
  }
}

#[cfg(target_pointer_width = "64")]
impl FromIntegerBits for isize {
  fn from_integer_bits(bits: u128) -> Result<Self, Never> {
    let [a, b, c, d, e, f, g, h, ..] = bits.to_le_bytes();

    Ok(Self::from_le_bytes([a, b, c, d, e, f, g, h]))
  }
}

pub fn wrapped_index<F: IntegerBits>(value: F) -> Result<usize, Never> {
  wrapped(value)
}

pub fn wrapped_offset<F: IntegerBits>(value: F) -> Result<isize, Never> {
  wrapped(value)
}

pub fn nearest_f32_from_i64(value: i64) -> Result<f32, Never> {
  let Ok(magnitude) = nearest_f32_from_u64(value.unsigned_abs());

  Ok(match Sign::from(value.is_negative()) {
    Sign::Negative => -magnitude,
    Sign::Nonnegative => magnitude,
  })
}

pub fn nearest_f32_from_isize(value: isize) -> Result<f32, Never> {
  let Ok(fixed_width) = wrapped(value);

  nearest_f32_from_i64(fixed_width)
}

pub fn nearest_f32_from_usize(value: usize) -> Result<f32, Never> {
  let Ok(fixed_width) = wrapped(value);

  nearest_f32_from_u64(fixed_width)
}

pub fn nearest_f32_from_u64(value: u64) -> Result<f32, Never> {
  match value {
    0 => return Ok(0.0),
    1.. => {}
  }

  let exponent = 63u32.wrapping_sub(value.leading_zeros());
  let mut biased_exponent = exponent.wrapping_add(127);
  let significand = match exponent {
    24.. => {
      let shift = exponent.wrapping_sub(23);
      let retained = value.wrapping_shr(shift);
      let remainder = value & 1u64.wrapping_shl(shift).wrapping_sub(1);
      let halfway = 1u64.wrapping_shl(shift.wrapping_sub(1));
      let rounding = Rounding::from(
        remainder > halfway || (remainder == halfway && retained & 1 != 0),
      );
      let increment = match rounding {
        Rounding::Increment => 1,
        Rounding::Keep => 0,
      };

      retained.wrapping_add(increment)
    }
    0..=23 => value.wrapping_shl(23u32.wrapping_sub(exponent)),
  };
  let normalized = match significand {
    0x100_0000 => {
      biased_exponent = biased_exponent.wrapping_add(1);

      significand.wrapping_shr(1)
    }
    remaining => remaining,
  };
  let representation = u64::from(biased_exponent).wrapping_shl(23) | (normalized & 0x7f_ffff);
  let Ok(bits) = wrapped::<u64, u32>(representation);

  Ok(f32::from_bits(bits))
}
