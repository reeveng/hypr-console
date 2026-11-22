//! The volume rocker on the top edge.
//!
//! Turning it up also unsilences, after the level has been set. Turning it
//! down leaves the current mute state in place.
//!
//! The other half is where the number went when it left the bar. The bar wears
//! a glyph and no percentage: three speaker marks say quiet, middling and loud,
//! which is what an icon is for. What the number was actually for is this
//! rocker -- pressing it and seeing the figure move is how anyone knows it did
//! anything -- so the figure is said at the moment it changes, where someone
//! is already looking.

use std::ffi::OsStr;
use std::io;

use console_core_arguments::Subcommand;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_words::Words;
use console_sound_effects::SoundEffects;

use crate::level::Muted;

const NOTHING_HEARD: &str = "?";

pub const STEP: &str = "5%";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Words)]
pub enum ButtonPress {
  #[words(word = "up", about = "louder, and unsilenced")]
  Up,
  #[words(word = "down", about = "quieter")]
  Down,
  #[words(word = "mute", about = "silence it, or stop silencing it")]
  Mute,
}

impl Subcommand for ButtonPress {
  fn variants() -> Result<impl Iterator<Item = Self>, Never> {
    Ok(ButtonPress::VARIANTS.iter().copied())
  }

  fn spelling(self) -> Result<&'static str, Never> {
    self.word()
  }

  fn about(self) -> Result<&'static str, Never> {
    ButtonPress::about(self)
  }
}

pub fn asks(press: ButtonPress, sink: &str) -> Result<Vec<Vec<String>>, Never> {
  let words = |arguments: &[&str]| arguments.iter().map(|word| (*word).to_string()).collect();

  match press {
    ButtonPress::Up => Ok(vec![
      words(&["set-sink-volume", sink, &format!("+{STEP}")]),
      words(&["set-sink-mute", sink, "0"]),
    ]),
    ButtonPress::Down => Ok(vec![words(&["set-sink-volume", sink, &format!("-{STEP}")])]),
    ButtonPress::Mute => Ok(vec![words(&["set-sink-mute", sink, "toggle"])]),
  }
}

pub fn sound_server(arguments: &[impl AsRef<OsStr>]) -> io::Result<String> {
  let Ok(mut asking) = Program::Pactl.command();
  let said = asking.env("LC_ALL", "C").args(arguments).output()?;

  match said.status.success() {
    true => String::from_utf8(said.stdout).map_err(io::Error::other),
    false => Err(io::Error::other(
      String::from_utf8_lossy(&said.stderr).into_owned(),
    )),
  }
}

pub fn preview(sink: &str, muted: Muted, level: Option<i64>) -> Result<(), Never> {
  let Ok(effects) = SoundEffects::current();

  match (effects, muted, level) {
    (SoundEffects::On, Muted::No, Some(1..)) if !sink.is_empty() => {
      match console_sound_effects::play_volume(sink) {
        Ok(()) => {}
        Err(fault) => console_core_logging::warn!("could not play the volume preview: {fault}"),
      }
    }
    _silent => {}
  }

  Ok(())
}

pub fn volume_label(level: Option<&str>, muted: Muted) -> Result<String, Never> {
  match muted {
    Muted::Yes => Ok("Silent".to_string()),
    Muted::No => Ok(format!(
      "Volume {}",
      match level {
        Some(level_2) => level_2,
        None => NOTHING_HEARD,
      }
    )),
  }
}

pub fn level(said: &str) -> Result<Option<&str>, Never> {
  Ok(
    said
      .split_whitespace()
      .filter_map(|word| {
        let percent = word.trim_end_matches(',');
        let number = percent.strip_suffix('%')?.parse::<u32>().ok()?;

        Some((number, percent))
      })
      .max_by_key(|&(number, _)| number)
      .map(|(_, percent)| percent),
  )
}

