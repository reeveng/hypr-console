//! The sixteen colours a program may ask for by number.

use indexmap::IndexMap;
use console_core_colour as col;
use console_core_never::Never;

use crate::palette::Palette;
use crate::spec::Spec;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shade {
    Normal,
    Bright,
}

impl Shade {
    pub fn name(self) -> Result<&'static str, Never> {
        Ok(match self {
            Shade::Normal => "normal",
            Shade::Bright => "bright",
        })
    }
}

impl Terminal {
    pub fn of(spec: &Spec, palette: &Palette) -> Result<Self, col::Short> {
        let setting = &spec.terminal;
        let normal: IndexMap<String, String> = setting
            .normal
            .iter()
            .map(|(slot, name)| {
                let colour = palette.must(name)?;

                Ok((slot.clone(), colour.to_owned()))
            })
            .collect::<Result<_, col::Short>>()?;
        let bright: IndexMap<String, String> = normal
            .iter()
            .map(|(slot, code)| match slot.as_str() {
                "white" => {
                    let text = palette.must("text")?;

                    Ok((slot.clone(), text.to_owned()))
                }
                _ => {
                    let Ok(lifted) = col::lift(code, setting.bright_lift);

                    Ok((slot.clone(), lifted))
                }
            })
            .collect::<Result<_, col::Short>>()?;

        for slot in SLOTS {
            match (normal.get(slot), bright.get(slot)) {
                (Some(_), Some(_)) => {},
                (Some(_) | None, _) => {
                    return Err(col::Short(format!("the terminal table names no {slot}")));
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
    use crate::spec::Spec;

    const PALETTE: &str = include_str!("../../../theme/palette.toml");

    fn spent() -> (Spec, Palette, Terminal) {
        let spec: Spec = toml::from_str(PALETTE).expect("the palette parses");
        let palette = crate::palette::resolve(&spec.colour).expect("it resolves");
        let terminal = Terminal::of(&spec, &palette).expect("the terminal table is declared");
        (spec, palette, terminal)
    }

    #[test]
    fn bright_white_is_the_ink_and_not_a_lift_of_black() {
        let (_, palette, terminal) = spent();
        assert_eq!(
            terminal.slot(Shade::Bright, "white"),
            Ok(palette.must("text").expect("a declared colour"))
        );
    }

    #[test]
    fn every_bright_is_lighter_than_its_normal() {
        let (_, _, terminal) = spent();
        for slot in SLOTS {
            let Ok(normal) = terminal.slot(Shade::Normal, slot);

            let Ok(bright) = terminal.slot(Shade::Bright, slot);

            let Ok(lighter) = col::luminance(bright);
            let Ok(darker) = col::luminance(normal);

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

                let Ok(got) = col::contrast(code, &terminal.background);

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
