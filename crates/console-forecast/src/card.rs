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
//! The place is the timezone's city, which is where `console_weather::here`
//! says the machine is; see its head for why that is near enough and why no
//! address is stored. The last tab lists every zone's city to choose another
//! one from, and choosing forgets the kept forecast, since it was for the
//! place before.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_places::Base;
use console_panel::card::{Card, Door};
use console_panel::icons::Icon;
use console_panel::page::{self, Aside, Cell, Handler, Headline, Active, Page, Picture, Row, Rows, YET};
use console_wallpaper::choose::{self, Outside, Turn};
use console_weather::conditions::{self, Weather};
use console_weather::forecast::{self, Day, Degrees, Forecast, Freshness, Hour, LocalTime, Percent, Unforecast, Unix};
use console_weather::here::{self, Zone};

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

pub fn door(_argv: &[String]) -> Result<Door, Never> {
    Door::closing(DOOR)
}

pub fn card(argv: &[String]) -> Result<Card, Never> {
    let Ok(card) = Card::new(Arc::new(|| {
        let Ok(pages) = pages();

        pages
    }));

    card.opening_at(argv.first().map(String::as_str))
}

#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
        explicit039_no_reading_the_clock,
        reason = "this is the edge the panel's question starts at: whether the kept forecast is still worth drawing, and which part of the day its picture shows, are questions about now, and nothing calls this that could hand a now in"
    )
)]
fn clock() -> Result<Duration, Never> {
    let since = SystemTime::now().duration_since(UNIX_EPOCH);

    Ok(match since {
        Ok(since) => since,
        Err(_the_clock_is_before_1970) => Duration::ZERO,
    })
}

fn now() -> Result<Unix, Never> {
    let Ok(since) = clock();
    let Ok(seconds) = fitted(since.as_secs());

    Ok(Unix(seconds))
}

fn still(forecast: &Forecast) -> Result<Option<PathBuf>, Never> {
    let Ok(since) = clock();
    let seconds = since.as_secs_f64();
    let Ok(here) = here::asking();
    let Ok(weather) = Weather::of_code(forecast.now.code);
    let Ok(outside) = Outside::at(&here, seconds, Some(weather));
    let Ok(turn) = Turn::at(seconds);

    choose::still(&outside, turn)
}

fn kept_at() -> Result<Option<PathBuf>, Never> {
    let Ok(cache) = Base::Cache.hers();

    Ok(match cache {
        Some(cache) => {
            let Ok(at) = forecast::kept_at(&cache);

            Some(at)
        }
        None => None,
    })
}

fn kept(at: Option<&Path>) -> Result<Option<Forecast>, Never> {
    match at {
        Some(at) => forecast::kept(at),
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
    let Ok(rows) = Rows::asked(|| {
        let Ok(rows) = location_rows();

        rows
    });

    Page::new(LOCATION, rows)
}

fn location_rows() -> Result<Vec<Row>, Never> {
    let Ok(chosen) = here::chosen();
    let Ok(every) = here::every_zone();
    let Ok(clock) = clock_city();
    let Ok(mine) = location_row(&format!("{HERE}  {clock}"), None, chosen.as_deref());
    let mut cities = Vec::new();

    for zone in every {
        let Ok(city) = here::city(Zone(&zone));
        let Ok(row) = location_row(&city, Some(zone), chosen.as_deref());

        cities.push(row);
    }

    let Ok(cities) = page::lettered(cities);
    let mut rows = vec![mine];

    rows.extend(cities);

    Ok(rows)
}

fn clock_city() -> Result<String, Never> {
    let Ok(zone) = here::zone();

    match zone {
        Some(zone) => here::city(Zone(&zone)),
        None => Ok(String::new()),
    }
}

fn location_row(says: &str, zone: Option<String>, chosen: Option<&str>) -> Result<Row, Never> {
    let mark = match zone.as_deref() == chosen {
        true => page::NOW,
        false => "",
    };
    let Ok(chooses) = Handler::and_stay(move |showing| {
        let Ok(()) = here::choose(zone.as_deref().map(Zone));
        let Ok(()) = forget_forecast();

        showing.turn_to(0);
    });

    Row::new(says, Aside(mark), chooses)
}

fn forget_forecast() -> Result<(), Never> {
    let Ok(at) = kept_at();

    let at = match at {
        Some(at) => at,
        None => return Ok(()),
    };

    match std::fs::remove_file(&at) {
        Ok(()) => {},
        Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
            true => {},
            false => eprintln!("{WHO}: {}: {fault}", at.display()),
        },
    }

    Ok(())
}

