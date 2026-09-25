//! The palette as custom properties, and the colors a stylesheet cannot reach.
//!
//! LibreWolf is Firefox underneath, so this is the shape any browser of that
//! family takes: a stylesheet the profile imports, and a handful of prefs for
//! what the stylesheet never reaches.

use crate::palette::{Palette, Spent};
use console_core_color::Short;

use console_core_number_conversion::index;

use crate::spend::{ROLES, widest};

pub fn stylesheet(palette: &Palette) -> Result<String, Short> {
    let Ok(width) = widest(&ROLES);
    let Ok(width) = index(width);
    let lines = palette.lines(&ROLES, |Spent { name, color }| format!("  --{name:<width$}: #{color};"))?;
    let body = lines.join("\n");
    Ok(format!(
        "/* Written by console-palette from theme/palette.toml.\n\
         \x20  userChrome.css and userContent.css both import this and neither\n\
         \x20  holds a color of its own. */\n\n:host, :root {{\n{body}\n}}\n"
    ))
}

pub fn preferences(palette: &Palette) -> Result<String, Short> {
    [
        ("browser.display.background_color", "night"),
        ("browser.display.background_color.dark", "night"),
        ("browser.display.foreground_color", "text"),
        ("browser.anchor_color", "sky"),
        ("browser.visited_color", "mauve"),
        ("browser.active_color", "pink"),
    ]
    .iter()
    .map(|(pref, role)| {
        let color = palette.must(role)?;

        Ok(format!("user_pref(\"{pref}\", \"#{color}\");"))
    })
    .collect::<Result<Vec<_>, Short>>()
    .map(|lines| lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    #[test]
    fn every_role_is_a_custom_property() {
        let css = stylesheet(&blossom()).expect("every color it spends is declared");
        for name in ROLES {
            assert!(css.contains(&format!("--{name}")), "{name} is missing");
        }
    }

    #[test]
    fn the_properties_are_inside_the_root_block() {
        let css = stylesheet(&blossom()).expect("every color it spends is declared");
        let (before, inside) = css.split_once(":host, :root {").expect("a root block");
        assert!(!before.contains("--night"));
        assert!(inside.trim_end().ends_with('}'));
    }

    #[test]
    fn a_page_that_has_not_painted_is_painted_the_darkest_ground() {
        let script = preferences(&blossom()).expect("every color it spends is declared");
        let palette = blossom();
        let night = palette.must("night").expect("a declared color");
        assert!(script.contains(&format!("\"browser.display.background_color\", \"#{night}\"")));
        assert!(script.contains(&format!("\"browser.display.background_color.dark\", \"#{night}\"")));
    }

    #[test]
    fn a_link_and_a_visited_link_are_told_apart() {
        let script = preferences(&blossom()).expect("every color it spends is declared");
        let palette = blossom();
        assert_ne!(palette.must("sky").expect("a declared color"), palette.must("mauve").expect("a declared color"));
        assert!(script.contains(&format!("anchor_color\", \"#{}\"", palette.must("sky").expect("a declared color"))));
        assert!(script.contains(&format!("visited_color\", \"#{}\"", palette.must("mauve").expect("a declared color"))));
    }

    #[test]
    fn the_prefs_are_a_block_to_splice_and_do_not_end_in_a_newline() {
        assert!(!preferences(&blossom()).expect("every color it spends is declared").ends_with('\n'));
    }
}
