//! The palette as custom properties, and the colours a stylesheet cannot reach.
//!
//! LibreWolf is Firefox underneath, so this is the shape any browser of that
//! family takes: a stylesheet the profile imports, and a handful of prefs for
//! what the stylesheet never reaches.

use crate::palette::Palette;
use console_core_colour::Short;

use crate::spend::{ROLES, widest};

pub fn stylesheet(palette: &Palette) -> Result<String, Short> {
    let Ok(width) = widest(ROLES);
    let lines = ROLES
        .iter()
        .map(|name| {
            let colour = palette.must(name)?;

            Ok(format!("  --{name:<width$}: #{colour};"))
        })
        .collect::<Result<Vec<_>, Short>>()?;
    let body = lines.join("\n");
    Ok(format!(
        "/* Written by console-palette from theme/palette.toml.\n\
         \x20  userChrome.css and userContent.css both import this and neither\n\
         \x20  holds a colour of its own. */\n\n:root {{\n{body}\n}}\n"
    ))
}

pub fn prefs(palette: &Palette) -> Result<String, Short> {
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
        let colour = palette.must(role)?;

        Ok(format!("user_pref(\"{pref}\", \"#{colour}\");"))
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
        let css = stylesheet(&blossom()).expect("every colour it spends is declared");
        for name in ROLES {
            assert!(css.contains(&format!("--{name}")), "{name} is missing");
        }
    }

    #[test]
    fn the_properties_are_inside_the_root_block() {
        let css = stylesheet(&blossom()).expect("every colour it spends is declared");
        let (before, inside) = css.split_once(":root {").expect("a root block");
        assert!(!before.contains("--night"));
        assert!(inside.trim_end().ends_with('}'));
    }

    #[test]
    fn a_page_that_has_not_painted_is_painted_the_darkest_ground() {
        let js = prefs(&blossom()).expect("every colour it spends is declared");
        let palette = blossom();
        let night = palette.must("night").expect("a declared colour");
        assert!(js.contains(&format!("\"browser.display.background_color\", \"#{night}\"")));
        assert!(js.contains(&format!("\"browser.display.background_color.dark\", \"#{night}\"")));
    }

    #[test]
    fn a_link_and_a_visited_link_are_told_apart() {
        let js = prefs(&blossom()).expect("every colour it spends is declared");
        let palette = blossom();
        assert_ne!(palette.must("sky").expect("a declared colour"), palette.must("mauve").expect("a declared colour"));
        assert!(js.contains(&format!("anchor_color\", \"#{}\"", palette.must("sky").expect("a declared colour"))));
        assert!(js.contains(&format!("visited_color\", \"#{}\"", palette.must("mauve").expect("a declared colour"))));
    }

    #[test]
    fn the_prefs_are_a_block_to_splice_and_do_not_end_in_a_newline() {
        assert!(!prefs(&blossom()).expect("every colour it spends is declared").ends_with('\n'));
    }
}
