//! How often each application has been opened.
//!
//! Applications come out in the order you actually use them: the ones you open
//! most, most often, and everything else alphabetically after them.

use console_core_never::Never;
use std::collections::BTreeMap;

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

pub fn written(counts: &BTreeMap<String, u64>) -> Result<String, Never> {
    Ok(counts.iter().map(|(name, number)| format!("{number} {name}\n")).collect())
}

pub fn bumped(mut counts: BTreeMap<String, u64>, name: &str) -> Result<BTreeMap<String, u64>, Never> {
    let count = counts.entry(name.to_string()).or_insert(0);
    *count = count.saturating_add(1);

    Ok(counts)
}

pub fn order(names: &[String], counts: &BTreeMap<String, u64>) -> Result<Vec<String>, Never> {
    let mut order: Vec<String> = names.to_vec();
    order.sort_by_key(|name| {
        (std::cmp::Reverse(counts.get(name).copied().unwrap_or(0)), name.to_lowercase())
    });

    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    fn names(said: &[&str]) -> Vec<String> {
        said.iter().map(|name| (*name).to_string()).collect()
    }

    #[test]
    fn a_count_is_a_number_and_a_name() {
        let counts = ok(read("3 Firefox\n1 A Long Name\n"));
        assert_eq!(counts["Firefox"], 3);
        assert_eq!(counts["A Long Name"], 1, "a name with spaces in it is one name");
    }

    #[test]
    fn a_line_that_is_not_a_count_is_not_one() {
        assert!(ok(read("what\n\nFirefox 3\n")).is_empty());
    }

    #[test]
    fn what_is_read_is_what_is_written() {
        let said = "1 Alacritty\n3 Firefox\n";
        assert_eq!(ok(written(&ok(read(said)))), said);
    }

    #[test]
    fn the_ones_opened_most_come_first() {
        let counts = ok(read("3 Firefox\n1 Alacritty\n"));
        assert_eq!(
            ok(order(&names(&["Zed", "Alacritty", "Firefox", "Blender"]), &counts)),
            names(&["Firefox", "Alacritty", "Blender", "Zed"])
        );
    }

    #[test]
    fn everything_else_is_alphabetical_whatever_case_it_is_written_in() {
        let counts = BTreeMap::new();
        assert_eq!(
            ok(order(&names(&["gimp", "Blender", "alacritty"]), &counts)),
            names(&["alacritty", "Blender", "gimp"])
        );
    }

    #[test]
    fn opening_one_counts_it() {
        let counts = ok(bumped(ok(read("1 Firefox\n")), "Firefox"));
        assert_eq!(counts["Firefox"], 2);
        assert_eq!(ok(bumped(counts, "Zed"))["Zed"], 1);
    }
}
