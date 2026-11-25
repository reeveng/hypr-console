//! The power profiles shared by Battery settings and the keyboard shortcut.

use console_core_localization::text;
use console_core_never::Never;
use console_core_words::Words;

use crate::words::Word;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Words)]
pub enum Profile {
  #[words(written = "power-saver")]
  Saving,
  #[words(written = "balanced")]
  Balanced,
  #[words(written = "performance")]
  Fast,
}

impl Profile {
  pub const ALL: [Self; 3] = [Self::Saving, Self::Balanced, Self::Fast];

  pub fn label(self) -> Result<String, Never> {
    text(&match self {
      Self::Saving => Word::LowPower,
      Self::Balanced => Word::Automatic,
      Self::Fast => Word::HighPower,
    })
  }

  pub fn next(list: &str) -> Result<Option<Self>, Never> {
    let mut available = Self::ALL.into_iter().filter(|profile| {
      let Ok(name) = profile.written();

      list.lines().any(|line| {
        let Ok(parsed) = profile_name(line);

        parsed == Some(name)
      })
    });
    let first = available.next();
    let current = list.lines().find_map(|line| {
      let heading = line.trim_start().strip_prefix('*')?;
      let Ok(parsed) = profile_name(heading);

      parsed
    });
    let next = first
      .into_iter()
      .chain(available)
      .skip_while(|profile| {
        let Ok(name) = profile.written();

        Some(name) != current
      })
      .nth(1);

    Ok(next.or(first))
  }
}

fn profile_name(line: &str) -> Result<Option<&str>, Never> {
  Ok(line.trim().trim_start_matches('*').trim().strip_suffix(':'))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_shortcut_cycles_only_available_profiles() {
    assert_eq!(
      Profile::next("  performance:\n* balanced:\n  power-saver:\n"),
      Ok(Some(Profile::Fast))
    );
    assert_eq!(
      Profile::next("* performance:\n  balanced:\n  power-saver:\n"),
      Ok(Some(Profile::Saving))
    );
    assert_eq!(
      Profile::next("* balanced:\n  power-saver:\n"),
      Ok(Some(Profile::Saving))
    );
  }
}
