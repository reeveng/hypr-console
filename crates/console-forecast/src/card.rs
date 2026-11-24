//! The forecast, drawn.
//!
//! ```text
//!     forecast-panel
//!     forecast-panel Hourly
//!     forecast-panel Daily
//! ```
//!
//! Laid out the way Apple's Weather lays it out, because that is the forecast
//! most people have already learned to read: where it is and how warm, the
//! sky in words with the day's high and low, the next hours across one row,
//! then the days ahead one to a line. What it feels like, the humidity and the
//! wind close the first half, since they are what somebody deciding on a coat
//! asks next.
//!
//! Four tabs, because a week of days under the hours is a week below the
//! fold: now is the first, the whole of today an hour to a line is the second,
//! and the week is the third, a day to a line and all seven on the screen at
//! once. The fourth is where the forecast is for. The first three are drawn from one answer, since the later tabs read the answer
//! the first one kept. The question is asked where
//! the rows are read, off the drawing, and the card says it is asking until the
//! answer arrives: the service is given eight seconds, and a panel that sits
//! blank for eight seconds reads as one that failed to open.
//!
//! The last answer is kept, and `console_weather::forecast` says for how long
//! it is worth believing. Inside that it is drawn and nothing is asked. Past
//! it, the kept one is drawn while a new one is asked for, so the panel opens
//! on a forecast rather than on a wait, and if the service will not answer the
//! kept one stays up. When it was asked is not written anywhere: a forecast
//! half an hour old says the same thing as a new one, and a line saying so is
//! a line nobody reads. With nothing kept at all it says it is asking.
//!
//! Above the temperature is the wallpaper's picture for the weather it is,
//! chosen out of `theme/sky.toml` by the same rule the wallpaper chooses by,
//! but for the sky in this forecast rather than whichever one the wallpaper was
//! last told or pinned to. A picture of the rain says it is raining before a
//! word is read.
//!
//! The place is the timezone's city, which is where `console_location::timezone`
//! says the machine is. See that module for how it estimates the location.
//! The last tab lists every zone's city. Choosing another city clears the
//! cached forecast for the previous location.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use console_core_arguments::{Command, Operands};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_places::Base;
use console_location::timezone::{self, Zone};
use console_panel::card::{Card, Door};
use console_panel::icons::Icon;
use console_panel::page::{
  self, Active, Aside, Cell, Handler, Headline, Page, Picture, Row, Rows, YET,
};
use console_wallpaper::choose::{self, Outside, Turn};
use console_weather::conditions::{self, Weather};
use console_weather::forecast::{
  self, Day, Degrees, Forecast, Freshness, Hour, LocalTime, Percent, Unforecast, Unix,
};

pub const WHO: &str = "forecast-panel";

const DOOR: &str = "forecast";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
  Now,
  Hourly,
  Daily,
}

impl Tab {
  fn title(self) -> Result<&'static str, Never> {
    Ok(match self {
      Tab::Now => "Now",
      Tab::Hourly => "Hourly",
      Tab::Daily => "Daily",
    })
  }
}

const HERE: &str = "My Location";

const LOCATION: &str = "Location";

const ASKING: &str = "Updating";

const UNAVAILABLE: &str = "Weather Unavailable";

const NOW: &str = "Now";

const TODAY: &str = "Today";

const HOURS_ACROSS: u8 = 6;

const DAYLIGHT: std::ops::RangeInclusive<u8> = 7..=19;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Light {
  Day,
  Night,
}

pub const COMMAND: Command = Command {
  name: "forecast-panel",
  about: "the weather where this machine is",
  flags: &[],
  operands: Operands::Optional("TAB"),
};

pub fn door(_argv: &[String]) -> Result<Door, Never> {
  Door::closing(DOOR)
}

pub fn card(argv: &[String]) -> Result<Card, Never> {
  let Ok(card) = Card::new(|| {
    let Ok(pages) = pages();

    pages
  });

  card.opening_at(argv.first().map(String::as_str))
}

