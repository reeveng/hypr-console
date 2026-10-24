//! Which rows of a note are on which page.
//!
//! A note is written on a page the way a book is read on one, rather than on
//! a strip that scrolls: the rows are stacked until the next one would not fit
//! under the last, and that one starts the next page. The page shown is the
//! one holding the caret, so writing past the foot of a page turns it, the way
//! a typewriter's carriage would have wanted to. A row taller than a whole page
//! still goes on one, alone, rather than being a page nobody can reach.
//!
//! The rows' heights are handed in, because how tall a row is depends on the
//! face it is set in, and nothing here has heard of one.

use std::ops::Range;

use console_core_never::Never;

pub fn pages(heights: &[u32], room: u32) -> Result<Vec<Range<u32>>, Never> {
    let mut found: Vec<Range<u32>> = Vec::new();
    let mut from = 0_u32;
    let mut used = 0_u32;

    for (at, tall) in (0_u32..).zip(heights.iter().copied()) {
        let past = used.saturating_add(tall);

        match (past > room, at > from) {
            (true, true) => {
                found.push(from..at);
                from = at;
                used = tall;
            }
            (true, false) | (false, _) => used = past,
        }
    }

    let Ok(count) = console_core_number_conversion::fitted::<_, u32>(heights.len());

    found.push(from..count);

    Ok(found)
}

pub fn holding(pages: &[Range<u32>], row: u32) -> Result<u32, Never> {
    let found = (0_u32..).zip(pages).find(|(_, page)| page.contains(&row));

    Ok(match found {
        Some((at, _)) => at,
        None => {
            let Ok(count) = console_core_number_conversion::fitted::<_, u32>(pages.len());

            count.saturating_sub(1)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_stacked_until_the_next_would_not_fit() {
        assert_eq!(pages(&[30, 20, 20, 20, 20], 60), Ok(vec![0..2, 2..5]));
    }

    #[test]
    fn a_row_taller_than_the_page_is_a_page_of_its_own() {
        assert_eq!(pages(&[20, 90, 20], 60), Ok(vec![0..1, 1..2, 2..3]));
    }

    #[test]
    fn nothing_written_is_one_empty_page() {
        let Ok(stacked) = pages(&[], 60);

        assert_eq!(stacked.len(), 1);
        assert_eq!(stacked.first().map(ExactSizeIterator::len), Some(0));
    }

    #[test]
    fn the_page_shown_is_the_one_holding_the_caret() {
        let Ok(stacked) = pages(&[20, 20, 20, 20], 40);

        assert_eq!(holding(&stacked, 0), Ok(0));
        assert_eq!(holding(&stacked, 3), Ok(1));
        assert_eq!(holding(&stacked, 9), Ok(1), "a row past the end is on the last page");
    }
}
