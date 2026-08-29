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
