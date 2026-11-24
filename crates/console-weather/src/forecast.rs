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

use serde::de::IgnoredAny;
use serde_json::Number;

use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_core_number_conversion::{fitted, whole_i32, whole_u8, whole_u32};
use console_location::Coordinates;

use crate::conditions::{self, FetchError, SERVICE};

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
  pub fn name(self) -> Result<&'static str, Never> {
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

    Ok(
      match ::core::primitive::bool::then_some(age < FRESH_FOR, ()) {
        ::core::option::Option::Some(()) => Freshness::Fresh,
        ::core::option::Option::None => Freshness::Stale,
      },
    )
  }
}

pub fn kept_at(cache: &Path) -> Result<PathBuf, Never> {
  Ok(cache.join(KEPT))
}

pub fn read_cache(at: &Path) -> Result<Option<Forecast>, Never> {
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

pub fn request_url(at: &Coordinates) -> Result<String, Never> {
  Ok(format!(
    "{SERVICE}?latitude={:.4}&longitude={:.4}{ASKED}",
    at.latitude.0, at.longitude.0
  ))
}

pub fn fetch(at: &Coordinates, keep: Option<&Path>) -> Result<Forecast, Unforecast> {
  let Ok(asked) = request_url(at);

  let said = conditions::answer(&asked)?;

  let Ok(read) = read(&said);

  match read {
    Some(forecast) => {
      let Ok(()) = store_cache(keep, &said);

      Ok(forecast)
    }
    None => Err(Unforecast::Parse),
  }
}

fn store_cache(keep: Option<&Path>, said: &str) -> Result<(), Never> {
  let keep_3 = match keep {
    Some(keep_2) => keep_2,
    None => return Ok(()),
  };

  let made = match keep_3.parent() {
    Some(folder) => std::fs::create_dir_all(folder),
    None => Ok(()),
  };

  let written = match made {
    Ok(()) => console_core_atomic_writes::whole(keep_3, said.as_bytes())
      .map_err(|fault| fault.to_string()),
    Err(fault) => Err(fault.to_string()),
  };

  match written {
    Ok(()) => {}
    Err(fault) => eprintln!("console-weather: the forecast could not be kept: {fault}"),
  }

  Ok(())
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentReply {
  time: i64,
  temperature_2m: Number,
  apparent_temperature: Number,
  relative_humidity_2m: Number,
  wind_speed_10m: Number,
  weather_code: u64,
  #[serde(rename = "interval")]
  _interval: Option<IgnoredAny>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct HourlyReply {
  time: Box<[Option<i64>]>,
  temperature_2m: Box<[Option<Number>]>,
  precipitation_probability: Box<[Option<Number>]>,
  weather_code: Box<[Option<u64>]>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DailyReply {
  time: Box<[Option<i64>]>,
  weather_code: Box<[Option<u64>]>,
  temperature_2m_min: Box<[Option<Number>]>,
  temperature_2m_max: Box<[Option<Number>]>,
  precipitation_probability_max: Box<[Option<Number>]>,
}

pub fn read(answer: &str) -> Result<Option<Forecast>, Never> {
  let parsed: conditions::Reply<CurrentReply, HourlyReply, DailyReply> =
    match conditions::decode(answer) {
      Ok(parsed) => parsed,
      Err(_unreadable_reply) => return Ok(None),
    };
  let (offset, hourly, daily) = match (parsed.utc_offset_seconds, parsed.hourly, parsed.daily) {
    (Some(offset), Some(hourly), Some(daily)) => (offset, hourly, daily),
    (None, _, _) | (_, None, _) | (_, _, None) => return Ok(None),
  };
  let Ok(current) = now(&parsed.current, offset);
  let now = match current {
    Some(current_weather) => current_weather,
    None => return Ok(None),
  };
  let Ok(all_hours) = hours(&hourly, offset);
  let Ok(hours) = from_midnight(all_hours, now.at);
  let Ok(days) = days(&daily, offset);

  Ok(Some(Forecast {
    asked: Unix(parsed.current.time),
    now,
    hours,
    days,
  }))
}

fn now(block: &CurrentReply, offset: i64) -> Result<Option<Now>, Never> {
  let at = LocalTime(block.time.saturating_add(offset));
  let Ok(temperature) = degrees(Some(&block.temperature_2m));
  let Ok(feels_like) = degrees(Some(&block.apparent_temperature));
  let Ok(humidity) = percent(Some(&block.relative_humidity_2m));
  let Ok(wind) = speed(&block.wind_speed_10m);
  let Ok(code) = fitted(block.weather_code);

  Ok(match (temperature, feels_like, humidity, wind) {
    (Some(temperature_2), Some(feels_like_2), Some(humidity_2), Some(wind_2)) => Some(Now {
      at,
      temperature: temperature_2,
      feels_like: feels_like_2,
      humidity: humidity_2,
      wind: wind_2,
      code,
    }),
    _ => None,
  })
}

fn hours(block: &HourlyReply, offset: i64) -> Result<Vec<Hour>, Never> {
  Ok(
    block
      .time
      .iter()
      .zip(block.temperature_2m.iter())
      .zip(block.precipitation_probability.iter())
      .zip(block.weather_code.iter())
      .filter_map(|(((&unix, temperature), rain), &weather_code)| {
        let Ok(at) = time(unix, offset);
        let Ok(warmth) = degrees(temperature.as_ref());
        let Ok(probability) = percent(rain.as_ref());
        let Ok(code) = code(weather_code);

        match (at, warmth) {
          (Some(local), Some(temperature_2)) => Some(Hour {
            at: local,
            temperature: temperature_2,
            rain: probability,
            code,
          }),
          (None, _) | (_, None) => None,
        }
      })
      .collect(),
  )
}

fn days(block: &DailyReply, offset: i64) -> Result<Vec<Day>, Never> {
  Ok(
    block
      .time
      .iter()
      .zip(block.weather_code.iter())
      .zip(block.temperature_2m_min.iter())
      .zip(block.temperature_2m_max.iter())
      .zip(block.precipitation_probability_max.iter())
      .filter_map(|((((&unix, &weather_code), low), high), rain)| {
        let Ok(at) = time(unix, offset);
        let Ok(code) = code(weather_code);
        let Ok(lowest) = degrees(low.as_ref());
        let Ok(highest) = degrees(high.as_ref());
        let Ok(probability) = percent(rain.as_ref());

        match (at, code, lowest, highest) {
          (Some(local), Some(wmo), Some(low_2), Some(high_2)) => Some(Day {
            at: local,
            code: wmo,
            low: low_2,
            high: high_2,
            rain: probability,
          }),
          _ => None,
        }
      })
      .collect(),
  )
}

fn from_midnight(hours: Vec<Hour>, now: LocalTime) -> Result<Vec<Hour>, Never> {
  let midnight = LocalTime(now.0.saturating_sub(now.0.rem_euclid(DAY)));

  Ok(
    hours
      .into_iter()
      .filter(|hour| hour.at >= midnight)
      .collect(),
  )
}

fn time(said: Option<i64>, offset: i64) -> Result<Option<LocalTime>, Never> {
  Ok(said.map(|unix| LocalTime(unix.saturating_add(offset))))
}

fn degrees(said: Option<&Number>) -> Result<Option<Degrees>, Never> {
  Ok(said.and_then(Number::as_f64).map(|number| {
    let Ok(whole) = whole_i32(number);

    Degrees(whole)
  }))
}

fn percent(said: Option<&Number>) -> Result<Option<Percent>, Never> {
  Ok(said.and_then(Number::as_f64).map(|number| {
    let Ok(whole) = whole_u8(number);

    Percent(whole)
  }))
}

fn speed(said: &Number) -> Result<Option<KilometersPerHour>, Never> {
  Ok(said.as_f64().map(|number| {
    let Ok(whole) = whole_u32(number);

    KilometersPerHour(whole)
  }))
}

fn code(said: Option<u64>) -> Result<Option<u32>, Never> {
  Ok(said.map(|number| {
    let Ok(wmo) = fitted(number);

    wmo
  }))
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_location::{Latitude, Longitude};
  use std::error::Error;

  const ANSWER: &str = r#"{
        "latitude": 50.85,
        "longitude": 4.35,
        "generationtime_ms": 0.2,
        "elevation": 47.0,
        "timezone": "Europe/Brussels",
        "timezone_abbreviation": "GMT+2",
        "utc_offset_seconds": 7200,
        "current_units": {"time": "unixtime", "interval": "seconds", "temperature_2m": "°C"},
        "hourly_units": {"temperature_2m": "°C"},
        "daily_units": {"temperature_2m_min": "°C"},
        "current": {"time": 1790151300, "interval": 900, "temperature_2m": 17.6, "apparent_temperature": 16.2,
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

  fn forecast() -> Result<Forecast, Box<dyn Error>> {
    let Ok(read) = read(ANSWER);

    let forecast = read.ok_or("the answer should have been read")?;

    Ok(forecast)
  }

  #[test]
  fn the_temperature_now_is_rounded_to_a_whole_degree() -> Result<(), Box<dyn Error>> {
    let Forecast { now, .. } = forecast()?;

    assert_eq!(now.temperature, Degrees(18));
    assert_eq!(now.feels_like, Degrees(16));
    assert_eq!(now.humidity, Percent(71));
    assert_eq!(now.wind, KilometersPerHour(12));
    assert_eq!(now.code, 2);

    Ok(())
  }

  #[test]
  fn the_hours_start_at_midnight_and_skip_what_has_no_temperature() -> Result<(), Box<dyn Error>>
  {
    let Forecast { hours, .. } = forecast()?;
    let temperatures: Vec<Degrees> = hours.iter().map(|hour| hour.temperature).collect();

    assert_eq!(temperatures, vec![Degrees(15), Degrees(16), Degrees(18)]);
    assert_eq!(
      hours.get(2).map(|hour| (hour.rain, hour.code)),
      Some((None, Some(2)))
    );

    Ok(())
  }

  #[test]
  fn a_time_is_read_on_the_place_own_clock() -> Result<(), Box<dyn Error>> {
    let Forecast { now, .. } = forecast()?;

    assert_eq!(now.at.hour(), Ok(10));
    assert_eq!(now.at.weekday(), Ok(Weekday::Wednesday));

    Ok(())
  }

  #[test]
  fn the_days_keep_their_low_their_high_and_their_chance_of_rain() -> Result<(), Box<dyn Error>> {
    let Forecast { days, .. } = forecast()?;

    let second = days.get(1).cloned();

    assert_eq!(
      second.map(|day| (day.low, day.high, day.rain, day.code)),
      Some((Degrees(10), Degrees(14), Some(Percent(80)), 61))
    );
    assert_eq!(
      days.first().map(|day| day.at.weekday()),
      Some(Ok(Weekday::Wednesday))
    );

    Ok(())
  }

  #[test]
  fn a_forecast_is_fresh_for_half_an_hour_after_the_moment_it_describes()
  -> Result<(), Box<dyn Error>> {
    let forecast = forecast()?;

    assert_eq!(
      forecast.freshness(Unix(1_790_151_300_i64.saturating_add(1_740))),
      Ok(Freshness::Fresh)
    );
    assert_eq!(
      forecast.freshness(Unix(1_790_151_300_i64.saturating_add(1_800))),
      Ok(Freshness::Stale)
    );

    Ok(())
  }

  #[test]
  fn a_kept_answer_is_read_back_as_the_same_forecast() -> Result<(), Box<dyn Error>> {
    let folder = console_core_temporary_directories::fresh("weather")?;
    let Ok(at) = kept_at(&folder);
    let expected = forecast()?;

    let Ok(()) = store_cache(Some(&at), ANSWER);

    assert_eq!(read_cache(&at), Ok(Some(expected)));
    std::fs::remove_dir_all(&folder)?;
    assert_eq!(read_cache(&at), Ok(None));

    Ok(())
  }

  #[test]
  fn an_answer_missing_a_part_is_no_forecast() {
    assert_eq!(read(""), Ok(None));
    assert_eq!(read(r#"{"error":true,"reason":"out of range"}"#), Ok(None));
    assert_eq!(read(&ANSWER.replace("\"daily\"", "\"weekly\"")), Ok(None));
    assert_eq!(
      read(&ANSWER.replace("utc_offset_seconds", "offset")),
      Ok(None)
    );
  }

  #[test]
  fn the_question_asks_for_times_the_place_can_read() {
    let Ok(asked) = request_url(&Coordinates {
      latitude: Latitude(50.85),
      longitude: Longitude(4.35),
    });

    assert!(asked.contains("latitude=50.8500"), "{asked}");
    assert!(asked.contains("timezone=auto"), "{asked}");
    assert!(asked.contains("timeformat=unixtime"), "{asked}");
  }
}
