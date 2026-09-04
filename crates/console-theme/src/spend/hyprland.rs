//! The compositor's two borders, and the colour behind everything.

use console_colour::Short;
use crate::palette::Palette;

/// `behind` is what the screen is where no window and no wallpaper covers it.
///
/// Hyprland's own default is a neutral grey, which is close enough to a
/// background that a wallpaper daemon can stop working without anybody going
/// to look for it. Told the palette instead, the desktop is the right colour
/// even with nothing painting on it.
pub fn spend(palette: &Palette) -> Result<String, Short> {
    // Aligned on the equals sign, which is three lines of a Lua table that
    // somebody will read on the device with no editor to hand.
    let width = "inactive".len();
    let entry = |name: &str, role: &str| {
        Ok(format!("    {name:<width$} = \"rgba({}ff)\",", palette.must(role)?))
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
        let lua = spend(&blossom()).expect("every colour it spends is declared");
        assert!(lua.starts_with("local blossom = {"));
        assert!(lua.ends_with('}'));
        assert!(!lua.ends_with('\n'), "a block to splice, not a file");
    }

    #[test]
    fn every_colour_is_opaque() {
        // A border at anything under full alpha reads as a border on a screen
        // that has not finished drawing.
        for line in spend(&blossom()).expect("every colour it spends is declared").lines().filter(|l| l.contains("rgba")) {
            assert!(line.contains("ff)"), "{line:?} is not opaque");
        }
    }

    #[test]
    fn the_window_you_are_typing_into_is_not_the_colour_of_the_ones_you_are_not() {
        let lua = spend(&blossom()).expect("every colour it spends is declared");
        let of = |name: &str| {
            lua.lines().find(|l| l.trim_start().starts_with(name)).expect(name).to_string()
        };
        assert_ne!(of("active"), of("inactive"));
    }

    #[test]
    fn what_is_behind_everything_is_the_deepest_ground() {
        let palette = blossom();
        assert!(spend(&palette).expect("every colour it spends is declared").contains(&format!("behind   = \"rgba({}ff)\"", palette.must("night").expect("a declared colour"))));
    }
}
