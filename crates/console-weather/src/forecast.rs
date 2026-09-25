//! The week ahead, asked for in one question.
//!
//! What the wallpaper asks is one number, the sky right now. A forecast is the
//! temperature now and what it feels like, the day's hours, and the days after
//! it, and open-meteo answers all of that at once and in columns: one list of
//! times, and beside it one list per thing asked, each the same length.
//!
//! Times come back as seconds since the epoch, and the service says how far the
//! place's own clock is from that. Shifting them here once means an hour and a
//! weekday are arithmetic on a whole number rather than a date parsed out of a
//! string, and nothing downstream reads this machine's clock to say what hour
//! the forecast was written at: the answer carries its own `now`.
//!
//! Anything the answer is missing makes the whole of it unreadable, apart from
//! a chance of rain, which the service leaves empty for hours it has no model
//! of. An hour with no temperature is left out rather than drawn as zero.
//!
//! The answer is kept, as open-meteo wrote it, under the cache, and a forecast
//! younger than `FRESH_FOR` is not asked for again. The service models the
//! weather hourly and a panel is opened many times an hour, so asking at every
//! opening was eight seconds of network for an answer that could not have
//! changed. How old it is comes from the answer itself, the moment the service
//! said it was describing, so a clock that was wrong when it was kept cannot
//! make it look new.

use std::path::{Path, PathBuf};

use serde_json::Value;

use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_core_number_conversion::{fitted, whole_i32, whole_u8, whole_u32};

use crate::conditions::{self, SERVICE, FetchError};
use crate::here::Where;

const ASKED: &str = "&current=temperature_2m,apparent_temperature,relative_humidity_2m,wind_speed_10m,weather_code\
&hourly=temperature_2m,precipitation_probability,weather_code\
&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max\
&timezone=auto&timeformat=unixtime&forecast_days=7";

const HOUR: i64 = 3_600;

const DAY: i64 = 86_400;

const MINUTE: i64 = 60;

pub const FRESH_FOR: Seconds = Seconds(30 * 60);

const KEPT: &str = "console/forecast.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Seconds(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unix(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Degrees(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Percent(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KilometersPerHour(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalTime(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weekday {
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Now {
    pub at: LocalTime,
    pub temperature: Degrees,
    pub feels_like: Degrees,
    pub humidity: Percent,
    pub wind: KilometersPerHour,
    pub code: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hour {
    pub at: LocalTime,
    pub temperature: Degrees,
    pub rain: Option<Percent>,
    pub code: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Day {
    pub at: LocalTime,
    pub code: u32,
    pub low: Degrees,
    pub high: Degrees,
    pub rain: Option<Percent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forecast {
    pub asked: Unix,
    pub now: Now,
    pub hours: Vec<Hour>,
    pub days: Vec<Day>,
}

#[derive(Debug)]
pub enum Unforecast {
    Fetch(FetchError),
    Parse,
}

impl std::fmt::Display for Unforecast {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unforecast::Fetch(fault) => write!(to, "{fault}"),
            Unforecast::Parse => write!(to, "open-meteo answered with a forecast this cannot read"),
        }
    }
}

impl From<FetchError> for Unforecast {
    fn from(fault: FetchError) -> Self {
        Unforecast::Fetch(fault)
    }
}

impl LocalTime {
    pub fn hour(self) -> Result<u8, Never> {
        fitted(self.0.rem_euclid(DAY).div_euclid(HOUR))
    }

    pub fn on_the_hour(self) -> Result<LocalTime, Never> {
        Ok(LocalTime(self.0.saturating_sub(self.0.rem_euclid(HOUR))))
    }

    pub fn minute(self) -> Result<u8, Never> {
        fitted(self.0.rem_euclid(HOUR).div_euclid(MINUTE))
    }

    pub fn day(self) -> Result<i64, Never> {
        Ok(self.0.div_euclid(DAY))
    }

    pub fn weekday(self) -> Result<Weekday, Never> {
        let Ok(day) = self.day();

        Ok(match day.saturating_add(4).rem_euclid(7) {
            0 => Weekday::Sunday,
            1 => Weekday::Monday,
            2 => Weekday::Tuesday,
            3 => Weekday::Wednesday,
            4 => Weekday::Thursday,
            5 => Weekday::Friday,
            _ => Weekday::Saturday,
        })
    }
}

impl Weekday {
    pub fn named(self) -> Result<&'static str, Never> {
        Ok(match self {
            Weekday::Sunday => "Sunday",
            Weekday::Monday => "Monday",
            Weekday::Tuesday => "Tuesday",
            Weekday::Wednesday => "Wednesday",
            Weekday::Thursday => "Thursday",
            Weekday::Friday => "Friday",
            Weekday::Saturday => "Saturday",
        })
    }
}

