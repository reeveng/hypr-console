//! The terminal's colors as a file alacritty imports.

use console_core_never::Never;

use crate::terminal::{SLOTS, Shade, Terminal};

pub fn spend(terminal: &Terminal) -> Result<String, Never> {
    let head = [
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
        let Ok(name) = shade.name();

        [String::new(), format!("[colors.{name}]")].into_iter().chain(SLOTS.map(|slot| {
            let Ok(code) = terminal.slot(shade, slot);

            format!("{slot} = \"0x{code}\"")
        }))
    });

    Ok(format!("{}\n", head.into_iter().chain(sixteen).collect::<Vec<_>>().join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use crate::spend::tests::{blossom, declared_palette};

    fn alacritty_config() -> Result<String, Box<dyn Error>> {
        let palette = blossom()?;
        let configuration = declared_palette()?;
        let terminal = Terminal::of(&configuration, &palette)?;

        let Ok(said) = spend(&terminal);

        Ok(said)
    }

    #[test]
    fn all_sixteen_are_spelled_out() -> Result<(), Box<dyn Error>> {
        let toml = alacritty_config()?;

        for shade in ["normal", "bright"] {
            let (_, body) = toml.split_once(&format!("[colors.{shade}]")).ok_or("the table")?;

            for slot in SLOTS {
                assert!(body.contains(&format!("{slot} = ")), "{shade} {slot} is missing");
            }
        }

        Ok(())
    }

    #[test]
    fn a_color_is_written_the_way_alacritty_reads_one() -> Result<(), Box<dyn Error>> {
        let toml = alacritty_config()?;

        for line in toml.lines().filter(|line| line.contains(" = ")) {
            let (_, value) = line.split_once(" = ").ok_or("an assignment")?;
            assert!(value.starts_with("\"0x") && value.ends_with('"'), "{line:?}");
            assert_eq!(value.len(), "\"0x".len().saturating_add(6).saturating_add("\"".len()), "{line:?}");
        }

        Ok(())
    }

    #[test]
    fn what_the_cursor_and_the_selection_carry_is_the_background() -> Result<(), Box<dyn Error>> {
        let toml = alacritty_config()?;
        let palette = blossom()?;
        let configuration = declared_palette()?;
        let terminal = Terminal::of(&configuration, &palette)?;
        let carried = format!("text = \"0x{}\"", terminal.background);
        assert_eq!(toml.matches(&carried).count(), 2, "cursor and selection");

        Ok(())
    }

    #[test]
    fn it_parses_as_the_toml_alacritty_would_read() -> Result<(), Box<dyn Error>> {
        let toml = alacritty_config()?;
        let parsed: toml::Table = toml.parse()?;
        assert!(parsed.contains_key("colors"));

        Ok(())
    }
}