fn page(tab: Tab) -> Result<Page, Never> {
    let Ok(rows) = Rows::asked(move || {
        let Ok(rows) = rows(tab);

        rows
    });
    let Ok(title) = tab.title();
    let Ok(page) = Page::new(title, rows);

    page.meanwhile(move || {
        let Ok(rows) = asking(tab);

        rows
    })
}

fn asking(tab: Tab) -> Result<Vec<Row>, Never> {
    let Ok(place) = place();
    let Ok(at) = kept_at();
    let Ok(kept) = kept(at.as_deref());

    match kept {
        Some(kept) => drawn(tab, &place, &kept),
        None => {
            let Ok(row) = Row::nothing(&format!("{ASKING}{YET}"));
            let Ok(heading) = Row::naming(&place, Aside(""));

            Ok(vec![heading, row])
        }
    }
}

fn rows(tab: Tab) -> Result<Vec<Row>, Never> {
    let Ok(place) = place();
    let Ok(at) = kept_at();
    let Ok(kept) = kept(at.as_deref());
    let Ok(now) = now();

    let freshness = match &kept {
        Some(kept) => {
            let Ok(freshness) = kept.freshness(now);

            freshness
        }
        None => Freshness::Stale,
    };

    match (freshness, kept) {
        (Freshness::Fresh, Some(kept)) => drawn(tab, &place, &kept),
        (Freshness::Fresh, None) | (Freshness::Stale, _) => asked(tab, &place, at.as_deref()),
    }
}

fn asked(tab: Tab, place: &str, keep: Option<&Path>) -> Result<Vec<Row>, Never> {
    let Ok(here) = here::asking();

    match forecast::fetched(&here, keep) {
        Ok(forecast) => drawn(tab, place, &forecast),
        Err(fault) => {
            let Ok(kept) = kept(keep);

            match kept {
                Some(kept) => drawn(tab, place, &kept),
                None => unavailable(place, &fault),
            }
        }
    }
}

fn place() -> Result<String, Never> {
    let Ok(zone) = here::place();

    match zone {
        Some(zone) => here::city(Zone(&zone)),
        None => Ok(HERE.to_string()),
    }
}

fn unavailable(place: &str, fault: &Unforecast) -> Result<Vec<Row>, Never> {
    let Ok(heading) = Row::naming(place, Aside(""));
    let Ok(said) = Row::said(UNAVAILABLE, Aside(""));
    let Ok(why) = Row::nothing(&fault.to_string());

    Ok(vec![heading, said, why])
}

fn drawn(tab: Tab, place: &str, forecast: &Forecast) -> Result<Vec<Row>, Never> {
    match tab {
        Tab::Now => today_drawn(place, forecast),
        Tab::Hourly => the_day(&forecast.hours, forecast.now.at),
        Tab::Daily => days(&forecast.days, forecast.now.at),
    }
}

fn the_day(hours: &[Hour], now: LocalTime) -> Result<Vec<Row>, Never> {
    let Ok(today) = now.day();
    let mut rows = Vec::new();

    for hour in hours {
        let Ok(day) = hour.at.day();

        match day == today {
            true => {
                let Ok(row) = hour_row(hour, now);

                rows.push(row);
            }
            false => {}
        }
    }

    Ok(rows)
}

