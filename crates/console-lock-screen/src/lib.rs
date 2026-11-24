//! What the lock screen decides: whether there is anything to lock with, and
//! what a drawn pattern is answered with.
//!
//! **The greeter, answered here instead of by the login window.** The dots, the
//! cursor, a finger drawn across them and what the screen says after a refusal
//! are all `console_login_greeter::greeting`, so the lock is the same ring in
//! the same place a person drew at login. What differs is who is asked: the
//! greeter hands its letters to a window running as root, because before a
//! login nothing else may read the hash; after one the hash is the person's
//! own file, and the session that is locked is theirs.
//!
//! **No pattern is no lock.** A person who never chose one is never asked for
//! one, at login or here -- a lock nobody can answer with a pad is the device
//! that cannot be got into, which is the reason this desktop had no lock at
//! all until the pattern existed.
//!
//! **A pattern can be kept for login alone.** [`LockScreen`] is the choice in
//! the settings, and nothing chosen is `On`, because a person who chose a
//! pattern chose it to keep somebody out. `Off` keeps the pattern at the way
//! in and lets the desktop wake as it did before there was a lock. The word is
//! spelled here and read by both ends -- the lock asks it before it locks, the
//! settings before they draw the row -- so the two cannot disagree about
//! what `off` means.
//!
//! **A lock does not open a terminal.** The greeter can ask the window for
//! one, because the way in is where a broken desktop is mended. Over a locked
//! session that would be the way round the lock.

use console_core_never::Never;
use console_core_words::Words;
use console_login_window::protocol::{FromGreeter, ToGreeter};
use console_login_window::stored_pattern::{Hash, PatternAttempts, StoredPattern};

pub mod idle;

pub const PATTERN_UNAVAILABLE: &str =
  "The saved pattern cannot be read. Choose another unlock method.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wanted {
  Lock(Hash),
  Absent,
}

pub const SETTING: &str = "lock-screen";
const METHOD_SETTING: &str = "lock-screen-method";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Words)]
pub enum Method {
  #[default]
  #[words(key = "pattern", label = "Pattern")]
  Pattern,
  #[words(key = "fingerprint", label = "Fingerprint")]
  Fingerprint,
  #[words(key = "password", label = "Password")]
  Password,
}

impl Method {
  pub const ALL: [Self; 3] = [Self::Pattern, Self::Fingerprint, Self::Password];
}

pub fn preferred() -> Result<Method, Never> {
  let Ok(value) = console_defaults::setting(METHOD_SETTING);
  let Ok(chosen) = value.as_deref().map_or(Ok(None), Method::from_key);
  Ok(match chosen {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => crate::Method::Pattern,
  })
}

