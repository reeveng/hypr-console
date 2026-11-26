//! The ADTS reader: raw AAC frames, each with a seven byte header of its own.
//!
//! There is no container around the frames and so no index to seek by and no
//! duration to read. A seek walks the headers from the start, and the length is
//! estimated from the average size of the frames in a few places in the file,
//! which is as right as the bitrate is constant.

use std::io::{Seek, SeekFrom};

use core::cmp::Ordering;

use console_core_audio_decoding_interfaces::audio::Channels;
use console_core_audio_decoding_interfaces::codecs::{CODEC_TYPE_AAC, CodecParameters};
use console_core_audio_decoding_interfaces::errors::{
  Error, Result, SeekErrorKind, seek_error, unsupported_error,
};
use console_core_audio_decoding_interfaces::formats::prelude::*;
use console_core_audio_decoding_interfaces::meta::{Metadata, MetadataLog};
use console_core_audio_decoding_interfaces::probe::{Descriptor, Instantiate, QueryDescriptor};
use console_core_audio_decoding_interfaces::support_format;
use console_core_byte_streams::{
  MediaSource, MediaSourceStream, ReadBytes, ScopedStream, SeekBuffered, Seeking,
};
use console_core_iteration::{Endless, Step, iterate};
use console_core_number_conversion::index;

use crate::config::{AAC_SAMPLE_RATES, map_channels};
use crate::lane::{Lane, Sure, invalid};

const SAMPLES_PER_AAC_PACKET: u64 = 1024;

const HEADER_SIZE: u32 = 7;

const SAMPLED_FRAMES: u32 = 101;

const SAMPLE_POINTS: u64 = 4;

const SCOPED_LENGTH: u64 = 16 * 1024;

pub struct AdtsReader {
  reader: MediaSourceStream,
  tracks: Vec<Track>,
  cues: Vec<Cue>,
  metadata: MetadataLog,
  first_frame_pos: u64,
  next_packet_ts: u64,
}

impl QueryDescriptor for AdtsReader {
  fn query() -> std::result::Result<&'static [Descriptor], console_core_never::Never> {
    Ok(&[support_format!(
      "aac",
      "Audio Data Transport Stream (native AAC)",
      &["aac"],
      &["audio/aac"],
      &[&[0xff, 0xf1]]
    )])
  }

  fn score(_context: &[u8]) -> std::result::Result<u8, console_core_never::Never> {
    Ok(255)
  }
}

struct AdtsHeader {
  channels: Option<Channels>,
  sample_rate: u32,
  frame_len: u32,
}

pub struct AdtsFrame {
  pub sample_rate: u32,
  pub bytes: u32,
  pub blocks: u32,
  pub stream: u32,
}

pub fn adts_frame([a, b, c, d, e, f, g]: [u8; 7]) -> Result<Option<AdtsFrame>> {
  match (a, b & 0xf6) {
    (0xff, 0xf0) => {}
    _signature => return Ok(None),
  }

  let rate_index = u32::from(c.wrapping_shr(2) & 15);
  let sample_rate = match rate_index {
    13.. => return Ok(None),
    rate => AAC_SAMPLE_RATES.at(rate)?,
  };
  let bytes = u32::from(d & 3).wrapping_shl(11)
    | u32::from(e).wrapping_shl(3)
    | u32::from(f.wrapping_shr(5));
  let header_size = match b & 1 {
    0 => 9,
    _unprotected => 7,
  };

  Ok(match bytes.cmp(&header_size) {
    Ordering::Equal | Ordering::Greater => Some(AdtsFrame {
      sample_rate,
      bytes,
      blocks: u32::from(g & 3).wrapping_add(1),
      stream: u32::from_be_bytes([a, b, c, d]) & 0xffff_ffe0,
    }),
    Ordering::Less => None,
  })
}

impl AdtsHeader {
  fn sync<B: ReadBytes>(reader: &mut B) -> Result<()> {
    let synchronized = iterate((reader, 0u16), |(source, window)| {
      let byte = match source.read_u8() {
        Ok(byte) => byte,
        Err(fault) => return Ok(Step::Halt(Err(Error::IoError(fault)))),
      };
      let next = window.wrapping_shl(8) | u16::from(byte);

      Ok(match next {
        0xfff1 => Step::Halt(Ok(())),
        _other => Step::Again((source, next)),
      })
    });

    match synchronized {
      Ok(result) => result,
      Err(Endless) => invalid(),
    }
  }

