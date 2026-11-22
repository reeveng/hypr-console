//! What a container demuxer is: the traits and the small structures every one
//! of them shares.
//!
//! A reader probes its container, lists the tracks inside, and hands out the
//! packets of them one by one, in the order they were stored. It never decodes
//! one, so a caller chooses which tracks to decode and which to step over.

use crate::codecs::CodecParameters;
use crate::errors::Result;
use crate::io::{BufReader, MediaSourceStream};
use crate::meta::{Metadata, Tag};
use crate::units::{Time, TimeStamp};

pub mod prelude {
  pub use crate::units::{Duration, TimeBase, TimeStamp};

  pub use super::{
    Cue, FormatOptions, FormatReader, GaplessPlayback, Packet, SeekIndexPolicy, SeekMode, SeekTo,
    SeekedTo, Track,
  };
}

pub enum SeekTo {
  Time { time: Time, track_id: Option<u32> },
  TimeStamp { ts: TimeStamp, track_id: u32 },
}

#[derive(Copy, Clone, Debug)]
pub struct SeekedTo {
  pub track_id: u32,
  pub required_ts: TimeStamp,
  pub actual_ts: TimeStamp,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SeekMode {
  Coarse,
  Accurate,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum SeekIndexPolicy {
  #[default]
  OnDemand,
  Prebuild,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum GaplessPlayback {
  #[default]
  Disabled,
  Enabled,
}

#[derive(Copy, Clone, Debug)]
pub struct FormatOptions {
  pub seek_index: SeekIndexPolicy,
  pub seek_index_fill_rate: u16,
  pub gapless: GaplessPlayback,
}

impl Default for FormatOptions {
  fn default() -> Self {
    FormatOptions {
      seek_index: SeekIndexPolicy::OnDemand,
      seek_index_fill_rate: 20,
      gapless: GaplessPlayback::Disabled,
    }
  }
}

#[derive(Clone, Debug)]
pub struct Cue {
  pub index: u32,
  pub start_ts: u64,
  pub tags: Vec<Tag>,
  pub points: Vec<CuePoint>,
}

#[derive(Clone, Debug)]
pub struct CuePoint {
  pub start_offset_ts: u64,
  pub tags: Vec<Tag>,
}

#[derive(Clone, Debug)]
pub struct Track {
  pub id: u32,
  pub codec_params: CodecParameters,
  pub language: Option<String>,
}

impl Track {
  #[must_use]
  pub fn new(
    id: u32,
    codec_params: CodecParameters,
  ) -> std::result::Result<Self, console_core_never::Never> {
    Ok(Track {
      id,
      codec_params,
      language: None,
    })
  }
}

pub trait FormatReader: Send + Sync {
  fn try_new(source: MediaSourceStream, options: &FormatOptions) -> Result<Self>
  where
    Self: Sized;

  fn cues(&self) -> std::result::Result<&[Cue], console_core_never::Never>;

  fn metadata(&mut self) -> std::result::Result<Metadata<'_>, console_core_never::Never>;

  fn seek(&mut self, mode: SeekMode, to: SeekTo) -> Result<SeekedTo>;

  fn tracks(&self) -> std::result::Result<&[Track], console_core_never::Never>;

  #[must_use]
  fn default_track(&self) -> std::result::Result<Option<&Track>, console_core_never::Never> {
    let Ok(value_1) = self.tracks();

    Ok(value_1.first())
  }

  fn next_packet(&mut self) -> Result<Packet>;

  fn into_inner(
    self: Box<Self>,
  ) -> std::result::Result<MediaSourceStream, console_core_never::Never>;
}

#[derive(Clone)]
pub struct Packet {
  track_id: u32,
  pub ts: u64,
  pub dur: u64,
  pub trim_start: u32,
  pub trim_end: u32,
  pub data: Box<[u8]>,
}

#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  allow(
    explicit024_no_swappable_params,
    reason = "a timestamp and a duration are the two numbers every demuxer already passes to make a packet, in this order"
  )
)]
impl Packet {
  #[must_use]
  pub fn new_from_slice(
    track_id: u32,
    ts: u64,
    dur: u64,
    buf: &[u8],
  ) -> std::result::Result<Self, console_core_never::Never> {
    Ok(Packet {
      track_id,
      ts,
      dur,
      trim_start: 0,
      trim_end: 0,
      data: Box::from(buf),
    })
  }

  #[must_use]
  pub fn new_from_boxed_slice(
    track_id: u32,
    ts: u64,
    dur: u64,
    data: Box<[u8]>,
  ) -> std::result::Result<Self, console_core_never::Never> {
    Ok(Packet {
      track_id,
      ts,
      dur,
      trim_start: 0,
      trim_end: 0,
      data,
    })
  }

  #[must_use]
  pub fn track_id(&self) -> std::result::Result<u32, console_core_never::Never> {
    Ok(self.track_id)
  }

  #[must_use]
  pub fn ts(&self) -> std::result::Result<u64, console_core_never::Never> {
    Ok(self.ts)
  }

  #[must_use]
  pub fn dur(&self) -> std::result::Result<u64, console_core_never::Never> {
    Ok(self.dur)
  }

  #[must_use]
  pub fn trim_start(&self) -> std::result::Result<u32, console_core_never::Never> {
    Ok(self.trim_start)
  }

  #[must_use]
  pub fn trim_end(&self) -> std::result::Result<u32, console_core_never::Never> {
    Ok(self.trim_end)
  }

  #[must_use]
  pub fn buf(&self) -> std::result::Result<&[u8], console_core_never::Never> {
    Ok(&self.data)
  }

