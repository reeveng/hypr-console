//! Which alphabets this machine types.
//!
//! The keyboard ships an arrangement for every script wvkbd ever carried and
//! composes the keymap out of the system's own xkb symbols, so what a person
//! can type has never been the keyboard's limit. What decided it was a word on
//! the command line in a unit file: `--landscape-layers landscape,thai,...`,
//! written by whoever built the device, in a tree that names nobody. Somebody
//! who reads Greek had no way to say so and no reason to know there was
//! anything to say.
//!
//! So it is a setting, and this is where the setting lives. It is here rather
//! than in `console-input-keyboard` because that crate is GPL-3.0-or-later --
//! it is a port of wvkbd -- and the settings panel is AGPL like the rest of the
//! tree. The two sit beside each other and nothing in this workspace links
//! them. A list both of them have to agree on therefore cannot live in either,
//! and this crate is small enough to be the thing they agree through: the
//! keyboard reads it to build its walk, the panel reads it to draw the rows,
//! and neither spells an arrangement name.
//!
//! An alphabet is a script rather than a language. French and German are Latin
//! with a different xkb tag, and the keyboard's compose shelves already reach
//! their letters, so they are not choices here; a script that needs keys the
//! latin arrangement has not got is. What makes the difference is whether the
//! keyboard has an arrangement of its own for it, which is the same rule
//! `primary` marks over there.
//!
//! **Latin does not come off.** Every word this desktop says is English, every
//! address is latin, and a person who took it away would be holding a machine
//! that cannot type its own search box back. `read` puts it in front whatever
//! it was handed, which makes the row that would remove it a row that does
//! nothing rather than a way to brick the keyboard.
//!
//! ## Two keyboards, one word
//!
//! An alphabet carries an xkb name as well as an arrangement, because there
//! are two keyboards on this machine and switching language has to mean the
//! same thing on both. The one drawn on the screen wears an arrangement; a
//! keyboard somebody plugged in wears a layout the compositor sets. They are
//! the same choice said twice, so they are one row here rather than two lists
//! that would have to be kept in step, and the walk a person steps through is
//! the same walk whichever board is under their hands.
//!
//! `crate::wearing` is what each one is wearing now, and it is per keyboard
//! rather than per machine: two boards on one desk are two people's habits as
//! often as one person's, and a switch on one that moved the other would be a
//! machine deciding they are the same hand.

use console_core_never::Never;

pub mod wearing;

pub struct Alphabet {
    pub key: &'static str,
    pub says: &'static str,
    pub upright: &'static str,
    pub across: &'static str,
    pub xkb: &'static str,
}

pub const LATIN: &str = "latin";

pub const EVERY: [Alphabet; 8] = [
    Alphabet { key: LATIN, says: "Latin", upright: "full", across: "landscape", xkb: US },
    Alphabet { key: "arabic", says: "Arabic", upright: "arabic", across: "arabic", xkb: "ara" },
    Alphabet {
        key: "georgian",
        says: "Georgian",
        upright: "georgian",
        across: "georgian",
        xkb: "ge",
    },
    Alphabet { key: "greek", says: "Greek", upright: "greek", across: "greek", xkb: "gr" },
    Alphabet { key: "hebrew", says: "Hebrew", upright: "hebrew", across: "hebrew", xkb: "il" },
    Alphabet { key: "persian", says: "Persian", upright: "persian", across: "persian", xkb: "ir" },
    Alphabet { key: "cyrillic", says: "Russian", upright: "cyrillic", across: "cyrillic", xkb: "ru" },
    Alphabet { key: "thai", says: "Thai", upright: "thai", across: "thai", xkb: "th" },
];

pub const US: &str = "us";

pub const UNLESS_TOLD: &str = "latin,thai";

const SETTING: &str = "alphabets";

const SHELF_UPRIGHT: &str = "special";

const SHELF_ACROSS: &str = "landscapespecial";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Upright,
    Across,
}

pub fn one(key: &str) -> Result<Option<&'static Alphabet>, Never> {
    Ok(EVERY.iter().find(|alphabet| alphabet.key == key))
}

pub fn latin() -> Result<&'static Alphabet, Never> {
    let [first, ..] = &EVERY;

    Ok(first)
}

pub fn read(said: &str) -> Result<Vec<&'static Alphabet>, Never> {
    let asked: Vec<&str> = said.split(',').map(str::trim).collect();
    let mut kept: Vec<&'static Alphabet> = Vec::new();

    for alphabet in &EVERY {
        let wanted = alphabet.key == LATIN || asked.contains(&alphabet.key);

        match wanted {
            true => kept.push(alphabet),
            false => {},
        }
    }

    Ok(kept)
}

pub fn chosen() -> Result<Vec<&'static Alphabet>, Never> {
    let told = console_default_applications::setting(SETTING)?;

    let said = match told {
        Some(said) => said,
        None => UNLESS_TOLD.to_string(),
    };

    read(&said)
}

pub fn choose(keys: &[&str]) -> Result<(), Never> {
    let Ok(kept) = read(&keys.join(","));

    let said: Vec<&str> = kept.iter().map(|alphabet| alphabet.key).collect();

    console_default_applications::set(SETTING, &said.join(","))
}

