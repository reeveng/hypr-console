//! The argv the keyboard is started with, in the shape the rest of the program
//! takes.
//!
//! `main.c` parses the command line and the `VIRTUAL_KEYBOARD_*` environment
//! variables into a struct of colours, dimensions, and flags. This is that, in
//! Rust, and it accepts both `-x` and `--x` forms the way the C does.
//!
//! One rule decides what is in `Config` and what is not: **this accepts every
//! flag the wire carries, and stores only what it draws.** `palette::argv`
//! builds one command line, and while the C was the way back there were two
//! programs that could be handed it, so a flag the C understood and the port
//! has nothing to do with is taken and dropped on the floor here rather than
//! refused. The C has gone and with it the second reader: what the wire still
//! carries beyond what the port draws is now habit, and can be narrowed.
//!
//! What that leaves out, and why, in one place: the port draws no popup over
//! the key being pressed, no highlight of its own, and no swipe trail, and it
//! has no debug rectangles or layout-printing modes. Those were wvkbd's, none
//! of them was ever ported, and a field nobody reads is a field the next
//! person to read this believes in.

use std::env;

/// What the keyboard is started with, after parsing the command line and the
/// `VIRTUAL_KEYBOARD_*` environment variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The colours the keyboard is told to wear.
    ///
    /// Two sets: `normal` and `sp` (for non-letter keys like Esc and Tab).
    /// Each set has the same fields. The default for every field is what the
    /// keyboard compiles with, which is somebody else's palette.
    pub schemes: [Scheme; 2],
    /// Pixels tall in portrait orientation.
    pub height: u32,
    /// Pixels tall in landscape orientation.
    pub landscape_height: u32,
    /// How much to round the corners of keys.
    pub rounding: u32,
    /// Fontconfig pattern, like `Noto Sans 16`.
    pub font: String,
    /// Start hidden and stay for the session; toggled with a signal.
    pub hidden: bool,
    /// Comma-separated list of layer names to show in portrait.
    pub layers: Vec<String>,
    /// Comma-separated list of layer names to show in landscape.
    pub landscape_layers: Vec<String>,
}

/// One colour set. Two of these make up the keyboard: one for letter keys,
/// one for non-letter keys (Esc, Tab, arrows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheme {
    /// The slab the keys lie on.
    pub bg: Colour,
    /// The face of a key at rest. `fg` and not `bg`: the scheme is named from
    /// the C, where `bg` is the slab and `fg` is the key lying on it.
    pub fg: Colour,
    /// The face of the key being pressed, and the ink on it.
    pub high: Colour,
    pub text_press: Colour,
    /// The face of the key the pad's selection is sitting on, and its ink.
    pub sel: Colour,
    pub text_sel: Colour,
    /// What is written on a key at rest.
    pub text: Colour,
}

/// A 32-bit BGRA colour, matching the C `drw.c` layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colour(pub [u8; 4]);

impl Colour {
    pub const fn from_hex(six: &str) -> Self {
        let bytes = six.as_bytes();
        let r = hex(bytes[0]) * 16 + hex(bytes[1]);
        let g = hex(bytes[2]) * 16 + hex(bytes[3]);
        let b = hex(bytes[4]) * 16 + hex(bytes[5]);
        Colour([b, g, r, 0xff])
    }
}

const fn hex(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            schemes: [Scheme::default(), Scheme::default()],
            height: 260,
            landscape_height: 260,
            // `DEFAULT_ROUNDING` in the C's `config.mobintl.h`. Nothing on the
            // command line sets it -- `palette::argv` does not pass `-R` --
            // so this default is the only thing that decides whether the keys
            // on this machine have corners, and 0 gave them square ones.
            rounding: 5,
            font: "Noto Sans 16".to_string(),
            hidden: false,
            layers: Vec::new(),
            landscape_layers: Vec::new(),
        }
    }
}

