//! A folder, in the order it is read and the words it is read in.


use console_core_never::Never;
use console_core_number_conversion::{Float, whole_u64};

use crate::unzipping::{self, Packed};

const SMALLEST: (&str, u64) = ("B", 1);


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
    Retained,
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
            true => return Ok(Room::Retained),
            false => {},
        }
    }

    Ok(Room::Spared)
}

pub fn visibility(name: &str) -> Result<Shown, Never> {
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
        false => format_size(entry.size),
    }
}

const UNITS: [(&str, u64); 4] = [("B", 1), ("KB", 1 << 10), ("MB", 1 << 20), ("GB", 1 << 30)];

pub fn format_size(bytes: u64) -> Result<String, Never> {
    let found =
        UNITS.iter().rev().find(|(_, worth)| bytes >= *worth).copied().or_else(|| UNITS.first().copied());

    let (unit, worth) = match found {
        Some(both) => both,
        None => SMALLEST,
    };
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

    fn names(things: &[Entry]) -> Result<Vec<&str>, Never> {
        Ok(things.iter().map(|thing| thing.name.as_str()).collect())
    }

    fn folder(name: &str) -> Result<Entry, Never> {
        let Ok(entry) = Entry::folder(name);

        Ok(entry)
    }

    fn file(name: &str, size: u64) -> Result<Entry, Never> {
        let Ok(entry) = Entry::file(name, size);

        Ok(entry)
    }

    fn of_kind(entry: Entry, kind: &str) -> Result<Entry, Never> {
        let Ok(entry) = entry.of_kind(kind);

        Ok(entry)
    }

    fn in_order(things: Vec<Entry>) -> Result<Vec<Entry>, Never> {
        let Ok(things) = sorted(things);

        Ok(things)
    }

    #[test]
    fn folders_come_before_files_however_they_are_named() {
        let Ok(zebra) = folder("zebra");
        let Ok(aardvark) = folder("aardvark");
        let Ok(apple_txt) = file("apple.txt", 10);
        let Ok(banana_txt) = file("banana.txt", 10);

        let Ok(things) = in_order(vec![
            apple_txt,
            zebra,
            banana_txt,
            aardvark,
        ]);

        let Ok(names) = names(&things);

        assert_eq!(names, ["aardvark", "zebra", "apple.txt", "banana.txt"]);
    }

    #[test]
    fn a_name_sorts_where_it_reads_rather_than_where_its_capitals_put_it() {
        let Ok(banana) = file("banana", 1);
        let Ok(apple) = file("Apple", 1);
        let Ok(cherry) = file("cherry", 1);
        let Ok(things) = in_order(vec![banana, apple, cherry]);
        let Ok(names) = names(&things);

        assert_eq!(names, ["Apple", "banana", "cherry"]);
    }

    #[test]
    fn what_a_program_keeps_for_itself_is_not_shown() {
        assert_eq!(visibility(".config"), Ok(Shown::No));
        assert_eq!(visibility(".bashrc"), Ok(Shown::No));
        assert_eq!(visibility("holiday.jpg"), Ok(Shown::Yes));
    }

    #[test]
    fn a_size_is_said_in_the_largest_unit_it_fills() {
        assert_eq!(format_size(0), Ok("0 B".to_string()));
        assert_eq!(format_size(824), Ok("824 B".to_string()));
        assert_eq!(format_size(1024), Ok("1.0 KB".to_string()));
        assert_eq!(format_size(4_399_104), Ok("4.2 MB".to_string()));
        assert_eq!(format_size(3_221_225_472), Ok("3.0 GB".to_string()));
    }

    #[test]
    fn a_big_number_loses_the_decimal_no_one_reads() {
        assert_eq!(format_size(451_936_256), Ok("431 MB".to_string()));
        assert_eq!(format_size(9_437_184), Ok("9.0 MB".to_string()));
    }

    #[test]
    fn a_photograph_and_a_film_are_worth_a_picture_and_nothing_else_is() {
        let Ok(beach_jpg) = file("beach.jpg", 1);
        let Ok(photograph) = of_kind(beach_jpg, "image/jpeg");

        assert_eq!(
            photograph.worth_a_picture(),
            Ok(Worth::APicture),
        );

        let Ok(holiday_mp4) = file("holiday.mp4", 1);
        let Ok(film) = of_kind(holiday_mp4, "video/mp4");

        assert_eq!(
            film.worth_a_picture(),
            Ok(Worth::APicture),
        );

        let Ok(notes_txt) = file("notes.txt", 1);
        let Ok(text) = of_kind(notes_txt, "text/plain");

        assert_eq!(
            text.worth_a_picture(),
            Ok(Worth::ItsNameAlone),
        );

        let Ok(holiday) = folder("Holiday");
        let Ok(folder) = of_kind(holiday, "inode/directory");

        assert_eq!(
            folder.worth_a_picture(),
            Ok(Worth::ItsNameAlone),
        );
    }

    #[test]
    fn one_thing_worth_drawing_gives_the_whole_listing_room_for_it() {
        let Ok(beach_jpg) = file("beach.jpg", 1);
        let Ok(photo) = of_kind(beach_jpg, "image/jpeg");
        let Ok(notes_txt) = file("notes.txt", 1);
        let Ok(notes) = of_kind(notes_txt, "text/plain");
        let Ok(holiday) = folder("Holiday");

        assert_eq!(wants_room(&[holiday, photo]), Ok(Room::Retained));

        let Ok(folder) = folder("Holiday");

        assert_eq!(wants_room(&[folder, notes.clone()]), Ok(Room::Retained));
        assert_eq!(wants_room(&[notes]), Ok(Room::Spared));
        assert_eq!(wants_room(&[]), Ok(Room::Spared));
    }

    #[test]
    fn a_folder_says_nothing_beside_itself_and_a_file_says_its_size() {
        let Ok(pictures) = folder("Pictures");

        assert_eq!(aside(&pictures), Ok(String::new()));

        let Ok(holiday_jpg) = file("holiday.jpg", 1024);

        assert_eq!(aside(&holiday_jpg), Ok("1.0 KB".to_string()));
    }
}
