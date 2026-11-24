//! The front of a file, read up to its first cluster and then out of order.
//!
//! The tracks, the info, the cues and the tags of a segment usually come before
//! its first cluster, and when they do not the seek head says where they are.
//! A stream that can be seeked is read through once to the first cluster, then
//! each place the seek head names is visited, and the walk goes back to the
//! start of the segment to be read again as packets. A stream that cannot be
//! seeked keeps what it found on the way and carries on from the cluster.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::{Seek, SeekFrom};

use console_core_audio_decoding_interfaces::errors::SeekErrorKind;
use console_core_audio_decoding_interfaces::formats::Track;
use console_core_audio_decoding_interfaces::meta::MetadataLog;
use console_core_audio_decoding_interfaces::units::TimeBase;
use console_core_byte_streams::{MediaSource, MediaSourceStream, ReadBytes, Seeking};
use console_core_iteration::{Step, iterate};
use console_core_matroska_blocks::elements::SeekEntry;
use console_core_never::Never;

use crate::ebml::{ElementHeader, Walker};
use crate::fault::{DecodeError, Invalid, MissingElement, Unsupported};
use crate::ids;
use crate::segment::{
  InfoElement, TrackElement, read_cues, read_doc_type, read_info, read_seek_head, read_tags,
  read_tracks,
};
use crate::tracks::{build_tracks, default_durations, time_base};

#[derive(Debug)]
pub(crate) struct ClusterState {
  pub(crate) timestamp: Option<u64>,
  pub(crate) end: Option<u64>,
}

pub(crate) struct ClusterIndex {
  pub(crate) timestamp: u64,
  pub(crate) position: u64,
}

pub(crate) struct Opened {
  pub(crate) walker: Walker,
  pub(crate) tracks: Vec<Track>,
  pub(crate) default_durations: HashMap<u32, u64>,
  pub(crate) time_base: TimeBase,
  pub(crate) clusters: Vec<ClusterIndex>,
  pub(crate) metadata: MetadataLog,
  pub(crate) current_cluster: Option<ClusterState>,
}

struct Visit {
  id: u32,
  position: u64,
}

struct Scan {
  tracks: Option<Vec<TrackElement>>,
  info: Option<InfoElement>,
  clusters: Vec<ClusterIndex>,
  metadata: MetadataLog,
  visits: Vec<Visit>,
  current_cluster: Option<ClusterState>,
}

enum Flow {
  Onward,
  Finished,
}

impl Scan {
  fn new() -> Result<Self, Never> {
    Ok(Self {
      tracks: None,
      info: None,
      clusters: Vec::new(),
      metadata: MetadataLog::default(),
      visits: Vec::new(),
      current_cluster: None,
    })
  }

  fn absorb(&mut self, id: u32, data: &[u8], segment_pos: u64) -> Result<(), DecodeError> {
    match id {
      ids::SEEK_HEAD => {
        let seeks = read_seek_head(data)?;

        self.visits.extend(seeks.into_iter().filter_map(|seek| {
          let Ok(visit) = visit_of(&seek, segment_pos);

          visit
        }));
      }
      ids::TRACKS => {
        let tracks = read_tracks(data)?;

        self.tracks = Some(tracks);
      }
      ids::INFO => {
        let Ok(info) = read_info(data);

        self.info = Some(info);
      }
      ids::CUES => {
        let points = read_cues(data)?;

        self
          .clusters
          .extend(points.into_iter().map(|point| ClusterIndex {
            timestamp: point.time,
            position: segment_pos.saturating_add(point.cluster_position),
          }));
      }
      ids::TAGS => {
        let revision = read_tags(data)?;

        let Ok(()) = self.metadata.push(revision);
      }
      _ => {}
    }

    Ok(())
  }
}

fn visit_of(seek: &SeekEntry, segment_pos: u64) -> Result<Option<Visit>, Never> {
  Ok(match u32::try_from(seek.identifier) {
    Ok(id @ (ids::TRACKS | ids::INFO | ids::TAGS | ids::CUES)) => Some(Visit {
      id,
      position: segment_pos.saturating_add(seek.position),
    }),
    Ok(_not_asked_for) => None,
    Err(_wider_than_an_element_id) => None,
  })
}

pub(crate) fn open(mut reader: MediaSourceStream) -> Result<Opened, DecodeError> {
  let total_len = stream_length(&mut reader)?;
  let Ok(mut walker) = Walker::new(reader, total_len);

  check_header(&mut walker)?;

  let segment_pos = find_segment(&mut walker)?;
  let mut scan = scan_front(&mut walker, segment_pos)?;

  match total_len {
    Some(_) => follow_visits(&mut walker, &mut scan, segment_pos)?,
    None => {}
  }

  let tracks = scan
    .tracks
    .take()
    .ok_or(DecodeError::Absent(MissingElement::Tracks))?;
  let walker_2 = match total_len {
    Some(_) => walker.rewind(segment_pos)?,
    None => walker,
  };
  let info = scan
    .info
    .take()
    .ok_or(DecodeError::Absent(MissingElement::Info))?;

  finish(walker_2, scan, tracks, &info)
}