impl Default for Scheme {
    fn default() -> Self {
        Scheme {
            bg: Colour::from_hex("000000"),
            fg: Colour::from_hex("f0f0f0"),
            high: Colour::from_hex("ff2020"),
            text_press: Colour::from_hex("ffffff"),
            sel: Colour::from_hex("20ff20"),
            text_sel: Colour::from_hex("ffffff"),
            text: Colour::from_hex("f0f0f0"),
        }
    }
}

/// What argv parsing can refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A flag was given without anything after it. The C version prints
    /// usage and exits 1; this returns the flag so the caller can do the
    /// same.
    MissingValue(String),
    /// A flag the parser does not know.
    Unknown(String),
}

/// Build a `Config` from a slice of arguments, with the environment providing
/// the defaults for `layers`, `landscape_layers`, `height`, and
/// `landscape_height`.
///
/// `argv[0]` is the program name and is skipped, the way the C version does.
pub fn parse(argv: &[String], env: &impl Fn(&str) -> Option<String>) -> Result<Config, Error> {
    let mut config = Config::default();
    apply_env(&mut config, env);
    parse_args(&mut config, argv)?;
    Ok(config)
}

/// Pull the four things the C version reads from the environment onto the
/// config. Done before argv so any explicit flag wins.
fn apply_env(config: &mut Config, env: &impl Fn(&str) -> Option<String>) {
    if let Some(layers) = env("VIRTUAL_KEYBOARD_LAYERS") {
        config.layers = layers.split(',').map(str::to_string).collect();
    }

    if let Some(layers) = env("VIRTUAL_KEYBOARD_LANDSCAPE_LAYERS") {
        config.landscape_layers = layers.split(',').map(str::to_string).collect();
    }

    if let Some(h) = env("VIRTUAL_KEYBOARD_HEIGHT") {
        if let Ok(n) = h.parse() {
            config.height = n;
        }
    }

    if let Some(h) = env("VIRTUAL_KEYBOARD_LANDSCAPE_HEIGHT") {
        if let Ok(n) = h.parse() {
            config.landscape_height = n;
        }
    }
}

fn parse_args(config: &mut Config, argv: &[String]) -> Result<(), Error> {
    let mut i = 1; // skip argv[0]

    while i < argv.len() {
        let flag = argv[i].clone();
        i += 1;

        match flag.as_str() {
            "-v" | "--version" => return Err(Error::MissingValue("--version".into())),
            "-h" | "--help" => return Err(Error::MissingValue("--help".into())),
            "-hidden" | "--hidden" => config.hidden = true,
            // Taken and dropped. `palette::argv` puts `--no-popup` on the wire
            // for the C, which draws a magnified key above the one being
            // pressed; the port draws no popup, so there is nothing here to
            // turn off. It is not refused, because the same command line starts
            // the way back.
            "-no-popup" | "--no-popup" => {}
            "-list-layers" | "--list-layers" => {
                return Err(Error::MissingValue("--list-layers".into()))
            }
            _ => {
                let value = take_value(argv, &mut i, &flag)?;
                apply_flag(config, &flag, &value)?;
            }
        }
    }

    Ok(())
}

fn take_value(argv: &[String], i: &mut usize, flag: &str) -> Result<String, Error> {
    if *i >= argv.len() {
        return Err(Error::MissingValue(flag.to_string()));
    }

    let v = argv[*i].clone();
    *i += 1;
    Ok(v)
}

fn apply_flag(config: &mut Config, flag: &str, value: &str) -> Result<(), Error> {
    let short = flag.trim_start_matches('-');
    let long = short.trim_start_matches('-');
    // Strip the canonical name. The C version supports both `-x` and `--x`;
    // we accept both.
    let name = long.trim_start_matches('-');

    match name {
        "l" => config.layers = value.split(',').map(str::to_string).collect(),
        "landscape-layers" => {
            config.landscape_layers = value.split(',').map(str::to_string).collect()
        }
        "H" => config.height = parse_number(value, flag)?,
        "L" => {
            config.height = parse_number(value, flag)?;
            config.landscape_height = config.height;
        }
        "R" => config.rounding = parse_number(value, flag)?,
        "fn" => config.font = value.to_string(),
        _ => apply_colour(config, name, value)?,
    }

    Ok(())
}

