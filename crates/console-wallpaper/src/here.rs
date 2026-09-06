//! Where the machine is, taken from the timezone it is already keeping.
//!
//! The wallpaper wants a place for three things: which hemisphere it is in, so
//! the seasons are the right way round; the sun's height, so it knows a dawn
//! from a dusk; and a pair of numbers to ask the weather service about. None of
//! them is a question about a street, and none of them wants an address written
//! down anywhere.
//!
//! So the place is not stored. `/etc/localtime` already says what zone the
//! clock is keeping, and `zone1970.tab` beside it already says roughly where
//! each zone is, because that is how the timezone database describes itself.
//! Between them the machine can answer where it is without anybody having told
//! it, and what it answers is the zone's own city rather than the one somebody
//! is standing in.
//!
//! That is coarser than a person's address by design, and coarse is enough. A
//! zone's city is often a few hundred kilometres from the person holding the
//! machine, and a country that keeps another country's zone is further still.
//! Measured at two hundred and fifty kilometres, which is about the worst a
//! zone gets, the two disagree about the part of the day for forty-six minutes
//! out of every fourteen hundred and forty, and never once about the season.
//! Eight minutes at each of the day's bounds is less than the time a picture
//! takes to be noticed, and the season is the thing a picture would be most
//! obviously wrong about.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use console_never::Never;

use crate::sun::Where;

pub const CLOCK: &str = "/etc/localtime";

pub const ZONES: [&str; 2] = [
    "/usr/share/zoneinfo/zone1970.tab",
    "/usr/share/zoneinfo/zone.tab",
];

const KEEP_FOR: Duration = Duration::from_secs(12 * 60 * 60);

static KEPT: Mutex<Option<(Instant, Where)>> = Mutex::new(None);

pub const NOWHERE: Where = Where {
    latitude: 51.48,
    longitude: 0.0,
};

pub fn here() -> Result<Where, Never> {
    let mut kept = match KEPT.lock() {
        Ok(kept) => kept,
        Err(held) => held.into_inner(),
    };

    match *kept {
        Some((asked, at)) => match asked.elapsed() < KEEP_FOR {
            true => return Ok(at),
            false => {},
        },
        None => {},
    }

    let at = asking()?;

    *kept = Some((Instant::now(), at));

    Ok(at)
}

fn asking() -> Result<Where, Never> {
    let Some(zone) = zone()? else { return Ok(NOWHERE) };

    for named in ZONES {
        let table = match std::fs::read_to_string(named) {
            Ok(table) => table,
            Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => continue,
            Err(fault) => {
                eprintln!("console-sky: {named}: {fault}");

                continue;
            }
        };

        let found = at(&zone, &table)?;

        match found {
            Some(found) => return Ok(found),
            None => {},
        }
    }

    Ok(NOWHERE)
}

pub fn zone() -> Result<Option<String>, Never> {
    let at = match std::fs::read_link(CLOCK) {
        Ok(at) => at,
        Err(fault)
            if matches!(
                fault.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidInput
            ) =>
        {
            return Ok(None);
        }
        Err(fault) => {
            eprintln!("console-sky: {CLOCK}: {fault}");

            return Ok(None);
        }
    };

    let Some(said) = at.to_str() else { return Ok(None) };

    Ok(said.split_once("zoneinfo/").map(|(_, zone)| zone.to_string()))
}

pub fn at(zone: &str, table: &str) -> Result<Option<Where>, Never> {
    for line in table.lines() {
        match line.starts_with('#') {
            true => continue,
            false => {},
        }

        let mut columns = line.split('\t');

        let Some(place) = columns.nth(1) else { continue };

        let Some(said) = columns.next() else { continue };

        match said == zone {
            true => {},
            false => continue,
        }

        let found = pair(place)?;

        match found {
            Some(found) => return Ok(Some(found)),
            None => {},
        }
    }

    Ok(None)
}

fn pair(said: &str) -> Result<Option<Where>, Never> {
    let found = said
        .char_indices()
        .skip(1)
        .find(|(_, c)| *c == '+' || *c == '-');

    let Some(found) = found else { return Ok(None) };

    let at = found.0;

    let Some(before) = said.get(..at) else { return Ok(None) };

    let Some(after) = said.get(at..) else { return Ok(None) };

    let Some(latitude) = degrees(before)? else { return Ok(None) };

    let Some(longitude) = degrees(after)? else { return Ok(None) };

    Ok(Some(Where { latitude, longitude }))
}

