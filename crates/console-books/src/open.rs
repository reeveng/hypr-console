//! A book off the disk, held open.
//!
//! An EPUB and a comic are both a zip, read whole into memory once and asked
//! for one page at a time: a novel is a megabyte and a volume of a comic is a
//! few dozen, which is less than a film's worth of frames and far less than
//! the cost of opening the file again on every turn. A PDF is not held at all,
//! because poppler is asked for each page by the file's name.
//!
//! The pictures a comic is made of are its images in the order of their names,
//! which is the only order a CBZ has: the format is a zip somebody filled, and
//! the names are how they numbered the pages.

use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_zip_files::{Entry, ZipError};

use crate::publication::{self, Publication, PublicationError, Package};

#[derive(Debug)]
pub enum BookError {
    Reading(PathBuf, std::io::Error),
    Zip(ZipError),
    Publication(PublicationError),
    NotFound(String),
    NoPictures,
}

impl std::fmt::Display for BookError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BookError::Reading(at, fault) => write!(to, "{}: {fault}", at.display()),
            BookError::Zip(fault) => write!(to, "{fault}"),
            BookError::Publication(fault) => write!(to, "{fault}"),
            BookError::NotFound(name) => write!(to, "this book has no {name} in it"),
            BookError::NoPictures => write!(to, "this comic has no pictures in it"),
        }
    }
}

impl std::error::Error for BookError {}

impl From<ZipError> for BookError {
    fn from(fault: ZipError) -> Self {
        BookError::Zip(fault)
    }
}

impl From<PublicationError> for BookError {
    fn from(fault: PublicationError) -> Self {
        BookError::Publication(fault)
    }
}

pub struct Archive {
    pub bytes: Vec<u8>,
    pub entries: Vec<Entry>,
}

impl Archive {
    pub fn read(at: &Path) -> Result<Archive, BookError> {
        let bytes = std::fs::read(at).map_err(|fault| BookError::Reading(at.to_path_buf(), fault))?;
        let entries = console_core_zip_files::entries(&bytes)?;

        Ok(Archive { bytes, entries })
    }

    pub fn extract(&self, name: &str) -> Result<Vec<u8>, BookError> {
        let Ok(found) = console_core_zip_files::find(&self.entries, name);

        match found {
            Some(entry) => {
                let bytes = console_core_zip_files::extract(&self.bytes, entry)?;

                Ok(bytes)
            },
            None => Err(BookError::NotFound(name.to_string())),
        }
    }

    pub fn text(&self, name: &str) -> Result<String, BookError> {
        let bytes = self.extract(name)?;

        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

pub struct OpenPublication {
    pub archive: Archive,
    pub publication: Publication,
}

pub fn publication(at: &Path) -> Result<OpenPublication, BookError> {
    let archive = Archive::read(at)?;
    let container = archive.text(publication::CONTAINER)?;
    let Ok(package_at) = publication::package_path(&container);

    let package_at = match package_at {
        Some(package_at) => package_at,
        None => return Err(BookError::Publication(PublicationError::NoContainer)),
    };

    let package = match archive.text(&package_at) {
        Ok(package) => package,
        Err(_missing) => return Err(BookError::Publication(PublicationError::NoPackage(package_at))),
    };

    let publication = publication::parse(Package { path: &package_at, contents: &package })?;

    Ok(OpenPublication { archive, publication })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picture {
    Yes,
    No,
}

pub const PICTURES: [&str; 6] = ["jpg", "jpeg", "png", "webp", "gif", "avif"];

pub fn is_a_picture(name: &str) -> Result<Picture, Never> {
    let ending = match name.rsplit_once('.') {
        Some((_, ending)) => ending.to_ascii_lowercase(),
        None => return Ok(Picture::No),
    };

    Ok(match PICTURES.contains(&ending.as_str()) {
        true => Picture::Yes,
        false => Picture::No,
    })
}

pub struct Comic {
    pub archive: Archive,
    pub pages: Vec<String>,
}

pub fn comic(at: &Path) -> Result<Comic, BookError> {
    let archive = Archive::read(at)?;
    let mut pages: Vec<String> = Vec::new();

    for entry in &archive.entries {
        let Ok(picture) = is_a_picture(&entry.name);

        match (picture, entry.name.contains("__MACOSX")) {
            (Picture::Yes, false) => pages.push(entry.name.clone()),
            (Picture::Yes, true) | (Picture::No, _) => {},
        }
    }

    pages.sort_by_key(|name| name.to_lowercase());

    match pages.is_empty() {
        true => Err(BookError::NoPictures),
        false => Ok(Comic { archive, pages }),
    }
}

pub fn ending(name: &str) -> Result<String, Never> {
    Ok(match name.rsplit_once('.') {
        Some((_, ending)) => ending.to_ascii_lowercase(),
        None => "picture".to_string(),
    })
}
