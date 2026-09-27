//! Where a tab is standing, and the way back up.

use std::path::{Path, PathBuf};

use console_core_never::Never;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Walk {
    top: PathBuf,
    at: PathBuf,
    marks: Vec<u32>,
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
        let named = file_name(&self.at)?;

        Ok(match at_top {
            Top::Yes => place.to_string(),
            Top::No => named,
        })
    }

    pub fn above(&self, place: &str) -> Result<Option<String>, Never> {
        let above = match self.at.parent() {
            Some(above) => above,
            None => &self.top,
        };

        let holding = file_name(above)?;

        Ok(match self.marks.len() {
            0 => None,
            1 => Some(place.to_string()),
            _ => Some(holding),
        })
    }

    pub fn enter(&mut self, name: &str, from: u32) -> Result<(), Never> {
        self.at.push(name);
        self.marks.push(from);

        Ok(())
    }

    pub fn up(&mut self) -> Result<Option<u32>, Never> {
        let back_to = match self.marks.pop() {
            Some(back_to) => back_to,
            None => return Ok(None),
        };

        match self.at.parent().map(Path::to_path_buf) {
            Some(above) => self.at = above,
            None => {},
        }

        Ok(Some(back_to))
    }
}

fn file_name(path: &Path) -> Result<String, Never> {
    Ok(match path.file_name() {
        None => path.display().to_string(),
        Some(name) => name.to_string_lossy().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pictures() -> Result<Walk, Never> {
        let Ok(walk) = Walk::of(Path::new("/home/ada/Pictures"));

        Ok(walk)
    }

    fn at(top: &str) -> Result<Walk, Never> {
        let Ok(walk) = Walk::of(Path::new(top));

        Ok(walk)
    }

    fn into(walk: &mut Walk, name: &str, from: u32) -> Result<(), Never> {
        let Ok(()) = walk.enter(name, from);

        Ok(())
    }

    #[test]
    fn a_tab_starts_at_the_top_of_its_place() {
        let Ok(walk) = pictures();

        assert_eq!(walk.at_top(), Ok(Top::Yes));
        assert_eq!(walk.above("Pictures"), Ok(None));
    }

    #[test]
    fn walking_in_goes_down_one_and_names_the_way_back() {
        let Ok(mut walk) = pictures();
        let Ok(()) = into(&mut walk, "2026", 4);

        assert_eq!(walk.here(), Ok(Path::new("/home/ada/Pictures/2026")));
        assert_eq!(walk.at_top(), Ok(Top::No));
        assert_eq!(walk.above("Pictures"), Ok(Some("Pictures".to_string())));
    }

    #[test]
    fn the_top_of_a_place_is_called_what_its_tab_is_called() {
        let Ok(mut walk) = at("/home/ada");

        assert_eq!(walk.called("Home"), Ok("Home".to_string()));

        let Ok(()) = into(&mut walk, "Projects", 2);

        assert_eq!(walk.called("Home"), Ok("Projects".to_string()));
        assert_eq!(walk.above("Home"), Ok(Some("Home".to_string())));

        let Ok(()) = into(&mut walk, "console", 1);

        assert_eq!(walk.above("Home"), Ok(Some("Projects".to_string())));
    }

    #[test]
    fn back_unwinds_to_the_top_and_then_says_it_has_nowhere_left() {
        let Ok(mut walk) = pictures();
        let Ok(()) = into(&mut walk, "2026", 4);
        let Ok(()) = into(&mut walk, "summer", 2);
        let Ok(()) = into(&mut walk, "boat", 7);

        assert_eq!(walk.up(), Ok(Some(7)));
        assert_eq!(walk.up(), Ok(Some(2)));
        assert_eq!(walk.up(), Ok(Some(4)));
        assert_eq!(walk.here(), Ok(Path::new("/home/ada/Pictures")));
        assert_eq!(walk.up(), Ok(None));
    }

    #[test]
    fn coming_back_up_stands_on_the_folder_you_came_out_of() {
        let Ok(mut walk) = pictures();
        let Ok(()) = into(&mut walk, "2026", 4);

        assert_eq!(walk.up(), Ok(Some(4)));
    }

    #[test]
    fn a_place_mounted_at_the_root_of_itself_still_has_something_to_say() {
        let Ok(mut walk) = at("/");
        let Ok(()) = into(&mut walk, "etc", 1);
        let Ok(()) = into(&mut walk, "console", 0);

        assert_eq!(walk.above("Disk"), Ok(Some("etc".to_string())));
        assert_eq!(walk.called("Disk"), Ok("console".to_string()));
    }
}