fn hour_row(hour: &Hour, now: LocalTime) -> Result<Row, Never> {
    let Ok(when) = hour_named(hour.at, now);
    let Ok(when) = o_clock(when);
    let Ok(sky) = match hour.code {
        Some(code) => named(code),
        None => Ok(String::new()),
    };
    let Ok(rain) = rain(hour.rain);
    let Ok(temperature) = degrees(hour.temperature);
    let Ok(row) = Row::said(&format!("{when}  {sky}{rain}"), Aside(&temperature));
    let Ok(light) = light(hour.at);

    match hour.code {
        Some(code) => {
            let Ok(icon) = icon(code, light);

            row.picturing(Picture::Named(icon))
        },
        None => row.picturing(Picture::Space),
    }
}

fn today_drawn(place: &str, forecast: &Forecast) -> Result<Vec<Row>, Never> {
    let Ok(headline) = headline(place, forecast);
    let Ok(hours) = hours(&forecast.hours, forecast.now.at);
    let Ok(details) = details(forecast);
    let mut rows = vec![headline];

    rows.extend(hours);
    rows.extend(details);

    Ok(rows)
}

fn headline(place: &str, forecast: &Forecast) -> Result<Row, Never> {
    let Ok(still) = still(forecast);
    let Ok(big) = degrees(forecast.now.temperature);
    let Ok(says) = named(forecast.now.code);
    let Ok(aside) = today(forecast);
    let Ok(light) = light(forecast.now.at);
    let Ok(icon) = icon(forecast.now.code, light);

    Row::headline(
        Picture::Showing(still),
        Headline { title: place.to_string(), subtitle: String::new(), icon: Some(icon), says, big, aside, ..Headline::default() },
    )
}

fn light(at: LocalTime) -> Result<Light, Never> {
    let Ok(hour) = at.hour();

    Ok(match DAYLIGHT.contains(&hour) {
        true => Light::Day,
        false => Light::Night,
    })
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

    for hour in hours.iter().filter(|hour| hour.at >= this_hour).take(across) {
        let Ok(when) = hour_named(hour.at, now);
        let Ok(temperature) = degrees(hour.temperature);
        let Ok(when) = Cell::new(&when, Active::No);
        let Ok(temperature) = Cell::new(&temperature, Active::No);
        let Ok(light) = light(hour.at);
        let Ok(temperature) = match hour.code {
            Some(code) => {
                let Ok(icon) = icon(code, light);

                temperature.with_icon(icon)
            },
            None => Ok(temperature),
        };

        named.push(when);
        warm.push(temperature);
    }

    let Ok(named) = Row::naming_cells(named);
    let Ok(warm) = Row::celled(warm);

    Ok(vec![named, warm])
}

fn o_clock(named: String) -> Result<String, Never> {
    Ok(match named.as_str() {
        NOW => named,
        _ => format!("{named}:00"),
    })
}

fn hour_named(at: LocalTime, now: LocalTime) -> Result<String, Never> {
    let Ok(hour) = at.hour();
    let Ok(at) = at.on_the_hour();
    let Ok(now) = now.on_the_hour();

    Ok(match at == now {
        true => NOW.to_string(),
        false => format!("{hour:02}"),
    })
}

fn details(forecast: &Forecast) -> Result<Vec<Row>, Never> {
    let now = &forecast.now;
    let Ok(feels) = degrees(now.feels_like);
    let Ok(humidity) = percent(now.humidity);
    let wind = format!("{} km/h", now.wind.0);
    let Ok(this_hour) = now.at.on_the_hour();
    let chance = forecast.hours.iter().find(|hour| hour.at >= this_hour).and_then(|hour| hour.rain);
    let Ok(chance) = percent(match chance {
        Some(chance) => chance,
        None => Percent(0),
    });

    let Ok(feels) = detail(Icon::Thermometer, "Feels Like", Aside(&feels));
    let Ok(wind) = detail(Icon::Wind, "Wind", Aside(&wind));
    let Ok(rain) = detail(Icon::Umbrella, "Rain", Aside(&chance));
    let Ok(humidity) = detail(Icon::Humidity, "Humidity", Aside(&humidity));
    let Ok(first) = Row::celled(vec![feels, wind]);
    let Ok(second) = Row::celled(vec![rain, humidity]);

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
    let Ok(sky) = named(day.code);
    let Ok(rain) = rain(day.rain);
    let Ok(range) = range(day);
    let Ok(icon) = icon(day.code, Light::Day);
    let Ok(row) = Row::said(&format!("{when}  {sky}{rain}"), Aside(&range));

    row.picturing(Picture::Named(icon))
}