pub fn muted(said: &str) -> Result<Muted, Never> {
  match ::core::primitive::bool::then_some(said.trim().ends_with("yes"), ()) {
    ::core::option::Option::Some(()) => Ok(Muted::Yes),
    ::core::option::Option::None => Ok(Muted::No),
  }
}

pub fn value(level: Option<&str>) -> Result<Option<i64>, Never> {
  let said = match level {
    Some(said) => said,
    None => return Ok(None),
  };

  let value = match said.trim_end_matches('%').parse::<i64>() {
    Ok(value) => value,
    Err(_not_a_number) => return Ok(None),
  };

  Ok(Some(value))
}

#[cfg(test)]
mod tests {
  use super::*;

  type Failure = Box<dyn std::error::Error>;
  const SINK: &str = "output";

  #[test]
  fn turning_it_up_sets_the_level_before_unmuting() -> Result<(), Failure> {
    let Ok(asked) = asks(ButtonPress::Up, SINK);
    let raising = asked.first().ok_or("nothing asked")?;
    let unmuting = asked.get(1).ok_or("one thing asked")?;

    assert_eq!(raising.first().map(String::as_str), Some("set-sink-volume"));
    assert_eq!(raising.get(1).map(String::as_str), Some(SINK));
    assert_eq!(unmuting.first().map(String::as_str), Some("set-sink-mute"));
    assert_eq!(unmuting.get(1).map(String::as_str), Some(SINK));
    assert_eq!(unmuting.get(2).map(String::as_str), Some("0"));

    Ok(())
  }

  #[test]
  fn turning_it_down_leaves_it_silenced() {
    let Ok(asked) = asks(ButtonPress::Down, SINK);

    assert_eq!(asked.len(), 1);
    assert!(!asked.iter().any(|arguments| {
      arguments
        .first()
        .is_some_and(|verb| verb == "set-sink-mute")
    }));
  }

  #[test]
  fn mute_is_a_toggle_and_moves_no_number() {
    assert_eq!(
      asks(ButtonPress::Mute, SINK),
      Ok(vec![vec![
        "set-sink-mute".to_string(),
        SINK.to_string(),
        "toggle".to_string()
      ]])
    );
  }

  #[test]
  fn nothing_but_the_three_words_is_a_press() {
    assert_eq!(ButtonPress::from_word("up"), Ok(Some(ButtonPress::Up)));
    assert_eq!(ButtonPress::from_word("UP"), Ok(None));
    assert_eq!(ButtonPress::from_word(""), Ok(None));
  }

  #[test]
  fn the_percentage_is_read_out_of_what_pactl_says() {
    let said = "Volume: front-left: 32768 /  50% / -18.06 dB,   front-right: 32768 /  50%\n";
    let Ok(found) = level(said);

    assert_eq!(found, Some("50%"));
    assert_eq!(value(found), Ok(Some(50)));
  }

  #[test]
  fn the_preview_accounts_for_the_loudest_channel() {
    let said = "Volume: front-left: 0 / 0% / -inf dB, front-right: 32768 / 50% / -18.06 dB";
    assert_eq!(level(said), Ok(Some("50%")));
  }

  #[test]
  fn nothing_pactl_says_is_ever_a_reason_to_fail() {
    assert_eq!(level(""), Ok(None));
    assert_eq!(value(None), Ok(None));
    assert_eq!(volume_label(None, Muted::No), Ok("Volume ?".to_string()));
  }

  #[test]
  fn silence_is_said_rather_than_shown_as_a_number() {
    assert_eq!(
      volume_label(Some("50%"), Muted::Yes),
      Ok("Silent".to_string())
    );
    assert_eq!(
      volume_label(Some("50%"), Muted::No),
      Ok("Volume 50%".to_string())
    );
  }

  #[test]
  fn what_mute_says_is_read_off_the_end_of_the_line() {
    assert_eq!(muted("Mute: yes"), Ok(Muted::Yes));
    assert_eq!(muted("Mute: no"), Ok(Muted::No));
    assert_eq!(muted(""), Ok(Muted::No));
  }
}
