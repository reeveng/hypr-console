//! Calculate the scheduled white point from the sun or the fallback clock.
//!
//! Sunrise and sunset use the apparent horizon for the sun's upper edge.
//! https://aa.usno.navy.mil/faq/RST_defs
//! When dawn overlaps dusk, the lower fraction of warmth determines the color.

use std::cmp::Ordering;

use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_location::Coordinates;
use console_location::sun::{self, UnixSeconds};

use crate::{Along, DAWN, DAYLIGHT, DUSK, Kelvin, Step, Temperature, WARM, interpolate};

const HORIZON_DEGREES: f64 = -0.833;
const SECONDS_PER_MINUTE: f64 = 60.0;

pub fn at_utc(curve: &[Step], unix_seconds: i64) -> Result<Temperature, Never> {
  let Ok(minute) = fitted::<i64, u32>(unix_seconds.rem_euclid(86_400).saturating_div(60));
  let step = curve
    .iter()
    .filter(|step| step.at <= minute)
    .max_by_key(|step| step.at)
    .or_else(|| curve.iter().max_by_key(|step| step.at));

  Ok(match step {
    Some(active) => active.temperature,
    None => Temperature::Neutral,
  })
}

pub fn at_location(place: &Coordinates, unix: UnixSeconds) -> Result<Temperature, Never> {
  let Ok(now) = sun::height(place, unix);

  match (now >= HORIZON_DEGREES).then_some(()) {
    Some(()) => return Ok(Temperature::Neutral),
    None => {}
  }

  let dusk_minutes = DUSK.1.saturating_sub(DUSK.0);
  let Ok(since_sunset) = elapsed_to_sun(place, unix, Direction::Before, dusk_minutes);
  let Ok(until_sunrise) = elapsed_to_sun(place, unix, Direction::After, DAWN);

  let dusk = since_sunset.saturating_mul(DAWN);
  let dawn = until_sunrise.saturating_mul(dusk_minutes);
  let along = match dusk.cmp(&dawn) {
    Ordering::Less | Ordering::Equal => Along {
      part: since_sunset,
      whole: dusk_minutes,
    },
    Ordering::Greater => Along {
      part: until_sunrise,
      whole: DAWN,
    },
  };
  let Ok(kelvin) = interpolate(
    Kelvin {
      from: DAYLIGHT,
      to: WARM,
    },
    along,
  );

  Ok(Temperature::Kelvin(kelvin))
}

#[derive(Clone, Copy)]
enum Direction {
  Before,
  After,
}

fn elapsed_to_sun(
  place: &Coordinates,
  unix: UnixSeconds,
  direction: Direction,
  minutes: u32,
) -> Result<u32, Never> {
  let step = match direction {
    Direction::Before => -SECONDS_PER_MINUTE,
    Direction::After => SECONDS_PER_MINUTE,
  };
  let daylight = (1..minutes).find(|minute| {
    let time = UnixSeconds(unix.0 + f64::from(*minute) * step);
    let Ok(height) = sun::height(place, time);

    height >= HORIZON_DEGREES
  });

  Ok(match daylight {
    Some(found) => found,
    None => minutes,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_location::{Latitude, Longitude};

  const EQUINOX_NOON: f64 = 1_774_008_000.0;
  const BRUSSELS: Coordinates = Coordinates {
    latitude: Latitude(50.85),
    longitude: Longitude(4.35),
  };
  const OTHER_SIDE: Coordinates = Coordinates {
    latitude: Latitude(-50.85),
    longitude: Longitude(-175.65),
  };

  #[test]
  fn the_same_time_can_be_daylight_or_night_by_location() {
    assert_eq!(
      at_location(&BRUSSELS, UnixSeconds(EQUINOX_NOON)),
      Ok(Temperature::Neutral)
    );
    assert_eq!(
      at_location(&OTHER_SIDE, UnixSeconds(EQUINOX_NOON)),
      Ok(Temperature::Kelvin(WARM))
    );
  }

  #[test]
  fn the_curve_warms_after_sunset_and_cools_before_sunrise() {
    let kelvin = |hours_from_noon| match at_location(
      &BRUSSELS,
      UnixSeconds(EQUINOX_NOON + hours_from_noon * 3600.0),
    ) {
      Ok(Temperature::Kelvin(kelvin)) => kelvin,
      Ok(Temperature::Neutral) => DAYLIGHT,
    };

    assert_eq!(kelvin(5.0), DAYLIGHT);
    assert!((WARM..DAYLIGHT).contains(&kelvin(7.0)));
    assert!(kelvin(6.5) > kelvin(7.0));
    assert_eq!(kelvin(9.0), WARM);
    assert_eq!(kelvin(17.0), WARM);
    assert!(kelvin(17.5) < kelvin(17.67));
    assert_eq!(kelvin(18.0), DAYLIGHT);
  }

  #[test]
  fn fallback_wraps_across_midnight() {
    const MORNING: i64 = 25_200;
    const DUSK_START: i64 = 70_200;
    let Ok(curve) = crate::curve();

    assert_eq!(at_utc(&curve, 0), Ok(Temperature::Kelvin(WARM)));
    assert_eq!(at_utc(&curve, -60), Ok(Temperature::Kelvin(WARM)));
    assert_eq!(at_utc(&curve, MORNING), Ok(Temperature::Neutral));
    assert_eq!(
      at_utc(&curve, DUSK_START),
      Ok(Temperature::Kelvin(DAYLIGHT))
    );
  }
}
