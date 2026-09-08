//! Which alphabet each keyboard is on, and where that is kept between boots.
//!
//! Per keyboard rather than per machine. Somebody who switches the board on
//! their desk to Greek has said something about that board and nothing about
//! the one drawn on the screen of a handheld in another room, and a machine
//! that moved both would be deciding those are the same hand. So the store is
//! a name to an alphabet, and the name is the compositor's own for a keyboard
//! somebody plugged in and [`SCREEN`] for the one this desktop draws.
//!
//! ## What a keyboard nobody has met is wearing
//!
//! The chain, in order, and each step is a real answer rather than a fallback
//! to be embarrassed about:
//!
//! 1. **What it was last switched to**, if it has ever been switched. From its
//!    first switch a keyboard keeps its own, which is the whole point of
//!    keying this by name.
//! 2. **The machine's chosen alphabets**, first one. A board plugged in for
//!    the first time should type what this machine types, not what xkb's
//!    defaults think a keyboard is -- somebody who set this desktop to Greek
//!    and Latin did not do it once per device.
//! 3. **Latin**, which is `us`. Every word this desktop says is English and
//!    every address is latin, so this is the answer that cannot leave somebody
//!    unable to type their way back out.
//!
//! It is state and not a setting. Nothing offers to change it and nothing
//! reads it as a preference: a machine that lost the file is a machine where
//! every keyboard starts on step two, which is where they all started the
//! first time anyway.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_core_never::Never;

use crate::Alphabet;

pub const NAMED: &str = "alphabets";

pub const SCREEN: &str = "screen";

pub fn path_in(home: &Path) -> Result<PathBuf, Never> {
    let Ok(ours) = console_core_places::Base::State.ours_under(home);

    Ok(ours.join(NAMED))
}

pub fn of(said: &str) -> Result<BTreeMap<String, String>, Never> {
    let mut every = BTreeMap::new();

    for line in said.lines() {
        let line = line.trim();

        let (whose, key) = match line.split_once('=') {
            Some((whose, key)) => (whose.trim(), key.trim()),
            None => continue,
        };

        match whose.is_empty() || key.is_empty() {
            true => continue,
            false => {},
        }

        let _ = every.insert(whose.to_string(), key.to_string());
    }

    Ok(every)
}

pub fn written(every: &BTreeMap<String, String>) -> Result<String, Never> {
    let mut said = String::new();

    for (whose, key) in every {
        said.push_str(whose);
        said.push_str(" = ");
        said.push_str(key);
        said.push('\n');
    }

    Ok(said)
}

pub fn every(home: &Path) -> Result<BTreeMap<String, String>, Never> {
    let Ok(at) = path_in(home);
    let Ok(held) = console_core_atomic_writes::read(&at);
    let Ok(said) = held.said();

    match said {
        Some(said) => of(&said),
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
        None => {},
    }

    match walk.first() {
        Some(alphabet) => Ok(alphabet),
        None => crate::latin(),
    }
}

pub fn read(home: &Path, whose: &str) -> Result<&'static Alphabet, Never> {
    let Ok(every) = every(home);
    let Ok(walk) = crate::chosen();

    among(&every, whose, &walk)
}

pub fn remember(home: &Path, whose: &str, alphabet: &Alphabet) -> Result<(), String> {
    let Ok(mut every) = every(home);
    let _ = every.insert(whose.to_string(), alphabet.key.to_string());

    let Ok(said) = written(&every);
    let Ok(at) = path_in(home);

    let under = at.parent().ok_or("what the keyboards wear has no directory")?;

    std::fs::create_dir_all(under).map_err(|fault| format!("{}: {fault}", under.display()))?;

    console_core_atomic_writes::whole(&at, said.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::LATIN;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    fn walk(said: &str) -> Vec<&'static Alphabet> {
        ok(crate::read(said))
    }

    #[test]
    fn a_keyboard_nobody_has_met_wears_what_this_machine_types() {
        let every = BTreeMap::new();
        let walk = walk("latin,greek");

        assert_eq!(ok(among(&every, "Logitech K380", &walk)).key, LATIN);
    }

    #[test]
    fn from_its_first_switch_a_keyboard_keeps_its_own() {
        let Ok(every) = of("Logitech K380 = greek\n");
        let walk = walk("latin,greek,thai");

        assert_eq!(ok(among(&every, "Logitech K380", &walk)).key, "greek");
        assert_eq!(
            ok(among(&every, "Some Other Board", &walk)).key,
            LATIN,
            "the board beside it was not switched to anything"
        );
    }

    #[test]
    fn the_screen_is_a_keyboard_with_a_name_like_any_other() {
        let Ok(every) = of("screen = thai\nLogitech K380 = greek\n");
        let walk = walk("latin,greek,thai");

        assert_eq!(ok(among(&every, SCREEN, &walk)).key, "thai");
    }

    #[test]
    fn an_alphabet_this_machine_no_longer_types_is_not_worn_by_anybody() {
        let Ok(every) = of("Logitech K380 = greek\n");
        let walk = walk("latin,thai");

        assert_eq!(
            ok(among(&every, "Logitech K380", &walk)).key,
            LATIN,
            "somebody took Greek off this machine, so the board cannot be left on it"
        );
    }

    #[test]
    fn a_word_nobody_here_wrote_is_not_an_alphabet() {
        let Ok(every) = of("Logitech K380 = klingon\n");
        let walk = walk("latin,greek");

        assert_eq!(ok(among(&every, "Logitech K380", &walk)).key, LATIN);
    }

    #[test]
    fn what_is_written_reads_back_the_same() {
        let Ok(every) = of("b = greek\na = thai\n");
        let Ok(said) = written(&every);
        let Ok(again) = of(&said);

        assert_eq!(every, again);
        assert_eq!(said, "a = thai\nb = greek\n", "one line per keyboard, in a settled order");
    }

    #[test]
    fn a_name_with_room_in_it_is_a_name() {
        let Ok(every) = of("Keychron K3 Pro = greek\n");

        assert_eq!(every.get("Keychron K3 Pro").map(String::as_str), Some("greek"));
    }

    #[test]
    fn what_is_remembered_is_what_comes_back() {
        let home = tempfile::tempdir().expect("somewhere");
        let Ok(greek) = crate::one("greek");
        let greek = greek.expect("greek");

        remember(home.path(), "Logitech K380", greek).expect("a line written");

        let Ok(every) = every(home.path());
        let walk = walk("latin,greek");

        assert_eq!(ok(among(&every, "Logitech K380", &walk)).key, "greek");
    }
}
