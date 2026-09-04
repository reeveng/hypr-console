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
//! read two ways, which is why `console_menu::found` answers both.
//!
//! Everything here is about the grid and the file, and none of it draws: the
//! surface is `console-home`, the card that puts something on it is
//! `home-place`, and both are held to what this says.

use std::collections::BTreeMap;

pub mod shape;

pub use shape::{Shape, Square, square};

/// Where what is on the home screen is written down.
///
/// Under the state directory rather than the cache: a cache is a thing that
/// can be worked out again, and where somebody put their applications cannot
/// be. Taken as an argument so it can be tried without a home directory.
pub fn file(home: &std::path::Path) -> std::path::PathBuf {
    home.join(".local/state/console/home")
}

/// This desktop's own applications, in the order a first home screen puts
/// them.
///
/// Named rather than found, because there is nothing in a desktop file that
/// says "this one came with the machine": they are ordinary entries under
/// `/usr/share/applications` like everything else. Held to the files that
/// install them by `the_desktops_own_applications_are_on_this_machine`, so a
/// name changed on one side is a test that fails rather than a square that
/// quietly does not fill.
pub const OURS: [&str; 5] = ["Files", "Music", "Download", "Notifications", "Buttons"];

/// One square on the home screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Spot {
    pub pane: usize,
    pub row: usize,
    pub column: usize,
}

impl Spot {
    pub const FIRST: Spot = Spot { pane: 0, row: 0, column: 0 };

    pub fn new(pane: usize, row: usize, column: usize) -> Spot {
        Spot { pane, row, column }
    }

    /// Whether this is a square the grid has, in the shape it is in now.
    ///
    /// The pane is held to nothing, because a pane is not a shape the grid has
    /// -- it exists because something is on it, and a far pane in the file is a
    /// home screen with that many panes.
    ///
    /// A file written when the grid was another shape names squares that are
    /// not on it any more, and this is what notices. What happens to those is
    /// [`Home::fitted`]'s: they are moved onto the grid rather than dropped,
    /// because the shape is now something a person changes on a settings tab
    /// and a narrower grid must not be a way to lose applications.
    pub fn on_the_grid(self, shape: Shape) -> On {
        match self.row < shape.rows && self.column < shape.columns {
            true => On::TheGrid,
            false => On::Nothing,
        }
    }
}

/// Whether a square is one the home screen has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum On {
    TheGrid,
    Nothing,
}

/// Which way the d-pad went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    Up,
    Down,
    Left,
    Right,
}

/// Where the d-pad lands, going that way from here.
///
/// Left off the first column is the pane before, at the far column of the same
/// row, and right off the last is the pane after -- which is how the panes are
/// reached without a button of their own. The shoulders stay the workspaces'
/// on the home screen, because the home screen is the desktop and leaving it
/// has to go on working the way it does everywhere else.
///
/// How many panes there are to walk is handed in rather than known here,
/// because the panes are as many as what is on the home screen needs -- and
/// one more while something is being carried, which only the screen can know.
///
/// Up off the top and down off the bottom stay where they are. There is
/// nothing above the first row but the bar, and a highlight that wrapped to
/// the bottom would be a thumb that has lost where it was.
pub fn moved(spot: Spot, way: Way, panes: usize, shape: Shape) -> Spot {
    let (columns, rows) = (shape.columns.max(1), shape.rows.max(1));

    match way {
        Way::Up => Spot { row: spot.row.saturating_sub(1), ..spot },
        Way::Down => Spot { row: (spot.row + 1).min(rows - 1), ..spot },
        Way::Left => match (spot.column, spot.pane) {
            (0, 0) => spot,
            (0, pane) => Spot { pane: pane - 1, column: columns - 1, ..spot },
            (column, _) => Spot { column: column - 1, ..spot },
        },
        Way::Right => match (spot.column + 1, spot.pane + 1) {
            (at, after) if at >= columns && after >= panes => spot,
            (at, pane) if at >= columns => Spot { pane, column: 0, ..spot },
            (column, _) => Spot { column, ..spot },
        },
    }
}

