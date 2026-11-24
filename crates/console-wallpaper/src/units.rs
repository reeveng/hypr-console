//! The units used by the astronomical and rendering calculations.

pub use console_location::sun::UnixSeconds;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this is a relative proportion used for blending or measuring an area"
  )
)]
pub struct Fraction(pub f64);

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this is a relative interval in seconds and retains fractional frame timing"
  )
)]
pub struct Seconds(pub f64);

impl<'de> serde::Deserialize<'de> for Fraction {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    finite(deserializer).map(Self)
  }
}

impl<'de> serde::Deserialize<'de> for Seconds {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    finite(deserializer).map(Self)
  }
}

fn finite<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
  let value = <f64 as serde::Deserialize>::deserialize(deserializer)?;

  match value.is_finite().then_some(()) {
    Some(()) => Ok(value),
    None => Err(serde::de::Error::custom(
      "a wallpaper quantity must be finite",
    )),
  }
}
