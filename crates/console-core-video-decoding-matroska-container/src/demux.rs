//! A Matroska or WebM file opened: its EBML header, its first Segment, and the
//! sample tables a scan of its clusters builds.
//!
//! The EBML header is looked for in the first window of the file rather than
//! at its first byte, as upstream did, and must name `matroska` or `webm`.
//! The first Segment after it is walked by `segment`; an element of unknown
//! size before it cannot be stepped over, and is a fault.
//!
//! What is read afterwards is read from the tables: a sample's bytes are asked
//! of the source where the scan found them, with header stripping undone, and
//! a track whose frames are compressed or encrypted is listed but not read.
//! Reading packets in file order and seeking among them went with the writer,
//! to `forks/console-core-video-decoding-matroska-container-writer`, which is
//! the only thing that read a file that way.

use std::collections::BTreeSet;
use std::io;

use console_core_byte_sources::resize_read_buffer;

use console_core_byte_sources::ByteSource;
use console_core_extensible_binary_meta_language::{Children, Size, string, uint};
use console_core_never::Never;
use console_core_number_conversion::{Float, index, whole_u64, wrapped};

use crate::clusters::finish_tracks;
use crate::ebml::width;
use crate::error::{Error, Invalid, NotMatroska, Unsupported, Warning};
use crate::ids::{
  DOC_TYPE, DOC_TYPE_READ_VERSION, DOC_TYPE_VERSION, EBML_MAX_ID_LENGTH, EBML_MAX_SIZE_LENGTH,
  SEGMENT,
};
use crate::meta::{Attachment, ClusterInfo, CuePoint, Edition, SeekEntry, SegmentInfo, Tag};
use crate::segment::{Pass, Walking};
use crate::states::{ContainerKind, ElementBounds, FrameAccess};
use crate::track::{Track, TrackKind};
use crate::window::{Header, Span, WINDOW, Window, walk};

const EBML_MAGIC: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "building the sample tables and checking CRC-32s are two separate costs a caller chooses between, and every combination of the two is an open someone may want"
  )
)]
pub struct OpenOptions {
  pub index: bool,
  pub verify_crc: bool,
}

impl Default for OpenOptions {
  fn default() -> Self {
    Self {
      index: true,
      verify_crc: false,
    }
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "`live` is what the Segment's size said and `indexed` is what the open was asked to do, and the first cluster's position is where one was found; a live file may have no cluster yet and an indexed one may have none at all, so every combination is a file that can be opened"
  )
)]
pub struct MkvFile {
  pub doc_type: String,
  pub doc_type_version: u64,
  pub doc_type_read_version: u64,
  pub segment_data_start: u64,
  pub segment_end: u64,
  pub live: bool,
  pub info: SegmentInfo,
  pub tracks: Vec<Track>,
  pub seek_head: Vec<SeekEntry>,
  pub cues: Vec<CuePoint>,
  pub clusters: Vec<ClusterInfo>,
  pub chapters: Vec<Edition>,
  pub tags: Vec<Tag>,
  pub attachments: Vec<Attachment>,
  pub first_cluster: Option<u64>,
  pub indexed: bool,
  pub crc_checked: u32,
  pub crc_errors: Vec<u64>,
  pub warnings: Vec<Warning>,
}

struct EbmlHeader {
  doc_type: String,
  version: u64,
  read_version: u64,
  end: u64,
}

pub fn open<S: ByteSource>(src: S) -> Result<MkvFile, Error> {
  open_with(src, &OpenOptions::default())
}

pub fn open_with<S: ByteSource>(src: S, opts: &OpenOptions) -> Result<MkvFile, Error> {
  let mut window = Window::new(&src)?;
  let ebml = read_ebml_header(&mut window)?;
  let segment = find_segment(&mut window, ebml.end)?;
  let Ok(mut file) = opened(&window, ebml, &segment);

  walk_segment(&mut window, &mut file, opts)?;

  let Ok(()) = finish_tracks(&mut file);

  Ok(file)
}