/// Which pane a swipe asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Along {
    /// The one to the left, which a finger reaches by dragging right.
    Before,
    After,
}

/// The pane a swipe lands on, from this one.
///
/// One at a time, because a swipe is one flick and a flick is one pane -- and
/// kept inside the panes there are rather than wrapping, for the reason the
/// rows do not wrap: the end of the last pane is a place, and a screen that
/// went from there back to the first would be one that moved when somebody
/// meant to find out they had reached the end. How many there are is handed
/// in, for the reason [`moved`] is handed it.
pub fn paned(spot: Spot, along: Along, panes: usize) -> Spot {
    let pane = match along {
        Along::Before => spot.pane.saturating_sub(1),
        Along::After => (spot.pane + 1).min(panes.saturating_sub(1)),
    };

    Spot { pane, ..spot }
}

/// How far a finger may travel and still have gone nowhere, in pixels.
///
/// A thumb never comes up exactly where it went down, so a press is allowed
/// some wander; a swipe crosses a good part of the screen. There is a wide gap
/// between those two and nothing here has to be exact -- this is well inside a
/// square, so a finger that meant the square it is on is never read as having
/// left it, and well under a flick.
const DRIFT: f64 = 24.0;

/// What a finger did between going down and coming up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    /// It stayed where it was, which is a press of what is under it.
    Pressed,
    /// It went somewhere. A swipe, or a thumb that changed its mind: either
    /// way it is not a press of the square it started on.
    Travelled,
}

/// Whether a finger that went down here and came up there pressed anything.
///
/// A swipe is a finger on the surface itself, and it starts on whatever
/// happens to be under the thumb -- which on a screen of applications is
/// usually one of them. The release at the end of that swipe is the same
/// release a tap ends with, so without asking how far it went, one flick both
/// moved the panes and opened an application. How far it went is the whole
/// difference: a press does not go anywhere.
pub fn touched(from: (f64, f64), to: (f64, f64)) -> Touch {
    match (to.0 - from.0).hypot(to.1 - from.1) <= DRIFT {
        true => Touch::Pressed,
        false => Touch::Travelled,
    }
}

/// What is on the home screen: an application on some of the squares.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Home {
    placed: BTreeMap<Spot, String>,
}

impl Home {
    /// What the file says.
    ///
    /// A line is a pane, a row, a column and a name, tab-separated, as the
    /// menu's own remembered list is. A line that is not those four is not a
    /// placement -- which makes a half-written file a home screen with fewer
    /// things on it rather than one that will not draw.
    ///
    /// A square the grid has not got is kept, not dropped. The shape is a
    /// thing somebody sets on a settings tab now, and a file read against a
    /// narrower grid than it was written for would be a way to lose
    /// applications by pressing minus twice. [`Home::fitted`] is what moves
    /// them back onto it.
    pub fn read(said: &str) -> Home {
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

            let spot = Spot::new(pane, row, column);
            let name = name.trim();

            if !name.is_empty() {
                placed.insert(spot, name.to_string());
            }
        }

