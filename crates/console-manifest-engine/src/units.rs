//! What a unit names, and what has to be told a file changed.

use std::collections::BTreeSet;

use console_core_never::Never;

pub fn named_by(unit: &str) -> Result<BTreeSet<String>, Never> {
  Ok(
    unit
      .lines()
      .filter_map(|line| line.split_once('='))
      .filter(|(key, _)| {
        key
          .strip_prefix("Exec")
          .is_some_and(|rest| rest.chars().all(char::is_alphanumeric))
      })
      .flat_map(|(_, command)| command.split_whitespace())
      .map(|word| word.trim_start_matches(['-', '@', ':', '+', '!']))
      .filter(|word| word.starts_with('/'))
      .map(str::to_owned)
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_program_a_unit_runs_is_named_by_it() {
    let unit = "[Service]\nExecStart=/usr/local/bin/controller-desktop\n";
    let Ok(named) = named_by(unit);

    assert!(named.contains("/usr/local/bin/controller-desktop"));
  }

  #[test]
  fn an_argument_is_named_too() {
    let unit = "[Service]\nExecStart=/usr/local/bin/console-paper img /usr/share/backgrounds/console.webp\n";
    let Ok(named) = named_by(unit);

    assert!(
      named.contains("/usr/share/backgrounds/console.webp"),
      "{named:?}"
    );
  }

  #[test]
  fn every_kind_of_exec_line_is_read() {
    let unit = "ExecStartPre=/a\nExecStart=/b\nExecStop=/c\nExecReload=/d\n";
    let Ok(named) = named_by(unit);

    assert_eq!(named.len(), 4);
  }

  #[test]
  fn the_prefixes_systemd_allows_are_not_part_of_the_path() {
    let unit = "ExecStartPre=-/usr/bin/rm\nExecStart=+@!/usr/bin/thing\n";
    let Ok(named) = named_by(unit);

    assert!(named.contains("/usr/bin/rm"), "{named:?}");
    assert!(named.contains("/usr/bin/thing"), "{named:?}");
  }

  #[test]
  fn a_word_that_is_not_a_path_is_not_a_file() {
    let unit = "ExecStart=/usr/bin/thing --flag value -x\n";

    assert_eq!(
      named_by(unit),
      Ok(BTreeSet::from(["/usr/bin/thing".to_string()]))
    );
  }

  #[test]
  fn a_setting_that_merely_starts_with_exec_is_not_an_exec_line() {
    let unit = "Execute_this=/nope\nExecStart=/yes\n";

    assert_eq!(named_by(unit), Ok(BTreeSet::from(["/yes".to_string()])));
  }
}
