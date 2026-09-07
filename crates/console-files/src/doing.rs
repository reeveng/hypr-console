//! What can be done with one thing, and what is being carried.

use std::path::PathBuf;

use console_core_never::Never;

use crate::listing::{Entry, Still};
use crate::unzipping::Packed;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Deed {
    Copy,
    Delete,
    Move,
    Open,
    OpenWith,
    Rename,
    Unzip,
    Wallpaper,
}

impl Deed {
    pub fn says(self) -> Result<&'static str, Never> {
        Ok(match self {
            Deed::Copy => "Copy",
            Deed::Delete => "Delete",
            Deed::Move => "Move",
            Deed::Open => "Open",
            Deed::OpenWith => "Open with",
            Deed::Rename => "Rename",
            Deed::Unzip => "Unzip",
            Deed::Wallpaper => "Use as wallpaper",
        })
    }

    pub fn wants(self) -> Result<Wants, Never> {
        Ok(match self {
            Deed::Wallpaper => Wants::APicture,
            Deed::Unzip => Wants::AnArchive,
            Deed::Open | Deed::OpenWith => Wants::AFile,
            Deed::Copy | Deed::Delete | Deed::Move | Deed::Rename => Wants::Anything,
        })
    }

    pub fn asks(self) -> Result<Asks, Never> {
        Ok(match self == Deed::Delete {
            true => Asks::First,
            false => Asks::Nothing,
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
    Nothing,
}

pub fn ways(entry: &Entry) -> Result<Vec<Deed>, Never> {
    let still = entry.a_picture()?;
    let packed = entry.an_archive()?;
    let mut ways: Vec<Deed> = Vec::new();

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

pub const EVERY: [Deed; 8] = [
    Deed::Open,
    Deed::OpenWith,
    Deed::Unzip,
    Deed::Rename,
    Deed::Copy,
    Deed::Move,
    Deed::Wallpaper,
    Deed::Delete,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Carrying {
    ToMove,
    ToCopy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Holding {
    pub name: String,
    pub path: PathBuf,
    pub moving: Carrying,
}

impl Holding {
    pub fn of(entry: &Entry, path: PathBuf, moving: Carrying) -> Result<Self, Never> {
        Ok(Holding { name: entry.name.clone(), path, moving })
    }

    pub fn says(&self) -> Result<String, Never> {
        let word = match self.moving {
            Carrying::ToMove => "Move",
            Carrying::ToCopy => "Put",
        };

        Ok(format!("{word} {} here", self.name))
    }
}

pub fn a_name(word: &str) -> Result<Option<String>, Never> {
    let word = word.trim();
    let usable = !word.is_empty() && !word.contains('/') && word != "." && word != "..";

    Ok(usable.then(|| word.to_string()))
}

pub const SURE: &str = "Throw this away?";

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

    fn open_to(entry: &Entry) -> Vec<Deed> {
        let Ok(ways) = ways(entry);

        ways
    }

    fn word(deed: Deed) -> &'static str {
        let Ok(says) = deed.says();

        says
    }

    #[test]
    fn a_file_can_be_opened_and_a_folder_is_walked_into() {
        assert!(open_to(&file("beach.jpg")).contains(&Deed::Open));
        assert!(!open_to(&folder("Holiday")).contains(&Deed::Open));
        assert!(!open_to(&folder("Holiday")).contains(&Deed::OpenWith));
    }

    #[test]
    fn a_folder_can_be_renamed_carried_and_thrown_away_like_anything_else() {
        for deed in [Deed::Copy, Deed::Delete, Deed::Move, Deed::Rename] {
            assert!(open_to(&folder("Holiday")).contains(&deed), "{}", word(deed));
        }
    }

    #[test]
    fn open_is_the_first_way_and_delete_is_the_last() {
        let ways = open_to(&file("beach.jpg"));

        assert_eq!(ways.first(), Some(&Deed::Open));
        assert_eq!(ways.last(), Some(&Deed::Delete));
    }

    #[test]
    fn only_a_picture_can_be_made_the_wallpaper() {
        let photograph = of_kind(file("beach.jpg"), "image/jpeg");
        let film = of_kind(file("beach.mp4"), "video/mp4");

        assert!(open_to(&photograph).contains(&Deed::Wallpaper));
        assert!(!open_to(&film).contains(&Deed::Wallpaper));
        assert!(!open_to(&file("notes.txt")).contains(&Deed::Wallpaper));
        assert!(!open_to(&folder("Holiday")).contains(&Deed::Wallpaper));
    }

    #[test]
    fn only_an_archive_is_offered_the_way_out_of_one() {
        let mod_ = of_kind(file("WickedWhims.zip"), "application/zip");
        let photograph = of_kind(file("beach.jpg"), "image/jpeg");

        assert!(open_to(&mod_).contains(&Deed::Unzip));
        assert!(!open_to(&photograph).contains(&Deed::Unzip));
        assert!(!open_to(&file("notes.txt")).contains(&Deed::Unzip));
        assert!(!open_to(&folder("Mods")).contains(&Deed::Unzip));
    }

    #[test]
    fn unzip_is_offered_before_the_ways_of_carrying_a_thing_about() {
        let mod_ = of_kind(file("WickedWhims.zip"), "application/zip");
        let ways = open_to(&mod_);

        let Some(unzip) = ways.iter().position(|deed| *deed == Deed::Unzip) else {
            panic!("an archive offers no way out of itself")
        };
        let Some(rename) = ways.iter().position(|deed| *deed == Deed::Rename) else {
            panic!("an archive cannot be renamed")
        };

        assert!(unzip < rename);
    }

    #[test]
    fn only_throwing_something_away_is_asked_about() {
        assert_eq!(Deed::Delete.asks(), Ok(Asks::First));

        for deed in
            [Deed::Copy, Deed::Move, Deed::Open, Deed::OpenWith, Deed::Rename, Deed::Unzip]
        {
            assert_eq!(deed.asks(), Ok(Asks::Nothing), "{} asks and should not", word(deed));
        }
    }

    #[test]
    fn what_is_held_says_which_of_the_two_things_it_is_waiting_to_do() {
        assert_eq!(held("beach.jpg", Carrying::ToCopy).says(), Ok("Put beach.jpg here".to_string()));
        assert_eq!(held("beach.jpg", Carrying::ToMove).says(), Ok("Move beach.jpg here".to_string()));
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
}
