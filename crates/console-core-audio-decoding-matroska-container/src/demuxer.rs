//! Matroska and WebM as a `FormatReader`: the packets of a file, in order, and seeking among them.

use std::collections::{HashMap, VecDeque};

use console_core_audio_decoding_interfaces::errors::{Error, Result as Decoded, SeekErrorKind};
use console_core_audio_decoding_interfaces::formats::{
  Cue, FormatOptions, FormatReader, Packet, SeekMode, SeekTo, SeekedTo, Track,
};
use console_core_audio_decoding_interfaces::meta::{Metadata, MetadataLog};
use console_core_audio_decoding_interfaces::probe::Instantiate;
use console_core_audio_decoding_interfaces::probe::{Descriptor, QueryDescriptor};
use console_core_audio_decoding_interfaces::support_format;
use console_core_audio_decoding_interfaces::units::TimeBase;
use console_core_byte_streams::MediaSourceStream;
use console_core_extensible_binary_meta_language::uint;
use console_core_iteration::{Step, iterate};
use console_core_never::Never;

use crate::ebml::{ElementHeader, Walker};
use crate::fault::DecodeError;
use crate::ids;
use crate::lacing::{Frame, extract_frames};
use crate::scan::{ClusterIndex, ClusterState, open};
use crate::segment::{read_block_group, read_tags};

pub struct MkvReader {
  walker: Walker,
  tracks: Vec<Track>,
  default_durations: HashMap<u32, u64>,
  time_base: TimeBase,
  current_cluster: Option<ClusterState>,
  metadata: MetadataLog,
  frames: VecDeque<Frame>,
  clusters: Vec<ClusterIndex>,
}

impl MkvReader {
  fn cluster_timestamp(&self) -> Result<Option<u64>, Never> {
    Ok(
      self
        .current_cluster
        .as_ref()
        .and_then(|cluster| cluster.timestamp),
    )
  }

  fn leave_finished_cluster(&mut self) -> Result<(), Never> {
    let Ok(at) = self.walker.position();

    self.current_cluster = self
      .current_cluster
      .take()
      .filter(|cluster| cluster.end.is_none_or(|end| at < end));

    Ok(())
  }

  fn read_cluster_timestamp(&mut self, header: &ElementHeader) -> Result<(), DecodeError> {
    match self.current_cluster.as_mut() {
      Some(cluster) => {
        let data = self.walker.read_payload(header)?;

        let Ok(timestamp) = uint(&data);

        cluster.timestamp = Some(timestamp);

        Ok(())
      }
      None => self.walker.skip(header),
    }
  }

  fn read_simple_block(&mut self, header: &ElementHeader) -> Result<(), DecodeError> {
    let Ok(timestamp) = self.cluster_timestamp();

    match timestamp {
      Some(cluster) => {
        let data = self.walker.read_payload(header)?;
        let frames = extract_frames(&data, None, &self.default_durations, cluster)?;

        self.frames.extend(frames);

        Ok(())
      }
      None => self.walker.skip(header),
    }
  }

  fn read_block_group(&mut self, header: &ElementHeader) -> Result<(), DecodeError> {
    let Ok(timestamp) = self.cluster_timestamp();

    match timestamp {
      Some(cluster) => {
        let data = self.walker.read_payload(header)?;
        let group = read_block_group(&data)?;
        let frames = extract_frames(
          &group.data,
          group.duration,
          &self.default_durations,
          cluster,
        )?;

        self.frames.extend(frames);

        Ok(())
      }
      None => self.walker.skip(header),
    }
  }

  fn read_tags(&mut self, header: &ElementHeader) -> Result<(), DecodeError> {
    let data = self.walker.read_payload(header)?;
    let revision = read_tags(&data)?;

    let Ok(()) = self.metadata.push(revision);
    self.current_cluster = None;

    Ok(())
  }

  fn next_element(&mut self) -> Result<(), DecodeError> {
    let Ok(()) = self.leave_finished_cluster();
    let header = self.walker.read_header()?;
    let header_2 = header.ok_or(DecodeError::EndOfStream)?;

    match header_2.id {
      ids::CLUSTER => {
        self.current_cluster = Some(ClusterState {
          timestamp: None,
          end: header_2.end,
        });

        Ok(())
      }
      ids::TIMESTAMP => self.read_cluster_timestamp(&header_2),
      ids::SIMPLE_BLOCK => self.read_simple_block(&header_2),
      ids::BLOCK_GROUP => self.read_block_group(&header_2),
      ids::TAGS => self.read_tags(&header_2),
      ids::TRACKS | ids::CUES | ids::INFO | ids::SEEK_HEAD => {
        self.current_cluster = None;

        Ok(())
      }
      _ => self.walker.skip(&header_2),
    }
  }

  fn next_frame(&mut self) -> Result<Frame, DecodeError> {
    iterate(self, |reader| {
      Ok(match reader.frames.pop_front() {
        Some(frame) => Step::Halt(Ok(frame)),
        None => match reader.next_element() {
          Ok(()) => Step::Again(reader),
          Err(fault) => Step::Halt(Err(fault)),
        },
      })
    })
    .map_err(DecodeError::from)
    .and_then(std::convert::identity)
  }

