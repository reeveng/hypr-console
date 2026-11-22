//! What WAVE and AIFF share once their own chunks are read: the walk over
//! chunks, the packets a run of sound is cut into, and the seek among them.
//!
//! Neither container is packetized, so a packet is a fixed number of blocks
//! read from wherever the stream stands, and a seek lands on a packet boundary
//! so that a packet has the same timestamp whether the stream was seeked or
//! not. A chunk's length is untrusted input and is compared with what is left
//! of its parent rather than added to anything.

use std::cmp::Ordering;
use std::io::{Seek, SeekFrom};
use std::num::NonZeroU64;

use console_core_audio_decoding_interfaces::audio::Channels;
use console_core_audio_decoding_interfaces::codecs::{CodecParameters, CodecType};
use console_core_audio_decoding_interfaces::errors::{
  Error, Result as Fallible, SeekErrorKind, decode_error, end_of_stream_error, seek_error,
};
use console_core_audio_decoding_interfaces::formats::prelude::*;
use console_core_audio_decoding_interfaces::meta::MetadataLog;
use console_core_byte_streams::{MediaSource, MediaSourceStream, ReadBytes, Seeking};
use console_core_never::Never;

const MAX_FRAMES_PER_PACKET: u64 = 1152;

#[derive(Clone, Copy)]
pub enum ByteOrder {
  Little,
  Big,
}

pub struct Chunk<T> {
  pub kind: T,
  pub len: u32,
}

pub struct Chunks {
  len: u32,
  byte_order: ByteOrder,
  consumed: u32,
}

impl Chunks {
  pub fn new(len: u32, byte_order: ByteOrder) -> Result<Self, Never> {
    Ok(Chunks {
      len,
      byte_order,
      consumed: 0,
    })
  }

  pub fn next<T>(
    &mut self,
    reader: &mut MediaSourceStream,
    recognize: fn([u8; 4]) -> Result<Option<T>, Never>,
  ) -> Fallible<Option<Chunk<T>>> {
    for _ in 0..=self.len {
      let header = self.header(reader)?;

      match header {
        None => return Ok(None),
        Some((tag, len)) => {
          let Ok(recognized) = recognize(tag);

          match recognized {
            Some(kind) => return Ok(Some(Chunk { kind, len })),
            None => {
              reader.ignore_bytes(u64::from(len))?;
            }
          }
        }
      }
    }

    Ok(None)
  }

  pub fn finish(&mut self, reader: &mut MediaSourceStream) -> Fallible<()> {
    match self.len.checked_sub(self.consumed) {
      Some(remaining) => {
        reader.ignore_bytes(u64::from(remaining))?;
      }
      None => (),
    }

    match self.len & 1 {
      1 => {
        reader.read_u8()?;
      }
      _ => (),
    }

    Ok(())
  }

  fn header(&mut self, reader: &mut MediaSourceStream) -> Fallible<Option<([u8; 4], u32)>> {
    match self.consumed & 1 {
      1 => {
        reader.read_u8()?;
        self.consumed = self.consumed.saturating_add(1);
      }
      _ => (),
    }

    match self.consumed.saturating_add(8).cmp(&self.len) {
      Ordering::Greater => return Ok(None),
      Ordering::Less | Ordering::Equal => (),
    }

    let tag = reader.read_quad_bytes()?;
    let len = match self.byte_order {
      ByteOrder::Little => reader.read_u32()?,
      ByteOrder::Big => reader.read_be_u32()?,
    };

    self.consumed = self.consumed.saturating_add(8);

    let remaining = self.len.saturating_sub(self.consumed);

    match (remaining.cmp(&len), (self.len, len)) {
      (Ordering::Less, (u32::MAX, u32::MAX)) | (Ordering::Equal | Ordering::Greater, _) => (),
      (Ordering::Less, _) => {
        return decode_error("riff: chunk length exceeds parent (list) chunk length");
      }
    }

    self.consumed = self.consumed.saturating_add(len);

    Ok(Some((tag, len)))
  }
}

pub struct Bits {
  pub coded: u16,
  pub sample: u16,
}

pub struct Encoding {
  pub codec: CodecType,
  pub channels: Channels,
  pub bits: Option<Bits>,
}

