//! How much room the panel has, and how much of it a whole number of rows
//! fills.
//!
//! The surface is anchored to all four edges and claims no exclusive zone of
//! its own, so what the compositor grants is the screen less whatever else has
//! taken a piece of it: the bar, and the on-screen keyboard while it is up. The
//! panel is measured against that room rather than against the screen, so it
//! never has to know that either exists.

use crate::shape;

/// What the panel keeps between itself and whatever else is on the screen.
///
/// A card that reaches the bar at the top and the keyboard at the bottom reads
/// as a thing wedged into a gap rather than as a thing lying on the desktop.
pub const BREATH: i32 = 16;

/// What a card about one picture spends under the picture, on the two rows it
/// began with.
///
/// Measured on the card rather than guessed at: what the file is, and either
/// where in the folder it is or how to start it, plus the margins holding the
/// list off the card's edges. It is kept as the measurement it is, and [`ROW`]
/// and [`EDGES`] are the two halves it was taken apart into once the number of
/// rows stopped being two.
pub const UNDER: i32 = 107;

/// What one row written under the picture costs, with the space around it.
///
/// A card no longer has a fixed number of rows beneath the picture. A film
/// gained a bar, and both kinds lose everything when the card is left alone,
/// so what the picture may be drawn at is whatever the rows that are actually
/// there have not taken.
pub const ROW: i32 = 45;

/// What the list spends holding itself off the card's edges, whatever is in it.
///
/// The remainder of [`UNDER`], rather than a second measurement: the two rows
/// it was measured with are two of these rows, and a card with none of them is
/// this much and no more.
pub const EDGES: i32 = UNDER - 2 * ROW;

/// What the row of tabs over the card costs, with the space round it.
///
/// Separate from [`UNDER`] because it is the one part of the frame that is not
/// always there: a card opened out has no strip, and a picture that went on
/// leaving room for one would leave a band of nothing across the top of a
/// screen somebody had just asked to fill.
pub const STRIP: i32 = 82;

/// Whether the row of tabs is drawn over the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strip {
    Shown,
    Hidden,
}

/// How tall the one picture a card is about may be drawn, on a card of a given
/// height.
///
/// A height and not a size. The width is the card's, less its margins, and is
/// the panel's to say because it is the panel that was granted it; this is the
/// one number a picture cannot work out for itself, which is how much of the
/// card is left under it.
///
/// Worked out rather than written down, because the card has two heights: its
/// share of the desktop, and the whole screen once it has been opened out, and
/// because how many rows are under the picture is a thing that changes while
/// somebody is looking at it. What the card gains the picture gains, and so is
/// every row that goes away.
///
/// Never nothing. A card too short to hold both the picture and the rows under
/// it is a card that has to give the picture something, because a row of zero
/// height is a hole a person cannot tell from a file that would not open.
pub fn showing(card: i32, strip: Strip, under: i32) -> i32 {
    let band = match strip {
        Strip::Shown => STRIP,
        Strip::Hidden => 0,
    };

    (card - band - EDGES - under.max(0) * ROW).max(SMALLEST)
}

/// The least a picture is ever drawn at, whatever the card has left.
const SMALLEST: i32 = 64;

/// How wide the panel is: its share of the room, or of the monitor while there
/// is no room to be had.
///
/// Before the window is on screen nothing has been granted yet and the monitor
/// is the nearest thing to an answer. The first fit corrects it.
pub fn across(given: i32, monitor: i32) -> i32 {
    shape::part_of(match given > 1 {
        true => given,
        false => monitor,
    })
}

/// As tall as the panel may get.
///
/// The share every one of these surfaces takes, and never more than the room
/// leaves after the breath on either side of it. Before the window is on
/// screen there is nothing to be given, and the share stands alone.
///
/// The share is of the screen and the cap is the room, so on an ordinary
/// desktop every panel is the same height and lands in the same place, and
/// with the on-screen keyboard up the cap is what is left and the panel
/// squeezes into it.
pub fn ceiling(given: i32, screen: i32) -> i32 {
    let wanted = shape::tall_part_of(screen);

    match given > 1 {
        true => wanted.min(given - 2 * BREATH),
        false => wanted,
    }
}

