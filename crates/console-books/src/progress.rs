//! Where each book was left.
//!
//! A book is opened where it was put down, which is the one thing a reader has
//! to get right before anything else. What is kept is the section of the book --
//! the chapter of an EPUB, the page of a PDF or a comic -- and how far through
//! that section's text the page begins, in ten-thousandths rather than as a
//! page number: how many pages a chapter is depends on the face, the size and
//! the screen, and a page number kept from one of those is the wrong page on
//! any other. Measured in letters rather than in pages, the same place comes
//! back as the page holding the letter the old page began with, which is the
//! very page when nothing about the screen has changed. The share is rounded up
//! and read back rounded down, so a page never comes back as the one before it.
//!
//! The library draws how far through each book is on its cover, so that is kept
//! too, worked out when the page was turned rather than by opening every book
//! in the library to ask it.
//!
//! One line a book, named by its file, in the state folder: it is something
//! this machine remembers about what somebody did, rather than a setting and
//! rather than something that can be made again. Beside it is the book that
//! was open, so that the reader comes back to it rather than to the library
//! after a restart; it is taken away when the book is closed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_places::Base;

pub const WHOLE: u32 = 10_000;

pub const NAMED: &str = "books";

pub const OPEN: &str = "books-open";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Location {
    pub section: u32,
    pub fraction: u32,
    pub percent: u32,
}

pub fn path(home: &Path) -> Result<PathBuf, Never> {
    let Ok(ours) = Base::State.ours_under(home);

    Ok(ours.join(NAMED))
}

pub fn open_path(home: &Path) -> Result<PathBuf, Never> {
    let Ok(ours) = Base::State.ours_under(home);

    Ok(ours.join(OPEN))
}

fn parse_line(text: &str) -> Result<Option<(String, Location)>, Never> {
    let mut words = text.splitn(4, ' ');

    let (percent, section, through, name) = match (words.next(), words.next(), words.next(), words.next()) {
        (Some(percent), Some(section), Some(through), Some(name)) => (percent, section, through, name),
        (None, _, _, _) | (_, None, _, _) | (_, _, None, _) | (_, _, _, None) => return Ok(None),
    };

    Ok(match (percent.parse::<u32>(), section.parse::<u32>(), through.parse::<u32>()) {
        (Ok(percent), Ok(section), Ok(through)) => Some((name.to_string(), Location { section, fraction: through, percent })),
        (Err(_not_a_number), _, _) | (_, Err(_not_a_number), _) | (_, _, Err(_not_a_number)) => None,
    })
}

pub fn parse(text: &str) -> Result<BTreeMap<String, Location>, Never> {
    let mut found = BTreeMap::new();

    for text in text.lines() {
        let Ok(read) = parse_line(text);

        match read {
            Some((name, location)) => {
                found.insert(name, location);
            },
            None => {},
        }
    }

    Ok(found)
}

pub fn serialize(locations: &BTreeMap<String, Location>) -> Result<String, Never> {
    let mut text = String::new();

    for (name, location) in locations {
        text.push_str(&format!("{} {} {} {name}\n", location.percent, location.section, location.fraction));
    }

    Ok(text)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub index: u32,
    pub count: u32,
}

pub fn fraction(position: Position) -> Result<u32, Never> {
    let Position { index: page, count: pages } = position;

    let through = u64::from(page).saturating_mul(u64::from(WHOLE)).checked_div(u64::from(pages));

    Ok(match through.map(u32::try_from) {
        None => 0,
        Some(Ok(through)) => through,
        Some(Err(_past_whole)) => WHOLE,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fraction(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reached {
    pub before: u64,
    pub total: u64,
}

pub fn through_text(reached: Reached) -> Result<u32, Never> {
    let Reached { before, total } = reached;
    let rounded_up = before.saturating_mul(u64::from(WHOLE)).saturating_add(total.saturating_sub(1)).checked_div(total);

    Ok(match rounded_up.map(u32::try_from) {
        None => 0,
        Some(Ok(through)) => through.min(WHOLE),
        Some(Err(_past_whole)) => WHOLE,
    })
}

pub fn page_holding(through: Fraction, letters: &[u64]) -> Result<u32, Never> {
    let Fraction(through) = through;
    let total = letters.iter().fold(0_u64, |sum, page| sum.saturating_add(*page));
    let wanted = u64::from(through).saturating_mul(total);
    let mut start = 0_u64;
    let mut begun = 0_u32;

    for page in letters {
        match start.saturating_mul(u64::from(WHOLE)) <= wanted {
            true => begun = begun.saturating_add(1),
            false => {},
        }

        start = start.saturating_add(*page);
    }

    Ok(begun.saturating_sub(1))
}

pub fn percent(position: Position, through: Fraction) -> Result<u32, Never> {
    let (Position { index: section, count: sections }, Fraction(through)) = (position, through);
    let done = u64::from(section).saturating_mul(u64::from(WHOLE)).saturating_add(u64::from(through));
    let whole = u64::from(sections).saturating_mul(u64::from(WHOLE));

    Ok(match done.saturating_mul(100).checked_div(whole) {
        Some(percent) => u32::try_from(percent).map_or(100, |percent| percent.min(100)),
        None => 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_is_written_and_read_back_with_a_name_that_has_spaces_in_it() {
        let mut locations = BTreeMap::new();
        locations.insert("The Great Gatsby.epub".to_string(), Location { section: 3, fraction: 2500, percent: 18 });

        let Ok(text) = serialize(&locations);

        assert_eq!(parse(&text), Ok(locations));
    }

    #[test]
    fn a_line_nobody_can_read_is_left_out_rather_than_read_as_the_start() {
        assert_eq!(parse("twelve 1 2 Moby Dick.epub\n"), Ok(BTreeMap::new()));
    }

    fn kept_and_found(page: u32, letters: &[u64]) -> Result<u32, Never> {
        let Ok(page) = console_core_number_conversion::index(page);
        let before = letters.iter().take(page).fold(0_u64, |sum, page| sum.saturating_add(*page));
        let total = letters.iter().fold(0_u64, |sum, page| sum.saturating_add(*page));
        let Ok(kept) = through_text(Reached { before, total });

        page_holding(Fraction(kept), letters)
    }

    #[test]
    fn every_page_comes_back_as_itself_when_nothing_about_the_screen_changed() {
        let letters = [1200, 1180, 1, 1230, 900, 1215, 40];

        for page in 0..7 {
            assert_eq!(kept_and_found(page, &letters), Ok(page), "page {page}");
        }
    }

    #[test]
    fn the_same_place_in_a_chapter_laid_out_larger_is_the_page_holding_its_first_letter() {
        let Ok(kept) = through_text(Reached { before: 2500, total: 10_000 });

        assert_eq!(page_holding(Fraction(kept), &[2000, 2000, 2000, 2000, 2000]), Ok(1));
        assert_eq!(page_holding(Fraction(WHOLE), &[10, 10, 10]), Ok(2), "the end is the last page and not one past it");
        assert_eq!(page_holding(Fraction(0), &[]), Ok(0));
    }

    #[test]
    fn the_percent_is_the_parts_read_and_the_share_of_the_one_open() {
        assert_eq!(percent(Position { index: 1, count: 4 }, Fraction(5000)), Ok(37));
        assert_eq!(percent(Position { index: 0, count: 0 }, Fraction(0)), Ok(0));
    }
}
