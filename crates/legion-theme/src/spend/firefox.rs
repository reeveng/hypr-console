//! The palette as custom properties, and the colours a stylesheet cannot reach.

use crate::palette::Palette;
use crate::spend::{ROLES, widest};

/// Custom properties, which are the only kind Firefox's chrome takes.
pub fn stylesheet(palette: &Palette) -> String {
    let width = widest(ROLES);
    let body = ROLES
        .iter()
        .map(|name| format!("  --{name:<width$}: #{};", &palette[name]))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "/* Written by legion-theme from theme/palette.toml.\n\
         \x20  userChrome.css and userContent.css both import this and neither\n\
         \x20  holds a colour of its own. */\n\n:root {{\n{body}\n}}\n"
    )
}

/// The colours Firefox will not take from a stylesheet.
///
/// A page that has not painted yet is painted by the browser, and left alone
/// that is white: on a dark desktop every link opens with a flash bright
/// enough to be the brightest thing that happens all day.
pub fn prefs(palette: &Palette) -> String {
    [
        ("browser.display.background_color", "night"),
        ("browser.display.background_color.dark", "night"),
        ("browser.display.foreground_color", "text"),
        ("browser.anchor_color", "sky"),
        ("browser.visited_color", "mauve"),
        ("browser.active_color", "pink"),
    ]
    .iter()
    .map(|(pref, role)| format!("user_pref(\"{pref}\", \"#{}\");", &palette[role]))
    .collect::<Vec<_>>()
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    #[test]
    fn every_role_is_a_custom_property() {
        let css = stylesheet(&blossom());
        for name in ROLES {
            assert!(css.contains(&format!("--{name}")), "{name} is missing");
        }
    }

    #[test]
    fn the_properties_are_inside_the_root_block() {
        let css = stylesheet(&blossom());
        let (before, inside) = css.split_once(":root {").expect("a root block");
        assert!(!before.contains("--night"));
        assert!(inside.trim_end().ends_with('}'));
    }

    #[test]
    fn a_page_that_has_not_painted_is_painted_the_darkest_ground() {
        let js = prefs(&blossom());
        let night = &blossom()["night"];
        assert!(js.contains(&format!("\"browser.display.background_color\", \"#{night}\"")));
        assert!(js.contains(&format!("\"browser.display.background_color.dark\", \"#{night}\"")));
    }

    #[test]
    fn a_link_and_a_visited_link_are_told_apart() {
        let js = prefs(&blossom());
        let palette = blossom();
        assert_ne!(&palette["sky"], &palette["mauve"]);
        assert!(js.contains(&format!("anchor_color\", \"#{}\"", &palette["sky"])));
        assert!(js.contains(&format!("visited_color\", \"#{}\"", &palette["mauve"])));
    }

    #[test]
    fn the_prefs_are_a_block_to_splice_and_do_not_end_in_a_newline() {
        assert!(!prefs(&blossom()).ends_with('\n'));
    }
}