impl Forecast {
    pub fn freshness(&self, now: Unix) -> Result<Freshness, Never> {
        let age = Seconds(now.0.saturating_sub(self.asked.0));

        Ok(match age < FRESH_FOR {
            true => Freshness::Fresh,
            false => Freshness::Stale,
        })
    }
}

pub fn kept_at(cache: &Path) -> Result<PathBuf, Never> {
    Ok(cache.join(KEPT))
}

pub fn kept(at: &Path) -> Result<Option<Forecast>, Never> {
    let Ok(held) = console_core_atomic_writes::read(at);

    match held {
        Stored::Text(said) => read(&said),
        Stored::Absent => Ok(None),
        Stored::Failed(fault) => {
            eprintln!("console-weather: {}: {fault}", at.display());

            Ok(None)
        }
    }
}

pub fn asking(at: &Where) -> Result<String, Never> {
    Ok(format!(
        "{SERVICE}?latitude={:.4}&longitude={:.4}{ASKED}",
        at.latitude, at.longitude
    ))
}

pub fn fetched(at: &Where, keep: Option<&Path>) -> Result<Forecast, Unforecast> {
    let Ok(asked) = asking(at);

    let said = conditions::answer(&asked)?;

    let Ok(read) = read(&said);

    match read {
        Some(forecast) => {
            let Ok(()) = keeping(keep, &said);

            Ok(forecast)
        }
        None => Err(Unforecast::Parse),
    }
}

fn keeping(keep: Option<&Path>, said: &str) -> Result<(), Never> {
    let keep = match keep {
        Some(keep) => keep,
        None => return Ok(()),
    };

    let made = match keep.parent() {
        Some(folder) => std::fs::create_dir_all(folder),
        None => Ok(()),
    };

    let written = match made {
        Ok(()) => console_core_atomic_writes::whole(keep, said.as_bytes()).map_err(|fault| fault.to_string()),
        Err(fault) => Err(fault.to_string()),
    };

    match written {
        Ok(()) => {},
        Err(fault) => eprintln!("console-weather: the forecast could not be kept: {fault}"),
    }

    Ok(())
}

pub fn read(answer: &str) -> Result<Option<Forecast>, Never> {
    let parsed: Value = match serde_json::from_str(answer) {
        Ok(parsed) => parsed,
        Err(_not_json) => return Ok(None),
    };

    let offset = match parsed.get("utc_offset_seconds").and_then(Value::as_i64) {
        Some(offset) => offset,
        None => return Ok(None),
    };

    let asked = match parsed.get("current").and_then(|current| current.get("time")).and_then(Value::as_i64) {
        Some(asked) => Unix(asked),
        None => return Ok(None),
    };

    let Ok(now) = now(parsed.get("current"), offset);
    let Ok(hours) = hours(parsed.get("hourly"), offset);
    let Ok(days) = days(parsed.get("daily"), offset);

    Ok(match (now, hours, days) {
        (Some(now), Some(hours), Some(days)) => {
            let Ok(hours) = from_midnight(hours, now.at);

            Some(Forecast { asked, now, hours, days })
        }
        (None, _, _) | (_, None, _) | (_, _, None) => None,
    })
}

fn now(block: Option<&Value>, offset: i64) -> Result<Option<Now>, Never> {
    let block = match block {
        Some(block) => block,
        None => return Ok(None),
    };

    let Ok(at) = time(block.get("time"), offset);
    let Ok(temperature) = degrees(block.get("temperature_2m"));
    let Ok(feels_like) = degrees(block.get("apparent_temperature"));
    let Ok(humidity) = percent(block.get("relative_humidity_2m"));
    let Ok(wind) = speed(block.get("wind_speed_10m"));
    let Ok(code) = code(block.get("weather_code"));

    Ok(match (at, temperature, feels_like, humidity, wind, code) {
        (Some(at), Some(temperature), Some(feels_like), Some(humidity), Some(wind), Some(code)) => {
            Some(Now { at, temperature, feels_like, humidity, wind, code })
        }
        _ => None,
    })
}

