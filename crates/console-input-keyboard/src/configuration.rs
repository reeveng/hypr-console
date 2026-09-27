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

use console_core_iteration::Step;
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
    pub background: Color,
    pub foreground: Color,
    pub high: Color,
    pub text_press: Color,
    pub selected: Color,
    pub text_selected: Color,
    pub text: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub [u8; 4]);

impl Color {
    pub const fn from_hex(six: &str) -> Result<Self, Never> {
        let bytes = six.as_bytes();
        let Ok(red) = band(Digits { high: bytes[0], low: bytes[1] });
        let Ok(green) = band(Digits { high: bytes[2], low: bytes[3] });
        let Ok(blue) = band(Digits { high: bytes[4], low: bytes[5] });

        Ok(Color([blue, green, red, 0xff]))
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
        let Ok(background) = Color::from_hex("000000");
        let Ok(foreground) = Color::from_hex("f0f0f0");
        let Ok(high) = Color::from_hex("ff2020");
        let Ok(text_press) = Color::from_hex("ffffff");
        let Ok(selected) = Color::from_hex("20ff20");
        let Ok(text_selected) = Color::from_hex("ffffff");
        let Ok(text) = Color::from_hex("f0f0f0");

        Scheme { background, foreground, high, text_press, selected, text_selected, text }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    MissingValue(String),
    Unknown(String),
}

pub fn parse(arguments: &[String], environment: &impl Fn(&str) -> Option<String>) -> Result<Configuration, Error> {
    let mut configuration = Configuration::default();
    let Ok(()) = apply_environment(&mut configuration, environment);
    parse_args(&mut configuration, arguments)?;
    Ok(configuration)
}

fn apply_environment(configuration: &mut Configuration, environment: &impl Fn(&str) -> Option<String>) -> Result<(), Never> {
    match environment("VIRTUAL_KEYBOARD_LAYERS") {
        Some(layers) => configuration.layers = layers.split(',').map(str::to_string).collect(),
        None => {},
    }

    match environment("VIRTUAL_KEYBOARD_LANDSCAPE_LAYERS") {
        Some(layers) => {
            configuration.landscape_layers = layers.split(',').map(str::to_string).collect();
        }
        None => {},
    }

    match environment("VIRTUAL_KEYBOARD_HEIGHT").map(|height| height.parse()) {
        Some(Ok(height)) => configuration.height = height,
        None => {},
        Some(Err(_not_a_number)) => {},
    }

    match environment("VIRTUAL_KEYBOARD_LANDSCAPE_HEIGHT").map(|height| height.parse()) {
        Some(Ok(height)) => configuration.landscape_height = height,
        None => {},
        Some(Err(_not_a_number)) => {},
    }

    Ok(())
}

fn parse_args(configuration: &mut Configuration, arguments: &[String]) -> Result<(), Error> {
    let parsed = console_core_iteration::iterate((configuration, arguments.iter().skip(1)), |(configuration, mut words)| {
        Ok(match words.next() {
            None => Step::Halt(Ok(())),
            Some(flag) => match parse_flag(configuration, flag, &mut words) {
                Ok(()) => Step::Again((configuration, words)),
                Err(fault) => Step::Halt(Err(fault)),
            },
        })
    });

    match parsed {
        Ok(parsed) => parsed,
        Err(_endless) => Ok(()),
    }
}

fn parse_flag<'a>(
    configuration: &mut Configuration,
    flag: &str,
    words: &mut impl Iterator<Item = &'a String>,
) -> Result<(), Error> {
    match flag {
        "-v" | "--version" => Err(Error::MissingValue(String::from("--version"))),
        "-h" | "--help" => Err(Error::MissingValue(String::from("--help"))),
        "-hidden" | "--hidden" => {
            configuration.hidden = true;

            Ok(())
        }
        "-no-popup" | "--no-popup" => Ok(()),
        "-list-layers" | "--list-layers" => Err(Error::MissingValue(String::from("--list-layers"))),
        _ => match words.next() {
            Some(value) => apply_flag(configuration, flag, Value(value)),
            None => Err(Error::MissingValue(flag.to_string())),
        },
    }
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
        "bg" => Some((0_u8, Slot::Background)),
        "fg" => Some((0, Slot::Foreground)),
        "fg-sp" => Some((1, Slot::Foreground)),
        "text" => Some((0, Slot::Text)),
        "text-sp" => Some((1, Slot::Text)),
        "press" => Some((0, Slot::High)),
        "press-sp" => Some((1, Slot::High)),
        "text-press" => Some((0, Slot::TextPress)),
        "text-press-sp" => Some((1, Slot::TextPress)),
        "swipe" | "swipe-sp" | "text-swipe" | "text-swipe-sp" => None,
        "sel" => Some((0, Slot::Selected)),
        "sel-sp" => Some((1, Slot::Selected)),
        "text-sel" => Some((0, Slot::TextSelected)),
        "text-sel-sp" => Some((1, Slot::TextSelected)),
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
    Background,
    Foreground,
    High,
    Selected,
    Text,
    TextPress,
    TextSelected,
}

