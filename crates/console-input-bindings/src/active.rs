//! Which input somebody is using, and the one line that remembers it.
//!
//! The last press wins. Not a setting, not a device that is plugged in, and
//! not a mode anybody puts the machine into: whichever of the two hands last
//! did something is the one the guide reads out and the one the setup screen
//! opens on. A keyboard on the desk beside a handheld nobody has touched in an
//! hour is still the keyboard, because pressing a key is the whole of what
//! saying so consists of.
//!
//! Only meaningful activity counts. A pad that is merely being held sends
//! events without anybody meaning anything by them -- a stick drifting on its
//! centre, a trigger resting a few units off zero -- and a machine that
//! switched screens because of that would be reading the table rather than the
//! person. So it is a press, on a button or a key, and nothing else moves it.
//!
//! ## One word in a file, and why it is not carried live
//!
//! `~/.local/state/console/input`. The daemon writes it whenever the answer
//! changes, and once at startup with whatever it read back -- which is not the
//! same as writing nothing. A daemon that has never seen the hand change is
//! indistinguishable, from outside, from a daemon that is not recording the
//! hand at all, and one of those two is a fault. The two screens that care
//! read it at the moment they open.
//!
//! It could have gone through `console-events`, which is how everything else
//! on this machine hears about something changing, and it deliberately does
//! not. Nobody wants a live one. The guide and the setup screen are read while
//! they are up, and a page that moved under somebody's eyes because they
//! reached for the other hand is worse than a page that stayed where it was
//! put. What they want is where to open, which is a question asked once, and
//! the file answers it without a daemon having to be up.
//!
//! State rather than config, because nobody chose it. Nothing reads it as a
//! preference and nothing offers to change it: a machine that lost the file is
//! a machine that starts on the pad, which is what the device is. It is the
//! answer only until the first press of a session -- somebody who left the
//! desktop typing comes back to a guide about keys, and picking the machine up
//! settles it the other way before they have read a line of it.
//!
//! The word is [`Input::word`], so the file and `buttons.toml` spell it the
//! same way.

use std::path::{Path, PathBuf};

use console_core_never::Never;

use crate::bound::Input;

pub const NAMED: &str = "input";

pub const FIRST: Input = Input::Pad;

pub fn path_in(home: &Path) -> Result<PathBuf, Never> {
    let Ok(ours) = console_core_places::Base::State.ours_under(home);

    Ok(ours.join(NAMED))
}

pub fn of(said: &str) -> Result<Input, Never> {
    let word = said.trim();

    Ok(crate::bound::EVERY
        .into_iter()
        .find(|input| {
            let Ok(spelt) = input.word();

            spelt == word
        })
        .unwrap_or(FIRST))
}

pub fn read(home: &Path) -> Result<Input, Never> {
    let Ok(at) = path_in(home);
    let Ok(held) = console_core_atomic_writes::read(&at);
    let Ok(said) = held.said();

    match said {
        Some(said) => of(&said),
        None => Ok(FIRST),
    }
}

pub fn remember(home: &Path, on: Input) -> Result<(), String> {
    let Ok(at) = path_in(home);

    let under = at.parent().ok_or("the remembered input has no directory")?;

    std::fs::create_dir_all(under).map_err(|fault| format!("{}: {fault}", under.display()))?;

    let Ok(word) = on.word();

    console_core_atomic_writes::whole(&at, word.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    #[test]
    fn a_word_that_names_an_input_is_that_input() {
        assert_eq!(ok(of("pad")), Input::Pad);
        assert_eq!(ok(of("keyboard\n")), Input::Keyboard);
    }

    #[test]
    fn a_word_that_names_nothing_is_the_machine_itself() {
        assert_eq!(ok(of("mouse")), Input::Pad);
        assert_eq!(ok(of("")), Input::Pad);
    }

    #[test]
    fn a_machine_nobody_has_typed_at_starts_on_the_pad() {
        let home = tempfile::tempdir().expect("somewhere");

        assert_eq!(ok(read(home.path())), Input::Pad);
    }

    #[test]
    fn what_was_last_pressed_is_what_it_comes_back_to() {
        let home = tempfile::tempdir().expect("somewhere");

        remember(home.path(), Input::Keyboard).expect("a word written");

        assert_eq!(ok(read(home.path())), Input::Keyboard);

        remember(home.path(), Input::Pad).expect("a word written");

        assert_eq!(ok(read(home.path())), Input::Pad);
    }

    #[test]
    fn the_word_on_the_file_is_the_word_everything_else_spells() {
        let home = tempfile::tempdir().expect("somewhere");

        remember(home.path(), Input::Keyboard).expect("a word written");

        let Ok(at) = path_in(home.path());
        let said = std::fs::read_to_string(&at).expect("the file");

        assert_eq!(said, "keyboard");
    }
}
