//! The pages someone keeps, as things this desktop opens.
//!
//! A bookmark and an application are one gesture under two names: a thing on
//! the machine, opened from where the thumb already is. Opening one cost the
//! browser, the address bar and a list drawn at the top of the screen, which
//! is three surfaces to reach what the home screen reaches in one press --
//! and the home screen already holds a handful of things by name, chosen in
//! the menu, out of every application there is.
//!
//! So a bookmark is written as a desktop entry, and then it is not a special
//! thing at all: `console-applications` finds it with everything else, the
//! menu lists it, Y puts it on the wallpaper or takes it off, and A opens it.
//! Nothing in the menu, the home screen or the panel knows a bookmark from an
//! application, which is the whole of why this is small. The entry itself is
//! spelled by `console_applications::entry::serialize`, because what a
//! `.desktop` file says is that crate's to say in both directions.
//!
//! What the browser keeps is not readable from out here. `places.sqlite` is a
//! database locked while the browser runs, and reading it would be this tree
//! learning a format to answer a question the browser already answers. So the
//! add-on says what the bookmarks are, over the same privileged door that
//! raises the keyboard, and this is what the saying means.
//!
//! Every file written is named for the browser's own id for the bookmark,
//! under one mark. That is what makes a bookmark someone deleted disappear:
//! a file under the mark that the browser did not name this time is one that
//! has gone, and nothing else in `applications/` is touched.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use console_applications::entry::{self, Application};
use console_applications::words;
use console_core_external_programs::Program;
use console_core_never::Never;

pub const MARK: &str = "console-bookmark-";

pub const SUFFIX: &str = ".desktop";

pub const ICONS: &str = "bookmark-icons";

const ADDRESSES: [&str; 2] = ["https://", "http://"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub id: String,
    pub url: String,
    pub name: String,
    pub icon: Vec<u8>,
}

pub fn read(said: &str) -> Result<Vec<Bookmark>, Never> {
    let mut bookmarks = Vec::new();

    for line in said.lines() {
        let mut fields = line.split('\t');

        let (id, url, name) = match (fields.next(), fields.next(), fields.next()) {
            (Some(id), Some(url), Some(name)) => (id, url, name),
            (None, _, _) | (_, None, _) | (_, _, None) => continue,
        };

        let icon = match fields.next() {
            Some(said) => bytes(said)?,
            None => Vec::new(),
        };

        let held = ADDRESSES.iter().any(|head| url.starts_with(head));

        match held {
            true => {},
            false => continue,
        }

        let id = slug(id)?;

        match id.is_empty() {
            true => continue,
            false => {},
        }

        bookmarks.push(Bookmark {
            id,
            url: url.to_string(),
            name: name.trim().to_string(),
            icon,
        });
    }

    Ok(bookmarks)
}

pub fn slug(id: &str) -> Result<String, Never> {
    Ok(id
        .chars()
        .filter(|letter| letter.is_ascii_alphanumeric() || *letter == '-' || *letter == '_')
        .collect())
}

fn bytes(said: &str) -> Result<Vec<u8>, Never> {
    let digits: Vec<char> = said.chars().collect();
    let read: Result<Vec<u8>, _> = digits
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(&pair.iter().collect::<String>(), 16))
        .collect();

    Ok(match read {
        Ok(bytes) => bytes,
        Err(_not_a_picture) => Vec::new(),
    })
}

pub fn host(url: &str) -> Result<String, Never> {
    let after = ADDRESSES.iter().find_map(|head| url.strip_prefix(head));

    let rest = match after {
        Some(rest) => rest,
        None => url,
    };

    Ok(match rest.split('/').next() {
        Some(host) => host.to_string(),
        None => rest.to_string(),
    })
}

pub fn apart(bookmarks: Vec<Bookmark>) -> Result<Vec<Bookmark>, Never> {
    let mut taken: BTreeSet<String> = BTreeSet::new();
    let mut apart: Vec<Bookmark> = Vec::new();

    for bookmark in bookmarks {
        let where_ = host(&bookmark.url)?;

        let said = match bookmark.name.is_empty() {
            true => where_.clone(),
            false => bookmark.name.clone(),
        };

        let name = free(&said, Host(&where_), &taken)?;

        taken.insert(name.clone());

        apart.push(Bookmark { name, ..bookmark });
    }

    Ok(apart)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Host<'a>(pub &'a str);

