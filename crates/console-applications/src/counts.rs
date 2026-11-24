//! How often each application has been opened.
//!
//! Applications come out in the order you actually use them: the ones you open
//! most, most often, and everything else alphabetically after them.

use console_core_never::Never;
use std::collections::BTreeMap;

const NEVER_OPENED: u64 = 0;

pub const RENAMED: [(&str, &str); 11] = [
  ("Books", "Console Books"),
  ("Calculator", "Console Calculator"),
  ("Camera", "Console Camera"),
  ("Clipboard History", "Console Clipboard History"),
  ("Download", "Console Downloads"),
  ("Files", "Console Files"),
  ("Habits", "Console Habits"),
  ("Music", "Console Music"),
  ("Notes", "Console Notes"),
  ("Task Manager", "Console Task Manager"),
  ("Viewer", "Console Viewer"),
];

pub fn read(said: &str) -> Result<BTreeMap<String, u64>, Never> {
  Ok(
    said
      .lines()
      .filter_map(|line| {
        let (number, name) = line.split_once(' ')?;

        match (number.parse(), name.is_empty()) {
          (Ok(number_2), false) => Some((name.to_string(), number_2)),
          _ => None,
        }
      })
      .collect(),
  )
}

pub fn serialize(counts: &BTreeMap<String, u64>) -> Result<String, Never> {
  Ok(
    counts
      .iter()
      .map(|(name, number)| format!("{number} {name}\n"))
      .collect(),
  )
}

pub fn increment(
  mut counts: BTreeMap<String, u64>,
  name: &str,
) -> Result<BTreeMap<String, u64>, Never> {
  let count = counts.entry(name.to_string()).or_insert(0);
  *count = count.saturating_add(1);

  Ok(counts)
}

pub fn order(names: &[String], counts: &BTreeMap<String, u64>) -> Result<Vec<String>, Never> {
  let installed: std::collections::BTreeSet<&str> = names.iter().map(String::as_str).collect();
  let previous: BTreeMap<&str, u64> = RENAMED
    .into_iter()
    .filter(|(old, _)| !installed.contains(old))
    .filter_map(|(old, new)| counts.get(old).map(|count| (new, *count)))
    .collect();
  let mut order: Vec<String> = names.to_vec();
  order.sort_by_key(|name| {
    let opened = match counts.get(name).copied() {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => NEVER_OPENED,
    };
    let opened_2 = opened.saturating_add(match previous.get(name.as_str()).copied() {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => NEVER_OPENED,
    });

    (std::cmp::Reverse(opened_2), name.to_lowercase())
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
    assert_eq!(
      counts.get("A Long Name"),
      Some(&1),
      "a name with spaces in it is one name"
    );
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

    assert_eq!(
      order(&opened, &counts),
      names(&["Firefox", "Alacritty", "Blender", "Zed"])
    );
  }

  #[test]
  fn a_renamed_application_keeps_its_count_if_the_old_label_is_free() {
    let Ok(counts) = read("3 Books\n2 Console Books\n4 Firefox\n");
    let Ok(opened) = names(&["Console Books", "Firefox"]);
    assert_eq!(
      order(&opened, &counts),
      names(&["Console Books", "Firefox"])
    );

    let Ok(opened_2) = names(&["Books", "Console Books", "Firefox"]);
    assert_eq!(
      order(&opened_2, &counts),
      names(&["Firefox", "Books", "Console Books"])
    );
  }

  #[test]
  fn everything_else_is_alphabetical_whatever_case_it_is_written_in() {
    let counts = BTreeMap::new();
    let Ok(opened) = names(&["gimp", "Blender", "alacritty"]);

    assert_eq!(
      order(&opened, &counts),
      names(&["alacritty", "Blender", "gimp"])
    );
  }

  #[test]
  fn opening_one_counts_it() {
    let Ok(counts) = read("1 Firefox\n");
    let Ok(counts_2) = increment(counts, "Firefox");

    assert_eq!(counts_2.get("Firefox"), Some(&2));

    let Ok(counts_3) = increment(counts_2, "Zed");

    assert_eq!(counts_3.get("Zed"), Some(&1));
  }
}
