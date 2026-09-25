//! The arguments the keyboard is started with, in the shape the rest of the program
//! takes.
//!
//! `main.c` parses the command line and the `VIRTUAL_KEYBOARD_*` environment
//! variables into a struct of colors, dimensions, and flags. This is that, in
//! Rust, and it accepts both `-x` and `--x` forms the way the C does.
//!
//! One rule decides what is in `Configuration` and what is not: **this accepts every
//! flag the wire carries, and stores only what it draws.** `palette::arguments`
//! builds one command line, and while the C was the way back there were two
//! programs that could be handed it, so a flag the C understood and the port
//! has nothing to do with is taken and dropped on the floor here rather than
//! refused. The C has gone and with it the second reader: what the wire still
//! carries beyond what the port draws is now habit, and can be narrowed.
//!
//! What that leaves out, and why, in one place: the port draws no popup over
//! the key being pressed, no highlight of its own, and no swipe trail, and it
//! has no debug rectangles or layout-printing modes. Those were wvkbd's, none
//! of them was ever ported, and a field no one reads is a field the next
//! person to read this believes in.

use console_core_never::Never;
use console_core_number_conversion::index;
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuration {
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
    pub bg: Color,
    pub fg: Color,
    pub high: Color,
    pub text_press: Color,
    pub sel: Color,
    pub text_sel: Color,
    pub text: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub [u8; 4]);

