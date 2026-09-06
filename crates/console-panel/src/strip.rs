//! Which tabs the strip has room for.
//!
//! The card is one width whatever is written on its tabs, so a strip of five
//! long words cannot ask for more than the card was going to be. What will not
//! fit is reached with the arrows at either end, which are the shoulders said
//! in the other language.


use console_never::Never;
use console_number_conversion::fitted;
use std::ops::Range;

pub const GAP: i32 = 4;
pub const MARGIN: i32 = 14;
pub const PAD: i32 = 6;

pub const PICTURE: i32 = 32;

pub const SLEEVE: i32 = 176;

pub const ANSWER: i32 = 150;

pub const PRESSED: i32 = 30;

pub const EDGE: i32 = 3;

pub fn room(wide: i32, spent: i32) -> Result<i32, Never> {
    Ok(wide
        .saturating_sub(2i32.saturating_mul(EDGE))
        .saturating_sub(2i32.saturating_mul(MARGIN))
        .saturating_sub(2i32.saturating_mul(PAD))
        .saturating_sub(spent))
}

pub fn fits(room: i32, cell: i32) -> Result<usize, Never> {
    let each = cell.saturating_add(GAP);

    let Ok(many) = fitted(room.saturating_add(GAP).saturating_div(each).max(1));

    Ok(match each > 0 {
        true => many,
        false => 1,
    })
}

pub fn showing(tabs: usize, here: usize, from: usize, fits: usize) -> Result<Range<usize>, Never> {
    match fits >= tabs {
        true => return Ok(0..tabs),
        false => {},
    }

    let first = from.min(here).max(here.saturating_add(1).saturating_sub(fits));
    let first = first.min(tabs.saturating_sub(fits));

    Ok(first..first.saturating_add(fits))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Tab(usize),
    Out,
}

pub fn along(tabs: usize, from: Stop, step: i32) -> Result<Stop, Never> {
    let Ok(out) = fitted::<usize, i32>(tabs);

    let at = match from {
        Stop::Tab(index) => {
            let Ok(index) = fitted::<usize, i32>(index);

            index.min(out)
        }
        Stop::Out => out,
    };

    Ok(match at.saturating_add(step).clamp(0, out) {
        going if going == out => Stop::Out,
        going => {
            let Ok(going) = fitted(going);

            Stop::Tab(going)
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_strip_with_room_for_all_of_them_starts_at_the_first() {
        assert_eq!(showing(5, 3, 0, 5), Ok(0..5));
        assert_eq!(showing(5, 3, 2, 9), Ok(0..5));
    }

    #[test]
    fn the_run_moves_as_little_as_it_can() {
        assert_eq!(showing(5, 0, 0, 3), Ok(0..3), "standing on the first");
        assert_eq!(showing(5, 2, 0, 3), Ok(0..3), "the third is already showing");
        assert_eq!(showing(5, 3, 0, 3), Ok(1..4), "one step, because it had to");
        assert_eq!(showing(5, 1, 2, 3), Ok(1..4), "back the other way, one step");
    }

    #[test]
    fn the_run_never_hangs_off_either_end() {
        assert_eq!(showing(5, 4, 0, 3), Ok(2..5));
        assert_eq!(showing(5, 4, 9, 3), Ok(2..5));
    }

    #[test]
    fn a_tab_and_a_gap_is_what_a_tab_costs() {
        assert_eq!(fits(100, 20), Ok(4), "four tabs and the gaps between them");
        assert_eq!(fits(0, 20), Ok(1), "somewhere to stand, whatever the room");
    }

    #[test]
    fn the_room_is_the_card_less_everything_that_is_not_a_tab() {
        assert_eq!(room(900, 120), Ok(900 - 2 * EDGE - 2 * MARGIN - 2 * PAD - 120));
    }

    #[test]
    fn a_shoulder_walks_the_tabs_and_then_the_way_out() {
        assert_eq!(along(3, Stop::Tab(0), 1), Ok(Stop::Tab(1)));
        assert_eq!(along(3, Stop::Tab(2), 1), Ok(Stop::Out));
        assert_eq!(along(3, Stop::Out, -1), Ok(Stop::Tab(2)));
    }

    #[test]
    fn the_walk_stops_at_both_ends() {
        assert_eq!(along(3, Stop::Tab(0), -1), Ok(Stop::Tab(0)));
        assert_eq!(along(3, Stop::Out, 1), Ok(Stop::Out));
    }

    #[test]
    fn the_way_out_is_there_on_a_panel_with_one_tab() {
        assert_eq!(along(1, Stop::Tab(0), 1), Ok(Stop::Out));
        assert_eq!(along(1, Stop::Out, -1), Ok(Stop::Tab(0)));
    }
}
