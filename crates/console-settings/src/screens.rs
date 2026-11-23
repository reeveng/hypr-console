//! Where a screen's own answers are kept.
//!
//! The size and the turn were one word each for the whole machine, which is
//! right for exactly as long as there is one screen. Two is enough to break
//! it in both directions: a monitor on a desk wearing the quarter turn a
//! handheld is held at comes up on its side, and a handheld wearing the
//! density someone chose for a 27 inch panel comes up at a size no one can
//! read. Neither of those is a setting gone wrong -- both are one answer being
//! asked of two different questions.
//!
//! So an answer is kept under the connector it is about:
//! `~/.config/console/screens/eDP-1/turn`, beside a `scale` of its own. A
//! screen with nothing under its name wears what the compositor gave it, which
//! is what `console apply` wrote out of the panel's own mode -- so a monitor
//! plugged in for the first time is upright at its own density rather than
//! wearing whatever the last screen was set to.
//!
//! A connector is named by the kernel and is letters, digits and hyphens.
//! Anything else is not a connector, and [`Output::under`] says so rather than
//! joining it onto a path: a name with a separator in it is a file written
//! somewhere no one asked for, and the one thing a directory named after
//! someone else's string must not do is leave the directory.

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use console_core_never::Never;

pub const UNDER: &str = "screens";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Output<'a>(pub &'a str);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unnamed {
  NotAConnector(String),
  NotAFileName(String),
}

impl std::fmt::Display for Unnamed {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Unnamed::NotAConnector(said) => {
        write!(to, "{said:?} is not what a kernel calls a connector")
      }
      Unnamed::NotAFileName(said) => {
        write!(to, "{said:?} is not a settings file name")
      }
    }
  }
}

impl std::error::Error for Unnamed {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spelled {
  LikeAConnector,
  LikeSomethingElse,
}

fn classify_name(named: &str) -> Result<Spelled, Never> {
  let letters = named
    .chars()
    .all(|one| one.is_ascii_alphanumeric() || one == '-');

  Ok(
    match ::core::primitive::bool::then_some(named.is_empty() || !letters, ()) {
      ::core::option::Option::Some(()) => Spelled::LikeSomethingElse,
      ::core::option::Option::None => Spelled::LikeAConnector,
    },
  )
}

struct FileName<'a>(&'a OsStr);

impl AsRef<Path> for FileName<'_> {
  fn as_ref(&self) -> &Path {
    Path::new(self.0)
  }
}

impl Output<'_> {
  pub fn under(self, home: &Path) -> Result<PathBuf, Unnamed> {
    let named = self.0.trim();
    let Ok(spelled) = classify_name(named);

    match spelled {
      Spelled::LikeAConnector => {}
      Spelled::LikeSomethingElse => return Err(Unnamed::NotAConnector(self.0.to_string())),
    }

    let Ok(ours) = console_core_places::Base::Configuration.application_under(home);

    let connector = FileName(OsStr::new(named));

    Ok(ours.join(UNDER).join(connector))
  }

  pub fn path_for(self, home: &Path, called: &str) -> Result<PathBuf, Unnamed> {
    let under = self.under(home)?;
    let path = Path::new(called);
    let mut components = path.components();
    let file = match (components.next(), components.next()) {
      (Some(Component::Normal(name)), None) => FileName(name),
      _ => return Err(Unnamed::NotAFileName(called.to_string())),
    };

    Ok(under.join(file))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn at(output: Output<'_>, called: &str) -> Result<PathBuf, Unnamed> {
    output.path_for(Path::new("/home/ada"), called)
  }

  #[test]
  fn a_screens_answers_are_kept_in_a_directory_named_after_it() {
    assert_eq!(
      at(Output("eDP-1"), "turn"),
      Ok(PathBuf::from(
        "/home/ada/.config/console/screens/eDP-1/turn"
      ))
    );
    assert_eq!(
      at(Output("DP-3"), "scale"),
      Ok(PathBuf::from(
        "/home/ada/.config/console/screens/DP-3/scale"
      ))
    );
  }

  #[test]
  fn two_screens_do_not_share_an_answer() {
    assert_ne!(at(Output("eDP-1"), "turn"), at(Output("HDMI-A-1"), "turn"));
  }

  #[test]
  fn an_answer_stays_under_its_connector() {
    assert_eq!(
      at(Output("eDP-1"), "../turn"),
      Err(Unnamed::NotAFileName("../turn".to_string()))
    );
    assert_eq!(
      at(Output("eDP-1"), "/scale"),
      Err(Unnamed::NotAFileName("/scale".to_string()))
    );
  }

  #[test]
  fn a_name_that_could_leave_the_directory_is_not_a_connector() {
    assert_eq!(
      at(Output("../../.."), "turn"),
      Err(Unnamed::NotAConnector("../../..".to_string()))
    );
    assert_eq!(
      at(Output("eDP-1/../../x"), "turn"),
      Err(Unnamed::NotAConnector("eDP-1/../../x".to_string()))
    );
    assert_eq!(
      at(Output(""), "turn"),
      Err(Unnamed::NotAConnector(String::new()))
    );
  }
}