fn stream_length(reader: &mut MediaSourceStream) -> Result<Option<u64>, DecodeError> {
  match reader.seeking() {
    Seeking::Anywhere => {
      let at = reader.pos();
      let len = reader
        .byte_len()
        .ok_or(DecodeError::Seek(SeekErrorKind::Unseekable))?;

      reader.seek(SeekFrom::Start(at))?;

      Ok(Some(len))
    }
    Seeking::ForwardOnly => Ok(None),
  }
}

fn check_header(walker: &mut Walker) -> Result<(), DecodeError> {
  let header = walker.read_header()?;
  let header_2 = header.ok_or(DecodeError::EndOfStream)?;

  match header_2.id {
    ids::EBML => {
      let data = walker.read_payload(&header_2)?;
      let doc_type = read_doc_type(&data)?;

      match doc_type.as_str() {
        "matroska" | "webm" => Ok(()),
        _ => Err(DecodeError::Unsupported(Unsupported::NotMatroska)),
      }
    }
    _ => Err(DecodeError::Invalid(Invalid::UnexpectedElement)),
  }
}

fn find_segment(walker: &mut Walker) -> Result<u64, DecodeError> {
  let first = walker.read_header()?;

  match first {
    Some(ElementHeader {
      id: ids::SEGMENT,
      data_pos,
      ..
    }) => Ok(data_pos),
    Some(_not_a_segment) => Err(DecodeError::Unsupported(Unsupported::MissingSegment)),
    None => Err(DecodeError::Unsupported(Unsupported::MissingSegment)),
  }
}

fn absorb_header(
  walker: &mut Walker,
  scan: &mut Scan,
  header: ElementHeader,
  segment_pos: u64,
) -> Result<Flow, DecodeError> {
  match header.id {
    ids::SEEK_HEAD | ids::TRACKS | ids::INFO | ids::CUES | ids::TAGS => {
      let data = walker.read_payload(&header)?;

      scan.absorb(header.id, &data, segment_pos)?;

      Ok(Flow::Onward)
    }
    ids::CLUSTER => {
      scan.current_cluster = Some(ClusterState {
        timestamp: None,
        end: header.end,
      });

      Ok(Flow::Finished)
    }
    _ => {
      walker.skip(&header)?;

      Ok(Flow::Onward)
    }
  }
}

fn scan_front(walker: &mut Walker, segment_pos: u64) -> Result<Scan, DecodeError> {
  let Ok(scan) = Scan::new();
  iterate((walker, scan), |(walker_2, mut scan_2)| {
    let outcome = match walker_2.read_header() {
      Ok(Some(header)) => absorb_header(walker_2, &mut scan_2, header, segment_pos),
      Ok(None) => Ok(Flow::Finished),
      Err(_no_more_headers_to_read) => Ok(Flow::Finished),
    };

    Ok(match outcome {
      Ok(Flow::Onward) => Step::Again((walker_2, scan_2)),
      Ok(Flow::Finished) => Step::Halt(Ok(scan_2)),
      Err(fault) => Step::Halt(Err(fault)),
    })
  })
  .map_err(DecodeError::from)
  .and_then(std::convert::identity)
}

fn follow(
  walker: &mut Walker,
  scan: &mut Scan,
  visit: &Visit,
  segment_pos: u64,
) -> Result<(), DecodeError> {
  walker.seek(visit.position)?;

  let header = walker.read_header()?;
  let header_2 = header.ok_or(DecodeError::EndOfStream)?;

  match header_2.id.cmp(&visit.id) {
    Ordering::Equal => {
      let data = walker.read_payload(&header_2)?;

      scan.absorb(header_2.id, &data, segment_pos)
    }
    Ordering::Less | Ordering::Greater => Err(DecodeError::Invalid(Invalid::UnexpectedElement)),
  }
}

fn follow_visits(
  walker: &mut Walker,
  scan: &mut Scan,
  segment_pos: u64,
) -> Result<(), DecodeError> {
  let mut visits = std::mem::take(&mut scan.visits);

  visits.sort_by_key(|visit| visit.position);
  visits
    .iter()
    .try_fold((walker, scan), |(walker_2, scan_2), visit| {
      follow(walker_2, scan_2, visit, segment_pos)?;

      Ok::<_, DecodeError>((walker_2, scan_2))
    })?;

  Ok(())
}

fn finish(
  walker: Walker,
  scan: Scan,
  tracks: Vec<TrackElement>,
  info: &InfoElement,
) -> Result<Opened, DecodeError> {
  let time_base = time_base(info)?;
  let default_durations = default_durations(&tracks, info)?;
  let built = build_tracks(tracks, info, time_base)?;

  Ok(Opened {
    walker,
    tracks: built,
    default_durations,
    time_base,
    clusters: scan.clusters,
    metadata: scan.metadata,
    current_cluster: scan.current_cluster,
  })
}
