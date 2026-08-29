//! What can be done with one thing, and what is being carried.

use std::path::PathBuf;

use crate::listing::Entry;

/// One thing that can be done to a file or a folder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Deed {
    Copy,
    Delete,
    Move,
    Open,
    OpenWith,
    Rename,
    Wallpaper,
}

impl Deed {
    /// What the row says.
    pub fn says(self) -> &'static str {
        match self {
            Deed::Copy => "Copy",
            Deed::Delete => "Delete",
            Deed::Move => "Move",
            Deed::Open => "Open",
            Deed::OpenWith => "Open with",
            Deed::Rename => "Rename",
            Deed::Wallpaper => "Use as wallpaper",
        }
    }

    /// Whether it is about a file rather than about anything in a folder.
    ///
    /// A folder is walked into rather than opened, and nothing on this machine
    /// opens one with a program.
    pub fn about_a_file(self) -> bool {
        matches!(self, Deed::Open | Deed::OpenWith | Deed::Wallpaper)
    }

    /// Whether it is about a picture rather than about any file.
    ///
    /// The settings can only offer what is already in Pictures/Wallpapers, and
    /// putting a photograph there means knowing there is such a folder. Offered
    /// on the photograph itself it is one press, from the folder her camera
    /// wrote it into.
    pub fn about_a_picture(self) -> bool {
        self == Deed::Wallpaper
    }

    /// Whether it has to be asked about before it is done.
    ///
    /// One of these throws something away. It goes to the wastebasket rather
    /// than off the disk, which is a thing a person can be told and not a thing
    /// this device shows her anywhere, so as far as anybody holding it is
    /// concerned it is gone. A menu where the wrong row under a thumb loses a
    /// photograph is a menu that has to ask.
    pub fn asks(self) -> bool {
        self == Deed::Delete
    }
}

/// What is offered for one thing, in the order a thumb meets it.
///
/// Not the alphabet, which is the rule for lists of names and the wrong one
/// here. Open is first because it is what most presses of Y are on the way to,
/// and Delete is last because it is the one that cannot be taken back and the
/// last row is the hardest one to arrive at by accident.
///
/// A folder is walked into rather than opened, and nothing on this machine
/// opens one with a program, so the two that are about a file are left off it.
pub fn ways(entry: &Entry) -> Vec<Deed> {
    EVERY
        .into_iter()
        .filter(|deed| !entry.folder || !deed.about_a_file())
        .filter(|deed| !deed.about_a_picture() || entry.a_picture())
        .collect()
}

/// Every deed there is, in the order a file offers them.
///
/// One list, so that the guide can say what Y is for without being told
/// separately: a deed added here is a deed the guide names.
pub const EVERY: [Deed; 7] = [
    Deed::Open,
    Deed::OpenWith,
    Deed::Rename,
    Deed::Copy,
    Deed::Move,
    Deed::Wallpaper,
    Deed::Delete,
];

/// Something picked up in one folder, waiting to be put down in another.
///
/// One for the whole panel rather than one per tab, because carrying a
/// photograph from Pictures to a stick is the reason this exists and the two
/// are different tabs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Holding {
    pub name: String,
    pub path: PathBuf,
    pub moving: bool,
}

impl Holding {
    pub fn of(entry: &Entry, path: PathBuf, moving: bool) -> Self {
        Holding { name: entry.name.clone(), path, moving }
    }

    /// The row that puts it down, which every folder carries while anything is
    /// held.
    ///
    /// At the top of the listing rather than the end of it. Putting a thing
    /// down is the finishing of something already begun, and a folder of two
    /// hundred photographs would otherwise have to be walked to the bottom of
    /// to finish it.
    pub fn says(&self) -> String {
        let word = match self.moving {
            true => "Move",
            false => "Put",
        };
        format!("{word} {} here", self.name)
    }
}

/// A typed name, or nothing if it is not one.
///
/// A slash would make the name a path, so a rename could put a thing somewhere
/// else entirely, and the two that walk the tree would make it disappear.
/// Trimmed because an on-screen keyboard has a space bar next to everything and
/// a name with one on the end is a name that looks right and matches nothing.
pub fn a_name(word: &str) -> Option<String> {
    let word = word.trim();
    let usable = !word.is_empty() && !word.contains('/') && word != "." && word != "..";
    usable.then(|| word.to_string())
}

