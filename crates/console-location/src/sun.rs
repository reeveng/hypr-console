//! The Astronomical Almanac's low-precision solar position.

use console_core_never::Never;

use crate::Coordinates;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this is wall-clock seconds since the Unix epoch"
  )
)]
pub struct UnixSeconds(pub f64);

impl From<f64> for UnixSeconds {
  fn from(seconds: f64) -> Self {
    Self(seconds)
  }
}

const EPOCH: f64 = 946_728_000.0;

fn days(unix: UnixSeconds) -> Result<f64, Never> {
  Ok((unix.0 - EPOCH) / 86_400.0)
}

pub fn height(at: &Coordinates, unix: UnixSeconds) -> Result<f64, Never> {
  let days_since_2000 = days(unix)?;

  let turns = |degrees: f64| degrees.rem_euclid(360.0).to_radians();

  let mean_longitude = turns(280.460 + 0.985_647_4 * days_since_2000);
  let anomaly = turns(357.528 + 0.985_600_3 * days_since_2000);
  let ecliptic = mean_longitude
    + (1.915_f64).to_radians() * anomaly.sin()
    + (0.020_f64).to_radians() * (2.0 * anomaly).sin();
  let tilt = (23.439 - 0.000_000_4 * days_since_2000).to_radians();

  let right_ascension = (tilt.cos() * ecliptic.sin()).atan2(ecliptic.cos());
  let declination = (tilt.sin() * ecliptic.sin()).asin();

  let sidereal = (18.697_374_558 + 24.065_709_824_419_08 * days_since_2000).rem_euclid(24.0);
  let local = sidereal * 15.0 + at.longitude.0;
  let hour_angle = (local - right_ascension.to_degrees()).to_radians();

  let latitude = at.latitude.0.to_radians();

  Ok(
    (latitude.sin() * declination.sin() + latitude.cos() * declination.cos() * hour_angle.cos())
      .clamp(-1.0, 1.0)
      .asin()
      .to_degrees(),
  )
}

pub fn along_the_year(unix: UnixSeconds) -> Result<f64, Never> {
  let days_since_2000 = days(unix)?;

  let turns = |degrees: f64| degrees.rem_euclid(360.0).to_radians();
  let anomaly = turns(357.528 + 0.985_600_3 * days_since_2000);
  let ecliptic = turns(280.460 + 0.985_647_4 * days_since_2000)
    + (1.915_f64).to_radians() * anomaly.sin()
    + (0.020_f64).to_radians() * (2.0 * anomaly).sin();

  Ok(ecliptic.to_degrees().rem_euclid(360.0))
}