  #[must_use]
  pub fn as_buf_reader(&self) -> std::result::Result<BufReader<'_>, console_core_never::Never> {
    let Ok(reader) = BufReader::new(&self.data);

    Ok(reader)
  }
}

pub mod util {
  //! What the readers that build a seek index have in common: the index, the
  //! answer it gives, and the trimming a gapless stream asks of a packet.

  use std::cmp::Ordering;

  use console_core_number_conversion::{fitted, index};

  use super::Packet;

  #[derive(Copy, Clone, Debug, PartialEq, Eq)]
  pub struct SeekPoint {
    pub frame_ts: u64,
    pub byte_offset: u64,
    pub n_frames: u32,
  }

  #[derive(Default)]
  pub struct SeekIndex {
    points: Vec<SeekPoint>,
  }

  #[derive(Copy, Clone, Debug, PartialEq, Eq)]
  pub enum SeekSearchResult {
    Stream,
    Upper(SeekPoint),
    Lower(SeekPoint),
    Range(SeekPoint, SeekPoint),
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    allow(
      explicit024_no_swappable_params,
      reason = "a timestamp and a byte offset are the two numbers every demuxer already hands an index, in this order"
    )
  )]
  impl SeekIndex {
    #[must_use]
    pub fn new() -> std::result::Result<SeekIndex, console_core_never::Never> {
      Ok(SeekIndex { points: Vec::new() })
    }

    pub fn insert(
      &mut self,
      ts: u64,
      byte_offset: u64,
      n_frames: u32,
    ) -> std::result::Result<(), console_core_never::Never> {
      let seek_point = SeekPoint {
        frame_ts: ts,
        byte_offset,
        n_frames,
      };
      let (last_ts, last_offset) = self
        .points
        .last()
        .map_or((u64::MAX, u64::MAX), |p| (p.frame_ts, p.byte_offset));

      match (ts.cmp(&last_ts), byte_offset.cmp(&last_offset)) {
        (Ordering::Greater, Ordering::Equal | Ordering::Greater) => self.points.push(seek_point),
        (Ordering::Less, _) => {
          let Ok(at) = fitted::<_, u32>(
            self
              .points
              .partition_point(|p| ts > p.frame_ts && byte_offset >= p.byte_offset),
          );

          let Ok(()) = self.insert_at(at, seek_point);
        }
        (Ordering::Equal, _) | (Ordering::Greater, Ordering::Less) => {}
      };

      Ok(())
    }

    fn insert_at(
      &mut self,
      at: u32,
      seek_point: SeekPoint,
    ) -> std::result::Result<(), console_core_never::Never> {
      let Ok(position) = index(at);

      match (position, position.cmp(&self.points.len())) {
        (0, _) | (_, Ordering::Less) => {
          self.points.push(seek_point);

          match self.points.get_mut(position..) {
            Some(tail @ [_, ..]) => tail.rotate_right(1),
            Some([]) | None => {}
          }
        }
        (1.., Ordering::Equal | Ordering::Greater) => {}
      }

      Ok(())
    }

    #[must_use]
    pub fn search(
      &self,
      frame_ts: u64,
    ) -> std::result::Result<SeekSearchResult, console_core_never::Never> {
      let (first, last) = match (self.points.first(), self.points.last()) {
        (Some(first), Some(last)) => (first, last),
        (None, _) | (_, None) => return Ok(SeekSearchResult::Stream),
      };

      Ok(match (frame_ts.cmp(&first.frame_ts), frame_ts.cmp(&last.frame_ts)) {
        (Ordering::Less, _) => SeekSearchResult::Upper(*first),
        (Ordering::Equal | Ordering::Greater, Ordering::Equal | Ordering::Greater) => {
          SeekSearchResult::Lower(*last)
        }
        (Ordering::Equal | Ordering::Greater, Ordering::Less) => {
          match self
            .points
            .split_at_checked(self.points.partition_point(|p| p.frame_ts <= frame_ts))
          {
            Some((before, after)) => match (before.last(), after.first()) {
              (Some(lower), Some(upper)) => SeekSearchResult::Range(*lower, *upper),
              (None, _) | (_, None) => SeekSearchResult::Stream,
            },
            None => SeekSearchResult::Stream,
          }
        }
      })
    }
  }

  pub fn trim_packet(
    packet: &mut Packet,
    delay: u32,
    num_frames: Option<u64>,
  ) -> std::result::Result<(), console_core_never::Never> {
    let delay_2 = u64::from(delay);

    packet.trim_start = match packet.ts.cmp(&delay_2) {
      Ordering::Less => {
        let trim = delay_2.saturating_sub(packet.ts).min(packet.dur);
        packet.ts = 0;
        packet.dur = packet.dur.saturating_sub(trim);
        let Ok(trimmed) = fitted::<u64, u32>(trim & u64::from(u32::MAX));

        trimmed
      }
      Ordering::Equal | Ordering::Greater => {
        packet.ts = packet.ts.saturating_sub(delay_2);

        0
      }
    };

    match num_frames {
      Some(num_frames_2) => {
        let end = packet.ts.saturating_add(packet.dur);

        packet.trim_end = match end.cmp(&num_frames_2) {
          Ordering::Greater => {
            let trim = end.saturating_sub(num_frames_2).min(packet.dur);
            packet.dur = packet.dur.saturating_sub(trim);
            let Ok(trimmed) = fitted::<u64, u32>(trim & u64::from(u32::MAX));

            trimmed
          }
          Ordering::Less | Ordering::Equal => 0,
        };
      }
      None => {}
    };

    Ok(())
  }
}