fn parse_number(value: &str, flag: &str) -> Result<u32, Error> {
    value.parse().map_err(|_| Error::MissingValue(flag.to_string()))
}

fn apply_colour(config: &mut Config, name: &str, value: &str) -> Result<(), Error> {
    let slot = match name {
        "bg" => Some((0, Slot::Bg)),
        "fg" => Some((0, Slot::Fg)),
        "fg-sp" => Some((1, Slot::Fg)),
        "text" => Some((0, Slot::Text)),
        "text-sp" => Some((1, Slot::Text)),
        "press" => Some((0, Slot::High)),
        "press-sp" => Some((1, Slot::High)),
        "text-press" => Some((0, Slot::TextPress)),
        "text-press-sp" => Some((1, Slot::TextPress)),
        // Taken and not stored, for the reason at the top of this file: the
        // trail a finger leaves is the C's, `Poke::Moved` here is an empty arm,
        // and a colour kept for a thing nothing draws is a colour somebody
        // eventually spends an evening on.
        "swipe" | "swipe-sp" | "text-swipe" | "text-swipe-sp" => None,
        "sel" => Some((0, Slot::Sel)),
        "sel-sp" => Some((1, Slot::Sel)),
        "text-sel" => Some((0, Slot::TextSel)),
        "text-sel-sp" => Some((1, Slot::TextSel)),
        _ => return Err(Error::Unknown(name.to_string())),
    };
    // Parsed whether or not it is kept, so that six digits is six digits on
    // every option and a typo in one the port ignores is still a typo.
    let colour = parse_colour(value)?;

    if let Some((scheme, slot)) = slot {
        config.schemes[scheme].set(slot, colour);
    }

    Ok(())
}

enum Slot {
    Bg,
    Fg,
    High,
    Sel,
    Text,
    TextPress,
    TextSel,
}

impl Scheme {
    fn set(&mut self, slot: Slot, colour: Colour) {
        match slot {
            Slot::Bg => self.bg = colour,
            Slot::Fg => self.fg = colour,
            Slot::High => self.high = colour,
            Slot::Sel => self.sel = colour,
            Slot::Text => self.text = colour,
            Slot::TextPress => self.text_press = colour,
            Slot::TextSel => self.text_sel = colour,
        }
    }
}

/// Parse a six-hex-digit colour, the way the C version's `set_kbd_colors` does.
fn parse_colour(value: &str) -> Result<Colour, Error> {
    if value.len() != 6 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Unknown(value.to_string()));
    }

    Ok(Colour::from_hex(value))
}

