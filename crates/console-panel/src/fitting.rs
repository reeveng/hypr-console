//! How much room the panel has, and how much of it a whole number of rows
//! fills.
//!
//! The surface is anchored to all four edges and claims no exclusive zone of
//! its own, so what the compositor grants is the screen less whatever else has
//! taken a piece of it: the bar, and the on-screen keyboard while it is up. The
//! panel is measured against that room rather than against the screen, so it
//! never has to know that either exists.

use crate::shape;
use console_core_fonts::EM;
use console_core_never::Never;
use console_core_number_conversion::fitted;

pub const BREATH: i32 = EM * 8 / 9;

pub const UNDER: i32 = EM * 107 / 18;

pub const ROW: i32 = EM * 26 / 9;

pub const EDGES: i32 = UNDER - 2 * ROW;

pub const STRIP: i32 = EM * 41 / 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabBar {
    Shown,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Room {
    pub granted: i32,
    pub monitor: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tall {
    pub frame: i32,
    pub row: i32,
    pub ceiling: i32,
}

pub fn showing(card: i32, strip: TabBar, under: u32) -> Result<i32, Never> {
    let band = match strip {
        TabBar::Shown => STRIP,
        TabBar::Hidden => 0,
    };

    let Ok(rows) = fitted::<u32, i32>(under);

    Ok(card
        .saturating_sub(band)
        .saturating_sub(EDGES)
        .saturating_sub(rows.saturating_mul(ROW))
        .max(SMALLEST))
}

const SMALLEST: i32 = EM * 32 / 9;

pub fn x(room: Room) -> Result<i32, Never> {
    shape::part_of(match room.granted > 1 {
        true => room.granted,
        false => room.monitor,
    })
}

pub fn ceiling(room: Room) -> Result<i32, Never> {
    let Ok(wanted) = shape::tall_part_of(room.monitor);

    Ok(match room.granted > 1 {
        true => wanted.min(room.granted.saturating_sub(2i32.saturating_mul(BREATH))),
        false => wanted,
    })
}

pub fn tall_enough(tall: Tall) -> Result<i32, Never> {
    Ok(tall.frame.saturating_add(tall.row.max(tall.ceiling.saturating_sub(tall.frame))))
}

#[cfg(test)]
mod tests {
    use super::TabBar;

    #[test]
    fn a_picture_on_the_ordinary_card_is_what_it_was_measured_at() {
        assert_eq!(super::showing(461, TabBar::Shown, 2), Ok(272));
    }

    #[test]
    fn what_the_rows_do_not_take_is_the_pictures() {
        let Ok(two) = super::showing(461, TabBar::Shown, 2);

        assert_eq!(super::showing(461, TabBar::Shown, 3), Ok(two - super::ROW));
        assert_eq!(super::showing(461, TabBar::Shown, 0), Ok(two + 2 * super::ROW));
    }

    #[test]
    fn opening_the_card_out_gives_the_room_to_the_picture() {
        let Ok(open) = super::showing(600, TabBar::Hidden, 2);
        let Ok(shut) = super::showing(461, TabBar::Shown, 2);

        assert_eq!(open - shut, 600 - 461 + super::STRIP);
    }

    #[test]
    fn a_card_with_nothing_to_spare_still_draws_something() {
        let Ok(nothing) = super::showing(0, TabBar::Shown, 2);
        let Ok(little) = super::showing(200, TabBar::Shown, 2);
        let Ok(crowded) = super::showing(200, TabBar::Shown, 9);

        assert!(nothing > 0);
        assert!(little > 0);
        assert!(crowded > 0, "more rows than the card holds");
    }

    use super::*;

    #[test]
    fn the_room_the_compositor_granted_beats_the_monitor() {
        assert_eq!(x(Room { granted: 800, monitor: 1024 }), shape::part_of(800));
        assert_eq!(
            x(Room { granted: 0, monitor: 1024 }),
            shape::part_of(1024),
            "nothing granted yet"
        );
    }

    #[test]
    fn a_keyboard_taking_the_screen_takes_it_from_the_panel_too() {
        let Ok(share) = shape::tall_part_of(640);

        assert_eq!(ceiling(Room { granted: 300, monitor: 640 }), Ok(300 - 2 * BREATH));
        assert_eq!(
            ceiling(Room { granted: 900, monitor: 640 }),
            Ok(share),
            "its share, where there is room"
        );
        assert_eq!(ceiling(Room { granted: 0, monitor: 640 }), Ok(share), "nothing granted yet");
    }

    #[test]
    fn the_ceiling_is_the_height_whatever_the_rows_are() {
        assert_eq!(tall_enough(Tall { frame: 100, row: 60, ceiling: 400 }), Ok(400));
        assert_eq!(tall_enough(Tall { frame: 100, row: 44, ceiling: 400 }), Ok(400));
    }

    #[test]
    fn there_is_room_for_one_row_however_little_room_there_is() {
        assert_eq!(tall_enough(Tall { frame: 100, row: 60, ceiling: 0 }), Ok(160));
    }
}
