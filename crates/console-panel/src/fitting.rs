//! How much room the panel has, and how much of it a whole number of rows
//! fills.
//!
//! The surface is anchored to all four edges and claims no exclusive zone of
//! its own, so what the compositor grants is the screen less whatever else has
//! taken a piece of it: the bar, and the on-screen keyboard while it is up. The
//! panel is measured against that room rather than against the screen, so it
//! never has to know that either exists.

use crate::shape;
use console_core_never::Never;

pub const BREATH: i32 = 16;

pub const UNDER: i32 = 107;

pub const ROW: i32 = 45;

pub const EDGES: i32 = UNDER - 2 * ROW;

pub const STRIP: i32 = 82;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strip {
    Shown,
    Hidden,
}

pub fn showing(card: i32, strip: Strip, under: i32) -> Result<i32, Never> {
    let band = match strip {
        Strip::Shown => STRIP,
        Strip::Hidden => 0,
    };

    Ok(card
        .saturating_sub(band)
        .saturating_sub(EDGES)
        .saturating_sub(under.max(0).saturating_mul(ROW))
        .max(SMALLEST))
}

const SMALLEST: i32 = 64;

pub fn across(given: i32, monitor: i32) -> Result<i32, Never> {
    shape::part_of(match given > 1 {
        true => given,
        false => monitor,
    })
}

pub fn ceiling(given: i32, screen: i32) -> Result<i32, Never> {
    let Ok(wanted) = shape::tall_part_of(screen);

    Ok(match given > 1 {
        true => wanted.min(given.saturating_sub(2i32.saturating_mul(BREATH))),
        false => wanted,
    })
}

pub fn tall_enough(frame: i32, row: i32, ceiling: i32) -> Result<i32, Never> {
    Ok(frame.saturating_add(row.max(ceiling.saturating_sub(frame))))
}

#[cfg(test)]
mod tests {
    use super::Strip;

    #[test]
    fn a_picture_on_the_ordinary_card_is_what_it_was_measured_at() {
        assert_eq!(super::showing(461, Strip::Shown, 2), Ok(272));
    }

    #[test]
    fn what_the_rows_do_not_take_is_the_pictures() {
        let Ok(two) = super::showing(461, Strip::Shown, 2);

        assert_eq!(super::showing(461, Strip::Shown, 3), Ok(two - super::ROW));
        assert_eq!(super::showing(461, Strip::Shown, 0), Ok(two + 2 * super::ROW));
    }

    #[test]
    fn opening_the_card_out_gives_the_room_to_the_picture() {
        let Ok(open) = super::showing(600, Strip::Hidden, 2);
        let Ok(shut) = super::showing(461, Strip::Shown, 2);

        assert_eq!(open - shut, 600 - 461 + super::STRIP);
    }

    #[test]
    fn a_card_with_nothing_to_spare_still_draws_something() {
        let Ok(nothing) = super::showing(0, Strip::Shown, 2);
        let Ok(little) = super::showing(200, Strip::Shown, 2);
        let Ok(crowded) = super::showing(200, Strip::Shown, 9);

        assert!(nothing > 0);
        assert!(little > 0);
        assert!(crowded > 0, "more rows than the card holds");
    }

    use super::*;

    #[test]
    fn the_room_the_compositor_granted_beats_the_monitor() {
        assert_eq!(across(800, 1024), shape::part_of(800));
        assert_eq!(across(0, 1024), shape::part_of(1024), "nothing granted yet");
    }

    #[test]
    fn a_keyboard_taking_the_screen_takes_it_from_the_panel_too() {
        let Ok(share) = shape::tall_part_of(640);

        assert_eq!(ceiling(300, 640), Ok(300 - 2 * BREATH));
        assert_eq!(ceiling(900, 640), Ok(share), "its share, where there is room");
        assert_eq!(ceiling(0, 640), Ok(share), "nothing granted yet");
    }

    #[test]
    fn the_ceiling_is_the_height_whatever_the_rows_are() {
        assert_eq!(tall_enough(100, 60, 400), Ok(400));
        assert_eq!(tall_enough(100, 44, 400), Ok(400));
    }

    #[test]
    fn there_is_room_for_one_row_however_little_room_there_is() {
        assert_eq!(tall_enough(100, 60, 0), Ok(160));
    }
}
