//! The sixteen colours a program may ask for by number.

use indexmap::IndexMap;
use legion_colour as col;

use crate::palette::Palette;
use crate::spec::Spec;

/// The order a terminal's eight are always written in.
pub const SLOTS: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

/// The sixteen, and what surrounds them.
#[derive(Debug, Clone)]
pub struct Terminal {
    pub background: String,
    pub foreground: String,
    pub cursor: String,
    pub selection: String,
    normal: IndexMap<String, String>,
    bright: IndexMap<String, String>,
}

/// Which half of the sixteen a slot is being asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shade {
    Normal,
    Bright,
}

impl Shade {
    pub fn name(self) -> &'static str {
        match self {
            Shade::Normal => "normal",
            Shade::Bright => "bright",
        }
    }
}

impl Terminal {
    /// The palette's terminal table, spent.
    ///
    /// The bright half is the same colour lifted, which keeps a bold line
    /// legible without turning it into a different colour. White is the
    /// exception: bright white is the ink everything else is read in, so it is
    /// taken rather than derived.
    pub fn of(spec: &Spec, palette: &Palette) -> Self {
        let setting = &spec.terminal;
        let normal: IndexMap<String, String> = setting
            .normal
            .iter()
            .map(|(slot, name)| (slot.clone(), palette[name.as_str()].to_owned()))
            .collect();
        let bright = normal
            .iter()
            .map(|(slot, code)| match slot.as_str() {
                "white" => (slot.clone(), palette["text"].to_owned()),
                _ => (slot.clone(), col::lift(code, setting.bright_lift)),
            })
            .collect();
        Terminal {
            background: palette[setting.background.as_str()].to_owned(),
            foreground: palette[setting.foreground.as_str()].to_owned(),
            cursor: palette[setting.cursor.as_str()].to_owned(),
            selection: palette[setting.selection.as_str()].to_owned(),
            normal,
            bright,
        }
    }

    pub fn slot(&self, shade: Shade, name: &str) -> &str {
        match shade {
            Shade::Normal => &self.normal[name],
            Shade::Bright => &self.bright[name],
        }
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
        let terminal = Terminal::of(&spec, &palette);
        (spec, palette, terminal)
    }

    #[test]
    fn bright_white_is_the_ink_and_not_a_lift_of_black() {
        let (_, palette, terminal) = spent();
        assert_eq!(terminal.slot(Shade::Bright, "white"), &palette["text"]);
    }

    #[test]
    fn every_bright_is_lighter_than_its_normal() {
        let (_, _, terminal) = spent();
        for slot in SLOTS {
            let (normal, bright) = (
                terminal.slot(Shade::Normal, slot),
                terminal.slot(Shade::Bright, slot),
            );
            assert!(
                col::luminance(bright) > col::luminance(normal),
                "bright {slot} ({bright}) is no lighter than normal ({normal})"
            );
        }
    }

    #[test]
    fn every_slot_can_be_read_on_the_background() {
        // The one thing a terminal palette is for. `black` is AA on purpose
        // and says so in palette.toml; everything else clears AAA.
        let (_, _, terminal) = spent();
        for shade in [Shade::Normal, Shade::Bright] {
            for slot in SLOTS {
                let got = col::contrast(terminal.slot(shade, slot), &terminal.background);
                let least = match slot {
                    "black" => 4.5,
                    _ => 7.0,
                };
                assert!(
                    got >= least,
                    "{} {slot} reaches only {got:.2}:1 on the background",
                    shade.name()
                );
            }
        }
    }
}
