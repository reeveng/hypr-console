//! Which tabs the strip has room for.
//!
//! The card is one width whatever is written on its tabs, so a strip of five
//! long words cannot ask for more than the card was going to be. What will not
//! fit is reached with the arrows at either end, which are the shoulders said
//! in the other language.

use std::ops::Range;

/// The card's own edge, the space between one tab and the next, and the space
/// inside the strip round the tabs.
///
/// The strip has to subtract every one of them to know how many tabs it has
/// room for, so they are named here and the stylesheet is written out of them
/// rather than beside them.
pub const GAP: i32 = 4;
pub const MARGIN: i32 = 14;
pub const PAD: i32 = 6;

/// How big a picture at the front of a row is, on a side.
///
/// Small enough that a row carrying one is the height it always was, so a list
/// of photographs shows as many rows as a list of anything else. Big enough to
/// tell one beach from another, which is the whole of what it is for.
pub const PICTURE: i32 = 32;

/// The line drawn round the card.
///
/// Named because the panel has to subtract it when it works out how many rows
/// will fit, and a number the drawing knows and the measuring does not is how
/// a list ends up cut through its last row.
pub const EDGE: i32 = 3;

/// The room on the row that tabs may have.
///
/// The card's width less everything on the row that is not a tab: the border,
/// the margins holding the strip off the card's edges, the padding inside the
/// strip, the way out, and both arrows. Both, always, even at an end where
/// only one of them is drawn, so the tabs do not shift sideways as you arrive
/// at the first or the last.
pub fn room(wide: i32, spent: i32) -> i32 {
    wide - 2 * EDGE - 2 * MARGIN - 2 * PAD - spent
}

/// How many tabs that room holds. Never none: a strip showing nothing is a
/// panel with no way along it.
pub fn fits(room: i32, cell: i32) -> usize {
    let each = cell + GAP;
    match each > 0 {
        true => ((room + GAP) / each).max(1) as usize,
        false => 1,
    }
}

/// The run of tabs to show, and the one it starts at.
///
/// The run moves as little as it can. Centred on the tab you are on it would
/// slide the whole strip under your thumb at every press, which is a row of
/// words that changes what it says each time you read it.
pub fn showing(tabs: usize, here: usize, from: usize, fits: usize) -> Range<usize> {
    if fits >= tabs {
        return 0..tabs;
    }
    let first = from.min(here).max((here + 1).saturating_sub(fits));
    let first = first.min(tabs - fits);
    first..first + fits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_strip_with_room_for_all_of_them_starts_at_the_first() {
        assert_eq!(showing(5, 3, 0, 5), 0..5);
        assert_eq!(showing(5, 3, 2, 9), 0..5);
    }

    /// A row of words that changes what it says each time you read it is worse
    /// than a row of words with an arrow at the end of it.
    #[test]
    fn the_run_moves_as_little_as_it_can() {
        assert_eq!(showing(5, 0, 0, 3), 0..3, "standing on the first");
        assert_eq!(showing(5, 2, 0, 3), 0..3, "the third is already showing");
        assert_eq!(showing(5, 3, 0, 3), 1..4, "one step, because it had to");
        assert_eq!(showing(5, 1, 2, 3), 1..4, "back the other way, one step");
    }

    #[test]
    fn the_run_never_hangs_off_either_end() {
        assert_eq!(showing(5, 4, 0, 3), 2..5);
        assert_eq!(showing(5, 4, 9, 3), 2..5);
    }

    #[test]
    fn a_tab_and_a_gap_is_what_a_tab_costs() {
        assert_eq!(fits(100, 20), 4, "four tabs and the gaps between them");
        assert_eq!(fits(0, 20), 1, "somewhere to stand, whatever the room");
    }

    #[test]
    fn the_room_is_the_card_less_everything_that_is_not_a_tab() {
        assert_eq!(room(900, 120), 900 - 2 * EDGE - 2 * MARGIN - 2 * PAD - 120);
    }
}
