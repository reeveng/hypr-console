//! Segment-level metadata: Info, SeekHead, Cues, Chapters, Tags, Attachments -- types and parsers
//! for their in-memory element data.
//!
//! Each parser is one walk over a master element's children with a `match` on the id, and an
//! element a file leaves out keeps the default RFC 9559 gives it. Chapters and simple tags nest,
//! and a nested element is read without the parser reaching itself: the elements are read a level
//! at a time, each level from the children the one above it found, and the levels are then put
//! together from the deepest up. A level is strictly inside the one above it, so the levels end
//! where the file's nesting does.

use std::cmp::Reverse;

use console_core_matroska_blocks::elements::{
  audio_track_fields, cue_track_position, seek_entries,
};
use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};

use crate::codec::map_codec;
use crate::ebml::width;
use crate::ids::*;
use crate::track::{
  AudioInfo, Colour, ContentEncoding, Degrees, LuminanceNits, MasteringMetadata, Projection,
  Track, TrackKind, VideoInfo, gcd,
};
use console_core_extensible_binary_meta_language::{Children, float, int, string, uint};

const MILLISECOND: u64 = 1_000_000;

const NANOSECONDS: u64 = 1_000_000_000;

const WHOLE_SEGMENT: u64 = 50;

const ENCRYPTED: u64 = 1;

const HEADER_STRIPPING: u64 = 3;

const PRIVATE_SCOPE: u64 = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct SegmentInfo {
  pub timestamp_scale: u64,
  pub duration: Option<f64>,
  pub title: Option<String>,
  pub muxing_app: String,
  pub writing_app: String,
  pub date_utc: Option<i64>,
  pub segment_uuid: Option<[u8; 16]>,
}

