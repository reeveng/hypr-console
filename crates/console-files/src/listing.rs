//! A folder, in the order it is read and the words it is read in.


use console_core_never::Never;
use console_core_number_conversion::{Float, whole_u64};

use crate::unzipping::{self, Packed};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Entry {
    pub name: String,
    pub folder: bool,
    pub kind: String,
    pub size: u64,
}

impl Entry {
    pub fn folder(name: &str) -> Result<Self, Never> {
        Ok(Entry { name: name.to_string(), folder: true, kind: String::new(), size: 0 })
    }

    pub fn file(name: &str, size: u64) -> Result<Self, Never> {
        Ok(Entry { name: name.to_string(), folder: false, kind: String::new(), size })
    }

    pub fn of_kind(mut self, kind: &str) -> Result<Self, Never> {
        self.kind = kind.to_string();

        Ok(self)
    }

    pub fn worth_a_picture(&self) -> Result<Worth, Never> {
        let drawn =
            !self.folder && (self.kind.starts_with("image/") || self.kind.starts_with("video/"));

        Ok(match drawn {
            true => Worth::APicture,
            false => Worth::ItsNameAlone,
        })
    }

    pub fn an_archive(&self) -> Result<Packed, Never> {
        let Ok(packed) = unzipping::packed(&self.kind);

        Ok(match self.folder {
            true => Packed::NotOne,
            false => packed,
        })
    }

    pub fn a_picture(&self) -> Result<Still, Never> {
        Ok(match !self.folder && self.kind.starts_with("image/") {
            true => Still::APicture,
            false => Still::NotOne,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worth {
    APicture,
    ItsNameAlone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Still {
    APicture,
    NotOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Room {
    Kept,
    Spared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    Yes,
    No,
}

pub fn wants_room(things: &[Entry]) -> Result<Room, Never> {
    for thing in things {
        let worth = thing.worth_a_picture()?;

        match thing.folder || worth == Worth::APicture {
            true => return Ok(Room::Kept),
            false => {},
        }
    }

    Ok(Room::Spared)
}

pub fn wanted(name: &str) -> Result<Shown, Never> {
    Ok(match name.starts_with('.') {
        true => Shown::No,
        false => Shown::Yes,
    })
}

pub fn sorted(mut things: Vec<Entry>) -> Result<Vec<Entry>, Never> {
    things.sort_by(|one, other| {
        other
            .folder
            .cmp(&one.folder)
            .then_with(|| one.name.to_lowercase().cmp(&other.name.to_lowercase()))
    });

    Ok(things)
}

pub fn aside(entry: &Entry) -> Result<String, Never> {
    match entry.folder {
        true => Ok(String::new()),
        false => said(entry.size),
    }
}

const UNITS: [(&str, u64); 4] = [("B", 1), ("KB", 1 << 10), ("MB", 1 << 20), ("GB", 1 << 30)];

pub fn said(bytes: u64) -> Result<String, Never> {
    let (unit, worth) = UNITS
        .iter()
        .rev()
        .find(|(_, worth)| bytes >= *worth)
        .copied()
        .or_else(|| UNITS.first().copied())
        .unwrap_or(("B", 1));
    let Ok(many) = bytes.float();
    let Ok(each) = worth.float();

    let much = many / each;

    let Ok(whole) = whole_u64(much);

    Ok(match unit == "B" || much >= 10.0 {
        true => format!("{whole} {unit}"),
        false => format!("{much:.1} {unit}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(things: &[Entry]) -> Vec<&str> {
        things.iter().map(|thing| thing.name.as_str()).collect()
    }

    fn folder(name: &str) -> Entry {
        let Ok(entry) = Entry::folder(name);

        entry
    }

    fn file(name: &str, size: u64) -> Entry {
        let Ok(entry) = Entry::file(name, size);

        entry
    }

    fn of_kind(entry: Entry, kind: &str) -> Entry {
        let Ok(entry) = entry.of_kind(kind);

        entry
    }

    fn in_order(things: Vec<Entry>) -> Vec<Entry> {
        let Ok(things) = sorted(things);

        things
    }

    #[test]
    fn folders_come_before_files_however_they_are_named() {
        let things = in_order(vec![
            file("apple.txt", 10),
            folder("zebra"),
            file("banana.txt", 10),
            folder("aardvark"),
        ]);

        assert_eq!(names(&things), ["aardvark", "zebra", "apple.txt", "banana.txt"]);
    }

    #[test]
    fn a_name_sorts_where_it_reads_rather_than_where_its_capitals_put_it() {
        let things = in_order(vec![file("banana", 1), file("Apple", 1), file("cherry", 1)]);

        assert_eq!(names(&things), ["Apple", "banana", "cherry"]);
    }

    #[test]
    fn what_a_program_keeps_for_itself_is_not_shown() {
        assert_eq!(wanted(".config"), Ok(Shown::No));
        assert_eq!(wanted(".bashrc"), Ok(Shown::No));
        assert_eq!(wanted("holiday.jpg"), Ok(Shown::Yes));
    }

    #[test]
    fn a_size_is_said_in_the_largest_unit_it_fills() {
        assert_eq!(said(0), Ok("0 B".to_string()));
        assert_eq!(said(824), Ok("824 B".to_string()));
        assert_eq!(said(1024), Ok("1.0 KB".to_string()));
        assert_eq!(said(4 * 1024 * 1024 + 200 * 1024), Ok("4.2 MB".to_string()));
        assert_eq!(said(3 * (1 << 30)), Ok("3.0 GB".to_string()));
    }

    #[test]
    fn a_big_number_loses_the_decimal_nobody_reads() {
        assert_eq!(said(431 * (1 << 20)), Ok("431 MB".to_string()));
        assert_eq!(said(9 * (1 << 20)), Ok("9.0 MB".to_string()));
    }

    #[test]
    fn a_photograph_and_a_film_are_worth_a_picture_and_nothing_else_is() {
        assert_eq!(
            of_kind(file("beach.jpg", 1), "image/jpeg").worth_a_picture(),
            Ok(Worth::APicture),
        );
        assert_eq!(
            of_kind(file("holiday.mp4", 1), "video/mp4").worth_a_picture(),
            Ok(Worth::APicture),
        );
        assert_eq!(
            of_kind(file("notes.txt", 1), "text/plain").worth_a_picture(),
            Ok(Worth::ItsNameAlone),
        );
        assert_eq!(
            of_kind(folder("Holiday"), "inode/directory").worth_a_picture(),
            Ok(Worth::ItsNameAlone),
        );
    }

    #[test]
    fn one_thing_worth_drawing_gives_the_whole_listing_room_for_it() {
        let photo = of_kind(file("beach.jpg", 1), "image/jpeg");
        let notes = of_kind(file("notes.txt", 1), "text/plain");

        assert_eq!(wants_room(&[folder("Holiday"), photo]), Ok(Room::Kept));
        assert_eq!(wants_room(&[folder("Holiday"), notes.clone()]), Ok(Room::Kept));
        assert_eq!(wants_room(&[notes]), Ok(Room::Spared));
        assert_eq!(wants_room(&[]), Ok(Room::Spared));
    }

    #[test]
    fn a_folder_says_nothing_beside_itself_and_a_file_says_its_size() {
        assert_eq!(aside(&folder("Pictures")), Ok(String::new()));
        assert_eq!(aside(&file("holiday.jpg", 1024)), Ok("1.0 KB".to_string()));
    }
}