impl Color {
    pub const fn from_hex(six: &str) -> Result<Self, Never> {
        let bytes = six.as_bytes();
        let Ok(r) = band(Digits { high: bytes[0], low: bytes[1] });
        let Ok(g) = band(Digits { high: bytes[2], low: bytes[3] });
        let Ok(b) = band(Digits { high: bytes[4], low: bytes[5] });

        Ok(Color([b, g, r, 0xff]))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Digits {
    high: u8,
    low: u8,
}

const fn band(digits: Digits) -> Result<u8, Never> {
    let Ok(high) = hex(digits.high);
    let Ok(low) = hex(digits.low);

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

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
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
        let Ok(bg) = Color::from_hex("000000");
        let Ok(fg) = Color::from_hex("f0f0f0");
        let Ok(high) = Color::from_hex("ff2020");
        let Ok(text_press) = Color::from_hex("ffffff");
        let Ok(sel) = Color::from_hex("20ff20");
        let Ok(text_sel) = Color::from_hex("ffffff");
        let Ok(text) = Color::from_hex("f0f0f0");

        Scheme { bg, fg, high, text_press, sel, text_sel, text }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    MissingValue(String),
    Unknown(String),
}

pub fn parse(arguments: &[String], env: &impl Fn(&str) -> Option<String>) -> Result<Configuration, Error> {
    let mut configuration = Configuration::default();
    let Ok(()) = apply_env(&mut configuration, env);
    parse_args(&mut configuration, arguments)?;
    Ok(configuration)
}

fn apply_env(configuration: &mut Configuration, env: &impl Fn(&str) -> Option<String>) -> Result<(), Never> {
    match env("VIRTUAL_KEYBOARD_LAYERS") {
        Some(layers) => configuration.layers = layers.split(',').map(str::to_string).collect(),
        None => {},
    }

    match env("VIRTUAL_KEYBOARD_LANDSCAPE_LAYERS") {
        Some(layers) => {
            configuration.landscape_layers = layers.split(',').map(str::to_string).collect();
        }
        None => {},
    }

    match env("VIRTUAL_KEYBOARD_HEIGHT").map(|h| h.parse()) {
        Some(Ok(n)) => configuration.height = n,
        Some(Err(_)) | None => {},
    }

    match env("VIRTUAL_KEYBOARD_LANDSCAPE_HEIGHT").map(|h| h.parse()) {
        Some(Ok(n)) => configuration.landscape_height = n,
        Some(Err(_)) | None => {},
    }

    Ok(())
}

fn parse_args(configuration: &mut Configuration, arguments: &[String]) -> Result<(), Error> {
    let mut i: u32 = 1;

    loop {
        let Ok(at) = index(i);

        let flag = match arguments.get(at).cloned() {
            Some(flag) => flag,
            None => break,
        };

        i = i.saturating_add(1);

        match flag.as_str() {
            "-v" | "--version" => return Err(Error::MissingValue(String::from("--version"))),
            "-h" | "--help" => return Err(Error::MissingValue(String::from("--help"))),
            "-hidden" | "--hidden" => configuration.hidden = true,
            "-no-popup" | "--no-popup" => {}
            "-list-layers" | "--list-layers" => {
                return Err(Error::MissingValue(String::from("--list-layers")))
            }
            _ => {
                let value = take_value(arguments, &mut i, &flag)?;
                apply_flag(configuration, &flag, Value(&value))?;
            }
        }
    }

    Ok(())
}

fn take_value(arguments: &[String], i: &mut u32, flag: &str) -> Result<String, Error> {
    let Ok(at) = index(*i);

    let value = match arguments.get(at).cloned() {
        Some(value) => value,
        None => return Err(Error::MissingValue(flag.to_string())),
    };

    *i = i.saturating_add(1);
    Ok(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Value<'a>(&'a str);

fn apply_flag(configuration: &mut Configuration, flag: &str, value: Value<'_>) -> Result<(), Error> {
    let short = flag.trim_start_matches('-');
    let long = short.trim_start_matches('-');
    let name = long.trim_start_matches('-');

    match name {
        "l" => configuration.layers = value.0.split(',').map(str::to_string).collect(),
        "landscape-layers" => {
            configuration.landscape_layers = value.0.split(',').map(str::to_string).collect()
        }
        "H" => {
            let height = parse_number(value, flag)?;

            configuration.height = height;
        }
        "L" => {
            let height = parse_number(value, flag)?;

            configuration.height = height;
            configuration.landscape_height = configuration.height;
        }
        "R" => {
            let rounding = parse_number(value, flag)?;

            configuration.rounding = rounding;
        }
        "fn" => configuration.font = value.0.to_string(),
        _ => apply_color(configuration, name, value)?,
    }

    Ok(())
}

fn parse_number(value: Value<'_>, flag: &str) -> Result<u32, Error> {
    value.0.parse().map_err(|_| Error::MissingValue(flag.to_string()))
}

fn apply_color(configuration: &mut Configuration, name: &str, value: Value<'_>) -> Result<(), Error> {
    let slot = match name {
        "bg" => Some((0_u8, Slot::Bg)),
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
    let color = parse_color(value.0)?;

    match slot {
        Some((scheme, slot)) => {
            let Ok(scheme) = index(scheme);

            match configuration.schemes.get_mut(scheme) {
                Some(scheme) => {
                    let Ok(()) = scheme.set(slot, color);
                },
                None => {},
            }
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
    fn set(&mut self, slot: Slot, color: Color) -> Result<(), Never> {
        match slot {
            Slot::Bg => self.bg = color,
            Slot::Fg => self.fg = color,
            Slot::High => self.high = color,
            Slot::Sel => self.sel = color,
            Slot::Text => self.text = color,
            Slot::TextPress => self.text_press = color,
            Slot::TextSel => self.text_sel = color,
        }

        Ok(())
    }
}

fn parse_color(value: &str) -> Result<Color, Error> {
    match value.len() != 6 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        true => return Err(Error::Unknown(value.to_string())),
        false => {},
    }

    let Ok(color) = Color::from_hex(value);

    Ok(color)
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "the keyboard's own settings, and `parse` is handed a reader rather than reaching for one so that a test can answer it without an environment"
    )
)]
pub fn from_env(arguments: &[String]) -> Result<Configuration, Error> {
    parse(arguments, &|name| match env::var(name) {
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

    fn arguments(flags: &[&str]) -> Vec<String> {
        flags.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn defaults_match_the_c_binarys_compiled_in_palette() {
        let configuration = parse(&arguments(&["console-keyboard"]), &empty_env).expect("defaults");
        assert_eq!(Ok(configuration.schemes[0].bg), Color::from_hex("000000"));
        assert_eq!(Ok(configuration.schemes[0].fg), Color::from_hex("f0f0f0"));
        assert_eq!(configuration.height, 260);
    }

    #[test]
    fn a_flag_only_the_c_understands_is_taken_and_dropped() {
        let with = parse(
            &arguments(&["console-keyboard", "--no-popup", "--swipe", "ffb5e2", "--text-swipe", "110b12"]),
            &empty_env,
        )
        .expect("the C's flags are taken");
        assert_eq!(with, parse(&arguments(&["console-keyboard"]), &empty_env).expect("plain"));

        let err = parse(&arguments(&["console-keyboard", "--swipe", "ffbac"]), &empty_env)
            .expect_err("five digits is still five digits");
        assert_eq!(err, Error::Unknown("ffbac".into()));
    }

    #[test]
    fn a_color_flag_overrides_the_default() {
        let configuration = parse(&arguments(&["console-keyboard", "--bg", "110b12"]), &empty_env).expect("bg");
        assert_eq!(Ok(configuration.schemes[0].bg), Color::from_hex("110b12"));
        assert_eq!(Ok(configuration.schemes[1].bg), Color::from_hex("000000"));
    }

    #[test]
    fn the_sp_suffix_targets_the_non_letter_scheme() {
        let configuration = parse(&arguments(&["console-keyboard", "--fg-sp", "382a38"]), &empty_env).expect("fg-sp");
        assert_eq!(Ok(configuration.schemes[1].fg), Color::from_hex("382a38"));
        assert_eq!(Ok(configuration.schemes[0].fg), Color::from_hex("f0f0f0"));
    }

    #[test]
    fn height_takes_the_landscape_value_too() {
        let configuration = parse(&arguments(&["console-keyboard", "-L", "300"]), &empty_env).expect("-L");
        assert_eq!(configuration.height, 300);
        assert_eq!(configuration.landscape_height, 300);
    }

    #[test]
    fn environment_layers_split_on_commas() {
        let configuration = parse(
            &arguments(&["console-keyboard"]),
            &with_env(&[("VIRTUAL_KEYBOARD_LAYERS", "latin,thai,emoji")]),
        )
        .expect("env");
        assert_eq!(configuration.layers, vec!["latin", "thai", "emoji"]);
    }

    #[test]
    fn an_argv_layer_overrides_an_environment_one() {
        let configuration = parse(
            &arguments(&["console-keyboard", "-l", "simple"]),
            &with_env(&[("VIRTUAL_KEYBOARD_LAYERS", "latin,thai")]),
        )
        .expect("override");
        assert_eq!(configuration.layers, vec!["simple"]);
    }

    #[test]
    fn missing_value_for_a_flag_refuses() {
        let err = parse(&arguments(&["console-keyboard", "--bg"]), &empty_env).expect_err("no value");
        assert_eq!(err, Error::MissingValue("--bg".into()));
    }

    #[test]
    fn an_unknown_flag_refuses() {
        let err = parse(&arguments(&["console-keyboard", "--nonsense"]), &empty_env).expect_err("nonsense");
        assert_eq!(err, Error::MissingValue("--nonsense".into()));
    }

    #[test]
    fn a_color_with_too_few_digits_refuses() {
        let err = parse(&arguments(&["console-keyboard", "--bg", "ff"]), &empty_env).expect_err("short");
        assert_eq!(err, Error::Unknown("ff".into()));
    }

    #[test]
    fn short_and_long_forms_are_equivalent() {
        let short = parse(&arguments(&["console-keyboard", "-H", "400"]), &empty_env).expect("-H");
        let long = parse(&arguments(&["console-keyboard", "--H", "400"]), &empty_env).expect("--H");
        assert_eq!(short.height, long.height);
    }
}
