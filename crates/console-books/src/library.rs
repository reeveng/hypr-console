//! Which books there are, what each is called, and which of them a search
//! means.
//!
//! The books are the files in `~/Books` this reader can open: an EPUB, a PDF,
//! or a comic packed as a CBZ. One folder rather than a walk of the whole
//! home, because a PDF in Downloads is as likely to be a payslip as a novel,
//! and a library is a place somebody put things on purpose. The downloads panel
//! puts what it fetches there, and the files panel opens anything else in
//! place.
//!
//! What a book is called is its own title where it says one and its file's
//! name where it does not. Asking every book for its title means opening every
//! book, which is the library taking a second per hundred books to appear; so
//! `books-catalog` asks once, outside the library, and writes the answers beside the
//! covers it made, and the library reads that one file. A book the catalog has
//! not reached yet is in the library under its file's name until it has.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_places::Base;

pub const CATALOG: &str = "catalog";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Publication,
    PortableDocument,
    Comic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Book {
    pub path: PathBuf,
    pub name: String,
    pub format: Format,
    pub title: String,
    pub cover: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub title: String,
    pub cover: Option<PathBuf>,
}

pub fn books_folder(home: &Path) -> Result<PathBuf, Never> {
    console_core_places::books_under(home)
}

pub fn names(folder: &Path) -> Result<Vec<String>, Never> {
    let mut found: Vec<String> = Vec::new();

    for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        match entry.file_name().into_string() {
            Ok(name) => found.push(name),
            Err(_not_text) => {},
        }
    }

    Ok(found)
}

pub fn cache_folder(home: &Path) -> Result<PathBuf, Never> {
    let Ok(ours) = Base::Cache.ours_under(home);

    Ok(ours.join("books"))
}

pub fn format_of(name: &str) -> Result<Option<Format>, Never> {
    let ending = match name.rsplit_once('.') {
        Some((_, ending)) => ending.to_ascii_lowercase(),
        None => return Ok(None),
    };

    Ok(match ending.as_str() {
        "epub" => Some(Format::Publication),
        "pdf" => Some(Format::PortableDocument),
        "cbz" => Some(Format::Comic),
        _ => None,
    })
}

pub fn title_from_name(name: &str) -> Result<String, Never> {
    let stem = match name.rsplit_once('.') {
        Some((stem, _)) => stem,
        None => name,
    };

    Ok(stem.replace('_', " ").trim().to_string())
}

pub fn read_catalog(text: &str) -> Result<BTreeMap<String, CatalogEntry>, Never> {
    let mut found = BTreeMap::new();

    for line in text.lines() {
        let mut parts = line.split('\t');

        match (parts.next(), parts.next(), parts.next()) {
            (Some(name), Some(title), cover) => {
                let cover = cover.filter(|cover| !cover.is_empty()).map(PathBuf::from);

                found.insert(name.to_string(), CatalogEntry { title: title.to_string(), cover });
            },
            (None, _, _) | (_, None, _) => {},
        }
    }

    Ok(found)
}

pub fn written_catalog(entries: &BTreeMap<String, CatalogEntry>) -> Result<String, Never> {
    let mut text = String::new();

    for (name, cataloged) in entries {
        let cover = match &cataloged.cover {
            Some(cover) => cover.display().to_string(),
            None => String::new(),
        };

        text.push_str(&format!("{name}\t{}\t{cover}\n", cataloged.title.replace(['\t', '\n'], " ")));
    }

    Ok(text)
}

pub fn books(folder: &Path, names: &[String], catalog: &BTreeMap<String, CatalogEntry>) -> Result<Vec<Book>, Never> {
    let mut books = Vec::new();

    for name in names {
        let Ok(format) = format_of(name);

        let format = match format {
            Some(format) => format,
            None => continue,
        };

        let (title, cover) = match catalog.get(name) {
            Some(cataloged) => (cataloged.title.clone(), cataloged.cover.clone()),
            None => {
                let Ok(title) = title_from_name(name);

                (title, None)
            },
        };

        books.push(Book { path: folder.join(name), name: name.clone(), format, title, cover });
    }

    books.sort_by_key(|book| book.title.to_lowercase());

    Ok(books)
}

pub fn search<'a>(books: &'a [Book], search: &str) -> Result<Vec<&'a Book>, Never> {
    let wanted = search.trim().to_lowercase();

    Ok(books.iter().filter(|book| book.title.to_lowercase().contains(&wanted)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(text: &[&str]) -> Vec<String> {
        text.iter().map(|name| (*name).to_string()).collect()
    }

    #[test]
    fn the_library_is_the_books_in_the_folder_by_title_and_nothing_else() {
        let mut catalog = BTreeMap::new();
        catalog.insert(
            "pg2701.epub".to_string(),
            CatalogEntry { title: "Moby Dick".to_string(), cover: Some(PathBuf::from("/c/moby.jpg")) },
        );

        let Ok(books) = books(Path::new("/b"), &names(&["pg2701.epub", "notes.txt", "Akira_01.cbz", "manual.PDF"]), &catalog);
        let titles: Vec<(&str, Format)> = books.iter().map(|book| (book.title.as_str(), book.format)).collect();

        assert_eq!(titles, vec![("Akira 01", Format::Comic), ("manual", Format::PortableDocument), ("Moby Dick", Format::Publication)]);
        assert_eq!(books.last().and_then(|book| book.cover.clone()), Some(PathBuf::from("/c/moby.jpg")));
    }

    #[test]
    fn a_search_is_any_part_of_a_title_in_any_case() {
        let Ok(books) = books(Path::new("/b"), &names(&["The Martian.epub", "Moby Dick.epub", "Martian Chronicles.pdf"]), &BTreeMap::new());
        let Ok(found) = search(&books, " marTIAN ");
        let titles: Vec<&str> = found.iter().map(|book| book.title.as_str()).collect();

        assert_eq!(titles, vec!["Martian Chronicles", "The Martian"]);
    }

    #[test]
    fn the_catalog_is_read_back_as_it_was_written() {
        let mut entries = BTreeMap::new();
        entries.insert("a.epub".to_string(), CatalogEntry { title: "A\ttitle".to_string(), cover: None });
        entries.insert("b.pdf".to_string(), CatalogEntry { title: "B".to_string(), cover: Some(PathBuf::from("/c/b.png")) });

        let Ok(text) = written_catalog(&entries);
        let Ok(read) = read_catalog(&text);

        assert_eq!(read.get("a.epub").map(|entry| entry.title.as_str()), Some("A title"));
        assert_eq!(read.get("b.pdf").and_then(|entry| entry.cover.clone()), Some(PathBuf::from("/c/b.png")));
    }
}