pub fn channels_for(count: u16) -> Fallible<Channels> {
  32u32
    .checked_sub(u32::from(count))
    .and_then(|shift| u32::MAX.checked_shr(shift))
    .and_then(|bits| {
      let Ok(channels) = Channels::from_bits(bits);
      channels
    })
    .ok_or(Error::DecodeError("riff: invalid channel count"))
}

pub fn fix_channel_mask(mask: u32, channels: u16) -> Fallible<u32> {
  let wanted = u32::from(channels);
  let held = mask.count_ones();

  match (channels, wanted.cmp(&held)) {
    (33.., _) => decode_error("riff: too many channels for a channel mask"),
    (_, Ordering::Greater) => {
      let missing = wanted.saturating_sub(held);
      let above = 32u32.saturating_sub(mask.leading_zeros());
      let ones = u32::MAX.checked_shr(32u32.saturating_sub(missing));
      let added = match ones.and_then(|bits| bits.checked_shl(above)) {
        Some(bits) => bits,
        None => 0,
      };

      Ok(mask | added)
    }
    (_, Ordering::Less | Ordering::Equal) => Ok(
      (0..wanted)
        .fold((0u32, mask), |(kept, rest), _| {
          let lowest = rest.isolate_lowest_one();

          (kept | lowest, rest ^ lowest)
        })
        .0,
    ),
  }
}

#[derive(Clone, Copy)]
pub struct PacketInfo {
  block_size: NonZeroU64,
  frames_per_block: NonZeroU64,
  max_blocks_per_packet: NonZeroU64,
}

impl PacketInfo {
  pub fn with_blocks(block_size: u16, frames_per_block: u64) -> Fallible<Option<Self>> {
    let per_block = NonZeroU64::new(frames_per_block)
      .ok_or(Error::DecodeError("riff: frames per block is 0"))?;
    let max_blocks = NonZeroU64::new(per_block.get().max(MAX_FRAMES_PER_PACKET) / per_block)
      .ok_or(Error::DecodeError("riff: frames per block is 0"))?;

    Ok(
      NonZeroU64::new(u64::from(block_size)).map(|bytes| PacketInfo {
        block_size: bytes,
        frames_per_block: per_block,
        max_blocks_per_packet: max_blocks,
      }),
    )
  }

  pub fn without_blocks(frame_len: u16) -> Result<Option<Self>, Never> {
    Ok(NonZeroU64::new(u64::from(frame_len)).and_then(|bytes| {
      NonZeroU64::new(MAX_FRAMES_PER_PACKET).map(|max_blocks| PacketInfo {
        block_size: bytes,
        frames_per_block: NonZeroU64::MIN,
        max_blocks_per_packet: max_blocks,
      })
    }))
  }

  pub fn max_frames_per_packet(&self) -> Result<NonZeroU64, Never> {
    Ok(
      self
        .max_blocks_per_packet
        .saturating_mul(self.frames_per_block),
    )
  }

  fn frames(&self, data_len: u64) -> Result<u64, Never> {
    Ok((data_len / self.block_size).saturating_mul(self.frames_per_block.get()))
  }

  fn actual_ts(&self, ts: u64) -> Result<u64, Never> {
    let Ok(packet) = self.max_frames_per_packet();

    Ok((ts / packet).saturating_mul(packet.get()))
  }
}

pub struct Stream {
  pub(super) reader: MediaSourceStream,
  pub(super) track: Track,
  pub(super) metadata: MetadataLog,
  packet_info: Option<PacketInfo>,
  data_start_pos: u64,
  data_end_pos: u64,
}

