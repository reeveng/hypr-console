//! Symphonia
//! Copyright (c) 2019-2022 The Project Symphonia Developers.
//!
//! This Source Code Form is subject to the terms of the Mozilla Public
//! License, v. 2.0. If a copy of the MPL was not distributed with this
//! file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::cmp::Ordering;
use std::io::{Seek, SeekFrom};

use console_core_audio_decoding_interfaces::codecs::CodecParameters;
use console_core_audio_decoding_interfaces::errors::{
  Error, Result, SeekErrorKind, decode_error, end_of_stream_error, seek_error, unsupported_error,
};
use console_core_audio_decoding_interfaces::formats::prelude::*;
use console_core_audio_decoding_interfaces::meta::{Metadata, MetadataLog, MetadataRevision};
use console_core_audio_decoding_interfaces::probe::{Descriptor, Instantiate, QueryDescriptor};
use console_core_audio_decoding_interfaces::support_format;
use console_core_audio_decoding_interfaces::units::Time;
use console_core_byte_streams::{
  MediaSource, MediaSourceStream, ReadBytes, SeekBuffered, Seeking,
};
use console_core_number_conversion::{fitted, index};

use crate::atoms::fragment::MoofAtom;
use crate::atoms::metadata::MetaAtom;
use crate::atoms::movie::{MoovAtom, MvexAtom};
use crate::atoms::track::TrakAtom;
use crate::atoms::{AtomIterator, Kind, OVERFLOW};
use crate::segments::{MoofSegment, MoovSegment, Offset, Segment, TrackNumber};

const NO_TRACK: Error = Error::DecodeError("isomp4: no such track");
const NO_SAMPLE: Error = Error::DecodeError("isomp4: invalid sample index");
const NO_SEGMENT: Error = Error::DecodeError("isomp4: no such segment");

#[derive(Clone, Copy)]
enum Access {
  Seekable,
  Forward,
}

enum FileType {
  Present,
  Absent,
}

struct SeekLocation {
  segment: u32,
  sample: u32,
}

fn access(stream: &MediaSourceStream) -> std::result::Result<Access, console_core_never::Never> {
  Ok(match stream.seeking() {
    Seeking::Anywhere => Access::Seekable,
    Seeking::ForwardOnly => Access::Forward,
  })
}

fn keep_metadata(
  pending_revision: Option<MetadataRevision>,
  metadata: &mut MetadataLog,
) -> std::result::Result<(), console_core_never::Never> {
  match pending_revision {
    Some(revision) => {
      let Ok(()) = metadata.push(revision);
    }
    None => {}
  }

  Ok(())
}

fn stream_length(stream: &mut MediaSourceStream, access: Access) -> Result<Option<u64>> {
  match access {
    Access::Seekable => {
      let position = stream.pos();
      let len = stream
        .byte_len()
        .ok_or(Error::SeekError(SeekErrorKind::Unseekable))?;

      stream.seek(SeekFrom::Start(position))?;

      Ok(Some(len))
    }
    Access::Forward => Ok(None),
  }
}

fn read_movie(
  iter: &mut AtomIterator<MediaSourceStream>,
  access: Access,
  metadata: &mut MetadataLog,
) -> Result<MoovAtom> {
  let mut file_type = FileType::Absent;
  let mut movie = None;
  let Ok(limit) = iter.atoms_at_most();

  for _ in 0..limit {
    let next = iter.next()?;

    match next {
      Some(header) => match header.kind {
        Kind::FILE_TYPE => file_type = FileType::Present,
        Kind::MOVIE => {
          let parsed_value = iter.read_atom::<MoovAtom>()?;
          movie = Some(parsed_value)
        }
        Kind::SEGMENT_INDEX | Kind::MEDIA_DATA | Kind::MOVIE_FRAGMENT => match access {
          Access::Forward => break,
          Access::Seekable => {}
        },
        Kind::META => {
          let atom = iter.read_atom::<MetaAtom>()?;
          let Ok(()) = keep_metadata(atom.metadata, metadata);
        }
        _ => {}
      },
      None => break,
    }
  }

  match file_type {
    FileType::Present => movie.ok_or(Error::Unsupported("isomp4: missing moov atom")),
    FileType::Absent => unsupported_error("isomp4: missing ftyp atom"),
  }
}

