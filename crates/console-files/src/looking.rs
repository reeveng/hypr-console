//! What a typed word finds under the folder being shown.
//!
//! A listing is walked into a folder at a time, which is the right way to read
//! a place somebody knows and the wrong way to find one thing in a place they
//! do not. So the line at the top of a folder is not a filter on what is in
//! front of you: it looks under everything below it as well, and the row says
//! where what it found is.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use crate::listing::{self, Entry};

/// One thing a word found, and the folder it was found in.
///
/// The folder is said from where the search began, because that is what the row
/// has to carry: three files called notes.txt are a list nobody can choose
/// from, and "Holiday" beside one of them is the whole answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Found {
    pub thing: Entry,
    /// Empty for something in the folder the search began in.
    pub within: PathBuf,
}

impl Found {
    /// What the row says beside the name: where it is, or, for something right
    /// here, whatever the listing would have said.
    pub fn aside(&self) -> String {
        match self.within.as_os_str().is_empty() {
            true => listing::aside(&self.thing),
            false => self.within.display().to_string(),
        }
    }

    /// Where it is, given where the search began.
    pub fn at(&self, from: &Path) -> PathBuf {
        from.join(&self.within).join(&self.thing.name)
    }

    /// The folders to walk into, in order, to arrive at this one.
    ///
    /// A search reaches past several folders at once and the walk a tab keeps
    /// is one folder at a time. Taken as steps, a folder found three deep has
    /// the two above it behind B, which is what backing out of it should mean;
    /// arrived at in one jump, its way back would be the place.
    pub fn steps(&self) -> Vec<String> {
        self.within
            .components()
            .map(|part| part.as_os_str().to_string_lossy().to_string())
            .chain(std::iter::once(self.thing.name.clone()))
            .collect()
    }
}

/// Whether a name answers to what has been typed.
///
/// Plain containment, the way the menu narrows. The letters arrive one thumb at
/// a time off a keyboard covering half the screen, and a list that rearranges
/// itself around a letter nobody meant to press is worse than one that simply
/// gets shorter.
pub fn answers(name: &str, word: &str) -> Answers {
    match name.to_lowercase().contains(word.trim().to_lowercase().as_str()) {
        true => Answers::Yes,
        false => Answers::No,
    }
}

/// Whether a name is one the typed word was looking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answers {
    /// The word is somewhere in it.
    Yes,
    /// It is not.
    No,
}

/// Where a search gives up finding, and where it gives up looking.
///
/// It reads a folder at a time, nearest first, so what it has when it stops is
/// what was closest to where she was standing. Both ends earn their place: a
/// home directory holds far more than anybody is going to read down, and a
/// folder that is a link to the one above it would otherwise be walked for as
/// long as the panel is open.
const ENOUGH: usize = 120;
const FAR: usize = 600;

/// Everything under a folder that answers to the word, nearest first.
///
/// The reading is handed in rather than done here, so a search can be asked
/// without a disk to ask it of. The panel hands it the same read its listing is
/// made of.
pub fn under(here: &Path, word: &str, read: &dyn Fn(&Path) -> Vec<Entry>) -> Vec<Found> {
    if word.trim().is_empty() {
        return Vec::new();
    }

    let mut found: Vec<Found> = Vec::new();
    let mut waiting = VecDeque::from([PathBuf::new()]);
    let mut read_so_far = 0;

    while let Some(within) = waiting.pop_front() {
        if found.len() >= ENOUGH || read_so_far >= FAR {
            break;
        }

        read_so_far += 1;
        // Joined onto nothing a path grows a separator, and a folder read as
        // "/home/" is a folder read twice under two names.
        let at = match within.as_os_str().is_empty() {
            true => here.to_path_buf(),
            false => here.join(&within),
        };

        for thing in read(&at) {
            if thing.folder {
                waiting.push_back(within.join(&thing.name));
            }

            if answers(&thing.name, word) == Answers::Yes {
                found.push(Found { thing, within: within.clone() });
            }
        }
    }

    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tree with no disk under it: a folder, and what is in it.
    fn tree(at: &Path) -> Vec<Entry> {
        let said = at.to_string_lossy().to_string();
        let of = |names: &[&str], files: &[&str]| {
            let mut things: Vec<Entry> = names.iter().map(|name| Entry::folder(name)).collect();
            things.extend(files.iter().map(|name| Entry::file(name, 1)));
            listing::sorted(things)
        };
        match said.as_str() {
            "/home" => of(&["Documents", "Pictures"], &["notes.txt"]),
            "/home/Documents" => of(&["Holiday"], &["taxes.pdf"]),
            "/home/Documents/Holiday" => of(&[], &["notes.txt", "beach.jpg"]),
            "/home/Pictures" => of(&[], &["beach.jpg"]),
            _ => Vec::new(),
        }
    }

    fn under_home(word: &str) -> Vec<Found> {
        under(Path::new("/home"), word, &tree)
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

    /// Nearest first, because the thing being looked for is usually the one
    /// closest to where she was standing when she typed.
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

    /// Walked into a folder at a time, so backing out of one found deep goes
    /// up through the folders above it rather than back to the place.
    #[test]
    fn a_found_folder_is_arrived_at_a_step_at_a_time() {
        assert_eq!(under_home("holi")[0].steps(), ["Documents", "Holiday"]);
        assert_eq!(under_home("docum")[0].steps(), ["Documents"]);
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

    /// The row says where it is, and something in the folder itself says what
    /// the listing would have said about it.
    #[test]
    fn a_row_says_where_what_it_found_is() {
        let found = under_home("notes");
        assert_eq!(found[0].aside(), listing::aside(&found[0].thing));
        assert_eq!(found[1].aside(), "Documents/Holiday");
        assert_eq!(found[1].at(Path::new("/home")), Path::new("/home/Documents/Holiday/notes.txt"));
    }

    /// A folder that is a link to the one above it is a walk with no end, and
    /// the panel is open while it happens.
    #[test]
    fn a_tree_that_goes_on_for_ever_is_still_left() {
        let round = |at: &Path| match at.to_string_lossy().len() < 4000 {
            true => vec![Entry::folder("down"), Entry::file("notes.txt", 1)],
            false => Vec::new(),
        };
        let found = under(Path::new("/home"), "notes", &round);
        assert!(!found.is_empty());
        assert!(found.len() <= ENOUGH);
    }
}
