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

/// A place on the earth, in degrees.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
pub struct Where {
    pub latitude: f64,
    pub longitude: f64,
}

/// What part of the day it is.
///
/// The bounds are the sun's height, and they are the ones photographers and
/// almanacs already use. Civil twilight is six degrees below the horizon: the
/// point at which the sky stops giving enough light to read by, and the oldest
/// answer there is to when evening becomes night. The horizon itself is
/// sunrise and sunset. Six degrees above it is the far end of the golden hour.
///
/// So there are three bands either side of noon and one for the night, and
/// which side of noon a band is on is what separates a dawn from a dusk. They
/// are named for what they look like rather than for their angles, because a
/// picture is chosen by somebody looking out of a window and not by somebody
/// reading an almanac.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sky {
    /// Under civil twilight: dark, and dark for hours.
    Night,
    /// The blue hour before the sun is up.
    Dawn,
    /// The golden hour after it comes up.
    Sunrise,
    /// The sun well up.
    Day,
    /// The golden hour before it goes down.
    Sunset,
    /// The blue hour after it has gone.
    Dusk,
}

impl Sky {
    /// The word this is written as, in the picture table and in the settings.
    pub fn word(&self) -> &'static str {
        match self {
            Sky::Dawn => "dawn",
            Sky::Day => "day",
            Sky::Dusk => "dusk",
            Sky::Night => "night",
            Sky::Sunrise => "sunrise",
            Sky::Sunset => "sunset",
        }
    }

    /// Every one of them, for reading a table back and for saying what a word
    /// could have been.
    pub const EVERY: [Sky; 6] =
        [Sky::Dawn, Sky::Day, Sky::Dusk, Sky::Night, Sky::Sunrise, Sky::Sunset];

    /// From a word.
    pub fn of(word: &str) -> Option<Self> {
        let word = word.trim().to_lowercase();
        Sky::EVERY.into_iter().find(|sky| sky.word() == word)
    }
}

/// What time of the year it is, taken from where the sun is on the ecliptic.
///
/// From the sun rather than from the calendar, which gets two things right for
/// free. The bounds are the equinoxes and the solstices themselves, rather than
/// the first of a month somebody picked to stand near one. And the southern
/// hemisphere gets its own seasons rather than the north's with the wrong names
/// on, which a calendar month cannot do without being told which half of the
/// world it is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Season {
    Autumn,
    Spring,
    Summer,
    Winter,
}

impl Season {
    pub fn word(&self) -> &'static str {
        match self {
            Season::Autumn => "autumn",
            Season::Spring => "spring",
            Season::Summer => "summer",
            Season::Winter => "winter",
        }
    }

    pub const EVERY: [Season; 4] =
        [Season::Autumn, Season::Spring, Season::Summer, Season::Winter];

    pub fn of(word: &str) -> Option<Self> {
        let word = word.trim().to_lowercase();
        Season::EVERY.into_iter().find(|season| season.word() == word)
    }
}

/// Noon of the first day of 2000, which is what the formulae count from.
const EPOCH: f64 = 946_728_000.0;

/// Days since that noon.
fn days(unix: f64) -> f64 {
    (unix - EPOCH) / 86_400.0
}

/// How high the sun is at a place and a moment, in degrees above the horizon.
pub fn height(at: &Where, unix: f64) -> f64 {
    let n = days(unix);
    let turns = |degrees: f64| degrees.rem_euclid(360.0).to_radians();

    let mean_longitude = turns(280.460 + 0.985_647_4 * n);
    let anomaly = turns(357.528 + 0.985_600_3 * n);
    let ecliptic = mean_longitude
        + (1.915_f64).to_radians() * anomaly.sin()
        + (0.020_f64).to_radians() * (2.0 * anomaly).sin();
    let tilt = (23.439 - 0.000_000_4 * n).to_radians();

    let right_ascension = (tilt.cos() * ecliptic.sin()).atan2(ecliptic.cos());
    let declination = (tilt.sin() * ecliptic.sin()).asin();

    // Greenwich sidereal time, which is where the sky has turned to, plus the
    // longitude, which is where the machine has turned to.
    let sidereal = (18.697_374_558 + 24.065_709_824_419_08 * n).rem_euclid(24.0);
    let local = sidereal * 15.0 + at.longitude;
    let hour_angle = (local - right_ascension.to_degrees()).to_radians();

    let latitude = at.latitude.to_radians();
    (latitude.sin() * declination.sin()
        + latitude.cos() * declination.cos() * hour_angle.cos())
    .asin()
    .to_degrees()
}


