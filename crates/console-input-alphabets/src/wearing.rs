//! Which alphabet each keyboard is on, and where that is kept between boots.
//!
//! Per keyboard rather than per machine. Someone who switches the board on
//! their desk to Greek has said something about that board and nothing about
//! the one drawn on the screen of a handheld in another room, and a machine
//! that moved both would be deciding those are the same hand. So the store is
//! a name to an alphabet, and the name is the compositor's own for a keyboard
//! someone plugged in and [`SCREEN`] for the one this desktop draws.
//!
//! ## What a keyboard no one has met is wearing
//!
//! The chain, in order, and each step is a real answer rather than a fallback
//! to be embarrassed about:
//!
//! 1. **What it was last switched to**, if it has ever been switched. From its
//!    first switch a keyboard keeps its own, which is the whole point of
//!    keying this by name.
//! 2. **The machine's chosen alphabets**, first one. A board plugged in for
//!    the first time should type what this machine types, not what xkb's
//!    defaults think a keyboard is -- someone who set this desktop to Greek
//!    and Latin did not do it once per device.
//! 3. **Latin**, which is `us`. Every word this desktop says is English and
//!    every address is latin, so this is the answer that cannot leave someone
//!    unable to type their way back out.
//!
//! When someone chooses a layout in Settings, the screen and all remembered
//! keyboards use it. The compositor also uses that choice for new keyboards.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use console_core_never::Never;

use crate::Alphabet;

pub const NAMED: &str = "alphabets";

pub const SCREEN: &str = "screen";

const PHYSICAL_LAYOUT: &str = "physical-layout";

pub fn path_in(home: &Path) -> Result<PathBuf, Never> {
  let Ok(ours) = console_core_places::Base::State.application_under(home);

  Ok(ours.join(NAMED))
}

pub fn of(text: &str) -> Result<BTreeMap<String, String>, Never> {
  let mut every = BTreeMap::new();

  for line in text.lines() {
    let line_2 = line.trim();

    let (whose, key) = match line_2.split_once('=') {
      Some((whose, key)) => (whose.trim(), key.trim()),
      None => continue,
    };

    match ::core::primitive::bool::then_some(whose.is_empty() || key.is_empty(), ()) {
      ::core::option::Option::Some(()) => continue,
      ::core::option::Option::None => {}
    }

    let _ = every.insert(whose.to_string(), key.to_string());
  }

  Ok(every)
}

pub fn serialize(every: &BTreeMap<String, String>) -> Result<String, Never> {
  let mut text = String::new();

  for (whose, key) in every {
    text.push_str(whose);
    text.push_str(" = ");
    text.push_str(key);
    text.push('\n');
  }

  Ok(text)
}

pub fn every(home: &Path) -> Result<BTreeMap<String, String>, Never> {
  let Ok(at) = path_in(home);

  let Ok(held) = console_core_atomic_writes::read(&at);
  let Ok(text) = held.text();

  match text {
    Some(text_2) => of(&text_2),
    None => Ok(BTreeMap::new()),
  }
}

pub fn among(
  every: &BTreeMap<String, String>,
  whose: &str,
  walk: &[&'static Alphabet],
) -> Result<&'static Alphabet, Never> {
  let told = every.get(whose).and_then(|key| {
    let Ok(one) = crate::one(key);

    one.filter(|alphabet| walk.iter().any(|kept| kept.key == alphabet.key))
  });

  match told {
    Some(alphabet) => return Ok(alphabet),
    None => {}
  }

  let selected = every
    .get(PHYSICAL_LAYOUT)
    .and_then(|xkb| walk.iter().find(|alphabet| alphabet.xkb == xkb));

  match selected.or_else(|| walk.first()) {
    Some(alphabet) => Ok(alphabet),
    None => crate::latin(),
  }
}

pub fn read(home: &Path, whose: &str) -> Result<&'static Alphabet, Never> {
  let Ok(every) = every(home);
  let Ok(walk) = crate::current();

  among(&every, whose, &walk)
}

#[derive(Debug)]
pub enum Unremembered {
  Rootless,
  Holding(PathBuf, std::io::Error),
  Writing(console_core_atomic_writes::Unwritten),
}

