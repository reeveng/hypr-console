//! What is inside a zip file, and the bytes of one thing in it.
//!
//! A book arrives as a zip and so does a comic: an EPUB is pages of XHTML in
//! one, a CBZ is pictures in one. The files panel unpacks an archive by
//! handing it to `7z`, which is right for a folder someone asked to have on
//! the disk, and wrong for a page someone turned: a program started per page,
//! and a folder of leavings for every book opened, to read forty kilobytes the
//! reader already holds.
//!
//! So the format is read here, and only as far as a reader needs. The central
//! directory at the end says what is in the file and where; a local header in
//! front of each thing says how far past it the bytes begin; and the bytes are
//! stored or deflated, which [`inflate`] undoes. What the format also allows --
//! the other methods, a password, the sixty-four bit sizes of an archive past
//! four gigabytes -- is refused by name rather than read wrong, because a book
//! has never been written that way and a fault that says what it met is one a
//! person can act on.
//!
//! What comes out is checked against the CRC32 the directory carries for it,
//! and against the size it says, before anybody is handed it. A book arrives
//! off the network, and a download cut short or a bad sector inflates to the
//! wrong bytes as readily as to the right ones -- and a stored entry cannot
//! fail to unpack at all. Without the check a damaged book is a page of
//! garbage, and with it the page says the file is damaged.
//!
//! Nothing here opens a file. It is handed the bytes and answers about them,
//! which is what lets a test hold a whole archive in a constant.

pub mod inflate;

use console_core_never::Never;
use console_core_number_conversion::index;

const END_OF_CENTRAL_DIRECTORY: u32 = 0x0605_4b50;

const CENTRAL_DIRECTORY_ENTRY: u32 = 0x0201_4b50;

const LOCAL_FILE_HEADER: u32 = 0x0403_4b50;

const END_OF_CENTRAL_DIRECTORY_LENGTH: u32 = 22;

const LONGEST_COMMENT: u32 = 0xffff;

const WIDE: u32 = 0xffff_ffff;

const ENCRYPTED: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipError {
    NotAZip,
    Truncated,
    Corrupt,
    LargerThanDeclared,
    UnsupportedMethod(u16),
    Encrypted,
    TooLarge,
}

impl std::fmt::Display for ZipError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZipError::NotAZip => write!(to, "this is not a zip file"),
            ZipError::Truncated => write!(to, "the file ends before what it says is in it"),
            ZipError::Corrupt => write!(to, "what is packed in here is damaged"),
            ZipError::LargerThanDeclared => write!(to, "this unpacks to more than it says it holds"),
            ZipError::UnsupportedMethod(method) => write!(to, "this is packed a way nothing here reads ({method})"),
            ZipError::Encrypted => write!(to, "this is locked with a password"),
            ZipError::TooLarge => write!(to, "this is larger than four gigabytes"),
        }
    }
}

impl std::error::Error for ZipError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Method {
    Stored,
    Deflated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub size: u32,
    method: Method,
    crc: u32,
    packed: u32,
    header: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    at: u32,
    long: u32,
}

fn slice(bytes: &[u8], span: Span) -> Result<&[u8], ZipError> {
    let Span { at, long } = span;
    let Ok(from) = index(at);
    let Ok(past) = index(at.saturating_add(long));

    match bytes.get(from..past) {
        Some(taken) => Ok(taken),
        None => Err(ZipError::Truncated),
    }
}

fn read_u16(bytes: &[u8], at: u32) -> Result<u16, ZipError> {
    let taken = slice(bytes, Span { at, long: 2 })?;

    match <[u8; 2]>::try_from(taken) {
        Ok(taken) => Ok(u16::from_le_bytes(taken)),
        Err(_fault) => Err(ZipError::Truncated),
    }
}

fn read_u32(bytes: &[u8], at: u32) -> Result<u32, ZipError> {
    let taken = slice(bytes, Span { at, long: 4 })?;

    match <[u8; 4]>::try_from(taken) {
        Ok(taken) => Ok(u32::from_le_bytes(taken)),
        Err(_fault) => Err(ZipError::Truncated),
    }
}

fn length_of(bytes: &[u8]) -> Result<u32, ZipError> {
    match u32::try_from(bytes.len()) {
        Ok(long) => Ok(long),
        Err(_fault) => Err(ZipError::TooLarge),
    }
}

fn end_of_central_directory(bytes: &[u8]) -> Result<u32, ZipError> {
    let long = length_of(bytes)?;

    let last = match long.checked_sub(END_OF_CENTRAL_DIRECTORY_LENGTH) {
        Some(last) => last,
        None => return Err(ZipError::NotAZip),
    };

    let furthest = last.saturating_sub(LONGEST_COMMENT);
    let mut at = last;

    loop {
        let signature = read_u32(bytes, at)?;

        match (signature == END_OF_CENTRAL_DIRECTORY, at > furthest) {
            (true, _) => return Ok(at),
            (false, true) => at = at.saturating_sub(1),
            (false, false) => return Err(ZipError::NotAZip),
        }
    }
}

fn method_of(method: u16) -> Result<Method, ZipError> {
    match method {
        0 => Ok(Method::Stored),
        8 => Ok(Method::Deflated),
        other => Err(ZipError::UnsupportedMethod(other)),
    }
}

