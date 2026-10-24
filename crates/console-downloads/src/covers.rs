//! A result's picture, fetched to a file beside where it will be kept.
//!
//! What a site sends back with a refusal is a page about the refusal, and
//! sometimes a picture of one: a placeholder a CDN serves with its 404. Kept,
//! that is a cover nobody chose, drawn beside a row as though it were the
//! thing found. So a status curl calls a failure is no picture at all, which is
//! `--fail`, and the body that came with it is never written.
//!
//! What arrives is then drawn down to the size a row shows it at and kept as a
//! JPEG, by the same readers every other picture on this desktop goes through
//! and the encoder the viewer writes its copies with. That used to be ffmpeg,
//! started once per result for a picture a hundred and twenty-eight pixels
//! wide.

use std::path::Path;

use console_core_external_programs::Program;
use console_core_geometry::Size;
use console_core_jpeg_files::{JpegError, Quality};
use console_core_never::Never;
use console_pictures::PictureError;

pub const SIDE: u32 = 128;

const QUALITY: u8 = 85;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fetched {
    Arrived,
    Failed,
}

pub fn fetched(from: &str, into: &Path) -> Result<Fetched, Never> {
    let Ok(mut curl) = Program::Curl.command();

    let done = curl
        .args(["--silent", "--fail", "--location", "--max-time", "20", "--output"])
        .arg(into)
        .arg(from)
        .status();

    Ok(match done.is_ok_and(|how| how.success()) {
        true => Fetched::Arrived,
        false => {
            let _ = std::fs::remove_file(into);

            Fetched::Failed
        },
    })
}

#[derive(Debug)]
pub enum Unshrunk {
    Unread(PictureError),
    NotAPicture,
    Unwritable(JpegError),
    Unwritten(console_core_atomic_writes::Unwritten),
}

impl std::fmt::Display for Unshrunk {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unshrunk::Unread(fault) => write!(formatter, "reading the picture: {fault}"),
            Unshrunk::NotAPicture => write!(formatter, "it is not a picture this desktop reads"),
            Unshrunk::Unwritable(fault) => write!(formatter, "making it a JPEG: {fault}"),
            Unshrunk::Unwritten(fault) => write!(formatter, "writing it down: {fault}"),
        }
    }
}

impl std::error::Error for Unshrunk {}

pub fn shrunk(from: &Path, to: &Path) -> Result<(), Unshrunk> {
    let read = console_pictures::decoded(from, Size { width: SIDE, height: SIDE }).map_err(Unshrunk::Unread)?;
    let pixels = read.ok_or(Unshrunk::NotAPicture)?;
    let size = Size { width: pixels.width, height: pixels.height };
    let picture = console_core_jpeg_files::Picture { size, rgba: pixels.bytes.to_vec() };
    let Ok(quality) = Quality::percent(QUALITY);
    let jpeg = console_core_jpeg_files::encoded(&picture, quality).map_err(Unshrunk::Unwritable)?;

    console_core_atomic_writes::whole(to, &jpeg).map_err(Unshrunk::Unwritten)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_picture_is_kept_as_a_jpeg_no_wider_than_a_row_and_as_wide_as_it_was() -> Result<(), Box<dyn std::error::Error>> {
        let folder = console_core_temporary_directories::fresh("downloads-covers")?;
        let from = folder.join("cover.part");
        let to = folder.join("cover.jpg");
        let size = Size { width: 300, height: 200 };
        let rgba = vec![200_u8; 240_000];
        let png = console_core_png_files::encoded(&console_core_png_files::Picture { size, rgba })?;

        console_core_atomic_writes::whole(&from, &png)?;
        shrunk(&from, &to)?;

        let jpeg = std::fs::read(&to)?;
        let measured = console_core_jpeg_files::measured(&jpeg)?;

        assert_eq!(measured, Size { width: 128, height: 85 });

        Ok(())
    }

    #[test]
    fn what_is_not_a_picture_is_said_so_and_nothing_is_written() -> Result<(), Box<dyn std::error::Error>> {
        let folder = console_core_temporary_directories::fresh("downloads-covers-not")?;
        let from = folder.join("cover.part");
        let to = folder.join("cover.jpg");

        console_core_atomic_writes::whole(&from, b"<html>not found</html>")?;

        assert!(matches!(shrunk(&from, &to), Err(Unshrunk::NotAPicture)));
        assert!(!to.exists());

        Ok(())
    }
}
