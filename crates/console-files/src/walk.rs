//! Where a tab is standing, and the way back up.

use std::path::{Path, PathBuf};

/// A place, and how far into it you have walked.
///
/// The place is the top and nothing goes above it, so B has an end: three
/// folders deep is three presses back to the place and a fourth closes the
/// panel. Walking up past the place a tab is about would arrive at the root of
/// the disk eventually, which is nowhere anybody holding this asked to be, and
/// it would leave the tab saying Pictures over a listing of /etc.
///
/// Every step down remembers which row it was taken from, so coming back up
/// stands on the folder you came out of rather than at the top of a list you
/// have already read. The row is remembered rather than the name because the
/// panel is told where to stand by number, and because a folder read a second
/// time can have gained something since: the name would have to be searched
/// for, on the thread that draws, before the listing it is being searched in
/// has arrived.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Walk {
    top: PathBuf,
    at: PathBuf,
    /// The row each step down was taken from, deepest last.
    marks: Vec<usize>,
}

/// Whether a walk is at the top of its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Top {
    /// It is, so the row that goes back goes to the tab's own name.
    Yes,
    /// It is not, so there is a folder above this one.
    No,
}

impl Walk {
    /// A tab standing at the top of its place.
    pub fn of(top: &Path) -> Self {
        Walk { top: top.to_path_buf(), at: top.to_path_buf(), marks: Vec::new() }
    }

    /// The folder being shown.
    pub fn here(&self) -> &Path {
        &self.at
    }

    /// Whether there is anywhere above this.
    pub fn at_top(&self) -> Top {
        match self.marks.is_empty() {
            true => Top::Yes,
            false => Top::No,
        }
    }

    /// What the folder being shown is called, given what its place is called.
    ///
    /// A place is named by its tab rather than by the folder underneath it. A
    /// home directory is called whatever the person living in it is called, and
    /// a row offering to go back to `ada` is the machine's name for the place
    /// the strip calls Home. Below the top there is no tab to ask, and the
    /// folder's own name is the only name it has.
    pub fn called(&self, place: &str) -> String {
        match self.at_top() {
            Top::Yes => place.to_string(),
            Top::No => named(&self.at),
        }
    }

    /// What the folder above is called, or nothing at the top of the place.
    pub fn above(&self, place: &str) -> Option<String> {
        match self.marks.len() {
            0 => None,
            1 => Some(place.to_string()),
            _ => Some(named(self.at.parent().unwrap_or(&self.top))),
        }
    }

    /// Go into one of the folders being shown, from the row it was chosen on.
    pub fn enter(&mut self, name: &str, from: usize) {
        self.at.push(name);
        self.marks.push(from);
    }

    /// Up one, standing where you were when you came down.
    ///
    /// Nothing at the top, which is the tab saying it has no way out of its own
    /// and that back means the panel.
    pub fn up(&mut self) -> Option<usize> {
        let back_to = self.marks.pop()?;

        if let Some(above) = self.at.parent().map(Path::to_path_buf) {
            self.at = above;
        }

        Some(back_to)
    }
}

/// A path, by the name at the end of it.
///
/// A place mounted at the root of itself has no name to take, and a drive is
/// mounted at one often enough to be worth an answer rather than an empty row.
fn named(path: &Path) -> String {
    match path.file_name() {
        None => path.display().to_string(),
        Some(name) => name.to_string_lossy().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pictures() -> Walk {
        Walk::of(Path::new("/home/ada/Pictures"))
    }

    #[test]
    fn a_tab_starts_at_the_top_of_its_place() {
        let walk = pictures();
        assert_eq!(walk.at_top(), Top::Yes);
        assert_eq!(walk.above("Pictures"), None);
    }

    #[test]
    fn walking_in_goes_down_one_and_names_the_way_back() {
        let mut walk = pictures();
        walk.enter("2026", 4);
        assert_eq!(walk.here(), Path::new("/home/ada/Pictures/2026"));
        assert_eq!(walk.at_top(), Top::No);
        assert_eq!(walk.above("Pictures").as_deref(), Some("Pictures"));
    }

    /// The home directory is called `ada` and the tab is called Home. Every
    /// row that named the place was naming it the way the disk does.
    #[test]
    fn the_top_of_a_place_is_called_what_its_tab_is_called() {
        let mut walk = Walk::of(Path::new("/home/ada"));
        assert_eq!(walk.called("Home"), "Home");
        walk.enter("Projects", 2);
        assert_eq!(walk.called("Home"), "Projects");
        assert_eq!(walk.above("Home").as_deref(), Some("Home"));
        walk.enter("console", 1);
        assert_eq!(walk.above("Home").as_deref(), Some("Projects"));
    }

    /// The whole of what B is for. Three down is three presses back to the
    /// place, and the fourth is the panel's rather than the tab's.
    #[test]
    fn back_unwinds_to_the_top_and_then_says_it_has_nowhere_left() {
        let mut walk = pictures();
        walk.enter("2026", 4);
        walk.enter("summer", 2);
        walk.enter("boat", 7);
        assert_eq!(walk.up(), Some(7));
        assert_eq!(walk.up(), Some(2));
        assert_eq!(walk.up(), Some(4));
        assert_eq!(walk.here(), Path::new("/home/ada/Pictures"));
        assert_eq!(walk.up(), None);
    }

    /// Coming out of a folder puts the highlight back on it. Standing at the
    /// top of the parent instead means counting down the list again, every
    /// time, in the one place a person is most likely to be going in and out
    /// of several folders in a row.
    #[test]
    fn coming_back_up_stands_on_the_folder_you_came_out_of() {
        let mut walk = pictures();
        walk.enter("2026", 4);
        assert_eq!(walk.up(), Some(4));
    }

    #[test]
    fn a_place_mounted_at_the_root_of_itself_still_has_something_to_say() {
        let mut walk = Walk::of(Path::new("/"));
        walk.enter("etc", 1);
        walk.enter("console", 0);
        assert_eq!(walk.above("Disk").as_deref(), Some("etc"));
        assert_eq!(walk.called("Disk"), "console");
    }
}