/// Where the sun is along the ecliptic, in degrees from the spring equinox.
///
/// Zero is the equinox in March, ninety the solstice in June, and so round.
/// This is the same number `height` works out on its way to an angle, and it is
/// the whole of what a season is.
pub fn along_the_year(unix: f64) -> f64 {
    let n = days(unix);
    let turns = |degrees: f64| degrees.rem_euclid(360.0).to_radians();
    let anomaly = turns(357.528 + 0.985_600_3 * n);
    let ecliptic = turns(280.460 + 0.985_647_4 * n)
        + (1.915_f64).to_radians() * anomaly.sin()
        + (0.020_f64).to_radians() * (2.0 * anomaly).sin();
    ecliptic.to_degrees().rem_euclid(360.0)
}

/// What time of the year it is at a place.
pub fn season(at: &Where, unix: f64) -> Season {
    let northern = match (along_the_year(unix) / 90.0) as u32 {
        0 => Season::Spring,
        1 => Season::Summer,
        2 => Season::Autumn,
        _ => Season::Winter,
    };
    if at.latitude >= 0.0 {
        return northern;
    }
    // The far side of the year, which is the far side of the world.
    match northern {
        Season::Autumn => Season::Spring,
        Season::Spring => Season::Autumn,
        Season::Summer => Season::Winter,
        Season::Winter => Season::Summer,
    }
}

/// How high civil twilight is, and how high the golden light gives out.
const TWILIGHT: f64 = -6.0;
const RISEN: f64 = 6.0;

