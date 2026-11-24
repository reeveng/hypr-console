//! PlasmaLogin's choice of the next desktop session.

use std::fs;
use std::io;

use console_core_ini_files::{Key, Under, field};
use console_core_never::Never;
use console_session::{DESKTOP_ENTRY, LoginBackend};

const CHOSEN: &str = "/etc/plasmalogin.conf.d/zz-steamos-autologin.conf";

const AUTOLOGIN: Under<'static> = Under("Autologin");

const SESSION: Key<'static> = Key("Session");

fn parse_session(said: Option<&str>) -> Result<String, Never> {
  let named = match said {
    Some(said_2) => {
      let Ok(named) = field(said_2, AUTOLOGIN, SESSION);

      named
    }
    None => None,
  };

  Ok(match named.map(str::trim) {
    Some("") | None => DESKTOP_ENTRY.to_string(),
    Some(named_2) => {
      match ::core::primitive::bool::then_some(named_2.ends_with(".desktop"), ()) {
        ::core::option::Option::Some(()) => named_2.to_string(),
        ::core::option::Option::None => format!("{named}.desktop", named = named_2),
      }
    }
  })
}

pub struct PlasmaLogin;

impl LoginBackend for PlasmaLogin {
  fn next_entry(&self) -> Result<String, Never> {
    match fs::read_to_string(CHOSEN) {
      Ok(text) => parse_session(Some(&text)),
      Err(fault) => match bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
        Some(()) => parse_session(None),
        None => {
          console_core_logging::warn!(
            "cannot read {CHOSEN}, so the Console desktop starts: {fault}"
          );
          parse_session(None)
        }
      },
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_machine_that_never_chose_starts_this_desktop() {
    let Ok(none) = parse_session(None);
    let Ok(empty) = parse_session(Some("[Autologin]\nSession=\n"));

    assert_eq!(none, DESKTOP_ENTRY);
    assert_eq!(empty, DESKTOP_ENTRY);
  }

  #[test]
  fn game_mode_is_the_session_the_switch_wrote() {
    let Ok(named) = parse_session(Some("[Autologin]\nSession=gamescope-session-steam\n"));

    assert_eq!(named, "gamescope-session-steam.desktop");
  }
}
