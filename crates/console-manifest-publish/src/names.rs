//! Scan the export for known personal identifiers collected on the build
//! machine or supplied in a private file. Match raw bytes in both paths and
//! contents so a binary file with an embedded home path can be checked too.

use console_core_never::Never;

mod identity;

pub use identity::forbidden_names;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watched {
  pub name: String,
  pub what: &'static str,
}

fn push(names: &mut Vec<Watched>, found: Watched) -> Result<(), Never> {
  let Watched { name, what } = found;
  let name_2 = name.trim().to_ascii_lowercase();

  let worth = name_2.len() > 2 && !names.iter().any(|held: &Watched| held.name == name_2);

  match ::core::primitive::bool::then_some(worth, ()) {
    ::core::option::Option::Some(()) => names.push(Watched { name: name_2, what }),
    ::core::option::Option::None => {}
  }

  Ok(())
}

pub fn leaks<'a>(text: &[u8], names: &'a [Watched]) -> Result<Option<&'a Watched>, Never> {
  Ok(names.iter().find(|watched| {
    let Ok(says) = says(text, Name(watched.name.as_bytes()));

    says == Spelling::TheName
  }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Name<'a>(&'a [u8]);

fn says(text: &[u8], name: Name<'_>) -> Result<Spelling, Never> {
  let name_2 = name.0;

  let said = match name_2.len() {
    0 => false,
    long => text
      .windows(long.max(1))
      .enumerate()
      .filter(|(_, window)| window.eq_ignore_ascii_case(name_2))
      .any(|(at, _)| {
        let before = at
          .checked_sub(1)
          .and_then(|before| text.get(before).copied());
        let after = text.get(at.saturating_add(long)).copied();
        let alone = ![before, after]
          .into_iter()
          .flatten()
          .any(|byte| byte.is_ascii_alphanumeric() || byte == b'_');

        alone
      }),
  };

  Ok(match ::core::primitive::bool::then_some(said, ()) {
    ::core::option::Option::Some(()) => Spelling::TheName,
    ::core::option::Option::None => Spelling::JustLetters,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spelling {
  TheName,
  JustLetters,
}

#[cfg(test)]
mod tests {
  use super::*;

  fn watching(names: &[&str]) -> Result<Vec<Watched>, Never> {
    let mut watched = Vec::new();

    for name in names {
      let Ok(()) = push(
        &mut watched,
        Watched {
          name: name.to_string(),
          what: "someone",
        },
      );
    }

    Ok(watched)
  }

  fn leaked<'a>(text: &str, names: &'a [Watched]) -> Result<Option<&'a Watched>, Never> {
    leaks(text.as_bytes(), names)
  }

  #[test]
  fn a_name_that_is_said_is_the_name_that_comes_back() {
    let Ok(names) = watching(&["ada", "her-laptop"]);
    let Ok(both) = leaked("ada on her-laptop", &names);
    let Ok(one) = leaked("on her-laptop", &names);

    assert_eq!(both.map(|watched| watched.name.as_str()), Some("ada"));
    assert_eq!(one.map(|watched| watched.name.as_str()), Some("her-laptop"));
  }

  #[test]
  fn text_saying_none_of_them_says_nothing() {
    let Ok(names) = watching(&["ada"]);

    assert_eq!(leaked("a handheld belonging to a player", &names), Ok(None));
  }

  #[test]
  fn a_name_inside_a_longer_word_was_not_said() {
    let Ok(names) = watching(&["nimbus"]);

    assert_eq!(
      leaked("nimbusos-gamescope-autologin.service", &names),
      Ok(None)
    );
    assert_eq!(leaked("because NimbusOS put it there", &names), Ok(None));
    assert!(matches!(leaked("root@nimbus", &names), Ok(Some(_))));
    assert!(matches!(
      leaked("ssh://root@nimbus-handheld/etc/console", &names),
      Ok(Some(_))
    ));
    assert!(matches!(
      leaked("the nimbus in question", &names),
      Ok(Some(_))
    ));
  }

  #[test]
  fn a_name_in_a_path_was_said() {
    let Ok(names) = watching(&["ada"]);

    assert!(matches!(
      leaked("files/home/ada/.config", &names),
      Ok(Some(_))
    ));
    assert_eq!(leaked("files/home/adam/.config", &names), Ok(None));
    assert_eq!(leaked("the adage about it", &names), Ok(None));
  }

  #[test]
  fn a_name_with_a_capital_is_the_same_name() {
    let Ok(names) = watching(&["ada"]);

    assert!(matches!(leaked("Signed-off-by: ADA", &names), Ok(Some(_))));
    assert!(matches!(
      leaked("files/home/Ada/.config", &names),
      Ok(Some(_))
    ));
  }

  #[test]
  fn a_name_watched_for_with_a_capital_is_found_without_one() {
    let Ok(names) = watching(&["Nimbus"]);

    assert!(matches!(leaked("root@nimbus", &names), Ok(Some(_))));
  }

  #[test]
  fn a_file_that_is_not_text_is_still_read() {
    let Ok(names) = watching(&["ada"]);
    let mut picture = vec![0x89, b'P', b'N', b'G', 0xff, 0xfe, 0x00];

    picture.extend_from_slice(b"/home/ada/Pictures");
    picture.extend_from_slice(&[0xc3, 0x28, 0x00]);

    let Ok(found) = leaks(&picture, &names);

    assert!(found.is_some());
  }

  #[test]
  fn a_name_too_short_to_mean_anything_is_not_watched_for() {
    let Ok(names) = watching(&["", "  ", "al", "ada"]);

    assert_eq!(
      names.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(),
      vec!["ada"]
    );
  }
}
