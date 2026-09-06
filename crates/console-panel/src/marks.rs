//! What the panel draws that is not a row, and what each part is called.
//!
//! The names are how a part is styled, so they are written once and read by
//! both the drawing and the stylesheet. A part named in one and not the other
//! is a part that is there and cannot be seen, or one that is styled and does
//! not exist.

pub const SHUT: &str = "\u{d7}";

pub const BEFORE: &str = "\u{2039}";
pub const AFTER: &str = "\u{203a}";

pub const INTO: &str = AFTER;

pub const LESS: &str = "\u{2212}";
pub const MORE: &str = "+";

pub const ELSE: &str = "\u{2026}";

pub const NO: &str = "No";

pub const YES: &str = "Yes";

pub mod named {
    pub const ABOUT: &str = "about";
    pub const ANSWER: &str = "answer";
    pub const ASIDE: &str = "aside";
    pub const ASKED: &str = "asked";
    pub const BAR: &str = "bar";
    pub const CARD: &str = "card";
    pub const COVER: &str = "cover";
    pub const ELSE: &str = "else";
    pub const ICON: &str = "icon";
    pub const INTO: &str = "into";
    pub const MORE: &str = "more";
    pub const NOTE: &str = "note";
    pub const PANEL: &str = "panel";
    pub const PRESS: &str = "press";
    pub const SAID: &str = "said";
    pub const SHOWING: &str = "showing";
    pub const PLAYING: &str = "playing";
    pub const SHUT: &str = "shut";
    pub const SLEEVE: &str = "sleeve";
    pub const SOUGHT: &str = "sought";
    pub const STEP: &str = "step";
    pub const STRIP: &str = "strip";
    pub const SURE: &str = "sure";
    pub const TAB: &str = "tab";
    pub const TOP: &str = "top";

    pub const EVERY: [&str; 21] = [
        ABOUT, ANSWER, ASIDE, ASKED, BAR, CARD, COVER, ELSE, ICON, INTO, MORE, NOTE, PANEL, SAID,
        SHUT, SOUGHT, STEP, STRIP, SURE, TAB, TOP,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> String {
        let Ok(sheet) = crate::style::sheet();

        sheet
    }

    #[test]
    fn every_part_the_panel_draws_is_dressed() {
        let sheet = sheet();
        for name in named::EVERY {
            assert!(sheet.contains(&format!("#{name}")), "#{name} is drawn and not styled");
        }
    }

    #[test]
    fn a_level_draws_the_two_ends_of_itself() {
        assert_ne!(LESS, MORE);
        assert!(sheet().contains(&format!("#{}", named::STEP)));
    }

    #[test]
    fn what_else_a_row_offers_can_be_reached_by_hand() {
        for other in [LESS, MORE, SHUT, BEFORE, AFTER] {
            assert_ne!(ELSE, other);
        }

        assert!(sheet().contains(&format!("#{}", named::ELSE)));
    }

    #[test]
    fn a_tab_the_strip_has_no_room_for_can_be_reached_by_hand() {
        assert_ne!(BEFORE, AFTER);
        assert!(sheet().contains(&format!("#{}", named::MORE)));
    }

    #[test]
    fn nothing_is_called_two_things() {
        let mut every = named::EVERY.to_vec();
        every.sort_unstable();
        every.dedup();
        assert_eq!(every.len(), named::EVERY.len());
    }
}