fn rewind_to_media(
  mut iter: AtomIterator<MediaSourceStream>,
  access: Access,
  len: Option<u64>,
) -> Result<AtomIterator<MediaSourceStream>> {
  match access {
    Access::Seekable => {
      let Ok(mut stream) = iter.into_inner();

      stream.seek(SeekFrom::Start(0))?;

      let Ok(rewound) = AtomIterator::new_root(stream, len);
      iter = rewound;
      let Ok(limit) = iter.atoms_at_most();

      skip_to_media(&mut iter, limit)?;

      Ok(iter)
    }
    Access::Forward => Ok(iter),
  }
}

fn skip_to_media(iter: &mut AtomIterator<MediaSourceStream>, limit: u64) -> Result<()> {
  for _ in 0..limit {
    let next = iter.next_no_consume()?;

    match next {
      Some(header) => match header.kind {
        Kind::MEDIA_DATA | Kind::MOVIE_FRAGMENT => break,
        _ => {
          let Ok(()) = iter.consume_atom();
        }
      },
      None => break,
    }
  }

  Ok(())
}

fn track_states(tracks: &[TrakAtom]) -> Result<Vec<TrackState>> {
  tracks
    .iter()
    .enumerate()
    .map(|(position, track)| {
      let Ok(number) = fitted::<_, u32>(position);

      TrackState::new(TrackNumber(number), track)
    })
    .collect()
}

fn validate_defaults(pending_defaults: Option<&MvexAtom>, tracks: u32) -> Result<()> {
  match pending_defaults {
    Some(defaults) => {
      let Ok(count) = fitted::<_, u32>(defaults.trexs.len());

      match ::core::primitive::bool::then_some(count == tracks, ()) {
        ::core::option::Option::Some(()) => Ok(()),
        ::core::option::Option::None => {
          decode_error("isomp4: mvex and moov track number mismatch")
        }
      }
    }
    None => Ok(()),
  }
}

pub struct TrackState {
  codec_params: CodecParameters,
  track_num: TrackNumber,
  cur_seg: u32,
  next_sample: u32,
  next_sample_pos: u64,
}

impl TrackState {
  pub fn new(track_num: TrackNumber, trak: &TrakAtom) -> Result<Self> {
    let Ok(mut codec_params) = CodecParameters::new();
    let time_base = TimeBase::new(1, trak.mdia.mdhd.timescale)?;

    let Ok(_configured_with_time_base) = codec_params.with_time_base(time_base);
    let Ok(_configured_with_n_frames) = codec_params.with_n_frames(trak.mdia.mdhd.duration);

    trak
      .mdia
      .minf
      .stbl
      .stsd
      .fill_codec_params(&mut codec_params)?;
    Ok(Self {
      codec_params,
      track_num,
      cur_seg: 0,
      next_sample: 0,
      next_sample_pos: 0,
    })
  }
}

struct NextSampleInfo {
  track_num: TrackNumber,
  ts: u64,
  time: Time,
  dur: u32,
  seg_idx: u32,
}

fn earlier_sample(
  current: &mut Option<NextSampleInfo>,
  candidate: Option<NextSampleInfo>,
) -> std::result::Result<(), console_core_never::Never> {
  match (current.as_ref(), candidate) {
    (Some(held), Some(next)) => {
      match ::core::primitive::bool::then_some(held.time <= next.time, ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => *current = Some(next),
      }
    }
    (None, next) => *current = next,
    (Some(_), None) => {}
  }

  Ok(())
}

struct SampleDataInfo {
  pos: u64,
  len: u32,
}

pub struct IsoMp4Reader {
  iter: AtomIterator<MediaSourceStream>,
  tracks: Vec<Track>,
  cues: Vec<Cue>,
  metadata: MetadataLog,
  segs: Vec<Segment>,
  track_states: Vec<TrackState>,
  mvex: Option<MvexAtom>,
}

