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
//! And then it is hers to argue with. How many across, how many down, and a
//! ladder of sizes either side of what the room suggests: a person who wants
//! twenty small applications on a pane and a person who wants six large ones
//! are both right about their own screen, and neither of them should have to
//! be told which one this desktop was written by.
//!
//! Nothing here draws or reads a disk. What it is handed is a room in logical
//! pixels and what it answers is numbers, so how big a square comes out on a
//! screen this laptop has not got is a question with an answer here.

/// How the squares are arranged, and how big they are drawn.
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
    /// What a machine that has never been asked gets.
    ///
    /// The five by three the home screen was written as, so that a device that
    /// upgrades finds its applications where it left them: the file that says
    /// where they are names squares by row and column, and a grid that changed
    /// shape underneath it is a home screen that has been rearranged by an
    /// update.
    pub const USUAL: Shape = Shape { columns: 5, rows: 3, size: Size::Normal };

    /// The fewest across, and the most.
    ///
    /// Two columns is a pane that holds less than the row of icons already on
    /// the bar, and past nine the name under a square is narrower than the
    /// word on it -- every application would read as the first six letters of
    /// itself.
    pub const COLUMNS: std::ops::RangeInclusive<usize> = 3..=9;

    /// And down. One row is a shelf rather than a screen; past six, a square is
    /// shorter than the two lines it has to hold.
    pub const ROWS: std::ops::RangeInclusive<usize> = 2..=6;

    /// The same, with a different number across, and never off the ends.
    ///
    /// Clamped rather than refused, because what asks for this is a press of a
    /// plus or a minus and a press at the end of a range has to do something
    /// that can be seen -- which is to stay where it is.
    pub fn across(self, columns: usize) -> Shape {
        Shape { columns: clamped(columns, Shape::COLUMNS), ..self }
    }

    pub fn down(self, rows: usize) -> Shape {
        Shape { rows: clamped(rows, Shape::ROWS), ..self }
    }

    pub fn sized(self, size: Size) -> Shape {
        Shape { size, ..self }
    }

    /// How many squares one pane holds.
    pub fn squares(self) -> usize {
        self.columns * self.rows
    }

    /// The file, as this says it.
    ///
    /// A word and a number to a line, so a person looking at it can see what it
    /// says and a half-written one is the settings that survived rather than a
    /// home screen that will not draw.
    pub fn written(self) -> String {
        format!("columns {}\nrows {}\nsize {}\n", self.columns, self.rows, self.size.word())
    }

    /// What the file says, with the usual answer for everything it does not.
    ///
    /// Every line it cannot read is a line ignored. This is a preference file
    /// under her own home and nothing checks it: a typed number that is not a
    /// number should be the home screen she had, not the home screen refusing
    /// to come up.
    pub fn read(said: &str) -> Shape {
        let mut shape = Shape::USUAL;

        for line in said.lines() {
            let Some((word, value)) = line.trim().split_once(char::is_whitespace) else {
                continue;
            };

            let value = value.trim();

            match word {
                "columns" => {
                    if let Ok(columns) = value.parse() {
                        shape = shape.across(columns);
                    }
                },
                "rows" => {
                    if let Ok(rows) = value.parse() {
                        shape = shape.down(rows);
                    }
                },
                "size" => {
                    if let Some(size) = Size::read(value) {
                        shape = shape.sized(size);
                    }
                },
                _ => {},
            }
        }

        shape
    }
}

fn clamped(asked: usize, range: std::ops::RangeInclusive<usize>) -> usize {
    asked.clamp(*range.start(), *range.end())
}

/// Where the shape is written down.
///
/// Under her own config rather than in this repository, for the reason
/// `console_settings::size` gives about the compositor's file: what this
/// repository ships is what the device is set up as, and a machine standing
/// somewhere else says so in a file that is nobody's to check.
pub fn at(home: &std::path::Path) -> std::path::PathBuf {
    home.join(".config/console/home-screen")
}

/// How big the squares are, either side of what the room suggests.
///
/// A ladder rather than a number, and the same five words the desktop's own
/// size ladder uses, because it is the same question asked of a smaller thing:
/// somebody who found Bigger on the Size tab and wants the home screen to agree
/// should find the word they already know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Tiny,
    Smaller,
    Normal,
    Bigger,
    Huge,
}

/// Every rung, smallest first, so that walking a list is walking one way along
/// the thing the list measures.
pub const EVERY: [Size; 5] = [Size::Tiny, Size::Smaller, Size::Normal, Size::Bigger, Size::Huge];

