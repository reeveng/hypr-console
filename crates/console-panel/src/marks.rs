//! What the panel draws that is not a row, and what each part is called.
//!
//! The names are how a part is styled, so they are written once and read by
//! both the drawing and the stylesheet. A part named in one and not the other
//! is a part that is there and cannot be seen, or one that is styled and does
//! not exist.

/// The way out, for a hand with no B under its thumb.
///
/// B closes a panel and a finger has no B. Four of the bar's icons open one, so
/// without this a tap could put a panel on the screen that only the controller
/// could take off again.
pub const SHUT: &str = "\u{d7}";

/// The way to the tabs the strip has no room for, which are the shoulders said
/// in the other language.
pub const BEFORE: &str = "\u{2039}";
pub const AFTER: &str = "\u{203a}";

/// The mark on a row that opens onto another list.
///
/// The strip's mark, said in another place. On the strip it is the tab after
/// this one and on a row it is the list under it, and where it is drawn is
/// what tells the two apart. Both say there is more that way, which is the
/// whole of what a thumb has to know before it presses A.
pub const INTO: &str = AFTER;

/// The two ends of a level.
///
/// A level is the one thing on a panel that left and right do and a tap cannot:
/// tapping the row it is on silences it. Without these the volume is a reading
/// a person can look at and not change.
pub const LESS: &str = "\u{2212}";
pub const MORE: &str = "+";

/// The answer that leaves a question alone, which every question has.
pub const NO: &str = "No";

/// What each part of a panel is called.
pub mod named {
    pub const ABOUT: &str = "about";
    pub const ANSWER: &str = "answer";
    pub const ASIDE: &str = "aside";
    pub const ASKED: &str = "asked";
    pub const BAR: &str = "bar";
    pub const CARD: &str = "card";
    pub const COVER: &str = "cover";
    pub const ICON: &str = "icon";
    pub const INTO: &str = "into";
    pub const MORE: &str = "more";
    pub const NOTE: &str = "note";
    pub const PANEL: &str = "panel";
    pub const SAID: &str = "said";
    pub const SHUT: &str = "shut";
    pub const SOUGHT: &str = "sought";
    pub const STEP: &str = "step";
    pub const STRIP: &str = "strip";
    pub const SURE: &str = "sure";
    pub const TAB: &str = "tab";
    pub const TOP: &str = "top";

    /// Every one of them, which is what the stylesheet is checked against.
    pub const EVERY: [&str; 20] = [
        ABOUT, ANSWER, ASIDE, ASKED, BAR, CARD, COVER, ICON, INTO, MORE, NOTE, PANEL, SAID, SHUT,
        SOUGHT, STEP, STRIP, SURE, TAB, TOP,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A part styled and never drawn, or drawn and never styled, is the same
    /// bug read from two ends.
    #[test]
    fn every_part_the_panel_draws_is_dressed() {
        let sheet = crate::style::sheet();
        for name in named::EVERY {
            assert!(sheet.contains(&format!("#{name}")), "#{name} is drawn and not styled");
        }
    }

    #[test]
    fn a_level_draws_the_two_ends_of_itself() {
        assert_ne!(LESS, MORE);
        assert!(crate::style::sheet().contains(&format!("#{}", named::STEP)));
    }

    /// A strip with more tabs than the panel is wide shows the ones it has
    /// room for and hides the rest, which the shoulders still reach. A finger
    /// has no shoulders, so without these marks a hidden tab is a part of the
    /// panel nobody holding the device by the screen can get to.
    #[test]
    fn a_tab_the_strip_has_no_room_for_can_be_reached_by_hand() {
        assert_ne!(BEFORE, AFTER);
        assert!(crate::style::sheet().contains(&format!("#{}", named::MORE)));
    }

    #[test]
    fn nothing_is_called_two_things() {
        let mut every = named::EVERY.to_vec();
        every.sort_unstable();
        every.dedup();
        assert_eq!(every.len(), named::EVERY.len());
    }
}
