//! What the library knows about each book: its title and its cover.
//!
//!     books-catalog
//!
//! Outside the library, like `panel-pictures` and for the same reason: asking a book
//! its title means opening it, and a cover means unpacking a picture out of it
//! or asking poppler to draw a first page. The library starts this for the books
//! it has not seen, goes on drawing them under their file names, and reads the
//! catalog again when it has been written.
//!
//! An EPUB's cover is the picture it names as one, written out as it was
//! packed. A comic's is its first page. A PDF's is its first page drawn by
//! poppler, because that is what a PDF's cover is. The pictures are written
//! whole into the cache and the panel's picture store decodes them at the size
//! a cover is drawn, the same way it does every thumbnail on this machine.
//!
//! A book that leaves the folder leaves the catalog, with its cover.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_books::library::{self, CatalogEntry, Format};
use console_books::open;
use console_core_external_programs::Program;
use console_core_never::Never;

const COVER_TALL: &str = "900";

fn main() -> ExitCode {
    let Ok(home) = console_core_places::home();

    let home = match home {
        Some(home) => home,
        None => {
            eprintln!("books-catalog: HOME is not set, so there is no Books folder to read");

            return ExitCode::FAILURE;
        },
    };

    match update_catalog(&home) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("books-catalog: {fault}");

            ExitCode::FAILURE
        },
    }
}

#[derive(Debug)]
enum CatalogError {
    Folder(PathBuf, std::io::Error),
    Written(console_core_atomic_writes::Unwritten),
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CatalogError::Folder(at, fault) => write!(to, "{}: {fault}", at.display()),
            CatalogError::Written(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for CatalogError {}

fn update_catalog(home: &Path) -> Result<(), CatalogError> {
    let Ok(folder) = library::books_folder(home);
    let Ok(cache) = library::cache_folder(home);
    let covers = cache.join("covers");
    let at = cache.join(library::CATALOG);

    std::fs::create_dir_all(&covers).map_err(|fault| CatalogError::Folder(covers.clone(), fault))?;

    let Ok(contents) = console_core_atomic_writes::read(&at);
    let contents = match contents.text() {
        Ok(Some(contents)) => contents,
        Ok(None) | Err(_) => String::new(),
    };

    let Ok(mut catalog) = library::read_catalog(&contents);
    let Ok(names) = library::names(&folder);
    let Ok(books) = library::books(&folder, &names, &BTreeMap::new());

    let present: std::collections::BTreeSet<&String> = names.iter().collect();

    catalog.retain(|name, _| present.contains(name));

    let Ok(()) = add_missing(&books, &covers, &mut catalog);
    let Ok(text) = library::written_catalog(&catalog);

    console_core_atomic_writes::whole(&at, text.as_bytes()).map_err(CatalogError::Written)
}

#[cfg_attr(
    dylint_lib = "explicit029_no_asking_per_item",
    allow(
        explicit029_no_asking_per_item,
        reason = "a catalog is a walk over the books the library has not seen, opening each one; that is the whole of the work this program exists to do off the library.s loop, and it is done once per book rather than once per opening"
    )
)]
fn add_missing(books: &[library::Book], covers: &Path, catalog: &mut BTreeMap<String, CatalogEntry>) -> Result<(), Never> {
    for book in books {
        match catalog.contains_key(&book.name) {
            true => continue,
            false => {},
        }

        let Ok(entry) = entry(book, covers);

        catalog.insert(book.name.clone(), entry);
    }

    Ok(())
}

fn entry(book: &library::Book, covers: &Path) -> Result<CatalogEntry, Never> {
    let read = match book.format {
        Format::Publication => from_publication(book, covers),
        Format::Comic => from_comic(book, covers),
        Format::PortableDocument => from_portable_document(book, covers),
    };

    Ok(match read {
        Ok(entry) => entry,
        Err(fault) => {
            eprintln!("books-catalog: {}: {fault}", book.path.display());

            CatalogEntry { title: book.title.clone(), cover: None }
        },
    })
}

fn write_cover(covers: &Path, book: &library::Book, cover: CoverImage<'_>) -> Result<Option<PathBuf>, Never> {
    let Ok(ending) = open::ending(cover.name);
    let at = covers.join(format!("{}.{ending}", book.name));

    Ok(match console_core_atomic_writes::whole(&at, cover.bytes) {
        Ok(()) => Some(at),
        Err(fault) => {
            eprintln!("books-catalog: {fault}");

            None
        },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CoverImage<'a> {
    name: &'a str,
    bytes: &'a [u8],
}

fn from_publication(book: &library::Book, covers: &Path) -> Result<CatalogEntry, open::BookError> {
    let opened = open::publication(&book.path)?;

    let title = match &opened.publication.title {
        Some(title) => title.clone(),
        None => book.title.clone(),
    };

    let cover = match &opened.publication.cover {
        Some(named) => {
            let bytes = opened.archive.extract(named)?;
            let Ok(written) = write_cover(covers, book, CoverImage { name: named, bytes: &bytes });

            written
        },
        None => None,
    };

    Ok(CatalogEntry { title, cover })
}

fn from_comic(book: &library::Book, covers: &Path) -> Result<CatalogEntry, open::BookError> {
    let opened = open::comic(&book.path)?;

    let cover = match opened.pages.first() {
        Some(named) => {
            let bytes = opened.archive.extract(named)?;
            let Ok(written) = write_cover(covers, book, CoverImage { name: named, bytes: &bytes });

            written
        },
        None => None,
    };

    Ok(CatalogEntry { title: book.title.clone(), cover })
}

fn from_portable_document(book: &library::Book, covers: &Path) -> Result<CatalogEntry, open::BookError> {
    let at = covers.join(&book.name);
    let Ok(mut drawing) = Program::Pdftoppm.command();

    let drawn = drawing
        .args(["-f", "1", "-l", "1", "-scale-to-y", COVER_TALL, "-scale-to-x", "-1", "-png", "-singlefile"])
        .arg(&book.path)
        .arg(&at)
        .status()
        .map_err(|fault| open::BookError::Reading(book.path.clone(), fault))?;

    let png = PathBuf::from(format!("{}.png", at.display()));

    let cover = match (drawn.success(), png.exists()) {
        (true, true) => Some(png),
        (true, false) | (false, _) => None,
    };

    Ok(CatalogEntry { title: book.title.clone(), cover })
}
