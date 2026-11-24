//! Geographic coordinates, timezone-based location lookup, and solar position.
//! Latitude and longitude give one point in signed decimal degrees.

pub mod sun;
pub mod timezone;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coordinates {
  pub latitude: Latitude,
  pub longitude: Longitude,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped value gives latitude in decimal degrees north"
  )
)]
pub struct Latitude(pub f64);

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped value gives longitude in decimal degrees east"
  )
)]
pub struct Longitude(pub f64);
