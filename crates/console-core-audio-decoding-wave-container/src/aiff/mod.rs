//! An AIFF or AIFC file: a FORM stream whose `COMM` chunk says what the sound
//! is and whose `SSND` chunk holds it.
//!
//! Reading stops at the `SSND` chunk. The form is not checked when the stream
//! opens but when `COMM` is met, because a form this reader does not know
//! only matters to a file that has something to be read in it. A sound chunk
//! that is block-aligned is refused: offset and block size are always zero in
//! what is written today.

use console_core_audio_decoding_interfaces::codecs::CodecParameters;
use console_core_audio_decoding_interfaces::errors::{Result as Fallible, unsupported_error};
use console_core_audio_decoding_interfaces::formats::prelude::*;
use console_core_audio_decoding_interfaces::meta::{Metadata, MetadataLog};
use console_core_audio_decoding_interfaces::probe::{Descriptor, Instantiate, QueryDescriptor};
use console_core_audio_decoding_interfaces::support_format;
use console_core_byte_streams::{MediaSourceStream, ReadBytes};
use console_core_never::Never;

use crate::common::{ByteOrder, Chunk, Chunks, PacketInfo, Stream, append_format_params};

mod format;

use format::{Common, read_aifc, read_aiff};

const AIFF_STREAM_MARKER: [u8; 4] = *b"FORM";
const AIFF_RIFF_FORM: [u8; 4] = *b"AIFF";
const AIFC_RIFF_FORM: [u8; 4] = *b"AIFC";

enum AiffChunk {
  Common,
  Sound,
}

fn recognize(tag: [u8; 4]) -> Result<Option<AiffChunk>, Never> {
  Ok(match &tag {
    b"COMM" => Some(AiffChunk::Common),
    b"SSND" => Some(AiffChunk::Sound),
    _ => None,
  })
}

fn read_common(source: &mut MediaSourceStream, form: [u8; 4]) -> Fallible<Common> {
  match form {
    AIFF_RIFF_FORM => read_aiff(source),
    AIFC_RIFF_FORM => read_aifc(source),
    _ => unsupported_error("aiff: riff form is not supported"),
  }
}

fn check_sound(source: &mut MediaSourceStream) -> Fallible<()> {
  let offset = source.read_be_u32()?;
  let block_size = source.read_be_u32()?;

  match (offset, block_size) {
    (0, 0) => Ok(()),
    _ => unsupported_error("riff: No support for AIFF block-aligned data"),
  }
}

pub struct AiffReader {
  stream: Stream,
}

impl QueryDescriptor for AiffReader {
  fn query() -> std::result::Result<&'static [Descriptor], console_core_never::Never> {
    Ok(&[support_format!(
      "riff",
      " Resource Interchange File Format",
      &["aiff", "aif", "aifc"],
      &[
        "audio/aiff",
        "audio/x-aiff",
        " sound/aiff",
        "audio/x-pn-aiff"
      ],
      &[b"FORM"]
    )])
  }

  fn score(_context: &[u8]) -> std::result::Result<u8, console_core_never::Never> {
    Ok(255)
  }
}

impl FormatReader for AiffReader {
  fn try_new(mut source: MediaSourceStream, _options: &FormatOptions) -> Fallible<Self> {
    let marker = source.read_quad_bytes()?;

    match marker {
      AIFF_STREAM_MARKER => (),
      _ => return unsupported_error("aiff: missing riff stream marker"),
    }

    let riff_len = source.read_be_u32()?;
    let form = source.read_quad_bytes()?;
    let Ok(mut chunks) = Chunks::new(riff_len, ByteOrder::Big);
    let Ok(mut codec_params) = CodecParameters::new();
    let Ok(mut packet_info) = PacketInfo::without_blocks(0);

    for _ in 0..=riff_len {
      let next = chunks.next(&mut source, recognize)?;

      match next {
        None => return unsupported_error("aiff: missing sound chunk"),
        Some(Chunk {
          kind: AiffChunk::Common,
          ..
        }) => {
          let common = read_common(&mut source, form)?;

          packet_info = common.packet_info;

          append_format_params(
            &mut codec_params,
            packet_info,
            &common.encoding,
            common.sample_rate,
          )?;
        }
        Some(Chunk {
          kind: AiffChunk::Sound,
          len,
        }) => {
          check_sound(&mut source)?;

          let Ok(stream) = Stream::new(
            source,
            codec_params,
            MetadataLog::default(),
            packet_info,
            len,
          );

          return Ok(AiffReader { stream });
        }
      }
    }

    unsupported_error("aiff: missing sound chunk")
  }

  fn next_packet(&mut self) -> Fallible<Packet> {
    self.stream.next_packet()
  }

  fn metadata(&mut self) -> std::result::Result<Metadata<'_>, console_core_never::Never> {
    let Ok(value_1) = self.stream.metadata.metadata();

    Ok(value_1)
  }

  fn cues(&self) -> std::result::Result<&[Cue], console_core_never::Never> {
    Ok(&[])
  }

  fn tracks(&self) -> std::result::Result<&[Track], console_core_never::Never> {
    Ok(std::slice::from_ref(&self.stream.track))
  }

  fn seek(&mut self, _mode: SeekMode, to: SeekTo) -> Fallible<SeekedTo> {
    self.stream.seek(to)
  }

  fn into_inner(
    self: Box<Self>,
  ) -> std::result::Result<MediaSourceStream, console_core_never::Never> {
    Ok(self.stream.reader)
  }
}
