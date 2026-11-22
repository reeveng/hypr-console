//! The tags, the pictures and the length kept inside an audio file, read from
//! a reader the caller opened, and an ID3v2 tag written into a WAVE file's
//! bytes.
//!
//! The crate began as lofty, by Serial under MIT or Apache-2.0, carried in
//! `forks/`. The music player is what asks: what a song is called, who made
//! it, how long it is and the picture on its cover. So what is kept is what a
//! reading of those needs, format by format, with lofty's answers -- which
//! tag wins, which field means what, how long a stream with no length in it
//! is -- held by the goldens in `tests/fixtures` rather than re-argued. What
//! nothing asked was left behind: the format-specific tag types and their
//! accessors, every writer but the one, AIFF, APE files, WavPack, Musepack
//! and Speex, the properties beyond the length, and the options that turned
//! parts of a read off.
//!
//! The extension decides the format, as it did for the player through lofty,
//! and the caller opens the file and hands in what reads it, so a crate with
//! no machine in it never names one. A file is read in the place its format
//! keeps each part: the tags where they are written and the audio frames
//! only as far as a length needs.
//!
//! Writing is bytes in and bytes out, so the caller decides how the file is
//! replaced and can do it whole.

mod aac;
mod ape;
mod bytes;
mod duration;
mod fault;
mod flac;
mod id3v1;
mod id3v2;
mod keys;
mod mp4;
mod mpeg;
mod ogg;
mod picture;
mod tag;
mod text;
mod vorbis_comments;
mod wave;

use std::io::{Read, Seek};
use std::path::Path;
use std::time::Duration;

use console_core_never::Never;

pub use fault::{FileEncodingError, FileParseError, Malformed};
pub use keys::ItemKey;
pub use picture::{MimeType, Picture, PictureType};
pub use tag::{Tag, TagItem, TagType};
pub use wave::tagged_wave;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
  Aac,
  Flac,
  Mp4,
  Mpeg,
  Opus,
  Vorbis,
  Wav,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedFile {
  pub file_type: FileType,
  pub duration: Duration,
  pub tags: Vec<Tag>,
}

pub(crate) struct Contents {
  pub(crate) duration: Duration,
  pub(crate) tags: Vec<Tag>,
}

impl FileType {
  pub fn named(path: &Path) -> Result<Option<FileType>, Never> {
    let extension = match path.extension().map(std::ffi::OsStr::to_str) {
      Some(Some(extension)) => extension.to_ascii_lowercase(),
      Some(None) | None => return Ok(None),
    };

    Ok(match extension.as_str() {
      "aac" => Some(FileType::Aac),
      "flac" => Some(FileType::Flac),
      "mp4" | "m4a" | "m4b" | "m4p" | "m4r" | "m4v" | "3gp" => Some(FileType::Mp4),
      "mp3" | "mp2" | "mp1" => Some(FileType::Mpeg),
      "opus" => Some(FileType::Opus),
      "ogg" => Some(FileType::Vorbis),
      "wav" | "wave" => Some(FileType::Wav),
      _unknown => None,
    })
  }

  pub fn primary_tag_type(self) -> Result<TagType, Never> {
    Ok(match self {
      FileType::Aac | FileType::Mpeg | FileType::Wav => TagType::Id3v2,
      FileType::Flac | FileType::Opus | FileType::Vorbis => TagType::VorbisComments,
      FileType::Mp4 => TagType::Mp4Ilst,
    })
  }
}

impl TaggedFile {
  pub fn primary_tag(&self) -> Result<Option<&Tag>, Never> {
    let Ok(primary) = self.file_type.primary_tag_type();

    Ok(self.tags.iter().find(|tag| tag.tag_type == primary))
  }
}

pub fn read<R: Read + Seek>(reader: &mut R, named: &Path) -> Result<TaggedFile, FileParseError> {
  let Ok(format) = FileType::named(named);

  let file_type = match format {
    Some(file_type) => file_type,
    None => return Err(FileParseError::UnknownFormat),
  };

  let read = match file_type {
    FileType::Aac => aac::read(reader),
    FileType::Flac => flac::read(reader),
    FileType::Mp4 => mp4::read(reader),
    FileType::Mpeg => mpeg::read(reader),
    FileType::Opus => ogg::read_opus(reader),
    FileType::Vorbis => ogg::read_vorbis(reader),
    FileType::Wav => wave::read(reader),
  };

  match read {
    Ok(Contents { duration, tags }) => Ok(TaggedFile {
      file_type,
      duration,
      tags,
    }),
    Err(fault) => {
      let Ok(fault_2) = fault.of(file_type);

      Err(fault_2)
    }
  }
}
