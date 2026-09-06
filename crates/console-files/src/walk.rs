//! Where a tab is standing, and the way back up.

use std::path::{Path, PathBuf};

use console_never::Never;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Walk {
    top: PathBuf,
    at: PathBuf,
    marks: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Top {
    Yes,
    No,
}

impl Walk {
    pub fn of(top: &Path) -> Result<Self, Never> {
        Ok(Walk { top: top.to_path_buf(), at: top.to_path_buf(), marks: Vec::new() })
    }

    pub fn here(&self) -> Result<&Path, Never> {
        Ok(&self.at)
    }

    pub fn at_top(&self) -> Result<Top, Never> {
        Ok(match self.marks.is_empty() {
            true => Top::Yes,
            false => Top::No,
        })
    }

    pub fn called(&self, place: &str) -> Result<String, Never> {
        let at_top = self.at_top()?;
        let named = named(&self.at)?;

        Ok(match at_top {
            Top::Yes => place.to_string(),
            Top::No => named,
        })
    }

    pub fn above(&self, place: &str) -> Result<Option<String>, Never> {
        let holding = named(self.at.parent().unwrap_or(&self.top))?;

        Ok(match self.marks.len() {
            0 => None,
            1 => Some(place.to_string()),
            _ => Some(holding),
        })
    }

    pub fn enter(&mut self, name: &str, from: usize) -> Result<(), Never> {
        self.at.push(name);
        self.marks.push(from);

        Ok(())
    }

    pub fn up(&mut self) -> Result<Option<usize>, Never> {
        let Some(back_to) = self.marks.pop() else { return Ok(None) };

        match self.at.parent().map(Path::to_path_buf) {
            Some(above) => self.at = above,
            None => {},
        }

        Ok(Some(back_to))
    }
}

fn named(path: &Path) -> Result<String, Never> {
    Ok(match path.file_name() {
        None => path.display().to_string(),
        Some(name) => name.to_string_lossy().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pictures() -> Walk {
        let Ok(walk) = Walk::of(Path::new("/home/ada/Pictures"));

        walk
    }

    fn at(top: &str) -> Walk {
        let Ok(walk) = Walk::of(Path::new(top));

        walk
    }

    fn into(walk: &mut Walk, name: &str, from: usize) {
        let Ok(()) = walk.enter(name, from);
    }

    #[test]
    fn a_tab_starts_at_the_top_of_its_place() {
        let walk = pictures();

        assert_eq!(walk.at_top(), Ok(Top::Yes));
        assert_eq!(walk.above("Pictures"), Ok(None));
    }

    #[test]
    fn walking_in_goes_down_one_and_names_the_way_back() {
        let mut walk = pictures();

        into(&mut walk, "2026", 4);

        assert_eq!(walk.here(), Ok(Path::new("/home/ada/Pictures/2026")));
        assert_eq!(walk.at_top(), Ok(Top::No));
        assert_eq!(walk.above("Pictures"), Ok(Some("Pictures".to_string())));
    }

    #[test]
    fn the_top_of_a_place_is_called_what_its_tab_is_called() {
        let mut walk = at("/home/ada");

        assert_eq!(walk.called("Home"), Ok("Home".to_string()));

        into(&mut walk, "Projects", 2);

        assert_eq!(walk.called("Home"), Ok("Projects".to_string()));
        assert_eq!(walk.above("Home"), Ok(Some("Home".to_string())));

        into(&mut walk, "console", 1);

        assert_eq!(walk.above("Home"), Ok(Some("Projects".to_string())));
    }

    #[test]
    fn back_unwinds_to_the_top_and_then_says_it_has_nowhere_left() {
        let mut walk = pictures();

        into(&mut walk, "2026", 4);
        into(&mut walk, "summer", 2);
        into(&mut walk, "boat", 7);

        assert_eq!(walk.up(), Ok(Some(7)));
        assert_eq!(walk.up(), Ok(Some(2)));
        assert_eq!(walk.up(), Ok(Some(4)));
        assert_eq!(walk.here(), Ok(Path::new("/home/ada/Pictures")));
        assert_eq!(walk.up(), Ok(None));
    }

    #[test]
    fn coming_back_up_stands_on_the_folder_you_came_out_of() {
        let mut walk = pictures();

        into(&mut walk, "2026", 4);

        assert_eq!(walk.up(), Ok(Some(4)));
    }

    #[test]
    fn a_place_mounted_at_the_root_of_itself_still_has_something_to_say() {
        let mut walk = at("/");

        into(&mut walk, "etc", 1);
        into(&mut walk, "console", 0);

        assert_eq!(walk.above("Disk"), Ok(Some("etc".to_string())));
        assert_eq!(walk.called("Disk"), Ok("console".to_string()));
    }
}
