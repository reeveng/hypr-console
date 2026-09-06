//! What a typed word finds under the folder being shown.
//!
//! A listing is walked into a folder at a time, which is the right way to read
//! a place somebody knows and the wrong way to find one thing in a place they
//! do not. So the line at the top of a folder is not a filter on what is in
//! front of you: it looks under everything below it as well, and the row says
//! where what it found is.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use console_never::Never;

use crate::listing::{self, Entry};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Found {
    pub thing: Entry,
    pub within: PathBuf,
}

impl Found {
    pub fn aside(&self) -> Result<String, Never> {
        match self.within.as_os_str().is_empty() {
            true => listing::aside(&self.thing),
            false => Ok(self.within.display().to_string()),
        }
    }

    pub fn at(&self, from: &Path) -> Result<PathBuf, Never> {
        Ok(from.join(&self.within).join(&self.thing.name))
    }

    pub fn steps(&self) -> Result<Vec<String>, Never> {
        Ok(self
            .within
            .components()
            .map(|part| part.as_os_str().to_string_lossy().to_string())
            .chain(std::iter::once(self.thing.name.clone()))
            .collect())
    }
}

pub fn answers(name: &str, word: &str) -> Result<Answers, Never> {
    Ok(match name.to_lowercase().contains(word.trim().to_lowercase().as_str()) {
        true => Answers::Yes,
        false => Answers::No,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answers {
    Yes,
    No,
}

const ENOUGH: usize = 120;
const FAR: usize = 600;

pub fn under(
    here: &Path,
    word: &str,
    read: &dyn Fn(&Path) -> Result<Vec<Entry>, Never>,
) -> Result<Vec<Found>, Never> {
    match word.trim().is_empty() {
        true => return Ok(Vec::new()),
        false => {},
    }

    let mut found: Vec<Found> = Vec::new();
    let mut waiting = VecDeque::from([PathBuf::new()]);
    let mut read_so_far: usize = 0;

    while let Some(within) = waiting.pop_front() {
        match found.len() >= ENOUGH || read_so_far >= FAR {
            true => break,
            false => {},
        }

        read_so_far = read_so_far.saturating_add(1);
        let at = match within.as_os_str().is_empty() {
            true => here.to_path_buf(),
            false => here.join(&within),
        };

        let things = read(&at)?;

        for thing in things {
            match thing.folder {
                true => waiting.push_back(within.join(&thing.name)),
                false => {},
            }

            let answers = answers(&thing.name, word)?;

            match answers {
                Answers::Yes => found.push(Found { thing, within: within.clone() }),
                Answers::No => {},
            }
        }
    }

    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> Entry {
        let Ok(entry) = Entry::folder(name);

        entry
    }

    fn file(name: &str, size: u64) -> Entry {
        let Ok(entry) = Entry::file(name, size);

        entry
    }

    fn tree(at: &Path) -> Result<Vec<Entry>, Never> {
        let said = at.to_string_lossy().to_string();
        let of = |names: &[&str], files: &[&str]| {
            let mut things: Vec<Entry> = names.iter().map(|name| folder(name)).collect();

            things.extend(files.iter().map(|name| file(name, 1)));

            let Ok(things) = listing::sorted(things);

            things
        };

        Ok(match said.as_str() {
            "/home" => of(&["Documents", "Pictures"], &["notes.txt"]),
            "/home/Documents" => of(&["Holiday"], &["taxes.pdf"]),
            "/home/Documents/Holiday" => of(&[], &["notes.txt", "beach.jpg"]),
            "/home/Pictures" => of(&[], &["beach.jpg"]),
            _ => Vec::new(),
        })
    }

    fn under_home(word: &str) -> Vec<Found> {
        let Ok(found) = under(Path::new("/home"), word, &tree);

        found
    }

    fn names(found: &[Found]) -> Vec<&str> {
        found.iter().map(|one| one.thing.name.as_str()).collect()
    }

    #[test]
    fn a_word_finds_what_is_under_the_folder_as_well_as_what_is_in_it() {
        assert_eq!(names(&under_home("notes")), ["notes.txt", "notes.txt"]);
        assert_eq!(under_home("notes")[0].within, PathBuf::new());
        assert_eq!(under_home("notes")[1].within, PathBuf::from("Documents/Holiday"));
    }

    #[test]
    fn what_is_nearest_is_found_first() {
        let found = under_home("beach");
        assert_eq!(found[0].within, PathBuf::from("Pictures"));
        assert_eq!(found[1].within, PathBuf::from("Documents/Holiday"));
    }

    #[test]
    fn a_folder_answers_to_a_word_the_same_way_a_file_does() {
        assert_eq!(names(&under_home("holi")), ["Holiday"]);
    }

    #[test]
    fn a_found_folder_is_arrived_at_a_step_at_a_time() {
        assert_eq!(under_home("holi")[0].steps(), Ok(vec!["Documents".to_string(), "Holiday".to_string()]));
        assert_eq!(under_home("docum")[0].steps(), Ok(vec!["Documents".to_string()]));
    }

    #[test]
    fn the_case_it_was_typed_in_does_not_matter() {
        assert_eq!(names(&under_home("TAXES")), ["taxes.pdf"]);
        assert_eq!(names(&under_home("  taxes ")), ["taxes.pdf"]);
    }

    #[test]
    fn nothing_typed_looks_at_nothing() {
        assert!(under_home("").is_empty());
        assert!(under_home("   ").is_empty());
    }

    #[test]
    fn a_word_nothing_answers_to_finds_nothing() {
        assert!(under_home("kangaroo").is_empty());
    }

    #[test]
    fn a_row_says_where_what_it_found_is() {
        let found = under_home("notes");

        assert_eq!(found[0].aside(), listing::aside(&found[0].thing));
        assert_eq!(found[1].aside(), Ok("Documents/Holiday".to_string()));
        assert_eq!(
            found[1].at(Path::new("/home")),
            Ok(PathBuf::from("/home/Documents/Holiday/notes.txt")),
        );
    }

    #[test]
    fn a_tree_that_goes_on_for_ever_is_still_left() {
        let round = |at: &Path| {
            Ok(match at.to_string_lossy().len() < 4000 {
                true => vec![folder("down"), file("notes.txt", 1)],
                false => Vec::new(),
            })
        };

        let Ok(found) = under(Path::new("/home"), "notes", &round);

        assert!(!found.is_empty());
        assert!(found.len() <= ENOUGH);
    }
}
