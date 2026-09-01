//! The on-screen keyboard, and the colours it is started with.
//!
//! wvkbd takes its colours as arguments and has no configuration file, so
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

use std::collections::BTreeMap;

/// The keyboard itself. A fork, so the built program is carried in the
/// manifest rather than installed from a package.
pub const WVKBD: &str = "/usr/local/bin/wvkbd-mobintl";

/// How tall it is, and what it is written in. Not colours.
pub const HEIGHT: u32 = 260;
pub const FONT: &str = "Noto Sans 16";

/// Which colour of ours each of wvkbd's options is given.
///
/// `-sp` is wvkbd's word for a key that is not a letter: Esc, Tab, the arrows,
/// Enter. It is a background of its own because it has to be: given the slab's
/// colour, those keys have nothing under them and read as letters lying on the
/// desktop.
pub const COLOURS: [(&str, &str); 17] = [
    // The three that are behind things, darkest first.
    ("bg", "night"),
    ("fg", "panel"),
    ("fg-sp", "ground"),
    // What is written on them.
    ("text", "text"),
    ("text-sp", "soft"),
    // A key with a thumb on it, which is pink with dark ink, the same as a row
    // with a thumb on it everywhere else here. It has to be a colour of its
    // own: given the slab's colour it vanishes into the slab, and a key that
    // disappears when pressed reads as a key that is not there.
    ("press", "pink"),
    ("press-sp", "pink"),
    ("text-press", "night"),
    ("text-press-sp", "night"),
    // The trail a swipe leaves behind.
    ("swipe", "pink"),
    ("swipe-sp", "pink"),
    ("text-swipe", "night"),
    ("text-swipe-sp", "night"),
    // The key the stick is sitting on. Painted in the swipe colour once, which
    // is a wash by design, and the wallpaper came through the letter.
    ("sel", "rose"),
    ("sel-sp", "rose"),
    ("text-sel", "night"),
    ("text-sel-sp", "night"),
];

/// What is behind something, and the ink written on it.
///
/// The slab has no ink of its own: nothing is written on the space between
/// keys.
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

/// The five that are a colour behind something rather than a colour written on
/// it. No two of them may be the same.
pub const BACKGROUNDS: [&str; 5] = ["bg", "fg", "fg-sp", "press", "sel"];

/// Which of our colours an option is given, by the option's name.
pub fn role(option: &str) -> Option<&'static str> {
    COLOURS.iter().find(|(named, _)| *named == option).map(|(_, role)| *role)
}

/// Any colour this asks for that the palette does not have.
///
/// Named rather than counted, because a keyboard started without one of these
/// keeps whatever colour it was compiled with, and that is somebody else's
/// palette.
pub fn missing(palette: &BTreeMap<String, String>) -> Vec<&'static str> {
    let mut wanted: Vec<&'static str> =
        COLOURS.iter().map(|(_, role)| *role).filter(|role| !palette.contains_key(*role)).collect();
    wanted.sort_unstable();
    wanted.dedup();
    wanted
}

/// The whole command line, worked out and not run.
///
/// Six hex digits and no hash for every colour. A hash would be a comment in
/// the file they are written in, and the seventh and eighth digits would be an
/// alpha: a key you can see the wallpaper through is a key you cannot read.
///
/// `rest` is whatever the caller was given, handed on, which is how a layout
/// can be asked for on the command line.
pub fn argv(palette: &BTreeMap<String, String>, rest: &[String]) -> Vec<String> {
    let mut argv = vec![
        WVKBD.to_string(),
        // Started hidden and left running for the session: it is toggled with
        // a signal rather than started and stopped, so it owns the answer to
        // whether it is up.
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
    argv
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
            ("pink", "ffbac5"),
            ("rose", "ff8fb0"),
        ]
        .iter()
        .map(|(name, colour)| ((*name).to_string(), (*colour).to_string()))
        .collect()
    }

    #[test]
    fn every_option_is_handed_the_colour_it_is_for() {
        let argv = argv(&palette(), &[]);
        let at = argv.iter().position(|word| word == "--bg").expect("--bg");
        assert_eq!(argv[at + 1], "110b12");
    }

    /// wvkbd reads a colour as `rrggbb` or `rrggbbaa`, and six digits leaves
    /// the alpha where its own defaults put it. Everything after the sixth
    /// digit is a key you can see the wallpaper through.
    #[test]
    fn every_colour_is_six_digits_and_nothing_else() {
        let argv = argv(&palette(), &[]);
        for (option, _) in COLOURS {
            let at = argv.iter().position(|word| *word == format!("--{option}")).expect(option);
            let given = &argv[at + 1];
            assert_eq!(given.len(), 6, "--{option} is given {given}");
            assert!(given.chars().all(|l| l.is_ascii_hexdigit()), "--{option} is given {given}");
        }
    }

    /// A key the colour of the slab is a key with nothing under it. This has
    /// happened three times.
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

    /// An option not passed leaves wvkbd on the colour it was compiled with,
    /// which is somebody else's palette and, for the selected key, a wash.
    #[test]
    fn every_background_is_named_at_all() {
        for option in BACKGROUNDS {
            assert!(role(option).is_some(), "the keyboard is never told what colour --{option} is");
        }
    }

    /// A palette missing a colour is said out loud rather than quietly leaving
    /// an option off.
    #[test]
    fn a_colour_the_palette_does_not_have_is_named() {
        let mut palette = palette();
        palette.remove("rose");
        assert_eq!(missing(&palette), ["rose"]);
        assert!(missing(&self::palette()).is_empty());
    }

    /// Whatever the caller was given is handed on, so a layout can be asked
    /// for on the command line.
    #[test]
    fn what_it_was_given_is_handed_on_after_the_colours() {
        let argv = argv(&palette(), &["-l".to_string(), "simple".to_string()]);
        assert_eq!(&argv[argv.len() - 2..], ["-l", "simple"]);
    }
}
