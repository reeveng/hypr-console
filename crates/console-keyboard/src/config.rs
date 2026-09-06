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

use console_never::Never;
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub schemes: [Scheme; 2],
    pub height: u32,
    pub landscape_height: u32,
    pub rounding: u32,
    pub font: String,
    pub hidden: bool,
    pub layers: Vec<String>,
    pub landscape_layers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheme {
    pub bg: Colour,
    pub fg: Colour,
    pub high: Colour,
    pub text_press: Colour,
    pub sel: Colour,
    pub text_sel: Colour,
    pub text: Colour,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colour(pub [u8; 4]);

impl Colour {
    pub const fn from_hex(six: &str) -> Result<Self, Never> {
        let bytes = six.as_bytes();
        let Ok(r) = band(bytes[0], bytes[1]);
        let Ok(g) = band(bytes[2], bytes[3]);
        let Ok(b) = band(bytes[4], bytes[5]);

        Ok(Colour([b, g, r, 0xff]))
    }
}

const fn band(high: u8, low: u8) -> Result<u8, Never> {
    let Ok(high) = hex(high);
    let Ok(low) = hex(low);

    Ok(high * 16 + low)
}

const fn hex(byte: u8) -> Result<u8, Never> {
    Ok(match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    })
}

impl Default for Config {
    fn default() -> Self {
        Config {
            schemes: [Scheme::default(), Scheme::default()],
            height: 260,
            landscape_height: 260,
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
        let Ok(bg) = Colour::from_hex("000000");
        let Ok(fg) = Colour::from_hex("f0f0f0");
        let Ok(high) = Colour::from_hex("ff2020");
        let Ok(text_press) = Colour::from_hex("ffffff");
        let Ok(sel) = Colour::from_hex("20ff20");
        let Ok(text_sel) = Colour::from_hex("ffffff");
        let Ok(text) = Colour::from_hex("f0f0f0");

        Scheme { bg, fg, high, text_press, sel, text_sel, text }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    MissingValue(String),
    Unknown(String),
}

pub fn parse(argv: &[String], env: &impl Fn(&str) -> Option<String>) -> Result<Config, Error> {
    let mut config = Config::default();
    let Ok(()) = apply_env(&mut config, env);
    parse_args(&mut config, argv)?;
    Ok(config)
}

fn apply_env(config: &mut Config, env: &impl Fn(&str) -> Option<String>) -> Result<(), Never> {
    match env("VIRTUAL_KEYBOARD_LAYERS") {
        Some(layers) => config.layers = layers.split(',').map(str::to_string).collect(),
        None => {},
    }

    match env("VIRTUAL_KEYBOARD_LANDSCAPE_LAYERS") {
        Some(layers) => {
            config.landscape_layers = layers.split(',').map(str::to_string).collect();
        }
        None => {},
    }

    match env("VIRTUAL_KEYBOARD_HEIGHT").map(|h| h.parse()) {
        Some(Ok(n)) => config.height = n,
        Some(Err(_)) | None => {},
    }

    match env("VIRTUAL_KEYBOARD_LANDSCAPE_HEIGHT").map(|h| h.parse()) {
        Some(Ok(n)) => config.landscape_height = n,
        Some(Err(_)) | None => {},
    }

    Ok(())
}

fn parse_args(config: &mut Config, argv: &[String]) -> Result<(), Error> {
    let mut i = 1;

    while i < argv.len() {
        let Some(flag) = argv.get(i).cloned() else { break };

        i = i.saturating_add(1);

        match flag.as_str() {
            "-v" | "--version" => return Err(Error::MissingValue("--version".into())),
            "-h" | "--help" => return Err(Error::MissingValue("--help".into())),
            "-hidden" | "--hidden" => config.hidden = true,
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
    match *i >= argv.len() {
        true => return Err(Error::MissingValue(flag.to_string())),
        false => {},
    }

    let Some(value) = argv.get(*i).cloned() else {
        return Err(Error::MissingValue(flag.to_string()));
    };

    *i = i.saturating_add(1);
    Ok(value)
}

fn apply_flag(config: &mut Config, flag: &str, value: &str) -> Result<(), Error> {
    let short = flag.trim_start_matches('-');
    let long = short.trim_start_matches('-');
    let name = long.trim_start_matches('-');

    match name {
        "l" => config.layers = value.split(',').map(str::to_string).collect(),
        "landscape-layers" => {
            config.landscape_layers = value.split(',').map(str::to_string).collect()
        }
        "H" => {
            let height = parse_number(value, flag)?;

            config.height = height;
        }
        "L" => {
            let height = parse_number(value, flag)?;

            config.height = height;
            config.landscape_height = config.height;
        }
        "R" => {
            let rounding = parse_number(value, flag)?;

            config.rounding = rounding;
        }
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
        "swipe" | "swipe-sp" | "text-swipe" | "text-swipe-sp" => None,
        "sel" => Some((0, Slot::Sel)),
        "sel-sp" => Some((1, Slot::Sel)),
        "text-sel" => Some((0, Slot::TextSel)),
        "text-sel-sp" => Some((1, Slot::TextSel)),
        _ => return Err(Error::Unknown(name.to_string())),
    };
    let colour = parse_colour(value)?;

    match slot {
        Some((scheme, slot)) => match config.schemes.get_mut(scheme) {
            Some(scheme) => {
                let Ok(()) = scheme.set(slot, colour);
            },
            None => {},
        },
        None => {},
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
    fn set(&mut self, slot: Slot, colour: Colour) -> Result<(), Never> {
        match slot {
            Slot::Bg => self.bg = colour,
            Slot::Fg => self.fg = colour,
            Slot::High => self.high = colour,
            Slot::Sel => self.sel = colour,
            Slot::Text => self.text = colour,
            Slot::TextPress => self.text_press = colour,
            Slot::TextSel => self.text_sel = colour,
        }

        Ok(())
    }
}

fn parse_colour(value: &str) -> Result<Colour, Error> {
    match value.len() != 6 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        true => return Err(Error::Unknown(value.to_string())),
        false => {},
    }

    let Ok(colour) = Colour::from_hex(value);

    Ok(colour)
}

pub fn from_env(argv: &[String]) -> Result<Config, Error> {
    parse(argv, &|name| match env::var(name) {
        Ok(said) => Some(said),
        Err(env::VarError::NotPresent) => None,
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
        assert_eq!(Ok(config.schemes[0].bg), Colour::from_hex("000000"));
        assert_eq!(Ok(config.schemes[0].fg), Colour::from_hex("f0f0f0"));
        assert_eq!(config.height, 260);
    }

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
        assert_eq!(Ok(config.schemes[0].bg), Colour::from_hex("110b12"));
        assert_eq!(Ok(config.schemes[1].bg), Colour::from_hex("000000"));
    }

    #[test]
    fn the_sp_suffix_targets_the_non_letter_scheme() {
        let config = parse(&argv(&["virtual-keyboard", "--fg-sp", "382a38"]), &empty_env).expect("fg-sp");
        assert_eq!(Ok(config.schemes[1].fg), Colour::from_hex("382a38"));
        assert_eq!(Ok(config.schemes[0].fg), Colour::from_hex("f0f0f0"));
    }

    #[test]
    fn height_takes_the_landscape_value_too() {
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
        assert_eq!(short.height, long.height);
    }
}