pub fn prefer(method: Method) -> Result<(), Never> {
  let Ok(value) = method.key();
  console_defaults::set(console_defaults::Setting {
    key: METHOD_SETTING,
    value,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum LockScreen {
  #[words(key = "on")]
  On,
  #[words(key = "off")]
  Off,
}

impl LockScreen {
  pub fn flipped(self) -> Result<LockScreen, Never> {
    Ok(match self {
      LockScreen::On => LockScreen::Off,
      LockScreen::Off => LockScreen::On,
    })
  }
}

pub fn read(setting: Option<&str>) -> Result<LockScreen, Never> {
  read_switch(SETTING, setting)
}

fn read_switch(key: &str, setting: Option<&str>) -> Result<LockScreen, Never> {
  let said = match setting {
    Some(text) => text,
    None => return Ok(LockScreen::On),
  };
  let Ok(found) = LockScreen::from_key(said);

  match found {
    Some(chosen) => Ok(chosen),
    None => {
      console_core_logging::warn!("{key}={said} is invalid; using on");

      Ok(LockScreen::On)
    }
  }
}

pub fn current() -> Result<LockScreen, Never> {
  let told = console_defaults::setting(SETTING)?;

  read(told.as_deref())
}

pub fn choose(chosen: LockScreen) -> Result<(), Never> {
  set_switch(SETTING, chosen)
}

fn set_switch(key: &str, chosen: LockScreen) -> Result<(), Never> {
  let Ok(value) = chosen.key();

  console_defaults::set(console_defaults::Setting { key, value })
}

pub fn desired(stored: StoredPattern, chosen: LockScreen) -> Result<Wanted, Never> {
  Ok(match (stored, chosen) {
    (StoredPattern::Hash(hash), LockScreen::On) => Wanted::Lock(hash),
    (StoredPattern::Hash(_), LockScreen::Off)
    | (StoredPattern::Absent, LockScreen::On | LockScreen::Off) => Wanted::Absent,
  })
}

pub fn respond(
  said: &FromGreeter,
  hash: Option<&Hash>,
  attempts: &mut PatternAttempts,
  now: std::time::Instant,
) -> Result<Option<ToGreeter>, Never> {
  Ok(match said {
    FromGreeter::Login(letters) => {
      let Ok(reply) = match hash {
        Some(hash_2) => attempts.check(letters, hash_2, now),
        None => Ok(ToGreeter::Message(PATTERN_UNAVAILABLE.to_owned())),
      };
      Some(reply)
    }
    FromGreeter::Fingerprint | FromGreeter::Terminal => None,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_login_window::stored_pattern::{self, NOT_THE_PATTERN};

  #[test]
  fn a_person_who_never_chose_a_pattern_is_never_locked() {
    let Ok(wanted) = desired(StoredPattern::Absent, LockScreen::On);

    assert_eq!(wanted, Wanted::Absent);
  }

  #[test]
  fn a_chosen_pattern_is_what_the_lock_is_answered_with() -> Result<(), Box<dyn std::error::Error>>
  {
    let chosen = stored_pattern::hashed("aec")?;
    let Ok(wanted) = desired(StoredPattern::Hash(chosen.clone()), LockScreen::On);

    assert_eq!(wanted, Wanted::Lock(chosen));

    Ok(())
  }

  #[test]
  fn a_pattern_kept_for_login_alone_does_not_lock() -> Result<(), Box<dyn std::error::Error>> {
    let aec = stored_pattern::hashed("aec")?;
    let Ok(wanted) = desired(StoredPattern::Hash(aec), LockScreen::Off);

    assert_eq!(wanted, Wanted::Absent);

    Ok(())
  }

  #[test]
  fn nothing_chosen_locks_as_a_pattern_was_chosen_to() {
    assert_eq!(read(None), Ok(LockScreen::On));
  }

  #[test]
  fn off_is_off_and_a_word_that_is_neither_still_locks() {
    assert_eq!(read(Some("off")), Ok(LockScreen::Off));
    assert_eq!(read(Some("on")), Ok(LockScreen::On));
    assert_eq!(read(Some("no")), Ok(LockScreen::On));
  }

  #[test]
  fn the_pattern_that_was_chosen_unlocks() -> Result<(), Box<dyn std::error::Error>> {
    let aec = stored_pattern::hashed("aec")?;
    let Ok(answer) = respond(
      &FromGreeter::Login("aec".to_string()),
      Some(&aec),
      &mut PatternAttempts::default(),
      std::time::Instant::now(),
    );

    assert_eq!(answer, Some(ToGreeter::Welcome));

    Ok(())
  }

  #[test]
  fn any_other_pattern_is_refused_in_the_login_window_s_words()
  -> Result<(), Box<dyn std::error::Error>> {
    let aec = stored_pattern::hashed("aec")?;
    let Ok(answer) = respond(
      &FromGreeter::Login("ace".to_string()),
      Some(&aec),
      &mut PatternAttempts::default(),
      std::time::Instant::now(),
    );

    assert_eq!(answer, Some(ToGreeter::Failed(NOT_THE_PATTERN.to_string())));

    Ok(())
  }

  #[test]
  fn an_unreadable_pattern_cannot_unlock_the_session() {
    let Ok(answer) = respond(
      &FromGreeter::Login("abcd".to_owned()),
      None,
      &mut PatternAttempts::default(),
      std::time::Instant::now(),
    );

    assert_eq!(
      answer,
      Some(ToGreeter::Message(PATTERN_UNAVAILABLE.to_owned()))
    );
  }

  #[test]
  fn asking_for_a_terminal_over_a_locked_session_is_not_answered()
  -> Result<(), Box<dyn std::error::Error>> {
    let aec = stored_pattern::hashed("aec")?;
    let Ok(answer) = respond(
      &FromGreeter::Terminal,
      Some(&aec),
      &mut PatternAttempts::default(),
      std::time::Instant::now(),
    );

    assert_eq!(answer, None);

    Ok(())
  }
}
