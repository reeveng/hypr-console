//! No two things on the keyboard may be the same colour, and none is a wash.
//!
//! This fault has happened three times. First the slab behind the keys and a
//! key that is not a letter were both `ground`, so Esc and Tab and the arrows
//! had nothing under them. Then the slab and a key being pressed were both
//! `night`, so a key vanished at the moment it was pressed. Both times the
//! keyboard looked see-through, and both times nothing said anything. The third
//! time was the key the stick is sitting on, drawn in the swipe colour, and a
//! swipe's colour is a quarter of a colour by design: it is a wash laid over a
//! key to show where a finger went. The wallpaper came through the letter.
//!
//! The unit tests beside `argv` ask this of a palette written for them, which
//! catches a table that pairs two things wrongly. This asks it of the palette
//! the machine actually spends, which is the other half: a table that is right
//! and two colours in `theme/palette.toml` that have drifted into each other.
//!
//! It read `osk-start` with regular expressions until the script became a
//! program. What it asks is unchanged.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_core_colour::spent::{SPENT, read};
use console_input_keyboard::palette::{self, BACKGROUNDS, COLOURS, INK};

fn role(option: &str) -> Option<&'static str> {
    let Ok(role) = palette::role(option);

    role
}

fn missing(palette: &BTreeMap<String, String>) -> Vec<&'static str> {
    let Ok(missing) = palette::missing(palette);

    missing
}

fn argv(palette: &BTreeMap<String, String>, rest: &[String]) -> Vec<String> {
    let Ok(argv) = palette::argv(palette, rest);

    argv
}

fn root() -> PathBuf {
    {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

fn palette() -> BTreeMap<String, String> {
    let held = std::fs::read_to_string(root().join("files").join(SPENT)).expect("the palette");
    let Ok(palette) = read(&held);

    assert!(!palette.is_empty(), "no colours in files/{SPENT}");

    palette
}

#[test]
fn every_colour_it_spends_is_in_the_palette() {
    assert_eq!(missing(&palette()), Vec::<&str>::new());
}

#[test]
fn no_two_backgrounds_are_the_same_colour() {
    let palette = palette();
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for option in BACKGROUNDS {
        let colour = match palette.get(role(option).expect(option)) {
            Some(colour) => colour,
            None => continue,
        };
        if let Some(other) = seen.get(colour.as_str()) {
            panic!(
                "--{option} and --{other} are both #{colour}, so one of them is invisible \
                 against the other"
            );
        }
        seen.insert(colour, option);
    }
}

#[test]
fn pressed_and_selected_keys_are_seen() {
    let palette = palette();
    let dark_ink = match palette.get("night") {
        Some(ink) => ink.as_str(),
        None => "",
    };
    assert!(!dark_ink.is_empty(), "the palette has no `night` for the keyboard to write in");
    for option in ["press", "sel"] {
        let background = match role(option).and_then(|named| palette.get(named)) {
            Some(colour) => colour.as_str(),
            None => "",
        };
        assert!(!background.is_empty(), "--{option} has no colour in the palette");
        let Ok(apart) = console_core_colour::contrast(background, dark_ink);

        assert!(
            apart >= 7.0,
            "--{option} (#{background}) carries #{dark_ink} ink at {apart:.2}:1, less than the \
             7:1 a thumb on a key needs to be told from the key at rest"
        );
    }
}

const APART: f64 = 35.0;

#[test]
fn a_pressed_key_is_not_the_key_under_the_stick() {
    let palette = palette();
    let colour = |option: &str| {
        role(option)
            .and_then(|named| palette.get(named))
            .unwrap_or_else(|| panic!("--{option} has no colour in the palette"))
            .clone()
    };
    let (press, sel) = (colour("press"), colour("sel"));
    assert_ne!(press, sel, "--press and --sel are the same colour, so a key never looks typed");

    let Ok((_, _, one)) = console_core_colour::to_oklch(&press);
    let Ok((_, _, other)) = console_core_colour::to_oklch(&sel);
    let round = (one - other).abs();
    let apart = round.min(360.0 - round);
    assert!(
        apart >= APART,
        "--press (#{press}) and --sel (#{sel}) are {apart:.1} degrees of hue apart, under the \
         {APART:.0} two pastels of one lightness need to read as two colours. They are the same \
         key a moment apart, so a thumb cannot tell what it has just done."
    );
}

#[test]
fn nothing_is_written_in_the_colour_it_is_written_on() {
    let palette = palette();
    for (background, ink) in INK {
        let ink = match ink {
            Some(ink) => ink,
            None => continue,
        };
        let (under, over) = (role(background).expect(background), role(ink).expect(ink));
        assert_ne!(
            palette.get(under),
            palette.get(over),
            "--{background} and --{ink} are the same colour, so the writing is invisible"
        );
    }
}

#[test]
fn every_background_is_named_at_all() {
    let argv = argv(&palette(), &[]);
    for option in BACKGROUNDS {
        assert!(
            argv.iter().any(|word| *word == format!("--{option}")),
            "the keyboard is never told what colour --{option} is, so it keeps the one it was \
             compiled with"
        );
    }
}

#[test]
fn no_colour_is_written_as_anything_but_six_digits() {
    let argv = argv(&palette(), &[]);
    for (option, _) in COLOURS {
        let at = argv.iter().position(|word| *word == format!("--{option}")).expect(option);
        let given = &argv[at + 1];
        assert_eq!(given.len(), 6, "--{option} is given {given}");
        assert!(
            given.chars().all(|l| l.is_ascii_hexdigit()),
            "--{option} is given {given}. A colour here is six digits and nothing else: the \
             keyboard is read against the wallpaper, and anything after them is an alpha."
        );
    }
}
