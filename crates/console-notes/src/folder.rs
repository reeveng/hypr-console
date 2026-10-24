//! The notes on the disk, read and written.
//!
//! A note is a Markdown file whose name is its title: the title as the
//! heading Markdown reads as one, a blank line, and the rest of the note under
//! it. In the app it is one text whose first line is the title, the
//! way Apple's Notes has it, so there is nothing to name before writing and
//! nothing to rename through a second question. [`text`] and [`file`] are the
//! two directions between the file and that text, and a file written somewhere
//! else with no heading is read as its name over everything it holds.
//!
//! A note is kept whole at every change, because the moment somebody stops
//! typing is the moment nobody is going to press anything to say so. What the
//! file held when it was last read or written is carried beside it, and a file
//! that no longer holds that was changed by something else -- the folder may
//! be synced. The change is then kept under a name of its own rather than
//! written over the other one, so neither of the two is lost.
//!
//! The name follows the title when the note is left rather than at every
//! letter, so a title being typed is not a file renamed once per key; a note
//! left with nothing in it was never made, and goes.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use console_core_atomic_writes::{Unread, Unwritten};
use console_core_never::Never;

pub const FOLDER: &str = "Notes";

pub const UNTITLED: &str = "New Note";

const ENDING: &str = "md";

const HEADING: &str = "# ";

const ANOTHER: std::ops::Range<u32> = 2..1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub path: PathBuf,
    pub title: String,
    pub opening: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnDisk<'a> {
    pub at: Option<&'a Path>,
    pub known: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exists {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub at: PathBuf,
    pub known: String,
    pub destination: Destination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Original,
    Copy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Leave {
    Retain,
    Rename(PathBuf),
    Remove,
}

#[derive(Debug)]
pub enum Unkept {
    Unlisted(PathBuf, std::io::Error),
    Unread(Unread),
    Unwritten(Unwritten),
    Unnamed(PathBuf, std::io::Error),
}

impl fmt::Display for Unkept {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unkept::Unlisted(at, fault) => write!(to, "{}: listing the notes: {fault}", at.display()),
            Unkept::Unread(fault) => write!(to, "{fault}"),
            Unkept::Unwritten(fault) => write!(to, "{fault}"),
            Unkept::Unnamed(at, fault) => write!(to, "{}: naming the note after its title: {fault}", at.display()),
        }
    }
}

impl std::error::Error for Unkept {}

impl From<Unread> for Unkept {
    fn from(fault: Unread) -> Self {
        Unkept::Unread(fault)
    }
}

impl From<Unwritten> for Unkept {
    fn from(fault: Unwritten) -> Self {
        Unkept::Unwritten(fault)
    }
}

pub fn listed(folder: &Path) -> Result<Vec<Note>, Unkept> {
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(fault) => {
            return match fault.kind() == std::io::ErrorKind::NotFound {
                true => Ok(Vec::new()),
                false => Err(Unkept::Unlisted(folder.to_path_buf(), fault)),
            };
        }
    };

    let mut found: Vec<(SystemTime, Note)> = entries
        .filter_map(|entry| match entry {
            Ok(entry) => Some(entry.path()),
            Err(_gone_while_it_was_listed) => None,
        })
        .filter_map(|path| {
            let Ok(note) = note(path);

            note
        })
        .collect();

    found.sort_by(|(one, _), (other, _)| other.cmp(one));

    Ok(found.into_iter().map(|(_, note)| note).collect())
}

fn note(path: PathBuf) -> Result<Option<(SystemTime, Note)>, Never> {
    let title = match (path.extension().and_then(|ending| ending.to_str()), path.file_stem()) {
        (Some(ENDING), Some(stem)) => stem.to_string_lossy().to_string(),
        (Some(_) | None, Some(_) | None) => return Ok(None),
    };

    let changed = match std::fs::metadata(&path).and_then(|held| held.modified()) {
        Ok(changed) => changed,
        Err(_said_when_it_is_opened) => SystemTime::UNIX_EPOCH,
    };

    let opening = match console_core_atomic_writes::text_or_empty(&path) {
        Ok(file) => {
            let Ok(written) = text(&file, &path);
            let Ok(opening) = opening(&written);

            opening
        }
        Err(_said_when_it_is_opened) => String::new(),
    };

    Ok(Some((changed, Note { path, title, opening })))
}