fn day_named(at: LocalTime, now: LocalTime) -> Result<String, Never> {
    let Ok(day) = at.day();
    let Ok(today) = now.day();

    match day == today {
        true => Ok(TODAY.to_string()),
        false => {
            let Ok(weekday) = at.weekday();
            let Ok(named) = weekday.named();

            Ok(named.to_string())
        }
    }
}

fn rain(chance: Option<Percent>) -> Result<String, Never> {
    Ok(match chance {
        Some(Percent(0)) | None => String::new(),
        Some(chance) => {
            let Ok(chance) = percent(chance);

            format!(", {chance}")
        }
    })
}

fn range(day: &Day) -> Result<String, Never> {
    let Ok(high) = degrees(day.high);
    let Ok(low) = degrees(day.low);

    Ok(format!("High {high}  Low {low}"))
}

fn named(code: u32) -> Result<String, Never> {
    let Ok(said) = conditions::named(code);

    Ok(match said {
        Some(said) => said.to_string(),
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

    fn day(at: LocalTime, rain: Option<Percent>) -> Day {
        Day { at, code: 61, low: Degrees(-2), high: Degrees(14), rain }
    }

    #[test]
    fn the_first_day_is_today_and_the_rest_are_named() {
        let Ok(first) = day_named(WEDNESDAY_NOON, WEDNESDAY_NOON);
        let Ok(second) = day_named(LocalTime(WEDNESDAY_NOON.0 + 86_400), WEDNESDAY_NOON);

        assert_eq!(first, "Today");
        assert_eq!(second, "Thursday");
    }

    #[test]
    fn a_day_says_its_sky_its_chance_of_rain_and_its_range() {
        let Ok(row) = day_row(&day(WEDNESDAY_NOON, Some(Percent(80))), WEDNESDAY_NOON);

        assert_eq!(row.says, "Today  Rain, 80%");
        assert_eq!(row.aside, "High 14\u{b0}  Low -2\u{b0}");
    }

    #[test]
    fn no_chance_of_rain_is_not_written_down() {
        let Ok(none) = day_row(&day(WEDNESDAY_NOON, Some(Percent(0))), WEDNESDAY_NOON);
        let Ok(unknown) = day_row(&day(WEDNESDAY_NOON, None), WEDNESDAY_NOON);

        assert_eq!(none.says, "Today  Rain");
        assert_eq!(unknown.says, "Today  Rain");
    }

    #[test]
    fn the_day_is_every_hour_of_today_and_only_this_one_is_now() {
        let hour = |at: i64, code| Hour { at: LocalTime(at), temperature: Degrees(9), rain: Some(Percent(20)), code };
        let midnight = WEDNESDAY_NOON.0 - 12 * 3_600;
        let hours = vec![hour(midnight - 3_600, Some(0)), hour(midnight, Some(0)), hour(WEDNESDAY_NOON.0, Some(61)), hour(midnight + 86_400, None)];

        let Ok(rows) = the_day(&hours, LocalTime(WEDNESDAY_NOON.0 + 900));
        let said: Vec<(&str, &str)> = rows.iter().map(|row| (row.says.as_str(), row.aside.as_str())).collect();

        assert_eq!(said, vec![("00:00  Clear, 20%", "9\u{b0}"), ("Now  Rain, 20%", "9\u{b0}")]);
    }

    #[test]
    fn the_first_hour_is_now_and_the_rest_are_the_clock() {
        let quarter_past = LocalTime(WEDNESDAY_NOON.0 + 900);

        assert_eq!(hour_named(WEDNESDAY_NOON, quarter_past), Ok("Now".to_string()));
        assert_eq!(hour_named(LocalTime(WEDNESDAY_NOON.0 + 3 * 3_600), quarter_past), Ok("15".to_string()));
    }
}
