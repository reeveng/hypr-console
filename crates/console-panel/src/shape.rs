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


use console_core_never::Never;
use console_core_number_conversion::whole_i32;
pub const PART: i32 = 93;

pub fn part_of(room: i32) -> Result<i32, Never> {
    share(room, PART)
}

pub const TALL: i32 = 80;

pub fn tall_part_of(screen: i32) -> Result<i32, Never> {
    share(screen, TALL)
}

fn share(room: i32, part: i32) -> Result<i32, Never> {
    whole_i32(f64::from(room) * f64::from(part) / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_share_of_either_way_up() {
        assert_eq!(part_of(1024), Ok(952));
        assert_eq!(part_of(640), Ok(595));
    }

    #[test]
    fn no_room_is_no_panel_rather_than_a_negative_one() {
        assert_eq!(part_of(0), Ok(0));
        assert_eq!(tall_part_of(0), Ok(0));
    }

    #[test]
    fn the_same_height_whatever_is_being_shown() {
        assert_eq!(tall_part_of(640), Ok(512));
        assert_eq!(tall_part_of(1024), Ok(819));
    }
}
