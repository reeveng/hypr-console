//! How many squares the home screen has, and how big one is drawn.
//!
//! None of it was ever a choice. Five across and three down were two constants,
//! and a square's picture was ninety-six logical pixels wherever it was drawn
//! -- which is a number that is only right on one screen at one density. Turn
//! the desktop's size down and the same ninety-six is a third of what it was as
//! a share of the screen; turn it up and fifteen squares no longer fit under
//! the bar. The screen is the thing that changed and the grid did not hear
//! about it.
//!
//! So a square is worked out from the room there is. The pane is divided into
//! cells, a square's picture is a share of the smaller side of a cell, and
//! everything else about the square -- the space round the picture, the plate's
//! corners, the size of the name under it -- is a fraction of the picture. One
//! number moves and the whole square moves with it, which is what makes it the
//! same square at every density.
//!
//! Two things that follows from, both of them learnt the hard way. The room
//! divided is the room the grid is given and not the surface it is drawn on:
//! the stylesheet insets the pane and stands the pane dots under it, and a
//! grid dividing room the stylesheet had already spent puts its bottom row
//! under the edge of the screen. And what has to fit a cell is the square and
//! not the picture: the picture is the largest part of it but the plate's
//! margin and padding and the name's line are the rest, they grow with it, and
//! a rung of the ladder that asks for more than the cell holds is given what
//! the cell holds. `laid` is where both of those land: it answers where one
//! square's plate, picture and name go, so the program that draws them places
//! nothing itself and what it draws can be asserted with no screen.
//!
//! And it is decided once for each way the screen stands. Five across and
//! three down on a panel held wide is three across and five down on the same
//! panel stood on its end, and a person who turns the device and changes the
//! grid is saying what this way up should be rather than correcting the other
//! one -- the argument `console-settings`' `size` makes for the scale, and the
//! same two shapes. The wide one keeps the file it always had, because that is
//! the grid everybody chose before a screen could be turned.
//!
//! And then it is hers to argue with. How many across, how many down, and a
//! ladder of sizes either side of what the room suggests: a person who wants
//! twenty small applications on a pane and a person who wants six large ones
//! are both right about their own screen, and neither of them should have to
//! be told which one this desktop was written by.
//!
//! Nothing here draws or reads a disk. What it is handed is a room in logical
//! pixels and what it answers is numbers, so how big a square comes out on a
//! screen this laptop has not got is a question with an answer here.

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_words::Words;
use console_screen::Shape as Standing;

use crate::Spot;

const NO_ROOM: i32 = 0;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    pub columns: u32,
    pub rows: u32,
    pub size: Size,
}

impl Default for Shape {
    fn default() -> Self {
        Shape::USUAL
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Value<'a>(&'a str);

impl Shape {
    pub const USUAL: Shape = Shape { columns: 5, rows: 3, size: Size::Normal };

    pub const USUAL_TALLER: Shape = Shape { columns: 3, rows: 5, size: Size::Normal };

    pub fn usual(standing: Standing) -> Result<Shape, Never> {
        Ok(match standing {
            Standing::Wider => Shape::USUAL,
            Standing::Taller => Shape::USUAL_TALLER,
        })
    }

    pub const COLUMNS: std::ops::RangeInclusive<u32> = 2..=10;

    pub const ROWS: std::ops::RangeInclusive<u32> = 2..=10;

    pub fn columns_for(standing: Standing) -> Result<std::ops::RangeInclusive<u32>, Never> {
        Ok(match standing {
            Standing::Wider => 3..=9,
            Standing::Taller => 2..=6,
        })
    }

    pub fn rows_for(standing: Standing) -> Result<std::ops::RangeInclusive<u32>, Never> {
        Ok(match standing {
            Standing::Wider => 2..=6,
            Standing::Taller => 3..=10,
        })
    }