pub fn turned(now: &[&'static Alphabet], key: &str) -> Result<Vec<&'static str>, Never> {
    let held = now.iter().any(|alphabet| alphabet.key == key);

    let keys: Vec<&str> = match held {
        true => now.iter().map(|alphabet| alphabet.key).filter(|kept| *kept != key).collect(),
        false => {
            let mut keys: Vec<&str> = now.iter().map(|alphabet| alphabet.key).collect();

            keys.push(key);

            keys
        }
    };

    let Ok(kept) = read(&keys.join(","));

    Ok(kept.iter().map(|alphabet| alphabet.key).collect())
}

pub fn walk(alphabets: &[&'static Alphabet], held: Held) -> Result<Vec<String>, Never> {
    let mut walk: Vec<String> = alphabets
        .iter()
        .map(|alphabet| {
            match held {
                Held::Upright => alphabet.upright.to_string(),
                Held::Across => alphabet.across.to_string(),
            }
        })
        .collect();

    walk.push(match held {
        Held::Upright => SHELF_UPRIGHT.to_string(),
        Held::Across => SHELF_ACROSS.to_string(),
    });

    Ok(walk)
}

pub fn said(alphabets: &[&'static Alphabet]) -> Result<String, Never> {
    let says: Vec<&str> = alphabets.iter().map(|alphabet| alphabet.says).collect();

    Ok(says.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn read(said: &str) -> Vec<&'static str> {
        let Ok(kept) = super::read(said);

        kept.iter().map(|alphabet| alphabet.key).collect()
    }

    #[test]
    fn what_the_device_typed_before_this_was_a_setting_is_what_it_types_untold() {
        assert_eq!(read(UNLESS_TOLD), vec![LATIN, "thai"]);
    }

    #[test]
    fn the_first_row_is_latin_which_is_what_lets_it_be_asked_for_infallibly() {
        let Ok(first) = latin();

        assert_eq!(first.key, LATIN);
        assert_eq!(first.xkb, US, "and it is what a keyboard nobody has met wears");
    }

    #[test]
    fn every_alphabet_carries_a_layout_the_compositor_would_take() {
        for alphabet in &EVERY {
            assert!(!alphabet.xkb.is_empty(), "{} has no xkb name", alphabet.key);
            assert!(
                alphabet.xkb.chars().all(|letter| letter.is_ascii_lowercase()),
                "{} is not a layout name: {}",
                alphabet.key,
                alphabet.xkb
            );
        }
    }

    #[test]
    fn latin_is_there_whatever_it_was_handed() {
        assert_eq!(read("thai"), vec![LATIN, "thai"]);
        assert_eq!(read(""), vec![LATIN]);
        assert_eq!(read("nothing anybody has written"), vec![LATIN]);
    }

    #[test]
    fn the_row_that_would_take_latin_away_is_a_row_that_does_nothing() {
        let Ok(now) = super::read("latin,thai");
        let Ok(turned) = turned(&now, LATIN);

        assert_eq!(turned, vec![LATIN, "thai"]);
    }

    #[test]
    fn pressing_one_that_is_not_there_adds_it_and_pressing_it_again_takes_it_off() {
        let Ok(now) = super::read(UNLESS_TOLD);
        let Ok(with) = turned(&now, "greek");

        assert_eq!(with, vec![LATIN, "greek", "thai"]);

        let Ok(now) = super::read(&with.join(","));
        let Ok(without) = turned(&now, "greek");

        assert_eq!(without, vec![LATIN, "thai"]);
    }

    #[test]
    fn an_alphabet_asked_for_twice_is_walked_once() {
        assert_eq!(read("thai,thai,latin"), vec![LATIN, "thai"]);
    }

    #[test]
    fn the_shelf_of_symbols_is_the_last_thing_the_language_key_reaches() {
        let Ok(now) = super::read(UNLESS_TOLD);
        let Ok(across) = walk(&now, Held::Across);
        let Ok(upright) = walk(&now, Held::Upright);

        assert_eq!(across, vec!["landscape", "thai", "landscapespecial"]);
        assert_eq!(upright, vec!["full", "thai", "special"]);
    }

    #[test]
    fn no_two_alphabets_share_a_key_a_word_or_an_arrangement() {
        let keys: BTreeSet<&str> = EVERY.iter().map(|alphabet| alphabet.key).collect();
        let says: BTreeSet<&str> = EVERY.iter().map(|alphabet| alphabet.says).collect();
        let upright: BTreeSet<&str> = EVERY.iter().map(|alphabet| alphabet.upright).collect();
        let across: BTreeSet<&str> = EVERY.iter().map(|alphabet| alphabet.across).collect();

        assert_eq!(keys.len(), EVERY.len());
        assert_eq!(says.len(), EVERY.len());
        assert_eq!(upright.len(), EVERY.len());
        assert_eq!(across.len(), EVERY.len());
    }

    #[test]
    fn what_is_drawn_beside_the_row_is_the_alphabets_in_the_order_they_are_walked() {
        let Ok(now) = super::read("thai,greek");
        let Ok(said) = said(&now);

        assert_eq!(said, "Latin, Greek, Thai");
    }
}
