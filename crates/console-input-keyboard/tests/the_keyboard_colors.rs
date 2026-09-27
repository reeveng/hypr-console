//! No two things on the keyboard may be the same color, and none is a wash.
//!
//! This fault has happened three times. First the slab behind the keys and a
//! key that is not a letter were both `ground`, so Esc and Tab and the arrows
//! had nothing under them. Then the slab and a key being pressed were both
//! `night`, so a key vanished at the moment it was pressed. Both times the
//! keyboard looked see-through, and both times nothing said anything. The third
//! time was the key the stick is sitting on, drawn in the swipe color, and a
//! swipe's color is a quarter of a color by design: it is a wash laid over a
//! key to show where a finger went. The wallpaper came through the letter.
//!
//! The unit tests beside `arguments` ask this of a palette written for them, which
//! catches a table that pairs two things wrongly. This asks it of the palette
//! the machine actually spends, which is the other half: a table that is right
//! and two colors in `theme/palette.toml` that have drifted into each other.
//!
//! It read `osk-start` with regular expressions until the script became a
//! program. What it asks is unchanged.

use std::collections::BTreeMap;
use std::path::Path;

use console_core_color::{Ground, HexColor};
use console_core_color::palette::{SPENT, read};
use console_input_keyboard::palette::{self, BACKGROUNDS, COLORS, INK};

type Failure = Box<dyn std::error::Error>;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

const APART: f64 = 35.0;

fn spent() -> Result<BTreeMap<String, String>, Failure> {
    let held = std::fs::read_to_string(Path::new(ROOT).join("files").join(SPENT))?;
    let Ok(palette) = read(&held);

    assert!(!palette.is_empty(), "no colors in files/{SPENT}");

    Ok(palette)
}

fn role(option: &str) -> Result<&'static str, Failure> {
    let Ok(role) = palette::role(option);
    let role = role.ok_or(format!("--{option} has no role in the palette"))?;

    Ok(role)
}

#[test]
fn every_color_it_spends_is_in_the_palette() -> Result<(), Failure> {
    let spent = spent()?;

    assert_eq!(palette::missing_colors(&spent), Ok(Vec::<&str>::new()));

    Ok(())
}

#[test]
fn no_two_backgrounds_are_the_same_color() -> Result<(), Failure> {
    let spent = spent()?;
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();

    for option in BACKGROUNDS {
        let named = role(option)?;
        let color = match spent.get(named) {
            Some(color) => color,
            None => continue,
        };
        let other = seen.insert(color.as_str(), option);

        assert!(
            other.is_none(),
            "--{option} and --{other:?} are both #{color}, so one of them is invisible \
             against the other"
        );
    }

    Ok(())
}

#[test]
fn pressed_and_selected_keys_are_seen() -> Result<(), Failure> {
    let spent = spent()?;
    let dark_ink = spent.get("night").ok_or("the palette has no `night` for the keyboard to write in")?;

    for option in ["press", "sel"] {
        let named = role(option)?;
        let background = spent.get(named).ok_or(format!("--{option} has no color in the palette"))?;
        let Ok(apart) = console_core_color::contrast(HexColor(dark_ink), Ground(background));

        assert!(
            apart >= 7.0,
            "--{option} (#{background}) carries #{dark_ink} ink at {apart:.2}:1, less than the \
             7:1 a thumb on a key needs to be told from the key at rest"
        );
    }

    Ok(())
}

#[test]
fn a_pressed_key_is_not_the_key_under_the_stick() -> Result<(), Failure> {
    let spent = spent()?;
    let press_role = role("press")?;
    let selected_role = role("sel")?;
    let press = spent.get(press_role).ok_or("--press has no color in the palette")?;
    let selected = spent.get(selected_role).ok_or("--sel has no color in the palette")?;

    assert_ne!(press, selected, "--press and --sel are the same color, so a key never looks typed");

    let Ok(pressed) = console_core_color::to_oklch(press);
    let Ok(under) = console_core_color::to_oklch(selected);
    let round = (pressed.hue - under.hue).abs();
    let apart = round.min(360.0 - round);

    assert!(
        apart >= APART,
        "--press (#{press}) and --sel (#{selected}) are {apart:.1} degrees of hue apart, under the \
         {APART:.0} two pastels of one lightness need to read as two colors. They are the same \
         key a moment apart, so a thumb cannot tell what it has just done."
    );

    Ok(())
}

#[test]
fn nothing_is_written_in_the_color_it_is_written_on() -> Result<(), Failure> {
    let spent = spent()?;

    for (background, ink) in INK {
        let ink = match ink {
            Some(ink) => ink,
            None => continue,
        };
        let under = role(background)?;
        let over = role(ink)?;

        assert_ne!(
            spent.get(under),
            spent.get(over),
            "--{background} and --{ink} are the same color, so the writing is invisible"
        );
    }

    Ok(())
}

#[test]
fn every_background_is_named_at_all() -> Result<(), Failure> {
    let spent = spent()?;
    let Ok(arguments) = palette::arguments(&spent, &[]);

    for option in BACKGROUNDS {
        let flag = format!("--{option}");

        assert!(
            arguments.contains(&flag),
            "the keyboard is never told what color --{option} is, so it keeps the one it was \
             compiled with"
        );
    }

    Ok(())
}

#[test]
fn no_color_is_written_as_anything_but_six_digits() -> Result<(), Failure> {
    let spent = spent()?;
    let Ok(arguments) = palette::arguments(&spent, &[]);

    for (option, _) in COLORS {
        let flag = format!("--{option}");
        let given = arguments
            .iter()
            .skip_while(|word| **word != flag)
            .nth(1)
            .ok_or(format!("--{option} is never given a color"))?;

        assert_eq!(given.len(), 6, "--{option} is given {given}");
        assert!(
            given.chars().all(|digit| digit.is_ascii_hexdigit()),
            "--{option} is given {given}. A color here is six digits and nothing else: the \
             keyboard is read against the wallpaper, and anything after them is an alpha."
        );
    }

    Ok(())
}
