//! What it is doing outside.
//!
//! Open-Meteo, because it wants no account and no key: a machine that is given
//! to someone has nothing to sign up for and nothing to leak, and a service
//! that asks for a key is a service that stops working the day someone's key
//! is rotated by a person who has forgotten this device exists.
//!
//! What comes back is a WMO code, which is the number a weather station in any
//! country writes down for what it can see. The codes are finer than a picture
//! needs, so they are grouped here: this table's whole job is to turn ninety-
//! nine numbers into the handful of things there is a wallpaper for.
//!
//! A person reading a forecast wants the finer words, so `named` keeps them:
//! the ones Apple's Weather writes, since that is the forecast most people have
//! already learned to read. A code with no word is `None` rather than a guess,
//! and the panel leaves the line out rather than calling a sky it cannot read
//! cloudy.
//!
//! The reading is separate from the fetching, so what the service said can be
//! tested without the service.

use std::fmt;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_words::Words;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Words)]
pub enum Weather {
    #[words(word = "clear")]
    Clear,
    #[words(word = "cloud")]
    Cloud,
    #[words(word = "fog")]
    Fog,
    #[words(word = "rain")]
    Rain,
    #[words(word = "snow")]
    Snow,
    #[words(word = "storm")]
    Storm,
}

impl Weather {
    pub const EVERY: [Weather; 6] = [
        Weather::Clear,
        Weather::Cloud,
        Weather::Fog,
        Weather::Rain,
        Weather::Snow,
        Weather::Storm,
    ];

    pub fn of(word: &str) -> Result<Option<Self>, Never> {
        Weather::from_word(&word.trim().to_lowercase())
    }

    pub fn of_code(code: u32) -> Result<Self, Never> {
        Ok(match code {
            0 => Weather::Clear,
            1..=3 => Weather::Cloud,
            45 | 48 => Weather::Fog,
            51..=67 | 80..=82 => Weather::Rain,
            71..=77 | 85 | 86 => Weather::Snow,
            95..=99 => Weather::Storm,
            _ => Weather::Cloud,
        })
    }
}

pub fn named(code: u32) -> Result<Option<&'static str>, Never> {
    Ok(Some(match code {
        0 => "Clear",
        1 => "Mostly Clear",
        2 => "Partly Cloudy",
        3 => "Cloudy",
        45 | 48 => "Foggy",
        51..=55 => "Drizzle",
        56 | 57 => "Freezing Drizzle",
        61 | 63 => "Rain",
        65 => "Heavy Rain",
        66 | 67 => "Freezing Rain",
        71 | 73 => "Snow",
        75 => "Heavy Snow",
        77 => "Flurries",
        80 | 81 => "Showers",
        82 => "Heavy Showers",
        85 | 86 => "Snow Showers",
        95 => "Thunderstorms",
        96 | 99 => "Hail",
        _ => return Ok(None),
    }))
}

pub const SERVICE: &str = "https://api.open-meteo.com/v1/forecast";

pub fn asking(at: &crate::here::Where) -> Result<String, Never> {
    Ok(format!(
        "{SERVICE}?latitude={:.4}&longitude={:.4}&current=weather_code",
        at.latitude, at.longitude
    ))
}

pub fn read(answer: &str) -> Result<Option<Weather>, Never> {
    let parsed: serde_json::Value = match serde_json::from_str(answer) {
        Ok(parsed) => parsed,
        Err(fault) => {
            eprintln!("console-weather: the weather service answered with something that is not JSON: {fault}");

            return Ok(None);
        }
    };

    let current = match parsed.get("current") {
        Some(current) => current,
        None => return Ok(None),
    };

    let said = match current.get("weather_code") {
        Some(said) => said,
        None => return Ok(None),
    };

    let code = match said.as_u64() {
        Some(code) => code,
        None => return Ok(None),
    };

    let Ok(code) = fitted(code);

    let weather = Weather::of_code(code)?;

    Ok(Some(weather))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Notified {
    #[default]
    NotYet,
    Already,
}

pub fn now(at: &crate::here::Where, told: &mut Notified) -> Result<Option<Weather>, Never> {
    let asked = asking(at)?;

    let said = match answer(&asked) {
        Ok(said) => {
            let read = read(&said)?;

            match read {
                found @ Some(_) => found,
                None => complain(told, "open-meteo answered with something this cannot read")?,
            }
        }
        Err(fault) => complain(told, &fault.to_string())?,
    };

    match said {
        Some(_) => *told = Notified::NotYet,
        None => {},
    }

    Ok(said)
}

#[derive(Debug)]
pub enum FetchError {
    NotAsked(std::io::Error),
    Failed(std::process::ExitStatus),
}

impl fmt::Display for FetchError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::NotAsked(fault) => write!(to, "the weather could not be asked for: {fault}"),
            FetchError::Failed(status) => write!(to, "open-meteo would not answer: curl {status}"),
        }
    }
}

