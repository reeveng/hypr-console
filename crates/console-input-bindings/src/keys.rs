//! The keyboard's words, and the two other vocabularies they have to reach.
//!
//! A person says `super + i`. The kernel says `KEY_I` with `KEY_LEFTMETA`
//! held, which is what the card reading a press off a device is handed. The
//! compositor wants a bind, and that is the third: `SUPER+code:31`, joined
//! with a plus because the compositor is configured in lua and `hl.bind`
//! parses one key string rather than the legacy parser's comma. Its own
//! answer comes back a fourth way -- a number with a bit per modifier -- so
//! [`mask`] is the same list read the other way round, and neither is spelled
//! anywhere but here.
//!
//! `code:` rather than a keysym is the decision worth arguing. Hyprland will
//! take either, and a keysym is what the key *produces*, which is a function
//! of the layout the keyboard is wearing -- so a shortcut written as `I` stops
//! working the moment somebody switches that keyboard to Greek, and stops
//! working differently on each layout. A code is the key itself, in the place
//! it is on the board, whatever it is currently printing. That is what a
//! person means by "Super and I": the key their finger is already on. The
//! number is the kernel's plus eight, which is X11's offset and the one piece
//! of arithmetic in this file.
//!
//! What is *not* here is what a key types. That is xkb's, and asking it is
//! `console_input_keyboard::keymap` -- a crate this one may not link, because
//! it is a port of wvkbd and carries wvkbd's licence. Nothing here needs it:
//! a binding names a key, and naming a key is not the same question as what
//! comes out when it is pressed.

use std::str::FromStr;

use console_core_never::Never;
use evdev::KeyCode;

const X11: u16 = 8;

const PREFIX: &str = "KEY_";

const JOINED: &str = "+";

pub const MODIFIERS: [(&str, &str, u64, KeyCode, KeyCode); 4] = [
    ("super", "SUPER", 64, KeyCode::KEY_LEFTMETA, KeyCode::KEY_RIGHTMETA),
    ("ctrl", "CTRL", 4, KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTCTRL),
    ("shift", "SHIFT", 1, KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_RIGHTSHIFT),
    ("alt", "ALT", 8, KeyCode::KEY_LEFTALT, KeyCode::KEY_RIGHTALT),
];

const NAMED: [&str; 35] = [
    "space",
    "enter",
    "tab",
    "escape",
    "backspace",
    "delete",
    "insert",
    "home",
    "end",
    "pageup",
    "pagedown",
    "up",
    "down",
    "left",
    "right",
    "minus",
    "equal",
    "leftbrace",
    "rightbrace",
    "semicolon",
    "apostrophe",
    "grave",
    "backslash",
    "comma",
    "period",
    "slash",
    "capslock",
    "print",
    "power",
    "mute",
    "volume-up",
    "volume-down",
    "brightness-up",
    "brightness-down",
    "calculator",
];

const CALLED: [(&str, &str); 9] = [
    ("escape", "esc"),
    ("period", "dot"),
    ("return", "enter"),
    ("print", "sysrq"),
    ("volume-up", "volumeup"),
    ("volume-down", "volumedown"),
    ("brightness-up", "brightnessup"),
    ("brightness-down", "brightnessdown"),
    ("calculator", "calc"),
];

const LETTERS: &str = "abcdefghijklmnopqrstuvwxyz";

const DIGITS: &str = "1234567890";

const FUNCTIONS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Words {
    AModifier,
    AKey,
}

pub fn every() -> Result<Vec<String>, Never> {
    let mut said: Vec<String> = LETTERS.chars().map(|letter| letter.to_string()).collect();

    said.extend(DIGITS.chars().map(|digit| digit.to_string()));
    said.extend((1..=FUNCTIONS).map(|which| format!("f{which}")));
    said.extend(NAMED.iter().map(|word| (*word).to_string()));

    Ok(said)
}

pub fn is_a_modifier(word: &str) -> Result<Words, Never> {
    Ok(match MODIFIERS.iter().any(|(spoken, _, _, _, _)| *spoken == word) {
        true => Words::AModifier,
        false => Words::AKey,
    })
}

pub fn modifier_of(code: KeyCode) -> Result<Option<&'static str>, Never> {
    Ok(MODIFIERS
        .iter()
        .find(|(_, _, _, left, right)| *left == code || *right == code)
        .map(|(spoken, _, _, _, _)| *spoken))
}

pub fn held_as(word: &str) -> Result<Option<&'static str>, Never> {
    Ok(MODIFIERS
        .iter()
        .find(|(spoken, _, _, _, _)| *spoken == word)
        .map(|(_, hyprland, _, _, _)| *hyprland))
}