impl fmt::Display for Unremembered {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Unremembered::Rootless => write!(to, "what the keyboards wear has no directory"),
      Unremembered::Holding(under, fault) => write!(to, "{}: {fault}", under.display()),
      Unremembered::Writing(fault) => write!(to, "{fault}"),
    }
  }
}

impl std::error::Error for Unremembered {}

impl From<console_core_atomic_writes::Unwritten> for Unremembered {
  fn from(fault: console_core_atomic_writes::Unwritten) -> Self {
    Unremembered::Writing(fault)
  }
}

pub fn remember(home: &Path, whose: &str, alphabet: &Alphabet) -> Result<(), Unremembered> {
  change(home, |every| {
    let _ = every.insert(whose.to_string(), alphabet.key.to_string());
  })
}

#[derive(Debug)]
pub enum Unselected {
  Store(Unremembered),
  Apply(String),
  Rollback {
    apply: String,
    restore: Unremembered,
  },
}

impl fmt::Display for Unselected {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Store(fault) => write!(out, "{fault}"),
      Self::Apply(message) => out.write_str(message),
      Self::Rollback { apply, restore } => {
        write!(
          out,
          "{apply}. Could not restore the previous layout: {restore}"
        )
      }
    }
  }
}

impl std::error::Error for Unselected {}

pub fn select(
  home: &Path,
  alphabet: &Alphabet,
  apply: impl FnOnce(&[&str]) -> Result<(), String>,
) -> Result<(), Unselected> {
  let (at, _directory) = lock(home).map_err(Unselected::Store)?;
  let Ok(previous) = every(home);
  let mut selected = previous.clone();
  let devices: Vec<&str> = previous
    .keys()
    .map(String::as_str)
    .filter(|name| *name != SCREEN && *name != PHYSICAL_LAYOUT)
    .filter(|name| !name.starts_with("hl-virtual-keyboard"))
    .collect();

  for chosen in selected.values_mut() {
    *chosen = alphabet.key.to_string();
  }

  let _ = selected.insert(SCREEN.to_string(), alphabet.key.to_string());
  let _ = selected.insert(PHYSICAL_LAYOUT.to_string(), alphabet.xkb.to_string());

  save(&at, &selected).map_err(Unselected::Store)?;

  match apply(&devices) {
    Ok(()) => Ok(()),
    Err(apply_fault) => match save(&at, &previous) {
      Ok(()) => Err(Unselected::Apply(apply_fault)),
      Err(restore) => Err(Unselected::Rollback {
        apply: apply_fault,
        restore,
      }),
    },
  }
}

fn change(
  home: &Path,
  update: impl FnOnce(&mut BTreeMap<String, String>),
) -> Result<(), Unremembered> {
  let (at, _directory) = lock(home)?;
  let Ok(mut every) = every(home);
  update(&mut every);

  save(&at, &every)
}

fn lock(home: &Path) -> Result<(PathBuf, std::fs::File), Unremembered> {
  let Ok(at) = path_in(home);

  let under = match at.parent() {
    Some(under) => under,
    None => return Err(Unremembered::Rootless),
  };

  std::fs::create_dir_all(under)
    .map_err(|fault| Unremembered::Holding(under.to_path_buf(), fault))?;

  let directory = std::fs::File::open(under)
    .map_err(|fault| Unremembered::Holding(under.to_path_buf(), fault))?;
  directory
    .lock()
    .map_err(|fault| Unremembered::Holding(under.to_path_buf(), fault))?;

  Ok((at, directory))
}

