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
//! the cell holds. Both numbers are written into `home.css` from here, so
//! there is one place to change either of them.
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

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    pub columns: usize,
    pub rows: usize,
    pub size: Size,
}

impl Default for Shape {
    fn default() -> Self {
        Shape::USUAL
    }
}

impl Shape {
    pub const USUAL: Shape = Shape { columns: 5, rows: 3, size: Size::Normal };

    pub const COLUMNS: std::ops::RangeInclusive<usize> = 3..=9;

    pub const ROWS: std::ops::RangeInclusive<usize> = 2..=6;

    pub fn across(self, columns: usize) -> Result<Shape, Never> {
        let columns = clamped(columns, Shape::COLUMNS)?;

        Ok(Shape { columns, ..self })
    }

    pub fn down(self, rows: usize) -> Result<Shape, Never> {
        let rows = clamped(rows, Shape::ROWS)?;

        Ok(Shape { rows, ..self })
    }

    pub fn sized(self, size: Size) -> Result<Shape, Never> {
        Ok(Shape { size, ..self })
    }

    pub fn squares(self) -> Result<usize, Never> {
        Ok(self.columns.saturating_mul(self.rows))
    }

    pub fn written(self) -> Result<String, Never> {
        let word = self.size.word()?;

        Ok(format!("columns {}\nrows {}\nsize {word}\n", self.columns, self.rows))
    }

