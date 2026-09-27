//! The home screen: what is on the wallpaper, and where the thumb is on it.
//!
//! A desktop that opens into nothing is a desktop that has to be asked before
//! it will do anything, and everything else a person holds -- a phone, a
//! console, a laptop -- opens into something. This is that something: a few
//! panes of applications drawn on the wallpaper, walked with the d-pad,
//! opened with A, and rearranged with Y.
//!
//! It is not a menu. The menu is every application this machine has, in the
//! order they are used, found by typing; this is the handful someone put
//! where they want them, in the place they put them. The two are the same list
//! read two ways, which is why `console_applications::found` answers both -- and it is
//! why which applications are on the home screen at all is decided in the
//! menu, where the list already is, rather than on a card that was that list
//! with a word beside some of the rows.
//!
//! Everything here is about the grid and the file, and none of it draws. What
//! does: `console-home` is the surface, `home-square` is the card behind Y on
//! one of its squares, and `launcher` both puts things on the home screen and
//! answers the empty square that asks for one. All three are held to what this
//! says.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use console_core_atomic_writes::Stored;
use console_screen::Shape as Standing;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_walking::Ring;

const ONE_EMPTY_PANE: u32 = 1;


pub mod shape;

pub use shape::{Shape, Square, square};

pub const NAMED: &str = "home";

pub const NAMED_TALLER: &str = "home-taller";

pub fn file(home: &Path, standing: Standing) -> Result<PathBuf, Never> {
    let Ok(ours) = console_core_places::Base::State.application_under(home);

    Ok(ours.join(match standing {
        Standing::Wider => NAMED,
        Standing::Taller => NAMED_TALLER,
    }))
}

pub fn kept(home: &Path, standing: Standing) -> Result<Stored, Never> {
    let Ok(at) = file(home, standing);
    let Ok(held) = console_core_atomic_writes::read(&at);

    match held {
        Stored::Absent => {
            let Ok(other) = standing.other();
            let Ok(there) = file(home, other);

            console_core_atomic_writes::read(&there)
        }
        Stored::Text(_) | Stored::Failed(_) => Ok(held),
    }
}

#[derive(Debug)]
pub enum Unkept {
    Unread(PathBuf, String),
    Unwritten(PathBuf, console_core_atomic_writes::Unwritten),
}

impl std::fmt::Display for Unkept {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unkept::Unread(at, fault) => write!(f, "{}: {fault}", at.display()),
            Unkept::Unwritten(at, fault) => write!(f, "{}: {fault}", at.display()),
        }
    }
}

impl std::error::Error for Unkept {}

pub fn keep(home: &Path, standing: Standing, layout: &HomeScreen) -> Result<(), Unkept> {
    let Ok(at) = file(home, standing);

    written(&at, layout)?;

    let Ok(other) = standing.other();
    let Ok(there) = file(home, other);
    let Ok(held) = console_core_atomic_writes::read(&there);

    let theirs = match held {
        Stored::Text(said) => {
            let Ok(theirs) = HomeScreen::read(&said);

            theirs
        }
        Stored::Absent => return Ok(()),
        Stored::Failed(fault) => return Err(Unkept::Unread(there, fault)),
    };

    let grid = grid(home, other)?;
    let Ok(followed) = theirs.followed(layout, grid);

    match followed == theirs {
        true => Ok(()),
        false => written(&there, &followed),
    }
}

fn written(at: &Path, layout: &HomeScreen) -> Result<(), Unkept> {
    let Ok(said) = layout.serialize();

    match std::fs::read_to_string(at).is_ok_and(|before| before == said) {
        true => return Ok(()),
        false => {},
    }

    match at.parent() {
        Some(above) => {
            let _ = std::fs::create_dir_all(above);
        }
        None => {},
    }

    console_core_atomic_writes::whole(at, said.as_bytes()).map_err(|fault| Unkept::Unwritten(at.to_path_buf(), fault))
}

