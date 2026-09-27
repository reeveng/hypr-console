//! Where the sun is, worked out rather than asked.
//!
//! What the wallpaper wants to know is whether it is morning, day, evening or
//! night, and that is a question about the sun's height above the horizon at a
//! place. It is arithmetic, so it is done here: no network, no timezone
//! database, and an answer on a machine that has been in a bag for a week and
//! has never heard of the country it was switched on in.
//!
//! Height rather than a table of sunrise and sunset times, because height also
//! answers which side of noon it is: ask twice, a few minutes apart, and a sun
//! that has gone up is a morning and one that has gone down is an evening. A
//! table would need the same two readings to say the same thing.
//!
//! The formulae are the low-precision solar position from the Astronomical
//! Almanac, good to about a hundredth of a degree for a century either side of
//! 2000, which is a great deal more than a wallpaper needs.


use console_core_never::Never;
use console_core_number_conversion::toward_zero_u32;
use console_core_words::Words;
use console_weather::here::Where;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Words)]
pub enum Sky {
    #[words(word = "night")]
    Night,
    #[words(word = "dawn")]
    Dawn,
    #[words(word = "sunrise")]
    Sunrise,
    #[words(word = "day")]
    Day,
    #[words(word = "sunset")]
    Sunset,
    #[words(word = "dusk")]
    Dusk,
}

impl Sky {
    pub const EVERY: [Sky; 6] =
        [Sky::Dawn, Sky::Day, Sky::Dusk, Sky::Night, Sky::Sunrise, Sky::Sunset];

    pub fn of(word: &str) -> Result<Option<Self>, Never> {
        Sky::from_word(&word.trim().to_lowercase())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Words)]
pub enum Season {
    #[words(word = "autumn")]
    Autumn,
    #[words(word = "spring")]
    Spring,
    #[words(word = "summer")]
    Summer,
    #[words(word = "winter")]
    Winter,
}

impl Season {
    pub const EVERY: [Season; 4] =
        [Season::Autumn, Season::Spring, Season::Summer, Season::Winter];

    pub fn of(word: &str) -> Result<Option<Self>, Never> {
        Season::from_word(&word.trim().to_lowercase())
    }
}

const EPOCH: f64 = 946_728_000.0;

fn days(unix: f64) -> Result<f64, Never> {
    Ok((unix - EPOCH) / 86_400.0)
}

pub fn height(at: &Where, unix: f64) -> Result<f64, Never> {
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
    let local = sidereal * 15.0 + at.longitude;
    let hour_angle = (local - right_ascension.to_degrees()).to_radians();

    let latitude = at.latitude.to_radians();

    Ok((latitude.sin() * declination.sin()
        + latitude.cos() * declination.cos() * hour_angle.cos())
    .asin()
    .to_degrees())
}


pub fn along_the_year(unix: f64) -> Result<f64, Never> {
    let days_since_2000 = days(unix)?;

    let turns = |degrees: f64| degrees.rem_euclid(360.0).to_radians();
    let anomaly = turns(357.528 + 0.985_600_3 * days_since_2000);
    let ecliptic = turns(280.460 + 0.985_647_4 * days_since_2000)
        + (1.915_f64).to_radians() * anomaly.sin()
        + (0.020_f64).to_radians() * (2.0 * anomaly).sin();

    Ok(ecliptic.to_degrees().rem_euclid(360.0))
}

pub fn season(at: &Where, unix: f64) -> Result<Season, Never> {
    let along = along_the_year(unix)?;

    let Ok(quarter) = toward_zero_u32(along / 90.0);

    let northern = match quarter {
        0 => Season::Spring,
        1 => Season::Summer,
        2 => Season::Autumn,
        _ => Season::Winter,
    };

    match at.latitude >= 0.0 {
        true => return Ok(northern),
        false => {},
    }

    Ok(match northern {
        Season::Autumn => Season::Spring,
        Season::Spring => Season::Autumn,
        Season::Summer => Season::Winter,
        Season::Winter => Season::Summer,
    })
}

const TWILIGHT: f64 = -6.0;
const RISEN: f64 = 6.0;

