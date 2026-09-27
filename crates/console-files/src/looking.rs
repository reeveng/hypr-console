//! What a typed word finds under the folder being shown.
//!
//! A listing is walked into a folder at a time, which is the right way to read
//! a place someone knows and the wrong way to find one thing in a place they
//! do not. So the line at the top of a folder is not a filter on what is in
//! front of you: it looks under everything below it as well, and the row says
//! where what it found is.

use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Word<'a>(pub &'a str);

pub fn answers(name: &str, word: Word<'_>) -> Result<Answers, Never> {
    Ok(match name.to_lowercase().contains(word.0.trim().to_lowercase().as_str()) {
        true => Answers::Yes,
        false => Answers::No,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answers {
    Yes,
    No,
}

const ENOUGH: u32 = 120;
const FAR: u32 = 600;

pub fn under(
    here: &Path,
    word: &str,
    read: &dyn Fn(&Path) -> Result<Vec<Entry>, Never>,
) -> Result<Vec<Found>, Never> {
    match word.trim().is_empty() {
        true => return Ok(Vec::new()),
        false => {},
    }

    let looking = Looking { here, word: Word(word), read };

    let looked = iterate(Walked { found: Vec::new(), opened: 0, deeper: vec![PathBuf::new()] }, |walked| looking.level(walked));

    Ok(match looked {
        Ok(found) => found,
        Err(Endless) => Vec::new(),
    })
}

struct Looking<'a> {
    here: &'a Path,
    word: Word<'a>,
    read: &'a dyn Fn(&Path) -> Result<Vec<Entry>, Never>,
}

struct Walked {
    found: Vec<Found>,
    opened: u32,
    deeper: Vec<PathBuf>,
}

impl Looking<'_> {
    fn level(&self, walked: Walked) -> Result<Step<Walked, Vec<Found>>, Never> {
        let Walked { found, opened, deeper } = walked;
        let begun = Walked { found, opened, deeper: Vec::new() };

        let walked = deeper.into_iter().try_fold(begun, |walked, within| {
            let Ok(many) = console_core_number_conversion::fitted::<_, u32>(walked.found.len());

            match many >= ENOUGH || walked.opened >= FAR {
                true => ControlFlow::Break(walked),
                false => {
                    let Ok(walked) = self.opened(walked, within);

                    ControlFlow::Continue(walked)
                }
            }
        });

        Ok(match walked {
            ControlFlow::Break(walked) => Step::Halt(walked.found),
            ControlFlow::Continue(walked) => match walked.deeper.is_empty() {
                true => Step::Halt(walked.found),
                false => Step::Again(walked),
            },
        })
    }

    fn opened(&self, walked: Walked, within: PathBuf) -> Result<Walked, Never> {
        let Walked { mut found, opened, mut deeper } = walked;
        let at = match within.as_os_str().is_empty() {
            true => self.here.to_path_buf(),
            false => self.here.join(&within),
        };

        let things = (self.read)(&at)?;

        for thing in things {
            match thing.folder {
                true => deeper.push(within.join(&thing.name)),
                false => {},
            }

            let answers = answers(&thing.name, self.word)?;

            match answers {
                Answers::Yes => found.push(Found { thing, within: within.clone() }),
                Answers::No => {},
            }
        }

        Ok(Walked { found, opened: opened.saturating_add(1), deeper })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    fn tree(at: &Path) -> Result<Vec<Entry>, Never> {
        let said = at.to_string_lossy().to_string();
        let of = |names: &[&str], files: &[&str]| {
            let mut things: Vec<Entry> = Vec::new();

            for name in names {
                let Ok(folder) = Entry::folder(name);

                things.push(folder);
            }

            for name in files {
                let Ok(file) = Entry::file(name, 1);

                things.push(file);
            }

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

    fn under_home(word: &str) -> Result<Vec<Found>, Never> {
        under(Path::new("/home"), word, &tree)
    }

    fn names(found: &[Found]) -> Result<Vec<&str>, Never> {
        Ok(found.iter().map(|one| one.thing.name.as_str()).collect())
    }

    #[test]
    fn a_word_finds_what_is_under_the_folder_as_well_as_what_is_in_it() -> Result<(), Box<dyn Error>> {
        let Ok(found) = under_home("notes");
        let Ok(named) = names(&found);
        let [here, deeper] = found.first_chunk::<2>().ok_or("two notes")?;

        assert_eq!(named, ["notes.txt", "notes.txt"]);
        assert_eq!(here.within, PathBuf::new());
        assert_eq!(deeper.within, PathBuf::from("Documents/Holiday"));

        Ok(())
    }

    #[test]
    fn what_is_nearest_is_found_first() -> Result<(), Box<dyn Error>> {
        let Ok(found) = under_home("beach");
        let [nearest, further] = found.first_chunk::<2>().ok_or("two beaches")?;

        assert_eq!(nearest.within, PathBuf::from("Pictures"));
        assert_eq!(further.within, PathBuf::from("Documents/Holiday"));

        Ok(())
    }

    #[test]
    fn a_folder_answers_to_a_word_the_same_way_a_file_does() {
        let Ok(found) = under_home("holi");
        let Ok(named) = names(&found);

        assert_eq!(named, ["Holiday"]);
    }

    #[test]
    fn a_found_folder_is_arrived_at_a_step_at_a_time() -> Result<(), Box<dyn Error>> {
        let Ok(holiday) = under_home("holi");
        let Ok(documents) = under_home("docum");
        let holiday = holiday.first().ok_or("the holiday")?;
        let documents = documents.first().ok_or("the documents")?;

        assert_eq!(holiday.steps(), Ok(vec!["Documents".to_string(), "Holiday".to_string()]));
        assert_eq!(documents.steps(), Ok(vec!["Documents".to_string()]));

        Ok(())
    }

    #[test]
    fn the_case_it_was_typed_in_does_not_matter() {
        let Ok(shouted) = under_home("TAXES");
        let Ok(spaced) = under_home("  taxes ");
        let Ok(shouted) = names(&shouted);
        let Ok(spaced) = names(&spaced);

        assert_eq!(shouted, ["taxes.pdf"]);
        assert_eq!(spaced, ["taxes.pdf"]);
    }

    #[test]
    fn nothing_typed_looks_at_nothing() {
        let Ok(empty) = under_home("");
        let Ok(blank) = under_home("   ");

        assert!(empty.is_empty());
        assert!(blank.is_empty());
    }

    #[test]
    fn a_word_nothing_answers_to_finds_nothing() {
        let Ok(kangaroo) = under_home("kangaroo");

        assert!(kangaroo.is_empty());
    }

    #[test]
    fn a_row_says_where_what_it_found_is() -> Result<(), Box<dyn Error>> {
        let Ok(found) = under_home("notes");
        let [here, deeper] = found.first_chunk::<2>().ok_or("two notes")?;

        assert_eq!(here.aside(), listing::aside(&here.thing));
        assert_eq!(deeper.aside(), Ok("Documents/Holiday".to_string()));
        assert_eq!(deeper.at(Path::new("/home")), Ok(PathBuf::from("/home/Documents/Holiday/notes.txt")));

        Ok(())
    }

    #[test]
    fn a_tree_that_goes_on_for_ever_is_still_left() -> Result<(), Box<dyn Error>> {
        let round = |at: &Path| {
            let Ok(folder) = Entry::folder("down");
            let Ok(file) = Entry::file("notes.txt", 1);

            Ok(match at.to_string_lossy().len() < 4000 {
                true => vec![folder, file],
                false => Vec::new(),
            })
        };

        let Ok(found) = under(Path::new("/home"), "notes", &round);
        let many = u32::try_from(found.len())?;

        assert!(!found.is_empty());
        assert!(many <= ENOUGH);

        Ok(())
    }
}
