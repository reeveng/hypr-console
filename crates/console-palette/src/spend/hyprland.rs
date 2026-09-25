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
    use crate::spend::tests::blossom;

    #[test]
    fn it_is_a_lua_table_that_can_be_spliced_into_a_config() {
        let lua = spend(&blossom()).expect("every color it spends is declared");
        assert!(lua.starts_with("local blossom = {"));
        assert!(lua.ends_with('}'));
        assert!(!lua.ends_with('\n'), "a block to splice, not a file");
    }

    #[test]
    fn every_color_is_opaque() {
        for line in spend(&blossom()).expect("every color it spends is declared").lines().filter(|line| line.contains("rgba")) {
            assert!(line.contains("ff)"), "{line:?} is not opaque");
        }
    }

    #[test]
    fn the_window_you_are_typing_into_is_not_the_color_of_the_ones_you_are_not() {
        let lua = spend(&blossom()).expect("every color it spends is declared");
        let of = |name: &str| {
            lua.lines().find(|line| line.trim_start().starts_with(name)).expect(name).to_string()
        };
        assert_ne!(of("active"), of("inactive"));
    }

    #[test]
    fn what_is_behind_everything_is_the_deepest_ground() {
        let palette = blossom();
        assert!(spend(&palette).expect("every color it spends is declared").contains(&format!("behind   = \"rgba({}ff)\"", palette.must("night").expect("a declared color"))));
    }
}
