//! Which tabs the strip has room for.
//!
//! The card is one width whatever is written on its tabs, so a strip of five
//! long words cannot ask for more than the card was going to be. What will not
//! fit is reached with the arrows at either end, which are the shoulders said
//! in the other language.


use console_core_fonts::EM;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::ops::Range;

pub const GAP: i32 = EM * 2 / 9;
pub const MARGIN: i32 = EM * 7 / 9;
pub const PAD: i32 = EM / 3;

pub const PICTURE: i32 = EM * 16 / 9;

pub const SLEEVE: i32 = EM * 88 / 9;

pub const ANSWER: i32 = EM * 25 / 3;

pub const PRESSED: i32 = EM * 5 / 3;

pub const EDGE: i32 = EM / 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card {
    pub width: i32,
    pub spent: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tabs {
    pub many: u32,
    pub here: u32,
    pub from: u32,
    pub fits: u32,
}

pub fn room(card: Card) -> Result<i32, Never> {
    Ok(card
        .width
        .saturating_sub(2i32.saturating_mul(EDGE))
        .saturating_sub(2i32.saturating_mul(MARGIN))
        .saturating_sub(2i32.saturating_mul(PAD))
        .saturating_sub(card.spent))
}

pub fn fits(room: i32, cell: Cell) -> Result<u32, Never> {
    let each = cell.0.saturating_add(GAP);

    let Ok(many) = fitted(room.saturating_add(GAP).saturating_div(each).max(1));

    Ok(match each > 0 {
        true => many,
        false => 1,
    })
}

pub fn showing(tabs: Tabs) -> Result<Range<u32>, Never> {
    match tabs.fits >= tabs.many {
        true => return Ok(0..tabs.many),
        false => {},
    }

    let least = tabs.here.saturating_add(1).saturating_sub(tabs.fits);
    let first = tabs.from.min(tabs.here).max(least);
    let first = first.min(tabs.many.saturating_sub(tabs.fits));

    Ok(first..first.saturating_add(tabs.fits))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Tab(u32),
    Outside,
}

pub fn along(tabs: u32, from: Stop, step: i32) -> Result<Stop, Never> {
    let Ok(out) = fitted::<u32, i32>(tabs);

    let at = match from {
        Stop::Tab(index) => {
            let Ok(index) = fitted::<u32, i32>(index);

            index.min(out)
        }
        Stop::Outside => out,
    };

    let going = at.saturating_add(step).clamp(0, out);

    Ok(match going == out {
        true => Stop::Outside,
        false => {
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
        assert_eq!(showing(Tabs { many: 5, here: 3, from: 0, fits: 5 }), Ok(0..5));
        assert_eq!(showing(Tabs { many: 5, here: 3, from: 2, fits: 9 }), Ok(0..5));
    }

    #[test]
    fn the_run_moves_as_little_as_it_can() {
        let run = |here, from| showing(Tabs { many: 5, here, from, fits: 3 });

        assert_eq!(run(0, 0), Ok(0..3), "standing on the first");
        assert_eq!(run(2, 0), Ok(0..3), "the third is already showing");
        assert_eq!(run(3, 0), Ok(1..4), "one step, because it had to");
        assert_eq!(run(1, 2), Ok(1..4), "back the other way, one step");
    }

    #[test]
    fn the_run_never_hangs_off_either_end() {
        assert_eq!(showing(Tabs { many: 5, here: 4, from: 0, fits: 3 }), Ok(2..5));
        assert_eq!(showing(Tabs { many: 5, here: 4, from: 9, fits: 3 }), Ok(2..5));
    }

    #[test]
    fn a_tab_and_a_gap_is_what_a_tab_costs() {
        assert_eq!(fits(100, Cell(20)), Ok(4), "four tabs and the gaps between them");
        assert_eq!(fits(0, Cell(20)), Ok(1), "somewhere to stand, whatever the room");
    }

    #[test]
    fn the_room_is_the_card_less_everything_that_is_not_a_tab() {
        assert_eq!(
            room(Card { width: 900, spent: 120 }),
            Ok(900 - 2 * EDGE - 2 * MARGIN - 2 * PAD - 120)
        );
    }

    #[test]
    fn a_shoulder_walks_the_tabs_and_then_the_way_out() {
        assert_eq!(along(3, Stop::Tab(0), 1), Ok(Stop::Tab(1)));
        assert_eq!(along(3, Stop::Tab(2), 1), Ok(Stop::Outside));
        assert_eq!(along(3, Stop::Outside, -1), Ok(Stop::Tab(2)));
    }

    #[test]
    fn the_walk_stops_at_both_ends() {
        assert_eq!(along(3, Stop::Tab(0), -1), Ok(Stop::Tab(0)));
        assert_eq!(along(3, Stop::Outside, 1), Ok(Stop::Outside));
    }

    #[test]
    fn the_way_out_is_there_on_a_panel_with_one_tab() {
        assert_eq!(along(1, Stop::Tab(0), 1), Ok(Stop::Outside));
        assert_eq!(along(1, Stop::Outside, -1), Ok(Stop::Tab(0)));
    }
}
