//! The names Breeze GTK asks for, and which of our colours each one wants.
//!
//! Left alone this list is written by Plasma's settings, which this machine
//! does not run, so it sat a whole flavour behind everything else on screen.

use console_never::Never;

pub const NAMES: [&str; 84] = [
    "borders", "content_view_bg", "error_color_backdrop", "error_color",
    "error_color_insensitive_backdrop", "error_color_insensitive",
    "insensitive_base_color", "insensitive_base_fg_color", "insensitive_bg_color",
    "insensitive_borders", "insensitive_fg_color", "insensitive_selected_bg_color",
    "insensitive_selected_fg_color", "insensitive_unfocused_bg_color",
    "insensitive_unfocused_fg_color", "insensitive_unfocused_selected_bg_color",
    "insensitive_unfocused_selected_fg_color", "link_color", "link_visited_color",
    "success_color_backdrop", "success_color", "success_color_insensitive_backdrop",
    "success_color_insensitive", "theme_base_color", "theme_bg_color",
    "theme_button_background_backdrop", "theme_button_background_backdrop_insensitive",
    "theme_button_background_insensitive", "theme_button_background_normal",
    "theme_button_decoration_focus_backdrop",
    "theme_button_decoration_focus_backdrop_insensitive",
    "theme_button_decoration_focus", "theme_button_decoration_focus_insensitive",
    "theme_button_decoration_hover_backdrop",
    "theme_button_decoration_hover_backdrop_insensitive",
    "theme_button_decoration_hover", "theme_button_decoration_hover_insensitive",
    "theme_button_foreground_active_backdrop",
    "theme_button_foreground_active_backdrop_insensitive",
    "theme_button_foreground_active", "theme_button_foreground_active_insensitive",
    "theme_button_foreground_backdrop", "theme_button_foreground_backdrop_insensitive",
    "theme_button_foreground_insensitive", "theme_button_foreground_normal",
    "theme_fg_color", "theme_header_background_backdrop", "theme_header_background",
    "theme_header_background_light", "theme_header_foreground_backdrop",
    "theme_header_foreground", "theme_header_foreground_insensitive_backdrop",
    "theme_header_foreground_insensitive", "theme_hovering_selected_bg_color",
    "theme_selected_bg_color", "theme_selected_fg_color", "theme_text_color",
    "theme_titlebar_background_backdrop", "theme_titlebar_background",
    "theme_titlebar_background_light", "theme_titlebar_foreground_backdrop",
    "theme_titlebar_foreground", "theme_titlebar_foreground_insensitive_backdrop",
    "theme_titlebar_foreground_insensitive", "theme_unfocused_base_color",
    "theme_unfocused_bg_color", "theme_unfocused_fg_color",
    "theme_unfocused_selected_bg_color_alt", "theme_unfocused_selected_bg_color",
    "theme_unfocused_selected_fg_color", "theme_unfocused_text_color",
    "theme_unfocused_view_bg_color", "theme_unfocused_view_text_color",
    "theme_view_active_decoration_color", "theme_view_hover_decoration_color",
    "tooltip_background", "tooltip_border", "tooltip_text", "unfocused_borders",
    "unfocused_insensitive_borders", "warning_color_backdrop", "warning_color",
    "warning_color_insensitive_backdrop", "warning_color_insensitive",
];

const RULE: [(&str, &str); 29] = [
    ("selected_fg", "night"), ("selected_bg", "pink"),
    ("decoration_focus", "pink"), ("decoration_hover", "pink"),
    ("view_active_decoration", "pink"), ("view_hover_decoration", "edge"),
    ("error", "coral"), ("warning", "butter"), ("success", "leaf"),
    ("link_visited", "mauve"), ("link_color", "sky"),
    ("borders", "edge"), ("tooltip_border", "edge"),
    ("tooltip_background", "panel"), ("tooltip_text", "text"),
    ("base_fg", "text"), ("base_color", "night"),
    ("content_view_bg", "night"), ("view_bg", "night"),
    ("view_text", "text"), ("text_color", "text"),
    ("foreground_active", "pink"), ("foreground", "text"),
    ("fg_color", "text"),
    ("background_light", "panel"), ("button_background", "panel"),
    ("header_background", "ground"), ("titlebar_background", "ground"),
    ("bg_color", "panel"),
];

pub fn role(name: &str) -> Result<Option<&'static str>, Never> {
    Ok(RULE
        .iter()
        .find(|(word, _)| name.contains(word))
        .map(|(_, role)| match (*role, name.contains("insensitive")) {
            ("text", true) => "soft",
            (role, _) => role,
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_breeze_reads_has_a_colour_decided_for_it() {
        let undecided: Vec<&str> = NAMES
            .iter()
            .copied()
            .filter(|name| {
                let Ok(role) = role(name);

                role.is_none()
            })
            .collect();
        assert!(undecided.is_empty(), "no colour decided for {undecided:?}");
    }

    #[test]
    fn the_second_word_decides_and_not_the_first() {
        assert_eq!(role("theme_selected_fg_color"), Ok(Some("night")));
        assert_eq!(role("theme_fg_color"), Ok(Some("text")));
    }

    #[test]
    fn greyed_out_text_is_soft_and_stays_readable() {
        assert_eq!(role("theme_button_foreground_insensitive"), Ok(Some("soft")));
        assert_eq!(role("theme_button_foreground_normal"), Ok(Some("text")));
    }

    #[test]
    fn a_greyed_out_fill_is_still_the_fill() {
        assert_eq!(role("insensitive_bg_color"), Ok(Some("panel")));
        assert_eq!(role("insensitive_selected_bg_color"), Ok(Some("pink")));
    }

    #[test]
    fn a_name_nobody_wrote_a_rule_for_answers_nothing() {
        assert_eq!(role("some_name_from_a_later_breeze"), Ok(None));
    }

    #[test]
    fn no_name_is_written_twice() {
        let mut sorted = NAMES.to_vec();
        sorted.sort_unstable();
        let mut once = sorted.clone();
        once.dedup();
        assert_eq!(sorted, once, "a name is in the list twice");
    }
}