fn degrees(said: &str) -> Result<Option<f64>, Never> {
    let Some(first) = said.as_bytes().first() else { return Ok(None) };

    let sign = match first {
        b'+' => 1.0,
        b'-' => -1.0,
        _ => return Ok(None),
    };

    let Some(digits) = said.get(1..) else { return Ok(None) };

    match digits.bytes().all(|byte| byte.is_ascii_digit()) {
        true => {},
        false => return Ok(None),
    }

    let (whole, rest) = match digits.len() {
        4 | 5 => digits.split_at(digits.len().saturating_sub(2)),
        6 | 7 => digits.split_at(digits.len().saturating_sub(4)),
        _ => return Ok(None),
    };

    let Some(first_two) = rest.get(..2) else { return Ok(None) };

    let (Ok(minutes), Ok(whole)) = (first_two.parse::<f64>(), whole.parse::<f64>()) else {
        return Ok(None);
    };

    let seconds: f64 = match rest.get(2..) {
        Some(said) if !said.is_empty() => match said.parse() {
            Ok(seconds) => seconds,
            Err(_) => return Ok(None),
        },
        Some(_) | None => 0.0,
    };

    Ok(Some(sign * (whole + minutes / 60.0 + seconds / 3600.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "# a comment, which is not a row\n\
                         AD\t+4230+00131\tEurope/Andorra\n\
                         CA\t+4439-06336\tAmerica/Halifax\n\
                         NZ\t-3652+17446\tPacific/Auckland\n\
                         AQ\t-720041+0023206\tAntarctica/Troll\n";

    #[test]
    fn a_zone_is_the_tail_of_where_the_clock_points() {
        assert_eq!(
            "/usr/share/zoneinfo/Europe/Andorra"
                .split_once("zoneinfo/")
                .map(|(_, zone)| zone),
            Some("Europe/Andorra")
        );
    }

    #[test]
    fn a_zone_is_looked_up_by_its_name_and_not_by_its_country() {
        let Ok(andorra) = at("Europe/Andorra", TABLE);

        let andorra = andorra.expect("a place");
        assert!((andorra.latitude - 42.5).abs() < 0.001, "{andorra:?}");
        assert!((andorra.longitude - 1.5167).abs() < 0.001, "{andorra:?}");
    }

    #[test]
    fn a_place_south_or_west_is_a_negative_number() {
        let Ok(west) = at("America/Halifax", TABLE);

        let west = west.expect("a place");

        assert!(west.latitude > 0.0 && west.longitude < 0.0, "{west:?}");

        let Ok(south) = at("Pacific/Auckland", TABLE);

        let south = south.expect("a place");
        assert!(south.latitude < 0.0 && south.longitude > 0.0, "{south:?}");
    }

    #[test]
    fn a_row_written_to_the_second_is_read_to_the_second() {
        let Ok(troll) = at("Antarctica/Troll", TABLE);

        let troll = troll.expect("a place");
        assert!((troll.latitude + 72.0114).abs() < 0.001, "{troll:?}");
        assert!((troll.longitude - 2.5350).abs() < 0.001, "{troll:?}");
    }

    #[test]
    fn a_zone_the_table_does_not_hold_is_no_place() {
        assert_eq!(at("Mars/Olympus", TABLE), Ok(None));
    }

    #[test]
    fn nothing_readable_is_no_place() {
        assert_eq!(at("Europe/Andorra", ""), Ok(None));
        assert_eq!(at("Europe/Andorra", "not a row at all"), Ok(None));
        assert_eq!(
            at("Europe/Andorra", "AD\tnot a place\tEurope/Andorra"),
            Ok(None)
        );
    }

    #[test]
    fn this_machine_says_where_it_is() {
        let Ok(asking) = asking();

        assert!(asking.latitude.abs() <= 90.0);
        assert!(asking.longitude.abs() <= 180.0);
    }

    #[test]
    fn an_answer_is_kept_rather_than_asked_for_twice() {
        assert_eq!(here(), here());
    }
}