#[cfg_attr(
  dylint_lib = "explicit039_no_reading_the_clock",
  allow(
    explicit039_no_reading_the_clock,
    reason = "this is the edge the panel's question starts at: whether the kept forecast is still worth drawing, and which part of the day its picture shows, are questions about now, and nothing calls this that could hand a now in"
  )
)]
#[cfg_attr(
  all(dylint_lib = "explicit077_no_ambient_randomness", not(test)),
  expect(
    explicit077_no_ambient_randomness,
    reason = "the panel's deferred row callbacks sample wall time once per invocation and pass that sample to the forecast cache and daylight calculations"
  )
)]
fn clock() -> Result<Duration, Never> {
  let since = SystemTime::now().duration_since(UNIX_EPOCH);

  Ok(match since {
    Ok(since_2) => since_2,
    Err(_the_clock_is_before_1970) => Duration::ZERO,
  })
}

fn unix(since: Duration) -> Result<Unix, Never> {
  let Ok(seconds) = fitted(since.as_secs());

  Ok(Unix(seconds))
}

fn still(forecast: &Forecast, since: Duration) -> Result<Option<PathBuf>, Never> {
  let seconds = since.as_secs_f64();
  let Ok(coordinates) = timezone::locate();
  let Ok(weather) = Weather::of_code(forecast.now.code);
  let Ok(outside) = Outside::at(&coordinates, seconds, Some(weather));
  let Ok(turn) = Turn::at(seconds);

  choose::still(&outside, turn)
}

fn kept_at() -> Result<Option<PathBuf>, Never> {
  let Ok(cache) = Base::Cache.user();

  Ok(match cache {
    Some(cache_2) => {
      let Ok(at) = forecast::kept_at(&cache_2);

      Some(at)
    }
    None => None,
  })
}

fn cached_forecast(at: Option<&Path>) -> Result<Option<Forecast>, Never> {
  match at {
    Some(at_2) => forecast::read_cache(at_2),
    None => Ok(None),
  }
}

fn pages() -> Result<Vec<Page>, Never> {
  let Ok(now) = page(Tab::Now);
  let Ok(hourly) = page(Tab::Hourly);
  let Ok(daily) = page(Tab::Daily);
  let Ok(location) = location_page();

  Ok(vec![now, hourly, daily, location])
}

fn location_page() -> Result<Page, Never> {
  let Ok(rows) = Rows::computed(|| {
    let Ok(rows) = location_rows();

    rows
  });

  Page::new(LOCATION, rows)
}

fn location_rows() -> Result<Vec<Row>, Never> {
  let Ok(chosen) = timezone::current();
  let Ok(every) = timezone::every_zone();
  let Ok(clock) = clock_city();
  let Ok(mine) = location_row(&format!("{HERE}  {clock}"), None, chosen.as_deref());
  let mut cities = Vec::new();

  for zone in every {
    let Ok(city) = timezone::city(Zone(&zone));
    let Ok(row) = location_row(&city, Some(zone), chosen.as_deref());

    cities.push(row);
  }

  let Ok(cities_2) = page::lettered(cities);
  let mut rows = vec![mine];

  rows.extend(cities_2);

  Ok(rows)
}

fn clock_city() -> Result<String, Never> {
  let Ok(zone) = timezone::zone();

  match zone {
    Some(zone_2) => timezone::city(Zone(&zone_2)),
    None => Ok(String::new()),
  }
}

fn location_row(says: &str, zone: Option<String>, chosen: Option<&str>) -> Result<Row, Never> {
  let mark = match ::core::primitive::bool::then_some(zone.as_deref() == chosen, ()) {
    ::core::option::Option::Some(()) => page::NOW,
    ::core::option::Option::None => "",
  };
  let Ok(chooses) = Handler::and_stay(move |showing| {
    let Ok(()) = timezone::choose(zone.as_deref().map(Zone));
    let Ok(()) = forget_forecast();

    showing.turn_to(0);
  });

  Row::new(says, Aside(mark), chooses)
}