fn save(at: &Path, every: &BTreeMap<String, String>) -> Result<(), Unremembered> {
  let Ok(text) = serialize(every);

  console_core_atomic_writes::whole(at, text.as_bytes()).map_err(Unremembered::Writing)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;

  use crate::LATIN;

  #[test]
  fn a_keyboard_no_one_has_met_wears_what_this_machine_types() {
    let every = BTreeMap::new();
    let Ok(walk) = crate::read("latin,greek");
    let Ok(worn) = among(&every, "Logitech K380", &walk);

    assert_eq!(worn.key, LATIN);
  }

  #[test]
  fn from_its_first_switch_a_keyboard_keeps_its_own() {
    let Ok(every) = of("Logitech K380 = greek\n");
    let Ok(walk) = crate::read("latin,greek,thai");
    let Ok(worn) = among(&every, "Logitech K380", &walk);

    assert_eq!(worn.key, "greek");

    let Ok(other_board) = among(&every, "Some Other Board", &walk);

    assert_eq!(
      other_board.key, LATIN,
      "the board beside it was not switched to anything"
    );
  }

  #[test]
  fn the_screen_is_a_keyboard_with_a_name_like_any_other() {
    let Ok(every) = of("screen = thai\nLogitech K380 = greek\n");
    let Ok(walk) = crate::read("latin,greek,thai");
    let Ok(worn) = among(&every, SCREEN, &walk);

    assert_eq!(worn.key, "thai");
  }

  #[test]
  fn an_alphabet_this_machine_no_longer_types_is_not_worn_by_anyone() {
    let Ok(every) = of("Logitech K380 = greek\n");
    let Ok(walk) = crate::read("latin,thai");
    let Ok(worn) = among(&every, "Logitech K380", &walk);

    assert_eq!(
      worn.key, LATIN,
      "someone took Greek off this machine, so the board cannot be left on it"
    );
  }

  #[test]
  fn a_word_no_one_here_wrote_is_not_an_alphabet() {
    let Ok(every) = of("Logitech K380 = klingon\n");
    let Ok(walk) = crate::read("latin,greek");
    let Ok(worn) = among(&every, "Logitech K380", &walk);

    assert_eq!(worn.key, LATIN);
  }

  #[test]
  fn what_is_written_reads_back_the_same() {
    let Ok(every) = of("b = greek\na = thai\n");
    let Ok(text) = serialize(&every);
    let Ok(again) = of(&text);

    assert_eq!(every, again);
    assert_eq!(
      text, "a = thai\nb = greek\n",
      "one line per keyboard, in a settled order"
    );
  }

  #[test]
  fn a_name_with_room_in_it_is_a_name() {
    let Ok(every) = of("Keychron K3 Pro = greek\n");

    assert_eq!(
      every.get("Keychron K3 Pro").map(String::as_str),
      Some("greek")
    );
  }

  #[test]
  fn what_is_remembered_is_what_comes_back() -> Result<(), Box<dyn Error>> {
    let home = console_core_temporary_directories::fresh(
      "alphabets-what_is_remembered_is_what_comes_back",
    )?;
    let Ok(greek) = crate::one("greek");
    let alphabet = greek.ok_or("greek is not an alphabet")?;

    remember(&home, "Logitech K380", alphabet)?;

    let Ok(every) = every(&home);
    let Ok(walk) = crate::read("latin,greek");
    let Ok(worn) = among(&every, "Logitech K380", &walk);

    assert_eq!(worn.key, "greek");

    Ok(())
  }

  #[test]
  fn selecting_a_layout_updates_the_default_for_new_keyboards_and_can_be_retried()
  -> Result<(), Box<dyn Error>> {
    let home = console_core_temporary_directories::fresh("selected-layout")?;
    let Ok(walk) = crate::read("greek,thai");
    let Ok(thai) = crate::one("thai");
    let thai_2 = thai.ok_or("Thai is missing")?;
    remember(&home, "desk-keyboard", thai_2)?;
    let Ok(before) = every(&home);
    let Ok(found) = crate::one("greek");
    let greek = found.ok_or("Greek is missing")?;

    assert!(matches!(
      select(&home, greek, |_| Err(String::from(
        "compositor unavailable"
      ))),
      Err(Unselected::Apply(_message))
    ));
    assert_eq!(every(&home), Ok(before));
    select(&home, greek, |devices| {
      assert_eq!(devices, ["desk-keyboard"]);
      Ok(())
    })?;

    let Ok(current) = every(&home);

    for device in [SCREEN, "desk-keyboard", "new-keyboard"] {
      let Ok(active) = among(&current, device, &walk);
      assert_eq!(active.key, "greek");
    }

    assert_eq!(current.get(PHYSICAL_LAYOUT).map(String::as_str), Some("gr"));

    Ok(())
  }
}
