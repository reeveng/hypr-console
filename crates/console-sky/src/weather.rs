//! What it is doing outside.
//!
//! Open-Meteo, because it wants no account and no key: a machine that is given
//! to somebody has nothing to sign up for and nothing to leak, and a service
//! that asks for a key is a service that stops working the day somebody's key
//! is rotated by a person who has forgotten this device exists.
//!
//! What comes back is a WMO code, which is the number a weather station in any
//! country writes down for what it can see. The codes are finer than a picture
//! needs, so they are grouped here: this table's whole job is to turn ninety-
//! nine numbers into the handful of things there is a wallpaper for.
//!
//! The reading is separate from the fetching, so what the service said can be
//! tested without the service.

use std::sync::atomic::{AtomicBool, Ordering};

/// What a picture is chosen for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Weather {
    Clear,
    Cloud,
    Fog,
    Rain,
    Snow,
    Storm,
}

impl Weather {
    /// The word this is written as, in the picture table and in the settings.
    pub fn word(&self) -> &'static str {
        match self {
            Weather::Clear => "clear",
            Weather::Cloud => "cloud",
            Weather::Fog => "fog",
            Weather::Rain => "rain",
            Weather::Snow => "snow",
            Weather::Storm => "storm",
        }
    }

    /// From a word, for reading the table back.
    pub fn of(word: &str) -> Option<Self> {
        [
            Weather::Clear,
            Weather::Cloud,
            Weather::Fog,
            Weather::Rain,
            Weather::Snow,
            Weather::Storm,
        ]
        .into_iter()
        .find(|weather| weather.word() == word.trim().to_lowercase())
    }

    /// What a WMO present-weather code means to a wallpaper.
    ///
    /// Freezing rain is rain and not snow, because it falls as rain and it is
    /// a wet picture that is wanted rather than a white one. A code nothing
    /// knows about is cloud, which is what most weather is and the least wrong
    /// thing to show when the answer is not understood.
    pub fn of_code(code: u32) -> Self {
        match code {
            0 => Weather::Clear,
            1..=3 => Weather::Cloud,
            45 | 48 => Weather::Fog,
            51..=67 | 80..=82 => Weather::Rain,
            71..=77 | 85 | 86 => Weather::Snow,
            95..=99 => Weather::Storm,
            _ => Weather::Cloud,
        }
    }
}

/// Where the reading comes from, and what is asked of it.
pub const SERVICE: &str = "https://api.open-meteo.com/v1/forecast";

/// The whole question, as a web address.
pub fn asking(at: &crate::sun::Where) -> String {
    format!(
        "{SERVICE}?latitude={:.4}&longitude={:.4}&current=weather_code",
        at.latitude, at.longitude
    )
}

/// What the service said, if it said anything this understands.
pub fn read(answer: &str) -> Option<Weather> {
    let parsed: serde_json::Value = serde_json::from_str(answer).ok()?;
    let code = parsed.get("current")?.get("weather_code")?.as_u64()?;
    Some(Weather::of_code(code as u32))
}

/// Go and look, giving up rather than waiting.
///
/// curl rather than a client of this program's own, because everything else on
/// this desktop that talks to something asks a program that is already on the
/// machine, and a handheld that compiles its own software should not be
/// compiling a TLS stack to find out whether it is raining.
///
/// Nothing here retries and nothing here reports. A wallpaper that cannot find
/// out what the weather is falls back on the time of day, which it can always
/// work out for itself, and tries again at the next reading.
pub fn now(at: &crate::sun::Where) -> Option<Weather> {
    let done = std::process::Command::new("curl")
        .args(["--silent", "--fail", "--max-time", "8", &asking(at)])
        .output();
    let said = match done {
        Ok(said) if said.status.success() => match read(&String::from_utf8_lossy(&said.stdout)) {
            found @ Some(_) => found,
            // A service that answers with something else is the one fault here
            // that looks like weather nobody has a picture for.
            None => complain("open-meteo answered with something this cannot read"),
        },
        Ok(said) => complain(&format!(
            "open-meteo would not answer: curl {}",
            said.status
        )),
        Err(fault) => complain(&format!("the weather could not be asked for: {fault}")),
    };
    ANSWERED.store(said.is_some(), Ordering::Relaxed);
    said
}

/// Whether the last question was answered.
///
/// A machine that is off the network is asked again every minute for as long
/// as it is off the network, and a line for each of those is a journal nobody
/// can read. So the first one after an answer is said and the rest are not.
static ANSWERED: AtomicBool = AtomicBool::new(true);

/// Say a fault the first time, and nothing until it has worked again.
///
/// Not knowing the weather is a state this handles: a picture that names one is
/// simply not chosen while it lasts. What it should not be is invisible, since
/// a wallpaper that never shows rain again looks like a table with a wrong
/// entry in it rather than a machine that cannot reach the network.
fn complain(said: &str) -> Option<Weather> {
    if ANSWERED.swap(false, Ordering::Relaxed) {
        eprintln!("{said}");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clear_sky_and_an_overcast_one_are_told_apart() {
        assert_eq!(Weather::of_code(0), Weather::Clear);
        assert_eq!(Weather::of_code(3), Weather::Cloud);
    }

    /// The groupings that are a decision rather than a reading of the table.
    #[test]
    fn what_falls_as_water_is_rain_however_cold_it_is() {
        // Freezing drizzle and freezing rain.
        assert_eq!(Weather::of_code(56), Weather::Rain);
        assert_eq!(Weather::of_code(66), Weather::Rain);
        // Snow grains, and a shower of snow.
        assert_eq!(Weather::of_code(77), Weather::Snow);
        assert_eq!(Weather::of_code(85), Weather::Snow);
    }

    #[test]
    fn a_thunderstorm_is_its_own_weather() {
        assert_eq!(Weather::of_code(95), Weather::Storm);
        assert_eq!(Weather::of_code(99), Weather::Storm);
    }

    /// A number this table has never met is the commonest weather there is,
    /// which is the least wrong picture to put up.
    #[test]
    fn a_code_nothing_knows_about_is_cloud() {
        assert_eq!(Weather::of_code(4), Weather::Cloud);
        assert_eq!(Weather::of_code(1000), Weather::Cloud);
    }

    #[test]
    fn the_reading_is_taken_out_of_what_the_service_answered() {
        let said = r#"{"latitude":52.0,"current":{"time":"2026-08-29T10:00","weather_code":61}}"#;
        assert_eq!(read(said), Some(Weather::Rain));
    }

    /// Everything that can come back and is not an answer.
    #[test]
    fn an_answer_that_is_not_one_is_no_reading_at_all() {
        assert_eq!(read(""), None);
        assert_eq!(read("<html>down for maintenance</html>"), None);
        assert_eq!(read(r#"{"error":true,"reason":"out of range"}"#), None);
        assert_eq!(read(r#"{"current":{"time":"now"}}"#), None);
    }

    #[test]
    fn the_question_names_the_place_it_is_asked_about() {
        let asked = asking(&crate::sun::Where {
            latitude: 13.7563,
            longitude: 100.5018,
        });
        assert!(asked.contains("latitude=13.7563"), "{asked}");
        assert!(asked.contains("longitude=100.5018"), "{asked}");
    }

    #[test]
    fn a_weather_is_written_and_read_as_the_same_word() {
        for weather in [
            Weather::Clear,
            Weather::Cloud,
            Weather::Fog,
            Weather::Rain,
            Weather::Snow,
            Weather::Storm,
        ] {
            assert_eq!(Weather::of(weather.word()), Some(weather));
        }
        assert_eq!(Weather::of("plague of frogs"), None);
    }
}