pub fn code(word: &str) -> Result<Option<KeyCode>, Never> {
    let Ok(every) = every();

    match every.iter().any(|said| said == word) {
        true => {},
        false => return Ok(None),
    }

    let tail = CALLED
        .iter()
        .find(|(spoken, _)| *spoken == word)
        .map_or(word, |(_, kernel)| *kernel);

    let named = format!("{PREFIX}{}", tail.to_uppercase());

    Ok(match KeyCode::from_str(&named) {
        Ok(code) => Some(code),
        Err(_unknown) => {
            eprintln!(
                "console-input-bindings: {word:?} is a word this desktop uses and the kernel \
                 has no {named}, so nothing can be bound to it"
            );

            None
        }
    })
}

pub fn spoken(code: KeyCode) -> Result<Option<String>, Never> {
    let Ok(every) = every();

    Ok(every.into_iter().find(|word| {
        let Ok(found) = self::code(word);

        found == Some(code)
    }))
}

pub fn key_named(word: &str) -> Result<KeyCode, String> {
    let Ok(found) = code(word);

    found.ok_or_else(|| format!("no key called {word:?}"))
}

pub fn bind(held: &[String], pressed: &str) -> Result<Option<String>, Never> {
    let Ok(code) = code(pressed);

    let code = match code {
        Some(code) => code,
        None => return Ok(None),
    };
    let mut words: Vec<&str> = Vec::new();

    for word in held {
        let Ok(hyprland) = held_as(word);

        match hyprland {
            Some(hyprland) => words.push(hyprland),
            None => return Ok(None),
        }
    }

    let mut said = words.join(JOINED);

    match said.is_empty() {
        true => {},
        false => said.push_str(JOINED),
    }

    Ok(Some(format!("{said}code:{}", code.0.saturating_add(X11))))
}

pub fn mask(held: &[String]) -> Result<Option<u64>, Never> {
    let mut said: u64 = 0;

    for word in held {
        let found = MODIFIERS.iter().find(|(spoken, _, _, _, _)| *spoken == word);

        match found {
            Some((_, _, mask, _, _)) => said |= mask,
            None => return Ok(None),
        }
    }

    Ok(Some(said))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    #[test]
    fn a_letter_is_the_key_of_that_name() {
        assert_eq!(ok(code("i")), Some(KeyCode::KEY_I));
        assert_eq!(ok(code("f5")), Some(KeyCode::KEY_F5));
        assert_eq!(ok(code("space")), Some(KeyCode::KEY_SPACE));
    }

    #[test]
    fn the_words_a_person_uses_are_not_always_the_kernels() {
        assert_eq!(ok(code("escape")), Some(KeyCode::KEY_ESC));
        assert_eq!(ok(code("period")), Some(KeyCode::KEY_DOT));
    }

    #[test]
    fn every_word_this_desktop_uses_is_a_key_the_kernel_has() {
        let Ok(every) = every();

        for word in &every {
            let Ok(found) = code(word);

            assert!(found.is_some(), "{word:?} is a word here and no key on any keyboard");
        }
    }

    #[test]
    fn a_key_is_said_back_the_way_it_was_written() {
        for word in ok(every()) {
            let Ok(code) = code(&word);
            let code = code.unwrap_or_else(|| panic!("{word} is offered and is no key"));

            assert_eq!(ok(spoken(code)), Some(word.clone()), "{word}");
        }
    }

    #[test]
    fn what_is_held_is_a_number_the_compositor_says_back() {
        assert_eq!(ok(mask(&[])), Some(0));
        assert_eq!(ok(mask(&["super".to_string()])), Some(64));
        assert_eq!(
            ok(mask(&["super".to_string(), "shift".to_string()])),
            ok(mask(&["shift".to_string(), "super".to_string()])),
            "a mask is what is held and not the order it was written in"
        );
        assert_eq!(ok(mask(&["nonesuch".to_string()])), None);
    }

    #[test]
    fn nothing_this_desktop_cannot_say_is_a_key() {
        assert_eq!(ok(code("leftmeta")), None, "a modifier is held, not pressed");
        assert_eq!(ok(code("nonesuch")), None);
        assert!(key_named("nonesuch").is_err());
    }

    #[test]
    fn a_modifier_is_held_and_the_compositor_has_its_own_word_for_it() {
        assert_eq!(is_a_modifier("super"), Ok(Words::AModifier));
        assert_eq!(is_a_modifier("i"), Ok(Words::AKey));
        assert_eq!(ok(held_as("ctrl")), Some("CTRL"));
        assert_eq!(ok(modifier_of(KeyCode::KEY_RIGHTSHIFT)), Some("shift"));
    }

    #[test]
    fn a_bind_names_the_key_by_where_it_is_and_not_by_what_it_types() {
        let held = vec!["super".to_string()];

        assert_eq!(
            ok(bind(&held, "i")),
            Some(format!("SUPER+code:{}", KeyCode::KEY_I.0.saturating_add(X11)))
        );
    }

    #[test]
    fn a_key_on_its_own_is_a_bind_with_nothing_in_front_of_it() {
        assert_eq!(
            ok(bind(&[], "f5")),
            Some(format!("code:{}", KeyCode::KEY_F5.0.saturating_add(X11)))
        );
    }
}