    fn told(self, word: &str, value: &str) -> Result<Shape, Never> {
        match word {
            "columns" => match value.parse() {
                Ok(columns) => self.across(columns),
                Err(_not_a_number) => Ok(self),
            },
            "rows" => match value.parse() {
                Ok(rows) => self.down(rows),
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

    pub fn read(said: &str) -> Result<Shape, Never> {
        said.lines()
            .filter_map(|line| line.trim().split_once(char::is_whitespace))
            .try_fold(Shape::USUAL, |shape, (word, value)| shape.told(word, value.trim()))
    }
}

fn clamped(asked: usize, range: std::ops::RangeInclusive<usize>) -> Result<usize, Never> {
    Ok(asked.clamp(*range.start(), *range.end()))
}

pub const NAMED: &str = "home-screen";

pub fn at(home: &std::path::Path) -> Result<std::path::PathBuf, Never> {
    let Ok(ours) = console_core_places::Base::Config.ours_under(home);

    Ok(ours.join(NAMED))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Tiny,
    Smaller,
    Normal,
    Bigger,
    Huge,
}

pub const EVERY: [Size; 5] = [Size::Tiny, Size::Smaller, Size::Normal, Size::Bigger, Size::Huge];

impl Size {
    pub fn word(self) -> Result<&'static str, Never> {
        Ok(match self {
            Size::Tiny => "tiny",
            Size::Smaller => "smaller",
            Size::Normal => "normal",
            Size::Bigger => "bigger",
            Size::Huge => "huge",
        })
    }

    pub fn says(self) -> Result<&'static str, Never> {
        Ok(match self {
            Size::Tiny => "Tiny",
            Size::Smaller => "Smaller",
            Size::Normal => "Normal",
            Size::Bigger => "Bigger",
            Size::Huge => "Huge",
        })
    }

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

    let across = pane.0.checked_div(columns).unwrap_or(0);
    let down = pane.1.checked_div(rows).unwrap_or(0);
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

    let mut icon =
        cell.saturating_sub(FIXED).saturating_mul(OVER).saturating_div(UNDER).max(0);

    loop {
        let square = Square::of(icon)?;

        let tall = square.tall()?;

        match icon > LEAST && tall > cell {
            true => icon = icon.saturating_sub(1),
            false => return Ok(icon),
        }
    }
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

    pub fn tall(self) -> Result<i32, Never> {
        Ok(self.margin
            .saturating_mul(2)
            .saturating_add(BORDER.saturating_mul(2))
            .saturating_add(self.padding.saturating_mul(2))
            .saturating_add(self.icon)
            .saturating_add(SPACING)
            .saturating_add(self.named.saturating_mul(4).saturating_div(3)))
    }
}

const LEAST: i32 = 24;

const LEAST_WORD: i32 = 10;
const MOST_WORD: i32 = 22;

fn i32_of(many: usize) -> Result<i32, Never> {
    console_core_number_conversion::fitted(many.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(answer) = answer;

        answer
    }

    const ROOM: (i32, i32) = (1024, 600);

    #[test]
    fn a_machine_nobody_has_asked_gets_the_grid_the_home_screen_was_written_as() {
        assert_eq!(Shape::USUAL.columns, 5);
        assert_eq!(Shape::USUAL.rows, 3);
        assert_eq!(Shape::default(), Shape::USUAL);
    }

    #[test]
    fn a_square_is_a_share_of_the_room_and_not_a_number_of_pixels() {
        let pane = ok(grid(ROOM));
        let here = ok(square(ROOM, Shape::USUAL));
        let denser = ok(square((ROOM.0 + pane.0, ROOM.1 + pane.1), Shape::USUAL));

        assert!((denser.icon - here.icon * 2).abs() <= 1, "{denser:?} is not twice {here:?}");
        assert!((denser.padding - here.padding * 2).abs() <= 1, "{denser:?} against {here:?}");
    }

    #[test]
    fn the_usual_square_is_smaller_than_the_ninety_six_it_used_to_be() {
        assert!(ok(square(ROOM, Shape::USUAL)).icon < 96);
    }

    #[test]
    fn dividing_the_room_further_draws_them_smaller() {
        let five = ok(square(ROOM, Shape::USUAL)).icon;
        let eight = ok(square(ROOM, ok(Shape::USUAL.across(8)))).icon;
        let deeper = ok(square(ROOM, ok(Shape::USUAL.down(5)))).icon;

        assert!(eight < five, "{eight} is not under {five}");
        assert!(deeper < five, "{deeper} is not under {five}");
    }

    #[test]
    fn every_rung_of_the_ladder_is_bigger_than_the_one_below_it() {
        let sizes: Vec<i32> =
            EVERY.iter().map(|size| ok(square(ROOM, ok(Shape::USUAL.sized(*size)))).icon).collect();

        assert!(sizes.windows(2).all(|two| two[0] < two[1]), "{sizes:?}");
    }

    #[test]
    fn a_square_keeps_its_proportions_at_every_size() {
        for size in EVERY {
            for columns in Shape::COLUMNS {
                let wide = ok(Shape::USUAL.across(columns));
                let shape = ok(wide.sized(size));
                let drawn = ok(square(ROOM, shape));

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

        for size in EVERY {
            for columns in Shape::COLUMNS {
                for rows in Shape::ROWS {
                    let wide = ok(Shape::USUAL.across(columns));
                    let deep = ok(wide.down(rows));
                    let shape = ok(deep.sized(size));
                    let drawn = ok(square(ROOM, shape));
                    let cell = ok(grid(ROOM)).1 / ok(i32_of(rows));

                    if ok(drawn.tall()) > cell {
                        over.push(format!(
                            "{size:?} {columns}x{rows}: {} tall in a cell of {cell}",
                            ok(drawn.tall())
                        ));
                    }
                }
            }
        }

        assert!(over.is_empty(), "squares taller than their cell:\n  {}", over.join("\n  "));
    }

    #[test]
    fn no_room_at_all_still_draws_something() {
        let drawn = ok(square((0, 0), Shape::USUAL));

        assert_eq!(drawn.icon, LEAST);
        assert!(drawn.margin > 0);
    }

    #[test]
    fn a_shape_survives_being_written_down_and_read_back() {
        let wide = ok(Shape::USUAL.across(7));
        let deep = ok(wide.down(4));
        let shape = ok(deep.sized(Size::Bigger));
        let written = ok(shape.written());

        assert_eq!(ok(Shape::read(&written)), shape);
    }

    #[test]
    fn what_the_file_does_not_say_is_the_usual_answer() {
        assert_eq!(ok(Shape::read("")), Shape::USUAL);
        assert_eq!(ok(Shape::read("columns 7")).columns, 7);
        assert_eq!(ok(Shape::read("columns 7")).rows, Shape::USUAL.rows);
        assert_eq!(ok(Shape::read("columns seven")), Shape::USUAL, "not a number");
        assert_eq!(ok(Shape::read("wallpaper yes")), Shape::USUAL, "not ours");
        assert_eq!(ok(Shape::read("size enormous")), Shape::USUAL, "not a rung");
    }

    #[test]
    fn a_shape_asked_for_off_the_ends_stays_on_them() {
        assert_eq!(ok(Shape::USUAL.across(0)).columns, *Shape::COLUMNS.start());
        assert_eq!(ok(Shape::USUAL.across(99)).columns, *Shape::COLUMNS.end());
        assert_eq!(ok(Shape::USUAL.down(0)).rows, *Shape::ROWS.start());
        assert_eq!(ok(Shape::USUAL.down(99)).rows, *Shape::ROWS.end());
        assert_eq!(ok(Shape::read("columns 200")).columns, *Shape::COLUMNS.end());
    }

    #[test]
    fn every_rung_reads_back_as_the_word_that_wrote_it() {
        for size in EVERY {
            assert_eq!(ok(Size::read(ok(size.word()))), Some(size));
            assert_eq!(ok(size.says()).to_lowercase(), ok(size.word()));
        }
    }

    #[test]
    fn the_shape_is_written_under_her_own_home() {
        let at = ok(at(std::path::Path::new("/home/somebody")));

        assert!(at.starts_with("/home/somebody/.config/console"), "{}", at.display());
    }
}
