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

use console_colour::spent::{SPENT, read};
use console_keyboard::{BACKGROUNDS, COLOURS, INK, argv, missing, role};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository")
}

/// The colours this repository spends, by the word each is spent as.
fn palette() -> BTreeMap<String, String> {
    let held = std::fs::read_to_string(root().join("files").join(SPENT)).expect("the palette");
    let palette = read(&held);
    assert!(!palette.is_empty(), "no colours in files/{SPENT}");
    palette
}

#[test]
fn every_colour_it_spends_is_in_the_palette() {
    assert_eq!(missing(&palette()), Vec::<&str>::new());
}

/// A key the colour of the slab is a key with nothing under it.
#[test]
fn no_two_backgrounds_are_the_same_colour() {
    let palette = palette();
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for option in BACKGROUNDS {
        let Some(colour) = palette.get(role(option).expect(option)) else { continue };
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
fn nothing_is_written_in_the_colour_it_is_written_on() {
    let palette = palette();
    for (background, ink) in INK {
        let Some(ink) = ink else { continue };
        let (under, over) = (role(background).expect(background), role(ink).expect(ink));
        assert_ne!(
            palette.get(under),
            palette.get(over),
            "--{background} and --{ink} are the same colour, so the writing is invisible"
        );
    }
}

/// An option not passed leaves wvkbd on its own colour for it, which is
/// somebody else's palette and, for the selected key, a wash.
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

/// A colour here is six digits and nothing else.
///
/// wvkbd reads a colour as `rrggbb` or as `rrggbbaa`, and six digits leaves the
/// alpha wherever its own defaults put it. Every default is opaque except the
/// swipe trail, which is a wash on purpose. So the two ways to end up with a
/// see-through key are to write the digits out with an alpha on the end, and to
/// hand a key the trail's colour. This is the first; the second is
/// `no_two_backgrounds_are_the_same_colour` above.
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