impl Default for SegmentInfo {
  fn default() -> Self {
    Self {
      timestamp_scale: MILLISECOND,
      duration: None,
      title: None,
      muxing_app: String::new(),
      writing_app: String::new(),
      date_utc: None,
      segment_uuid: None,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeekEntry {
  pub id: u32,
  pub position: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CuePosition {
  pub track: u64,
  pub cluster_position: u64,
  pub relative_position: Option<u64>,
  pub block_number: Option<u64>,
  pub duration: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CuePoint {
  pub time: u64,
  pub positions: Vec<CuePosition>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClusterInfo {
  pub offset: u64,
  pub data_start: u64,
  pub end: u64,
  pub timestamp: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChapterDisplay {
  pub string: String,
  pub languages: Vec<String>,
  pub languages_bcp47: Vec<String>,
  pub countries: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "ChapterFlagHidden and ChapterFlagEnabled are two elements a muxer writes independently, and the string UID and the end time beside them are elements a file may leave out; every combination is one a ChapterAtom may hold"
  )
)]
pub struct Chapter {
  pub uid: u64,
  pub string_uid: Option<String>,
  pub start_ns: u64,
  pub end_ns: Option<u64>,
  pub hidden: bool,
  pub enabled: bool,
  pub displays: Vec<ChapterDisplay>,
  pub children: Vec<Chapter>,
}

impl Chapter {
  pub fn title(&self) -> Result<Option<&str>, Never> {
    Ok(self.displays.first().map(|display| display.string.as_str()))
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "EditionFlagHidden, EditionFlagDefault and EditionFlagOrdered are three elements a muxer writes independently, and an EditionUID is one a file may leave out; every combination is one an EditionEntry may hold"
  )
)]
pub struct Edition {
  pub uid: Option<u64>,
  pub hidden: bool,
  pub default: bool,
  pub ordered: bool,
  pub chapters: Vec<Chapter>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TagTargets {
  pub type_value: u64,
  pub type_name: Option<String>,
  pub track_uids: Vec<u64>,
  pub edition_uids: Vec<u64>,
  pub chapter_uids: Vec<u64>,
  pub attachment_uids: Vec<u64>,
}

impl Default for TagTargets {
  fn default() -> Self {
    Self {
      type_value: WHOLE_SEGMENT,
      type_name: None,
      track_uids: Vec::new(),
      edition_uids: Vec::new(),
      chapter_uids: Vec::new(),
      attachment_uids: Vec::new(),
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "TagDefault is an element of its own, and TagString and TagBinary are two elements a SimpleTag may hold, either, both or neither; the flag says nothing about whether the values are there"
  )
)]
pub struct SimpleTag {
  pub name: String,
  pub language: String,
  pub default: bool,
  pub string: Option<String>,
  pub binary: Option<Vec<u8>>,
  pub children: Vec<SimpleTag>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tag {
  pub targets: TagTargets,
  pub simple: Vec<SimpleTag>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Attachment {
  pub uid: u64,
  pub name: String,
  pub media_type: String,
  pub description: Option<String>,
  pub data_offset: u64,
  pub data_size: u64,
}

type Depth<'a, T> = Vec<(u32, T, Vec<&'a [u8]>)>;

type Read<'a, T> = fn(&'a [u8]) -> Result<(T, Vec<&'a [u8]>), Never>;

fn nested<'a, T>(
  roots: Vec<&'a [u8]>,
  read: Read<'a, T>,
  attach: fn(&mut T, T) -> Result<(), Never>,
) -> Result<Vec<T>, Never> {
  let largest_root = roots
    .iter()
    .map(|root| {
      let Ok(size) = width(root);
      size
    })
    .max();
  let max_levels = match largest_root {
    Some(bytes) => bytes.saturating_add(1),
    None => 0,
  };
  let mut level: Depth<'a, T> = roots
    .into_iter()
    .map(|data| {
      let Ok((node, inner)) = read(data);

      (0, node, inner)
    })
    .collect();
  let mut levels = Vec::new();

  for _ in 0..max_levels {
    let next: Depth<'a, T> = level
      .iter()
      .zip(0u32..)
      .flat_map(|((_, _, inner), parent)| {
        inner.iter().map(move |data| {
          let Ok((node, deeper)) = read(data);

          (parent, node, deeper)
        })
      })
      .collect();

    levels.push(level);

    match ::core::primitive::bool::then_some(next.is_empty(), ()) {
      ::core::option::Option::Some(()) => break,
      ::core::option::Option::None => level = next,
    }
  }

  let mut below: Vec<(u32, T)> = Vec::new();

  for stored_level in levels.into_iter().rev() {
    let mut here: Vec<(u32, T)> = stored_level
      .into_iter()
      .map(|(parent, node, _)| (parent, node))
      .collect();

    for (parent, child) in below {
      let Ok(at) = index(parent);

      match here.get_mut(at) {
        Some((_, held)) => {
          let Ok(()) = attach(held, child);
        }
        None => {}
      }
    }

    below = here;
  }

  Ok(below.into_iter().map(|(_, node)| node).collect())
}

fn sixteen(data: &[u8]) -> Result<Option<[u8; 16]>, Never> {
  Ok(match <[u8; 16]>::try_from(data) {
    Ok(uuid) => Some(uuid),
    Err(_another_length) => None,
  })
}

fn four(data: &[u8]) -> Result<Option<[u8; 4]>, Never> {
  Ok(match <[u8; 4]>::try_from(data) {
    Ok(space) => Some(space),
    Err(_another_length) => None,
  })
}

fn narrow(data: &[u8]) -> Result<u32, Never> {
  let Ok(value) = uint(data);

  wrapped::<u64, u32>(value)
}

pub(crate) fn parse_info(data: &[u8]) -> Result<SegmentInfo, Never> {
  let mut info = SegmentInfo::default();

  for child in Children::from(data) {
    match child.id {
      TIMESTAMP_SCALE => {
        let Ok(scale) = uint(child.data);

        match scale {
          0 => {}
          _scaled => info.timestamp_scale = scale,
        }
      }
      DURATION => {
        let Ok(duration) = float(child.data);

        info.duration = Some(duration).filter(|ticks| ticks.is_finite() && *ticks >= 0.0);
      }
      TITLE => {
        let Ok(title) = string(child.data);

        info.title = Some(title);
      }
      MUXING_APP => {
        let Ok(app) = string(child.data);

        info.muxing_app = app;
      }
      WRITING_APP => {
        let Ok(app) = string(child.data);

        info.writing_app = app;
      }
      DATE_UTC => {
        let Ok(date) = int(child.data);

        info.date_utc = Some(date);
      }
      SEGMENT_UUID => {
        let Ok(uuid) = sixteen(child.data);

        info.segment_uuid = uuid;
      }
      _ => {}
    }
  }

  Ok(info)
}

pub(crate) fn parse_seekhead(data: &[u8]) -> Result<Vec<SeekEntry>, Never> {
  let Ok(entries) = seek_entries(data);

  Ok(
    entries
      .filter_map(|entry| match entry {
        Ok(entry_2) => {
          let Ok(identifier) = wrapped::<u64, u32>(entry_2.identifier);

          Some(SeekEntry {
            id: identifier,
            position: entry_2.position,
          })
        }
        Err(_missing_field) => None,
      })
      .collect(),
  )
}

fn parse_cue_position(data: &[u8]) -> Result<Option<CuePosition>, Never> {
  let Ok(position) = cue_track_position(data);

  Ok(match (position.track, position.cluster_position) {
    (Some(0) | None, _) | (_, None) => None,
    (Some(track), Some(cluster_position)) => Some(CuePosition {
      track,
      cluster_position,
      relative_position: position.relative_position,
      block_number: position.block_number,
      duration: position.duration,
    }),
  })
}

fn parse_cue_point(data: &[u8]) -> Result<Option<CuePoint>, Never> {
  let mut time = None;
  let mut positions = Vec::new();

  for child in Children::from(data) {
    match child.id {
      CUE_TIME => {
        let Ok(said) = uint(child.data);

        time = Some(said);
      }
      CUE_TRACK_POSITIONS => {
        let Ok(position) = parse_cue_position(child.data);

        positions.extend(position);
      }
      _ => {}
    }
  }

  Ok(match (time, positions.is_empty()) {
    (Some(time_2), false) => Some(CuePoint {
      time: time_2,
      positions,
    }),
    (Some(_) | None, true) | (None, false) => None,
  })
}

pub(crate) fn parse_cues(data: &[u8], out: &mut Vec<CuePoint>) -> Result<(), Never> {
  for point in Children::from(data).filter(|child| child.id == CUE_POINT) {
    let Ok(read) = parse_cue_point(point.data);

    out.extend(read);
  }

  Ok(())
}

fn parse_display(data: &[u8]) -> Result<ChapterDisplay, Never> {
  let mut display = ChapterDisplay::default();

  for child in Children::from(data) {
    let Ok(said) = string(child.data);

    match child.id {
      CHAP_STRING => display.string = said,
      CHAP_LANGUAGE => display.languages.push(said),
      CHAP_LANGUAGE_BCP47 => display.languages_bcp47.push(said),
      CHAP_COUNTRY => display.countries.push(said),
      _ => {}
    }
  }

  match ::core::primitive::bool::then_some(display.languages.is_empty(), ()) {
    ::core::option::Option::Some(()) => display.languages.push(String::from("eng")),
    ::core::option::Option::None => {}
  }

  Ok(display)
}

fn parse_chapter(data: &[u8]) -> Result<(Chapter, Vec<&[u8]>), Never> {
  let mut chapter = Chapter {
    uid: 0,
    string_uid: None,
    start_ns: 0,
    end_ns: None,
    hidden: false,
    enabled: true,
    displays: Vec::new(),
    children: Vec::new(),
  };
  let mut atoms = Vec::new();

  for child in Children::from(data) {
    let Ok(value) = uint(child.data);

    match child.id {
      CHAPTER_UID => chapter.uid = value,
      CHAPTER_STRING_UID => {
        let Ok(said) = string(child.data);

        chapter.string_uid = Some(said);
      }
      CHAPTER_TIME_START => chapter.start_ns = value,
      CHAPTER_TIME_END => chapter.end_ns = Some(value),
      CHAPTER_FLAG_HIDDEN => chapter.hidden = value != 0,
      CHAPTER_FLAG_ENABLED => chapter.enabled = value != 0,
      CHAPTER_DISPLAY => {
        let Ok(display) = parse_display(child.data);

        chapter.displays.push(display);
      }
      CHAPTER_ATOM => atoms.push(child.data),
      _ => {}
    }
  }

  Ok((chapter, atoms))
}

fn adopt_chapter(parent: &mut Chapter, child: Chapter) -> Result<(), Never> {
  parent.children.push(child);

  Ok(())
}

fn parse_edition(data: &[u8]) -> Result<Edition, Never> {
  let mut edition = Edition::default();
  let mut atoms = Vec::new();

  for child in Children::from(data) {
    let Ok(value) = uint(child.data);

    match child.id {
      EDITION_UID => edition.uid = Some(value),
      EDITION_FLAG_HIDDEN => edition.hidden = value != 0,
      EDITION_FLAG_DEFAULT => edition.default = value != 0,
      EDITION_FLAG_ORDERED => edition.ordered = value != 0,
      CHAPTER_ATOM => atoms.push(child.data),
      _ => {}
    }
  }

  let Ok(chapters) = nested(atoms, parse_chapter, adopt_chapter);

  edition.chapters = chapters;

  Ok(edition)
}

pub(crate) fn parse_chapters(data: &[u8], out: &mut Vec<Edition>) -> Result<(), Never> {
  for entry in Children::from(data).filter(|child| child.id == EDITION_ENTRY) {
    let Ok(edition) = parse_edition(entry.data);

    out.push(edition);
  }

  Ok(())
}

fn parse_simple_tag(data: &[u8]) -> Result<(SimpleTag, Vec<&[u8]>), Never> {
  let mut tag = SimpleTag {
    name: String::new(),
    language: String::from("und"),
    default: true,
    string: None,
    binary: None,
    children: Vec::new(),
  };
  let mut inner = Vec::new();

  for child in Children::from(data) {
    let Ok(said) = string(child.data);

    match child.id {
      TAG_NAME => tag.name = said,
      TAG_LANGUAGE => tag.language = said,
      TAG_DEFAULT => {
        let Ok(value) = uint(child.data);

        tag.default = value != 0;
      }
      TAG_STRING => tag.string = Some(said),
      TAG_BINARY => tag.binary = Some(child.data.to_vec()),
      SIMPLE_TAG => inner.push(child.data),
      _ => {}
    }
  }

  Ok((tag, inner))
}

fn adopt_tag(parent: &mut SimpleTag, child: SimpleTag) -> Result<(), Never> {
  parent.children.push(child);

  Ok(())
}

fn parse_targets(data: &[u8]) -> Result<TagTargets, Never> {
  let mut targets = TagTargets::default();

  for child in Children::from(data) {
    let Ok(value) = uint(child.data);

    match child.id {
      TARGET_TYPE_VALUE => targets.type_value = value,
      TARGET_TYPE => {
        let Ok(said) = string(child.data);

        targets.type_name = Some(said);
      }
      TAG_TRACK_UID => targets.track_uids.push(value),
      TAG_EDITION_UID => targets.edition_uids.push(value),
      TAG_CHAPTER_UID => targets.chapter_uids.push(value),
      TAG_ATTACHMENT_UID => targets.attachment_uids.push(value),
      _ => {}
    }
  }

  Ok(targets)
}

fn parse_tag(data: &[u8]) -> Result<Tag, Never> {
  let mut tag = Tag::default();
  let mut simple = Vec::new();

  for child in Children::from(data) {
    match child.id {
      TARGETS => {
        let Ok(targets) = parse_targets(child.data);

        tag.targets = targets;
      }
      SIMPLE_TAG => simple.push(child.data),
      _ => {}
    }
  }

  let Ok(simple_2) = nested(simple, parse_simple_tag, adopt_tag);

  tag.simple = simple_2;

  Ok(tag)
}

pub(crate) fn parse_tags(data: &[u8], out: &mut Vec<Tag>) -> Result<(), Never> {
  for read in Children::from(data).filter(|child| child.id == TAG) {
    let Ok(tag) = parse_tag(read.data);

    out.push(tag);
  }

  Ok(())
}

fn parse_mastering(data: &[u8]) -> Result<MasteringMetadata, Never> {
  let mut mastering = MasteringMetadata::default();
  let [red, green, blue] = &mut mastering.primaries;

  for child in Children::from(data) {
    let Ok(value) = float(child.data);

    match child.id {
      PRIMARY_R_X => red.0 = value,
      PRIMARY_R_Y => red.1 = value,
      PRIMARY_G_X => green.0 = value,
      PRIMARY_G_Y => green.1 = value,
      PRIMARY_B_X => blue.0 = value,
      PRIMARY_B_Y => blue.1 = value,
      WHITE_POINT_X => mastering.white_point.0 = value,
      WHITE_POINT_Y => mastering.white_point.1 = value,
      LUMINANCE_MAX => mastering.luminance_max = LuminanceNits(value),
      LUMINANCE_MIN => mastering.luminance_min = LuminanceNits(value),
      _ => {}
    }
  }

  Ok(mastering)
}

fn parse_colour(data: &[u8]) -> Result<Colour, Never> {
  let mut colour = Colour::default();

  for child in Children::from(data) {
    let Ok(value) = uint(child.data);
    let said = Some(value);

    match child.id {
      MATRIX_COEFFICIENTS => colour.matrix_coefficients = said,
      BITS_PER_CHANNEL => colour.bits_per_channel = said,
      CHROMA_SUBSAMPLING_HORZ => colour.chroma_subsampling_horz = said,
      CHROMA_SUBSAMPLING_VERT => colour.chroma_subsampling_vert = said,
      CB_SUBSAMPLING_HORZ => colour.cb_subsampling_horz = said,
      CB_SUBSAMPLING_VERT => colour.cb_subsampling_vert = said,
      CHROMA_SITING_HORZ => colour.chroma_siting_horz = said,
      CHROMA_SITING_VERT => colour.chroma_siting_vert = said,
      RANGE => colour.range = said,
      TRANSFER_CHARACTERISTICS => colour.transfer_characteristics = said,
      PRIMARIES => colour.primaries = said,
      MAX_CLL => colour.max_cll = said,
      MAX_FALL => colour.max_fall = said,
      MASTERING_METADATA => {
        let Ok(mastering) = parse_mastering(child.data);

        colour.mastering = Some(mastering);
      }
      _ => {}
    }
  }

  Ok(colour)
}

fn parse_projection(data: &[u8]) -> Result<Projection, Never> {
  let mut projection = Projection::default();

  for child in Children::from(data) {
    let Ok(angle) = float(child.data);

    match child.id {
      PROJECTION_TYPE => {
        let Ok(kind) = uint(child.data);

        projection.projection_type = kind;
      }
      PROJECTION_POSE_YAW => projection.yaw = Degrees(angle),
      PROJECTION_POSE_PITCH => projection.pitch = Degrees(angle),
      PROJECTION_POSE_ROLL => projection.roll = Degrees(angle),
      _ => {}
    }
  }

  Ok(projection)
}

fn parse_video(data: &[u8]) -> Result<VideoInfo, Never> {
  let mut video = VideoInfo::default();

  for child in Children::from(data) {
    let Ok(value) = uint(child.data);
    let Ok(pixels) = narrow(child.data);

    match child.id {
      PIXEL_WIDTH => video.pixel_width = pixels,
      PIXEL_HEIGHT => video.pixel_height = pixels,
      DISPLAY_WIDTH => video.display_width = Some(pixels),
      DISPLAY_HEIGHT => video.display_height = Some(pixels),
      DISPLAY_UNIT => video.display_unit = value,
      PIXEL_CROP_TOP => video.crop.0 = pixels,
      PIXEL_CROP_BOTTOM => video.crop.1 = pixels,
      PIXEL_CROP_LEFT => video.crop.2 = pixels,
      PIXEL_CROP_RIGHT => video.crop.3 = pixels,
      FLAG_INTERLACED => video.flag_interlaced = value,
      FIELD_ORDER => video.field_order = Some(value),
      STEREO_MODE => video.stereo_mode = Some(value),
      ALPHA_MODE => video.alpha_mode = value,
      COLOUR_SPACE => {
        let Ok(space) = four(child.data);

        video.colour_space = space;
      }
      COLOUR => {
        let Ok(colour) = parse_colour(child.data);

        video.colour = Some(colour);
      }
      PROJECTION => {
        let Ok(projection) = parse_projection(child.data);

        video.projection = Some(projection);
      }
      _ => {}
    }
  }

  Ok(video)
}

fn parse_compression(data: &[u8]) -> Result<(u64, Vec<u8>), Never> {
  let (mut algo, mut settings) = (0u64, Vec::new());

  for child in Children::from(data) {
    match child.id {
      CONTENT_COMP_ALGO => {
        let Ok(said) = uint(child.data);

        algo = said;
      }
      CONTENT_COMP_SETTINGS => settings = child.data.to_vec(),
      _ => {}
    }
  }

  Ok((algo, settings))
}

fn parse_encoding(data: &[u8]) -> Result<(u64, ContentEncoding), Never> {
  let (mut order, mut scope, mut kind) = (0u64, 1u64, 0u64);
  let mut compression = None;

  for child in Children::from(data) {
    let Ok(value) = uint(child.data);

    match child.id {
      CONTENT_ENCODING_ORDER => order = value,
      CONTENT_ENCODING_SCOPE => scope = value,
      CONTENT_ENCODING_TYPE => kind = value,
      CONTENT_COMPRESSION => {
        let Ok(read) = parse_compression(child.data);

        compression = Some(read);
      }
      _ => {}
    }
  }

  let (algo, settings) = match compression {
    Some(read) => read,
    None => (0, Vec::new()),
  };

  Ok((
    order,
    match (kind, algo) {
      (ENCRYPTED, _) => ContentEncoding::Encryption { scope },
      (_, HEADER_STRIPPING) => ContentEncoding::HeaderStripping {
        bytes: settings,
        scope,
      },
      (_, _) => ContentEncoding::Compression {
        algo,
        settings,
        scope,
      },
    },
  ))
}

fn parse_encodings(data: &[u8]) -> Result<Vec<ContentEncoding>, Never> {
  let mut ordered: Vec<(u64, ContentEncoding)> = Children::from(data)
    .filter(|child| child.id == CONTENT_ENCODING)
    .map(|child| {
      let Ok(read) = parse_encoding(child.data);

      read
    })
    .collect();

  ordered.sort_by_key(|(order, _)| Reverse(*order));

  Ok(ordered.into_iter().map(|(_, encoding)| encoding).collect())
}

fn blank_track(timebase: (u64, u64)) -> Result<Track, Never> {
  Ok(Track {
    number: 0,
    uid: 0,
    kind: TrackKind::Other(0),
    enabled: true,
    default: true,
    forced: false,
    hearing_impaired: false,
    lacing: true,
    default_duration_ns: None,
    name: String::new(),
    language: String::from("eng"),
    language_bcp47: None,
    codec_id: String::new(),
    codec_private: Vec::new(),
    codec_name: String::new(),
    codec: crate::Codec::Other(String::new()),
    codec_delay_ns: 0,
    seek_pre_roll_ns: 0,
    video: None,
    audio: None,
    encodings: Vec::new(),
    timebase,
    samples: Vec::new(),
    sync: Vec::new(),
  })
}

fn read_track_child(track: &mut Track, id: u32, data: &[u8]) -> Result<(), Never> {
  let Ok(value) = uint(data);
  let Ok(said) = string(data);

  match id {
    TRACK_NUMBER => track.number = value,
    TRACK_UID => track.uid = value,
    TRACK_TYPE => {
      let Ok(kind) = TrackKind::from_type(value);

      track.kind = kind;
    }
    FLAG_ENABLED => track.enabled = value != 0,
    FLAG_DEFAULT => track.default = value != 0,
    FLAG_FORCED => track.forced = value != 0,
    FLAG_HEARING_IMPAIRED => track.hearing_impaired = value != 0,
    FLAG_LACING => track.lacing = value != 0,
    DEFAULT_DURATION => {
      track.default_duration_ns = Some(value).filter(|nanoseconds| *nanoseconds > 0)
    }
    NAME => track.name = said,
    LANGUAGE => track.language = said,
    LANGUAGE_BCP47 => track.language_bcp47 = Some(said),
    CODEC_ID => track.codec_id = said,
    CODEC_PRIVATE => track.codec_private = data.to_vec(),
    CODEC_NAME => track.codec_name = said,
    CODEC_DELAY => track.codec_delay_ns = value,
    SEEK_PRE_ROLL => track.seek_pre_roll_ns = value,
    VIDEO => {
      let Ok(video) = parse_video(data);

      track.video = Some(video);
    }
    AUDIO => {
      let Ok(fields) = audio_track_fields(data);

      track.audio = Some(AudioInfo::from(fields));
    }
    CONTENT_ENCODINGS => {
      let Ok(encodings) = parse_encodings(data);

      track.encodings = encodings;
    }
    _ => {}
  }

  Ok(())
}

fn restore_private(track: &mut Track) -> Result<(), Never> {
  let stripped: Vec<u8> = track
    .encodings
    .iter()
    .filter_map(|encoding| match encoding {
      ContentEncoding::HeaderStripping { bytes, scope } => match scope & PRIVATE_SCOPE {
        0 => None,
        _private => Some(bytes.as_slice()),
      },
      ContentEncoding::Compression { .. } | ContentEncoding::Encryption { .. } => None,
    })
    .rev()
    .flatten()
    .copied()
    .collect();

  match ::core::primitive::bool::then_some(stripped.is_empty(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      let mut restored = stripped;

      restored.extend_from_slice(&track.codec_private);
      track.codec_private = restored;
    }
  }

  Ok(())
}

fn parse_track(data: &[u8], timebase: (u64, u64)) -> Result<Option<Track>, Never> {
  let Ok(mut track) = blank_track(timebase);

  for child in Children::from(data) {
    let Ok(()) = read_track_child(&mut track, child.id, child.data);
  }

  match track.number {
    0 => return Ok(None),
    _numbered => {}
  }

  match (track.kind, &track.audio) {
    (TrackKind::Audio, None) => track.audio = Some(AudioInfo::default()),
    (_, _) => {}
  }

  let Ok(()) = restore_private(&mut track);
  let Ok(codec) = map_codec(&track.codec_id, &track.codec_private, track.audio.as_ref());

  track.codec = codec;

  Ok(Some(track))
}

pub(crate) fn parse_tracks(data: &[u8], timestamp_scale: u64) -> Result<Vec<Track>, Never> {
  let Ok(common) = gcd([timestamp_scale, NANOSECONDS]);
  let timebase = (timestamp_scale / common, NANOSECONDS / common);

  Ok(
    Children::from(data)
      .filter(|child| child.id == TRACK_ENTRY)
      .filter_map(|entry| {
        let Ok(track) = parse_track(entry.data, timebase);

        track
      })
      .collect(),
  )
}