fn forget_forecast() -> Result<(), Never> {
  let Ok(kept) = kept_at();

  let at = match kept {
    Some(path) => path,
    None => return Ok(()),
  };

  match std::fs::remove_file(&at) {
    Ok(()) => {}
    Err(fault) => {
      match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => eprintln!("{WHO}: {}: {fault}", at.display()),
      }
    }
  }

  Ok(())
}

fn page(tab: Tab) -> Result<Page, Never> {
  let Ok(rows) = Rows::computed(move || {
    let Ok(since) = clock();
    let Ok(rows) = rows(tab, since);

    rows
  });
  let Ok(title) = tab.title();
  let Ok(page) = Page::new(title, rows);

  page.meanwhile(move || {
    let Ok(since) = clock();
    let Ok(rows_2) = cached_rows(tab, since);

    rows_2
  })
}

fn cached_rows(tab: Tab, since: Duration) -> Result<Vec<Row>, Never> {
  let Ok(place) = place();
  let Ok(at) = kept_at();
  let Ok(kept) = cached_forecast(at.as_deref());

  match kept {
    Some(kept_2) => render_rows(tab, &place, &kept_2, since),
    None => {
      let Ok(row) = Row::placeholder(&format!("{ASKING}{YET}"));
      let Ok(heading) = Row::naming(&place, Aside(""));

      Ok(vec![heading, row])
    }
  }
}

fn rows(tab: Tab, since: Duration) -> Result<Vec<Row>, Never> {
  let Ok(place) = place();
  let Ok(at) = kept_at();
  let Ok(kept) = cached_forecast(at.as_deref());
  let Ok(now) = unix(since);

  let freshness = match &kept {
    Some(kept_2) => {
      let Ok(freshness) = kept_2.freshness(now);

      freshness
    }
    None => Freshness::Stale,
  };

  match (freshness, kept) {
    (Freshness::Fresh, Some(kept_3)) => render_rows(tab, &place, &kept_3, since),
    (Freshness::Fresh, None) | (Freshness::Stale, _) => {
      fetch_rows(tab, &place, at.as_deref(), since)
    }
  }
}

fn fetch_rows(
  tab: Tab,
  place: &str,
  keep: Option<&Path>,
  since: Duration,
) -> Result<Vec<Row>, Never> {
  let Ok(coordinates) = timezone::locate();

  match forecast::fetch(&coordinates, keep) {
    Ok(forecast) => render_rows(tab, place, &forecast, since),
    Err(fault) => {
      let Ok(kept) = cached_forecast(keep);

      match kept {
        Some(kept_2) => render_rows(tab, place, &kept_2, since),
        None => unavailable(place, &fault),
      }
    }
  }
}

fn place() -> Result<String, Never> {
  let Ok(zone) = timezone::place();

  match zone {
    Some(zone_2) => timezone::city(Zone(&zone_2)),
    None => Ok(HERE.to_string()),
  }
}

fn unavailable(place: &str, fault: &Unforecast) -> Result<Vec<Row>, Never> {
  let Ok(heading) = Row::naming(place, Aside(""));
  let Ok(said) = Row::text(UNAVAILABLE, Aside(""));
  let Ok(why) = Row::placeholder(&fault.to_string());

  Ok(vec![heading, said, why])
}

fn render_rows(
  tab: Tab,
  place: &str,
  forecast: &Forecast,
  since: Duration,
) -> Result<Vec<Row>, Never> {
  match tab {
    Tab::Now => today_drawn(place, forecast, since),
    Tab::Hourly => the_day(&forecast.hours, forecast.now.at),
    Tab::Daily => days(&forecast.days, forecast.now.at),
  }
}

fn the_day(hours: &[Hour], now: LocalTime) -> Result<Vec<Row>, Never> {
  let Ok(today) = now.day();
  let mut rows = Vec::new();

  for hour in hours {
    let Ok(day) = hour.at.day();

    match ::core::primitive::bool::then_some(day == today, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(row) = hour_row(hour, now);

        rows.push(row);
      }
      ::core::option::Option::None => {}
    }
  }

  Ok(rows)
}