fn read_ebml_header<S: ByteSource + ?Sized>(
  window: &mut Window<'_, S>,
) -> Result<EbmlHeader, Error> {
  let first = window.peek(Span {
    start: 0,
    end: WINDOW,
  })?;
  let found = first
    .windows(4)
    .zip(0u64..)
    .find(|(four, _)| *four == EBML_MAGIC);
  let start = match found {
    Some((_, at)) => at,
    None => return Err(Error::NotMatroska(NotMatroska::NoHeader)),
  };
  let len = window.len;
  let read = window.header(Span { start, end: len })?;
  let header = match read {
    Some(header) => header,
    None => return Err(Error::NotMatroska(NotMatroska::DamagedHeader)),
  };
  let Ok(body) = header.body(len);
  let data = window.read_vec(body)?;
  let mut said = EbmlHeader {
    doc_type: String::from("matroska"),
    version: 1,
    read_version: 1,
    end: body.end,
  };

  for child in Children::from(data.as_slice()) {
    let Ok(value) = uint(child.data);

    match (child.id, value) {
      (DOC_TYPE, _) => {
        let Ok(doc_type) = string(child.data);

        said.doc_type = doc_type;
      }
      (DOC_TYPE_VERSION, _) => said.version = value,
      (DOC_TYPE_READ_VERSION, _) => said.read_version = value,
      (EBML_MAX_ID_LENGTH, 5..) => return Err(Error::Unsupported(Unsupported::LongIds)),
      (EBML_MAX_SIZE_LENGTH, 9..) => return Err(Error::Unsupported(Unsupported::LongSizes)),
      (_, _) => {}
    }
  }

  match said.doc_type.as_str() {
    "matroska" | "webm" => Ok(said),
    _ => Err(Error::NotMatroska(NotMatroska::DocType(said.doc_type))),
  }
}

fn find_segment<S: ByteSource + ?Sized>(
  window: &mut Window<'_, S>,
  from: u64,
) -> Result<Header, Error> {
  walk((window, from), |(window_2, at)| {
    let len = window_2.len;
    let found = window_2.header(Span {
      start: at,
      end: len,
    })?;
    let header = match found {
      Some(header) => header,
      None => return Err(Error::Invalid(Invalid::NoSegment)),
    };

    match (header.id, header.size) {
      (SEGMENT, _) => Ok(console_core_iteration::Step::Halt(header)),
      (_, Size::Unknown) => Err(Error::Invalid(Invalid::UnknownSizeBeforeSegment)),
      (_, Size::Known(_)) => {
        let Ok(end) = header.end(len);

        Ok(console_core_iteration::Step::Again((window_2, end)))
      }
    }
  })
}

fn opened<S: ByteSource + ?Sized>(
  window: &Window<'_, S>,
  ebml: EbmlHeader,
  segment: &Header,
) -> Result<MkvFile, Never> {
  let Ok(body) = segment.body(window.len);
  let Ok(bounds) = segment.overruns(window.len);
  let warnings = match bounds {
    ElementBounds::Overrun => vec![Warning::SegmentPastEnd],
    ElementBounds::Within => Vec::new(),
  };

  Ok(MkvFile {
    doc_type: ebml.doc_type,
    doc_type_version: ebml.version,
    doc_type_read_version: ebml.read_version,
    segment_data_start: body.start,
    segment_end: body.end,
    live: segment.size == Size::Unknown,
    info: SegmentInfo::default(),
    tracks: Vec::new(),
    seek_head: Vec::new(),
    cues: Vec::new(),
    clusters: Vec::new(),
    chapters: Vec::new(),
    tags: Vec::new(),
    attachments: Vec::new(),
    first_cluster: None,
    indexed: false,
    crc_checked: 0,
    crc_errors: Vec::new(),
    warnings,
  })
}

