//! How often each application has been opened.
//!
//! Applications come out in the order you actually use them: the ones you open
//! most, most often, and everything else alphabetically after them.

use console_core_never::Never;
use std::collections::BTreeMap;

const NEVER_OPENED: u64 = 0;


pub fn read(said: &str) -> Result<BTreeMap<String, u64>, Never> {
    Ok(said
        .lines()
        .filter_map(|line| {
            let (number, name) = line.split_once(' ')?;

            match (number.parse(), name.is_empty()) {
                (Ok(number), false) => Some((name.to_string(), number)),
                _ => None,
            }
        })
        .collect())
}

pub fn serialize(counts: &BTreeMap<String, u64>) -> Result<String, Never> {
    Ok(counts.iter().map(|(name, number)| format!("{number} {name}\n")).collect())
}

pub fn increment(mut counts: BTreeMap<String, u64>, name: &str) -> Result<BTreeMap<String, u64>, Never> {
    let count = counts.entry(name.to_string()).or_insert(0);
    *count = count.saturating_add(1);

    Ok(counts)
}

pub fn order(names: &[String], counts: &BTreeMap<String, u64>) -> Result<Vec<String>, Never> {
    let mut order: Vec<String> = names.to_vec();
    order.sort_by_key(|name| {
        let opened = match counts.get(name).copied() {
            Some(opened) => opened,
            None => NEVER_OPENED,
        };

        (std::cmp::Reverse(opened), name.to_lowercase())
    });

    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(said: &[&str]) -> Result<Vec<String>, Never> {
        Ok(said.iter().map(|name| (*name).to_string()).collect())
    }

    #[test]
    fn a_count_is_a_number_and_a_name() {
        let Ok(counts) = read("3 Firefox\n1 A Long Name\n");

        assert_eq!(counts.get("Firefox"), Some(&3));
        assert_eq!(counts.get("A Long Name"), Some(&1), "a name with spaces in it is one name");
    }

    #[test]
    fn a_line_that_is_not_a_count_is_not_one() {
        let Ok(counts) = read("what\n\nFirefox 3\n");

        assert!(counts.is_empty());
    }

    #[test]
    fn what_is_read_is_what_is_written() {
        let said = "1 Alacritty\n3 Firefox\n";
        let Ok(counts) = read(said);

        assert_eq!(serialize(&counts), Ok(said.to_string()));
    }

    #[test]
    fn the_ones_opened_most_come_first() {
        let Ok(counts) = read("3 Firefox\n1 Alacritty\n");
        let Ok(opened) = names(&["Zed", "Alacritty", "Firefox", "Blender"]);

        assert_eq!(order(&opened, &counts), names(&["Firefox", "Alacritty", "Blender", "Zed"]));
    }

    #[test]
    fn everything_else_is_alphabetical_whatever_case_it_is_written_in() {
        let counts = BTreeMap::new();
        let Ok(opened) = names(&["gimp", "Blender", "alacritty"]);

        assert_eq!(order(&opened, &counts), names(&["alacritty", "Blender", "gimp"]));
    }

    #[test]
    fn opening_one_counts_it() {
        let Ok(counts) = read("1 Firefox\n");
        let Ok(counts) = increment(counts, "Firefox");

        assert_eq!(counts.get("Firefox"), Some(&2));

        let Ok(counts) = increment(counts, "Zed");

        assert_eq!(counts.get("Zed"), Some(&1));
    }
}