pub fn answer(address: &str) -> Result<String, FetchError> {
    let Ok(mut asking) = Program::Curl.command();

    let done = asking
        .args(["--silent", "--fail", "--max-time", "8", address])
        .output();

    match done {
        Ok(said) => match said.status.success() {
            true => Ok(String::from_utf8_lossy(&said.stdout).into_owned()),
            false => Err(FetchError::Failed(said.status)),
        },
        Err(fault) => Err(FetchError::NotAsked(fault)),
    }
}

fn complain(told: &mut Notified, said: &str) -> Result<Option<Weather>, Never> {
    match *told {
        Notified::NotYet => {
            *told = Notified::Already;

            eprintln!("{said}");
        }
        Notified::Already => {},
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clear_sky_and_an_overcast_one_are_told_apart() {
        assert_eq!(Weather::of_code(0), Ok(Weather::Clear));
        assert_eq!(Weather::of_code(3), Ok(Weather::Cloud));
    }

    #[test]
    fn what_falls_as_water_is_rain_however_cold_it_is() {
        assert_eq!(Weather::of_code(56), Ok(Weather::Rain));
        assert_eq!(Weather::of_code(66), Ok(Weather::Rain));
        assert_eq!(Weather::of_code(77), Ok(Weather::Snow));
        assert_eq!(Weather::of_code(85), Ok(Weather::Snow));
    }

    #[test]
    fn a_thunderstorm_is_its_own_weather() {
        assert_eq!(Weather::of_code(95), Ok(Weather::Storm));
        assert_eq!(Weather::of_code(99), Ok(Weather::Storm));
    }

    #[test]
    fn a_code_nothing_knows_about_is_cloud() {
        assert_eq!(Weather::of_code(4), Ok(Weather::Cloud));
        assert_eq!(Weather::of_code(1000), Ok(Weather::Cloud));
    }

    #[test]
    fn the_reading_is_taken_out_of_what_the_service_answered() {
        let said = r#"{"latitude":52.0,"current":{"time":"2026-08-29T10:00","weather_code":61}}"#;
        assert_eq!(read(said), Ok(Some(Weather::Rain)));
    }

    #[test]
    fn an_answer_that_is_not_one_is_no_reading_at_all() {
        assert_eq!(read(""), Ok(None));
        assert_eq!(read("<html>down for maintenance</html>"), Ok(None));
        assert_eq!(read(r#"{"error":true,"reason":"out of range"}"#), Ok(None));
        assert_eq!(read(r#"{"current":{"time":"now"}}"#), Ok(None));
    }

    #[test]
    fn the_question_names_the_place_it_is_asked_about() {
        let Ok(asked) = asking(&crate::here::Where {
            latitude: 13.7563,
            longitude: 100.5018,
        });

        assert!(asked.contains("latitude=13.7563"), "{asked}");
        assert!(asked.contains("longitude=100.5018"), "{asked}");
    }

    #[test]
    fn a_weather_is_written_and_read_as_the_same_word() {
        for weather in Weather::EVERY {
            let Ok(word) = weather.word();

            assert_eq!(Weather::of(word), Ok(Some(weather)));
        }

        assert_eq!(Weather::of("plague of frogs"), Ok(None));
    }

    #[test]
    fn a_code_is_named_as_finely_as_it_was_written_down() {
        assert_eq!(named(2), Ok(Some("Partly Cloudy")));
        assert_eq!(named(65), Ok(Some("Heavy Rain")));
        assert_eq!(named(4), Ok(None));
    }
}