        Home { placed }
    }

    /// The file, as this says it.
    pub fn written(&self) -> String {
        self.placed
            .iter()
            .map(|(spot, name)| format!("{}\t{}\t{}\t{name}\n", spot.pane, spot.row, spot.column))
            .collect()
    }

    /// What is on that square.
    pub fn at(&self, spot: Spot) -> Option<&str> {
        self.placed.get(&spot).map(String::as_str)
    }

    /// Put this there, over whatever was there.
    pub fn place(&mut self, spot: Spot, name: &str) {
        self.placed.insert(spot, name.to_string());
    }

    /// The same home screen, with everything on a square this grid has.
    ///
    /// What a shape change means. Somebody takes a column off on the settings
    /// tab, and the far column of every pane is a set of applications standing
    /// where there is no longer a square: dropping them would be a press of
    /// minus that throws things away, and clamping them onto the last column
    /// would fold two onto one and lose the one underneath.
    ///
    /// So they are picked up and put down again in the first free square, in
    /// the order they were read, which is across a pane and then down it. What
    /// somebody sees is the far column folding round onto the end of the
    /// pane -- and a pane after this one, if that is what it takes.
    ///
    /// Untouched where nothing has moved, so the ordinary case reads the file
    /// and hands it straight back.
    pub fn fitted(&self, shape: Shape) -> Home {
        let (kept, adrift): (Vec<_>, Vec<_>) = self
            .placed
            .iter()
            .map(|(spot, name)| (*spot, name.clone()))
            .partition(|(spot, _)| spot.on_the_grid(shape) == On::TheGrid);

        if adrift.is_empty() {
            return Home { placed: self.placed.clone() };
        }

        let mut home = Home { placed: kept.into_iter().collect() };

        for (_, name) in adrift {
            let spot = home.first_free(shape);
            home.place(spot, &name);
        }

        home
    }

    /// Take whatever is there off.
    pub fn remove(&mut self, spot: Spot) {
        self.placed.remove(&spot);
    }

    /// Every square that has something on it.
    pub fn every(&self) -> impl Iterator<Item = (Spot, &str)> {
        self.placed.iter().map(|(spot, name)| (*spot, name.as_str()))
    }

    /// The home screen a machine that has never had one gets.
    ///
    /// The applications it is opened most, in that order, filling the first
    /// pane left to right and top to bottom. An empty home screen on a first
    /// boot would be the wallpaper this is here to replace.
    ///
    /// On the first boot there are no counts, and the order is then whatever
    /// alphabetical gives -- which on this machine was Aether, Alacritty and
    /// three Avahi entries, none of which anybody put there and one of which
    /// is a network browser for printers. So the desktop's own go first, in
    /// the order they are written in [`OURS`]: they are the ones a machine
    /// with nothing on it can be relied upon to have, and they are what
    /// somebody who has just turned it on is looking for.
    pub fn first(order: &[String], shape: Shape) -> Home {
        let mut home = Home::default();
        let ours = OURS.iter().map(|said| said.to_string());
        let rest = order.iter().filter(|name| !OURS.contains(&name.as_str())).cloned();
        let names = ours.filter(|name| order.contains(name)).chain(rest);
        let columns = shape.columns.max(1);

        for (at, name) in names.take(shape.squares()).enumerate() {
            home.place(Spot::new(0, at / columns, at % columns), &name);
        }

        home
    }

    /// How many panes there are: as many as what is placed reaches, and never
    /// fewer than one.
    ///
    /// Not a number anybody set. A pane exists because something is on it, so
    /// the count is read off the placements: put something on a fresh pane and
    /// there is one more, take the last thing off the far pane and there is
    /// one fewer. A pane emptied in the middle stays, because the panes after
    /// it are places somebody learnt.
    pub fn panes(&self) -> usize {
        self.placed.keys().map(|spot| spot.pane + 1).max().unwrap_or(1)
    }

    /// The first square with nothing on it, reading the way the squares are
    /// read: across the first pane, then down it, then the pane after.
    ///
    /// What "add this to the home screen" means when nobody said where. There
    /// is always one: every pane full is answered with the first square of a
    /// fresh pane, which is how the home screen holds as much as anybody
    /// wants to put on it.
    pub fn first_free(&self, shape: Shape) -> Spot {
        let (columns, rows) = (shape.columns, shape.rows);

        (0..self.panes())
            .flat_map(|pane| {
                (0..rows).flat_map(move |row| (0..columns).map(move |column| Spot::new(pane, row, column)))
            })
            .find(|spot| self.at(*spot).is_none())
            // Every pane there is came out full, and the pane after them is
            // empty by what a pane is: it would already count if anything
            // stood on it.
            .unwrap_or_else(|| Spot::new(self.panes(), 0, 0))
    }

    /// Where this is, if it is anywhere.
    ///
    /// One square, because the card that puts things on the home screen is a
    /// list of applications rather than of squares: what it does to a row is
    /// decided by whether the application is on the screen at all.
    pub fn where_(&self, name: &str) -> Option<Spot> {
        self.placed.iter().find(|(_, placed)| placed.as_str() == name).map(|(spot, _)| *spot)
    }

    /// Take this off, wherever it is.
    pub fn forget(&mut self, name: &str) {
        self.placed.retain(|_, placed| placed != name);
    }

    /// Whether anything is on it at all.
    pub fn holding(&self) -> Holding {
        match self.placed.is_empty() {
            true => Holding::Nothing,
            false => Holding::Something,
        }
    }
}