fn walk_segment<S: ByteSource + ?Sized>(
  window: &mut Window<'_, S>,
  file: &mut MkvFile,
  opts: &OpenOptions,
) -> Result<(), Error> {
  let at = file.segment_data_start;
  let walking = Walking {
    window,
    file,
    options: opts,
    parsed: BTreeSet::new(),
    at,
  };
  let mut walked = walk(walking, Walking::step)?;

  walked.follow_seek_head(Pass::Everything)?;
  walked.follow_seek_head(Pass::Late)?;
  walked.file.cues.sort_by_key(|cue| cue.time);
  walked.file.indexed = opts.index;

  Ok(())
}

impl MkvFile {
  pub fn is_webm(&self) -> Result<ContainerKind, Never> {
    Ok(match self.doc_type.as_str() {
      "webm" => ContainerKind::Webm,
      _ => ContainerKind::Matroska,
    })
  }

  pub fn duration_ns(&self) -> Result<Option<u64>, Never> {
    let scale = self.info.timestamp_scale;

    match self.info.duration {
      Some(ticks) => {
        let Ok(each) = scale.float();
        let Ok(nanoseconds) = whole_u64(ticks * each);

        return Ok(Some(nanoseconds));
      }
      None => {}
    }

    let end = self
      .tracks
      .iter()
      .flat_map(|track| track.samples.iter())
      .map(|sample| {
        let Ok(duration) = wrapped::<u64, i64>(sample.duration);

        sample.pts.wrapping_add(duration)
      })
      .max();

    Ok(end.map(|end_2| {
      let Ok(ticks) = wrapped::<i64, u64>(end_2.max(0));

      ticks.saturating_mul(scale)
    }))
  }

  pub fn track_by_number(&self, number: u64) -> Result<Option<u64>, Never> {
    Ok(
      self
        .tracks
        .iter()
        .zip(0u64..)
        .find(|(track, _)| track.number == number)
        .map(|(_, at)| at),
    )
  }

  pub fn track_of_kind(&self, kind: TrackKind) -> Result<Option<u64>, Never> {
    let mut of_kind = self
      .tracks
      .iter()
      .zip(0u64..)
      .filter(|(track, _)| track.kind == kind);
    let first = of_kind.clone().next().map(|(_, at)| at);

    Ok(match of_kind.find(|(track, _)| track.default) {
      Some((_, at)) => Some(at),
      None => first,
    })
  }

  pub fn read_sample<S: ByteSource + ?Sized>(
    &self,
    src: &S,
    track: u64,
    sample: u32,
  ) -> Result<Vec<u8>, Error> {
    let Ok(track_at) = index(track);
    let Ok(sample_at) = index(sample);
    let held = match self.tracks.get(track_at) {
      Some(held) => held,
      None => return Err(Error::NoSuchTrack(track)),
    };
    let stored = match held.samples.get(sample_at) {
      Some(stored) => stored,
      None => return Err(Error::NoSuchSample(sample)),
    };
    let Ok(access) = held.frames_readable();

    match access {
      FrameAccess::Readable => {}
      FrameAccess::Unavailable => {
        return Err(Error::Unsupported(Unsupported::EncodedFrames {
          track: held.number,
        }));
      }
    }

    let size = u64::from(stored.size);
    let end = stored
      .offset
      .checked_add(size)
      .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))?;
    let source_length = src.len()?;

    match ::core::primitive::bool::then_some(end <= source_length, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        return Err(Error::Io(io::Error::from(io::ErrorKind::UnexpectedEof)));
      }
    }

    let Ok(mut data) = held.frame_prefix(size);
    let Ok(prefix) = width(&data);
    let Ok(from) = index(prefix);

    resize_read_buffer(&mut data, prefix.saturating_add(size))?;

    match data.get_mut(from..) {
      Some(frame) => src.read_at(stored.offset, frame)?,
      None => {}
    }

    Ok(data)
  }

  pub fn keyframe_before(&self, track: u64, time_ns: i64) -> Result<Option<u32>, Never> {
    let Ok(at) = index(track);

    Ok(self.tracks.get(at).and_then(|held| {
      let Ok(ticks) = held.ns_to_ticks(time_ns);
      let Ok(keyframe) = held.keyframe_before_pts(ticks);

      keyframe
    }))
  }
}