  fn read<B: ReadBytes>(reader: &mut B) -> Result<Self> {
    AdtsHeader::sync(reader)?;

    let mut buf = [0u8; 5];

    reader.read_buf_exact(&mut buf)?;

    let [c, d, e, f, g] = buf;
    let frame = adts_frame([0xff, 0xf1, c, d, e, f, g])?;
    let frame_2 = frame.ok_or(Error::DecodeError("adts: invalid frame header"))?;
    let channel_config = u32::from(c & 1).wrapping_shl(2) | u32::from(d.wrapping_shr(6));
    let channels = match channel_config {
      0 => None,
      config => {
        let Ok(mapped) = map_channels(config);

        mapped
      }
    };

    match frame_2.blocks {
      1 => Ok(AdtsHeader {
        channels,
        sample_rate: frame_2.sample_rate,
        frame_len: frame_2.bytes.saturating_sub(HEADER_SIZE),
      }),
      _ => unsupported_error("adts: only 1 aac frame per adts packet is supported"),
    }
  }
}

fn rewind(source: &mut MediaSourceStream) -> Sure<()> {
  let Ok(()) = source.seek_buffered_rev(u64::from(HEADER_SIZE));

  Ok(())
}

impl FormatReader for AdtsReader {
  fn try_new(mut source: MediaSourceStream, _options: &FormatOptions) -> Result<Self> {
    let header = AdtsHeader::read(&mut source)?;
    let Ok(mut params) = CodecParameters::new();

    let time_base = TimeBase::new(1, header.sample_rate)?;

    let Ok(_configured) = params.for_codec(CODEC_TYPE_AAC);
    let Ok(_configured) = params.with_sample_rate(header.sample_rate);
    let Ok(_configured) = params.with_time_base(time_base);

    match header.channels {
      Some(channels) => {
        let Ok(_configured_with_channels) = params.with_channels(channels);
      }
      None => {}
    }

    let Ok(()) = rewind(&mut source);
    let first_frame_pos = source.pos();
    let n_frames = approximate_frame_count(&mut source)?;

    match n_frames {
      Some(count) => {
        let Ok(_configured_with_n_frames) = params.with_n_frames(count);
      }
      None => {}
    }

    let Ok(value_1) = Track::new(0, params);

    Ok(AdtsReader {
      reader: source,
      tracks: vec![value_1],
      cues: Vec::new(),
      metadata: Default::default(),
      first_frame_pos,
      next_packet_ts: 0,
    })
  }

  fn next_packet(&mut self) -> Result<Packet> {
    let header = AdtsHeader::read(&mut self.reader)?;
    let ts = self.next_packet_ts;
    let length = u64::from(header.frame_len);

    self.next_packet_ts = self.next_packet_ts.saturating_add(SAMPLES_PER_AAC_PACKET);

    let body = self.reader.read_boxed_slice_exact(length)?;

    let Ok(value_2) = Packet::new_from_boxed_slice(0, ts, SAMPLES_PER_AAC_PACKET, body);

    Ok(value_2)
  }

  fn metadata(&mut self) -> std::result::Result<Metadata<'_>, console_core_never::Never> {
    let Ok(value_3) = self.metadata.metadata();

    Ok(value_3)
  }

  fn cues(&self) -> std::result::Result<&[Cue], console_core_never::Never> {
    Ok(&self.cues)
  }

  fn tracks(&self) -> std::result::Result<&[Track], console_core_never::Never> {
    Ok(&self.tracks)
  }

  fn seek(&mut self, _mode: SeekMode, to: SeekTo) -> Result<SeekedTo> {
    let required_ts = match to {
      SeekTo::TimeStamp { ts, .. } => ts,
      SeekTo::Time { time, .. } => {
        let Ok(value_4) = {
          let sample_rate = self
            .tracks
            .first()
            .and_then(|track| track.codec_params.sample_rate);

          match sample_rate {
            Some(sample_rate_2) => {
              let time_base = TimeBase::new(1, sample_rate_2)?;

              time_base.calc_timestamp(time)
            }
            None => return seek_error(SeekErrorKind::Unseekable),
          }
        };

        value_4
      }
    };

    match required_ts.cmp(&self.next_packet_ts) {
      Ordering::Less => self.rewind_to_start()?,
      Ordering::Equal | Ordering::Greater => {}
    }

    self.skip_to(required_ts)?;

    Ok(SeekedTo {
      track_id: 0,
      required_ts,
      actual_ts: self.next_packet_ts,
    })
  }

  fn into_inner(
    self: Box<Self>,
  ) -> std::result::Result<MediaSourceStream, console_core_never::Never> {
    Ok(self.reader)
  }
}