impl Size {
    /// The word in the file.
    pub fn word(self) -> &'static str {
        match self {
            Size::Tiny => "tiny",
            Size::Smaller => "smaller",
            Size::Normal => "normal",
            Size::Bigger => "bigger",
            Size::Huge => "huge",
        }
    }

    /// The word on the card, which is the same word with its capital.
    pub fn says(self) -> &'static str {
        match self {
            Size::Tiny => "Tiny",
            Size::Smaller => "Smaller",
            Size::Normal => "Normal",
            Size::Bigger => "Bigger",
            Size::Huge => "Huge",
        }
    }

    /// Read back. Anything else is not one of these.
    pub fn read(word: &str) -> Option<Size> {
        EVERY.iter().copied().find(|size| size.word() == word.trim())
    }

    /// How much of a cell's shorter side the picture takes, out of a hundred.
    ///
    /// Normal is what the home screen is drawn at when nobody has said, and it
    /// is smaller than the ninety-six pixels this used to be: fifteen squares
    /// at that size reached both edges of the screen and left no wallpaper
    /// between the rows, which is a grid rather than applications standing on a
    /// photograph.
    ///
    /// The rungs are about a fifth apart, which is far enough that changing
    /// rung is a change somebody meant to make.
    pub fn part(self) -> i32 {
        match self {
            Size::Tiny => 26,
            Size::Smaller => 32,
            Size::Normal => 38,
            Size::Bigger => 46,
            Size::Huge => 55,
        }
    }
}

/// One square, in logical pixels.
///
/// Everything but the picture is a fraction of the picture, so the square holds
/// its proportions at every size: the plate does not keep a fat border round a
/// small icon, and a large one does not sit in a plate that grips it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Square {
    /// How big the application's picture is drawn.
    pub icon: i32,
    /// The type size of the name under it.
    pub named: i32,
    /// Between the plate's edge and what stands on it.
    pub padding: i32,
    /// Between one plate and the next, which is what makes them plates rather
    /// than one dark sheet with icons on it.
    pub margin: i32,
    /// How round the plate's corners are.
    pub rounding: i32,
}

/// How big a square is, on a pane of that size.
///
/// The room is the whole pane in logical pixels -- what is left of the screen
/// once the bar and the margins have had theirs -- and the shape is how it is
/// divided. The picture is a share of the shorter side of a cell, because a
/// square is as tall as it is wide and the side that runs out first is the one
/// that decides.
///
/// Never nothing. A pane divided into more squares than the screen has room
/// for still draws a picture somebody can see, because a square of no size is
/// a home screen that looks broken rather than one that looks crowded.
pub fn square(room: (i32, i32), shape: Shape) -> Square {
    let across = room.0 / i32_of(shape.columns);
    let down = room.1 / i32_of(shape.rows);
    let cell = across.min(down).max(0);
    let icon = (cell * shape.size.part() / 100).max(LEAST);

    Square {
        icon,
        // Not the same fraction as the rest. A picture can be any size and
        // still be a picture; a word below a certain size is not a word, and
        // the name under a small icon is the half of the square that says
        // which application it is.
        named: (icon * 15 / 96).clamp(LEAST_WORD, MOST_WORD),
        padding: (icon / 7).max(2),
        margin: (icon / 14).max(1),
        rounding: (icon / 5).max(4),
    }
}

/// The smallest a picture is ever drawn, whatever the arithmetic says.
const LEAST: i32 = 24;

/// And the bounds on the name. Below the first it is a smudge; above the
/// second it is wider than the square it names however large the picture is.
const LEAST_WORD: i32 = 10;
const MOST_WORD: i32 = 22;

