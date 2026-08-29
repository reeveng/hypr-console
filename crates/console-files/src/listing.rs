//! A folder, in the order it is read and the words it is read in.

/// One thing in a folder.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Entry {
    pub name: String,
    pub folder: bool,
    /// What kind of thing the machine says it is, where it has said.
    pub kind: String,
    pub size: u64,
}

impl Entry {
    pub fn folder(name: &str) -> Self {
        Entry { name: name.to_string(), folder: true, kind: String::new(), size: 0 }
    }

    pub fn file(name: &str, size: u64) -> Self {
        Entry { name: name.to_string(), folder: false, kind: String::new(), size }
    }

    pub fn of_kind(mut self, kind: &str) -> Self {
        self.kind = kind.to_string();
        self
    }

    /// Whether this is the sort of thing a picture of it would say anything
    /// about.
    ///
    /// A photograph and a film. Everything else on this device is told apart by
    /// its name faster than by a picture of it, and a page of documents each
    /// wearing a small grey rectangle is a page that is harder to read than one
    /// without.
    pub fn worth_a_picture(&self) -> bool {
        !self.folder && (self.kind.starts_with("image/") || self.kind.starts_with("video/"))
    }

    /// Whether it is a picture, which a film is not.
    ///
    /// A wallpaper is one still image, and the machine draws it once and leaves
    /// it there. Offering to make a film into one is offering something that
    /// cannot be done.
    pub fn a_picture(&self) -> bool {
        !self.folder && self.kind.starts_with("image/")
    }
}

/// Whether a listing keeps room at the front of its rows.
///
/// Asked of the whole folder rather than of each row. Room only where there is
/// something to put in it would leave a folder of photographs and folders with
/// its names starting in two places, and a folder with nothing to draw keeps
/// none of it.
///
/// A folder is something to draw: it wears the mark that says it is one and
/// opens like one, which is the difference a listing of nothing but folders
/// used to make you press A to find out.
pub fn wants_room(things: &[Entry]) -> bool {
    things.iter().any(|thing| thing.folder || thing.worth_a_picture())
}

/// Whether a name is one to show.
///
/// A dotfile is a file some program keeps for itself, and a home directory has
/// more of them in it than things anybody put there. Shown, the first screen of
/// Home is a list of configuration nobody opened this to look at, and what she
/// was looking for is three screens down.
pub fn wanted(name: &str) -> bool {
    !name.starts_with('.')
}

/// Folders first, then everything else, each by name.
///
/// Folders first because walking is what this is for: the thing a thumb does
/// most is go one deeper, and a listing that mixes folders through the files
/// makes that a hunt. By name without regard to case, because a folder called
/// Photos and one called photos sitting at opposite ends of the list is the
/// alphabet of a machine rather than of a person.
pub fn sorted(mut things: Vec<Entry>) -> Vec<Entry> {
    things.sort_by(|one, other| {
        other
            .folder
            .cmp(&one.folder)
            .then_with(|| one.name.to_lowercase().cmp(&other.name.to_lowercase()))
    });
    things
}

/// What is written beside a row.
///
/// Nothing for a folder. How many things are in one is another read of another
/// directory, for every folder on the screen, and on a stick over USB that is
/// the listing arriving in its own time rather than at once.
pub fn aside(entry: &Entry) -> String {
    match entry.folder {
        true => String::new(),
        false => said(entry.size),
    }
}

/// The units, smallest first, and what each is worth.
const UNITS: [(&str, u64); 4] = [("B", 1), ("KB", 1 << 10), ("MB", 1 << 20), ("GB", 1 << 30)];

/// A size, in as few characters as it can be said in.
///
/// One decimal place while the number is small enough for it to mean anything,
/// and none once it is not: 4.2 MB and 0.9 KB are readings, and 431.7 MB is a
/// number with a digit on the end nobody reads and every file has a different
/// one of.
pub fn said(bytes: u64) -> String {
    let (unit, worth) = UNITS
        .iter()
        .rev()
        .find(|(_, worth)| bytes >= *worth)
        .copied()
        .unwrap_or(UNITS[0]);
    let much = bytes as f64 / worth as f64;
    match unit == "B" || much >= 10.0 {
        true => format!("{} {unit}", much.round() as u64),
        false => format!("{much:.1} {unit}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(things: &[Entry]) -> Vec<&str> {
        things.iter().map(|thing| thing.name.as_str()).collect()
    }

    #[test]
    fn folders_come_before_files_however_they_are_named() {
        let things = sorted(vec![
            Entry::file("apple.txt", 10),
            Entry::folder("zebra"),
            Entry::file("banana.txt", 10),
            Entry::folder("aardvark"),
        ]);
        assert_eq!(names(&things), ["aardvark", "zebra", "apple.txt", "banana.txt"]);
    }

    /// A person's alphabet rather than a machine's, where the capitals do not
    /// all sort before the rest.
    #[test]
    fn a_name_sorts_where_it_reads_rather_than_where_its_capitals_put_it() {
        let things = sorted(vec![
            Entry::file("banana", 1),
            Entry::file("Apple", 1),
            Entry::file("cherry", 1),
        ]);
        assert_eq!(names(&things), ["Apple", "banana", "cherry"]);
    }

    #[test]
    fn what_a_program_keeps_for_itself_is_not_shown() {
        assert!(!wanted(".config"));
        assert!(!wanted(".bashrc"));
        assert!(wanted("holiday.jpg"));
    }

    #[test]
    fn a_size_is_said_in_the_largest_unit_it_fills() {
        assert_eq!(said(0), "0 B");
        assert_eq!(said(824), "824 B");
        assert_eq!(said(1024), "1.0 KB");
        assert_eq!(said(4 * 1024 * 1024 + 200 * 1024), "4.2 MB");
        assert_eq!(said(3 * (1 << 30)), "3.0 GB");
    }

    /// The digit is dropped once there are enough in front of it that nobody
    /// reads it.
    #[test]
    fn a_big_number_loses_the_decimal_nobody_reads() {
        assert_eq!(said(431 * (1 << 20)), "431 MB");
        assert_eq!(said(9 * (1 << 20)), "9.0 MB");
    }

    #[test]
    fn a_photograph_and_a_film_are_worth_a_picture_and_nothing_else_is() {
        assert!(Entry::file("beach.jpg", 1).of_kind("image/jpeg").worth_a_picture());
        assert!(Entry::file("holiday.mp4", 1).of_kind("video/mp4").worth_a_picture());
        assert!(!Entry::file("notes.txt", 1).of_kind("text/plain").worth_a_picture());
        assert!(!Entry::folder("Holiday").of_kind("inode/directory").worth_a_picture());
    }

    /// Otherwise a folder holding both would have its names starting in two
    /// places, and the ones without a picture would read as indented.
    #[test]
    fn one_thing_worth_drawing_gives_the_whole_listing_room_for_it() {
        let photo = Entry::file("beach.jpg", 1).of_kind("image/jpeg");
        let notes = Entry::file("notes.txt", 1).of_kind("text/plain");
        assert!(wants_room(&[Entry::folder("Holiday"), photo]));
        assert!(wants_room(&[Entry::folder("Holiday"), notes.clone()]));
        assert!(!wants_room(&[notes]));
        assert!(!wants_room(&[]));
    }

    #[test]
    fn a_folder_says_nothing_beside_itself_and_a_file_says_its_size() {
        assert_eq!(aside(&Entry::folder("Pictures")), "");
        assert_eq!(aside(&Entry::file("holiday.jpg", 1024)), "1.0 KB");
    }
}