fn hour_row(hour: &Hour, now: LocalTime) -> Result<Row, Never> {
  let Ok(hour_name) = hour_named(hour.at, now);
  let Ok(when) = o_clock(hour_name);
  let Ok(sky) = match hour.code {
    Some(code) => condition_name(code),
    None => Ok(String::new()),
  };
  let Ok(rain) = rain(hour.rain);
  let Ok(temperature) = degrees(hour.temperature);
  let Ok(row) = Row::text(&format!("{when}  {sky}{rain}"), Aside(&temperature));
  let Ok(light) = light(hour.at);

  match hour.code {
    Some(code) => {
      let Ok(icon) = icon(code, light);

      row.picturing(Picture::Named(icon))
    }
    None => row.picturing(Picture::Space),
  }
}

fn today_drawn(place: &str, forecast: &Forecast, since: Duration) -> Result<Vec<Row>, Never> {
  let Ok(headline) = headline(place, forecast, since);
  let Ok(hours) = hours(&forecast.hours, forecast.now.at);
  let Ok(details) = details(forecast);
  let mut rows = vec![headline];

  rows.extend(hours);
  rows.extend(details);

  Ok(rows)
}

fn headline(place: &str, forecast: &Forecast, since: Duration) -> Result<Row, Never> {
  let Ok(still) = still(forecast, since);
  let Ok(big) = degrees(forecast.now.temperature);
  let Ok(says) = condition_name(forecast.now.code);
  let Ok(aside) = today(forecast);
  let Ok(light) = light(forecast.now.at);
  let Ok(icon) = icon(forecast.now.code, light);

  Row::headline(
    Picture::Showing(still),
    Headline {
      title: place.to_string(),
      subtitle: String::new(),
      icon: Some(icon),
      says,
      big,
      aside,
      ..Headline::default()
    },
  )
}

fn light(at: LocalTime) -> Result<Light, Never> {
  let Ok(hour) = at.hour();

  Ok(
    match ::core::primitive::bool::then_some(DAYLIGHT.contains(&hour), ()) {
      ::core::option::Option::Some(()) => Light::Day,
      ::core::option::Option::None => Light::Night,
    },
  )
}

fn icon(code: u32, light: Light) -> Result<Icon, Never> {
  Ok(match (code, light) {
    (0, Light::Day) => Icon::Sun,
    (0, Light::Night) => Icon::Moon,
    (1 | 2, Light::Day) => Icon::CloudSun,
    (1 | 2, Light::Night) => Icon::CloudMoon,
    (45 | 48, _) => Icon::Fog,
    (51..=67 | 80..=82, _) => Icon::Rain,
    (71..=77 | 85 | 86, _) => Icon::Snow,
    (95..=99, _) => Icon::Thunderstorm,
    (_, _) => Icon::Cloud,
  })
}

fn today(forecast: &Forecast) -> Result<String, Never> {
  Ok(match forecast.days.first() {
    Some(day) => {
      let Ok(range) = range(day);

      range
    }
    None => String::new(),
  })
}

fn hours(hours: &[Hour], now: LocalTime) -> Result<Vec<Row>, Never> {
  let mut named = Vec::new();
  let mut warm = Vec::new();
  let Ok(across) = index(HOURS_ACROSS);

  let Ok(this_hour) = now.on_the_hour();

  for hour in hours
    .iter()
    .filter(|hour| hour.at >= this_hour)
    .take(across)
  {
    let Ok(when) = hour_named(hour.at, now);
    let Ok(temperature) = degrees(hour.temperature);
    let Ok(when_2) = Cell::new(&when, Active::No);
    let Ok(temperature_2) = Cell::new(&temperature, Active::No);
    let Ok(light) = light(hour.at);
    let Ok(temperature_3) = match hour.code {
      Some(code) => {
        let Ok(icon) = icon(code, light);

        temperature_2.with_icon(icon)
      }
      None => Ok(temperature_2),
    };

    named.push(when_2);
    warm.push(temperature_3);
  }

  let Ok(headings) = Row::naming_cells(named);
  let Ok(temperatures) = Row::celled(warm);

  Ok(vec![headings, temperatures])
}