/// Whether the home screen has anything on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holding {
    Something,
    /// Which is what a machine that has never drawn one looks like, and what
    /// somebody who has taken everything off has asked for.
    Nothing,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grid every one of these is about, unless it says otherwise: the
    /// five by three the home screen was written as.
    const GRID: Shape = Shape::USUAL;
    const COLUMNS: usize = GRID.columns;
    const ROWS: usize = GRID.rows;

    #[test]
    fn the_dpad_walks_the_squares_and_falls_off_neither_end() {
        let middle = Spot::new(1, 1, 2);
        assert_eq!(moved(middle, Way::Up, 3, GRID), Spot::new(1, 0, 2));
        assert_eq!(moved(middle, Way::Down, 3, GRID), Spot::new(1, 2, 2));
        assert_eq!(moved(middle, Way::Left, 3, GRID), Spot::new(1, 1, 1));
        assert_eq!(moved(middle, Way::Right, 3, GRID), Spot::new(1, 1, 3));

        let top = Spot::new(0, 0, 0);
        assert_eq!(moved(top, Way::Up, 3, GRID), top, "there is nothing above the first row");
        assert_eq!(moved(top, Way::Left, 3, GRID), top, "nor before the first pane");

        let last = Spot::new(2, ROWS - 1, COLUMNS - 1);
        assert_eq!(moved(last, Way::Down, 3, GRID), last);
        assert_eq!(moved(last, Way::Right, 3, GRID), last, "the panes end where the caller said");
    }

    /// The panes are reached by walking off the side, because the shoulders
    /// are the workspaces' on the home screen as they are on the desktop.
    #[test]
    fn walking_off_the_side_of_a_pane_is_how_the_next_one_is_reached() {
        assert_eq!(moved(Spot::new(0, 1, COLUMNS - 1), Way::Right, 2, GRID), Spot::new(1, 1, 0));
        assert_eq!(moved(Spot::new(1, 1, 0), Way::Left, 2, GRID), Spot::new(0, 1, COLUMNS - 1));
    }

    /// One more pane than the home screen holds is what the screen asks to
    /// walk while something is carried: the same step right that stops at the
    /// last pane with an empty hand walks onto a fresh one with a full one.
    #[test]
    fn a_full_hand_is_offered_the_pane_past_the_end_and_an_empty_one_is_not() {
        let edge = Spot::new(1, 0, COLUMNS - 1);
        assert_eq!(moved(edge, Way::Right, 2, GRID), edge);
        assert_eq!(moved(edge, Way::Right, 3, GRID), Spot::new(2, 0, 0));
    }

    #[test]
    fn a_swipe_moves_a_whole_pane_and_stops_at_the_ends() {
        assert_eq!(paned(Spot::new(0, 2, 3), Along::After, 2), Spot::new(1, 2, 3));
        assert_eq!(paned(Spot::new(0, 2, 3), Along::Before, 2), Spot::new(0, 2, 3));
        assert_eq!(paned(Spot::new(1, 0, 0), Along::After, 2), Spot::new(1, 0, 0));
    }

    /// A swipe ends the way a tap does, with the finger coming up, and it
    /// began on whatever square the thumb was over. That release is not a
    /// press of it.
    #[test]
    fn a_finger_that_travelled_pressed_nothing() {
        assert_eq!(touched((100.0, 100.0), (100.0, 100.0)), Touch::Pressed);
        assert_eq!(touched((100.0, 100.0), (108.0, 94.0)), Touch::Pressed, "a thumb wanders");
        assert_eq!(touched((100.0, 100.0), (400.0, 108.0)), Touch::Travelled, "a pane, sideways");
        assert_eq!(touched((100.0, 300.0), (112.0, 40.0)), Touch::Travelled, "the menu, upwards");
    }

    #[test]
    fn what_is_placed_is_what_is_written_down_and_read_back() {
        let mut home = Home::default();
        home.place(Spot::new(0, 0, 0), "Files");
        home.place(Spot::new(2, 1, 4), "Steam");

        assert_eq!(home.written(), "0\t0\t0\tFiles\n2\t1\t4\tSteam\n");
        assert_eq!(Home::read(&home.written()), home);
        assert_eq!(home.at(Spot::new(2, 1, 4)), Some("Steam"));
        assert_eq!(home.at(Spot::new(2, 1, 3)), None);
    }

    /// A name with a space in it is one field and an ordinary name. The tab is
    /// what divides them, and `console_menu::kept` keeps tabs out of a name
    /// for the same reason.
    #[test]
    fn a_name_with_spaces_in_it_survives_the_writing_down() {
        let home = Home::read("1\t0\t2\tText Editor\n");
        assert_eq!(home.at(Spot::new(1, 0, 2)), Some("Text Editor"));
    }

    #[test]
    fn a_line_that_is_not_a_placement_is_not_a_placement() {
        let home = Home::read("\nnonsense\n0\t0\n0\t0\t0\t\nx\ty\tz\tFiles\n0\t0\t0\tFiles\n");
        assert_eq!(home.every().count(), 1);
        assert_eq!(home.at(Spot::FIRST), Some("Files"));
    }

    /// Written when a pane was wider or taller, read on the pane there is.
    ///
    /// Kept and moved onto the grid, not dropped. The shape is a thing somebody
    /// sets on a settings tab, and a press of minus that throws applications
    /// away is a press nobody would make twice.
    #[test]
    fn a_square_this_grid_does_not_have_is_moved_onto_it_rather_than_dropped() {
        let home = Home::read(&format!("0\t0\t{COLUMNS}\tFiles\n0\t{ROWS}\t0\tSteam\n"));

        assert_eq!(home.holding(), Holding::Something, "nothing is thrown away by reading");

        let fitted = home.fitted(GRID);
        let names: Vec<&str> = fitted.every().map(|(_, name)| name).collect();

        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names.contains(&"Files") && names.contains(&"Steam"), "{names:?}");
        assert!(
            fitted.every().all(|(spot, _)| spot.on_the_grid(GRID) == On::TheGrid),
            "something is still off the grid"
        );
    }

    /// Taking a column off folds the far column round onto the end of the
    /// pane, and onto the pane after it where that is what it takes. Nothing
    /// lands on top of anything.
    #[test]
    fn narrowing_the_grid_folds_what_was_off_it_round_rather_than_over_anything() {
        let mut home = Home::default();

        for row in 0..GRID.rows {
            for column in 0..GRID.columns {
                home.place(Spot::new(0, row, column), &format!("{row}-{column}"));
            }
        }

        let narrow = GRID.across(3);
        let fitted = home.fitted(narrow);

        assert_eq!(fitted.every().count(), home.every().count(), "something was folded over");
        assert!(fitted.every().all(|(spot, _)| spot.on_the_grid(narrow) == On::TheGrid));
        assert!(fitted.panes() > 1, "fifteen things do not fit on nine squares");
    }

    /// And a grid that has not changed hands the same home screen straight
    /// back, so the ordinary reading costs nothing and moves nothing.
    #[test]
    fn a_grid_nothing_is_off_leaves_every_square_where_it_was() {
        let mut home = Home::default();
        home.place(Spot::new(0, 0, 0), "Files");
        home.place(Spot::new(1, 2, 4), "Steam");

        assert_eq!(home.fitted(GRID), home);
    }

    /// A pane is not a shape the grid has: it exists because something is on
    /// it. A file naming a far pane is a home screen with that many panes,
    /// not a placement to drop.
    #[test]
    fn a_far_pane_in_the_file_is_a_home_screen_with_that_many_panes() {
        let home = Home::read("7\t0\t0\tSteam\n");
        assert_eq!(home.at(Spot::new(7, 0, 0)), Some("Steam"));
        assert_eq!(home.panes(), 8);
    }

    /// The count is read off the placements, and an empty middle pane stays:
    /// the panes after it are places somebody learnt.
    #[test]
    fn there_are_as_many_panes_as_what_is_placed_reaches() {
        let mut home = Home::default();
        assert_eq!(home.panes(), 1, "an empty home screen is one pane of room");

        home.place(Spot::new(2, 1, 1), "Steam");
        home.place(Spot::new(0, 0, 0), "Files");
        assert_eq!(home.panes(), 3);

        home.forget("Files");
        assert_eq!(home.panes(), 3, "an emptied middle pane is still a place");

        home.forget("Steam");
        assert_eq!(home.panes(), 1, "the far panes go when the last thing on them does");
    }

    #[test]
    fn a_machine_that_has_never_had_one_opens_on_what_it_uses_most() {
        let order: Vec<String> =
            ["Aether", "Files", "Music", "Steam"].iter().map(|said| said.to_string()).collect();
        let home = Home::first(&order, GRID);

        assert_eq!(home.at(Spot::FIRST), Some("Files"), "the desktop's own come first");
        assert_eq!(home.at(Spot::new(0, 0, 1)), Some("Music"));
        assert_eq!(home.at(Spot::new(0, 0, 2)), Some("Aether"), "and the rest in their own order");
        assert_eq!(home.at(Spot::new(0, 0, 3)), Some("Steam"));
        assert_eq!(home.every().count(), 4);
    }

    /// One of ours that this machine has not got is not a square left blank
    /// in the middle of the first pane.
    #[test]
    fn one_of_ours_that_is_not_installed_leaves_no_hole() {
        let order: Vec<String> = vec!["Music".to_string(), "Steam".to_string()];
        let home = Home::first(&order, GRID);

        assert_eq!(home.at(Spot::FIRST), Some("Music"));
        assert_eq!(home.at(Spot::new(0, 0, 1)), Some("Steam"));
        assert_eq!(home.every().count(), 2);
    }

    /// More applications than a pane holds is a first pane that is full, and
    /// nothing spilled onto a pane somebody has not seen yet.
    #[test]
    fn the_first_pane_is_as_full_as_it_gets_and_no_fuller() {
        let order: Vec<String> = (0..100).map(|at| format!("App {at}")).collect();
        let home = Home::first(&order, GRID);

        assert_eq!(home.every().count(), ROWS * COLUMNS);
        assert!(home.every().all(|(spot, _)| spot.pane == 0));
    }

    #[test]
    fn what_is_on_the_home_screen_is_kept_where_state_is_kept() {
        assert_eq!(
            file(std::path::Path::new("/home/somebody")),
            std::path::PathBuf::from("/home/somebody/.local/state/console/home")
        );
    }

    #[test]
    fn the_first_free_square_is_the_first_one_reading_across() {
        let mut home = Home::default();
        assert_eq!(home.first_free(GRID), Spot::FIRST);

        home.place(Spot::FIRST, "Files");
        assert_eq!(home.first_free(GRID), Spot::new(0, 0, 1));

        for (at, name) in (0..2 * ROWS * COLUMNS).map(|at| (at, format!("App {at}"))) {
            let pane = at / (ROWS * COLUMNS);
            let left = at % (ROWS * COLUMNS);
            home.place(Spot::new(pane, left / COLUMNS, left % COLUMNS), &name);
        }

        assert_eq!(
            home.first_free(GRID),
            Spot::new(2, 0, 0),
            "every pane full is answered with a fresh one"
        );
    }

    #[test]
    fn an_application_is_found_by_name_and_taken_off_by_name() {
        let mut home = Home::default();
        home.place(Spot::new(1, 2, 3), "Steam");

        assert_eq!(home.where_("Steam"), Some(Spot::new(1, 2, 3)));
        assert_eq!(home.where_("Files"), None);

        home.forget("Steam");
        assert_eq!(home.where_("Steam"), None);
        assert_eq!(home.holding(), Holding::Nothing);
    }

    #[test]
    fn what_is_taken_off_is_off() {
        let mut home = Home::first(&["Files".to_string()], GRID);
        home.remove(Spot::FIRST);

        assert_eq!(home.holding(), Holding::Nothing);
        assert_eq!(home.written(), "");
    }
}
