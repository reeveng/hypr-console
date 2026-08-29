//! How big the things that take the screen are.
//!
//! The menu, the settings and the guide are the same kind of thing: something
//! that comes up over the desktop, is driven by the same buttons, and goes away
//! again. They were three widths. The menu was 70% of the screen, the settings
//! were 880 points and the guide was 900 and grew past it, so opening one after
//! another moved the edges of the screen about and read as three programs
//! rather than one desktop.
//!
//! A share of the room rather than a number of points. A number is only right
//! on the screen it was measured on: this device is 1024 points across in
//! landscape and 640 the other way up, and the same 900 that is a card on the
//! desktop in one is wider than the screen in the other. It is asked again
//! whenever the room changes, so a panel that is up while the room changes
//! under it is the same share of the new room rather than the old room's
//! number.
//!
//! It is here rather than in any one of them because a number written down in
//! two places is a number that goes out of step. They all work it out against
//! what the compositor grants them, and they all get it from here.

/// Out of a hundred.
///
/// Wide enough for a network's name and the reading beside it, with enough of
/// the desktop left down each side to say that this is a card lying on it
/// rather than the screen itself.
pub const PART: i32 = 88;

/// That share of a given room.
pub fn part_of(room: i32) -> i32 {
    share(room, PART)
}

/// Out of a hundred, downwards.
///
/// They were three heights once, and two of those were numbers of points: the
/// settings stopped at 430, the guide at 500, and the menu was as tall as it
/// had rows for, so every list was a different height and the tab strip, which
/// is what the shoulders act on, was never twice in the same place. One share
/// leaves them all where the last one was.
///
/// Of the screen rather than of the room, so that every panel is the same
/// height on a quiet desktop. They take the smaller of this and what they are
/// granted, so a keyboard coming up still takes its part of them.
pub const TALL: i32 = 72;

/// That share of a given screen, downwards.
pub fn tall_part_of(screen: i32) -> i32 {
    share(screen, TALL)
}

fn share(room: i32, part: i32) -> i32 {
    (f64::from(room) * f64::from(part) / 100.0).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_share_of_either_way_up() {
        assert_eq!(part_of(1024), 901);
        assert_eq!(part_of(640), 563);
    }

    #[test]
    fn no_room_is_no_panel_rather_than_a_negative_one() {
        assert_eq!(part_of(0), 0);
        assert_eq!(tall_part_of(0), 0);
    }

    /// The three surfaces are the same height on the same screen, which is the
    /// whole of what this is for.
    #[test]
    fn the_same_height_whatever_is_being_shown() {
        assert_eq!(tall_part_of(640), 461);
        assert_eq!(tall_part_of(1024), 737);
    }
}
