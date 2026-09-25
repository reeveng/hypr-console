//! The sixteen colors a program may ask for by number.

use indexmap::IndexMap;
use console_core_color as color;
use console_core_never::Never;
use console_core_words::Words;

use crate::palette::Palette;
use crate::configuration::Configuration;

pub const SLOTS: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

#[derive(Debug, Clone)]
pub struct Terminal {
    pub background: String,
    pub foreground: String,
    pub cursor: String,
    pub selection: String,
    normal: IndexMap<String, String>,
    bright: IndexMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Shade {
    #[words(name = "normal")]
    Normal,
    #[words(name = "bright")]
    Bright,
}

impl Terminal {
    pub fn of(configuration: &Configuration, palette: &Palette) -> Result<Self, color::Short> {
        let setting = &configuration.terminal;
        let normal: IndexMap<String, String> = setting
            .normal
            .iter()
            .map(|(slot, name)| {
                let color = palette.must(name)?;

                Ok((slot.clone(), color.to_owned()))
            })
            .collect::<Result<_, color::Short>>()?;
        let bright: IndexMap<String, String> = normal
            .iter()
            .map(|(slot, code)| match slot.as_str() {
                "white" => {
                    let text = palette.must("text")?;

                    Ok((slot.clone(), text.to_owned()))
                }
                _ => {
                    let Ok(lifted) = color::lift(code, setting.bright_lift);

                    Ok((slot.clone(), lifted))
                }
            })
            .collect::<Result<_, color::Short>>()?;

        for slot in SLOTS {
            match (normal.get(slot), bright.get(slot)) {
                (Some(_), Some(_)) => {},
                (Some(_) | None, _) => {
                    return Err(color::Short(format!("the terminal table names no {slot}")));
                }
            }
        }

        let background = palette.must(&setting.background)?;
        let foreground = palette.must(&setting.foreground)?;
        let cursor = palette.must(&setting.cursor)?;
        let selection = palette.must(&setting.selection)?;

        Ok(Terminal {
            background: background.to_owned(),
            foreground: foreground.to_owned(),
            cursor: cursor.to_owned(),
            selection: selection.to_owned(),
            normal,
            bright,
        })
    }

    pub fn slot(&self, shade: Shade, name: &str) -> Result<&str, Never> {
        let held = match shade {
            Shade::Normal => self.normal.get(name),
            Shade::Bright => self.bright.get(name),
        };

        Ok(match held {
            Some(code) => code,
            None => &self.background,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configuration::Configuration;
    use console_core_color::{Ground, HexColor};

    const PALETTE: &str = include_str!("../../../theme/palette.toml");

    fn spent() -> (Configuration, Palette, Terminal) {
        let configuration: Configuration = toml::from_str(PALETTE).expect("the palette parses");
        let palette = crate::palette::resolve(&configuration.color).expect("it resolves");
        let terminal = Terminal::of(&configuration, &palette).expect("the terminal table is declared");
        (configuration, palette, terminal)
    }

    #[test]
    fn bright_white_is_the_ink_and_not_a_lift_of_black() {
        let (_, palette, terminal) = spent();
        assert_eq!(
            terminal.slot(Shade::Bright, "white"),
            Ok(palette.must("text").expect("a declared color"))
        );
    }

    #[test]
    fn every_bright_is_lighter_than_its_normal() {
        let (_, _, terminal) = spent();
        for slot in SLOTS {
            let Ok(normal) = terminal.slot(Shade::Normal, slot);

            let Ok(bright) = terminal.slot(Shade::Bright, slot);

            let Ok(lighter) = color::luminance(bright);
            let Ok(darker) = color::luminance(normal);

            assert!(
                lighter > darker,
                "bright {slot} ({bright}) is no lighter than normal ({normal})"
            );
        }
    }

    #[test]
    fn every_slot_can_be_read_on_the_background() {
        let (_, _, terminal) = spent();
        for shade in [Shade::Normal, Shade::Bright] {
            for slot in SLOTS {
                let Ok(code) = terminal.slot(shade, slot);

                let Ok(got) = color::contrast(HexColor(code), Ground(&terminal.background));

                let least = match slot {
                    "black" => 4.5,
                    _ => 7.0,
                };

                let Ok(name) = shade.name();

                assert!(
                    got >= least,
                    "{name} {slot} reaches only {got:.2}:1 on the background"
                );
            }
        }
    }
}
