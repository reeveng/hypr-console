//! No two things on the keyboard may be the same colour, and none is a wash.
//!
//! This fault has happened three times. First the slab behind the keys and a key
//! that is not a letter were both `ground`, so Esc and Tab and the arrows had
//! nothing under them. Then the slab and a key being pressed were both `night`,
//! so a key vanished at the moment it was pressed. Both times the keyboard
//! looked see-through, and both times nothing said anything. The third time was
//! the key the stick is sitting on, drawn in the swipe colour, and a swipe's
//! colour is a quarter of a colour by design: it is a wash laid over a key to
//! show where a finger went. The wallpaper came through the letter.
//!
//! The colours are read out of the command `osk-start` builds, so this asks
//! about what the keyboard is actually given rather than about what anybody
//! meant.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use regex::Regex;

const START: &str = "files/usr/local/bin/osk-start";
const SPENT: &str = "files/usr/local/lib/console/palette.sh";

/// What each background is called, and the ink written on it. The slab has no
/// ink of its own: nothing is written on the space between keys.
const INK: [(&str, Option<&str>); 9] = [
    ("bg", None),
    ("fg", Some("text")),
    ("fg-sp", Some("text-sp")),
    ("press", Some("text-press")),
    ("press-sp", Some("text-press-sp")),
    ("sel", Some("text-sel")),
    ("sel-sp", Some("text-sel-sp")),
    ("swipe", Some("text-swipe")),
    ("swipe-sp", Some("text-swipe-sp")),
];

const BACKGROUNDS: [&str; 5] = ["bg", "fg", "fg-sp", "press", "sel"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository")
}

/// Everything on that command line that is a colour rather than a size or a
/// font.
fn colours() -> Vec<String> {
    INK.iter()
        .flat_map(|(background, ink)| [Some(*background), *ink])
        .flatten()
        .map(str::to_string)
        .collect()
}

fn started() -> String {
    std::fs::read_to_string(root().join(START)).expect("osk-start")
}

/// The colours the palette spends, by the word each is spent as.
fn palette() -> BTreeMap<String, String> {
    std::fs::read_to_string(root().join(SPENT))
        .expect("the palette")
        .lines()
        .filter_map(|line| line.trim_end().split_once('='))
        .filter(|(name, _)| name.chars().all(|l| l.is_alphanumeric() || l == '_'))
        .filter(|(_, colour)| colour.len() == 6 && colour.chars().all(|l| l.is_ascii_hexdigit()))
        .map(|(name, colour)| (name.to_string(), colour.to_lowercase()))
        .collect()
}

/// Which colour `osk-start` hands to each of wvkbd's colour options.
fn spends() -> BTreeMap<String, String> {
    let named = Regex::new(r#"--([a-z-]+) "\$(\w+)""#).expect("a pattern");
    let known = colours();
    let found: BTreeMap<String, String> = named
        .captures_iter(&started())
        .map(|caught| (caught[1].to_string(), caught[2].to_string()))
        .filter(|(option, _)| known.contains(option))
        .collect();
    assert!(!found.is_empty(), "no colours are passed to wvkbd");
    found
}

/// The same options, as they are written rather than as they resolve.
fn raw() -> BTreeMap<String, String> {
    let written = Regex::new(r#"--([a-z-]+) "([^"]*)""#).expect("a pattern");
    let known = colours();
    written
        .captures_iter(&started())
        .map(|caught| (caught[1].to_string(), caught[2].to_string()))
        .filter(|(option, _)| known.contains(option))
        .collect()
}

#[test]
fn every_colour_it_spends_is_in_the_palette() {
    let known = palette();
    for (option, colour) in spends() {
        assert!(known.contains_key(&colour), "--{option} is ${colour}, which the palette does not have");
    }
}

/// A key the colour of the slab is a key with nothing under it.
#[test]
fn no_two_backgrounds_are_the_same_colour() {
    let (known, spends) = (palette(), spends());
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for option in BACKGROUNDS {
        let Some(colour) = spends.get(option).and_then(|name| known.get(name)) else { continue };
        if let Some(other) = seen.get(colour) {
            panic!("--{option} and --{other} are both #{colour}, so one of them is invisible against the other");
        }
        seen.insert(colour.clone(), option.to_string());
    }
}

#[test]
fn nothing_is_written_in_the_colour_it_is_written_on() {
    let (known, spends) = (palette(), spends());
    for (background, ink) in INK {
        let Some(ink) = ink else { continue };
        let (Some(under), Some(over)) = (spends.get(background), spends.get(ink)) else { continue };
        assert_ne!(
            known.get(under),
            known.get(over),
            "--{background} and --{ink} are the same colour, so the writing is invisible"
        );
    }
}

/// An option not passed leaves wvkbd on its own colour for it, which is somebody
/// else's palette and, for the selected key, a wash.
#[test]
fn every_background_is_named_at_all() {
    let raw = raw();
    for option in BACKGROUNDS {
        assert!(
            raw.contains_key(option),
            "the keyboard is never told what colour --{option} is, so it keeps the one it was \
             compiled with"
        );
    }
}

/// A colour here is named, and nothing is added to it.
///
/// wvkbd reads a colour as `rrggbb` or as `rrggbbaa`, and six digits leaves the
/// alpha wherever its own defaults put it. Every default is opaque except the
/// swipe trail, which is a wash on purpose. So the two ways to end up with a
/// see-through key are to write the digits out with an alpha on the end, and to
/// hand a key the trail's colour. This is the first; the second is
/// `no_two_backgrounds_are_the_same_colour` above.
#[test]
fn no_colour_is_written_as_anything_but_a_palette_name() {
    let name = Regex::new(r"^\$\w+$").expect("a pattern");
    for (option, given) in raw() {
        assert!(
            name.is_match(&given),
            "--{option} is given {given}. A colour here is a palette name and nothing else: the \
             keyboard is read against the wallpaper, and anything after the six digits is an alpha."
        );
    }
}
