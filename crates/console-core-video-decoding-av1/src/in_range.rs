#![forbid(unsafe_code)]
use core::{cmp::Ordering, fmt};

use console_core_never::Never;

use crate::src::const_fn::const_for;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct InRange<T, const MIN: u128, const MAX: u128>(T);

impl<T, const MIN: u128, const MAX: u128> InRange<T, MIN, MAX>
where
  T: Copy,
  u128: From<T>,
{
  pub fn new(value: T) -> Result<Option<Self>, Never> {
    let wide_value = u128::from(value);

    Ok(match (wide_value.cmp(&MIN), wide_value.cmp(&MAX)) {
      (Ordering::Less, _) | (_, Ordering::Greater) => None,
      (Ordering::Equal | Ordering::Greater, Ordering::Equal | Ordering::Less) => Some(Self(value)),
    })
  }
}

impl<T, const MIN: u128, const MAX: u128> InRange<T, MIN, MAX> {
  pub fn get(self) -> Result<T, Never> {
    Ok(self.0)
  }
}

impl<T: From<u8>, const MAX: u128> Default for InRange<T, 0, MAX> {
  fn default() -> Self {
    Self(T::from(0u8))
  }
}

impl<T: fmt::Display, const MIN: u128, const MAX: u128> fmt::Display for InRange<T, MIN, MAX> {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(to, "{}", self.0)
  }
}

impl<const MIN: u128, const MAX: u128> InRange<u8, MIN, MAX> {
  pub const fn constant(value: u8) -> Result<Self, Never> {
    let wide_value = console_core_number_conversion::const_wrapped_integer!(unsigned u8 => u128, value);
    assert!(wide_value >= MIN && wide_value <= MAX);
    Ok(Self(value))
  }
}

impl<const MAX: u128> InRange<u16, 0, MAX> {
  #[cfg_attr(
    dylint_lib = "explicit011_no_as_cast",
    expect(explicit011_no_as_cast, reason = "the compile-time scan table range check widens a u16 exactly, and From is unavailable in constant evaluation")
  )]
  pub const fn new_array<const N: usize>(coefficients: [u16; N]) -> Result<[Self; N], Never> {
    let mut validated = [Self(0); N];
    const_for!(index in 0..N => {
      let coefficient = coefficients[index];
      assert!((coefficient as u128) <= MAX);
      validated[index] = Self(coefficient);
    });
    Ok(validated)
  }
}
