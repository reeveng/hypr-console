//! The compositor's two borders, and the color behind everything.

use console_core_color::Short;
use crate::palette::Palette;

pub fn spend(palette: &Palette) -> Result<String, Short> {
    let entry = |name: &str, role: &str| {
        let color = palette.must(role)?;

        Ok(format!("    {name:<width$} = \"rgba({color}ff)\",", width = "inactive".len()))
    };
    let table = [
        entry("active", "pink"),
        entry("inactive", "edge"),
        entry("behind", "night"),
    ]
    .into_iter()
    .collect::<Result<Vec<String>, Short>>()?;
    Ok(["local blossom = {".to_string()]
        .into_iter()
        .chain(table)
        .chain(["}".to_string()])
        .collect::<Vec<_>>()
        .join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use crate::spend::tests::blossom;

    #[test]
    fn it_is_a_lua_table_that_can_be_spliced_into_a_config() -> Result<(), Box<dyn Error>> {
        let palette = blossom()?;
        let lua = spend(&palette)?;
        assert!(lua.starts_with("local blossom = {"));
        assert!(lua.ends_with('}'));
        assert!(!lua.ends_with('\n'), "a block to splice, not a file");

        Ok(())
    }

    #[test]
    fn every_color_is_opaque() -> Result<(), Box<dyn Error>> {
        let palette = blossom()?;
        let spent = spend(&palette)?;

        for line in spent.lines().filter(|line| line.contains("rgba")) {
            assert!(line.contains("ff)"), "{line:?} is not opaque");
        }

        Ok(())
    }

    #[test]
    fn the_window_you_are_typing_into_is_not_the_color_of_the_ones_you_are_not() -> Result<(), Box<dyn Error>> {
        let palette = blossom()?;
        let lua = spend(&palette)?;
        let of = |name: &str| lua.lines().find(|line| line.trim_start().starts_with(name));

        let active = of("active").ok_or("active")?;
        let inactive = of("inactive").ok_or("inactive")?;

        assert_ne!(active, inactive);

        Ok(())
    }

    #[test]
    fn what_is_behind_everything_is_the_deepest_ground() -> Result<(), Box<dyn Error>> {
        let palette = blossom()?;
        let night = palette.must("night")?;
        let spent = spend(&palette)?;

        assert!(spent.contains(&format!("behind   = \"rgba({}ff)\"", night)));

        Ok(())
    }
}