fn free(said: &str, host: Host<'_>, taken: &BTreeSet<String>) -> Result<String, Never> {
    let Host(host) = host;

    match taken.contains(said) {
        false => return Ok(said.to_string()),
        true => {},
    }

    let beside = format!("{said} ({host})");

    match taken.contains(&beside) {
        false => return Ok(beside),
        true => {},
    }

    let free = (2..=u32::MAX).map(|again| format!("{said} ({host} {again})")).find(|more| !taken.contains(more));

    Ok(match free {
        Some(free) => free,
        None => beside,
    })
}

pub fn application(bookmark: &Bookmark, icon: Option<&Path>) -> Result<Application, Never> {
    let Ok(open) = Program::XdgOpen.name();

    let command = words::join_quoted(&[open.to_string(), bookmark.url.clone()])?;

    Ok(Application {
        name: bookmark.name.clone(),
        command,
        terminal: false,
        icon: match icon {
            Some(at) => at.display().to_string(),
            None => String::new(),
        },
    })
}

pub fn among() -> Result<Option<PathBuf>, Never> {
    let hers = console_core_places::Base::Share.user()?;

    Ok(hers.map(|hers| hers.join(console_core_places::APPLICATIONS)))
}

pub fn icons() -> Result<Option<PathBuf>, Never> {
    let ours = console_core_places::Base::Share.ours()?;

    Ok(ours.map(|ours| ours.join(ICONS)))
}

pub fn entry_at(among: &Path, id: &str) -> Result<PathBuf, Never> {
    Ok(among.join(format!("{MARK}{id}{SUFFIX}")))
}

pub fn icon_at(under: &Path, id: &str) -> Result<PathBuf, Never> {
    Ok(under.join(id))
}

#[derive(Debug)]
pub enum Unkept {
    Making(PathBuf, std::io::Error),
    Writing(console_core_atomic_writes::Unwritten),
    Reading(PathBuf, std::io::Error),
    Removing(PathBuf, std::io::Error),
}

impl fmt::Display for Unkept {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unkept::Making(at, fault) => write!(to, "{}: making it: {fault}", at.display()),
            Unkept::Writing(fault) => write!(to, "{fault}"),
            Unkept::Reading(at, fault) => write!(to, "{}: reading it: {fault}", at.display()),
            Unkept::Removing(at, fault) => {
                write!(to, "{}: taking it away: {fault}", at.display())
            }
        }
    }
}

impl std::error::Error for Unkept {}