pub fn sky(at: &Where, unix: f64) -> Result<Sky, Never> {
    let now = height(at, unix)?;

    match now < TWILIGHT {
        true => return Ok(Sky::Night),
        false => {},
    }

    match now > RISEN {
        true => return Ok(Sky::Day),
        false => {},
    }

    let before = height(at, unix - 900.0)?;

    let rising = now > before;

    Ok(match (rising, now >= 0.0) {
        (true, false) => Sky::Dawn,
        (true, true) => Sky::Sunrise,
        (false, true) => Sky::Sunset,
        (false, false) => Sky::Dusk,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOLSTICE: f64 = 1_781_784_000.0;
    const MIDWINTER: f64 = 1_797_681_600.0;
    const EQUINOX: f64 = 1_774_008_000.0;

    const POLE: Where = Where { latitude: 90.0, longitude: 0.0 };
    const MERIDIAN: Where = Where { latitude: 0.0, longitude: 0.0 };
    const FAR_SIDE: Where = Where { latitude: 0.0, longitude: 180.0 };
    const BRUSSELS: Where = Where { latitude: 50.85, longitude: 4.35 };
    const UTRECHT: Where = Where { latitude: 52.0, longitude: 5.0 };
    const SYDNEY: Where = Where { latitude: -33.9, longitude: 151.2 };

    #[test]
    fn the_midsummer_sun_over_the_pole_stands_at_the_tilt_of_the_earth() {
        for hour in 0..24 {
            let Ok(up) = height(&POLE, SOLSTICE + f64::from(hour) * 3600.0);

            assert!(
                (up - 23.4).abs() < 0.3,
                "at {hour}:00 the pole's sun was {up} degrees up, not the tilt"
            );
        }
    }

    #[test]
    fn the_midwinter_sun_over_the_pole_never_comes_up() {
        for hour in 0..24 {
            let Ok(up) = height(&POLE, MIDWINTER + f64::from(hour) * 3600.0);

            assert!(up < -22.0, "at {hour}:00 the pole's sun was {up} degrees up");
        }
    }

    #[test]
    fn the_equinox_sun_over_the_meridian_at_noon_is_overhead() {
        let Ok(up) = height(&MERIDIAN, EQUINOX);

        assert!(up > 87.0, "the equinox noon sun was only {up} degrees up");
    }

    #[test]
    fn the_far_side_of_the_earth_is_in_the_dark() {
        assert_eq!(sky(&FAR_SIDE, EQUINOX), Ok(Sky::Night));
        assert_eq!(sky(&MERIDIAN, EQUINOX), Ok(Sky::Day));
    }

    #[test]
    fn a_day_passes_through_its_bands_in_order() {
        let midnight = SOLSTICE - 43_200.0;
        let mut seen: Vec<Sky> = (0..1440)
            .map(|minute| {
                let Ok(band) = sky(&BRUSSELS, midnight + f64::from(minute) * 60.0);

                band
            })
            .collect();

        seen.dedup();

        assert_eq!(
            seen,
            [
                Sky::Night,
                Sky::Dawn,
                Sky::Sunrise,
                Sky::Day,
                Sky::Sunset,
                Sky::Dusk,
                Sky::Night,
            ]
        );
    }

    #[test]
    fn the_same_height_is_a_morning_going_up_and_an_evening_coming_down() -> Result<(), &'static str> {
        let level = |from: f64| {
            (0..2880)
                .map(|tick| from + f64::from(tick) * 60.0)
                .find(|moment| {
                    let Ok(up) = height(&UTRECHT, *moment);

                    (0.0..RISEN).contains(&up)
                })
        };
        let morning = level(SOLSTICE - 43_200.0).ok_or("a morning")?;
        let evening = level(SOLSTICE).ok_or("an evening")?;

        assert_eq!(sky(&UTRECHT, morning), Ok(Sky::Sunrise));
        assert_eq!(sky(&UTRECHT, evening), Ok(Sky::Sunset));

        Ok(())
    }

    #[test]
    fn a_part_of_the_day_is_written_and_read_as_the_same_word() {
        for sky in Sky::EVERY {
            let Ok(word) = sky.word();

            assert_eq!(Sky::of(word), Ok(Some(sky)));
        }

        assert_eq!(Sky::of("teatime"), Ok(None));
    }

    const MID_WINTER: f64 = 1_768_910_400.0;
    const MID_SPRING: f64 = 1_776_686_400.0;
    const MID_SUMMER: f64 = 1_784_548_800.0;
    const MID_AUTUMN: f64 = 1_792_497_600.0;

    #[test]
    fn each_quarter_of_the_year_is_its_own_season() {
        assert_eq!(season(&BRUSSELS, MID_SPRING), Ok(Season::Spring));
        assert_eq!(season(&BRUSSELS, MID_SUMMER), Ok(Season::Summer));
        assert_eq!(season(&BRUSSELS, MID_AUTUMN), Ok(Season::Autumn));
        assert_eq!(season(&BRUSSELS, MID_WINTER), Ok(Season::Winter));
    }

    #[test]
    fn the_year_turns_in_the_week_the_solstice_is_in() {
        assert_eq!(season(&BRUSSELS, 1_781_784_000.0), Ok(Season::Spring));
        assert_eq!(season(&BRUSSELS, 1_782_302_400.0), Ok(Season::Summer));
        assert_eq!(season(&BRUSSELS, 1_797_681_600.0), Ok(Season::Autumn));
        assert_eq!(season(&BRUSSELS, 1_798_113_600.0), Ok(Season::Winter));
    }

    #[test]
    fn the_southern_hemisphere_gets_its_own_seasons() {
        assert_eq!(season(&SYDNEY, MID_SUMMER), Ok(Season::Winter));
        assert_eq!(season(&SYDNEY, MID_WINTER), Ok(Season::Summer));
        assert_eq!(season(&SYDNEY, MID_SPRING), Ok(Season::Autumn));
        assert_eq!(season(&SYDNEY, MID_AUTUMN), Ok(Season::Spring));
    }

    #[test]
    fn a_season_is_written_and_read_as_the_same_word() {
        for season in Season::EVERY {
            let Ok(word) = season.word();

            assert_eq!(Season::of(word), Ok(Some(season)));
        }

        assert_eq!(Season::of("monsoon"), Ok(None));
    }

    #[test]
    fn the_sun_comes_back_to_where_it_started_after_a_year() {
        let Ok(later) = along_the_year(EQUINOX + 365.2422 * 86_400.0);
        let Ok(now) = along_the_year(EQUINOX);
        let apart = (later - now).abs();

        assert!(apart < 1.0 || apart > 359.0, "a year later it was {apart} degrees away");
    }
}
