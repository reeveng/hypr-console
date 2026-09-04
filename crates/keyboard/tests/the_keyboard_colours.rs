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
use keyboard::palette::{BACKGROUNDS, COLOURS, INK, argv, missing, role};

fn root() -> PathBuf {
    {
    // Tidied by `canonicalize` where that works and left as it stands where it
    // does not. What `CARGO_MANIFEST_DIR` gives is already absolute and already
    // right; canonicalizing only takes the `../..` out of the middle. It fails
    // under a sandbox that will not let a process resolve a path it can
    // otherwise read, and a test that stops there reports the sandbox as a
    // missing repository.
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
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
///
/// The mechanism that tells a pressed key from a selected one is the ink,
/// not the background: `--press` and `--sel` are both pastel, and what
/// changes between a thumb on a key and a key at rest is that the ink
/// turns from `text` (light, on `panel`) to `night` (dark, on pastel).
/// That is checked separately by `pressed_and_selected_keys_are_seen`.
/// This one keeps catching the older fault: two options written the same
/// hex, so one of them has nothing under it.
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

/// A thumb on a key is told from a key at rest by the ink flipping dark.
///
/// `--press` and `--sel` are pastel, near every other pastel by lightness:
/// `theme/palette.toml` deliberately pins every accent to `0.855` so no
/// colour shouts over its neighbour. Two pastels drawn next to each other
/// cannot be told apart by background alone. What tells them apart is that
/// the ink on a pressed or selected key is `night` (dark), whereas a key at
/// rest has `text` (light) on a dark background. So the affordance lives in
/// the *inversion*: pressed and selected keys clear the dark-ink-on-light
/// contract the palette writes for every pastel.
///
/// That contract is the `[[pair]]` of `night` against every pastel in
/// `palette.toml`, asking for 7:1. This test asks the same question of the
/// pair the keyboard actually composes on a key: the dark ink (`night`) against
/// the pastel that ink is written on (`mauve` for press, `pink` for sel).
/// If either pair falls under the 7:1 line, the press or selection reads
/// as faint ink on a pale field and the question the row was answering —
/// "is it pressed" — goes unanswered.
///
/// It says nothing about the two of them against *each other*. Nothing about
/// contrast can: they are both accents, every accent is pinned to one
/// lightness, and the pair is 1.02:1 whichever two are chosen. That is
/// `a_pressed_key_is_not_the_key_under_the_stick` below, and it is a question
/// about hue.
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
        let apart = console_colour::contrast(background, dark_ink);
        assert!(
            apart >= 7.0,
            "--{option} (#{background}) carries #{dark_ink} ink at {apart:.2}:1, less than the \
             7:1 a thumb on a key needs to be told from the key at rest"
        );
    }
}

/// How far apart in hue two pastels have to be to read as two colours.
///
/// Every accent in `theme/palette.toml` is pinned to `lightness = 0.855` so
/// that no colour shouts over its neighbour, which is right for ten terminal
/// colours seen side by side and leaves exactly one thing free to tell any two
/// of them apart. `pink` and `rose` sat 26 degrees apart and were the fault
/// this line exists above: the same lightness, the same chroma to within a
/// hundredth, and a hue step small enough that the two read as one pale key
/// with the light catching it differently. The line is drawn over that number
/// rather than at some round figure, because that number is the one that was
/// looked at on a screen and found wanting.
const APART: f64 = 35.0;

/// The key being typed and the key the stick is on are two colours.
///
/// These are the two states one key is in a fifth of a second apart -- the
/// stick arrives, then A is pressed -- so they are the pair a person compares
/// most often and the pair that has to survive being compared. `--press` is
/// `mauve` and `--sel` is `pink`, which is the palette's own arrangement read
/// back: `pink` is spent as "the highlighted row, the key under your thumb"
/// and `mauve` as "a key held down".
///
/// Asked as a hue and not as a contrast, and that is the whole point of the
/// test. `theme/palette.toml` gives every accent one lightness, so the
/// contrast between any two of them is between 1.00:1 and 1.02:1 and a
/// threshold on it can only ever be written as "they are the same brightness",
/// which they are, on purpose. Hue is the axis the palette actually varies,
/// so hue is the axis the question has to be asked on.
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

    let (_, _, one) = console_colour::to_oklch(&press);
    let (_, _, other) = console_colour::to_oklch(&sel);
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
        let Some(ink) = ink else { continue };
        let (under, over) = (role(background).expect(background), role(ink).expect(ink));
        assert_ne!(
            palette.get(under),
            palette.get(over),
            "--{background} and --{ink} are the same colour, so the writing is invisible"
        );
    }
}

/// An option not passed leaves the keyboard on its own colour for it, which is
/// somebody else's palette: a red pressed key and a green selected one.
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
/// The keyboard reads a colour as `rrggbb` or as `rrggbbaa`, and six digits
/// leaves the alpha wherever its own defaults put it. Every default the port
/// keeps is opaque, so the way to end up with a see-through key is to write the
/// digits out with an alpha on the end. It is asked of every option on the
/// wire, the four the port drops included: they reach the C on the way back,
/// where the trail's own default really is a wash.
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