impl IsoMp4Reader {
  fn next_sample_for_track(
    &self,
    state: &TrackState,
    track: &Track,
  ) -> Result<Option<NextSampleInfo>> {
    let time_base = track.codec_params.time_base.ok_or(NO_TRACK)?;
    let Ok(first_segment) = index(state.cur_seg);
    let segments = self.segs.get(first_segment..).ok_or(NO_SEGMENT)?;

    for (delta, segment) in segments.iter().enumerate() {
      let candidate_timing = segment.sample_timing(state.track_num, state.next_sample)?;

      match candidate_timing {
        Some(timing) => {
          let Ok(relative_segment) = fitted::<_, u32>(delta);
          let segment_index = state
            .cur_seg
            .checked_add(relative_segment)
            .ok_or(OVERFLOW)?;

          let Ok(value_1) = time_base.calc_time(timing.ts);

          return Ok(Some(NextSampleInfo {
            track_num: state.track_num,
            ts: timing.ts,
            time: value_1,
            dur: timing.dur,
            seg_idx: segment_index,
          }));
        }
        None => {}
      }
    }

    Ok(None)
  }

  fn next_sample_info(&self) -> Result<Option<NextSampleInfo>> {
    let mut earliest: Option<NextSampleInfo> = None;

    for (state, track) in self.track_states.iter().zip(&self.tracks) {
      let candidate = self.next_sample_for_track(state, track)?;
      let Ok(()) = earlier_sample(&mut earliest, candidate);
    }

    Ok(earliest)
  }

  fn next_sample_or_more(&mut self) -> Result<NextSampleInfo> {
    let Ok(limit) = self.iter.atoms_at_most();

    for _ in 0..limit {
      let next = self.next_sample_info()?;

      match next {
        Some(info) => return Ok(info),
        None => self.try_read_more_segments()?,
      }
    }

    end_of_stream_error()
  }

  fn consume_next_sample(&mut self, info: &NextSampleInfo) -> Result<SampleDataInfo> {
    let Ok(position) = index(info.track_num.0);
    let track = self.track_states.get_mut(position).ok_or(NO_TRACK)?;
    let Ok(segment_position) = index(info.seg_idx);
    let segment = self.segs.get(segment_position).ok_or(NO_SEGMENT)?;
    let data = segment.sample_data(track.track_num, track.next_sample, &Offset::Unwanted)?;
    let pos = data.base_pos.max(track.next_sample_pos);
    track.cur_seg = info.seg_idx;
    let parsed_next_sample = track.next_sample.checked_add(1).ok_or(OVERFLOW)?;
    track.next_sample = parsed_next_sample;
    let parsed_next_sample_pos = pos.checked_add(u64::from(data.size)).ok_or(OVERFLOW)?;
    track.next_sample_pos = parsed_next_sample_pos;
    Ok(SampleDataInfo {
      pos,
      len: data.size,
    })
  }

  fn read_fragment(&mut self) -> Result<()> {
    let moof = self.iter.read_atom::<MoofAtom>()?;
    let mvex = self.mvex.as_ref().ok_or(Error::DecodeError(
      "isomp4: moof atom present without mvex atom",
    ))?;
    let previous = self.segs.last().ok_or(NO_SEGMENT)?;
    let fragment = MoofSegment::new(moof, mvex, previous)?;

    self.segs.push(Segment::Fragment(fragment));

    Ok(())
  }

  fn try_read_more_segments(&mut self) -> Result<()> {
    let Ok(limit) = self.iter.atoms_at_most();

    for _ in 0..limit {
      let next = self.iter.next_no_consume()?;

      match next {
        Some(header) => match header.kind {
          Kind::MEDIA_DATA => {
            let Ok(()) = self.iter.consume_atom();
            return Ok(());
          }
          Kind::MOVIE_FRAGMENT => self.read_fragment()?,
          _ => {
            let Ok(()) = self.iter.consume_atom();
          }
        },
        None => break,
      }
    }

    end_of_stream_error()
  }

