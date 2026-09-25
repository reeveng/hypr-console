//! What can be done with one thing, and what is being carried.

use std::path::PathBuf;

use console_core_never::Never;
use console_core_words::Words;

use crate::listing::{Entry, Still};
use crate::unzipping::Packed;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Words)]
pub enum FileAction {
    #[words(says = "Copy")]
    Copy,
    #[words(says = "Delete")]
    Delete,
    #[words(says = "Move")]
    Move,
    #[words(says = "Open")]
    Open,
    #[words(says = "Open with")]
    OpenWith,
    #[words(says = "Rename")]
    Rename,
    #[words(says = "Select")]
    Select,
    #[words(says = "Unzip")]
    Unzip,
    #[words(says = "Use as wallpaper")]
    Wallpaper,
}

impl FileAction {
    pub fn wants(self) -> Result<Wants, Never> {
        Ok(match self {
            FileAction::Wallpaper => Wants::APicture,
            FileAction::Unzip => Wants::AnArchive,
            FileAction::Open | FileAction::OpenWith => Wants::AFile,
            FileAction::Copy | FileAction::Delete | FileAction::Move | FileAction::Rename | FileAction::Select => {
                Wants::Anything
            },
        })
    }

    pub fn asks(self) -> Result<Asks, Never> {
        Ok(match self == FileAction::Delete {
            true => Asks::First,
            false => Asks::None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wants {
    APicture,
    AnArchive,
    AFile,
    Anything,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asks {
    First,
    None,
}

pub fn ways(entry: &Entry) -> Result<Vec<FileAction>, Never> {
    let still = entry.a_picture()?;
    let packed = entry.an_archive()?;
    let mut ways: Vec<FileAction> = Vec::new();

    for deed in EVERY {
        let wants = deed.wants()?;
        let allowed = (!entry.folder || wants == Wants::Anything)
            && (wants != Wants::APicture || still == Still::APicture)
            && (wants != Wants::AnArchive || packed == Packed::AnArchive);

        match allowed {
            true => ways.push(deed),
            false => {},
        }
    }

    Ok(ways)
}

pub const EVERY: [FileAction; 9] = [
    FileAction::Open,
    FileAction::OpenWith,
    FileAction::Unzip,
    FileAction::Select,
    FileAction::Rename,
    FileAction::Copy,
    FileAction::Move,
    FileAction::Wallpaper,
    FileAction::Delete,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Carrying {
    ToMove,
    ToCopy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Holding {
    pub name: String,
    pub paths: Vec<PathBuf>,
    pub moving: Carrying,
}

impl Holding {
    pub fn of(entry: &Entry, path: PathBuf, moving: Carrying) -> Result<Self, Never> {
        Ok(Holding { name: entry.name.clone(), paths: vec![path], moving })
    }

    pub fn many(paths: Vec<PathBuf>, moving: Carrying) -> Result<Self, Never> {
        let Ok(name) = items(&paths);

        Ok(Holding { name, paths, moving })
    }

    pub fn says(&self) -> Result<String, Never> {
        let word = match self.moving {
            Carrying::ToMove => "Move",
            Carrying::ToCopy => "Paste",
        };

        Ok(format!("{word} {} Here", self.name))
    }
}

pub fn items(paths: &[PathBuf]) -> Result<String, Never> {
    Ok(match paths {
        [one] => match one.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => one.to_string_lossy().to_string(),
        },
        [] | [_, _, ..] => format!("{} Items", paths.len()),
    })
}

pub fn a_name(word: &str) -> Result<Option<String>, Never> {
    let word = word.trim();
    let usable = !word.is_empty() && !word.contains('/') && word != "." && word != "..";

    Ok(usable.then(|| word.to_string()))
}

pub const SURE: &str = "Delete this?";

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn file(name: &str) -> Entry {
        let Ok(entry) = Entry::file(name, 1);

        entry
    }

    fn folder(name: &str) -> Entry {
        let Ok(entry) = Entry::folder(name);

        entry
    }

    fn of_kind(entry: Entry, kind: &str) -> Entry {
        let Ok(entry) = entry.of_kind(kind);

        entry
    }

    fn held(name: &str, moving: Carrying) -> Holding {
        let Ok(holding) = Holding::of(&file(name), Path::new("/home/ada").join(name), moving);

        holding
    }

    fn open_to(entry: &Entry) -> Vec<FileAction> {
        let Ok(ways) = ways(entry);

        ways
    }

    fn word(deed: FileAction) -> &'static str {
        let Ok(says) = deed.says();

        says
    }

    #[test]
    fn a_file_can_be_opened_and_a_folder_is_walked_into() {
        assert!(open_to(&file("beach.jpg")).contains(&FileAction::Open));
        assert!(!open_to(&folder("Holiday")).contains(&FileAction::Open));
        assert!(!open_to(&folder("Holiday")).contains(&FileAction::OpenWith));
    }

    #[test]
    fn a_folder_can_be_renamed_carried_and_thrown_away_like_anything_else() {
        for deed in [FileAction::Copy, FileAction::Delete, FileAction::Move, FileAction::Rename, FileAction::Select] {
            assert!(open_to(&folder("Holiday")).contains(&deed), "{}", word(deed));
        }
    }

    #[test]
    fn open_is_the_first_way_and_delete_is_the_last() {
        let ways = open_to(&file("beach.jpg"));

        assert_eq!(ways.first(), Some(&FileAction::Open));
        assert_eq!(ways.last(), Some(&FileAction::Delete));
    }

    #[test]
    fn only_a_picture_can_be_made_the_wallpaper() {
        let photograph = of_kind(file("beach.jpg"), "image/jpeg");
        let film = of_kind(file("beach.mp4"), "video/mp4");

        assert!(open_to(&photograph).contains(&FileAction::Wallpaper));
        assert!(!open_to(&film).contains(&FileAction::Wallpaper));
        assert!(!open_to(&file("notes.txt")).contains(&FileAction::Wallpaper));
        assert!(!open_to(&folder("Holiday")).contains(&FileAction::Wallpaper));
    }

    #[test]
    fn only_an_archive_is_offered_the_way_out_of_one() {
        let mod_ = of_kind(file("WickedWhims.zip"), "application/zip");
        let photograph = of_kind(file("beach.jpg"), "image/jpeg");

        assert!(open_to(&mod_).contains(&FileAction::Unzip));
        assert!(!open_to(&photograph).contains(&FileAction::Unzip));
        assert!(!open_to(&file("notes.txt")).contains(&FileAction::Unzip));
        assert!(!open_to(&folder("Mods")).contains(&FileAction::Unzip));
    }

    #[test]
    fn unzip_is_offered_before_the_ways_of_carrying_a_thing_about() {
        let mod_ = of_kind(file("WickedWhims.zip"), "application/zip");
        let ways = open_to(&mod_);

        assert!(ways.contains(&FileAction::Unzip), "an archive offers no way out of itself");
        assert!(ways.contains(&FileAction::Rename), "an archive cannot be renamed");
        assert!(
            ways.iter().take_while(|deed| **deed != FileAction::Rename).any(|deed| *deed == FileAction::Unzip),
            "unzipping is offered after the ways of carrying it about"
        );
    }

    #[test]
    fn only_throwing_something_away_is_asked_about() {
        assert_eq!(FileAction::Delete.asks(), Ok(Asks::First));

        for deed in
            [FileAction::Copy, FileAction::Move, FileAction::Open, FileAction::OpenWith, FileAction::Rename, FileAction::Select, FileAction::Unzip]
        {
            assert_eq!(deed.asks(), Ok(Asks::None), "{} asks and should not", word(deed));
        }
    }

    #[test]
    fn what_is_held_says_which_of_the_two_things_it_is_waiting_to_do() {
        assert_eq!(held("beach.jpg", Carrying::ToCopy).says(), Ok("Paste beach.jpg Here".to_string()));
        assert_eq!(held("beach.jpg", Carrying::ToMove).says(), Ok("Move beach.jpg Here".to_string()));
    }

    #[test]
    fn many_things_carried_are_said_as_a_count_and_one_as_its_name() {
        let paths = vec![PathBuf::from("/p/beach.jpg"), PathBuf::from("/p/dune.jpg")];
        let Ok(many) = Holding::many(paths, Carrying::ToCopy);
        let Ok(one) = Holding::many(vec![PathBuf::from("/p/beach.jpg")], Carrying::ToMove);

        assert_eq!(many.says(), Ok("Paste 2 Items Here".to_string()));
        assert_eq!(one.says(), Ok("Move beach.jpg Here".to_string()));
    }

    #[test]
    fn a_name_that_would_be_a_path_is_not_a_name() {
        assert_eq!(a_name("holiday.jpg"), Ok(Some("holiday.jpg".to_string())));
        assert_eq!(a_name("  holiday.jpg  "), Ok(Some("holiday.jpg".to_string())));
        assert_eq!(a_name(""), Ok(None));
        assert_eq!(a_name("   "), Ok(None));
        assert_eq!(a_name("../holiday.jpg"), Ok(None));
        assert_eq!(a_name("holiday/2026"), Ok(None));
        assert_eq!(a_name(".."), Ok(None));
        assert_eq!(a_name("."), Ok(None));
    }

    #[test]
    fn a_name_may_begin_with_a_dot() {
        assert_eq!(a_name(".hidden"), Ok(Some(".hidden".to_string())));
    }

    #[test]
    fn the_question_is_a_sentence_and_carries_no_name() {
        assert!(SURE.ends_with('?'));
        assert!(!SURE.contains('{'));
    }

    #[test]
    fn the_question_asks_in_the_word_the_row_that_raised_it_said() {
        let Ok(says) = FileAction::Delete.says();

        assert!(
            SURE.to_lowercase().contains(&says.to_lowercase()),
            "{SURE:?} asks about something the row called {says:?}"
        );
    }
}
