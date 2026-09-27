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