impl Scheme {
    fn set(&mut self, slot: Slot, color: Color) -> Result<(), Never> {
        match slot {
            Slot::Background => self.background = color,
            Slot::Foreground => self.foreground = color,
            Slot::High => self.high = color,
            Slot::Selected => self.selected = color,
            Slot::Text => self.text = color,
            Slot::TextPress => self.text_press = color,
            Slot::TextSelected => self.text_selected = color,
        }

        Ok(())
    }
}

fn parse_color(value: &str) -> Result<Color, Error> {
    match value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
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
pub fn from_environment(arguments: &[String]) -> Result<Configuration, Error> {
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

    const LAYERS: &str = "VIRTUAL_KEYBOARD_LAYERS";

    fn given_with(flags: &[&str], (name, value): (&str, Option<&str>)) -> Result<Configuration, Error> {
        let arguments: Vec<String> = flags.iter().map(|flag| flag.to_string()).collect();
        let env = |asked: &str| (asked == name).then(|| value.map(str::to_string)).flatten();

        parse(&arguments, &env)
    }

    fn given(flags: &[&str]) -> Result<Configuration, Error> {
        given_with(flags, (LAYERS, None))
    }

    #[test]
    fn defaults_match_the_c_binarys_compiled_in_palette() -> Result<(), Error> {
        let configuration = given(&["console-keyboard"])?;
        let [letters, _] = configuration.schemes;

        assert_eq!(Ok(letters.background), Color::from_hex("000000"));
        assert_eq!(Ok(letters.foreground), Color::from_hex("f0f0f0"));
        assert_eq!(configuration.height, 260);

        Ok(())
    }

    #[test]
    fn a_flag_only_the_c_understands_is_taken_and_dropped() -> Result<(), Error> {
        let with = given(&["console-keyboard", "--no-popup", "--swipe", "ffb5e2", "--text-swipe", "110b12"])?;

        assert_eq!(Ok(with), given(&["console-keyboard"]));
        assert_eq!(
            given(&["console-keyboard", "--swipe", "ffbac"]),
            Err(Error::Unknown(String::from("ffbac"))),
            "five digits is still five digits"
        );

        Ok(())
    }

    #[test]
    fn a_color_flag_overrides_the_default() -> Result<(), Error> {
        let configuration = given(&["console-keyboard", "--bg", "110b12"])?;
        let [letters, others] = configuration.schemes;

        assert_eq!(Ok(letters.background), Color::from_hex("110b12"));
        assert_eq!(Ok(others.background), Color::from_hex("000000"));

        Ok(())
    }

    #[test]
    fn the_sp_suffix_targets_the_non_letter_scheme() -> Result<(), Error> {
        let configuration = given(&["console-keyboard", "--fg-sp", "382a38"])?;
        let [letters, others] = configuration.schemes;

        assert_eq!(Ok(others.foreground), Color::from_hex("382a38"));
        assert_eq!(Ok(letters.foreground), Color::from_hex("f0f0f0"));

        Ok(())
    }

    #[test]
    fn height_takes_the_landscape_value_too() -> Result<(), Error> {
        let configuration = given(&["console-keyboard", "-L", "300"])?;

        assert_eq!(configuration.height, 300);
        assert_eq!(configuration.landscape_height, 300);

        Ok(())
    }

    #[test]
    fn environment_layers_split_on_commas() -> Result<(), Error> {
        let configuration = given_with(&["console-keyboard"], (LAYERS, Some("latin,thai,emoji")))?;

        assert_eq!(configuration.layers, ["latin", "thai", "emoji"]);

        Ok(())
    }

    #[test]
    fn an_argv_layer_overrides_an_environment_one() -> Result<(), Error> {
        let configuration = given_with(&["console-keyboard", "-l", "simple"], (LAYERS, Some("latin,thai")))?;

        assert_eq!(configuration.layers, ["simple"]);

        Ok(())
    }

    #[test]
    fn missing_value_for_a_flag_refuses() {
        assert_eq!(given(&["console-keyboard", "--bg"]), Err(Error::MissingValue(String::from("--bg"))));
    }

    #[test]
    fn an_unknown_flag_refuses() {
        assert_eq!(given(&["console-keyboard", "--nonsense"]), Err(Error::MissingValue(String::from("--nonsense"))));
    }

    #[test]
    fn a_color_with_too_few_digits_refuses() {
        assert_eq!(given(&["console-keyboard", "--bg", "ff"]), Err(Error::Unknown(String::from("ff"))));
    }

    #[test]
    fn short_and_long_forms_are_equivalent() -> Result<(), Error> {
        let short = given(&["console-keyboard", "-H", "400"])?;
        let long = given(&["console-keyboard", "--H", "400"])?;

        assert_eq!(short.height, long.height);

        Ok(())
    }
}