fn o_clock(named: String) -> Result<String, Never> {
  Ok(match named.as_str() {
    NOW => named,
    _ => format!("{named}:00"),
  })
}

fn hour_named(at: LocalTime, now: LocalTime) -> Result<String, Never> {
  let Ok(hour) = at.hour();
  let Ok(at_2) = at.on_the_hour();
  let Ok(now_2) = now.on_the_hour();

  Ok(
    match ::core::primitive::bool::then_some(at_2 == now_2, ()) {
      ::core::option::Option::Some(()) => NOW.to_string(),
      ::core::option::Option::None => format!("{hour:02}"),
    },
  )
}

fn details(forecast: &Forecast) -> Result<Vec<Row>, Never> {
  let now = &forecast.now;
  let Ok(feels) = degrees(now.feels_like);
  let Ok(humidity) = percent(now.humidity);
  let wind = format!("{} km/h", now.wind.0);
  let Ok(this_hour) = now.at.on_the_hour();
  let chance = forecast
    .hours
    .iter()
    .find(|hour| hour.at >= this_hour)
    .and_then(|hour| hour.rain);
  let Ok(chance_3) = percent(match chance {
    Some(chance_2) => chance_2,
    None => Percent(0),
  });

  let Ok(feels_cell) = detail(Icon::Thermometer, "Feels Like", Aside(&feels));
  let Ok(wind_cell) = detail(Icon::Wind, "Wind", Aside(&wind));
  let Ok(rain) = detail(Icon::Umbrella, "Rain", Aside(&chance_3));
  let Ok(humidity_cell) = detail(Icon::Humidity, "Humidity", Aside(&humidity));
  let Ok(first) = Row::celled(vec![feels_cell, wind_cell]);
  let Ok(second) = Row::celled(vec![rain, humidity_cell]);

  Ok(vec![first, second])
}

fn detail(icon: Icon, named: &str, value: Aside<'_>) -> Result<Cell, Never> {
  let Ok(cell) = Cell::new(&format!("{named}  {}", value.0), Active::No);

  cell.with_icon(icon)
}

fn days(days: &[Day], now: LocalTime) -> Result<Vec<Row>, Never> {
  let mut rows = Vec::new();

  for day in days {
    let Ok(row) = day_row(day, now);

    rows.push(row);
  }

  Ok(rows)
}

fn day_row(day: &Day, now: LocalTime) -> Result<Row, Never> {
  let Ok(when) = day_named(day.at, now);
  let Ok(sky) = condition_name(day.code);
  let Ok(rain) = rain(day.rain);
  let Ok(range) = range(day);
  let Ok(icon) = icon(day.code, Light::Day);
  let Ok(row) = Row::text(&format!("{when}  {sky}{rain}"), Aside(&range));

  row.picturing(Picture::Named(icon))
}

fn day_named(at: LocalTime, now: LocalTime) -> Result<String, Never> {
  let Ok(day) = at.day();
  let Ok(today) = now.day();

  match ::core::primitive::bool::then_some(day == today, ()) {
    ::core::option::Option::Some(()) => Ok(TODAY.to_string()),
    ::core::option::Option::None => {
      let Ok(weekday) = at.weekday();
      let Ok(named) = weekday.name();

      Ok(named.to_string())
    }
  }
}

fn rain(chance: Option<Percent>) -> Result<String, Never> {
  Ok(match chance {
    Some(Percent(0)) | None => String::new(),
    Some(probability) => {
      let Ok(chance_text) = percent(probability);

      format!(", {chance_text}")
    }
  })
}

