//! Where this machine keeps the hour, and how it writes it down.
//!
//! Six hundred zones, which is a list nobody walks. They are not six hundred
//! flat things, though: every one of them is a place inside a part of the
//! world, and that is how somebody looks for their own -- Europe first, then
//! the city. So the list is the part of the world, and the cities are the page
//! under it, which turns one walk of six hundred into one of eleven and one of
//! forty.
//!
//! The names are the zone's own, with the underscores taken out. `tzdata`
//! spells them for machines and they read perfectly well as words --
//! `Argentina / Buenos Aires` -- and a table of prettier ones here would be a
//! second list to keep in step with the first every time the world moves a
//! border.
//!
//! Whether the hour is written 14:30 or 2:30 pm is not the zone and is not the
//! locale either, and it is not this crate\'s: the bar draws it and the panel
//! only offers it, so `console_default_applications::clock` is where it lives
//! and both of them read the one answer.

use console_core_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub zone: String,
    pub says: String,
}

pub fn zones(said: &str) -> Result<Vec<String>, Never> {
    Ok(said.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect())
}

pub fn part_of(zone: &str) -> Result<String, Never> {
    Ok(zone.split('/').next().unwrap_or_default().to_string())
}

pub fn regions(zones: &[String]) -> Result<Vec<String>, Never> {
    let mut parts: Vec<String> = Vec::new();

    for zone in zones {
        let Ok(part) = part_of(zone);

        let already = parts.contains(&part);

        match already || part.is_empty() {
            true => {},
            false => parts.push(part),
        }
    }

    parts.sort();

    Ok(parts)
}

pub fn says(zone: &str) -> Result<String, Never> {
    let after = match zone.split_once('/') {
        Some((_, after)) => after,
        None => zone,
    };

    Ok(after.replace('_', " ").replace('/', " / "))
}

pub fn places(zones: &[String], region: &str) -> Result<Vec<Place>, Never> {
    let mut kept: Vec<Place> = Vec::new();

    for zone in zones {
        let Ok(part) = part_of(zone);

        match part == region {
            true => {
                let Ok(says) = says(zone);

                kept.push(Place { zone: zone.clone(), says });
            }
            false => {},
        }
    }

    kept.sort_by_key(|place| place.says.to_lowercase());

    Ok(kept)
}

pub fn chosen(said: &str) -> Result<Option<String>, Never> {
    let zone = said.trim();

    Ok(match zone.is_empty() {
        true => None,
        false => Some(zone.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAID: &str = "\
America/Argentina/Buenos_Aires
America/New_York
Asia/Bangkok
Europe/Amsterdam
Europe/London
UTC
";

    fn zones() -> Vec<String> {
        let Ok(zones) = super::zones(SAID);

        zones
    }

    #[test]
    fn the_parts_of_the_world_are_the_first_walk_and_they_are_in_order() {
        let Ok(regions) = regions(&zones());

        assert_eq!(regions, ["America", "Asia", "Europe", "UTC"]);
    }

    #[test]
    fn a_part_of_the_world_holds_the_places_in_it_and_nothing_else() {
        let Ok(places) = places(&zones(), "Europe");
        let says: Vec<String> = places.into_iter().map(|place| place.says).collect();

        assert_eq!(says, ["Amsterdam", "London"]);
    }

    #[test]
    fn a_zone_with_a_country_inside_it_keeps_the_country_in_the_words() {
        let Ok(places) = places(&zones(), "America");
        let says: Vec<String> = places.into_iter().map(|place| place.says).collect();

        assert_eq!(says, ["Argentina / Buenos Aires", "New York"]);
    }

    #[test]
    fn the_one_that_is_a_zone_and_not_a_place_is_still_reachable() {
        let Ok(places) = places(&zones(), "UTC");

        assert_eq!(places, vec![Place { zone: "UTC".to_string(), says: "UTC".to_string() }]);
    }

    #[test]
    fn a_machine_that_will_not_say_where_it_is_is_not_a_machine_in_greenwich() {
        assert_eq!(chosen("Europe/Amsterdam\n"), Ok(Some("Europe/Amsterdam".to_string())));
        assert_eq!(chosen("   \n"), Ok(None));
    }
}