  fn drop_frames_before(&mut self, track_id: u32, ts: u64) -> Result<Option<u64>, Never> {
    let held = std::mem::take(&mut self.frames);

    self.frames = held
      .into_iter()
      .skip_while(|frame| {
        !(frame.timestamp.saturating_add(frame.duration) >= ts && frame.track == track_id)
      })
      .collect();

    Ok(self.frames.front().map(|frame| frame.timestamp))
  }

  fn seek_track_by_ts_forward(&mut self, track_id: u32, ts: u64) -> Result<SeekedTo, DecodeError> {
    let found = iterate(self, |reader| {
      let Ok(landed) = reader.drop_frames_before(track_id, ts);

      Ok(match landed {
        Some(actual_ts) => Step::Halt(Ok(actual_ts)),
        None => match reader.next_element() {
          Ok(()) => Step::Again(reader),
          Err(fault) => Step::Halt(Err(fault)),
        },
      })
    })
    .map_err(DecodeError::from)?;
    let actual_ts = found?;

    Ok(SeekedTo {
      track_id,
      required_ts: ts,
      actual_ts,
    })
  }

  fn seek_track_by_ts(&mut self, track_id: u32, ts: u64) -> Result<SeekedTo, DecodeError> {
    match self.clusters.as_slice() {
      [] => self.seek_track_by_ts_forward(track_id, ts),
      [_, ..] => {
        let cluster = self
          .clusters
          .iter()
          .take_while(|cluster| cluster.timestamp <= ts)
          .last();
        let cluster_2 = cluster.ok_or(DecodeError::Seek(SeekErrorKind::OutOfRange))?;
        let (timestamp, position) = (cluster_2.timestamp, cluster_2.position);

        self.walker.seek(position)?;
        self.current_cluster = Some(ClusterState {
          timestamp: Some(timestamp),
          end: None,
        });

        self.seek_track_by_ts_forward(track_id, ts)
      }
    }
  }
}

impl FormatReader for MkvReader {
  fn try_new(reader: MediaSourceStream, _options: &FormatOptions) -> Decoded<Self> {
    let opened = open(reader)?;

    Ok(Self {
      walker: opened.walker,
      tracks: opened.tracks,
      default_durations: opened.default_durations,
      time_base: opened.time_base,
      current_cluster: opened.current_cluster,
      metadata: opened.metadata,
      frames: VecDeque::new(),
      clusters: opened.clusters,
    })
  }

  fn cues(&self) -> std::result::Result<&[Cue], console_core_never::Never> {
    Ok(&[])
  }

  fn metadata(&mut self) -> std::result::Result<Metadata<'_>, console_core_never::Never> {
    let Ok(value_1) = self.metadata.metadata();

    Ok(value_1)
  }

  fn seek(&mut self, _mode: SeekMode, to: SeekTo) -> Decoded<SeekedTo> {
    match to {
      SeekTo::Time { time, track_id } => {
        let track = match track_id {
          Some(id) => self.tracks.iter().find(|track| track.id == id),
          None => self.tracks.first(),
        };
        let track_2 = track.ok_or(Error::SeekError(SeekErrorKind::InvalidTrack))?;
        let ts = self.time_base.calc_timestamp(time);
        let track_id_2 = track_2.id;

        let Ok(value_2) = ts;

        self
          .seek_track_by_ts(track_id_2, value_2)
          .map_err(Error::from)
      }
      SeekTo::TimeStamp { ts, track_id } => {
        match self.tracks.iter().find(|track| track.id == track_id) {
          Some(_) => self.seek_track_by_ts(track_id, ts).map_err(Error::from),
          None => Err(Error::SeekError(SeekErrorKind::InvalidTrack)),
        }
      }
    }
  }

  fn tracks(&self) -> std::result::Result<&[Track], console_core_never::Never> {
    Ok(&self.tracks)
  }

  fn next_packet(&mut self) -> Decoded<Packet> {
    let frame = self.next_frame()?;

    let Ok(value_3) =
      Packet::new_from_boxed_slice(frame.track, frame.timestamp, frame.duration, frame.data);

    Ok(value_3)
  }

  fn into_inner(
    self: Box<Self>,
  ) -> std::result::Result<MediaSourceStream, console_core_never::Never> {
    let Ok(reader) = self.walker.into_inner();

    Ok(reader)
  }
}

impl QueryDescriptor for MkvReader {
  fn query() -> std::result::Result<&'static [Descriptor], console_core_never::Never> {
    Ok(&[support_format!(
      "matroska",
      "Matroska / WebM",
      &["webm", "mkv"],
      &["video/webm", "video/x-matroska"],
      &[b"\x1A\x45\xDF\xA3"]
    )])
  }

  fn score(_context: &[u8]) -> std::result::Result<u8, console_core_never::Never> {
    Ok(255)
  }
}