/// Run argv parsing against the real environment. The thin wrapper most
/// callers want; tests use `parse` directly with a synthetic one.
pub fn from_env(argv: &[String]) -> Result<Config, Error> {
    parse(argv, &|name| match env::var(name) {
        Ok(said) => Some(said),
        Err(env::VarError::NotPresent) => None,
        // Somebody set it, to bytes that are not text. The default stands
        // either way, but this half is worth a word: an unset variable is
        // nobody asking, and this is somebody asking in a way nothing can
        // read.
        Err(env::VarError::NotUnicode(_)) => {
            eprintln!("{name} is set to something that is not text; going on without it");
            None
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_env(_: &str) -> Option<String> {
        None
    }

    fn with_env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    fn argv(flags: &[&str]) -> Vec<String> {
        flags.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn defaults_match_the_c_binarys_compiled_in_palette() {
        let config = parse(&argv(&["virtual-keyboard"]), &empty_env).expect("defaults");
        assert_eq!(config.schemes[0].bg, Colour::from_hex("000000"));
        assert_eq!(config.schemes[0].fg, Colour::from_hex("f0f0f0"));
        assert_eq!(config.height, 260);
    }

    /// A flag the port has nothing to do with is taken, not refused.
    ///
    /// `palette::argv` builds one command line and either binary can be handed
    /// it, so refusing the C's flags would make the way back a keyboard that
    /// will not start. The colour is still read as a colour: an option this
    /// draws nothing with is not an option where `ffbac` passes.
    #[test]
    fn a_flag_only_the_c_understands_is_taken_and_dropped() {
        let with = parse(
            &argv(&["virtual-keyboard", "--no-popup", "--swipe", "ffb5e2", "--text-swipe", "110b12"]),
            &empty_env,
        )
        .expect("the C's flags are taken");
        assert_eq!(with, parse(&argv(&["virtual-keyboard"]), &empty_env).expect("plain"));

        let err = parse(&argv(&["virtual-keyboard", "--swipe", "ffbac"]), &empty_env)
            .expect_err("five digits is still five digits");
        assert_eq!(err, Error::Unknown("ffbac".into()));
    }

    #[test]
    fn a_colour_flag_overrides_the_default() {
        let config = parse(&argv(&["virtual-keyboard", "--bg", "110b12"]), &empty_env).expect("bg");
        assert_eq!(config.schemes[0].bg, Colour::from_hex("110b12"));
        // A flag for scheme 0 does not touch scheme 1.
        assert_eq!(config.schemes[1].bg, Colour::from_hex("000000"));
    }

    #[test]
    fn the_sp_suffix_targets_the_non_letter_scheme() {
        let config = parse(&argv(&["virtual-keyboard", "--fg-sp", "382a38"]), &empty_env).expect("fg-sp");
        assert_eq!(config.schemes[1].fg, Colour::from_hex("382a38"));
        assert_eq!(config.schemes[0].fg, Colour::from_hex("f0f0f0"));
    }

    #[test]
    fn height_takes_the_landscape_value_too() {
        // `-L` is the landscape flag and is also the portrait height. The C
        // version sets both; we do too.
        let config = parse(&argv(&["virtual-keyboard", "-L", "300"]), &empty_env).expect("-L");
        assert_eq!(config.height, 300);
        assert_eq!(config.landscape_height, 300);
    }

    #[test]
    fn environment_layers_split_on_commas() {
        let config = parse(
            &argv(&["virtual-keyboard"]),
            &with_env(&[("VIRTUAL_KEYBOARD_LAYERS", "latin,thai,emoji")]),
        )
        .expect("env");
        assert_eq!(config.layers, vec!["latin", "thai", "emoji"]);
    }

    #[test]
    fn an_argv_layer_overrides_an_environment_one() {
        let config = parse(
            &argv(&["virtual-keyboard", "-l", "simple"]),
            &with_env(&[("VIRTUAL_KEYBOARD_LAYERS", "latin,thai")]),
        )
        .expect("override");
        assert_eq!(config.layers, vec!["simple"]);
    }

    #[test]
    fn missing_value_for_a_flag_refuses() {
        let err = parse(&argv(&["virtual-keyboard", "--bg"]), &empty_env).expect_err("no value");
        assert_eq!(err, Error::MissingValue("--bg".into()));
    }

    #[test]
    fn an_unknown_flag_refuses() {
        let err = parse(&argv(&["virtual-keyboard", "--nonsense"]), &empty_env).expect_err("nonsense");
        assert_eq!(err, Error::MissingValue("--nonsense".into()));
    }

    #[test]
    fn a_colour_with_too_few_digits_refuses() {
        let err = parse(&argv(&["virtual-keyboard", "--bg", "ff"]), &empty_env).expect_err("short");
        assert_eq!(err, Error::Unknown("ff".into()));
    }

    #[test]
    fn short_and_long_forms_are_equivalent() {
        let short = parse(&argv(&["virtual-keyboard", "-H", "400"]), &empty_env).expect("-H");
        let long = parse(&argv(&["virtual-keyboard", "--H", "400"]), &empty_env).expect("--H");
        // `-H` is one of the rare flags the C version does not give a `--h`
        // alias to, because that conflicts with `--help`. The double-dash form
        // works anyway because we trim leading dashes once.
        assert_eq!(short.height, long.height);
    }
}