fn range(day: &Day) -> Result<String, Never> {
  let Ok(high) = degrees(day.high);
  let Ok(low) = degrees(day.low);

  Ok(format!("High {high}  Low {low}"))
}

fn condition_name(code: u32) -> Result<String, Never> {
  let Ok(said) = conditions::label(code);

  Ok(match said {
    Some(said_2) => said_2.to_string(),
    None => String::new(),
  })
}

fn degrees(said: Degrees) -> Result<String, Never> {
  Ok(format!("{}\u{b0}", said.0))
}

fn percent(said: Percent) -> Result<String, Never> {
  Ok(format!("{}%", said.0))
}

#[cfg(test)]
mod tests {
  use super::*;

  const WEDNESDAY_NOON: LocalTime = LocalTime(1_790_164_800);

  fn day(at: LocalTime, rain: Option<Percent>) -> Result<Day, Never> {
    Ok(Day {
      at,
      code: 61,
      low: Degrees(-2),
      high: Degrees(14),
      rain,
    })
  }

  #[test]
  fn the_first_day_is_today_and_the_rest_are_named() {
    let Ok(first) = day_named(WEDNESDAY_NOON, WEDNESDAY_NOON);
    let Ok(second) = day_named(
      LocalTime(WEDNESDAY_NOON.0.saturating_add(86_400)),
      WEDNESDAY_NOON,
    );

    assert_eq!(first, "Today");
    assert_eq!(second, "Thursday");
  }

  #[test]
  fn a_day_says_its_sky_its_chance_of_rain_and_its_range() {
    let Ok(today) = day(WEDNESDAY_NOON, Some(Percent(80)));
    let Ok(row) = day_row(&today, WEDNESDAY_NOON);

    assert_eq!(row.says, "Today  Rain, 80%");
    assert_eq!(row.aside, "High 14\u{b0}  Low -2\u{b0}");
  }

  #[test]
  fn no_chance_of_rain_is_not_written_down() {
    let Ok(today) = day(WEDNESDAY_NOON, Some(Percent(0)));
    let Ok(none) = day_row(&today, WEDNESDAY_NOON);
    let Ok(today_2) = day(WEDNESDAY_NOON, None);
    let Ok(unknown) = day_row(&today_2, WEDNESDAY_NOON);

    assert_eq!(none.says, "Today  Rain");
    assert_eq!(unknown.says, "Today  Rain");
  }

  #[test]
  fn the_day_is_every_hour_of_today_and_only_this_one_is_now() {
    let hour = |at: i64, code| Hour {
      at: LocalTime(at),
      temperature: Degrees(9),
      rain: Some(Percent(20)),
      code,
    };
    let midnight = WEDNESDAY_NOON.0.saturating_sub(43_200);
    let hours = vec![
      hour(midnight.saturating_sub(3_600), Some(0)),
      hour(midnight, Some(0)),
      hour(WEDNESDAY_NOON.0, Some(61)),
      hour(midnight.saturating_add(86_400), None),
    ];
    let Ok(rows) = the_day(&hours, LocalTime(WEDNESDAY_NOON.0.saturating_add(900)));
    let said: Vec<(&str, &str)> = rows
      .iter()
      .map(|row| (row.says.as_str(), row.aside.as_str()))
      .collect();

    assert_eq!(
      said,
      vec![
        ("00:00  Clear, 20%", "9\u{b0}"),
        ("Now  Rain, 20%", "9\u{b0}")
      ]
    );
  }

  #[test]
  fn the_first_hour_is_now_and_the_rest_are_the_clock() {
    let quarter_past = LocalTime(WEDNESDAY_NOON.0.saturating_add(900));

    assert_eq!(
      hour_named(WEDNESDAY_NOON, quarter_past),
      Ok("Now".to_string())
    );
    assert_eq!(
      hour_named(
        LocalTime(WEDNESDAY_NOON.0.saturating_add(10_800)),
        quarter_past
      ),
      Ok("15".to_string())
    );
  }
}