impl AdtsReader {
  fn rewind_to_start(&mut self) -> Result<()> {
    match self.reader.seeking() {
      Seeking::Anywhere => {
        let seeked_pos = self.reader.seek(SeekFrom::Start(self.first_frame_pos))?;

        match seeked_pos.cmp(&self.first_frame_pos) {
          Ordering::Equal => {}
          Ordering::Less | Ordering::Greater => return seek_error(SeekErrorKind::Unseekable),
        }
      }
      Seeking::ForwardOnly => return seek_error(SeekErrorKind::ForwardOnly),
    }

    self.next_packet_ts = 0;

    Ok(())
  }

  fn skip_to(&mut self, required_ts: u64) -> Result<()> {
    let remaining = required_ts
      .saturating_sub(self.next_packet_ts)
      .wrapping_div(SAMPLES_PER_AAC_PACKET);

    for _ in 0..=remaining {
      let header = AdtsHeader::read(&mut self.reader)?;

      let next_ts = self.next_packet_ts.saturating_add(SAMPLES_PER_AAC_PACKET);

      match next_ts.cmp(&required_ts) {
        Ordering::Greater => {
          let Ok(()) = rewind(&mut self.reader);

          return Ok(());
        }
        Ordering::Less | Ordering::Equal => {}
      }

      self.reader.ignore_bytes(u64::from(header.frame_len))?;
      self.next_packet_ts = self.next_packet_ts.saturating_add(SAMPLES_PER_AAC_PACKET);
    }

    invalid()
  }
}

struct Extent {
  original_pos: u64,
  total_len: u64,
}

struct Sampled {
  frames: u64,
  bytes: u64,
}

fn approximate_frame_count(source: &mut MediaSourceStream) -> Result<Option<u64>> {
  let original_pos = source.pos();
  let total_len = match source.byte_len() {
    Some(len) => len.saturating_sub(original_pos),
    None => return Ok(None),
  };
  let sampled = match source.seeking() {
    Seeking::ForwardOnly => sample_stream(source, original_pos)?,
    Seeking::Anywhere => sample_file(
      source,
      Extent {
        original_pos,
        total_len,
      },
    )?,
  };

  match sampled.bytes.checked_div(sampled.frames) {
    None => Ok(None),
    Some(average) => Ok(
      total_len
        .checked_div(average)
        .map(|frames| frames.saturating_mul(SAMPLES_PER_AAC_PACKET)),
    ),
  }
}

fn sample_stream(source: &mut MediaSourceStream, original_pos: u64) -> Result<Sampled> {
  let mut sampled = Sampled {
    frames: 0,
    bytes: 0,
  };

  source.ensure_seekback_buffer(SCOPED_LENGTH);

  let Ok(mut scoped_stream) = ScopedStream::new(&mut *source, SCOPED_LENGTH);

  for _ in 0..SCOPED_LENGTH {
    let header = match AdtsHeader::read(&mut scoped_stream) {
      Ok(header) => header,
      Err(_ended) => break,
    };

    match scoped_stream.ignore_bytes(u64::from(header.frame_len)) {
      Ok(()) => {}
      Err(_ended) => break,
    }

    let Ok(()) = sampled.count(&header);
  }

  let _restored = source.seek_buffered(original_pos);

  Ok(sampled)
}

fn sample_file(source: &mut MediaSourceStream, extent: Extent) -> Result<Sampled> {
  let Extent {
    original_pos,
    total_len,
  } = extent;
  let step = total_len
    .saturating_sub(original_pos)
    .wrapping_div(SAMPLE_POINTS);
  let mut sampled = Sampled {
    frames: 0,
    bytes: 0,
  };

  match step {
    0 => {}
    _ => {
      let Ok(stride) = index(step);

      for new_pos in (original_pos..total_len.saturating_sub(step))
        .step_by(stride.max(1))
        .skip(1)
      {
        match source.seek(SeekFrom::Start(new_pos)) {
          Ok(_) => {
            let Ok(()) = sample_run(source, &mut sampled);
          }
          Err(_unreachable) => break,
        }
      }
    }
  }

  source.seek(SeekFrom::Start(original_pos))?;

  Ok(sampled)
}

fn sample_run(source: &mut MediaSourceStream, sampled: &mut Sampled) -> Sure<()> {
  for _ in 0..SAMPLED_FRAMES {
    let header = match AdtsHeader::read(source) {
      Ok(header) => header,
      Err(_ended) => break,
    };

    match source.ignore_bytes(u64::from(header.frame_len)) {
      Ok(()) => {}
      Err(_ended) => break,
    }

    let Ok(()) = sampled.count(&header);
  }

  Ok(())
}

impl Sampled {
  fn count(&mut self, header: &AdtsHeader) -> Sure<()> {
    self.frames = self.frames.saturating_add(1);
    self.bytes = self
      .bytes
      .saturating_add(u64::from(header.frame_len).saturating_add(u64::from(HEADER_SIZE)));

    Ok(())
  }
}