    pub fn fitted_to(self, standing: Standing) -> Result<Shape, Never> {
        let Ok(across) = Shape::columns_for(standing);
        let Ok(down) = Shape::rows_for(standing);
        let Ok(columns) = clamped(self.columns, across);
        let Ok(rows) = clamped(self.rows, down);

        Ok(Shape { columns, rows, ..self })
    }

    pub fn with_columns(self, columns: u32) -> Result<Shape, Never> {
        let columns = clamped(columns, Shape::COLUMNS)?;

        Ok(Shape { columns, ..self })
    }

    pub fn with_rows(self, rows: u32) -> Result<Shape, Never> {
        let rows = clamped(rows, Shape::ROWS)?;

        Ok(Shape { rows, ..self })
    }

    pub fn sized(self, size: Size) -> Result<Shape, Never> {
        Ok(Shape { size, ..self })
    }

    pub fn squares(self) -> Result<u32, Never> {
        Ok(self.columns.saturating_mul(self.rows))
    }

    pub fn serialize(self) -> Result<String, Never> {
        let word = self.size.word()?;

        Ok(format!("columns {}\nrows {}\nsize {word}\n", self.columns, self.rows))
    }

    fn with_field(self, word: &str, value: Value<'_>) -> Result<Shape, Never> {
        let value = value.0;

        match word {
            "columns" => match value.parse() {
                Ok(columns) => self.with_columns(columns),
                Err(_not_a_number) => Ok(self),
            },
            "rows" => match value.parse() {
                Ok(rows) => self.with_rows(rows),
                Err(_not_a_number) => Ok(self),
            },
            "size" => {
                let read = Size::read(value)?;

                match read {
                    Some(size) => self.sized(size),
                    None => Ok(self),
                }
            }
            _ => Ok(self),
        }
    }

