//! The palette in GTK's stylesheet language, which is imported by five files.
//!
//! waybar, wofi, the panel, GTK's own themes and libadwaita all read GTK CSS
//! and all support `@import`, so this is the only file among them that holds a
//! hex. The names libadwaita and Breeze ask for are defined here too, as
//! references rather than as colours, so that a role changing its shade
//! changes every name that stands for it.

use console_colour::Short;
use crate::palette::Palette;
use crate::spend::{ROLES, breeze, widest};

const ADWAITA: [(&str, &str); 28] = [
    ("accent_bg_color", "pink"), ("accent_color", "pink"),
    ("accent_fg_color", "night"), ("borders", "edge"),
    ("card_bg_color", "panel"), ("card_fg_color", "text"),
    ("destructive_bg_color", "coral"), ("destructive_color", "coral"),
    ("destructive_fg_color", "night"), ("dialog_bg_color", "panel"),
    ("dialog_fg_color", "text"), ("error_color", "coral"),
    ("headerbar_bg_color", "ground"), ("headerbar_fg_color", "text"),
    ("popover_bg_color", "panel"), ("popover_fg_color", "text"),
    ("selected_bg_color", "pink"), ("selected_fg_color", "night"),
    ("sidebar_bg_color", "ground"), ("sidebar_fg_color", "text"),
    ("success_color", "leaf"), ("theme_selected_bg_color", "pink"),
    ("theme_selected_fg_color", "night"), ("view_bg_color", "night"),
    ("view_fg_color", "text"), ("warning_color", "butter"),
    ("window_bg_color", "panel"), ("window_fg_color", "text"),
];

const ADWAITA_WIDTH: usize = 28;

const SUFFIX: &str = "_breeze";

pub fn spend(palette: &Palette) -> Result<String, Short> {
    let Ok(width) = widest(ROLES);
    let colours = ROLES
        .iter()
        .map(|name| {
            let colour = palette.must(name)?;

            Ok(format!("@define-color {name:<width$} #{colour};"))
        })
        .collect::<Result<Vec<_>, Short>>()?;
    let colours = colours.into_iter();

    let adwaita = ADWAITA.iter().map(|(name, role)| {
        format!("@define-color {name:<ADWAITA_WIDTH$} @{role};")
    });

    let Ok(widest) = widest(breeze::NAMES);

    let breeze_width = widest.saturating_add(SUFFIX.len());
    let sorted = {
        let mut names = breeze::NAMES;
        names.sort_unstable();
        names
    };
    let breeze = sorted.into_iter().filter_map(move |name| {
        let Ok(role) = breeze::role(name);

        let role = role?;

        Some(format!("@define-color {:<breeze_width$} @{role};", format!("{name}{SUFFIX}")))
    });

    let lines = [
        "/* Written by console-theme from theme/palette.toml.".to_string(),
        "   Everything on this machine that speaks GTK's stylesheet language".to_string(),
        "   imports this file. Nothing else among them holds a colour. */".to_string(),
        String::new(),
    ]
    .into_iter()
    .chain(colours)
    .chain([
        String::new(),
        "/* The names libadwaita reads, as references: a role changing its".to_string(),
        "   shade changes every name that stands for it. */".to_string(),
    ])
    .chain(adwaita)
    .chain([
        String::new(),
        "/* The names Breeze GTK reads, which are not libadwaita's and are many.".to_string(),
        "   Left alone this list is written by Plasma's settings, which this".to_string(),
        "   machine does not run, so it sat a whole flavour behind the screen. */".to_string(),
    ])
    .chain(breeze)
    .collect::<Vec<_>>();

    Ok(format!("{}\n", lines.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    #[test]
    fn every_role_is_written_once_as_a_colour() {
        let css = spend(&blossom()).expect("every colour it spends is declared");
        for name in ROLES {
            let written = css.lines().filter(|l| l.starts_with(&format!("@define-color {name} "))).count();
            assert_eq!(written, 1, "{name} is defined {written} times");
        }
    }

    #[test]
    fn only_the_roles_hold_a_hex_and_every_other_name_is_a_reference() {
        let css = spend(&blossom()).expect("every colour it spends is declared");
        let holds_hex = |line: &str| line.contains('#');
        for line in css.lines().filter(|l| l.starts_with("@define-color")).filter(|l| holds_hex(l)) {
            let name = line.split_whitespace().nth(1).expect("a name");
            assert!(ROLES.contains(&name), "{name} holds a hex and is not a role");
        }
    }

    #[test]
    fn every_reference_points_at_a_role_that_exists() {
        let css = spend(&blossom()).expect("every colour it spends is declared");
        for line in css.lines().filter(|l| l.contains(" @")) {
            let role = line.rsplit(" @").next().and_then(|r| r.strip_suffix(';')).expect("a role");
            assert!(ROLES.contains(&role), "{line} points at {role}, which is not a role");
        }
    }

    #[test]
    fn breeze_gets_every_name_it_asks_for() {
        let css = spend(&blossom()).expect("every colour it spends is declared");
        for name in breeze::NAMES {
            assert!(
                css.contains(&format!("@define-color {name}_breeze ")),
                "Breeze asks for {name} and it was not written"
            );
        }
    }

    #[test]
    fn it_ends_in_exactly_one_newline() {
        let css = spend(&blossom()).expect("every colour it spends is declared");
        assert!(css.ends_with(";\n") && !css.ends_with("\n\n"));
    }
}
