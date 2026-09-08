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


use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::sync::atomic::{AtomicBool, Ordering};

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
    pub fn word(&self) -> Result<&'static str, Never> {
        Ok(match self {
            Weather::Clear => "clear",
            Weather::Cloud => "cloud",
            Weather::Fog => "fog",
            Weather::Rain => "rain",
            Weather::Snow => "snow",
            Weather::Storm => "storm",
        })
    }

    pub const EVERY: [Weather; 6] = [
        Weather::Clear,
        Weather::Cloud,
        Weather::Fog,
        Weather::Rain,
        Weather::Snow,
        Weather::Storm,
    ];

    pub fn of(word: &str) -> Result<Option<Self>, Never> {
        let word = word.trim().to_lowercase();

        for weather in Weather::EVERY {
            let said = weather.word()?;

            match said == word {
                true => return Ok(Some(weather)),
                false => {},
            }
        }

        Ok(None)
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

pub const SERVICE: &str = "https://api.open-meteo.com/v1/forecast";

pub fn asking(at: &crate::sun::Where) -> Result<String, Never> {
    Ok(format!(
        "{SERVICE}?latitude={:.4}&longitude={:.4}&current=weather_code",
        at.latitude, at.longitude
    ))
}

pub fn read(answer: &str) -> Result<Option<Weather>, Never> {
    let parsed: serde_json::Value = match serde_json::from_str(answer) {
        Ok(parsed) => parsed,
        Err(fault) => {
            eprintln!("console-sky: the weather service answered with something that is not JSON: {fault}");

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

pub fn now(at: &crate::sun::Where) -> Result<Option<Weather>, Never> {
    let asked = asking(at)?;

    let Ok(mut asking) = Program::Curl.command();

    let done = asking
        .args(["--silent", "--fail", "--max-time", "8", &asked])
        .output();

    let said = match done {
        Ok(said) if said.status.success() => {
            let read = read(&String::from_utf8_lossy(&said.stdout))?;

            match read {
                found @ Some(_) => found,
                None => complain("open-meteo answered with something this cannot read")?,
            }
        }
        Ok(said) => complain(&format!(
            "open-meteo would not answer: curl {}",
            said.status
        ))?,
        Err(fault) => complain(&format!("the weather could not be asked for: {fault}"))?,
    };

    ANSWERED.store(said.is_some(), Ordering::Relaxed);

    Ok(said)
}

static ANSWERED: AtomicBool = AtomicBool::new(true);

fn complain(said: &str) -> Result<Option<Weather>, Never> {
    match ANSWERED.swap(false, Ordering::Relaxed) {
        true => {
            eprintln!("{said}");
        }
        false => {},
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
        let Ok(asked) = asking(&crate::sun::Where {
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
}