fn i32_of(many: usize) -> i32 {
    console_number::fitted(many.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The surface this desktop draws the home screen on: the screen at 1024
    /// by 640 logical pixels, less the rows the bar reserves at the top.
    const ROOM: (i32, i32) = (1024, 600);

    #[test]
    fn a_machine_nobody_has_asked_gets_the_grid_the_home_screen_was_written_as() {
        assert_eq!(Shape::USUAL.columns, 5);
        assert_eq!(Shape::USUAL.rows, 3);
        assert_eq!(Shape::default(), Shape::USUAL);
    }

    /// The whole point. The same shape on a screen laid out at half the density
    /// is a square twice the size in logical pixels, which is the same square
    /// to look at.
    #[test]
    fn a_square_is_a_share_of_the_room_and_not_a_number_of_pixels() {
        let here = square(ROOM, Shape::USUAL);
        let denser = square((ROOM.0 * 2, ROOM.1 * 2), Shape::USUAL);

        // The same square, to the one pixel the division rounds off. The
        // arithmetic is whole numbers because a fraction of a logical pixel is
        // a thing the compositor rounds off on its own and then warns about.
        assert!((denser.icon - here.icon * 2).abs() <= 1, "{denser:?} is not twice {here:?}");
        assert!((denser.padding - here.padding * 2).abs() <= 1, "{denser:?} against {here:?}");
    }

    /// What it was before this, and what it is now. The number is not the point
    /// -- that it came down is.
    #[test]
    fn the_usual_square_is_smaller_than_the_ninety_six_it_used_to_be() {
        assert!(square(ROOM, Shape::USUAL).icon < 96);
    }

    /// More squares on a pane is smaller squares, without anybody saying so.
    #[test]
    fn dividing_the_room_further_draws_them_smaller() {
        let five = square(ROOM, Shape::USUAL).icon;
        let eight = square(ROOM, Shape::USUAL.across(8)).icon;
        let deeper = square(ROOM, Shape::USUAL.down(5)).icon;

        assert!(eight < five, "{eight} is not under {five}");
        assert!(deeper < five, "{deeper} is not under {five}");
    }

    /// And the ladder moves it either way from there, in order.
    #[test]
    fn every_rung_of_the_ladder_is_bigger_than_the_one_below_it() {
        let sizes: Vec<i32> =
            EVERY.iter().map(|size| square(ROOM, Shape::USUAL.sized(*size)).icon).collect();

        assert!(sizes.windows(2).all(|two| two[0] < two[1]), "{sizes:?}");
    }

    /// A square whose plate is thicker than the picture on it, or whose name is
    /// too small to read, is a square the arithmetic got away from.
    #[test]
    fn a_square_keeps_its_proportions_at_every_size() {
        for size in EVERY {
            for columns in Shape::COLUMNS {
                let drawn = square(ROOM, Shape::USUAL.across(columns).sized(size));

                assert!(drawn.icon >= LEAST, "{size:?} {columns}: {drawn:?}");
                assert!(drawn.padding < drawn.icon, "{size:?} {columns}: {drawn:?}");
                assert!(drawn.named >= LEAST_WORD, "{size:?} {columns}: {drawn:?}");
                assert!(drawn.named <= MOST_WORD, "{size:?} {columns}: {drawn:?}");
            }
        }
    }

    /// A screen with nothing on it is not a crash and not a square of no size.
    #[test]
    fn no_room_at_all_still_draws_something() {
        let drawn = square((0, 0), Shape::USUAL);

        assert_eq!(drawn.icon, LEAST);
        assert!(drawn.margin > 0);
    }

    #[test]
    fn a_shape_survives_being_written_down_and_read_back() {
        let shape = Shape::USUAL.across(7).down(4).sized(Size::Bigger);

        assert_eq!(Shape::read(&shape.written()), shape);
    }

    /// A file somebody has edited by hand, or that was half written when the
    /// machine went off. What it says is taken and the rest is the usual.
    #[test]
    fn what_the_file_does_not_say_is_the_usual_answer() {
        assert_eq!(Shape::read(""), Shape::USUAL);
        assert_eq!(Shape::read("columns 7").columns, 7);
        assert_eq!(Shape::read("columns 7").rows, Shape::USUAL.rows);
        assert_eq!(Shape::read("columns seven"), Shape::USUAL, "not a number");
        assert_eq!(Shape::read("wallpaper yes"), Shape::USUAL, "not ours");
        assert_eq!(Shape::read("size enormous"), Shape::USUAL, "not a rung");
    }

    /// A number off either end is the end and not a grid nobody can walk.
    #[test]
    fn a_shape_asked_for_off_the_ends_stays_on_them() {
        assert_eq!(Shape::USUAL.across(0).columns, *Shape::COLUMNS.start());
        assert_eq!(Shape::USUAL.across(99).columns, *Shape::COLUMNS.end());
        assert_eq!(Shape::USUAL.down(0).rows, *Shape::ROWS.start());
        assert_eq!(Shape::USUAL.down(99).rows, *Shape::ROWS.end());
        assert_eq!(Shape::read("columns 200").columns, *Shape::COLUMNS.end());
    }

    #[test]
    fn every_rung_reads_back_as_the_word_that_wrote_it() {
        for size in EVERY {
            assert_eq!(Size::read(size.word()), Some(size));
            assert_eq!(size.says().to_lowercase(), size.word());
        }
    }

    /// The file is hers, and it is under her config beside the desktop's own
    /// size rather than in this repository.
    #[test]
    fn the_shape_is_written_under_her_own_home() {
        let at = at(std::path::Path::new("/home/somebody"));

        assert!(at.starts_with("/home/somebody/.config/console"), "{}", at.display());
    }
}