pub fn path(folder: &Path, title: &str) -> Result<PathBuf, Never> {
    Ok(folder.join(format!("{title}.{ENDING}")))
}

pub fn exists(path: &Path) -> Result<Exists, Never> {
    Ok(match path.try_exists() {
        Ok(false) => Exists::No,
        Ok(true) => Exists::Yes,
        Err(_cannot_tell_so_it_is_not_written_over) => Exists::Yes,
    })
}

pub fn text(file: &str, at: &Path) -> Result<String, Never> {
    let name = match at.file_stem() {
        Some(stem) => stem.to_string_lossy().to_string(),
        None => String::new(),
    };

    let (first, rest) = match file.split_once('\n') {
        Some((first, rest)) => (first, rest),
        None => (file, ""),
    };

    let (title, under) = match first.strip_prefix(HEADING) {
        Some(title) => (title, match rest.strip_prefix('\n') {
            Some(under) => under,
            None => rest,
        }),
        None => (name.as_str(), file),
    };

    let under = under.trim_end();

    Ok(match under.is_empty() {
        true => title.trim().to_string(),
        false => format!("{}\n{under}", title.trim()),
    })
}

pub fn file(text: &str) -> Result<String, Never> {
    let (title, under) = match text.split_once('\n') {
        Some((title, under)) => (title, under),
        None => (text, ""),
    };

    let under = under.trim_end();

    Ok(match (title.trim(), under.is_empty()) {
        ("", true) => String::new(),
        (title, true) => format!("{HEADING}{title}\n"),
        (title, false) => format!("{HEADING}{title}\n\n{under}\n"),
    })
}

pub fn title(text: &str) -> Result<String, Never> {
    let first = match text.split_once('\n') {
        Some((first, _under)) => first,
        None => text,
    };

    let Ok(named) = console_core_file_names::a_name(first);

    Ok(match named {
        Some(named) => named,
        None => UNTITLED.to_string(),
    })
}

fn opening(text: &str) -> Result<String, Never> {
    Ok(match text.lines().skip(1).map(str::trim).find(|line| !line.is_empty()) {
        Some(line) => line.to_string(),
        None => String::new(),
    })
}

pub fn free(folder: &Path, title: &str) -> Result<PathBuf, Never> {
    let Ok(first) = path(folder, title);

    let another = ANOTHER.map(|count| {
        let Ok(another) = path(folder, &format!("{title} {count}"));

        another
    });

    Ok(match std::iter::once(first.clone()).chain(another).find(|at| exists(at) == Ok(Exists::No)) {
        Some(free) => free,
        None => first,
    })
}

pub fn save(folder: &Path, written: OnDisk<'_>, text: &str) -> Result<Record, Unkept> {
    let OnDisk { at, known } = written;
    let Ok(whole) = file(text);
    let Ok(title) = title(text);

    let (at, destination) = match at {
        Some(at) => {
            let now = console_core_atomic_writes::text_or_empty(at)?;

            match now == known {
                true => (at.to_path_buf(), Destination::Original),
                false => {
                    let Ok(aside) = free(folder, &title);

                    (aside, Destination::Copy)
                }
            }
        }
        None => {
            let Ok(fresh) = free(folder, &title);

            (fresh, Destination::Original)
        }
    };

    console_core_atomic_writes::whole_with_folders(&at, whole.as_bytes())?;

    Ok(Record { at, known: whole, destination })
}

