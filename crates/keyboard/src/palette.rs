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

use std::collections::BTreeMap;

/// The keyboard binary, built from this workspace.
pub const VIRTUAL_KEYBOARD: &str = "/usr/local/bin/virtual-keyboard";

/// How tall it is, and what it is written in. Not colours.
pub const HEIGHT: u32 = 260;
pub const FONT: &str = "Noto Sans 16";

/// Which colour of ours each option is given.
///
/// `-sp` is the word for a key that is not a letter: Esc, Tab, the arrows,
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
    // The key the stick is sitting on, which is `pink` because that is what a
    // highlighted thing is everywhere else on this machine: the workspace you
    // are on in the bar, the row under the thumb in a menu, and now the key
    // under the stick. `theme/palette.toml` has said so all along -- `pink` is
    // spent as "the workspace you are on, the highlighted row, the key under
    // your thumb" -- and the keyboard was the one surface not spending it that
    // way. It was `rose`, which is a hue away from `pink` and the same
    // lightness, so crossing the keyboard looked like nothing in particular.
    ("sel", "pink"),
    ("sel-sp", "pink"),
    ("text-sel", "night"),
    ("text-sel-sp", "night"),
    // A key with a thumb on it. `mauve` for the same reason `sel` is `pink`:
    // the palette already spends it on "a key held down", and this is the key
    // held down. It has to be a colour of its own -- given the slab's colour a
    // key vanishes at the moment it is pressed, and a key that disappears when
    // pressed reads as a key that is not there -- and it has to be a colour
    // well away from `sel`, which is `a_pressed_key_is_not_the_key_under_the_
    // stick`. Pressed and selected are the two states one key can be in a
    // fifth of a second apart, and they were the two nearest colours here.
    ("press", "mauve"),
    ("press-sp", "mauve"),
    ("text-press", "night"),
    ("text-press-sp", "night"),
    // The trail a finger leaves behind it. Nothing here draws one: `Poke::Moved`
    // in the keyboard is an empty arm, and `config` takes these four and drops
    // them. They stayed on the wire for the C, which did draw a trail and would
    // have drawn it in somebody else's magenta without them. The C has gone, so
    // nothing reads these four any more and they are the next thing to take out.
    ("swipe", "pink"),
    ("swipe-sp", "pink"),
    ("text-swipe", "night"),
    ("text-swipe-sp", "night"),
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
        VIRTUAL_KEYBOARD.to_string(),
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
            ("pink", "ffb5e2"),
            ("mauve", "dbc2ff"),
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

    /// The keyboard reads a colour as `rrggbb` or `rrggbbaa`, and six digits leaves
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

    /// An option not passed leaves the keyboard on the colour it was compiled
    /// with, which is somebody else's palette: a red pressed key and a green
    /// selected one, neither of which is anything on this machine.
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
        palette.remove("mauve");
        assert_eq!(missing(&palette), ["mauve"]);
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
