//! The home screen: what is on the wallpaper, and where the thumb is on it.
//!
//! A desktop that opens into nothing is a desktop that has to be asked before
//! it will do anything, and everything else a person holds -- a phone, a
//! console, a laptop -- opens into something. This is that something: a few
//! panes of applications drawn on the wallpaper, walked with the d-pad,
//! opened with A, and rearranged with Y.
//!
//! It is not a menu. The menu is every application this machine has, in the
//! order they are used, found by typing; this is the handful somebody put
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

use std::collections::BTreeMap;

use console_never::Never;

pub mod shape;

pub use shape::{Shape, Square, square};

pub fn file(home: &std::path::Path) -> Result<std::path::PathBuf, Never> {
    Ok(home.join(".local/state/console/home"))
}

pub const OURS: [&str; 5] = ["Files", "Music", "Download", "Notifications", "Buttons"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Spot {
    pub pane: usize,
    pub row: usize,
    pub column: usize,
}

impl Spot {
    pub const FIRST: Spot = Spot { pane: 0, row: 0, column: 0 };

    pub fn new(pane: usize, row: usize, column: usize) -> Result<Spot, Never> {
        Ok(Spot { pane, row, column })
    }

    pub fn on_the_grid(self, shape: Shape) -> Result<On, Never> {
        Ok(match self.row < shape.rows && self.column < shape.columns {
            true => On::TheGrid,
            false => On::Nothing,
        })
    }

    pub fn said(self) -> Result<String, Never> {
        Ok(format!("{}.{}.{}", self.pane, self.row, self.column))
    }

    pub fn read(said: &str) -> Result<Option<Spot>, Never> {
        let mut fields = said.trim().split('.');

        let (Some(pane), Some(row), Some(column), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Ok(None);
        };

        let (Ok(pane), Ok(row), Ok(column)) =
            (pane.parse::<usize>(), row.parse::<usize>(), column.parse::<usize>())
        else {
            return Ok(None);
        };

        let spot = Spot::new(pane, row, column)?;

        Ok(Some(spot))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum On {
    TheGrid,
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    Up,
    Down,
    Left,
    Right,
}

pub fn moved(spot: Spot, way: Way, panes: usize, shape: Shape) -> Result<Spot, Never> {
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
        Way::Right => match (spot.column.saturating_add(1), spot.pane.saturating_add(1)) {
            (at, after) if at >= columns && after >= panes => spot,
            (at, pane) if at >= columns => Spot { pane, column: 0, ..spot },
            (column, _) => Spot { column, ..spot },
        },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Along {
    Before,
    After,
}

pub fn paned(spot: Spot, along: Along, panes: usize) -> Result<Spot, Never> {
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

pub fn touched(from: (f64, f64), to: (f64, f64)) -> Result<Touch, Never> {
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
pub struct Home {
    placed: BTreeMap<Spot, String>,
}

impl Home {
    pub fn read(said: &str) -> Result<Home, Never> {
        let mut placed = BTreeMap::new();

        for line in said.lines() {
            let mut fields = line.split('\t');

            let (Some(pane), Some(row), Some(column), Some(name)) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                continue;
            };

            let (Ok(pane), Ok(row), Ok(column)) = (
                pane.trim().parse::<usize>(),
                row.trim().parse::<usize>(),
                column.trim().parse::<usize>(),
            ) else {
                continue;
            };

            let spot = Spot::new(pane, row, column)?;

            let name = name.trim();

            match name.is_empty() {
                true => {},
                false => {
                    placed.insert(spot, name.to_string());
                }
            }
        }

        Ok(Home { placed })
    }

    pub fn written(&self) -> Result<String, Never> {
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

    pub fn fitted(&self, shape: Shape) -> Result<Home, Never> {
        let mut kept: Vec<(Spot, String)> = Vec::new();
        let mut adrift: Vec<String> = Vec::new();

        for (spot, name) in &self.placed {
            let on = spot.on_the_grid(shape)?;

            match on {
                On::TheGrid => kept.push((*spot, name.clone())),
                On::Nothing => adrift.push(name.clone()),
            }
        }

        match adrift.is_empty() {
            true => return Ok(Home { placed: self.placed.clone() }),
            false => {},
        }

        let mut home = Home { placed: kept.into_iter().collect() };

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

    pub fn first(order: &[String], shape: Shape) -> Result<Home, Never> {
        let mut home = Home::default();
        let ours = OURS.iter().map(|said| said.to_string());
        let rest = order.iter().filter(|name| !OURS.contains(&name.as_str())).cloned();
        let names = ours.filter(|name| order.contains(name)).chain(rest);
        let columns = shape.columns.max(1);

        let squares = shape.squares()?;

        for (at, name) in names.take(squares).enumerate() {
            let spot =
                Spot::new(0, at.saturating_div(columns), at.checked_rem(columns).unwrap_or(0))?;

            home.place(spot, &name)?;
        }

        Ok(home)
    }

    pub fn panes(&self) -> Result<usize, Never> {
        Ok(self.placed.keys().map(|spot| spot.pane.saturating_add(1)).max().unwrap_or(1))
    }

    pub fn first_free(&self, shape: Shape) -> Result<Spot, Never> {
        let (columns, rows) = (shape.columns, shape.rows);

        let panes = self.panes()?;

        for pane in 0..panes {
            for row in 0..rows {
                for column in 0..columns {
                    let spot = Spot::new(pane, row, column)?;

                    let held = self.at(spot)?;

                    match held {
                        Some(_) => {},
                        None => return Ok(spot),
                    }
                }
            }
        }

        Spot::new(panes, 0, 0)
    }

    pub fn where_(&self, name: &str) -> Result<Option<Spot>, Never> {
        Ok(self.placed.iter().find(|(_, placed)| placed.as_str() == name).map(|(spot, _)| *spot))
    }

    pub fn forget(&mut self, name: &str) -> Result<(), Never> {
        self.placed.retain(|_, placed| placed != name);

        Ok(())
    }

    pub fn holding(&self) -> Result<Holding, Never> {
        Ok(match self.placed.is_empty() {
            true => Holding::Nothing,
            false => Holding::Something,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holding {
    Something,
    Nothing,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(answer) = answer;

        answer
    }

    const GRID: Shape = Shape::USUAL;

    #[test]
    fn a_bare_square_is_only_offered_the_chooser_by_a_button() {
        assert_eq!(ok(on_a_bare_square(Reached::ByButton)), Bare::Chooses);
        assert_eq!(ok(on_a_bare_square(Reached::ByTouch)), Bare::Waits);
    }

    #[test]
    fn a_square_survives_being_handed_to_another_program() {
        for spot in [Spot::FIRST, ok(Spot::new(2, 1, 4)), ok(Spot::new(0, 0, 11))] {
            assert_eq!(ok(Spot::read(&ok(spot.said()))), Some(spot), "{spot:?}");
        }

        assert_eq!(ok(Spot::read("0.1")), None, "three numbers or nothing");
        assert_eq!(ok(Spot::read("0.1.2.3")), None, "three numbers or nothing");
        assert_eq!(ok(Spot::read("nought.one.two")), None, "numbers, not words");
        assert_eq!(ok(Spot::read("")), None, "nothing is not a square");
    }

    const COLUMNS: usize = GRID.columns;
    const ROWS: usize = GRID.rows;

    #[test]
    fn the_dpad_walks_the_squares_and_falls_off_neither_end() {
        let middle = ok(Spot::new(1, 1, 2));
        assert_eq!(ok(moved(middle, Way::Up, 3, GRID)), ok(Spot::new(1, 0, 2)));
        assert_eq!(ok(moved(middle, Way::Down, 3, GRID)), ok(Spot::new(1, 2, 2)));
        assert_eq!(ok(moved(middle, Way::Left, 3, GRID)), ok(Spot::new(1, 1, 1)));
        assert_eq!(ok(moved(middle, Way::Right, 3, GRID)), ok(Spot::new(1, 1, 3)));

        let top = ok(Spot::new(0, 0, 0));
        assert_eq!(ok(moved(top, Way::Up, 3, GRID)), top, "there is nothing above the first row");
        assert_eq!(ok(moved(top, Way::Left, 3, GRID)), top, "nor before the first pane");

        let last = ok(Spot::new(2, ROWS - 1, COLUMNS - 1));
        assert_eq!(ok(moved(last, Way::Down, 3, GRID)), last);
        assert_eq!(ok(moved(last, Way::Right, 3, GRID)), last, "the panes end where the caller said");
    }

    #[test]
    fn walking_off_the_side_of_a_pane_is_how_the_next_one_is_reached() {
        assert_eq!(ok(moved(ok(Spot::new(0, 1, COLUMNS - 1)), Way::Right, 2, GRID)), ok(Spot::new(1, 1, 0)));
        assert_eq!(ok(moved(ok(Spot::new(1, 1, 0)), Way::Left, 2, GRID)), ok(Spot::new(0, 1, COLUMNS - 1)));
    }

    #[test]
    fn a_full_hand_is_offered_the_pane_past_the_end_and_an_empty_one_is_not() {
        let edge = ok(Spot::new(1, 0, COLUMNS - 1));
        assert_eq!(ok(moved(edge, Way::Right, 2, GRID)), edge);
        assert_eq!(ok(moved(edge, Way::Right, 3, GRID)), ok(Spot::new(2, 0, 0)));
    }

    #[test]
    fn a_swipe_moves_a_whole_pane_and_stops_at_the_ends() {
        assert_eq!(ok(paned(ok(Spot::new(0, 2, 3)), Along::After, 2)), ok(Spot::new(1, 2, 3)));
        assert_eq!(ok(paned(ok(Spot::new(0, 2, 3)), Along::Before, 2)), ok(Spot::new(0, 2, 3)));
        assert_eq!(ok(paned(ok(Spot::new(1, 0, 0)), Along::After, 2)), ok(Spot::new(1, 0, 0)));
    }

    #[test]
    fn a_finger_that_travelled_pressed_nothing() {
        assert_eq!(ok(touched((100.0, 100.0), (100.0, 100.0))), Touch::Pressed);
        assert_eq!(ok(touched((100.0, 100.0), (108.0, 94.0))), Touch::Pressed, "a thumb wanders");
        assert_eq!(ok(touched((100.0, 100.0), (400.0, 108.0))), Touch::Travelled, "a pane, sideways");
        assert_eq!(ok(touched((100.0, 300.0), (112.0, 40.0))), Touch::Travelled, "the menu, upwards");
    }

    #[test]
    fn a_square_arriving_under_a_still_pointer_is_not_the_pointer_moving() {
        assert_eq!(ok(nudged((100.0, 100.0), (100.0, 100.0))), Moved::NotAtAll, "the same place");
        assert_eq!(ok(nudged((100.0, 100.0), (100.2, 100.2))), Moved::NotAtAll, "a rounding");
        assert_eq!(ok(nudged((100.0, 100.0), (101.0, 100.0))), Moved::Somewhere, "a pixel across");
        assert_eq!(ok(nudged((100.0, 100.0), (100.0, 99.0))), Moved::Somewhere, "a pixel up");
    }

    #[test]
    fn what_is_placed_is_what_is_written_down_and_read_back() {
        let mut home = Home::default();
        ok(home.place(ok(Spot::new(0, 0, 0)), "Files"));
        ok(home.place(ok(Spot::new(2, 1, 4)), "Steam"));

        assert_eq!(ok(home.written()), "0\t0\t0\tFiles\n2\t1\t4\tSteam\n");
        assert_eq!(ok(Home::read(&ok(home.written()))), home);
        assert_eq!(ok(home.at(ok(Spot::new(2, 1, 4)))), Some("Steam"));
        assert_eq!(ok(home.at(ok(Spot::new(2, 1, 3)))), None);
    }

    #[test]
    fn a_name_with_spaces_in_it_survives_the_writing_down() {
        let home = ok(Home::read("1\t0\t2\tText Editor\n"));
        assert_eq!(ok(home.at(ok(Spot::new(1, 0, 2)))), Some("Text Editor"));
    }

    #[test]
    fn a_line_that_is_not_a_placement_is_not_a_placement() {
        let home = ok(Home::read("\nnonsense\n0\t0\n0\t0\t0\t\nx\ty\tz\tFiles\n0\t0\t0\tFiles\n"));
        assert_eq!(ok(home.every()).count(), 1);
        assert_eq!(ok(home.at(Spot::FIRST)), Some("Files"));
    }

    #[test]
    fn a_square_this_grid_does_not_have_is_moved_onto_it_rather_than_dropped() {
        let home = ok(Home::read(&format!("0\t0\t{COLUMNS}\tFiles\n0\t{ROWS}\t0\tSteam\n")));

        assert_eq!(ok(home.holding()), Holding::Something, "nothing is thrown away by reading");

        let fitted = ok(home.fitted(GRID));
        let names: Vec<&str> = ok(fitted.every()).map(|(_, name)| name).collect();

        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names.contains(&"Files") && names.contains(&"Steam"), "{names:?}");
        assert!(
            ok(fitted.every()).all(|(spot, _)| ok(spot.on_the_grid(GRID)) == On::TheGrid),
            "something is still off the grid"
        );
    }

    #[test]
    fn narrowing_the_grid_folds_what_was_off_it_round_rather_than_over_anything() {
        let mut home = Home::default();

        for row in 0..GRID.rows {
            for column in 0..GRID.columns {
                ok(home.place(ok(Spot::new(0, row, column)), &format!("{row}-{column}")));
            }
        }

        let narrow = ok(GRID.across(3));
        let fitted = ok(home.fitted(narrow));

        assert_eq!(ok(fitted.every()).count(), ok(home.every()).count(), "something was folded over");
        assert!(ok(fitted.every()).all(|(spot, _)| ok(spot.on_the_grid(narrow)) == On::TheGrid));
        assert!(ok(fitted.panes()) > 1, "fifteen things do not fit on nine squares");
    }

    #[test]
    fn a_grid_nothing_is_off_leaves_every_square_where_it_was() {
        let mut home = Home::default();
        ok(home.place(ok(Spot::new(0, 0, 0)), "Files"));
        ok(home.place(ok(Spot::new(1, 2, 4)), "Steam"));

        assert_eq!(ok(home.fitted(GRID)), home);
    }

    #[test]
    fn a_far_pane_in_the_file_is_a_home_screen_with_that_many_panes() {
        let home = ok(Home::read("7\t0\t0\tSteam\n"));
        assert_eq!(ok(home.at(ok(Spot::new(7, 0, 0)))), Some("Steam"));
        assert_eq!(ok(home.panes()), 8);
    }

    #[test]
    fn there_are_as_many_panes_as_what_is_placed_reaches() {
        let mut home = Home::default();
        assert_eq!(ok(home.panes()), 1, "an empty home screen is one pane of room");

        ok(home.place(ok(Spot::new(2, 1, 1)), "Steam"));
        ok(home.place(ok(Spot::new(0, 0, 0)), "Files"));
        assert_eq!(ok(home.panes()), 3);

        ok(home.forget("Files"));
        assert_eq!(ok(home.panes()), 3, "an emptied middle pane is still a place");

        ok(home.forget("Steam"));
        assert_eq!(ok(home.panes()), 1, "the far panes go when the last thing on them does");
    }

    #[test]
    fn a_machine_that_has_never_had_one_opens_on_what_it_uses_most() {
        let order: Vec<String> =
            ["Aether", "Files", "Music", "Steam"].iter().map(|said| said.to_string()).collect();
        let home = ok(Home::first(&order, GRID));

        assert_eq!(ok(home.at(Spot::FIRST)), Some("Files"), "the desktop's own come first");
        assert_eq!(ok(home.at(ok(Spot::new(0, 0, 1)))), Some("Music"));
        assert_eq!(ok(home.at(ok(Spot::new(0, 0, 2)))), Some("Aether"), "and the rest in their own order");
        assert_eq!(ok(home.at(ok(Spot::new(0, 0, 3)))), Some("Steam"));
        assert_eq!(ok(home.every()).count(), 4);
    }

    #[test]
    fn one_of_ours_that_is_not_installed_leaves_no_hole() {
        let order: Vec<String> = vec!["Music".to_string(), "Steam".to_string()];
        let home = ok(Home::first(&order, GRID));

        assert_eq!(ok(home.at(Spot::FIRST)), Some("Music"));
        assert_eq!(ok(home.at(ok(Spot::new(0, 0, 1)))), Some("Steam"));
        assert_eq!(ok(home.every()).count(), 2);
    }

    #[test]
    fn the_first_pane_is_as_full_as_it_gets_and_no_fuller() {
        let order: Vec<String> = (0..100).map(|at| format!("App {at}")).collect();
        let home = ok(Home::first(&order, GRID));

        assert_eq!(ok(home.every()).count(), ROWS * COLUMNS);
        assert!(ok(home.every()).all(|(spot, _)| spot.pane == 0));
    }

    #[test]
    fn what_is_on_the_home_screen_is_kept_where_state_is_kept() {
        assert_eq!(
            ok(file(std::path::Path::new("/home/somebody"))),
            std::path::PathBuf::from("/home/somebody/.local/state/console/home")
        );
    }

    #[test]
    fn the_first_free_square_is_the_first_one_reading_across() {
        let mut home = Home::default();
        assert_eq!(ok(home.first_free(GRID)), Spot::FIRST);

        ok(home.place(Spot::FIRST, "Files"));
        assert_eq!(ok(home.first_free(GRID)), ok(Spot::new(0, 0, 1)));

        for (at, name) in (0..2 * ROWS * COLUMNS).map(|at| (at, format!("App {at}"))) {
            let pane = at / (ROWS * COLUMNS);
            let left = at % (ROWS * COLUMNS);
            ok(home.place(ok(Spot::new(pane, left / COLUMNS, left % COLUMNS)), &name));
        }

        assert_eq!(
            ok(home.first_free(GRID)),
            ok(Spot::new(2, 0, 0)),
            "every pane full is answered with a fresh one"
        );
    }

    #[test]
    fn an_application_is_found_by_name_and_taken_off_by_name() {
        let mut home = Home::default();
        ok(home.place(ok(Spot::new(1, 2, 3)), "Steam"));

        assert_eq!(ok(home.where_("Steam")), Some(ok(Spot::new(1, 2, 3))));
        assert_eq!(ok(home.where_("Files")), None);

        ok(home.forget("Steam"));
        assert_eq!(ok(home.where_("Steam")), None);
        assert_eq!(ok(home.holding()), Holding::Nothing);
    }

    #[test]
    fn what_is_taken_off_is_off() {
        let mut home = ok(Home::first(&["Files".to_string()], GRID));
        ok(home.remove(Spot::FIRST));

        assert_eq!(ok(home.holding()), Holding::Nothing);
        assert_eq!(ok(home.written()), "");
    }
}