/// The height to ask for: the whole ceiling, and never less than one row.
///
/// The ceiling exactly, rather than as many whole rows as fit under it. Whole
/// rows were there to keep a card that grew with its list from cutting its own
/// last row, and a card that no longer grows with its list has nothing to be
/// kept from: what it does have is a different row height on every page, so a
/// whole number of them came to a different height on every page, and the tab
/// strip the shoulders act on never landed twice in the same place.
///
/// A last row cut by a card that is always this size is not a broken panel. It
/// is the list saying there is more of it below.
pub fn tall_enough(frame: i32, row: i32, ceiling: i32) -> i32 {
    frame + row.max(ceiling - frame)
}

#[cfg(test)]
mod tests {
    use super::Strip;

    /// The card this was measured on, the rows it was measured with, and the
    /// number it was measured as.
    #[test]
    fn a_picture_on_the_ordinary_card_is_what_it_was_measured_at() {
        assert_eq!(super::showing(461, Strip::Shown, 2), 272);
    }

    /// Every row that goes away is room the picture has. A card left alone
    /// until its rows have gone gets all of it back.
    #[test]
    fn what_the_rows_do_not_take_is_the_pictures() {
        let two = super::showing(461, Strip::Shown, 2);
        assert_eq!(super::showing(461, Strip::Shown, 3), two - super::ROW);
        assert_eq!(super::showing(461, Strip::Shown, 0), two + 2 * super::ROW);
    }

    /// Opened out the card is the whole screen, and the rows under the picture
    /// are the same rows: everything the card gained goes to the picture, and
    /// so does the band the tabs were in.
    #[test]
    fn opening_the_card_out_gives_the_room_to_the_picture() {
        let gained = super::showing(600, Strip::Hidden, 2) - super::showing(461, Strip::Shown, 2);
        assert_eq!(gained, 600 - 461 + super::STRIP);
    }

    /// A card with no room in it still draws a picture. A picture of no height
    /// is a hole nobody can tell from a file that would not open.
    #[test]
    fn a_card_with_nothing_to_spare_still_draws_something() {
        assert!(super::showing(0, Strip::Shown, 2) > 0);
        assert!(super::showing(200, Strip::Shown, 2) > 0);
        assert!(super::showing(200, Strip::Shown, 9) > 0, "more rows than the card holds");
    }

    use super::*;

    #[test]
    fn the_room_the_compositor_granted_beats_the_monitor() {
        assert_eq!(across(800, 1024), shape::part_of(800));
        assert_eq!(across(0, 1024), shape::part_of(1024), "nothing granted yet");
    }

    /// With the keyboard up the surface is the gap between the bar and the
    /// keys. Measured against the screen instead, the panel hung over the bar
    /// and its last rows were behind the keys.
    #[test]
    fn a_keyboard_taking_the_screen_takes_it_from_the_panel_too() {
        let share = shape::tall_part_of(640);
        assert_eq!(ceiling(300, 640), 300 - 2 * BREATH);
        assert_eq!(ceiling(900, 640), share, "its share, where there is room");
        assert_eq!(ceiling(0, 640), share, "nothing granted yet");
    }

    /// Two pages whose rows are different heights are the same height as each
    /// other, which is the whole of what the share is for.
    #[test]
    fn the_ceiling_is_the_height_whatever_the_rows_are() {
        assert_eq!(tall_enough(100, 60, 400), 400);
        assert_eq!(tall_enough(100, 44, 400), 400);
    }

    /// A panel with no room for even one row is a panel showing nothing, and
    /// there is always something to show.
    #[test]
    fn there_is_room_for_one_row_however_little_room_there_is() {
        assert_eq!(tall_enough(100, 60, 0), 160);
    }
}
