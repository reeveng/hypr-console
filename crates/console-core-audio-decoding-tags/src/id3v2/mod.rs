//! An ID3v2 reader for versions 2.2 to 2.4.
//!
//! The tag is read whole -- its size is in the header -- and every later step
//! is arithmetic over those bytes: undoing the unsynchronisation the header
//! asks for, then walking frames until padding or the end. An earlier shape
//! wrapped the stream in readers that decoded as they went, which needed a
//! `ReadBytes` that panicked in the three places nothing called, and had to
//! guess how much of the tag a decoded frame had used.

use console_core_audio_decoding_interfaces::errors::Error;
use console_core_audio_decoding_interfaces::meta::{
  MetadataBuilder, MetadataOptions, MetadataReader, MetadataRevision,
};
use console_core_audio_decoding_interfaces::probe::{Descriptor, Instantiate, QueryDescriptor};
use console_core_audio_decoding_interfaces::support_metadata;
use console_core_byte_streams::{BufReader, FiniteStream, MediaSourceStream, ReadBytes};
use console_core_logging::{info, warn};
use console_core_never::Never;

mod frames;
mod header;
mod kinds;
mod parsers;
mod text;
mod unsync;

use frames::{Frame, read_frame};
use header::{Extension, Header, Synchronisation, Version, read_header, skip_extended_header};
use unsync::decode_unsynchronisation;

pub mod util {
  use console_core_audio_decoding_interfaces::meta::StandardVisualKey;
  use console_core_never::Never;

  pub fn apic_picture_type_to_visual_key(apic: u32) -> Result<Option<StandardVisualKey>, Never> {
    Ok(match apic {
      0x01 => Some(StandardVisualKey::FileIcon),
      0x02 => Some(StandardVisualKey::OtherIcon),
      0x03 => Some(StandardVisualKey::FrontCover),
      0x04 => Some(StandardVisualKey::BackCover),
      0x05 => Some(StandardVisualKey::Leaflet),
      0x06 => Some(StandardVisualKey::Media),
      0x07 => Some(StandardVisualKey::LeadArtistPerformerSoloist),
      0x08 => Some(StandardVisualKey::ArtistPerformer),
      0x09 => Some(StandardVisualKey::Conductor),
      0x0a => Some(StandardVisualKey::BandOrchestra),
      0x0b => Some(StandardVisualKey::Composer),
      0x0c => Some(StandardVisualKey::Lyricist),
      0x0d => Some(StandardVisualKey::RecordingLocation),
      0x0e => Some(StandardVisualKey::RecordingSession),
      0x0f => Some(StandardVisualKey::Performance),
      0x10 => Some(StandardVisualKey::ScreenCapture),
      0x12 => Some(StandardVisualKey::Illustration),
      0x13 => Some(StandardVisualKey::BandArtistLogo),
      0x14 => Some(StandardVisualKey::PublisherStudioLogo),
      _ => None,
    })
  }
}

enum Walk {
  OnToTheNext,
  Finished,
}

fn add_frame(frame: Frame, metadata: &mut MetadataBuilder) -> Result<Walk, Never> {
  match frame {
    Frame::Padding => return Ok(Walk::Finished),
    Frame::Tag(tag) => {
      let Ok(_configured_add_tag) = metadata.add_tag(tag);
    }
    Frame::Tags(tags) => {
      for tag in tags {
        let Ok(_configured_add_tag) = metadata.add_tag(tag);
      }
    }
    Frame::Visual(visual) => {
      let Ok(_configured_add_visual) = metadata.add_visual(visual);
    }
    Frame::Unsupported(id) => info!("unsupported frame {id}"),
    Frame::Invalid(id) => warn!("invalid data for {id} frame"),
  }

  Ok(Walk::OnToTheNext)
}

fn read_body(
  reader: &mut BufReader<'_>,
  header: &Header,
  metadata: &mut MetadataBuilder,
) -> Result<(), Error> {
  match header.extension {
    Extension::Present => skip_extended_header(reader, header.version)?,
    Extension::Absent => {}
  }

  let minimum_frame: u64 = match header.version {
    Version::V2 => 6,
    Version::V3 | Version::V4 => 10,
  };
  let frame_limit = reader.byte_len().wrapping_div(minimum_frame.max(1));

  for _ in 0..frame_limit.saturating_add(1) {
    let frame = read_frame(reader, header.version)?;

    let Ok(walk) = add_frame(frame, metadata);

    match walk {
      Walk::Finished => break,
      Walk::OnToTheNext => {}
    }

    match reader.bytes_available().cmp(&minimum_frame) {
      std::cmp::Ordering::Less => break,
      std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => {}
    }
  }

  Ok(())
}

fn read_id3v2<B: ReadBytes>(reader: &mut B, metadata: &mut MetadataBuilder) -> Result<(), Error> {
  let header = read_header(reader)?;
  let raw = reader.read_boxed_slice_exact(u64::from(header.size))?;

  match (header.version, header.synchronisation) {
    (Version::V2 | Version::V3, Synchronisation::Unsynchronised) => {
      let Ok(decoded) = decode_unsynchronisation(&raw);
      let Ok(mut body) = BufReader::new(&decoded);

      read_body(&mut body, &header, metadata)
    }
    (Version::V4, Synchronisation::Plain | Synchronisation::Unsynchronised)
    | (Version::V2 | Version::V3, Synchronisation::Plain) => {
      let Ok(mut body) = BufReader::new(&raw);

      read_body(&mut body, &header, metadata)
    }
  }
}

pub struct Id3v2Reader;

impl QueryDescriptor for Id3v2Reader {
  fn query() -> std::result::Result<&'static [Descriptor], console_core_never::Never> {
    Ok(&[support_metadata!("id3v2", "ID3v2", &[], &[], &[b"ID3"])])
  }

  fn score(_context: &[u8]) -> std::result::Result<u8, console_core_never::Never> {
    Ok(255)
  }
}

impl MetadataReader for Id3v2Reader {
  fn new(_options: &MetadataOptions) -> std::result::Result<Self, console_core_never::Never> {
    Ok(Id3v2Reader {})
  }

  fn read_all(&mut self, reader: &mut MediaSourceStream) -> Result<MetadataRevision, Error> {
    let Ok(mut builder) = MetadataBuilder::new();

    read_id3v2(reader, &mut builder)?;

    let Ok(value_1) = builder.metadata();

    Ok(value_1)
  }
}
