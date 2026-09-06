//! The on-screen keyboard, and the colours it is started with.
//!
//! The keyboard takes its colours as arguments and has no configuration file, so
//! something has to turn the palette into a command line. This is that, and it
//! holds no colour of its own: every one of them is a name looked up in the
//! palette every other surface on this machine is themed from.
//!
//! It was a shell script, and the test that guarded it read the script with
//! regular expressions to find out which colour went to which option. That
//! test is the reason this file is worth having: the fault it exists for has
//! happened three times, always the same way -- two things on the keyboard
//! given the same colour, so one of them has nothing under it and the whole
//! keyboard reads as something you can see through. Asking the command line
//! about that is now asking a function rather than a regex.

use console_never::Never;
use std::collections::BTreeMap;

pub const NAME: &str = "virtual-keyboard";

pub const VIRTUAL_KEYBOARD: &str = "/usr/local/bin/virtual-keyboard";

pub const HEIGHT: u32 = 260;
pub const FONT: &str = "Noto Sans 16";

pub const COLOURS: [(&str, &str); 17] = [
    ("bg", "night"),
    ("fg", "panel"),
    ("fg-sp", "ground"),
    ("text", "text"),
    ("text-sp", "soft"),
    ("sel", "pink"),
    ("sel-sp", "pink"),
    ("text-sel", "night"),
    ("text-sel-sp", "night"),
    ("press", "mauve"),
    ("press-sp", "mauve"),
    ("text-press", "night"),
    ("text-press-sp", "night"),
    ("swipe", "pink"),
    ("swipe-sp", "pink"),
    ("text-swipe", "night"),
    ("text-swipe-sp", "night"),
];

pub const INK: [(&str, Option<&str>); 9] = [
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

pub const BACKGROUNDS: [&str; 5] = ["bg", "fg", "fg-sp", "press", "sel"];

pub fn role(option: &str) -> Result<Option<&'static str>, Never> {
    Ok(COLOURS.iter().find(|(named, _)| *named == option).map(|(_, role)| *role))
}

pub fn missing(palette: &BTreeMap<String, String>) -> Result<Vec<&'static str>, Never> {
    let mut wanted: Vec<&'static str> =
        COLOURS.iter().map(|(_, role)| *role).filter(|role| !palette.contains_key(*role)).collect();
    wanted.sort_unstable();
    wanted.dedup();
    Ok(wanted)
}

pub fn argv(palette: &BTreeMap<String, String>, rest: &[String]) -> Result<Vec<String>, Never> {
    let mut argv = vec![
        VIRTUAL_KEYBOARD.to_string(),
        "--hidden".to_string(),
        "--no-popup".to_string(),
        "-L".to_string(),
        HEIGHT.to_string(),
        "-H".to_string(),
        HEIGHT.to_string(),
        "--fn".to_string(),
        FONT.to_string(),
    ];

    for (option, role) in COLOURS {
        let Some(colour) = palette.get(role) else { continue };

        argv.push(format!("--{option}"));
        argv.push(colour.clone());
    }

    argv.extend(rest.iter().cloned());
    Ok(argv)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> BTreeMap<String, String> {
        [
            ("night", "110b12"),
            ("panel", "241a24"),
            ("ground", "382a38"),
            ("text", "ebdce7"),
            ("soft", "b79fb2"),
            ("pink", "ffb5e2"),
            ("mauve", "dbc2ff"),
        ]
        .iter()
        .map(|(name, colour)| ((*name).to_string(), (*colour).to_string()))
        .collect()
    }

    fn argv(palette: &BTreeMap<String, String>, rest: &[String]) -> Vec<String> {
        let Ok(argv) = super::argv(palette, rest);

        argv
    }

    fn role(option: &str) -> Option<&'static str> {
        let Ok(role) = super::role(option);

        role
    }

    fn missing(palette: &BTreeMap<String, String>) -> Vec<&'static str> {
        let Ok(missing) = super::missing(palette);

        missing
    }

    #[test]
    fn the_installed_path_ends_in_the_name_everything_else_looks_for() {
        assert_eq!(VIRTUAL_KEYBOARD, format!("/usr/local/bin/{NAME}"));
    }

    #[test]
    fn every_option_is_handed_the_colour_it_is_for() {
        let argv = argv(&palette(), &[]);
        let at = argv.iter().position(|word| word == "--bg").expect("--bg");
        assert_eq!(argv.get(at + 1).map(String::as_str), Some("110b12"));
    }

    #[test]
    fn every_colour_is_six_digits_and_nothing_else() {
        let argv = argv(&palette(), &[]);
        for (option, _) in COLOURS {
            let at = argv.iter().position(|word| *word == format!("--{option}")).expect(option);
            let given = argv.get(at + 1).map(String::as_str).unwrap_or_default();
            assert_eq!(given.len(), 6, "--{option} is given {given}");
            assert!(given.chars().all(|l| l.is_ascii_hexdigit()), "--{option} is given {given}");
        }
    }

    #[test]
    fn no_two_backgrounds_are_the_same_colour() {
        let palette = palette();
        let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
        for option in BACKGROUNDS {
            let colour = palette.get(role(option).expect(option)).expect("a colour");
            if let Some(other) = seen.get(colour.as_str()) {
                panic!("--{option} and --{other} are both #{colour}, so one is invisible on the other");
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

    #[test]
    fn every_background_is_named_at_all() {
        for option in BACKGROUNDS {
            assert!(role(option).is_some(), "the keyboard is never told what colour --{option} is");
        }
    }

    #[test]
    fn a_colour_the_palette_does_not_have_is_named() {
        let mut palette = palette();
        palette.remove("mauve");
        assert_eq!(missing(&palette), ["mauve"]);
        assert!(missing(&self::palette()).is_empty());
    }

    #[test]
    fn what_it_was_given_is_handed_on_after_the_colours() {
        let argv = argv(&palette(), &["-l".to_string(), "simple".to_string()]);
        assert_eq!(&argv[argv.len() - 2..], ["-l", "simple"]);
    }
}