impl From<console_core_atomic_writes::Unwritten> for Unkept {
    fn from(fault: console_core_atomic_writes::Unwritten) -> Unkept {
        Unkept::Writing(fault)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Where<'a> {
    pub among: &'a Path,
    pub icons: &'a Path,
}

pub fn keep(where_: Where<'_>, bookmarks: &[Bookmark]) -> Result<(), Unkept> {
    for at in [where_.among, where_.icons] {
        std::fs::create_dir_all(at).map_err(|fault| Unkept::Making(at.to_path_buf(), fault))?;
    }

    for bookmark in bookmarks {
        let Ok(at) = icon_at(where_.icons, &bookmark.id);

        let icon = match bookmark.icon.is_empty() {
            true => None,
            false => {
                console_core_atomic_writes::whole(&at, &bookmark.icon)?;

                Some(at)
            }
        };

        let Ok(application) = application(bookmark, icon.as_deref());
        let Ok(said) = entry::serialize(&application);
        let Ok(at) = entry_at(where_.among, &bookmark.id);

        console_core_atomic_writes::whole(&at, said.as_bytes())?;
    }

    let kept: BTreeSet<&str> = bookmarks.iter().map(|bookmark| bookmark.id.as_str()).collect();

    swept(where_.among, &kept)?;

    remove_stale_icons(where_.icons, &kept)
}

fn swept(among: &Path, kept: &BTreeSet<&str>) -> Result<(), Unkept> {
    let deleting = paths_to_delete(among, kept, Under::TheMark)?;

    away(&deleting)
}

fn remove_stale_icons(icons: &Path, kept: &BTreeSet<&str>) -> Result<(), Unkept> {
    let deleting = paths_to_delete(icons, kept, Under::OurOwn)?;

    away(&deleting)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Under {
    TheMark,
    OurOwn,
}

fn paths_to_delete(at: &Path, kept: &BTreeSet<&str>, under: Under) -> Result<Vec<PathBuf>, Unkept> {
    let reading =
        std::fs::read_dir(at).map_err(|fault| Unkept::Reading(at.to_path_buf(), fault))?;

    let mut deleting = Vec::new();

    for child in reading.filter_map(Result::ok) {
        let path = child.path();

        let said = match path.file_name().and_then(|said| said.to_str()) {
            Some(said) => said.to_string(),
            None => continue,
        };

        let id = match under {
            Under::TheMark => match said.strip_prefix(MARK).and_then(|rest| rest.strip_suffix(SUFFIX)) {
                Some(id) => id.to_string(),
                None => continue,
            },
            Under::OurOwn => said,
        };

        match kept.contains(id.as_str()) {
            true => continue,
            false => deleting.push(path),
        }
    }

    Ok(deleting)
}

fn away(deleting: &[PathBuf]) -> Result<(), Unkept> {
    for at in deleting {
        std::fs::remove_file(at).map_err(|fault| Unkept::Removing(at.clone(), fault))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    fn somewhere(named: &str) -> Result<PathBuf, console_core_temporary_directories::Unmade> {
        console_core_temporary_directories::fresh(&format!("bookmarks-{named}"))
    }

    fn bookmarks(rows: &[(&str, &str, &str, &[u8])]) -> Result<Vec<Bookmark>, Never> {
        Ok(rows
            .iter()
            .map(|(id, url, name, icon)| Bookmark {
                id: id.to_string(),
                url: url.to_string(),
                name: name.to_string(),
                icon: icon.to_vec(),
            })
            .collect())
    }

    #[test]
    fn a_bookmark_is_an_id_an_address_a_name_and_a_picture() -> Result<(), Box<dyn Error>> {
        let said = "abc\thttps://example.com/\tExample\t89504e47\n";
        let Ok(read) = read(said);
        let first = read.first().ok_or("no bookmark was read")?;

        assert_eq!(read.len(), 1);
        assert_eq!(first.url, "https://example.com/");
        assert_eq!(first.name, "Example");
        assert_eq!(first.icon, vec![0x89, 0x50, 0x4e, 0x47]);
        Ok(())
    }

    #[test]
    fn a_bookmark_with_no_picture_is_a_bookmark() -> Result<(), Box<dyn Error>> {
        let Ok(read) = read("abc\thttps://example.com/\tExample\n");
        let first = read.first().ok_or("no bookmark was read")?;

        assert_eq!(read.len(), 1);
        assert!(first.icon.is_empty());
        Ok(())
    }

    #[test]
    fn what_is_not_a_page_is_not_something_to_open() {
        let said = "\
a\tjavascript:alert(1)\tA trick
b\tplace:sort=8\tRecently used
c\tabout:config\tThe knobs
d\thttps://example.com/\tA page
";
        let Ok(read) = read(said);
        let names: Vec<&str> = read.iter().map(|bookmark| bookmark.name.as_str()).collect();

        assert_eq!(names, ["A page"]);
    }

    #[test]
    fn a_line_that_is_not_a_bookmark_is_not_one() {
        let Ok(read) = read("\nnonsense\nabc\thttps://example.com\n");

        assert!(read.is_empty());
    }

    #[test]
    fn an_id_is_only_what_can_be_a_file_name() -> Result<(), Box<dyn Error>> {
        let Ok(climbing) = slug("../../etc/passwd");
        let Ok(plain) = slug("YabvG_NXX-sxMK");
        let Ok(read) = read("../etc/passwd\thttps://example.com\tA page\n");
        let Ok(nameless) = super::read("/\thttps://example.com\tA page\n");
        let first = read.first().ok_or("no bookmark was read")?;

        assert_eq!(climbing, "etcpasswd");
        assert_eq!(plain, "YabvG_NXX-sxMK");
        assert_eq!(first.id, "etcpasswd");
        assert!(nameless.is_empty(), "a name of nothing");
        Ok(())
    }

    #[test]
    fn a_picture_that_is_not_one_is_no_picture_rather_than_half_of_one() -> Result<(), Box<dyn Error>> {
        let Ok(read) = read("abc\thttps://example.com\tA page\tnot a picture\n");
        let first = read.first().ok_or("no bookmark was read")?;

        assert!(first.icon.is_empty());
        Ok(())
    }

    #[test]
    fn two_pages_called_the_same_thing_are_told_apart_by_where_they_are() {
        let Ok(bookmarks) = bookmarks(&[
            ("a", "https://one.example.com/x", "Home", &[]),
            ("b", "https://two.example.com/y", "Home", &[]),
            ("c", "https://two.example.com/z", "Home", &[]),
        ]);
        let Ok(apart) = apart(bookmarks);
        let names: Vec<&str> = apart.iter().map(|bookmark| bookmark.name.as_str()).collect();

        assert_eq!(names, ["Home", "Home (two.example.com)", "Home (two.example.com 2)"]);
    }

    #[test]
    fn a_page_with_no_name_is_called_where_it_is() -> Result<(), Box<dyn Error>> {
        let Ok(bookmarks) = bookmarks(&[("a", "https://example.com/deep/page", "", &[])]);
        let Ok(apart) = apart(bookmarks);
        let first = apart.first().ok_or("no bookmark was kept")?;

        assert_eq!(first.name, "example.com");
        Ok(())
    }

    #[test]
    fn what_opens_a_bookmark_is_the_address_it_was_given() -> Result<(), Box<dyn Error>> {
        let Ok(bookmarks) = bookmarks(&[("a", "https://example.com/?q=a b&r=2", "A page", &[])]);
        let bookmark = bookmarks.first().ok_or("no bookmark was made")?;
        let Ok(opened) = application(bookmark, None);
        let Ok(read) = console_applications::words::without_field_codes(&opened.command);
        let Ok(split) = console_applications::words::split(&read);

        assert_eq!(
            split,
            Some(vec!["xdg-open".to_string(), "https://example.com/?q=a b&r=2".to_string()])
        );
        Ok(())
    }

    #[test]
    fn a_bookmark_is_written_where_the_menu_looks_for_applications() -> Result<(), Box<dyn Error>> {
        let at = somewhere("written")?;
        let among = at.join("applications");
        let icons = at.join("icons");
        let Ok(bookmarks) = bookmarks(&[("abc", "https://example.com/", "Example", &[0x89, 0x50])]);

        keep(Where { among: &among, icons: &icons }, &bookmarks)?;

        let said = std::fs::read_to_string(among.join("console-bookmark-abc.desktop"))?;
        let picture = std::fs::read(icons.join("abc"))?;

        assert!(said.contains("Name=Example"), "{said}");
        assert!(said.contains("Icon="), "{said}");
        assert_eq!(picture, vec![0x89, 0x50]);

        let _ = std::fs::remove_dir_all(&at);
        Ok(())
    }

    #[test]
    fn a_bookmark_that_has_gone_from_the_browser_goes_from_the_menu() -> Result<(), Box<dyn Error>> {
        let at = somewhere("gone")?;
        let among = at.join("applications");
        let icons = at.join("icons");
        let Ok(first) = bookmarks(&[
            ("abc", "https://example.com/", "Example", &[0x89]),
            ("def", "https://other.example/", "Other", &[0x89]),
        ]);
        let fewer = first.get(..1).ok_or("one bookmark fewer")?;

        keep(Where { among: &among, icons: &icons }, &first)?;
        keep(Where { among: &among, icons: &icons }, fewer)?;

        assert!(among.join("console-bookmark-abc.desktop").exists());
        assert!(!among.join("console-bookmark-def.desktop").exists());
        assert!(icons.join("abc").exists());
        assert!(!icons.join("def").exists());

        let _ = std::fs::remove_dir_all(&at);
        Ok(())
    }

    #[test]
    fn nothing_that_is_not_a_bookmark_is_swept() -> Result<(), Box<dyn Error>> {
        let at = somewhere("beside")?;
        let among = at.join("applications");
        let icons = at.join("icons");

        std::fs::create_dir_all(&among)?;
        console_core_atomic_writes::whole(&among.join("firefox.desktop"), b"[Desktop Entry]")?;
        keep(Where { among: &among, icons: &icons }, &[])?;

        assert!(among.join("firefox.desktop").exists());

        let _ = std::fs::remove_dir_all(&at);
        Ok(())
    }

    #[test]
    fn where_a_bookmark_is_kept_is_where_the_menu_reads_from() {
        let home = Path::new("/home/someone");
        let Ok(share) = console_core_places::Base::Share.under(home);
        let Ok(ours) = console_core_places::Base::Share.application_under(home);

        assert_eq!(share.join("applications"), PathBuf::from("/home/someone/.local/share/applications"));
        assert_eq!(
            ours.join(ICONS),
            PathBuf::from("/home/someone/.local/share/console/bookmark-icons")
        );
    }
}
