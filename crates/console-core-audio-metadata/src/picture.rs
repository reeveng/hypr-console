//! A picture a tag carries: what it shows, the format its bytes are in, and
//! the bytes.
//!
//! The kinds are the twenty-one ID3v2 numbers, which FLAC and Vorbis comments
//! borrowed, and a number past them is kept as itself rather than turned into
//! one of them. A format is read off the MIME type as lofty read it, so the
//! five image formats a tag usually holds have names and anything else keeps
//! the words it was written with. A FLAC picture block -- which a Vorbis
//! comment also carries, written out in base64 -- is read here because both
//! of them hold one.

use console_core_never::Never;

use crate::bytes::Cursor;
use crate::fault::Malformed;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PictureType {
  Other,
  Icon,
  OtherIcon,
  CoverFront,
  CoverBack,
  Leaflet,
  Media,
  LeadArtist,
  Artist,
  Conductor,
  Band,
  Composer,
  Lyricist,
  RecordingLocation,
  DuringRecording,
  DuringPerformance,
  ScreenCapture,
  BrightFish,
  Illustration,
  BandLogo,
  PublisherLogo,
  Undefined(u8),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MimeType {
  Png,
  Jpeg,
  Tiff,
  Bmp,
  Gif,
  Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Picture {
  pub kind: PictureType,
  pub mime_type: Option<MimeType>,
  pub data: Vec<u8>,
}

impl PictureType {
  pub fn from_u8(number: u8) -> Result<PictureType, Never> {
    Ok(match number {
      0 => PictureType::Other,
      1 => PictureType::Icon,
      2 => PictureType::OtherIcon,
      3 => PictureType::CoverFront,
      4 => PictureType::CoverBack,
      5 => PictureType::Leaflet,
      6 => PictureType::Media,
      7 => PictureType::LeadArtist,
      8 => PictureType::Artist,
      9 => PictureType::Conductor,
      10 => PictureType::Band,
      11 => PictureType::Composer,
      12 => PictureType::Lyricist,
      13 => PictureType::RecordingLocation,
      14 => PictureType::DuringRecording,
      15 => PictureType::DuringPerformance,
      16 => PictureType::ScreenCapture,
      17 => PictureType::BrightFish,
      18 => PictureType::Illustration,
      19 => PictureType::BandLogo,
      20 => PictureType::PublisherLogo,
      undefined => PictureType::Undefined(undefined),
    })
  }

  pub fn as_u8(self) -> Result<u8, Never> {
    Ok(match self {
      PictureType::Other => 0,
      PictureType::Icon => 1,
      PictureType::OtherIcon => 2,
      PictureType::CoverFront => 3,
      PictureType::CoverBack => 4,
      PictureType::Leaflet => 5,
      PictureType::Media => 6,
      PictureType::LeadArtist => 7,
      PictureType::Artist => 8,
      PictureType::Conductor => 9,
      PictureType::Band => 10,
      PictureType::Composer => 11,
      PictureType::Lyricist => 12,
      PictureType::RecordingLocation => 13,
      PictureType::DuringRecording => 14,
      PictureType::DuringPerformance => 15,
      PictureType::ScreenCapture => 16,
      PictureType::BrightFish => 17,
      PictureType::Illustration => 18,
      PictureType::BandLogo => 19,
      PictureType::PublisherLogo => 20,
      PictureType::Undefined(number) => number,
    })
  }
}

impl MimeType {
  pub fn from_text(text: &str) -> Result<MimeType, Never> {
    Ok(match text.to_lowercase().as_str() {
      "image/jpeg" | "image/jpg" => MimeType::Jpeg,
      "image/png" => MimeType::Png,
      "image/tiff" => MimeType::Tiff,
      "image/bmp" => MimeType::Bmp,
      "image/gif" => MimeType::Gif,
      _named => MimeType::Unknown(text.to_string()),
    })
  }

  pub fn as_text(&self) -> Result<&str, Never> {
    Ok(match self {
      MimeType::Jpeg => "image/jpeg",
      MimeType::Png => "image/png",
      MimeType::Tiff => "image/tiff",
      MimeType::Bmp => "image/bmp",
      MimeType::Gif => "image/gif",
      MimeType::Unknown(text) => text,
    })
  }

  pub(crate) fn sniffed(data: &[u8]) -> Result<MimeType, Never> {
    Ok(match data {
      [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => MimeType::Png,
      [0xFF, 0xD8, ..] => MimeType::Jpeg,
      [b'G', b'I', b'F', 0x38, 0x37 | 0x39, 0x61, ..] => MimeType::Gif,
      [b'B', b'M', ..] => MimeType::Bmp,
      [b'I', b'I', b'*', 0x00, ..] | [b'M', b'M', 0x00, b'*', ..] => MimeType::Tiff,
      _unknown => MimeType::Unknown(String::from("image/")),
    })
  }
}

pub(crate) fn from_flac_block(content: &[u8]) -> Result<Picture, Malformed> {
  let Ok(mut reader) = Cursor::new(content);
  let Ok(size) = reader.left();

  match size {
    0..=31 => return Err(Malformed::Picture),
    _has_header => {}
  }

  let kind = reader.u32_be()?;
  let mime_length = reader.u32_be()?;
  let mime_bytes = reader.take(u64::from(mime_length))?;

  let mime = match std::str::from_utf8(mime_bytes) {
    Ok(mime) => mime.trim_end_matches('\0'),
    Err(_not_utf8) => return Err(Malformed::Text),
  };

  let value_1 = reader.u32_be()?;

  let description_length = u64::from(value_1);
  let Ok(left) = reader.left();

  match (description_length, description_length.cmp(&left)) {
    (1.., std::cmp::Ordering::Less) => {
      reader.skip(description_length)?;
    }
    (0, _) | (_, std::cmp::Ordering::Equal | std::cmp::Ordering::Greater) => {}
  }

  reader.skip(16)?;

  let data_length = reader.u32_be()?;
  let data = reader.take(u64::from(data_length))?;
  let [.., number] = kind.to_be_bytes();
  let Ok(kind_2) = PictureType::from_u8(number);

  let mime_type = match mime {
    "" => None,
    media_type => {
      let Ok(mime_type) = MimeType::from_text(media_type);

      Some(mime_type)
    }
  };

  Ok(Picture {
    kind: kind_2,
    mime_type,
    data: data.to_vec(),
  })
}