fn hours(block: Option<&Value>, offset: i64) -> Result<Option<Vec<Hour>>, Never> {
    let block = match block {
        Some(block) => block,
        None => return Ok(None),
    };

    let Ok(times) = column(block, "time");
    let Ok(temperatures) = column(block, "temperature_2m");
    let Ok(rain) = column(block, "precipitation_probability");
    let Ok(codes) = column(block, "weather_code");
    let columns = (times, temperatures, rain, codes);

    let (times, temperatures, rain, codes) = match columns {
        (Some(times), Some(temperatures), Some(rain), Some(codes)) => (times, temperatures, rain, codes),
        (None, _, _, _) | (_, None, _, _) | (_, _, None, _) | (_, _, _, None) => return Ok(None),
    };

    let read = times.iter().zip(temperatures).zip(rain).zip(codes).filter_map(|(((at, temperature), rain), code)| {
        let Ok(at) = time(Some(at), offset);
        let Ok(temperature) = degrees(Some(temperature));
        let Ok(rain) = percent(Some(rain));
        let Ok(code) = self::code(Some(code));

        match (at, temperature) {
            (Some(at), Some(temperature)) => Some(Hour { at, temperature, rain, code }),
            (None, _) | (_, None) => None,
        }
    });

    Ok(Some(read.collect()))
}

fn days(block: Option<&Value>, offset: i64) -> Result<Option<Vec<Day>>, Never> {
    let block = match block {
        Some(block) => block,
        None => return Ok(None),
    };

    let Ok(times) = column(block, "time");
    let Ok(codes) = column(block, "weather_code");
    let Ok(lows) = column(block, "temperature_2m_min");
    let Ok(highs) = column(block, "temperature_2m_max");
    let Ok(rain) = column(block, "precipitation_probability_max");
    let columns = (times, codes, lows, highs, rain);

    let (times, codes, lows, highs, rain) = match columns {
        (Some(times), Some(codes), Some(lows), Some(highs), Some(rain)) => (times, codes, lows, highs, rain),
        _ => return Ok(None),
    };

    let read = times.iter().zip(codes).zip(lows).zip(highs).zip(rain).filter_map(
        |((((at, code), low), high), rain)| {
            let Ok(day) = one_day(offset, [at, code, low, high, rain]);

            day
        },
    );

    Ok(Some(read.collect()))
}

fn one_day(offset: i64, [at, code, low, high, rain]: [&Value; 5]) -> Result<Option<Day>, Never> {
    let Ok(at) = time(Some(at), offset);
    let Ok(code) = self::code(Some(code));
    let Ok(low) = degrees(Some(low));
    let Ok(high) = degrees(Some(high));
    let Ok(rain) = percent(Some(rain));

    Ok(match (at, code, low, high) {
        (Some(at), Some(code), Some(low), Some(high)) => Some(Day { at, code, low, high, rain }),
        _ => None,
    })
}

fn from_midnight(hours: Vec<Hour>, now: LocalTime) -> Result<Vec<Hour>, Never> {
    let midnight = LocalTime(now.0.saturating_sub(now.0.rem_euclid(DAY)));

    Ok(hours.into_iter().filter(|hour| hour.at >= midnight).collect())
}

fn column<'a>(block: &'a Value, named: &str) -> Result<Option<&'a Vec<Value>>, Never> {
    Ok(block.get(named).and_then(Value::as_array))
}

fn time(said: Option<&Value>, offset: i64) -> Result<Option<LocalTime>, Never> {
    Ok(said.and_then(Value::as_i64).map(|unix| LocalTime(unix.saturating_add(offset))))
}

fn degrees(said: Option<&Value>) -> Result<Option<Degrees>, Never> {
    Ok(said.and_then(Value::as_f64).map(|said| {
        let Ok(whole) = whole_i32(said);

        Degrees(whole)
    }))
}

fn percent(said: Option<&Value>) -> Result<Option<Percent>, Never> {
    Ok(said.and_then(Value::as_f64).map(|said| {
        let Ok(whole) = whole_u8(said);

        Percent(whole)
    }))
}

fn speed(said: Option<&Value>) -> Result<Option<KilometersPerHour>, Never> {
    Ok(said.and_then(Value::as_f64).map(|said| {
        let Ok(whole) = whole_u32(said);

        KilometersPerHour(whole)
    }))
}

