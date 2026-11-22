//! What the machine said last time it was asked.
//!
//! `Page::meanwhile` is the tab as it can be drawn before the machine has
//! answered anything, and it works because most of a tab is known in advance:
//! the three power profiles are the same three whatever powerprofilesctl says,
//! and all the answer decides is which of them is marked.
//!
//! Some tabs are not like that. Sound is whatever is plugged in and whatever is
//! playing, Wi-Fi is whatever is in the air, Bluetooth is whatever has ever
//! been paired. There is nothing to draw in advance, so those tabs went up
//! empty and filled in, which is the whole card changing height a moment after
//! it appeared -- and it appeared under a thumb already moving down it.
//!
//! They are not unknowable, though. They were known last time, and last time is
//! a far better guess than nothing: the speakers are the speakers, and the
//! networks in this room are the networks that were in this room. So what a
//! command says is written down as it answers, and the tab's `meanwhile` builds
//! its rows out of what was written down.
//!
//! The same builder fed an older answer, never a second opinion about what the
//! tab looks like. A hand-written `meanwhile` is a second list someone has to
//! remember to change when the first one changes; this one cannot drift,
//! because there is only one list and the two readings go into it.
//!
//! Under the cache and not beside the [`notes`](crate::notes), deliberately.
//! A note is something the desktop remembers about itself and could not work
//! out again -- which tab it was left on, how much room it was granted. This is
//! the machine's own answer to a question anyone can ask again, so it belongs
//! where a thing that can be rebuilt belongs. Someone who clears the cache
//! gets a panel that opens the way it did before there was one, which is the
//! only thing any of this is allowed to cost.
//!
//! An answer that never came from a program is kept here too, as the bytes it
//! arrived in. Bluetooth asks bluez over the bus, and what it keeps is bluez's
//! reply as it came off the wire, which the tab reads back with the reading it
//! gave the live one -- the rule above, held for an answer that was never text.

use std::path::PathBuf;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_places::Base;

use crate::running;

fn beside(note: &str) -> Result<Option<PathBuf>, Never> {
  let ours = Base::Cache.ours()?;

  let Ok(whose) = crate::whose::argv0();
  let Ok(filed) = filed(note);

  let name = format!("{whose}.{filed}");
  let Ok(file) = crate::notes::NoteFile::new(&name);

  Ok(
    ours
      .zip(file)
      .map(|(base, leaf)| base.join("asked").join(leaf)),
  )
}

fn filed(note: &str) -> Result<String, Never> {
  let filed: String = note
    .chars()
    .map(|letter| match ::core::primitive::bool::then_some(letter.is_ascii_alphanumeric(), ()) {
      ::core::option::Option::Some(()) => letter.to_ascii_lowercase(),
      ::core::option::Option::None => '-',
    })
    .collect();

  Ok(match ::core::primitive::bool::then_some(filed.is_empty(), ()) {
    ::core::option::Option::Some(()) => "asked".to_string(),
    ::core::option::Option::None => filed,
  })
}

pub fn kept(note: &str) -> Result<Vec<u8>, Never> {
  let Ok(beside) = beside(note);

  let path = match beside {
    Some(path) => path,
    None => return Ok(Vec::new()),
  };

  let said = match std::fs::read(path) {
    Ok(said) => said,
    Err(_unreadable) => return Ok(Vec::new()),
  };

  Ok(said)
}

pub fn last(note: &str) -> Result<String, Never> {
  let Ok(kept) = kept(note);

  Ok(match String::from_utf8(kept) {
    Ok(said) => said,
    Err(_not_text) => String::new(),
  })
}

pub fn run_and_note(note: &str, program: Program, rest: &[&str]) -> Result<String, Never> {
  let Ok(said) = running::run_output(program, rest);
  let Ok(()) = keep(note, said.as_bytes());

  Ok(said)
}

pub fn keep(note: &str, said: &[u8]) -> Result<(), Never> {
  let Ok(kept) = kept(note);

  match ::core::primitive::bool::then_some(kept == said, ()) {
    ::core::option::Option::Some(()) => return Ok(()),
    ::core::option::Option::None => {}
  }

  let Ok(beside) = beside(note);

  let path = match beside {
    Some(path) => path,
    None => return Ok(()),
  };

  let holding = match path.parent() {
    Some(holding) => holding,
    None => return Ok(()),
  };

  match std::fs::create_dir_all(holding) {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::warn!(
        "console: {}: keeping what a tab last said: {fault}",
        holding.display()
      );

      return Ok(());
    }
  }

  match console_core_atomic_writes::whole(&path, said) {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!(
      "console: {}: keeping what a tab last said: {fault}",
      path.display()
    ),
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_question_is_filed_under_a_name_any_filesystem_would_take() {
    assert_eq!(filed("sinks"), Ok("sinks".to_string()));
    assert_eq!(
      filed("bluetooth AA:BB:CC:DD:EE:FF"),
      Ok("bluetooth-aa-bb-cc-dd-ee-ff".to_string())
    );
    assert_eq!(
      filed("opens audio/x-opus+ogg"),
      Ok("opens-audio-x-opus-ogg".to_string())
    );
  }

  #[test]
  fn a_question_with_no_name_is_still_a_file() {
    assert_eq!(filed(""), Ok("asked".to_string()));
    assert_eq!(filed("///"), Ok("---".to_string()));
  }

  #[test]
  fn a_question_no_one_has_asked_says_nothing() {
    assert_eq!(
      last("a question nothing on this machine has ever asked"),
      Ok(String::new())
    );
  }
}
