//! Read EPUB publication metadata and access resources from an archive in memory.
//!
//! [`Epub::read`] loads the container and package documents. [`Epub::extract`]
//! unpacks a resource on demand by archive path, and [`Epub::into_parts`]
//! transfers ownership of the publication and ZIP directory.
//! Malformed UTF-8 sequences in documents and percent-decoded links become
//! the Unicode replacement character.

use console_core_never::Never;
use console_core_zip_files::{self as zip, Entry, ZipError};
use std::borrow::Cow;

mod paths;
pub mod publication;

pub use paths::{Relative, resolve};
use publication::{Package, Publication, PublicationError};

#[derive(Debug)]
pub enum ReadError {
  Zip(ZipError),
  MissingContainer,
  Publication(PublicationError),
}

impl core::fmt::Display for ReadError {
  fn fmt(&self, to: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Zip(fault) => fault.fmt(to),
      Self::MissingContainer => write!(to, "this book has no {} in it", publication::CONTAINER),
      Self::Publication(fault) => fault.fmt(to),
    }
  }
}

impl core::error::Error for ReadError {
  fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
    match self {
      Self::Zip(fault) => Some(fault),
      Self::Publication(fault) => Some(fault),
      Self::MissingContainer => None,
    }
  }
}

pub struct Epub<'a> {
  bytes: &'a [u8],
  entries: Vec<Entry>,
  pub publication: Publication,
}

impl<'a> Epub<'a> {
  pub fn read(bytes: &'a [u8]) -> Result<Self, ReadError> {
    let entries = zip::entries(bytes).map_err(ReadError::Zip)?;
    let container_entry =
      extract(bytes, &entries, publication::CONTAINER).map_err(ReadError::Zip)?;
    let container = container_entry.ok_or(ReadError::MissingContainer)?;
    let Ok(container_text) = utf8_with_replacement(&container);
    let Ok(package_path) = publication::package_path(&container_text);
    let path = package_path.ok_or(ReadError::Publication(PublicationError::NoContainer))?;
    let package = match extract(bytes, &entries, &path) {
      Ok(Some(package)) => package,
      Ok(None) | Err(_) => return Err(ReadError::Publication(PublicationError::NoPackage(path))),
    };
    let Ok(package_text) = utf8_with_replacement(&package);
    let publication = publication::parse(Package {
      path: &path,
      contents: &package_text,
    })
    .map_err(ReadError::Publication)?;

    Ok(Self {
      bytes,
      entries,
      publication,
    })
  }

  pub fn extract(&self, path: &str) -> Result<Option<Vec<u8>>, ZipError> {
    extract(self.bytes, &self.entries, path)
  }

  #[must_use]
  pub fn into_parts(self) -> Result<(Publication, Vec<Entry>), Never> {
    Ok((self.publication, self.entries))
  }
}

pub fn utf8_with_replacement(bytes: &[u8]) -> Result<Cow<'_, str>, Never> {
  Ok(match std::str::from_utf8(bytes) {
    Ok(text) => Cow::Borrowed(text),
    Err(_malformed_utf8) => {
      let mut text = String::with_capacity(bytes.len());

      for chunk in bytes.utf8_chunks() {
        text.push_str(chunk.valid());

        match chunk.invalid() {
          [] => {}
          [_, ..] => text.push(char::REPLACEMENT_CHARACTER),
        }
      }

      Cow::Owned(text)
    }
  })
}

fn extract(bytes: &[u8], entries: &[Entry], path: &str) -> Result<Option<Vec<u8>>, ZipError> {
  let Ok(entry) = zip::find(entries, path);

  entry
    .map(|entry_2| zip::extract(bytes, entry_2))
    .transpose()
}
