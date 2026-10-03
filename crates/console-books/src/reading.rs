//! Where a page turn goes.
//!
//! A book is sections of pages: an EPUB's chapters, each laid out into as many
//! pages as it takes; a PDF's or a comic's pages, each a section of one page.
//! Forward is the next page, and past the last page of a section the first
//! page of the next one. Back is the page before, and before the first page of
//! a section it is the *last* page of the one before, which is the turn a
//! printed book makes and the one a reader that paged by chapter gets wrong.
//! The ends of the book are where a turn goes nowhere.
//!
//! How many pages the section being turned into has is not known until it has
//! been laid out, so a turn into another section says which end of it to stand
//! on rather than a page number.
//!
//! A turn is asked for by a key, by the small bar at either side of the page,
//! or by a swipe. A tap anywhere else on the page turns nothing, because the
//! page is where a note is written. A swipe is a finger that went further
//! sideways than down, and far enough that it was not a tap that slid: pulled
//! to the left it brings the next page in, the way paper does.

use console_core_geometry::Point;
use console_core_never::Never;

use crate::progress::Position;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turn {
    Forward,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    First,
    Last,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Page(u32),
    Section { section: u32, end: End },
    AtEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PagePosition {
    pub section: Position,
    pub page: Position,
}

const SWIPE: u32 = 60;

pub fn swipe(from: Point<i32>, to: Point<i32>) -> Result<Option<Turn>, Never> {
    let across = to.x.saturating_sub(from.x);
    let down = to.y.saturating_sub(from.y);
    let sideways = across.unsigned_abs() > down.unsigned_abs();
    let far = across.unsigned_abs() >= SWIPE;

    Ok(match (sideways && far, across < 0) {
        (true, true) => Some(Turn::Forward),
        (true, false) => Some(Turn::Back),
        (false, _) => None,
    })
}

pub fn turn(spot: PagePosition, turn: Turn) -> Result<Destination, Never> {
    let PagePosition { section, page } = spot;
    let next_page = page.index.saturating_add(1);
    let next_section = section.index.saturating_add(1);

    Ok(match turn {
        Turn::Forward => match (next_page < page.count, next_section < section.count) {
            (true, _) => Destination::Page(next_page),
            (false, true) => Destination::Section { section: next_section, end: End::First },
            (false, false) => Destination::AtEnd,
        },
        Turn::Back => match (page.index.checked_sub(1), section.index.checked_sub(1)) {
            (Some(before), _) => Destination::Page(before),
            (None, Some(before)) => Destination::Section { section: before, end: End::Last },
            (None, None) => Destination::AtEnd,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_is_the_next_page_and_then_the_next_chapter() {
        let turns = [
            (0, 1, Destination::Page(2)),
            (0, 3, Destination::Section { section: 1, end: End::First }),
            (2, 3, Destination::AtEnd),
        ];

        for (section, page, destination) in turns {
            let here = PagePosition { section: Position { index: section, count: 3 }, page: Position { index: page, count: 4 } };

            assert_eq!(turn(here, Turn::Forward), Ok(destination), "forward from page {page} of chapter {section}");
        }
    }

    #[test]
    fn a_swipe_left_is_the_next_page_and_a_tap_that_slid_is_no_swipe() {
        let from = Point { x: 500, y: 300 };

        assert_eq!(swipe(from, Point { x: 380, y: 320 }), Ok(Some(Turn::Forward)));
        assert_eq!(swipe(from, Point { x: 620, y: 280 }), Ok(Some(Turn::Back)));
        assert_eq!(swipe(from, Point { x: 520, y: 305 }), Ok(None), "a finger that barely moved tapped");
        assert_eq!(swipe(from, Point { x: 400, y: 450 }), Ok(None), "a stroke further down than across is not a turn");
    }

    #[test]
    fn back_from_the_start_of_a_chapter_is_the_end_of_the_one_before() {
        let turns = [
            (1, 2, Destination::Page(1)),
            (1, 0, Destination::Section { section: 0, end: End::Last }),
            (0, 0, Destination::AtEnd),
        ];

        for (section, page, destination) in turns {
            let here = PagePosition { section: Position { index: section, count: 3 }, page: Position { index: page, count: 4 } };

            assert_eq!(turn(here, Turn::Back), Ok(destination), "back from page {page} of chapter {section}");
        }
    }
}
