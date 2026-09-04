//! The terminal's colours as a file alacritty imports.

use crate::terminal::{SLOTS, Shade, Terminal};

/// TOML has no way to name a colour once and use it twice, so the sixteen are
/// spelled out. They are spelled out here and nowhere else, and
/// `alacritty.toml` names this file rather than repeating them.
pub fn spend(terminal: &Terminal) -> String {
    let head = [
        "# Written by console-theme from theme/palette.toml.".to_string(),
        "# Imported by alacritty.toml, which holds no colour of its own.".to_string(),
        String::new(),
        "[colors.primary]".to_string(),
        format!("background = \"0x{}\"", terminal.background),
        format!("foreground = \"0x{}\"", terminal.foreground),
        String::new(),
        "[colors.cursor]".to_string(),
        format!("cursor = \"0x{}\"", terminal.cursor),
        format!("text = \"0x{}\"", terminal.background),
        String::new(),
        "[colors.selection]".to_string(),
        format!("background = \"0x{}\"", terminal.selection),
        format!("text = \"0x{}\"", terminal.background),
    ];

    let sixteen = [Shade::Normal, Shade::Bright].into_iter().flat_map(|shade| {
        [String::new(), format!("[colors.{}]", shade.name())]
            .into_iter()
            .chain(SLOTS.map(|slot| {
                format!("{slot} = \"0x{}\"", terminal.slot(shade, slot))
            }))
    });

    format!("{}\n", head.into_iter().chain(sixteen).collect::<Vec<_>>().join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::{blossom, palette_spec};

    fn written() -> String {
        spend(&Terminal::of(&palette_spec(), &blossom()).expect("the terminal table is declared"))
    }

    #[test]
    fn all_sixteen_are_spelled_out() {
        let toml = written();
        for shade in ["normal", "bright"] {
            let body = toml.split_once(&format!("[colors.{shade}]")).expect("the table").1;
            for slot in SLOTS {
                assert!(body.contains(&format!("{slot} = ")), "{shade} {slot} is missing");
            }
        }
    }

    #[test]
    fn a_colour_is_written_the_way_alacritty_reads_one() {
        for line in written().lines().filter(|l| l.contains(" = ")) {
            let (_, value) = line.split_once(" = ").expect("an assignment");
            assert!(value.starts_with("\"0x") && value.ends_with('"'), "{line:?}");
            assert_eq!(value.len(), 3 + 6 + 1, "{line:?}");
        }
    }

    #[test]
    fn what_the_cursor_and_the_selection_carry_is_the_background() {
        // Ink on a fill. The fill is what carries the contrast, so the letter
        // under the cursor is the terminal's own ground and not the foreground.
        let toml = written();
        let terminal = Terminal::of(&palette_spec(), &blossom()).expect("the terminal table is declared");
        let carried = format!("text = \"0x{}\"", terminal.background);
        assert_eq!(toml.matches(&carried).count(), 2, "cursor and selection");
    }

    #[test]
    fn it_parses_as_the_toml_alacritty_would_read() {
        let parsed: toml::Table = written().parse().expect("valid TOML");
        assert!(parsed.contains_key("colors"));
    }
}