pub fn grid(home: &Path, standing: Standing) -> Result<Shape, Unkept> {
    let Ok(at) = shape::at(home, standing);
    let Ok(held) = console_core_atomic_writes::read(&at);

    match held {
        Stored::Text(said) => {
            let Ok(grid) = Shape::read(&said, standing);

            Ok(grid)
        }
        Stored::Absent => {
            let Ok(grid) = Shape::usual(standing);

            Ok(grid)
        }
        Stored::Failed(fault) => Err(Unkept::Unread(at, fault)),
    }
}

pub fn standing() -> Result<Standing, console_compositor::HyprctlError> {
    let found = console_screen::here()?;

    Ok(match found {
        Some(screen) => {
            let Ok(shape) = screen.shape();

            shape
        }
        None => Standing::Wider,
    })
}

pub const APPLICATION: [&str; 5] = ["Files", "Music", "Download", "Notifications", "Buttons"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Spot {
    pub pane: u32,
    pub row: u32,
    pub column: u32,
}

impl Spot {
    pub const FIRST: Spot = Spot { pane: 0, row: 0, column: 0 };

    pub fn on_the_grid(self, shape: Shape) -> Result<On, Never> {
        Ok(match self.row < shape.rows && self.column < shape.columns {
            true => On::TheGrid,
            false => On::None,
        })
    }

    pub fn serialize(self) -> Result<String, Never> {
        Ok(format!("{}.{}.{}", self.pane, self.row, self.column))
    }

    pub fn read(said: &str) -> Result<Option<Spot>, Never> {
        let mut fields = said.trim().split('.');

        let (pane, row, column) =
            match (fields.next(), fields.next(), fields.next(), fields.next()) {
                (Some(pane), Some(row), Some(column), None) => (pane, row, column),
                (None, _, _, _) | (_, None, _, _) | (_, _, None, _) | (_, _, _, Some(_)) => {
                    return Ok(None);
                }
            };

        let (pane, row, column) =
            match (pane.parse::<u32>(), row.parse::<u32>(), column.parse::<u32>()) {
                (Ok(pane), Ok(row), Ok(column)) => (pane, row, column),
                (Err(_not_a_number), _, _) | (_, Err(_not_a_number), _) | (_, _, Err(_not_a_number)) => return Ok(None),
            };

        let spot = Spot { pane, row, column };

        Ok(Some(spot))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum On {
    TheGrid,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    Up,
    Down,
    Left,
    Right,
}

pub fn moved(spot: Spot, way: Way, panes: u32, shape: Shape) -> Result<Spot, Never> {
    let (columns, rows) = (shape.columns.max(1), shape.rows.max(1));

    Ok(match way {
        Way::Up => Spot { row: spot.row.saturating_sub(1), ..spot },
        Way::Down => {
            Spot { row: spot.row.saturating_add(1).min(rows.saturating_sub(1)), ..spot }
        }
        Way::Left => match (spot.column, spot.pane) {
            (0, 0) => spot,
            (0, pane) => Spot {
                pane: pane.saturating_sub(1),
                column: columns.saturating_sub(1),
                ..spot
            },
            (column, _) => Spot { column: column.saturating_sub(1), ..spot },
        },
        Way::Right => {
            let at = spot.column.saturating_add(1);
            let after = spot.pane.saturating_add(1);

            match (at >= columns, after >= panes) {
                (true, true) => spot,
                (true, false) => Spot { pane: after, column: 0, ..spot },
                (false, true) | (false, false) => Spot { column: at, ..spot },
            }
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Along {
    Before,
    After,
}

pub fn paned(spot: Spot, along: Along, panes: u32) -> Result<Spot, Never> {
    let pane = match along {
        Along::Before => spot.pane.saturating_sub(1),
        Along::After => spot.pane.saturating_add(1).min(panes.saturating_sub(1)),
    };

    Ok(Spot { pane, ..spot })
}

const DRIFT: f64 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    Pressed,
    Travelled,
}

pub fn classify_touch(from: (f64, f64), to: (f64, f64)) -> Result<Touch, Never> {
    Ok(match (to.0 - from.0).hypot(to.1 - from.1) <= DRIFT {
        true => Touch::Pressed,
        false => Touch::Travelled,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reached {
    ByButton,
    ByTouch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bare {
    Chooses,
    Waits,
}

pub fn on_a_bare_square(reached: Reached) -> Result<Bare, Never> {
    Ok(match reached {
        Reached::ByButton => Bare::Chooses,
        Reached::ByTouch => Bare::Waits,
    })
}

const FLICK: f64 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flick {
    Across(Along),
    Upward,
    Nowhere,
}

pub fn flicked(from: (f64, f64), to: (f64, f64)) -> Result<Flick, Never> {
    let across = to.0 - from.0;
    let down = to.1 - from.1;

    match (across.abs() > down.abs(), across < -FLICK, across > FLICK) {
        (true, true, _) => return Ok(Flick::Across(Along::After)),
        (true, _, true) => return Ok(Flick::Across(Along::Before)),
        (true, false, false) | (false, _, _) => {},
    }

    Ok(match down < -FLICK && down.abs() > across.abs() {
        true => Flick::Upward,
        false => Flick::Nowhere,
    })
}

pub const HELD: std::time::Duration = std::time::Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LongPress {
    LongEnough,
    NotYet,
}

pub fn long_press(since: std::time::Duration, from: (f64, f64), at: (f64, f64)) -> Result<LongPress, Never> {
    let travelled = classify_touch(from, at)?;

    Ok(match (travelled, since >= HELD) {
        (Touch::Pressed, true) => LongPress::LongEnough,
        (Touch::Pressed, false) | (Touch::Travelled, _) => LongPress::NotYet,
    })
}

const A_HAIR: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    Somewhere,
    NotAtAll,
}

pub fn nudged(from: (f64, f64), to: (f64, f64)) -> Result<Moved, Never> {
    Ok(match (to.0 - from.0).hypot(to.1 - from.1) < A_HAIR {
        true => Moved::NotAtAll,
        false => Moved::Somewhere,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HomeScreen {
    placed: BTreeMap<Spot, String>,
}

impl HomeScreen {
    pub fn read(said: &str) -> Result<HomeScreen, Never> {
        let mut placed = BTreeMap::new();

        for line in said.lines() {
            let mut fields = line.split('\t');

            let (pane, row, column, name) =
                match (fields.next(), fields.next(), fields.next(), fields.next()) {
                    (Some(pane), Some(row), Some(column), Some(name)) => (pane, row, column, name),
                    (None, _, _, _) | (_, None, _, _) | (_, _, None, _) | (_, _, _, None) => {
                        continue;
                    }
                };

            let (pane, row, column) = match (
                pane.trim().parse::<u32>(),
                row.trim().parse::<u32>(),
                column.trim().parse::<u32>(),
            ) {
                (Ok(pane), Ok(row), Ok(column)) => (pane, row, column),
                (Err(_not_a_number), _, _) | (_, Err(_not_a_number), _) | (_, _, Err(_not_a_number)) => continue,
            };

            let spot = Spot { pane, row, column };

            let name = name.trim();

            match name.is_empty() {
                true => {},
                false => {
                    placed.insert(spot, name.to_string());
                }
            }
        }

        Ok(HomeScreen { placed })
    }

    pub fn serialize(&self) -> Result<String, Never> {
        Ok(self
            .placed
            .iter()
            .map(|(spot, name)| format!("{}\t{}\t{}\t{name}\n", spot.pane, spot.row, spot.column))
            .collect())
    }

    pub fn at(&self, spot: Spot) -> Result<Option<&str>, Never> {
        Ok(self.placed.get(&spot).map(String::as_str))
    }

    pub fn place(&mut self, spot: Spot, name: &str) -> Result<(), Never> {
        self.placed.insert(spot, name.to_string());

        Ok(())
    }

    pub fn fitted(&self, shape: Shape) -> Result<HomeScreen, Never> {
        let mut kept: Vec<(Spot, String)> = Vec::new();
        let mut adrift: Vec<String> = Vec::new();

        for (spot, name) in &self.placed {
            let on = spot.on_the_grid(shape)?;

            match on {
                On::TheGrid => kept.push((*spot, name.clone())),
                On::None => adrift.push(name.clone()),
            }
        }

        match adrift.is_empty() {
            true => return Ok(HomeScreen { placed: self.placed.clone() }),
            false => {},
        }

        let mut home = HomeScreen { placed: kept.into_iter().collect() };

        for name in adrift {
            let spot = home.first_free(shape)?;

            home.place(spot, &name)?;
        }

        Ok(home)
    }

    pub fn remove(&mut self, spot: Spot) -> Result<(), Never> {
        self.placed.remove(&spot);

        Ok(())
    }

    pub fn every(&self) -> Result<impl Iterator<Item = (Spot, &str)>, Never> {
        Ok(self.placed.iter().map(|(spot, name)| (*spot, name.as_str())))
    }

    pub fn first(order: &[String], shape: Shape) -> Result<HomeScreen, Never> {
        let mut home = HomeScreen::default();
        let ours = APPLICATION.iter().map(|said| said.to_string());
        let rest = order.iter().filter(|name| !APPLICATION.contains(&name.as_str())).cloned();
        #[cfg_attr(
            dylint_lib = "explicit028_no_search_in_a_loop",
            allow(
                explicit028_no_search_in_a_loop,
                reason = "what is walked is `OURS`, which is this desktop's own programs and is written out above; the `let` is what hides that from the rule rather than anything about the length"
            )
        )]
        let names = ours.filter(|name| order.contains(name)).chain(rest);
        let Ok(round) = Ring::round(shape.columns);

        let ring = match round {
            Some(ring) => ring,
            None => return Ok(home),
        };

        let Ok(columns) = ring.many();
        let squares = shape.squares()?;
        let Ok(squares) = index(squares);

        for (at, name) in names.take(squares).enumerate() {
            let Ok(at) = fitted::<_, u32>(at);
            let Ok(column) = ring.at(at);

            let spot = Spot { pane: 0, row: at.saturating_div(columns), column };

            home.place(spot, &name)?;
        }

        Ok(home)
    }

    pub fn panes(&self) -> Result<u32, Never> {
        Ok(match self.placed.keys().map(|spot| spot.pane.saturating_add(1)).max() {
            Some(panes) => panes,
            None => ONE_EMPTY_PANE,
        })
    }

    pub fn first_free(&self, shape: Shape) -> Result<Spot, Never> {
        let (columns, rows) = (shape.columns, shape.rows);

        let panes = self.panes()?;

        for pane in 0..panes {
            for row in 0..rows {
                for column in 0..columns {
                    let spot = Spot { pane, row, column };

                    let held = self.at(spot)?;

                    match held {
                        Some(_) => {},
                        None => return Ok(spot),
                    }
                }
            }
        }

        Ok(Spot { pane: panes, row: 0, column: 0 })
    }

    pub fn where_(&self, name: &str) -> Result<Option<Spot>, Never> {
        Ok(self.placed.iter().find(|(_, placed)| placed.as_str() == name).map(|(spot, _)| *spot))
    }

    pub fn forget(&mut self, name: &str) -> Result<(), Never> {
        self.placed.retain(|_, placed| placed != name);

        Ok(())
    }

    pub fn followed(&self, leading: &HomeScreen, shape: Shape) -> Result<HomeScreen, Never> {
        let wanted: BTreeSet<&str> = leading.placed.values().map(String::as_str).collect();
        let mut home = HomeScreen {
            placed: self
                .placed
                .iter()
                .filter(|(_, name)| wanted.contains(name.as_str()))
                .map(|(spot, name)| (*spot, name.clone()))
                .collect(),
        };
        let held: BTreeSet<String> = home.placed.values().cloned().collect();

        for name in leading.placed.values().filter(|name| !held.contains(name.as_str())) {
            let spot = home.first_free(shape)?;

            home.place(spot, name)?;
        }

        Ok(home)
    }

    pub fn occupancy(&self) -> Result<Holding, Never> {
        Ok(match self.placed.is_empty() {
            true => Holding::None,
            false => Holding::Some,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holding {
    Some,
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(home: &HomeScreen) -> Result<u32, std::num::TryFromIntError> {
        let Ok(every) = home.every();

        u32::try_from(every.count())
    }

    fn all_on(home: &HomeScreen, shape: Shape) -> Result<On, Never> {
        let Ok(mut every) = home.every();

        match every.all(|(spot, _)| spot.on_the_grid(shape) == Ok(On::TheGrid)) {
            true => Ok(On::TheGrid),
            false => Ok(On::None),
        }
    }

    const GRID: Shape = Shape::USUAL;

    #[test]
    fn a_bare_square_is_only_offered_the_picker_by_a_button() {
        assert_eq!(on_a_bare_square(Reached::ByButton), Ok(Bare::Chooses));
        assert_eq!(on_a_bare_square(Reached::ByTouch), Ok(Bare::Waits));
    }

    #[test]
    fn a_square_survives_being_handed_to_another_program() {
        let squares = [
            Spot::FIRST,
            Spot { pane: 2, row: 1, column: 4 },
            Spot { pane: 0, row: 0, column: 11 },
        ];

        for spot in squares {
            let Ok(said) = spot.serialize();

            assert_eq!(Spot::read(&said), Ok(Some(spot)), "{spot:?}");
        }

        assert_eq!(Spot::read("0.1"), Ok(None), "three numbers or nothing");
        assert_eq!(Spot::read("0.1.2.3"), Ok(None), "three numbers or nothing");
        assert_eq!(Spot::read("zero.one.two"), Ok(None), "numbers, not words");
        assert_eq!(Spot::read(""), Ok(None), "nothing is not a square");
    }

    const COLUMNS: u32 = GRID.columns;
    const ROWS: u32 = GRID.rows;

    #[test]
    fn the_dpad_walks_the_squares_and_falls_off_neither_end() {
        let middle = Spot { pane: 1, row: 1, column: 2 };
        assert_eq!(moved(middle, Way::Up, 3, GRID), Ok(Spot { pane: 1, row: 0, column: 2 }));
        assert_eq!(moved(middle, Way::Down, 3, GRID), Ok(Spot { pane: 1, row: 2, column: 2 }));
        assert_eq!(moved(middle, Way::Left, 3, GRID), Ok(Spot { pane: 1, row: 1, column: 1 }));
        assert_eq!(moved(middle, Way::Right, 3, GRID), Ok(Spot { pane: 1, row: 1, column: 3 }));

        let top = Spot { pane: 0, row: 0, column: 0 };
        assert_eq!(moved(top, Way::Up, 3, GRID), Ok(top), "there is nothing above the first row");
        assert_eq!(moved(top, Way::Left, 3, GRID), Ok(top), "nor before the first pane");

        let last = Spot { pane: 2, row: ROWS.saturating_sub(1), column: COLUMNS.saturating_sub(1) };
        assert_eq!(moved(last, Way::Down, 3, GRID), Ok(last));
        assert_eq!(moved(last, Way::Right, 3, GRID), Ok(last), "the panes end where the caller said");
    }

    #[test]
    fn walking_off_the_side_of_a_pane_is_how_the_next_one_is_reached() {
        let last = Spot { pane: 0, row: 1, column: COLUMNS.saturating_sub(1) };
        let first = Spot { pane: 1, row: 1, column: 0 };

        assert_eq!(moved(last, Way::Right, 2, GRID), Ok(first));
        assert_eq!(moved(first, Way::Left, 2, GRID), Ok(last));
    }

    #[test]
    fn a_full_hand_is_offered_the_pane_past_the_end_and_an_empty_one_is_not() {
        let edge = Spot { pane: 1, row: 0, column: COLUMNS.saturating_sub(1) };
        assert_eq!(moved(edge, Way::Right, 2, GRID), Ok(edge));
        assert_eq!(moved(edge, Way::Right, 3, GRID), Ok(Spot { pane: 2, row: 0, column: 0 }));
    }

    #[test]
    fn a_swipe_moves_a_whole_pane_and_stops_at_the_ends() {
        let third = Spot { pane: 0, row: 2, column: 3 };
        let last = Spot { pane: 1, row: 0, column: 0 };

        assert_eq!(paned(third, Along::After, 2), Ok(Spot { pane: 1, row: 2, column: 3 }));
        assert_eq!(paned(third, Along::Before, 2), Ok(third));
        assert_eq!(paned(last, Along::After, 2), Ok(last));
    }

    #[test]
    fn a_finger_drawn_across_the_squares_turns_the_pane_it_was_drawn_away_from() {
        assert_eq!(flicked((400.0, 300.0), (200.0, 310.0)), Ok(Flick::Across(Along::After)));
        assert_eq!(flicked((200.0, 300.0), (400.0, 290.0)), Ok(Flick::Across(Along::Before)));
    }

    #[test]
    fn a_finger_drawn_up_the_screen_is_not_a_pane_and_a_short_one_is_neither() {
        assert_eq!(flicked((400.0, 500.0), (410.0, 300.0)), Ok(Flick::Upward));
        assert_eq!(flicked((400.0, 500.0), (430.0, 480.0)), Ok(Flick::Nowhere));
        assert_eq!(flicked((400.0, 300.0), (410.0, 500.0)), Ok(Flick::Nowhere), "downward");
    }

    #[test]
    fn a_finger_held_still_long_enough_is_a_hold_and_one_that_wandered_off_is_not() {
        let still = (100.0, 100.0);

        assert_eq!(long_press(HELD, still, still), Ok(LongPress::LongEnough));
        assert_eq!(long_press(HELD / 2, still, still), Ok(LongPress::NotYet));
        assert_eq!(long_press(HELD, still, (400.0, 100.0)), Ok(LongPress::NotYet));
    }

    #[test]
    fn a_finger_that_travelled_pressed_nothing() {
        assert_eq!(classify_touch((100.0, 100.0), (100.0, 100.0)), Ok(Touch::Pressed));
        assert_eq!(classify_touch((100.0, 100.0), (108.0, 94.0)), Ok(Touch::Pressed), "a thumb wanders");
        assert_eq!(classify_touch((100.0, 100.0), (400.0, 108.0)), Ok(Touch::Travelled), "a pane, sideways");
        assert_eq!(classify_touch((100.0, 300.0), (112.0, 40.0)), Ok(Touch::Travelled), "the menu, upwards");
    }

    #[test]
    fn a_square_arriving_under_a_still_pointer_is_not_the_pointer_moving() {
        assert_eq!(nudged((100.0, 100.0), (100.0, 100.0)), Ok(Moved::NotAtAll), "the same place");
        assert_eq!(nudged((100.0, 100.0), (100.2, 100.2)), Ok(Moved::NotAtAll), "a rounding");
        assert_eq!(nudged((100.0, 100.0), (101.0, 100.0)), Ok(Moved::Somewhere), "a pixel across");
        assert_eq!(nudged((100.0, 100.0), (100.0, 99.0)), Ok(Moved::Somewhere), "a pixel up");
    }

    #[test]
    fn what_is_placed_is_what_is_written_down_and_read_back() {
        let mut home = HomeScreen::default();
        let Ok(()) = home.place(Spot { pane: 0, row: 0, column: 0 }, "Files");
        let Ok(()) = home.place(Spot { pane: 2, row: 1, column: 4 }, "Steam");

        assert_eq!(home.serialize(), Ok("0\t0\t0\tFiles\n2\t1\t4\tSteam\n".to_string()));

        let Ok(written) = home.serialize();

        assert_eq!(HomeScreen::read(&written), Ok(home.clone()));
        assert_eq!(home.at(Spot { pane: 2, row: 1, column: 4 }), Ok(Some("Steam")));
        assert_eq!(home.at(Spot { pane: 2, row: 1, column: 3 }), Ok(None));
    }

    #[test]
    fn a_name_with_spaces_in_it_survives_the_writing_down() {
        let Ok(home) = HomeScreen::read("1\t0\t2\tText Editor\n");
        assert_eq!(home.at(Spot { pane: 1, row: 0, column: 2 }), Ok(Some("Text Editor")));
    }

    #[test]
    fn a_line_that_is_not_a_placement_is_not_a_placement() {
        let Ok(home) = HomeScreen::read("\nnonsense\n0\t0\n0\t0\t0\t\nx\ty\tz\tFiles\n0\t0\t0\tFiles\n");
        assert_eq!(count(&home), Ok(1));
        assert_eq!(home.at(Spot::FIRST), Ok(Some("Files")));
    }

    #[test]
    fn a_square_this_grid_does_not_have_is_moved_onto_it_rather_than_dropped() {
        let Ok(home) = HomeScreen::read(&format!("0\t0\t{COLUMNS}\tFiles\n0\t{ROWS}\t0\tSteam\n"));

        assert_eq!(home.occupancy(), Ok(Holding::Some), "nothing is thrown away by reading");

        let Ok(fitted) = home.fitted(GRID);
        let Ok(every) = fitted.every();
        let names: Vec<&str> = every.map(|(_, name)| name).collect();

        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names.contains(&"Files") && names.contains(&"Steam"), "{names:?}");
        assert_eq!(all_on(&fitted, GRID), Ok(On::TheGrid), "something is still off the grid");
    }

    #[test]
    fn narrowing_the_grid_folds_what_was_off_it_round_rather_than_over_anything() {
        let mut home = HomeScreen::default();

        for row in 0..GRID.rows {
            for column in 0..GRID.columns {
                let Ok(()) = home.place(Spot { pane: 0, row, column }, &format!("{row}-{column}"));
            }
        }

        let Ok(narrow) = GRID.with_columns(3);
        let Ok(fitted) = home.fitted(narrow);

        let Ok(panes) = fitted.panes();

        assert_eq!(count(&fitted), count(&home), "something was folded over");
        assert_eq!(all_on(&fitted, narrow), Ok(On::TheGrid));
        assert!(panes > 1, "fifteen things do not fit on nine squares");
    }

    #[test]
    fn a_grid_nothing_is_off_leaves_every_square_where_it_was() {
        let mut home = HomeScreen::default();
        let Ok(()) = home.place(Spot { pane: 0, row: 0, column: 0 }, "Files");
        let Ok(()) = home.place(Spot { pane: 1, row: 2, column: 4 }, "Steam");

        assert_eq!(home.fitted(GRID), Ok(home));
    }

    #[test]
    fn a_far_pane_in_the_file_is_a_home_screen_with_that_many_panes() {
        let Ok(home) = HomeScreen::read("7\t0\t0\tSteam\n");
        assert_eq!(home.at(Spot { pane: 7, row: 0, column: 0 }), Ok(Some("Steam")));
        assert_eq!(home.panes(), Ok(8));
    }

    #[test]
    fn there_are_as_many_panes_as_what_is_placed_reaches() {
        let mut home = HomeScreen::default();
        assert_eq!(home.panes(), Ok(1), "an empty home screen is one pane of room");

        let Ok(()) = home.place(Spot { pane: 2, row: 1, column: 1 }, "Steam");
        let Ok(()) = home.place(Spot { pane: 0, row: 0, column: 0 }, "Files");
        assert_eq!(home.panes(), Ok(3));

        let Ok(()) = home.forget("Files");
        assert_eq!(home.panes(), Ok(3), "an emptied middle pane is still a place");

        let Ok(()) = home.forget("Steam");
        assert_eq!(home.panes(), Ok(1), "the far panes go when the last thing on them does");
    }

    #[test]
    fn a_machine_that_has_never_had_one_opens_on_what_it_uses_most() {
        let order: Vec<String> =
            ["Aether", "Files", "Music", "Steam"].iter().map(|said| said.to_string()).collect();
        let Ok(home) = HomeScreen::first(&order, GRID);

        assert_eq!(home.at(Spot::FIRST), Ok(Some("Files")), "the desktop's own come first");
        assert_eq!(home.at(Spot { pane: 0, row: 0, column: 1 }), Ok(Some("Music")));

        let third = Spot { pane: 0, row: 0, column: 2 };

        assert_eq!(home.at(third), Ok(Some("Aether")), "and the rest in their own order");
        assert_eq!(home.at(Spot { pane: 0, row: 0, column: 3 }), Ok(Some("Steam")));
        assert_eq!(count(&home), Ok(4));
    }

    #[test]
    fn one_of_ours_that_is_not_installed_leaves_no_hole() {
        let order: Vec<String> = vec!["Music".to_string(), "Steam".to_string()];
        let Ok(home) = HomeScreen::first(&order, GRID);

        assert_eq!(home.at(Spot::FIRST), Ok(Some("Music")));
        assert_eq!(home.at(Spot { pane: 0, row: 0, column: 1 }), Ok(Some("Steam")));
        assert_eq!(count(&home), Ok(2));
    }

    #[test]
    fn the_first_pane_is_as_full_as_it_gets_and_no_fuller() {
        let order: Vec<String> = (0..100).map(|at| format!("App {at}")).collect();
        let Ok(home) = HomeScreen::first(&order, GRID);

        let Ok(every) = home.every();
        let panes: Vec<u32> = every.map(|(spot, _)| spot.pane).collect();

        assert_eq!(u32::try_from(panes.len()), Ok(ROWS.saturating_mul(COLUMNS)));
        assert!(panes.iter().all(|pane| *pane == 0));
    }

    #[test]
    fn what_is_on_the_home_screen_is_kept_where_state_is_kept() {
        assert_eq!(
            file(std::path::Path::new("/home/someone"), Standing::Wider),
            Ok(std::path::PathBuf::from("/home/someone/.local/state/console/home"))
        );
        assert_eq!(
            file(std::path::Path::new("/home/someone"), Standing::Taller),
            Ok(std::path::PathBuf::from("/home/someone/.local/state/console/home-taller"))
        );
    }

    #[test]
    fn the_first_free_square_is_the_first_one_reading_across() {
        let mut home = HomeScreen::default();
        assert_eq!(home.first_free(GRID), Ok(Spot::FIRST));

        let Ok(()) = home.place(Spot::FIRST, "Files");
        assert_eq!(home.first_free(GRID), Ok(Spot { pane: 0, row: 0, column: 1 }));

        let pane_of = ROWS.saturating_mul(COLUMNS);

        for (at, name) in (0..pane_of.saturating_mul(2)).map(|at| (at, format!("App {at}"))) {
            let pane = at.div_euclid(pane_of);
            let left = at.rem_euclid(pane_of);
            let Ok(()) = home.place(Spot { pane, row: left.div_euclid(COLUMNS), column: left.rem_euclid(COLUMNS) }, &name);
        }

        assert_eq!(
            home.first_free(GRID),
            Ok(Spot { pane: 2, row: 0, column: 0 }),
            "every pane full is answered with a fresh one"
        );
    }

    #[test]
    fn an_application_is_found_by_name_and_taken_off_by_name() {
        let mut home = HomeScreen::default();
        let Ok(()) = home.place(Spot { pane: 1, row: 2, column: 3 }, "Steam");

        assert_eq!(home.where_("Steam"), Ok(Some(Spot { pane: 1, row: 2, column: 3 })));
        assert_eq!(home.where_("Files"), Ok(None));

        let Ok(()) = home.forget("Steam");
        assert_eq!(home.where_("Steam"), Ok(None));
        assert_eq!(home.occupancy(), Ok(Holding::None));
    }

    #[test]
    fn what_is_taken_off_is_off() {
        let Ok(mut home) = HomeScreen::first(&["Files".to_string()], GRID);
        let Ok(()) = home.remove(Spot::FIRST);

        assert_eq!(home.occupancy(), Ok(Holding::None));
        assert_eq!(home.serialize(), Ok("".to_string()));
    }
}