/// What part of the day it is at a place and a moment.
///
/// Whether a band either side of the horizon is a morning or an evening is
/// settled by asking again a quarter of an hour earlier: the sun climbs about
/// two degrees in that time at temperate latitudes and rather less near the
/// poles, but the sign of the change is the same everywhere and the sign is all
/// this reads.
pub fn sky(at: &Where, unix: f64) -> Sky {
    let now = height(at, unix);
    if now < TWILIGHT {
        return Sky::Night;
    }
    if now > RISEN {
        return Sky::Day;
    }
    let rising = now > height(at, unix - 900.0);
    match (rising, now >= 0.0) {
        (true, false) => Sky::Dawn,
        (true, true) => Sky::Sunrise,
        (false, true) => Sky::Sunset,
        (false, false) => Sky::Dusk,
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// 21 June 2026, midday UTC.
    const SOLSTICE: f64 = 1_781_784_000.0;
    /// 21 December 2026, midday UTC.
    const MIDWINTER: f64 = 1_797_681_600.0;
    /// 20 March 2026, midday UTC.
    const EQUINOX: f64 = 1_774_008_000.0;

    fn at(latitude: f64, longitude: f64) -> Where {
        Where { latitude, longitude }
    }

    /// The strongest anchor there is: at the north pole on the longest day the
    /// sun is at the tilt of the earth, and it is there all day round.
    #[test]
    fn the_midsummer_sun_over_the_pole_stands_at_the_tilt_of_the_earth() {
        for hour in 0..24 {
            let height = height(&at(90.0, 0.0), SOLSTICE + f64::from(hour) * 3600.0);
            assert!(
                (height - 23.4).abs() < 0.3,
                "at {hour}:00 the pole's sun was {height} degrees up, not the tilt"
            );
        }
    }

    /// And at midwinter it is the same distance under, all day round.
    #[test]
    fn the_midwinter_sun_over_the_pole_never_comes_up() {
        for hour in 0..24 {
            let height = height(&at(90.0, 0.0), MIDWINTER + f64::from(hour) * 3600.0);
            assert!(height < -22.0, "at {hour}:00 the pole's sun was {height} degrees up");
        }
    }

    #[test]
    fn the_equinox_sun_over_the_meridian_at_noon_is_overhead() {
        let height = height(&at(0.0, 0.0), EQUINOX);
        assert!(height > 87.0, "the equinox noon sun was only {height} degrees up");
    }

    #[test]
    fn the_far_side_of_the_earth_is_in_the_dark() {
        assert_eq!(sky(&at(0.0, 180.0), EQUINOX), Sky::Night);
        assert_eq!(sky(&at(0.0, 0.0), EQUINOX), Sky::Day);
    }

    /// Every band of one day, in the order a day goes through them. This is
    /// the whole of what the table is chosen by, so it is worth walking a real
    /// day minute by minute and asking what it was.
    #[test]
    fn a_day_passes_through_its_bands_in_order() {
        let place = at(50.85, 4.35);
        let midnight = SOLSTICE - 43_200.0;
        let mut seen = Vec::new();
        for minute in 0..1440 {
            let band = sky(&place, midnight + f64::from(minute) * 60.0);
            if seen.last() != Some(&band) {
                seen.push(band);
            }
        }
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

    /// The one thing the height alone cannot say, and the reason it is asked
    /// twice: the same sun at the same height is a morning going up and an
    /// evening coming down.
    #[test]
    fn the_same_height_is_a_morning_going_up_and_an_evening_coming_down() {
        let place = at(52.0, 5.0);
        let level = |from: f64| {
            (0..2880)
                .map(|tick| from + f64::from(tick) * 60.0)
                .find(|moment| (0.0..RISEN).contains(&height(&place, *moment)))
        };
        let morning = level(SOLSTICE - 43_200.0).expect("a morning");
        let evening = level(SOLSTICE).expect("an evening");
        assert_eq!(sky(&place, morning), Sky::Sunrise);
        assert_eq!(sky(&place, evening), Sky::Sunset);
    }

    #[test]
    fn a_part_of_the_day_is_written_and_read_as_the_same_word() {
        for sky in Sky::EVERY {
            assert_eq!(Sky::of(sky.word()), Some(sky));
        }
        assert_eq!(Sky::of("teatime"), None);
    }

    /// The middle of each quarter of the year, well away from the days the
    /// season turns on, because those are an instant and not a date: the
    /// equinox in March 2026 is at a quarter to three in the afternoon, and a
    /// test anchored on noon that day is asserting which side of it noon fell.
    const MID_WINTER: f64 = 1_768_910_400.0;
    const MID_SPRING: f64 = 1_776_686_400.0;
    const MID_SUMMER: f64 = 1_784_548_800.0;
    const MID_AUTUMN: f64 = 1_792_497_600.0;

    #[test]
    fn each_quarter_of_the_year_is_its_own_season() {
        let north = at(50.85, 4.35);
        assert_eq!(season(&north, MID_SPRING), Season::Spring);
        assert_eq!(season(&north, MID_SUMMER), Season::Summer);
        assert_eq!(season(&north, MID_AUTUMN), Season::Autumn);
        assert_eq!(season(&north, MID_WINTER), Season::Winter);
    }

    /// The seasons are bounded by the solstices and the equinoxes themselves
    /// rather than by the first of a month, so the turn falls in the week the
    /// almanac puts it in and not at the end of the month.
    #[test]
    fn the_year_turns_in_the_week_the_solstice_is_in() {
        let north = at(50.85, 4.35);
        // 18 June is spring and 24 June is summer, so the turn is between.
        assert_eq!(season(&north, 1_781_784_000.0), Season::Spring);
        assert_eq!(season(&north, 1_782_302_400.0), Season::Summer);
        // The same either side of the solstice in December.
        assert_eq!(season(&north, 1_797_681_600.0), Season::Autumn);
        assert_eq!(season(&north, 1_798_113_600.0), Season::Winter);
    }

    /// The whole reason the season comes from the sun rather than the calendar:
    /// a snowy picture in July is right in one half of the world and absurd in
    /// the other, and the date alone cannot tell them apart.
    #[test]
    fn the_southern_hemisphere_gets_its_own_seasons() {
        let south = at(-33.9, 151.2);
        assert_eq!(season(&south, MID_SUMMER), Season::Winter);
        assert_eq!(season(&south, MID_WINTER), Season::Summer);
        assert_eq!(season(&south, MID_SPRING), Season::Autumn);
        assert_eq!(season(&south, MID_AUTUMN), Season::Spring);
    }

    #[test]
    fn a_season_is_written_and_read_as_the_same_word() {
        for season in Season::EVERY {
            assert_eq!(Season::of(season.word()), Some(season));
        }
        assert_eq!(Season::of("monsoon"), None);
    }

    /// The sun goes right round the ecliptic in a year and comes back.
    #[test]
    fn the_sun_comes_back_to_where_it_started_after_a_year() {
        let apart = (along_the_year(EQUINOX + 365.2422 * 86_400.0) - along_the_year(EQUINOX)).abs();
        assert!(apart < 1.0 || apart > 359.0, "a year later it was {apart} degrees away");
    }
}
