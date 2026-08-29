//! How often each application has been opened.
//!
//! Applications come out in the order you actually use them: the ones you open
//! most, most often, and everything else alphabetically after them.

use std::collections::BTreeMap;

/// What the file says, as a count for each name.
pub fn read(said: &str) -> BTreeMap<String, u64> {
    said.lines()
        .filter_map(|line| {
            let (number, name) = line.split_once(' ')?;
            match (number.parse().ok(), name.is_empty()) {
                (Some(number), false) => Some((name.to_string(), number)),
                _ => None,
            }
        })
        .collect()
}

/// The file, as it is written back.
pub fn written(counts: &BTreeMap<String, u64>) -> String {
    counts.iter().map(|(name, number)| format!("{number} {name}\n")).collect()
}

/// One more for one of them.
pub fn bumped(mut counts: BTreeMap<String, u64>, name: &str) -> BTreeMap<String, u64> {
    *counts.entry(name.to_string()).or_insert(0) += 1;
    counts
}

/// The applications in the order they are worth showing: the ones opened most,
/// most often, and everything else alphabetically after them.
pub fn order(names: &[String], counts: &BTreeMap<String, u64>) -> Vec<String> {
    let mut order: Vec<String> = names.to_vec();
    order.sort_by_key(|name| {
        (std::cmp::Reverse(counts.get(name).copied().unwrap_or(0)), name.to_lowercase())
    });
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(said: &[&str]) -> Vec<String> {
        said.iter().map(|name| (*name).to_string()).collect()
    }

    #[test]
    fn a_count_is_a_number_and_a_name() {
        let counts = read("3 Firefox\n1 A Long Name\n");
        assert_eq!(counts["Firefox"], 3);
        assert_eq!(counts["A Long Name"], 1, "a name with spaces in it is one name");
    }

    #[test]
    fn a_line_that_is_not_a_count_is_not_one() {
        assert!(read("what\n\nFirefox 3\n").is_empty());
    }

    #[test]
    fn what_is_read_is_what_is_written() {
        let said = "1 Alacritty\n3 Firefox\n";
        assert_eq!(written(&read(said)), said);
    }

    #[test]
    fn the_ones_opened_most_come_first() {
        let counts = read("3 Firefox\n1 Alacritty\n");
        assert_eq!(
            order(&names(&["Zed", "Alacritty", "Firefox", "Blender"]), &counts),
            names(&["Firefox", "Alacritty", "Blender", "Zed"])
        );
    }

    /// Two applications opened the same number of times are a list, and a list
    /// that reorders itself between openings is a list you cannot learn.
    #[test]
    fn everything_else_is_alphabetical_whatever_case_it_is_written_in() {
        let counts = BTreeMap::new();
        assert_eq!(
            order(&names(&["gimp", "Blender", "alacritty"]), &counts),
            names(&["alacritty", "Blender", "gimp"])
        );
    }

    #[test]
    fn opening_one_counts_it() {
        let counts = bumped(read("1 Firefox\n"), "Firefox");
        assert_eq!(counts["Firefox"], 2);
        assert_eq!(bumped(counts, "Zed")["Zed"], 1);
    }
}
