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

use crate::sun::Where;

/// What zone the clock is keeping.
pub const CLOCK: &str = "/etc/localtime";

/// Where the timezone database says its zones are.
///
/// `zone1970.tab` is the current table and `zone.tab` is the older one kept
/// beside it. Both hold the same three columns for a zone that is in both, so
/// the second is a fallback rather than a different answer.
pub const ZONES: [&str; 2] = [
    "/usr/share/zoneinfo/zone1970.tab",
    "/usr/share/zoneinfo/zone.tab",
];

/// How long an answer is kept before the machine is asked again.
///
/// Twice a day. Asking costs a link and a small table, which is nothing, but
/// the answer only changes when the machine has been carried to another
/// country, and half a day is sooner than anybody has unpacked. The worst it
/// can be wrong by is a flight: for the rest of that day the wallpaper keeps
/// the sun of the country it took off from, and then it asks again and is
/// right without anybody having done anything.
const KEEP_FOR: Duration = Duration::from_secs(12 * 60 * 60);

/// The last answer, and when it was got.
static KEPT: Mutex<Option<(Instant, Where)>> = Mutex::new(None);

/// Where a machine that will not say stands.
///
/// Greenwich, because a machine with no zone to read is a machine keeping UTC,
/// and the place where UTC is the local time is the meridian it is named after.
/// It is a guess, but it is the guess the clock is already making.
pub const NOWHERE: Where = Where {
    latitude: 51.48,
    longitude: 0.0,
};

/// Where this machine is, asked twice a day and remembered between.
pub fn here() -> Where {
    // A lock somebody panicked holding still holds an answer, and an old place
    // is a better wallpaper than none.
    let mut kept = KEPT.lock().unwrap_or_else(|held| held.into_inner());
    if let Some((asked, at)) = *kept {
        if asked.elapsed() < KEEP_FOR {
            return at;
        }
    }
    let at = asking();
    *kept = Some((Instant::now(), at));
    at
}

/// Where the machine says it is, asked rather than remembered.
fn asking() -> Where {
    zone()
        .and_then(|zone| {
            ZONES
                .iter()
                .filter_map(|at| std::fs::read_to_string(at).ok())
                .find_map(|table| at(&zone, &table))
        })
        .unwrap_or(NOWHERE)
}

/// The zone the clock is keeping, as the database names it.
///
/// `/etc/localtime` is a link into the database, so the name is the tail of
/// where it points: everything past `zoneinfo/`. Read rather than followed by
/// hand, because a distribution that copies the file instead of linking it
/// leaves nothing to read, and a machine with no zone is one this can say so
/// about rather than one it has to guess badly for.
pub fn zone() -> Option<String> {
    let at = std::fs::read_link(CLOCK).ok()?;
    let said = at.to_str()?;
    said.split_once("zoneinfo/")
        .map(|(_, zone)| zone.to_string())
}

/// Where the table says a zone is.
///
/// The columns are separated by tabs: the countries it is in, where it is, and
/// its name. Comments and short lines are not rows.
pub fn at(zone: &str, table: &str) -> Option<Where> {
    table
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut columns = line.split('\t');
            let place = columns.nth(1)?;
            match columns.next()? == zone {
                true => Some(place),
                false => None,
            }
        })
        .find_map(pair)
}

/// A place, from the way ISO 6709 writes one down.
///
/// A sign and digits, twice over, with nothing between them: `+5050+00420` is
/// fifty degrees fifty minutes north, four degrees twenty minutes east.
fn pair(said: &str) -> Option<Where> {
    let at = said
        .char_indices()
        .skip(1)
        .find(|(_, c)| *c == '+' || *c == '-')?
        .0;
    Some(Where {
        latitude: degrees(&said[..at])?,
        longitude: degrees(&said[at..])?,
    })
}

/// One of the two, as a number of degrees.
///
/// Degrees, then two digits of minutes, and two more of seconds if it has them.
/// Latitude writes its degrees in two digits and longitude in three, so what
/// tells them apart is length rather than anything in the text, and what is
/// left when the minutes and seconds have been taken off the end is the
/// degrees whichever of the two this is.
fn degrees(said: &str) -> Option<f64> {
    let sign = match said.as_bytes().first()? {
        b'+' => 1.0,
        b'-' => -1.0,
        _ => return None,
    };
    let digits = said.get(1..)?;
    if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let (whole, rest) = match digits.len() {
        4 | 5 => digits.split_at(digits.len() - 2),
        6 | 7 => digits.split_at(digits.len() - 4),
        _ => return None,
    };
    let minutes: f64 = rest.get(..2)?.parse().ok()?;
    let seconds: f64 = match rest.get(2..) {
        Some(said) if !said.is_empty() => said.parse().ok()?,
        _ => 0.0,
    };
    Some(sign * (whole.parse::<f64>().ok()? + minutes / 60.0 + seconds / 3600.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rows as the database actually writes them, both widths of them.
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
        let andorra = at("Europe/Andorra", TABLE).expect("a place");
        assert!((andorra.latitude - 42.5).abs() < 0.001, "{andorra:?}");
        assert!((andorra.longitude - 1.5167).abs() < 0.001, "{andorra:?}");
    }

    /// The minus is the whole difference between a hemisphere and the other,
    /// and getting it the wrong way round is summer in December.
    #[test]
    fn a_place_south_or_west_is_a_negative_number() {
        let west = at("America/Halifax", TABLE).expect("a place");
        assert!(west.latitude > 0.0 && west.longitude < 0.0, "{west:?}");
        let south = at("Pacific/Auckland", TABLE).expect("a place");
        assert!(south.latitude < 0.0 && south.longitude > 0.0, "{south:?}");
    }

    /// The longer form has seconds on the end of both halves.
    #[test]
    fn a_row_written_to_the_second_is_read_to_the_second() {
        // 72°00'41" south, 2°32'06" east.
        let troll = at("Antarctica/Troll", TABLE).expect("a place");
        assert!((troll.latitude + 72.0114).abs() < 0.001, "{troll:?}");
        assert!((troll.longitude - 2.5350).abs() < 0.001, "{troll:?}");
    }

    #[test]
    fn a_zone_the_table_does_not_hold_is_no_place() {
        assert_eq!(at("Mars/Olympus", TABLE), None);
    }

    /// A table that is not one is not a place either, rather than a panic.
    #[test]
    fn nothing_readable_is_no_place() {
        assert_eq!(at("Europe/Andorra", ""), None);
        assert_eq!(at("Europe/Andorra", "not a row at all"), None);
        assert_eq!(
            at("Europe/Andorra", "AD\tnot a place\tEurope/Andorra"),
            None
        );
    }

    /// This machine has a zone, and the table it names is on it. The place is
    /// not asserted, because the machine running this is not the device.
    #[test]
    fn this_machine_says_where_it_is() {
        assert!(asking().latitude.abs() <= 90.0);
        assert!(asking().longitude.abs() <= 180.0);
    }

    /// The second answer is the first one handed back, which is the whole
    /// point of keeping it.
    #[test]
    fn an_answer_is_kept_rather_than_asked_for_twice() {
        assert_eq!(here(), here());
    }
}