  fn seek_track_by_time(&mut self, track_num: TrackNumber, time: Time) -> Result<SeekedTo> {
    let Ok(position) = index(track_num.0);

    match self.tracks.get(position) {
      Some(track) => {
        let time_base = track.codec_params.time_base.ok_or(NO_TRACK)?;
        let Ok(value_2) = time_base.calc_timestamp(time);

        self.seek_track_by_ts(track_num, value_2)
      }
      None => seek_error(SeekErrorKind::Unseekable),
    }
  }

  fn find_seek_location(
    &self,
    track: TrackNumber,
    ts: u64,
    skipped: u32,
  ) -> Result<Option<SeekLocation>> {
    let Ok(first) = index(skipped);

    for (position, segment) in self.segs.iter().enumerate().skip(first) {
      let found_sample = segment.ts_sample(track, ts)?;

      match found_sample {
        Some(sample) => {
          let Ok(segment_number) = fitted::<_, u32>(position);
          return Ok(Some(SeekLocation {
            segment: segment_number,
            sample,
          }));
        }
        None => {}
      }
    }

    Ok(None)
  }

  fn seek_location(&mut self, track: TrackNumber, ts: u64) -> Result<SeekLocation> {
    let mut skipped = 0;
    let Ok(limit) = self.iter.atoms_at_most();

    for _ in 0..limit {
      let found_location = self.find_seek_location(track, ts, skipped)?;

      match found_location {
        Some(location) => return Ok(location),
        None => {
          let Ok(count) = fitted::<_, u32>(self.segs.len());
          skipped = count;
          self.try_read_more_segments()?;
        }
      }
    }

    seek_error(SeekErrorKind::OutOfRange)
  }

  fn seek_track_by_ts(&mut self, track_num: TrackNumber, ts: u64) -> Result<SeekedTo> {
    let location = self.seek_location(track_num, ts)?;
    let Ok(segment_position) = index(location.segment);
    let segment = self.segs.get(segment_position).ok_or(NO_SEGMENT)?;
    let data = segment.sample_data(track_num, location.sample, &Offset::Wanted)?;
    let found_timing = segment.sample_timing(track_num, location.sample)?;
    let timing = found_timing.ok_or(NO_SAMPLE)?;
    let offset = data.offset.ok_or(NO_SAMPLE)?;
    let position = data.base_pos.checked_add(offset).ok_or(OVERFLOW)?;
    let Ok(track_position) = index(track_num.0);
    let track = self.track_states.get_mut(track_position).ok_or(NO_TRACK)?;

    track.cur_seg = location.segment;
    track.next_sample = location.sample;
    track.next_sample_pos = position;

    Ok(SeekedTo {
      track_id: track_num.0,
      required_ts: ts,
      actual_ts: timing.ts,
    })
  }

  fn seek_other_tracks(&mut self, selected: TrackNumber, time: Time) -> Result<()> {
    let Ok(count) = fitted::<_, u32>(self.track_states.len());

    for number in 0..count {
      match ::core::primitive::bool::then_some(number == selected.0, ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          self.seek_track_by_time(TrackNumber(number), time)?;
        }
      }
    }

    Ok(())
  }
}

impl QueryDescriptor for IsoMp4Reader {
  fn query() -> std::result::Result<&'static [Descriptor], console_core_never::Never> {
    Ok(&[support_format!(
      "isomp4",
      "ISO Base Media File Format",
      &["mp4", "m4a", "m4p", "m4b", "m4r", "m4v", "mov"],
      &["video/mp4", "audio/mp4"],
      &[b"ftyp"]
    )])
  }

  fn score(_context: &[u8]) -> std::result::Result<u8, console_core_never::Never> {
    Ok(255)
  }
}

