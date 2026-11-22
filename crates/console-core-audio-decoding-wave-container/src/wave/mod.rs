//! A WAVE file: a RIFF stream whose chunks, in order, say what the sound is
//! and then hold it.
//!
//! Reading stops at the `data` chunk, because that is where the sound begins
//! and what comes after it is the packets. Everything before it is folded
//! into the one track's codec parameters and the metadata, in whatever order
//! the file wrote them: a `fact` chunk's frame count is replaced by the one
//! the data length implies whenever a format said how to count.

use std::cmp::Ordering;

use console_core_audio_decoding_interfaces::codecs::CodecParameters;
use console_core_audio_decoding_interfaces::errors::{
  Error, Result as Fallible, decode_error, end_of_stream_error, unsupported_error,
};
use console_core_audio_decoding_interfaces::formats::prelude::*;
use console_core_audio_decoding_interfaces::meta::{
  Metadata, MetadataBuilder, MetadataLog, MetadataRevision,
};
use console_core_audio_decoding_interfaces::probe::{Descriptor, Instantiate, QueryDescriptor};
use console_core_audio_decoding_interfaces::support_format;
use console_core_audio_decoding_tags::riff;
use console_core_byte_streams::{MediaSourceStream, ReadBytes};
use console_core_never::Never;

use crate::common::{ByteOrder, Chunk, Chunks, PacketInfo, Stream, append_format_params};

mod format;

use format::WaveFormat;

const WAVE_STREAM_MARKER: [u8; 4] = *b"RIFF";
const WAVE_RIFF_FORM: [u8; 4] = *b"WAVE";

enum WaveChunk {
  Format,
  List,
  Fact,
  Data,
}

fn recognize(tag: [u8; 4]) -> Result<Option<WaveChunk>, Never> {
  Ok(match &tag {
    b"fmt " => Some(WaveChunk::Format),
    b"LIST" => Some(WaveChunk::List),
    b"fact" => Some(WaveChunk::Fact),
    b"data" => Some(WaveChunk::Data),
    _ => None,
  })
}

fn any_tag(tag: [u8; 4]) -> Result<Option<[u8; 4]>, Never> {
  Ok(Some(tag))
}

fn read_value(source: &mut MediaSourceStream, len: u32) -> Fallible<Vec<u8>> {
  let value = source.read_prefix(u64::from(len))?;

  let read =
    u64::try_from(value.len()).map_err(|fault| Error::IoError(std::io::Error::other(fault)))?;

  match read.cmp(&u64::from(len)) {
    Ordering::Equal => Ok(value),
    Ordering::Less | Ordering::Greater => end_of_stream_error(),
  }
}

fn read_info(source: &mut MediaSourceStream, len: u32) -> Fallible<MetadataRevision> {
  let Ok(mut chunks) = Chunks::new(len, ByteOrder::Little);
  let Ok(mut builder) = MetadataBuilder::new();

  for _ in 0..=len {
    let next = chunks.next(source, any_tag)?;

    match next {
      Some(Chunk {
        kind: key,
        len: value_len,
      }) => {
        let value = read_value(source, value_len)?;
        let Ok(tag) = riff::parse(key, &value);

        let Ok(_configured_add_tag) = builder.add_tag(tag);
      }
      None => break,
    }
  }

  chunks.finish(source)?;

  let Ok(value_1) = builder.metadata();

  Ok(value_1)
}

struct Header {
  codec_params: CodecParameters,
  metadata: MetadataLog,
  packet_info: Option<PacketInfo>,
}

impl Header {
  fn read_format(&mut self, source: &mut MediaSourceStream, len: u32) -> Fallible<()> {
    let format = WaveFormat::parse(source, len)?;
    let packet_info = format.packet_info()?;

    self.packet_info = packet_info;

    append_format_params(
      &mut self.codec_params,
      packet_info,
      &format.encoding,
      format.sample_rate,
    )
  }

  fn read_fact(&mut self, source: &mut MediaSourceStream, len: u32) -> Fallible<()> {
    match len {
      4 => (),
      _ => return decode_error("wav: malformed fact chunk"),
    }

    let n_frames = source.read_u32()?;

    let Ok(_configured_frames) = self.codec_params.with_n_frames(u64::from(n_frames));

    Ok(())
  }

  fn read_list(&mut self, source: &mut MediaSourceStream, len: u32) -> Fallible<()> {
    let body_len = len
      .checked_sub(4)
      .ok_or(Error::DecodeError("wav: malformed list chunk"))?;
    let form = source.read_quad_bytes()?;

    match &form {
      b"INFO" => {
        let revision = read_info(source, body_len)?;

        let Ok(()) = self.metadata.push(revision);
      }
      _ => {
        let Ok(mut skipped) = Chunks::new(body_len, ByteOrder::Little);

        skipped.finish(source)?;
      }
    }

    Ok(())
  }
}

pub struct WavReader {
  stream: Stream,
}

impl QueryDescriptor for WavReader {
  fn query() -> std::result::Result<&'static [Descriptor], console_core_never::Never> {
    Ok(&[support_format!(
      "wave",
      "Waveform Audio File Format",
      &["wav", "wave"],
      &["audio/vnd.wave", "audio/x-wav", "audio/wav", "audio/wave"],
      &[b"RIFF"]
    )])
  }

  fn score(_context: &[u8]) -> std::result::Result<u8, console_core_never::Never> {
    Ok(255)
  }
}

impl FormatReader for WavReader {
  fn try_new(mut source: MediaSourceStream, _options: &FormatOptions) -> Fallible<Self> {
    let marker = source.read_quad_bytes()?;

    match marker {
      WAVE_STREAM_MARKER => (),
      _ => return unsupported_error("wav: missing riff stream marker"),
    }

    let riff_len = source.read_u32()?;
    let form = source.read_quad_bytes()?;

    match form {
      WAVE_RIFF_FORM => (),
      _ => return unsupported_error("wav: riff form is not wave"),
    }

    let Ok(mut chunks) = Chunks::new(riff_len, ByteOrder::Little);
    let Ok(packet_info) = PacketInfo::without_blocks(0);
    let Ok(value_2) = CodecParameters::new();

    let mut header = Header {
      codec_params: value_2,
      metadata: MetadataLog::default(),
      packet_info,
    };

    for _ in 0..=riff_len {
      let next = chunks.next(&mut source, recognize)?;

      match next {
        None => return unsupported_error("wav: missing data chunk"),
        Some(Chunk {
          kind: WaveChunk::Format,
          len,
        }) => {
          header.read_format(&mut source, len)?;
        }
        Some(Chunk {
          kind: WaveChunk::Fact,
          len,
        }) => {
          header.read_fact(&mut source, len)?;
        }
        Some(Chunk {
          kind: WaveChunk::List,
          len,
        }) => {
          header.read_list(&mut source, len)?;
        }
        Some(Chunk {
          kind: WaveChunk::Data,
          len,
        }) => {
          let Ok(stream) = Stream::new(
            source,
            header.codec_params,
            header.metadata,
            header.packet_info,
            len,
          );

          return Ok(WavReader { stream });
        }
      }
    }

    unsupported_error("wav: missing data chunk")
  }

  fn next_packet(&mut self) -> Fallible<Packet> {
    self.stream.next_packet()
  }

  fn metadata(&mut self) -> std::result::Result<Metadata<'_>, console_core_never::Never> {
    let Ok(value_3) = self.stream.metadata.metadata();

    Ok(value_3)
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