/// What is asked before a thing is thrown away. The thing itself is named
/// under it.
pub const SURE: &str = "Throw this away?";

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn held(name: &str, moving: bool) -> Holding {
        Holding::of(&Entry::file(name, 1), Path::new("/home/ada").join(name), moving)
    }

    #[test]
    fn a_file_can_be_opened_and_a_folder_is_walked_into() {
        assert!(ways(&Entry::file("beach.jpg", 1)).contains(&Deed::Open));
        assert!(!ways(&Entry::folder("Holiday")).contains(&Deed::Open));
        assert!(!ways(&Entry::folder("Holiday")).contains(&Deed::OpenWith));
    }

    /// Everything that can be done to a file can be done to a folder, apart
    /// from the two that are about opening one.
    #[test]
    fn a_folder_can_be_renamed_carried_and_thrown_away_like_anything_else() {
        for deed in [Deed::Copy, Deed::Delete, Deed::Move, Deed::Rename] {
            assert!(ways(&Entry::folder("Holiday")).contains(&deed), "{}", deed.says());
        }
    }

    /// The one that cannot be taken back is the hardest row to arrive at by
    /// accident, and the one most presses of Y are on the way to is the first.
    #[test]
    fn open_is_the_first_way_and_delete_is_the_last() {
        let ways = ways(&Entry::file("beach.jpg", 1));
        assert_eq!(ways.first(), Some(&Deed::Open));
        assert_eq!(ways.last(), Some(&Deed::Delete));
    }

    /// A wallpaper is one still image. Offered on a film or on a text file it
    /// would be a row that cannot do what it says.
    #[test]
    fn only_a_picture_can_be_made_the_wallpaper() {
        let photograph = Entry::file("beach.jpg", 1).of_kind("image/jpeg");
        let film = Entry::file("beach.mp4", 1).of_kind("video/mp4");
        assert!(ways(&photograph).contains(&Deed::Wallpaper));
        assert!(!ways(&film).contains(&Deed::Wallpaper));
        assert!(!ways(&Entry::file("notes.txt", 1)).contains(&Deed::Wallpaper));
        assert!(!ways(&Entry::folder("Holiday")).contains(&Deed::Wallpaper));
    }

    #[test]
    fn only_throwing_something_away_is_asked_about() {
        assert!(Deed::Delete.asks());
        for deed in [Deed::Copy, Deed::Move, Deed::Open, Deed::OpenWith, Deed::Rename] {
            assert!(!deed.asks(), "{} asks and should not", deed.says());
        }
    }

    #[test]
    fn what_is_held_says_which_of_the_two_things_it_is_waiting_to_do() {
        assert_eq!(held("beach.jpg", false).says(), "Put beach.jpg here");
        assert_eq!(held("beach.jpg", true).says(), "Move beach.jpg here");
    }

    #[test]
    fn a_name_that_would_be_a_path_is_not_a_name() {
        assert_eq!(a_name("holiday.jpg").as_deref(), Some("holiday.jpg"));
        assert_eq!(a_name("  holiday.jpg  ").as_deref(), Some("holiday.jpg"));
        assert_eq!(a_name(""), None);
        assert_eq!(a_name("   "), None);
        assert_eq!(a_name("../holiday.jpg"), None);
        assert_eq!(a_name("holiday/2026"), None);
        assert_eq!(a_name(".."), None);
        assert_eq!(a_name("."), None);
    }

    /// A dotfile is not shown, so a name beginning with one is a thing that
    /// vanishes the moment it is made. It is still a name somebody may mean.
    #[test]
    fn a_name_may_begin_with_a_dot() {
        assert_eq!(a_name(".hidden").as_deref(), Some(".hidden"));
    }

    /// The question is about whatever it was asked over, which the surface
    /// names underneath it, so nothing here builds a sentence out of a name.
    #[test]
    fn the_question_is_a_sentence_and_carries_no_name() {
        assert!(SURE.ends_with('?'));
        assert!(!SURE.contains('{'));
    }
}