impl FormatReader for IsoMp4Reader {
  fn try_new(mut stream: MediaSourceStream, _options: &FormatOptions) -> Result<Self> {
    stream.seek_buffered_rel(-4);

    let Ok(access) = access(&stream);
    let total_len = stream_length(&mut stream, access)?;
    let Ok(mut iter) = AtomIterator::new_root(stream, total_len);
    let mut metadata = MetadataLog::default();
    let mut movie = read_movie(&mut iter, access, &mut metadata)?;
    let ready_iter = rewind_to_media(iter, access, total_len)?;
    let Ok(()) = keep_metadata(movie.metadata.take(), &mut metadata);
    let track_states = track_states(&movie.traks)?;
    let tracks = track_states
      .iter()
      .map(|state| {
        let Ok(track) = Track::new(state.track_num.0, state.codec_params.clone());

        track
      })
      .collect();
    let mvex = movie.mvex.take();
    let Ok(count) = fitted::<_, u32>(movie.traks.len());

    validate_defaults(mvex.as_ref(), count)?;

    let Ok(segment) = MoovSegment::new(movie.traks);

    Ok(Self {
      iter: ready_iter,
      tracks,
      cues: Vec::new(),
      metadata,
      segs: vec![Segment::Movie(segment)],
      track_states,
      mvex,
    })
  }

  fn next_packet(&mut self) -> Result<Packet> {
    let info = self.next_sample_or_more()?;
    let sample = self.consume_next_sample(&info)?;
    let Ok(reader) = self.iter.inner_mut();
    let buffered = reader.seek_buffered(sample.pos);

    match ::core::primitive::bool::then_some(buffered == sample.pos, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => match reader.seeking() {
        Seeking::Anywhere => {
          reader.seek(SeekFrom::Start(sample.pos))?;
        }
        Seeking::ForwardOnly => match sample.pos.cmp(&reader.pos()) {
          Ordering::Greater => {
            let distance = sample.pos.checked_sub(reader.pos()).ok_or(OVERFLOW)?;
            reader.ignore_bytes(distance)?;
          }
          Ordering::Equal | Ordering::Less => {
            return decode_error("isomp4: packet out-of-bounds for a non-seekable stream");
          }
        },
      },
    }

    let len = u64::from(sample.len);
    let data = reader.read_boxed_slice_exact(len)?;

    let Ok(value_3) =
      Packet::new_from_boxed_slice(info.track_num.0, info.ts, u64::from(info.dur), data);

    Ok(value_3)
  }

  fn metadata(&mut self) -> std::result::Result<Metadata<'_>, console_core_never::Never> {
    let Ok(value_4) = self.metadata.metadata();

    Ok(value_4)
  }

  fn cues(&self) -> std::result::Result<&[Cue], console_core_never::Never> {
    Ok(&self.cues)
  }

  fn tracks(&self) -> std::result::Result<&[Track], console_core_never::Never> {
    Ok(&self.tracks)
  }

  fn seek(&mut self, _mode: SeekMode, to: SeekTo) -> Result<SeekedTo> {
    match ::core::primitive::bool::then_some(self.tracks.is_empty(), ()) {
      ::core::option::Option::Some(()) => return seek_error(SeekErrorKind::Unseekable),
      ::core::option::Option::None => {}
    }

    match to {
      SeekTo::TimeStamp { ts, track_id } => {
        let Ok(position) = index(track_id);
        let track = self
          .tracks
          .get(position)
          .ok_or(Error::SeekError(SeekErrorKind::Unseekable))?;
        let time_base = track.codec_params.time_base.ok_or(NO_TRACK)?;
        let time = time_base.calc_time(ts);
        let selected = TrackNumber(track_id);

        let Ok(value_5) = time;

        self.seek_other_tracks(selected, value_5)?;
        self.seek_track_by_ts(selected, ts)
      }
      SeekTo::Time { time, track_id } => {
        let selected = TrackNumber(match track_id {
          Some(number) => number,
          None => 0,
        });

        self.seek_other_tracks(selected, time)?;
        self.seek_track_by_time(selected, time)
      }
    }
  }

  fn into_inner(
    self: Box<Self>,
  ) -> std::result::Result<MediaSourceStream, console_core_never::Never> {
    let Ok(stream) = self.iter.into_inner();
    Ok(stream)
  }
}