pub fn append_format_params(
  codec_params: &mut CodecParameters,
  packet_info: Option<PacketInfo>,
  encoding: &Encoding,
  sample_rate: u32,
) -> Fallible<()> {
  let (max_frames, frames_per_block) = match packet_info {
    Some(info) => {
      let Ok(max_frames) = info.max_frames_per_packet();

      (max_frames.get(), info.frames_per_block.get())
    }
    None => (MAX_FRAMES_PER_PACKET, 1),
  };
  let time_base = TimeBase::new(1, sample_rate)?;

  let Ok(_configured_with_max_frames_per_packet) =
    codec_params.with_max_frames_per_packet(max_frames);
  let Ok(_configured_with_frames_per_block) =
    codec_params.with_frames_per_block(frames_per_block);

  let Ok(_configured_with_sample_rate) = codec_params.with_sample_rate(sample_rate);
  let Ok(_configured_with_time_base) = codec_params.with_time_base(time_base);

  let Ok(_configured_for_codec) = codec_params.for_codec(encoding.codec);
  let Ok(_configured_with_channels) = codec_params.with_channels(encoding.channels);

  match &encoding.bits {
    Some(bits) => {
      let Ok(_configured_with_bits_per_coded_sample) =
        codec_params.with_bits_per_coded_sample(u32::from(bits.coded));
      let Ok(_configured_with_bits_per_sample) =
        codec_params.with_bits_per_sample(u32::from(bits.sample));
    }
    None => (),
  }

  Ok(())
}

impl Stream {
  pub fn new(
    reader: MediaSourceStream,
    mut codec_params: CodecParameters,
    metadata: MetadataLog,
    packet_info: Option<PacketInfo>,
    data_len: u32,
  ) -> Result<Self, Never> {
    let data_start_pos = reader.pos();

    match packet_info {
      Some(info) => {
        let Ok(n_frames) = info.frames(u64::from(data_len));

        let Ok(_configured_with_n_frames) = codec_params.with_n_frames(n_frames);
      }
      None => (),
    }

    let Ok(value_1) = Track::new(0, codec_params);

    Ok(Stream {
      reader,
      track: value_1,
      metadata,
      packet_info,
      data_start_pos,
      data_end_pos: data_start_pos.saturating_add(u64::from(data_len)),
    })
  }

  pub fn next_packet(&mut self) -> Fallible<Packet> {
    let pos = self.reader.pos();
    let info = self
      .packet_info
      .ok_or(Error::DecodeError("riff: block size is 0"))?;
    let blocks_left = match self.data_end_pos.checked_sub(pos) {
      Some(left) => left / info.block_size,
      None => 0,
    };

    match blocks_left {
      0 => return end_of_stream_error(),
      _ => (),
    }

    let blocks = blocks_left.min(info.max_blocks_per_packet.get());
    let dur = blocks.saturating_mul(info.frames_per_block.get());
    let packet_len = blocks.saturating_mul(info.block_size.get());
    let buf = self.reader.read_prefix(packet_len)?;

    match buf.as_slice() {
      [] => return end_of_stream_error(),
      [_, ..] => (),
    }

    let Ok(pts) = info.frames(pos.saturating_sub(self.data_start_pos));

    let Ok(value_2) = Packet::new_from_boxed_slice(0, pts, dur, buf.into_boxed_slice());

    Ok(value_2)
  }

  pub fn seek(&mut self, to: SeekTo) -> Fallible<SeekedTo> {
    let info = self
      .packet_info
      .ok_or(Error::SeekError(SeekErrorKind::Unseekable))?;
    let params = &self.track.codec_params;
    let ts = match to {
      SeekTo::TimeStamp { ts, .. } => ts,
      SeekTo::Time { time, .. } => {
        let Ok(value_3) = match params.sample_rate {
          Some(rate) => {
            let time_base = TimeBase::new(1, rate)?;

            time_base.calc_timestamp(time)
          }
          None => return seek_error(SeekErrorKind::Unseekable),
        };

        value_3
      }
    };

    match params
      .n_frames
      .map_or(Ordering::Less, |n_frames| ts.cmp(&n_frames))
    {
      Ordering::Greater => return seek_error(SeekErrorKind::OutOfRange),
      Ordering::Less | Ordering::Equal => (),
    }

    let Ok(actual_ts) = info.actual_ts(ts);
    let seek_pos = self
      .data_start_pos
      .saturating_add(actual_ts.saturating_mul(info.block_size.get()));

    match self.reader.seeking() {
      Seeking::Anywhere => {
        self.reader.seek(SeekFrom::Start(seek_pos))?;
      }
      Seeking::ForwardOnly => match seek_pos.checked_sub(self.reader.pos()) {
        Some(skip) => {
          self.reader.ignore_bytes(skip)?;
        }
        None => return seek_error(SeekErrorKind::ForwardOnly),
      },
    }

    Ok(SeekedTo {
      track_id: 0,
      actual_ts,
      required_ts: ts,
    })
  }
}