fn code(said: Option<&Value>) -> Result<Option<u32>, Never> {
    Ok(said.and_then(Value::as_u64).map(|said| {
        let Ok(code) = fitted(said);

        code
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANSWER: &str = r#"{
        "utc_offset_seconds": 7200,
        "current": {"time": 1790151300, "temperature_2m": 17.6, "apparent_temperature": 16.2,
                    "relative_humidity_2m": 71, "wind_speed_10m": 12.4, "weather_code": 2},
        "hourly": {"time": [1790143200, 1790146800, 1790150400, 1790154000],
                   "temperature_2m": [15.0, 16.4, 17.6, null],
                   "precipitation_probability": [0, 5, null, 20],
                   "weather_code": [0, 1, 2, 3]},
        "daily": {"time": [1790114400, 1790200800],
                  "weather_code": [2, 61],
                  "temperature_2m_min": [11.2, 9.8],
                  "temperature_2m_max": [19.5, 14.1],
                  "precipitation_probability_max": [10, 80]}
    }"#;

    fn forecast() -> Forecast {
        match read(ANSWER) {
            Ok(Some(forecast)) => forecast,
            Ok(None) => panic!("the answer should have been read"),
        }
    }

    #[test]
    fn the_temperature_now_is_rounded_to_a_whole_degree() {
        let now = forecast().now;

        assert_eq!(now.temperature, Degrees(18));
        assert_eq!(now.feels_like, Degrees(16));
        assert_eq!(now.humidity, Percent(71));
        assert_eq!(now.wind, KilometersPerHour(12));
        assert_eq!(now.code, 2);
    }

    #[test]
    fn the_hours_start_at_midnight_and_skip_what_has_no_temperature() {
        let hours = forecast().hours;
        let temperatures: Vec<Degrees> = hours.iter().map(|hour| hour.temperature).collect();

        assert_eq!(temperatures, vec![Degrees(15), Degrees(16), Degrees(18)]);
        assert_eq!(hours.get(2).map(|hour| (hour.rain, hour.code)), Some((None, Some(2))));
    }

    #[test]
    fn a_time_is_read_on_the_place_own_clock() {
        let now = forecast().now;

        assert_eq!(now.at.hour(), Ok(10));
        assert_eq!(now.at.weekday(), Ok(Weekday::Wednesday));
    }

    #[test]
    fn the_days_keep_their_low_their_high_and_their_chance_of_rain() {
        let days = forecast().days;

        let second = days.get(1).cloned();

        assert_eq!(
            second.map(|day| (day.low, day.high, day.rain, day.code)),
            Some((Degrees(10), Degrees(14), Some(Percent(80)), 61))
        );
        assert_eq!(days.first().map(|day| day.at.weekday()), Some(Ok(Weekday::Wednesday)));
    }

    #[test]
    fn a_forecast_is_fresh_for_half_an_hour_after_the_moment_it_describes() {
        let forecast = forecast();

        assert_eq!(forecast.freshness(Unix(1_790_151_300 + 29 * 60)), Ok(Freshness::Fresh));
        assert_eq!(forecast.freshness(Unix(1_790_151_300 + 30 * 60)), Ok(Freshness::Stale));
    }

    #[test]
    fn a_kept_answer_is_read_back_as_the_same_forecast() {
        let folder = std::env::temp_dir().join(format!("console-weather-{}", std::process::id()));
        let Ok(at) = kept_at(&folder);

        let Ok(()) = keeping(Some(&at), ANSWER);

        assert_eq!(kept(&at), Ok(Some(forecast())));
        let _ = std::fs::remove_dir_all(&folder);
        assert_eq!(kept(&at), Ok(None));
    }

    #[test]
    fn an_answer_missing_a_part_is_no_forecast() {
        assert_eq!(read(""), Ok(None));
        assert_eq!(read(r#"{"error":true,"reason":"out of range"}"#), Ok(None));
        assert_eq!(read(&ANSWER.replace("\"daily\"", "\"weekly\"")), Ok(None));
        assert_eq!(read(&ANSWER.replace("utc_offset_seconds", "offset")), Ok(None));
    }

    #[test]
    fn the_question_asks_for_times_the_place_can_read() {
        let Ok(asked) = asking(&Where { latitude: 50.85, longitude: 4.35 });

        assert!(asked.contains("latitude=50.8500"), "{asked}");
        assert!(asked.contains("timezone=auto"), "{asked}");
        assert!(asked.contains("timeformat=unixtime"), "{asked}");
    }
}