fn central_directory_entry(bytes: &[u8], at: u32) -> Result<(Entry, u32), ZipError> {
    let signature = read_u32(bytes, at)?;

    match signature == CENTRAL_DIRECTORY_ENTRY {
        true => {},
        false => return Err(ZipError::Corrupt),
    }

    let flags = read_u16(bytes, at.saturating_add(8))?;
    let method = read_u16(bytes, at.saturating_add(10))?;
    let crc = read_u32(bytes, at.saturating_add(16))?;
    let packed = read_u32(bytes, at.saturating_add(20))?;
    let size = read_u32(bytes, at.saturating_add(24))?;
    let named = read_u16(bytes, at.saturating_add(28))?;
    let extra = read_u16(bytes, at.saturating_add(30))?;
    let comment = read_u16(bytes, at.saturating_add(32))?;
    let header = read_u32(bytes, at.saturating_add(42))?;
    let name = slice(bytes, Span { at: at.saturating_add(46), long: u32::from(named) })?;

    match (flags & ENCRYPTED, packed == WIDE || size == WIDE || header == WIDE) {
        (0, false) => {},
        (0, true) => return Err(ZipError::TooLarge),
        (_, _) => return Err(ZipError::Encrypted),
    }

    let method = method_of(method)?;
    let past = at
        .saturating_add(46)
        .saturating_add(u32::from(named))
        .saturating_add(u32::from(extra))
        .saturating_add(u32::from(comment));

    let name = String::from_utf8_lossy(name).into_owned();

    Ok((Entry { name, size, method, crc, packed, header }, past))
}

pub fn entries(bytes: &[u8]) -> Result<Vec<Entry>, ZipError> {
    let end = end_of_central_directory(bytes)?;
    let many = read_u16(bytes, end.saturating_add(10))?;
    let mut at = read_u32(bytes, end.saturating_add(16))?;
    let mut found = Vec::new();

    for _ in 0..many {
        let (entry, past) = central_directory_entry(bytes, at)?;

        found.push(entry);
        at = past;
    }

    Ok(found)
}

pub fn extract(bytes: &[u8], entry: &Entry) -> Result<Vec<u8>, ZipError> {
    let signature = read_u32(bytes, entry.header)?;

    match signature == LOCAL_FILE_HEADER {
        true => {},
        false => return Err(ZipError::Corrupt),
    }

    let named = read_u16(bytes, entry.header.saturating_add(26))?;
    let extra = read_u16(bytes, entry.header.saturating_add(28))?;
    let from = entry
        .header
        .saturating_add(30)
        .saturating_add(u32::from(named))
        .saturating_add(u32::from(extra));
    let packed = slice(bytes, Span { at: from, long: entry.packed })?;

    let unpacked = match entry.method {
        Method::Stored => packed.to_vec(),
        Method::Deflated => inflate::inflate(packed, entry.size)?,
    };

    let long = length_of(&unpacked)?;
    let Ok(crc) = console_core_checksums::crc32::of(&unpacked);

    match (long == entry.size, crc == entry.crc) {
        (true, true) => Ok(unpacked),
        (_, _) => Err(ZipError::Corrupt),
    }
}

pub fn find<'a>(entries: &'a [Entry], name: &str) -> Result<Option<&'a Entry>, Never> {
    Ok(entries.iter().find(|entry| entry.name == name))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOOK: &[u8] = include_bytes!("../tests/two-things.zip");

    #[test]
    fn what_is_in_an_archive_is_listed_by_name_and_size() {
        let found = entries(BOOK).map_err(|fault| fault.to_string());

        let names: Option<Vec<(String, u32)>> = found
            .ok()
            .map(|found| found.into_iter().map(|entry| (entry.name, entry.size)).collect());

        assert_eq!(
            names,
            Some(vec![("mimetype".to_string(), 20), ("chapter.xhtml".to_string(), 1638)])
        );
    }

    #[test]
    fn a_stored_thing_and_a_deflated_thing_both_come_back_whole() {
        let found = entries(BOOK).ok().unwrap_or_default();
        let Ok(stored) = find(&found, "mimetype");
        let Ok(deflated) = find(&found, "chapter.xhtml");

        let stored = stored.map(|entry| extract(BOOK, entry));
        let deflated = deflated.map(|entry| extract(BOOK, entry));

        assert_eq!(stored, Some(Ok(b"application/epub+zip".to_vec())));

        let expected = "<p>It was a dark and stormy night.</p>\n".repeat(42);

        assert_eq!(deflated, Some(Ok(expected.into_bytes())));
    }

    fn damaged(said: &[u8]) -> Vec<u8> {
        let flipped: Option<Vec<u8>> = BOOK.windows(said.len()).position(|held| held == said).map(|found| {
            BOOK.iter()
                .enumerate()
                .map(|(at, byte)| match at == found {
                    true => byte ^ 1,
                    false => *byte,
                })
                .collect()
        });

        flipped.expect("the fixture reads what is to be damaged")
    }

    fn extracted(bytes: &[u8], name: &str) -> Option<Result<Vec<u8>, ZipError>> {
        let found = entries(bytes).ok()?;
        let Ok(entry) = find(&found, name);

        entry.map(|entry| extract(bytes, entry))
    }

    #[test]
    fn a_stored_thing_with_a_byte_changed_is_refused_rather_than_read() {
        let book = damaged(b"application/epub+zip");

        assert_eq!(extracted(&book, "mimetype"), Some(Err(ZipError::Corrupt)));
    }

    #[test]
    fn something_that_is_not_an_archive_says_so() {
        assert_eq!(entries(b"PK but not really").err(), Some(ZipError::NotAZip));
    }
}
