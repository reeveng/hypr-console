//! The notes on the disk, read and written.
//!
//! A note is written once, whole: its title as the heading Markdown reads as
//! one, a blank line, and its body as the paragraph under it. The body read
//! back is everything after that heading, so a file written somewhere else
//! with no heading at all is all body rather than missing its first line.
//! A title that already exists as a file is not written over, because
//! the folder may be synced and the file under that name is somebody's note.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use console_core_atomic_writes::{Unread, Unwritten};
use console_core_never::Never;

pub const FOLDER: &str = "Notes";

const ENDING: &str = "md";

const HEADING: &str = "# ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub path: PathBuf,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Draft<'a> {
    pub title: &'a str,
    pub body: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exists {
    Yes,
    No,
}

#[derive(Debug)]
pub enum Unkept {
    Unlisted(PathBuf, std::io::Error),
    Unread(Unread),
    Unwritten(Unwritten),
}

impl fmt::Display for Unkept {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unkept::Unlisted(at, fault) => write!(to, "{}: listing the notes: {fault}", at.display()),
            Unkept::Unread(fault) => write!(to, "{fault}"),
            Unkept::Unwritten(fault) => write!(to, "{fault}"),
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

    Ok(Some((changed, Note { path, title })))
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

pub fn written(path: &Path, draft: Draft<'_>) -> Result<(), Unkept> {
    let Draft { title, body } = draft;
    let text = match body.trim() {
        "" => format!("{HEADING}{title}\n"),
        body => format!("{HEADING}{title}\n\n{body}\n"),
    };

    console_core_atomic_writes::whole_with_folders(path, text.as_bytes())?;

    Ok(())
}

pub fn read(path: &Path) -> Result<Vec<String>, Unkept> {
    let text = console_core_atomic_writes::text_or_empty(path)?;
    let Ok(body) = body(&text);

    Ok(body)
}

pub fn body(text: &str) -> Result<Vec<String>, Never> {
    let (first, rest) = match text.split_once('\n') {
        Some((first, rest)) => (first, rest),
        None => (text, ""),
    };

    let under = match first.starts_with(HEADING) {
        true => rest,
        false => text,
    };

    Ok(under.lines().map(str::trim_end).filter(|line| !line.is_empty()).map(str::to_string).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    type Failure = Box<dyn std::error::Error>;

    #[test]
    fn a_note_is_its_title_as_a_heading_and_its_body_under_it() -> Result<(), Failure> {
        let folder = console_core_temporary_directories::fresh("notes-written")?;
        let Ok(at) = path(&folder, "Groceries");

        written(&at, Draft { title: "Groceries", body: "  milk and eggs " })?;

        let kept = std::fs::read_to_string(&at)?;
        let shown = read(&at)?;

        assert_eq!(kept, "# Groceries\n\nmilk and eggs\n");
        assert_eq!(shown, vec!["milk and eggs".to_string()]);

        Ok(())
    }

    #[test]
    fn a_note_with_nothing_under_its_title_is_the_title_alone() -> Result<(), Failure> {
        let folder = console_core_temporary_directories::fresh("notes-bare")?;
        let Ok(at) = path(&folder, "Later");

        written(&at, Draft { title: "Later", body: "   " })?;

        let kept = std::fs::read_to_string(&at)?;
        let shown = read(&at)?;

        assert_eq!(kept, "# Later\n");
        assert_eq!(shown, Vec::<String>::new());

        Ok(())
    }

    #[test]
    fn a_file_written_somewhere_else_without_a_heading_is_all_body() {
        assert_eq!(body("first\n\nsecond\n"), Ok(vec!["first".to_string(), "second".to_string()]));
        assert_eq!(body("# Only a title"), Ok(Vec::new()));
        assert_eq!(body("#hashtag\nmore"), Ok(vec!["#hashtag".to_string(), "more".to_string()]), "a tag is not a heading");
    }

    #[test]
    fn the_notes_are_the_markdown_files_newest_first() -> Result<(), Failure> {
        let folder = console_core_temporary_directories::fresh("notes-listed")?;
        let Ok(old) = path(&folder, "Old");
        let Ok(new) = path(&folder, "New");

        written(&old, Draft { title: "Old", body: "" })?;
        written(&new, Draft { title: "New", body: "" })?;
        console_core_atomic_writes::whole(&folder.join("picture.png"), b"")?;

        let older = std::fs::File::open(&old)?;

        older.set_modified(SystemTime::UNIX_EPOCH)?;

        let notes = listed(&folder)?;
        let titles: Vec<String> = notes.into_iter().map(|note| note.title).collect();

        assert_eq!(titles, vec!["New".to_string(), "Old".to_string()]);

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
    fn a_title_already_kept_exists() -> Result<(), Failure> {
        let folder = console_core_temporary_directories::fresh("notes-exists")?;
        let Ok(at) = path(&folder, "Groceries");

        assert_eq!(exists(&at), Ok(Exists::No));

        written(&at, Draft { title: "Groceries", body: "milk" })?;

        assert_eq!(exists(&at), Ok(Exists::Yes));

        Ok(())
    }
}