pub fn leave(folder: &Path, written: OnDisk<'_>, text: &str) -> Result<Leave, Unkept> {
    let (at, known) = match written {
        OnDisk { at: Some(at), known } => (at, known),
        OnDisk { at: None, .. } => return Ok(Leave::Retain),
    };

    let now = console_core_atomic_writes::text_or_empty(at)?;

    match now == known {
        true => {},
        false => return Ok(Leave::Retain),
    }

    match text.trim().is_empty() {
        true => {
            return match std::fs::remove_file(at) {
                Ok(()) => Ok(Leave::Remove),
                Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
                    true => Ok(Leave::Remove),
                    false => Err(Unkept::Unnamed(at.to_path_buf(), fault)),
                },
            };
        }
        false => {},
    }

    let Ok(title) = title(text);
    let Ok(named) = path(folder, &title);
    let Ok(taken) = exists(&named);

    match (named == at, taken) {
        (true, _) | (false, Exists::Yes) => Ok(Leave::Retain),
        (false, Exists::No) => match std::fs::rename(at, &named) {
            Ok(()) => Ok(Leave::Rename(named)),
            Err(fault) => Err(Unkept::Unnamed(at.to_path_buf(), fault)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Failure = Box<dyn std::error::Error>;

    const NOTHING_YET: OnDisk<'static> = OnDisk { at: None, known: "" };

    #[test]
    fn a_note_is_its_title_over_a_blank_line_over_the_rest() {
        assert_eq!(file("Groceries\nmilk\n\neggs"), Ok("# Groceries\n\nmilk\n\neggs\n".to_string()));
        assert_eq!(file("Later"), Ok("# Later\n".to_string()));
        assert_eq!(file("  \n"), Ok(String::new()), "nothing written is nothing to keep");
    }

    #[test]
    fn a_file_read_back_is_the_text_it_was_written_from() {
        let written = "Groceries\nmilk\n\n- eggs\n- bread";
        let Ok(kept) = file(written);

        assert_eq!(text(&kept, Path::new("Groceries.md")), Ok(written.to_string()));
    }

    #[test]
    fn a_file_written_somewhere_else_without_a_heading_is_its_name_over_all_of_it() {
        assert_eq!(text("first\n\nsecond\n", Path::new("Ideas.md")), Ok("Ideas\nfirst\n\nsecond".to_string()));
        assert_eq!(text("#hashtag\nmore", Path::new("Tags.md")), Ok("Tags\n#hashtag\nmore".to_string()), "a tag is not a heading");
        assert_eq!(text("# Only a title", Path::new("Elsewhere.md")), Ok("Only a title".to_string()));
    }

    #[test]
    fn the_title_is_the_first_line_or_a_new_note() {
        assert_eq!(title("Groceries\nmilk"), Ok("Groceries".to_string()));
        assert_eq!(title("\nmilk"), Ok(UNTITLED.to_string()));
        assert_eq!(title("a/b"), Ok(UNTITLED.to_string()), "a slash cannot be in a file's name");
    }

    #[test]
    fn the_notes_are_the_markdown_files_newest_first_with_how_each_opens() -> Result<(), Failure> {
        let folder = console_core_temporary_directories::fresh("notes-listed")?;

        let old = save(&folder, NOTHING_YET, "Old\nfirst words")?;
        let _new = save(&folder, NOTHING_YET, "New")?;
        console_core_atomic_writes::whole(&folder.join("picture.png"), b"")?;

        let older = std::fs::File::open(&old.at)?;

        older.set_modified(SystemTime::UNIX_EPOCH)?;

        let notes = listed(&folder)?;
        let seen: Vec<(String, String)> = notes.into_iter().map(|note| (note.title, note.opening)).collect();

        assert_eq!(
            seen,
            vec![("New".to_string(), String::new()), ("Old".to_string(), "first words".to_string())]
        );

        Ok(())
    }

    #[test]
    fn a_folder_not_made_yet_holds_no_notes() -> Result<(), Failure> {
        let folder = console_core_temporary_directories::fresh("notes-none")?;

        let notes = listed(&folder.join(FOLDER))?;

        assert_eq!(notes, Vec::new());

        Ok(())
    }

    #[test]
    fn a_new_note_takes_a_name_nobody_has() -> Result<(), Failure> {
        let folder = console_core_temporary_directories::fresh("notes-free")?;

        let first = save(&folder, NOTHING_YET, "Groceries")?;
        let second = save(&folder, NOTHING_YET, "Groceries\nfor Sunday")?;

        assert_eq!(first.at, folder.join("Groceries.md"));
        assert_eq!(second.at, folder.join("Groceries 2.md"), "the first is somebody's note");

        Ok(())
    }

    #[test]
    fn a_note_kept_twice_is_written_over_itself() -> Result<(), Failure> {
        let folder = console_core_temporary_directories::fresh("notes-again")?;

        let first = save(&folder, NOTHING_YET, "Groceries\nmilk")?;
        let again = save(&folder, OnDisk { at: Some(&first.at), known: &first.known }, "Groceries\nmilk and eggs")?;

        assert_eq!(again, Record { at: first.at.clone(), known: "# Groceries\n\nmilk and eggs\n".to_string(), destination: Destination::Original });
        let on_the_disk = std::fs::read_to_string(&first.at)?;

        assert_eq!(on_the_disk, again.known);

        Ok(())
    }

    #[test]
    fn a_note_changed_somewhere_else_is_not_written_over() -> Result<(), Failure> {
        let folder = console_core_temporary_directories::fresh("notes-copy")?;

        let first = save(&folder, NOTHING_YET, "Groceries\nmilk")?;
        console_core_atomic_writes::whole(&first.at, b"# Groceries\n\nbread\n")?;

        let kept = save(&folder, OnDisk { at: Some(&first.at), known: &first.known }, "Groceries\nmilk and eggs")?;

        assert_eq!(kept.destination, Destination::Copy);
        assert_eq!(kept.at, folder.join("Groceries 2.md"));
        let theirs = std::fs::read_to_string(&first.at)?;
        let mine = std::fs::read_to_string(&kept.at)?;

        assert_eq!(theirs, "# Groceries\n\nbread\n", "the other change is still there");
        assert_eq!(mine, "# Groceries\n\nmilk and eggs\n", "and so is this one");

        Ok(())
    }

    #[test]
    fn a_note_left_takes_its_title_as_its_name() -> Result<(), Failure> {
        let folder = console_core_temporary_directories::fresh("notes-renamed")?;

        let first = save(&folder, NOTHING_YET, "G")?;
        let typed = save(&folder, OnDisk { at: Some(&first.at), known: &first.known }, "Groceries\nmilk")?;
        let left = leave(&folder, OnDisk { at: Some(&typed.at), known: &typed.known }, "Groceries\nmilk")?;

        assert_eq!(left, Leave::Rename(folder.join("Groceries.md")));
        assert_eq!(exists(&first.at), Ok(Exists::No));

        Ok(())
    }

    #[test]
    fn a_note_left_under_a_title_somebody_has_keeps_its_own_name() -> Result<(), Failure> {
        let folder = console_core_temporary_directories::fresh("notes-taken")?;

        let _theirs = save(&folder, NOTHING_YET, "Groceries")?;
        let mine = save(&folder, NOTHING_YET, "Shopping")?;
        let typed = save(&folder, OnDisk { at: Some(&mine.at), known: &mine.known }, "Groceries\nmilk")?;
        let left = leave(&folder, OnDisk { at: Some(&typed.at), known: &typed.known }, "Groceries\nmilk")?;

        assert_eq!(left, Leave::Retain);
        assert_eq!(exists(&mine.at), Ok(Exists::Yes));

        Ok(())
    }

    #[test]
    fn a_note_left_empty_was_never_made() -> Result<(), Failure> {
        let folder = console_core_temporary_directories::fresh("notes-empty")?;

        let first = save(&folder, NOTHING_YET, "G")?;
        let emptied = save(&folder, OnDisk { at: Some(&first.at), known: &first.known }, "")?;
        let left = leave(&folder, OnDisk { at: Some(&emptied.at), known: &emptied.known }, "")?;

        assert_eq!(left, Leave::Remove);
        assert_eq!(exists(&first.at), Ok(Exists::No));

        Ok(())
    }
}