    pub fn read(said: &str, standing: Standing) -> Result<Shape, Never> {
        let usual = Shape::usual(standing)?;

        let said = said
            .lines()
            .filter_map(|line| line.trim().split_once(char::is_whitespace))
            .try_fold(usual, |shape, (word, value)| shape.with_field(word, Value(value.trim())))?;

        said.fitted_to(standing)
    }
}

fn clamped(asked: u32, range: std::ops::RangeInclusive<u32>) -> Result<u32, Never> {
    Ok(asked.clamp(*range.start(), *range.end()))
}

pub const NAMED: &str = "home-screen";

pub const NAMED_TALLER: &str = "home-screen-taller";

pub fn at(home: &std::path::Path, standing: Standing) -> Result<std::path::PathBuf, Never> {
    let Ok(ours) = console_core_places::Base::Configuration.application_under(home);

    Ok(ours.join(match standing {
        Standing::Wider => NAMED,
        Standing::Taller => NAMED_TALLER,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Size {
    #[words(word = "tiny", says = "Smallest")]
    Tiny,
    #[words(word = "smaller", says = "Smaller")]
    Smaller,
    #[words(word = "normal", says = "Default")]
    Normal,
    #[words(word = "bigger", says = "Larger")]
    Bigger,
    #[words(word = "huge", says = "Largest")]
    Huge,
}

pub const EVERY: [Size; 5] = [Size::Tiny, Size::Smaller, Size::Normal, Size::Bigger, Size::Huge];

impl Size {
    pub fn read(word: &str) -> Result<Option<Size>, Never> {
        for size in EVERY {
            let said = size.word()?;

            match said == word.trim() {
                true => return Ok(Some(size)),
                false => {},
            }
        }

        Ok(None)
    }

    pub fn part(self) -> Result<i32, Never> {
        Ok(match self {
            Size::Tiny => 26,
            Size::Smaller => 32,
            Size::Normal => 38,
            Size::Bigger => 46,
            Size::Huge => 55,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Square {
    pub icon: i32,
    pub named: i32,
    pub padding: i32,
    pub margin: i32,
    pub rounding: i32,
}

pub fn square(room: (i32, i32), shape: Shape) -> Result<Square, Never> {
    let pane = grid(room)?;

    let columns = i32_of(shape.columns)?;

    let rows = i32_of(shape.rows)?;

    let across = match pane.0.checked_div(columns) {
        Some(across) => across,
        None => NO_ROOM,
    };

    let down = match pane.1.checked_div(rows) {
        Some(down) => down,
        None => NO_ROOM,
    };

    let cell = across.min(down).max(0);

    let part = shape.size.part()?;

    let wanted = cell.saturating_mul(part).saturating_div(100);

    let holds = holds(down)?;

    Square::of(wanted.min(holds).max(LEAST))
}

pub fn grid(room: (i32, i32)) -> Result<(i32, i32), Never> {
    let line = dots_line()?;

    Ok((
        room.0.saturating_sub(SIDES.saturating_mul(2)),
        room.1
            .saturating_sub(INSET.saturating_mul(2))
            .saturating_sub(DOTS)
            .saturating_sub(line),
    ))
}

fn holds(cell: i32) -> Result<i32, Never> {
    const OVER: i32 = 2016;
    const UNDER: i32 = 288 + 576 + 2016 + 420;

    let start = cell.saturating_sub(FIXED).saturating_mul(OVER).saturating_div(UNDER).max(0);

    let fits = (LEAST..=start).rev().find(|icon| {
        let Ok(square) = Square::of(*icon);
        let Ok(tall) = square.height();

        tall <= cell
    });

    Ok(match fits {
        Some(icon) => icon,
        None => start.min(LEAST),
    })
}

const FIXED: i32 = BORDER * 2 + SPACING;

pub const BORDER: i32 = 2;

pub const SPACING: i32 = 6;

pub const INSET: i32 = 16;
pub const SIDES: i32 = 40;

pub const DOTS: i32 = 28;

fn dots_line() -> Result<i32, Never> {
    Ok(MOST_WORD.saturating_mul(4).saturating_div(3))
}

impl Square {
    fn of(icon: i32) -> Result<Square, Never> {
        Ok(Square {
            icon,
            named: icon.saturating_mul(15).saturating_div(96).clamp(LEAST_WORD, MOST_WORD),
            padding: icon.saturating_div(7).max(2),
            margin: icon.saturating_div(14).max(1),
            rounding: icon.saturating_div(5).max(4),
        })
    }

    pub fn height(self) -> Result<i32, Never> {
        Ok(self.margin
            .saturating_mul(2)
            .saturating_add(BORDER.saturating_mul(2))
            .saturating_add(self.padding.saturating_mul(2))
            .saturating_add(self.icon)
            .saturating_add(SPACING)
            .saturating_add(self.named.saturating_mul(4).saturating_div(3)))
    }
}

pub const BETWEEN: i32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plate {
    pub at: Point<i32>,
    pub size: console_core_geometry::Size<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub plate: Plate,
    pub icon: Plate,
    pub named: Point<i32>,
    pub line: u32,
}

pub fn layout(room: (i32, i32), shape: Shape, spot: Spot) -> Result<Layout, Never> {
    let drawn = square(room, shape)?;
    let cell = cell(room, shape)?;
    let Ok(column) = i32_of(spot.column.saturating_add(1));
    let column = column.saturating_sub(1);
    let Ok(row) = i32_of(spot.row.saturating_add(1));
    let row = row.saturating_sub(1);
    let tall = drawn.height()?;

    let left = SIDES.saturating_add(cell.0.saturating_mul(column));
    let top = INSET
        .saturating_add(cell.1.saturating_mul(row))
        .saturating_add(cell.1.saturating_sub(tall).saturating_div(2));

    let at = Point {
        x: left.saturating_add(drawn.margin),
        y: top.saturating_add(drawn.margin),
    };
    let wide = up(cell.0.saturating_sub(drawn.margin.saturating_mul(2)))?;
    let deep = up(tall.saturating_sub(drawn.margin.saturating_mul(2)))?;
    let inside = drawn.margin.saturating_add(BORDER).saturating_add(drawn.padding);
    let icon = up(drawn.icon)?;
    let line = up(drawn.named.saturating_mul(4).saturating_div(3))?;
    let icon_left = left.saturating_add(cell.0.saturating_sub(drawn.icon).saturating_div(2));
    let icon_top = top.saturating_add(inside);

    Ok(Layout {
        plate: Plate { at, size: console_core_geometry::Size { width: wide, height: deep } },
        icon: Plate {
            at: Point { x: icon_left, y: icon_top },
            size: console_core_geometry::Size { width: icon, height: icon },
        },
        named: Point {
            x: at.x,
            y: icon_top.saturating_add(drawn.icon).saturating_add(SPACING),
        },
        line,
    })
}

pub fn cell(room: (i32, i32), shape: Shape) -> Result<(i32, i32), Never> {
    let pane = grid(room)?;
    let columns = i32_of(shape.columns)?;
    let rows = i32_of(shape.rows)?;

    let across = match pane.0.checked_div(columns) {
        Some(across) => across,
        None => NO_ROOM,
    };

    let down = match pane.1.checked_div(rows) {
        Some(down) => down,
        None => NO_ROOM,
    };

    Ok((across.max(0), down.max(0)))
}

pub fn over(room: (i32, i32), at: Point<i32>) -> Result<crate::On, Never> {
    let pane = grid(room)?;
    let right = SIDES.saturating_add(pane.0);
    let bottom = INSET.saturating_add(pane.1);

    let inside =
        at.x >= SIDES && at.x < right && at.y >= INSET && at.y < bottom;

    Ok(match inside {
        true => crate::On::TheGrid,
        false => crate::On::None,
    })
}

pub fn dotted(room: (i32, i32)) -> Result<Point<i32>, Never> {
    let pane = grid(room)?;

    Ok(Point {
        x: room.0.saturating_div(2),
        y: INSET.saturating_add(pane.1).saturating_add(INSET),
    })
}

fn up(many: i32) -> Result<u32, Never> {
    console_core_number_conversion::fitted(many.max(0))
}

const LEAST: i32 = 24;

const LEAST_WORD: i32 = 10;
const MOST_WORD: i32 = 22;

fn i32_of(many: u32) -> Result<i32, Never> {
    console_core_number_conversion::fitted(many.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::On;

    fn out(many: u32) -> Result<i32, Never> {
        console_core_number_conversion::fitted(many)
    }

    const ROOM: (i32, i32) = (1024, 600);

    const ROOMS: [((i32, i32), Standing); 2] = [((1024, 600), Standing::Wider), ((600, 1024), Standing::Taller)];

    #[test]
    fn a_machine_no_one_has_asked_gets_the_grid_the_home_screen_was_written_as() {
        assert_eq!(Shape::USUAL.columns, 5);
        assert_eq!(Shape::USUAL.rows, 3);
        assert_eq!(Shape::default(), Shape::USUAL);
    }

    #[test]
    fn a_square_is_a_share_of_the_room_and_not_a_number_of_pixels() {
        let Ok(pane) = grid(ROOM);
        let Ok(here) = square(ROOM, Shape::USUAL);
        let Ok(denser) = square((ROOM.0.saturating_add(pane.0), ROOM.1.saturating_add(pane.1)), Shape::USUAL);

        assert!(
            denser.icon.abs_diff(here.icon.saturating_mul(2)) <= 1,
            "{denser:?} is not twice {here:?}"
        );
        assert!(
            denser.padding.abs_diff(here.padding.saturating_mul(2)) <= 1,
            "{denser:?} against {here:?}"
        );
    }

    #[test]
    fn the_usual_square_is_smaller_than_the_ninety_six_it_used_to_be() {
        let Ok(usual) = square(ROOM, Shape::USUAL);

        assert!(usual.icon < 96);
    }

    #[test]
    fn dividing_the_room_further_draws_them_smaller() {
        let Ok(wider) = Shape::USUAL.with_columns(8);
        let Ok(deeper) = Shape::USUAL.with_rows(5);
        let Ok(five) = square(ROOM, Shape::USUAL);
        let Ok(eight) = square(ROOM, wider);
        let Ok(deeper) = square(ROOM, deeper);
        let (five, eight, deeper) = (five.icon, eight.icon, deeper.icon);

        assert!(eight < five, "{eight} is not under {five}");
        assert!(deeper < five, "{deeper} is not under {five}");
    }

    #[test]
    fn every_rung_of_the_ladder_is_bigger_than_the_one_below_it() {
        let mut sizes: Vec<i32> = Vec::new();

        for size in EVERY {
            let Ok(shape) = Shape::USUAL.sized(size);
            let Ok(drawn) = square(ROOM, shape);

            sizes.push(drawn.icon);
        }

        assert!(sizes.is_sorted_by(|smaller, bigger| smaller < bigger), "{sizes:?}");
    }

    #[test]
    fn a_square_keeps_its_proportions_at_every_size() {
        for size in EVERY {
            for columns in Shape::COLUMNS {
                let Ok(wide) = Shape::USUAL.with_columns(columns);
                let Ok(shape) = wide.sized(size);
                let Ok(drawn) = square(ROOM, shape);

                assert!(drawn.icon >= LEAST, "{size:?} {columns}: {drawn:?}");
                assert!(drawn.padding < drawn.icon, "{size:?} {columns}: {drawn:?}");
                assert!(drawn.named >= LEAST_WORD, "{size:?} {columns}: {drawn:?}");
                assert!(drawn.named <= MOST_WORD, "{size:?} {columns}: {drawn:?}");
            }
        }
    }

    #[test]
    fn a_whole_square_fits_the_cell_it_is_drawn_in() {
        let mut over = Vec::new();

        for (room, standing) in ROOMS {
            let Ok(pane) = grid(room);
            let Ok(across) = Shape::columns_for(standing);
            let Ok(down) = Shape::rows_for(standing);

            for size in EVERY {
                for columns in across.clone() {
                    for rows in down.clone() {
                        let Ok(wide) = Shape::USUAL.with_columns(columns);
                        let Ok(deep) = wide.with_rows(rows);
                        let Ok(shape) = deep.sized(size);
                        let Ok(drawn) = square(room, shape);
                        let Ok(many) = i32_of(rows);
                        let cell = pane.1.div_euclid(many);
                        let Ok(tall) = drawn.height();

                        match tall > cell {
                            true => over.push(format!("{standing:?} {size:?} {columns}x{rows}: {tall} tall in a cell of {cell}")),
                            false => {}
                        }
                    }
                }
            }
        }

        assert!(over.is_empty(), "squares taller than their cell:\n  {}", over.join("\n  "));
    }

    #[test]
    fn every_square_is_drawn_inside_the_room_it_was_given() {
        for (room, standing) in ROOMS {
            let Ok(across) = Shape::columns_for(standing);
            let Ok(down) = Shape::rows_for(standing);

            for size in EVERY {
                for columns in across.clone() {
                    for rows in down.clone() {
                        let Ok(wide) = Shape::USUAL.with_columns(columns);
                        let Ok(deep) = wide.with_rows(rows);
                        let Ok(shape) = deep.sized(size);

                        for row in 0..rows {
                            for column in 0..columns {
                                let spot = Spot { pane: 0, row, column };
                                let Ok(laid) = layout(room, shape, spot);
                                let Ok(wide) = out(laid.plate.size.width);
                                let Ok(tall) = out(laid.plate.size.height);

                                assert!(laid.plate.at.x >= 0, "{size:?} {spot:?}: {laid:?}");
                                assert!(laid.plate.at.y >= 0, "{size:?} {spot:?}: {laid:?}");
                                assert!(laid.plate.at.x.saturating_add(wide) <= room.0, "{size:?} {spot:?}: {laid:?}");
                                assert!(laid.plate.at.y.saturating_add(tall) <= room.1, "{size:?} {spot:?}: {laid:?}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_square_beside_this_one_does_not_touch_it() {
        let shape = Shape::USUAL;
        let Ok(here) = layout(ROOM, shape, Spot { pane: 0, row: 0, column: 0 });
        let Ok(beside) = layout(ROOM, shape, Spot { pane: 0, row: 0, column: 1 });
        let Ok(under) = layout(ROOM, shape, Spot { pane: 0, row: 1, column: 0 });
        let Ok(wide) = out(here.plate.size.width);
        let Ok(tall) = out(here.plate.size.height);

        assert!(here.plate.at.x.saturating_add(wide) < beside.plate.at.x, "{here:?} {beside:?}");
        assert!(here.plate.at.y.saturating_add(tall) < under.plate.at.y, "{here:?} {under:?}");
    }

    #[test]
    fn the_picture_and_the_name_are_inside_the_plate_they_are_drawn_on() {
        for size in EVERY {
            let Ok(shape) = Shape::USUAL.sized(size);
            let Ok(laid) = layout(ROOM, shape, Spot { pane: 0, row: 1, column: 2 });
            let Ok(tall) = out(laid.plate.size.height);
            let Ok(icon) = out(laid.icon.size.height);
            let Ok(line) = out(laid.line);

            assert!(laid.icon.at.y >= laid.plate.at.y, "{size:?}: {laid:?}");
            assert!(laid.icon.at.x >= laid.plate.at.x, "{size:?}: {laid:?}");
            assert!(laid.named.y >= laid.icon.at.y.saturating_add(icon), "{size:?}: {laid:?}");
            assert!(
                laid.named.y.saturating_add(line) <= laid.plate.at.y.saturating_add(tall),
                "{size:?}: {laid:?}"
            );
        }
    }

    #[test]
    fn the_gap_between_two_squares_is_still_the_grid_and_the_margin_round_it_is_not() {
        let shape = Shape::USUAL;
        let Ok(here) = layout(ROOM, shape, Spot { pane: 0, row: 0, column: 0 });
        let Ok(under) = layout(ROOM, shape, Spot { pane: 0, row: 1, column: 0 });
        let Ok(tall) = out(here.plate.size.height);
        let bottom = here.plate.at.y.saturating_add(tall);
        let gap = under.plate.at.y.saturating_sub(bottom);
        let between = Point { x: here.plate.at.x, y: bottom.saturating_add(gap.div_euclid(2)) };

        assert_eq!(over(ROOM, between), Ok(On::TheGrid), "{between:?}");
        assert_eq!(over(ROOM, Point { x: 2, y: 300 }), Ok(On::None));
        assert_eq!(over(ROOM, Point { x: 500, y: ROOM.1.saturating_sub(2) }), Ok(On::None));
    }

    #[test]
    fn the_pane_dots_are_under_the_bottom_row_and_not_over_it() {
        let shape = Shape::USUAL;
        let Ok(bottom) = layout(ROOM, shape, Spot { pane: 0, row: shape.rows.saturating_sub(1), column: 0 });
        let Ok(dots) = dotted(ROOM);
        let Ok(tall) = out(bottom.plate.size.height);

        assert!(dots.y >= bottom.plate.at.y.saturating_add(tall), "{dots:?} over {bottom:?}");
        assert!(dots.y < ROOM.1, "{dots:?} is off the bottom of {ROOM:?}");
        assert_eq!(dots.x, ROOM.0.div_euclid(2));
    }

    #[test]
    fn no_room_at_all_still_draws_something() {
        let Ok(drawn) = square((0, 0), Shape::USUAL);

        assert_eq!(drawn.icon, LEAST);
        assert!(drawn.margin > 0);
    }

    #[test]
    fn a_shape_survives_being_written_down_and_read_back() {
        let Ok(wide) = Shape::USUAL.with_columns(7);
        let Ok(deep) = wide.with_rows(4);
        let Ok(shape) = deep.sized(Size::Bigger);
        let Ok(written) = shape.serialize();

        assert_eq!(Shape::read(&written, Standing::Wider), Ok(shape));
    }

    #[test]
    fn what_the_file_does_not_say_is_the_usual_answer() {
        let Ok(seven) = Shape::read("columns 7", Standing::Wider);

        assert_eq!(Shape::read("", Standing::Wider), Ok(Shape::USUAL));
        assert_eq!(seven.columns, 7);
        assert_eq!(seven.rows, Shape::USUAL.rows);
        assert_eq!(Shape::read("columns seven", Standing::Wider), Ok(Shape::USUAL), "not a number");
        assert_eq!(Shape::read("wallpaper yes", Standing::Wider), Ok(Shape::USUAL), "not ours");
        assert_eq!(Shape::read("size enormous", Standing::Wider), Ok(Shape::USUAL), "not a rung");
    }

    #[test]
    fn a_shape_asked_for_off_the_ends_stays_on_them() {
        assert_eq!(Shape::USUAL.with_columns(0).map(|shape| shape.columns), Ok(*Shape::COLUMNS.start()));
        assert_eq!(Shape::USUAL.with_columns(99).map(|shape| shape.columns), Ok(*Shape::COLUMNS.end()));
        assert_eq!(Shape::USUAL.with_rows(0).map(|shape| shape.rows), Ok(*Shape::ROWS.start()));
        assert_eq!(Shape::USUAL.with_rows(99).map(|shape| shape.rows), Ok(*Shape::ROWS.end()));
        let Ok(wide) = Shape::columns_for(Standing::Wider);

        assert_eq!(Shape::read("columns 200", Standing::Wider).map(|shape| shape.columns), Ok(*wide.end()));
    }

    #[test]
    fn every_rung_reads_back_as_the_word_that_wrote_it() {
        for size in EVERY {
            let Ok(word) = size.word();

            assert_eq!(Size::read(word), Ok(Some(size)));
        }
    }

    #[test]
    fn the_shape_is_written_under_her_own_home() {
        let Ok(at) = at(std::path::Path::new("/home/someone"), Standing::Wider);

        assert!(at.starts_with("/home/someone/.config/console"), "{}", at.display());
    }

    #[test]
    fn a_grid_read_back_is_one_that_fits_the_way_the_screen_stands() {
        let Ok(wide) = Shape::read("columns 10\nrows 10\n", Standing::Wider);
        let Ok(tall) = Shape::read("columns 10\nrows 10\n", Standing::Taller);

        assert_eq!((wide.columns, wide.rows), (9, 6));
        assert_eq!((tall.columns, tall.rows), (6, 10));
    }

    #[test]
    fn a_screen_stood_on_its_end_starts_with_the_grid_turned() {
        let Ok(taller) = Shape::read("", Standing::Taller);

        assert_eq!(taller.columns, Shape::USUAL.rows);
        assert_eq!(taller.rows, Shape::USUAL.columns);
        assert_eq!(Shape::read("columns 4", Standing::Taller).map(|shape| shape.rows), Ok(Shape::USUAL.columns));
    }

    #[test]
    fn each_way_up_keeps_its_own_grid_and_the_wide_one_keeps_the_file_it_always_had() {
        let home = std::path::Path::new("/home/someone");
        let Ok(wider) = at(home, Standing::Wider);
        let Ok(taller) = at(home, Standing::Taller);

        assert_ne!(wider, taller);
        assert_eq!(wider.file_name().and_then(|name| name.to_str()), Some(NAMED));
    }
}
