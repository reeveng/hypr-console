//! Where each cover goes in the library, and where a press of the d-pad lands.
//!
//! The covers stand side by side at the proportions of a book, two wide to
//! three tall, as many to a row as the screen holds at a width a thumb can
//! land on, and the rows run down under a line holding the name and the search.
//! How many fit is worked out from the room rather than written down, so the
//! same library is right at every size the desktop is set to.
//!
//! The d-pad walks the covers the way the eye reads them: left and right along
//! a row and on to the next one at the end of it, up and down a column. The
//! library scrolls a row at a time so the selected cover is always all
//! there, which is the only promise a scrolled grid has to keep.

use console_core_geometry::{Point, Size};
use std::num::NonZeroU32;

use console_core_never::Never;
use console_core_number_conversion::fitted;

pub const HEADER: u32 = 76;

pub const MARGIN: u32 = 32;

pub const GAP: u32 = 18;

pub const COVER_WIDE: u32 = 150;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    pub columns: NonZeroU32,
    pub cover: Size<u32>,
    pub left: u32,
    pub rows_seen: NonZeroU32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

const ACROSS: NonZeroU32 = match NonZeroU32::new(COVER_WIDE + GAP) {
    Some(across) => across,
    None => NonZeroU32::MIN,
};

fn at_least_one(many: u32) -> Result<NonZeroU32, Never> {
    Ok(match NonZeroU32::new(many) {
        Some(many) => many,
        None => NonZeroU32::MIN,
    })
}

pub fn layout(room: Size<u32>) -> Result<Grid, Never> {
    let usable = room.width.saturating_sub(MARGIN.saturating_mul(2));
    let Ok(columns) = at_least_one(usable.saturating_add(GAP) / ACROSS);
    let gaps = GAP.saturating_mul(columns.get().saturating_sub(1));
    let wide = usable.saturating_sub(gaps) / columns;
    let tall = wide.saturating_mul(3).saturating_div(2);
    let Ok(row) = at_least_one(tall.saturating_add(GAP));
    let under = room.height.saturating_sub(HEADER).saturating_sub(MARGIN);
    let Ok(rows_seen) = at_least_one(under / row);

    Ok(Grid { columns, cover: Size { width: wide, height: tall }, left: MARGIN, rows_seen })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScrollOffset(pub u32);

pub fn position(grid: Grid, at: u32, scroll_offset: ScrollOffset) -> Result<Point<i32>, Never> {
    let ScrollOffset(scroll_offset) = scroll_offset;
    let column = at % grid.columns;
    let row = (at / grid.columns).saturating_sub(scroll_offset);
    let across = grid.left.saturating_add(column.saturating_mul(grid.cover.width.saturating_add(GAP)));
    let down = HEADER.saturating_add(row.saturating_mul(grid.cover.height.saturating_add(GAP)));
    let Ok(across) = fitted::<u32, i32>(across);
    let Ok(down) = fitted::<u32, i32>(down);

    Ok(Point { x: across, y: down })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub index: u32,
    pub count: u32,
}

pub fn moved(selection: Selection, columns: NonZeroU32, direction: Direction) -> Result<u32, Never> {
    let Selection { index: at, count: many } = selection;
    let last = many.saturating_sub(1);
    let columns = columns.get();

    Ok(match direction {
        Direction::Left => at.saturating_sub(1),
        Direction::Right => at.saturating_add(1).min(last),
        Direction::Up => match at.checked_sub(columns) {
            Some(above) => above,
            None => at,
        },
        Direction::Down => match at.saturating_add(columns) <= last {
            true => at.saturating_add(columns),
            false => at,
        },
    })
}

pub fn scroll_offset(grid: Grid, at: u32, was: ScrollOffset) -> Result<ScrollOffset, Never> {
    let ScrollOffset(was) = was;
    let row = at / grid.columns;
    let seen = grid.rows_seen.get();
    let last_seen = was.saturating_add(seen).saturating_sub(1);

    Ok(match (row < was, row > last_seen) {
        (true, _) => ScrollOffset(row),
        (false, true) => ScrollOffset(row.saturating_sub(seen.saturating_sub(1))),
        (false, false) => ScrollOffset(was),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEVICE: Size<u32> = Size { width: 1280, height: 800 };

    #[test]
    fn the_library_holds_as_many_covers_across_as_fit_at_a_thumbs_width() {
        let Ok(grid) = layout(DEVICE);

        assert_eq!(grid.columns.get(), 7);
        assert!(grid.cover.width >= COVER_WIDE, "a cover is never narrower than the width it was fitted at");
        assert_eq!(grid.cover.height, grid.cover.width.saturating_mul(3).saturating_div(2));
        assert_eq!(grid.rows_seen.get(), 2);
    }

    #[test]
    fn the_dpad_reads_the_library_the_way_the_eye_does() {
        let seven = NonZeroU32::MIN.saturating_add(6);

        assert_eq!(moved(Selection { index: 6, count: 20 }, seven, Direction::Right), Ok(7), "the end of a row goes on to the next one");
        assert_eq!(moved(Selection { index: 19, count: 20 }, seven, Direction::Right), Ok(19), "the last book is the end of the library");
        assert_eq!(moved(Selection { index: 3, count: 20 }, seven, Direction::Up), Ok(3), "the top row has nothing above it");
        assert_eq!(moved(Selection { index: 15, count: 20 }, seven, Direction::Down), Ok(15), "a column that runs out stays put");
        assert_eq!(moved(Selection { index: 10, count: 20 }, seven, Direction::Down), Ok(17));
    }

    #[test]
    fn the_library_scrolls_only_as_far_as_the_cover_being_stood_on() {
        let Ok(grid) = layout(DEVICE);

        assert_eq!(scroll_offset(grid, 15, ScrollOffset(0)), Ok(ScrollOffset(1)), "the third row brings the second to the top");
        assert_eq!(scroll_offset(grid, 9, ScrollOffset(1)), Ok(ScrollOffset(1)), "a row already in sight moves nothing");
        assert_eq!(scroll_offset(grid, 2, ScrollOffset(1)), Ok(ScrollOffset(0)));
    }
}
