//! A film's boxes read into tracks: what each track is, and where and when
//! each of its samples is.
//!
//! Only the headers are read whole -- `ftyp`, `moov` and every `moof` -- and the
//! media data stays in the file until a sample is asked for. The top level is
//! walked a header at a time, so a film of any length costs one read per box
//! before its sample tables are built. A sample table is the product of five
//! tables that each say one thing about a run of samples -- their durations,
//! their composition offsets, which are sync samples, how many sit in each
//! chunk and how large each is -- and the walk here reads all five in step,
//! once, into one sample at a time. Uncompressed sound is the exception: its
//! samples are single frames, so a chunk of them is kept as one sample rather
//! than as a table entry per frame. A fragmented film's samples come from its
//! `trun` boxes, after the movie's own tables, in the order the fragments are
//! written.

use std::collections::{BTreeMap, BTreeSet};

use console_core_byte_sources::resize_read_buffer;
use console_core_iso_media_boxes::{BoxHeader, BoxView};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::ByteSource;
use crate::bytes::{Cursor, FourCc, boxes, display_text, find, whole_boxes};
use crate::codec::{
  AudioParams, CodecConfig, Entry, Flavour, PcmConfig, SampleEntry, VideoParams,
  parse_sample_entry,
};
use crate::error::{Error, Invalid, Unsupported};

const MAX_SAMPLES: u64 = 1 << 26;

const MAX_HEADER_BOX: u64 = 1 << 30;

const MAX_FTYP: u64 = 1 << 16;

const EMPTY_EDIT: i64 = -1;

const IDENTITY: [i32; 9] = [0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x4000_0000];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Sample {
  pub offset: u64,
  pub size: u32,
  pub dts: i64,
  pub pts: i64,
  pub duration: u32,
  pub is_sync: bool,
  pub description_index: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edit {
  pub segment_duration: u64,
  pub media_time: i64,
  pub media_rate: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackKind {
  Video,
  Audio,
  Timecode,
  Subtitle,
  Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SampleGroup {
  pub grouping_type: FourCc,
  pub runs: Vec<(u32, u32)>,
  pub descriptions: Vec<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Enabled {
  Yes,
  No,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chunking {
  SamplePerEntry,
  ChunkPerSample,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
  pub id: u32,
  pub kind: TrackKind,
  pub handler: FourCc,
  pub handler_name: String,
  pub timescale: u32,
  pub duration: u64,
  pub tkhd_duration: u64,
  pub language: String,
  pub width: u32,
  pub height: u32,
  pub enabled: Enabled,
  pub alternate_group: u16,
  pub volume: i16,
  pub matrix: [i32; 9],
  pub entries: Vec<SampleEntry>,
  pub samples: Vec<Sample>,
  pub edits: Vec<Edit>,
  pub edit_offset: i64,
  pub references: Vec<(FourCc, Vec<u32>)>,
  pub sdtp: Vec<u8>,
  pub sample_groups: Vec<SampleGroup>,
  pub chunking: Chunking,
  sync_index: Vec<u32>,
  pts_order: Vec<(i64, u32)>,
}

impl Track {
  pub fn codec(&self) -> Result<Option<&CodecConfig>, Never> {
    Ok(self.entries.first().map(|entry| &entry.codec))
  }

  pub fn video(&self) -> Result<Option<&VideoParams>, Never> {
    Ok(self.entries.first().and_then(|entry| entry.video.as_ref()))
  }

  pub fn audio(&self) -> Result<Option<&AudioParams>, Never> {
    Ok(self.entries.first().and_then(|entry| entry.audio.as_ref()))
  }

  pub fn display_rotation(&self) -> Result<Option<u8>, Never> {
    let [a, b, _, c, d, _, _, _, _] = self.matrix;

    Ok(match (a.signum(), b.signum(), c.signum(), d.signum()) {
      (1, 0, 0, 1) => Some(0),
      (0, 1, -1, 0) => Some(1),
      (-1, 0, 0, -1) => Some(2),
      (0, -1, 1, 0) => Some(3),
      (_, _, _, _) => None,
    })
  }

  pub fn presentation_pts(&self, sample: u32) -> Result<Option<i64>, Never> {
    let Ok(at) = index(sample);

    Ok(
      self
        .samples
        .get(at)
        .map(|found| found.pts.saturating_add(self.edit_offset)),
    )
  }

  pub fn sample_at_pts(&self, pts: i64) -> Result<Option<u32>, Never> {
    let Ok(after) = fitted::<_, u32>(self.pts_order.partition_point(|(shown, _)| *shown <= pts));
    let Ok(at) = index(after.saturating_sub(1));

    Ok(self.pts_order.get(at).map(|(_, sample)| *sample))
  }

  pub fn sample_at_presentation_time(&self, time: i64) -> Result<Option<u32>, Never> {
    self.sample_at_pts(time.saturating_sub(self.edit_offset))
  }

  pub fn sync_sample_before(&self, sample: u32) -> Result<u32, Never> {
    let Ok(after) = fitted::<_, u32>(self.sync_index.partition_point(|sync| *sync <= sample));

    Ok(match after.checked_sub(1) {
      Some(before) => {
        let Ok(at) = index(before);

        match self.sync_index.get(at) {
          Some(sync) => *sync,
          None => 0,
        }
      }
      None => 0,
    })
  }

  pub fn sync_samples(&self) -> Result<&[u32], Never> {
    Ok(&self.sync_index)
  }

  pub fn read_sample<S: ByteSource + ?Sized>(
    &self,
    source: &S,
    sample: u32,
  ) -> Result<Vec<u8>, Error> {
    let Ok(at) = index(sample);
    let found = self.samples.get(at).ok_or(Error::NoSuchSample(sample))?;

    read_range(
      source,
      Span {
        offset: found.offset,
        size: u64::from(found.size),
      },
    )
  }

  fn finish_indexes(&mut self) -> Result<(), Never> {
    self.sync_index = self
      .samples
      .iter()
      .zip(0_u32..)
      .filter(|(sample, _)| sample.is_sync)
      .map(|(_, at)| at)
      .collect();

    let mut order: Vec<(i64, u32)> = self
      .samples
      .iter()
      .zip(0_u32..)
      .map(|(sample, at)| (sample.pts, at))
      .collect();

    order.sort_unstable();
    self.pts_order = order;

    Ok(())
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Metadata {
  pub entries: Vec<(String, String)>,
}

impl Metadata {
  pub fn get(&self, key: &str) -> Result<Option<&str>, Never> {
    Ok(
      self
        .entries
        .iter()
        .find(|(named, _)| named == key)
        .map(|(_, value)| value.as_str()),
    )
  }

  pub fn title(&self) -> Result<Option<&str>, Never> {
    let Ok(named) = self.get("©nam");
    let Ok(keyed) = self.get("com.apple.quicktime.title");
    let Ok(plain) = self.get("title");

    Ok(named.or(keyed).or(plain))
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fragmented {
  Yes,
  No,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mp4File {
  pub major_brand: FourCc,
  pub minor_version: u32,
  pub compatible_brands: Vec<FourCc>,
  pub flavour: Flavour,
  pub timescale: u32,
  pub duration: u64,
  pub tracks: Vec<Track>,
  pub metadata: Metadata,
  pub fragmented: Fragmented,
  pub moov_offset: u64,
  pub fragment_count: u32,
}

impl Mp4File {
  pub fn track_of_kind(&self, kind: TrackKind) -> Result<Option<u32>, Never> {
    Ok(
      self
        .tracks
        .iter()
        .zip(0_u32..)
        .find(|(track, _)| track.kind == kind)
        .map(|(_, at)| at),
    )
  }
}

#[derive(Clone, Copy)]
struct Span {
  offset: u64,
  size: u64,
}

fn read_range<S: ByteSource + ?Sized>(source: &S, span: Span) -> Result<Vec<u8>, Error> {
  let end = span
    .offset
    .checked_add(span.size)
    .ok_or(Error::Truncated("sample"))?;
  let length = source.len()?;

  match ::core::primitive::bool::then_some(end > length, ()) {
    ::core::option::Option::Some(()) => return Err(Error::Truncated("sample data")),
    ::core::option::Option::None => {}
  }

  let mut bytes = Vec::new();
  resize_read_buffer(&mut bytes, span.size)
    .map_err(|_allocation| Error::TooLarge("sample data"))?;
  source.read_at(span.offset, &mut bytes)?;

  Ok(bytes)
}

pub fn open<S: ByteSource>(source: S) -> Result<Mp4File, Error> {
  let length = source.len()?;
  let top = top_level(&source, length)?;
  let (moov_offset, moov) = top.moov.ok_or(Error::NoMoov)?;
  let Ok(fragment_count) = fitted::<_, u32>(top.moofs.len());

  let mut file = Mp4File {
    major_brand: FourCc::default(),
    minor_version: 0,
    compatible_brands: Vec::new(),
    flavour: Flavour::QuickTime,
    timescale: 0,
    duration: 0,
    tracks: Vec::new(),
    metadata: Metadata::default(),
    fragmented: Fragmented::No,
    moov_offset,
    fragment_count,
  };

  match &top.ftyp {
    Some(ftyp) => brands(&mut file, ftyp)?,
    None => {}
  }

  let movie = movie(&moov, &mut file)?;

  for payload in movie.tracks {
    let track = parse_trak(payload, &file, length)?;

    file.tracks.push(track);
  }

  fragments(
    &mut file,
    Fragments {
      moofs: &top.moofs,
      defaults: &movie.defaults,
      file_length: length,
    },
  )?;

  let fragmented = match (file.fragmented, top.moofs.is_empty()) {
    (Fragmented::Yes, false) => Fragmented::Yes,
    (Fragmented::Yes | Fragmented::No, _) => Fragmented::No,
  };

  for track in &mut file.tracks {
    let Ok(()) = finish(
      track,
      Finishing {
        fragmented,
        movie_timescale: file.timescale,
        movie_duration: &mut file.duration,
      },
    );

    timecode_start(track, &source)?;
  }

  Ok(file)
}

struct Finishing<'a> {
  fragmented: Fragmented,
  movie_timescale: u32,
  movie_duration: &'a mut u64,
}

fn finish(track: &mut Track, finishing: Finishing<'_>) -> Result<(), Never> {
  match finishing.fragmented {
    Fragmented::Yes => {
      let summed = track.samples.iter().fold(0_u64, |total, sample| {
        total.saturating_add(u64::from(sample.duration))
      });

      track.duration = track.duration.max(summed);

      let scaled = u128::from(track.duration)
        .saturating_mul(u128::from(finishing.movie_timescale))
        .checked_div(u128::from(track.timescale));

      match (scaled, finishing.movie_timescale) {
        (Some(scaled_2), 1..) => {
          let Ok(scaled_3) = fitted::<u128, u64>(scaled_2);

          *finishing.movie_duration = (*finishing.movie_duration).max(scaled_3);
        }
        (_, _) => {}
      }
    }
    Fragmented::No => {}
  }

  track.finish_indexes()
}

fn timecode_start<S: ByteSource>(track: &mut Track, source: &S) -> Result<(), Error> {
  let first = match track.samples.first() {
    Some(first) => *first,
    None => return Ok(()),
  };

  let timecode = match track.entries.first_mut().map(|entry| &mut entry.codec) {
    Some(CodecConfig::Timecode(timecode)) => timecode,
    Some(_) | None => return Ok(()),
  };

  match ::core::primitive::bool::then_some(first.size >= 4, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(()),
  }

  let mut frame = [0_u8; 4];

  match source.read_at(first.offset, &mut frame) {
    Ok(()) => timecode.start_frame = Some(u32::from_be_bytes(frame)),
    Err(_unread) => timecode.start_frame = None,
  }

  Ok(())
}

struct TopLevel {
  ftyp: Option<Vec<u8>>,
  moov: Option<(u64, Vec<u8>)>,
  moofs: Vec<(u64, Vec<u8>)>,
}

struct TopLevelBox {
  kind: FourCc,
  offset: u64,
  body: Span,
  next: Option<u64>,
}

struct TopLevelBoxes<'s, S> {
  source: &'s S,
  file_length: u64,
  next: Option<u64>,
}

impl<S: ByteSource> Iterator for TopLevelBoxes<'_, S> {
  type Item = Result<TopLevelBox, Error>;

  fn next(&mut self) -> Option<Self::Item> {
    let offset = self.next.take()?;

    match top_level_box(
      self.source,
      Placed {
        offset,
        file_length: self.file_length,
      },
    ) {
      Ok(Some(found)) => {
        self.next = found.next;

        Some(Ok(found))
      }
      Ok(None) => None,
      Err(fault) => Some(Err(fault)),
    }
  }
}

#[derive(Clone, Copy)]
struct Placed {
  offset: u64,
  file_length: u64,
}

fn top_level_box<S: ByteSource>(
  source: &S,
  placed: Placed,
) -> Result<Option<TopLevelBox>, Error> {
  let Placed {
    offset,
    file_length,
  } = placed;
  let room = file_length.saturating_sub(offset);

  match ::core::primitive::bool::then_some(room < BoxHeader::BASE_HEADER_LEN, ()) {
    ::core::option::Option::Some(()) => return Ok(None),
    ::core::option::Option::None => {}
  }

  let mut buffer = BoxHeader::BUFFER;
  let Ok(wanted) = index(room.min(BoxHeader::MAX_HEADER_LEN));
  let window = buffer
    .get_mut(..wanted)
    .ok_or(Error::Truncated("box header"))?;

  source.read_at(offset, window)?;

  let header = BoxHeader::parse(window)?;

  let size = match header.size {
    Some(size) => size,
    None => room,
  };

  let end = offset.saturating_add(size).min(file_length);
  let start = offset.saturating_add(header.header_len);
  let body = Span {
    offset: start,
    size: end.saturating_sub(start),
  };
  let next = header.size.map(|size_2| offset.saturating_add(size_2));

  Ok(Some(TopLevelBox {
    kind: header.kind,
    offset,
    body,
    next,
  }))
}

fn body_of<S: ByteSource>(
  source: &S,
  found: &TopLevelBox,
  ceiling: u64,
) -> Result<Vec<u8>, Error> {
  match ::core::primitive::bool::then_some(found.body.size > ceiling, ()) {
    ::core::option::Option::Some(()) => Err(Error::TooLarge("header box")),
    ::core::option::Option::None => read_range(source, found.body),
  }
}

fn top_level<S: ByteSource>(source: &S, file_length: u64) -> Result<TopLevel, Error> {
  let mut top = TopLevel {
    ftyp: None,
    moov: None,
    moofs: Vec::new(),
  };
  let walk = TopLevelBoxes {
    source,
    file_length,
    next: Some(0),
  };

  for each in walk {
    let found = each?;

    match (&found.kind.0, top.ftyp.is_none(), top.moov.is_none()) {
      (b"ftyp", true, _) => {
        let body = body_of(source, &found, MAX_FTYP)?;

        top.ftyp = Some(body);
      }
      (b"moov", _, true) => {
        let body = body_of(source, &found, MAX_HEADER_BOX)?;

        top.moov = Some((found.offset, body));
      }
      (b"moof", _, _) => {
        let body = body_of(source, &found, MAX_HEADER_BOX)?;

        top.moofs.push((found.offset, body));
      }
      (_, _, _) => {}
    }
  }

  Ok(top)
}

fn brands(file: &mut Mp4File, ftyp: &[u8]) -> Result<(), Error> {
  let Ok(mut cursor) = Cursor::new(ftyp, "ftyp");

  let major_brand = cursor.fourcc()?;
  file.major_brand = major_brand;
  file.minor_version = match cursor.u32() {
    Ok(version) => version,
    Err(_short) => 0,
  };

  let Ok(rest) = cursor.rest();

  file.compatible_brands = rest
    .as_chunks::<4>()
    .0
    .iter()
    .map(|code| FourCc(*code))
    .collect();
  file.flavour = match ::core::primitive::bool::then_some(file.major_brand == *b"qt  ", ()) {
    ::core::option::Option::Some(()) => Flavour::QuickTime,
    ::core::option::Option::None => Flavour::Iso,
  };

  Ok(())
}

#[derive(Clone, Copy, Debug, Default)]
struct Trex {
  description_index: u32,
  duration: u32,
  size: u32,
  flags: u32,
}

struct Movie<'a> {
  tracks: Vec<&'a [u8]>,
  defaults: BTreeMap<u32, Trex>,
}

fn movie<'a>(moov: &'a [u8], file: &mut Mp4File) -> Result<Movie<'a>, Error> {
  let mut movie = Movie {
    tracks: Vec::new(),
    defaults: BTreeMap::new(),
  };
  let Ok(children) = boxes(moov);

  for each in children {
    let child = each?;

    match &child.kind.0 {
      b"cmov" => return Err(Error::Unsupported(Unsupported::CompressedMovieHeader)),
      b"mvhd" => movie_header(child.payload, file)?,
      b"trak" => movie.tracks.push(child.payload),
      b"mvex" => {
        file.fragmented = Fragmented::Yes;

        track_defaults(child.payload, &mut movie.defaults)?;
      }
      b"udta" => {
        let Ok(()) = parse_udta(child.payload, &mut file.metadata);
      }
      b"meta" => {
        let Ok(()) = parse_meta(child.payload, &mut file.metadata);
      }
      _unread => {}
    }
  }

  Ok(movie)
}

struct Timing {
  timescale: u32,
  duration: u64,
}

fn timing(cursor: &mut Cursor<'_>) -> Result<Timing, Error> {
  let header = cursor.full_header()?;

  match header.version {
    1 => {
      cursor.skip(16)?;

      let timescale = cursor.u32()?;
      let duration = cursor.u64()?;

      Ok(Timing {
        timescale,
        duration,
      })
    }
    _compact => {
      cursor.skip(8)?;

      let timescale = cursor.u32()?;
      let duration = cursor.u32()?;

      Ok(Timing {
        timescale,
        duration: u64::from(duration),
      })
    }
  }
}

fn movie_header(payload: &[u8], file: &mut Mp4File) -> Result<(), Error> {
  let Ok(mut cursor) = Cursor::new(payload, "mvhd");
  let read = timing(&mut cursor)?;

  file.timescale = read.timescale;
  file.duration = read.duration;

  Ok(())
}

fn track_defaults(mvex: &[u8], defaults: &mut BTreeMap<u32, Trex>) -> Result<(), Error> {
  let Ok(children) = whole_boxes(mvex);

  for child in children.filter(|child| child.kind == *b"trex") {
    let Ok(mut cursor) = Cursor::new(child.payload, "trex");
    let _header = cursor.full_header()?;
    let id = cursor.u32()?;
    let description_index = cursor.u32()?;
    let duration = cursor.u32()?;
    let size = cursor.u32()?;
    let flags = cursor.u32()?;

    defaults.insert(
      id,
      Trex {
        description_index,
        duration,
        size,
        flags,
      },
    );
  }

  Ok(())
}

fn parse_language(code: u16) -> Result<String, Never> {
  match code {
    0 => return Ok(String::from("eng")),
    1..0x400 => return Ok(String::from("und")),
    _packed => {}
  }

  let letters = [10_u32, 5, 0].map(|shift| {
    let Ok(letter) = fitted::<u16, u8>(code.wrapping_shr(shift) & 0x1F);

    letter.saturating_add(0x60)
  });

  Ok(match ::core::primitive::bool::then_some(letters.iter().all(u8::is_ascii_lowercase), ()) {
    ::core::option::Option::Some(()) => letters.into_iter().map(char::from).collect(),
    ::core::option::Option::None => String::from("und"),
  })
}

#[derive(Clone, Copy)]
struct Timescales {
  from: u32,
  to: u32,
}

fn rescale(value: i128, scales: Timescales) -> Result<i64, Never> {
  let scaled = value
    .saturating_mul(i128::from(scales.to))
    .checked_div(i128::from(scales.from));

  match scaled {
    Some(scaled_2) => fitted::<i128, i64>(scaled_2),
    None => Ok(0),
  }
}

fn empty_track() -> Result<Track, Never> {
  Ok(Track {
    id: 0,
    kind: TrackKind::Other,
    handler: FourCc::default(),
    handler_name: String::new(),
    timescale: 0,
    duration: 0,
    tkhd_duration: 0,
    language: String::from("und"),
    width: 0,
    height: 0,
    enabled: Enabled::Yes,
    alternate_group: 0,
    volume: 0,
    matrix: IDENTITY,
    entries: Vec::new(),
    samples: Vec::new(),
    edits: Vec::new(),
    edit_offset: 0,
    references: Vec::new(),
    sdtp: Vec::new(),
    sample_groups: Vec::new(),
    chunking: Chunking::SamplePerEntry,
    sync_index: Vec::new(),
    pts_order: Vec::new(),
  })
}

fn parse_trak(payload: &[u8], file: &Mp4File, file_length: u64) -> Result<Track, Error> {
  let Ok(mut track) = empty_track();
  let mut media = None;
  let Ok(children) = boxes(payload);

  for each in children {
    let child = each?;

    match &child.kind.0 {
      b"tkhd" => track_header(child.payload, &mut track)?,
      b"edts" => edit_list(child.payload, &mut track)?,
      b"tref" => {
        let Ok(()) = references(child.payload, &mut track);
      }
      b"mdia" => media = Some(child.payload),
      _unread => {}
    }
  }

  let media_2 = media.ok_or(Error::Invalid(Invalid::TrackWithoutMedia))?;
  let information = parse_mdia(media_2, &mut track)?;

  track.kind = match &track.handler.0 {
    b"vide" => TrackKind::Video,
    b"soun" => TrackKind::Audio,
    b"tmcd" => TrackKind::Timecode,
    b"text" | b"sbtl" | b"subt" | b"clcp" => TrackKind::Subtitle,
    _other => TrackKind::Other,
  };

  let Ok(edit_offset) = edit_offset(
    &track.edits,
    Timescales {
      from: file.timescale,
      to: track.timescale,
    },
  );

  track.edit_offset = edit_offset;

  let Ok(tables) = information
    .map(|information_2| find(information_2, b"stbl"))
    .transpose();

  match tables.flatten() {
    Some(stbl) => parse_stbl(
      stbl,
      &mut track,
      Within {
        flavour: file.flavour,
        file_length,
      },
    )?,
    None => {}
  }

  Ok(track)
}

fn track_header(payload: &[u8], track: &mut Track) -> Result<(), Error> {
  let Ok(mut cursor) = Cursor::new(payload, "tkhd");
  let header = cursor.full_header()?;

  track.enabled = match header.flags & 1 {
    0 => Enabled::No,
    _enabled => Enabled::Yes,
  };

  match header.version {
    1 => {
      cursor.skip(16)?;
      let id = cursor.u32()?;
      track.id = id;
      cursor.skip(4)?;
      let tkhd_duration = cursor.u64()?;
      track.tkhd_duration = tkhd_duration;
    }
    _compact => {
      cursor.skip(8)?;
      let id = cursor.u32()?;
      track.id = id;
      cursor.skip(4)?;

      let duration = cursor.u32()?;

      track.tkhd_duration = u64::from(duration);
    }
  }

  cursor.skip(8)?;

  let _layer = cursor.i16()?;

  let alternate_group = cursor.u16()?;
  track.alternate_group = alternate_group;
  let volume = cursor.i16()?;
  track.volume = volume;
  cursor.skip(2)?;

  for entry in track.matrix.iter_mut() {
    let read = cursor.i32()?;
    *entry = read;
  }

  let width = cursor.u32()?;
  let height = cursor.u32()?;

  track.width = width.wrapping_shr(16);
  track.height = height.wrapping_shr(16);

  Ok(())
}

fn edit_list(edts: &[u8], track: &mut Track) -> Result<(), Error> {
  let Ok(found) = find(edts, b"elst");

  let elst = match found {
    Some(elst) => elst,
    None => return Ok(()),
  };

  let Ok(mut cursor) = Cursor::new(elst, "elst");
  let header = cursor.full_header()?;

  let entry_size = match header.version {
    1 => 20,
    _compact => 12,
  };

  let count = cursor.count(entry_size)?;

  for _ in 0..count {
    let (segment_duration, media_time) = match header.version {
      1 => {
        let segment = cursor.u64()?;
        let start = cursor.i64()?;

        (segment, start)
      }
      _compact => {
        let segment = cursor.u32()?;
        let start = cursor.i32()?;

        (u64::from(segment), i64::from(start))
      }
    };

    let media_rate = cursor.i32()?;

    track.edits.push(Edit {
      segment_duration,
      media_time,
      media_rate,
    });
  }

  Ok(())
}

fn references(tref: &[u8], track: &mut Track) -> Result<(), Never> {
  let Ok(children) = whole_boxes(tref);

  for child in children {
    let ids = child
      .payload
      .as_chunks::<4>()
      .0
      .iter()
      .map(|id| u32::from_be_bytes(*id))
      .collect();

    track.references.push((child.kind, ids));
  }

  Ok(())
}

fn parse_mdia<'a>(mdia: &'a [u8], track: &mut Track) -> Result<Option<&'a [u8]>, Error> {
  let mut information = None;
  let Ok(children) = boxes(mdia);

  for each in children {
    let child = each?;

    match &child.kind.0 {
      b"mdhd" => media_header(child.payload, track)?,
      b"hdlr" => handler(child.payload, track)?,
      b"minf" => information = Some(child.payload),
      _unread => {}
    }
  }

  Ok(information)
}

fn media_header(payload: &[u8], track: &mut Track) -> Result<(), Error> {
  let Ok(mut cursor) = Cursor::new(payload, "mdhd");
  let read = timing(&mut cursor)?;
  let language = cursor.u16()?;
  let Ok(language_2) = parse_language(language);

  track.timescale = read.timescale;
  track.duration = read.duration;
  track.language = language_2;

  Ok(())
}

fn handler(payload: &[u8], track: &mut Track) -> Result<(), Error> {
  let Ok(mut cursor) = Cursor::new(payload, "hdlr");
  let _header = cursor.full_header()?;
  let _predefined = cursor.u32()?;

  let handler = cursor.fourcc()?;
  track.handler = handler;
  cursor.skip(12)?;

  let Ok(name) = cursor.rest();
  let Ok(name_2) = handler_name(name);

  track.handler_name = name_2;

  Ok(())
}

fn handler_name(name: &[u8]) -> Result<String, Never> {
  let pascal = match name.split_first() {
    Some((length, text)) => {
      let Ok(written) = fitted::<_, u64>(text.len());

      match (u64::from(*length) == written, text.is_empty()) {
        (true, false) => Some(text),
        (_, _) => None,
      }
    }
    None => None,
  };

  let said = match pascal {
    Some(text) => text,
    None => match name.split(|byte| *byte == 0).next() {
      Some(text) => text,
      None => &[],
    },
  };

  display_text(said)
}

fn edit_offset(edits: &[Edit], scales: Timescales) -> Result<i64, Never> {
  let mut empty = 0_i128;

  for edit in edits {
    match edit.media_time {
      EMPTY_EDIT => empty = empty.saturating_add(i128::from(edit.segment_duration)),
      start => {
        let Ok(lead) = rescale(empty, scales);

        return Ok(lead.saturating_sub(start));
      }
    }
  }

  Ok(0)
}

#[derive(Clone, Copy)]
struct Within {
  flavour: Flavour,
  file_length: u64,
}

#[derive(Default)]
struct Tables {
  stts: Vec<(u32, u32)>,
  ctts: Vec<(u32, u32)>,
  stss: Option<BTreeSet<u32>>,
  stsc: Vec<ChunkRun>,
  const_size: u32,
  sample_count: u32,
  sizes: Vec<u32>,
  chunks: Vec<u64>,
  sbgp: Vec<(FourCc, Vec<(u32, u32)>)>,
  sgpd: BTreeMap<FourCc, Vec<Vec<u8>>>,
}

#[derive(Clone, Copy, Default)]
struct ChunkRun {
  first_chunk: u32,
  samples_per_chunk: u32,
  description_index: u32,
}

fn parse_stbl(stbl: &[u8], track: &mut Track, within: Within) -> Result<(), Error> {
  let mut tables = Tables::default();
  let Ok(children) = boxes(stbl);

  for each in children {
    let child = each?;

    sample_table_box(child, &mut tables, track, within)?;
  }

  let mut described = core::mem::take(&mut tables.sgpd);

  for (grouping_type, runs) in core::mem::take(&mut tables.sbgp) {
    let descriptions = match described.remove(&grouping_type) {
      Some(descriptions) => descriptions,
      None => Vec::new(),
    };

    track.sample_groups.push(SampleGroup {
      grouping_type,
      runs,
      descriptions,
    });
  }

  match (
    tables.sample_count,
    tables.chunks.is_empty(),
    tables.stsc.first(),
  ) {
    (1.., false, Some(first)) => {
      let ordered = tables
        .stsc
        .iter()
        .zip(tables.stsc.iter().skip(1))
        .all(|(earlier, later)| later.first_chunk > earlier.first_chunk);

      match (first.first_chunk, ordered) {
        (1.., true) => {}
        (_, _) => return Err(Error::Invalid(Invalid::ChunksOutOfOrder)),
      }
    }
    (_, _, _) => return Ok(()),
  }

  let pcm = match (
    track.entries.first().map(|entry| &entry.codec),
    tables.const_size,
  ) {
    (Some(CodecConfig::Pcm(pcm)), 1..) => Some(*pcm),
    (_, _) => None,
  };

  track.samples = match pcm {
    Some(pcm_2) => {
      track.chunking = Chunking::ChunkPerSample;

      let Ok(audio) = track.audio();
      let audio_2 = audio.cloned();

      pcm_samples(
        &tables,
        PcmLayout {
          pcm: pcm_2,
          audio: audio_2,
        },
      )?
    }
    None => coded_samples(&tables)?,
  };

  Ok(())
}

fn sample_table_box(
  child: BoxView<'_>,
  tables: &mut Tables,
  track: &mut Track,
  within: Within,
) -> Result<(), Error> {
  let Ok(mut cursor) = Cursor::new(child.payload, "stbl child");

  match &child.kind.0 {
    b"stsd" => {
      let _header = cursor.full_header()?;
      let count = cursor.count(8)?;
      let Ok(count_2) = index(count);
      let Ok(rest) = cursor.rest();
      let Ok(described) = boxes(rest);

      for each in described.take(count_2) {
        let entry = each?;
        let parsed = parse_sample_entry(Entry {
          format: entry.kind,
          payload: entry.payload,
          handler: track.handler,
          flavour: within.flavour,
        })?;

        track.entries.push(parsed);
      }
    }
    b"stts" => {
      let read = runs(&mut cursor)?;

      tables.stts = read;
    }
    b"ctts" => {
      let read = runs(&mut cursor)?;

      tables.ctts = read;
    }
    b"stss" => {
      let _header = cursor.full_header()?;
      let count = cursor.count(4)?;
      let mut sync = BTreeSet::new();

      for _ in 0..count {
        let number = cursor.u32()?;

        sync.insert(number);
      }

      tables.stss = Some(sync);
    }
    b"stsc" => {
      let read = chunk_runs(&mut cursor)?;

      tables.stsc = read;
    }
    b"stsz" => sample_sizes(&mut cursor, tables, within.file_length)?,
    b"stz2" => compact_sample_sizes(&mut cursor, tables)?,
    b"stco" => {
      let _header = cursor.full_header()?;
      let count = cursor.count(4)?;

      for _ in 0..count {
        let offset = cursor.u32()?;

        tables.chunks.push(u64::from(offset));
      }
    }
    b"co64" => {
      let _header = cursor.full_header()?;
      let count = cursor.count(8)?;

      for _ in 0..count {
        let offset = cursor.u64()?;

        tables.chunks.push(offset);
      }
    }
    b"sdtp" => {
      let _header = cursor.full_header()?;
      let Ok(rest) = cursor.rest();

      track.sdtp = rest.to_vec();
    }
    b"sbgp" => sample_to_group(&mut cursor, tables)?,
    b"sgpd" => group_descriptions(&mut cursor, tables)?,
    _unread => {}
  }

  Ok(())
}

fn runs(cursor: &mut Cursor<'_>) -> Result<Vec<(u32, u32)>, Error> {
  let _header = cursor.full_header()?;
  let count = cursor.count(8)?;
  let mut runs = Vec::new();

  for _ in 0..count {
    let samples = cursor.u32()?;
    let value = cursor.u32()?;

    runs.push((samples, value));
  }

  Ok(runs)
}

fn chunk_runs(cursor: &mut Cursor<'_>) -> Result<Vec<ChunkRun>, Error> {
  let _header = cursor.full_header()?;
  let count = cursor.count(12)?;
  let mut runs = Vec::new();

  for _ in 0..count {
    let first_chunk = cursor.u32()?;
    let samples_per_chunk = cursor.u32()?;
    let description_index = cursor.u32()?;

    runs.push(ChunkRun {
      first_chunk,
      samples_per_chunk,
      description_index,
    });
  }

  Ok(runs)
}

fn sample_sizes(
  cursor: &mut Cursor<'_>,
  tables: &mut Tables,
  file_length: u64,
) -> Result<(), Error> {
  let _header = cursor.full_header()?;

  let const_size = cursor.u32()?;
  tables.const_size = const_size;

  let count = cursor.u32()?;

  match core::num::NonZeroU32::new(tables.const_size) {
    None => {
      cursor.check_count(count, 4)?;

      for _ in 0..count {
        let size = cursor.u32()?;

        tables.sizes.push(size);
      }

      tables.sample_count = count;
    }
    Some(size) => {
      let fitting = file_length / core::num::NonZeroU64::from(size);
      let Ok(fitting_2) = fitted::<u64, u32>(u64::from(count).min(fitting));

      tables.sample_count = fitting_2;
    }
  }

  Ok(())
}

fn compact_sample_sizes(cursor: &mut Cursor<'_>, tables: &mut Tables) -> Result<(), Error> {
  let _header = cursor.full_header()?;
  let written = cursor.u32()?;
  let field = written & 0xFF;
  let count = cursor.u32()?;

  let bits = match field {
    4 | 8 | 16 => u64::from(field),
    other => return Err(Error::Invalid(Invalid::CompactSizeField(other))),
  };

  let Ok(remaining) = cursor.remaining();

  match u64::from(count)
    .checked_mul(bits)
    .map(|total| total.div_ceil(8) <= remaining)
  {
    Some(true) => {}
    Some(false) | None => return Err(Error::Truncated("stz2")),
  }

  let Ok(packed) = cursor.rest();
  let Ok(taken) = index(count);

  tables.sizes = match field {
    4 => packed
      .iter()
      .flat_map(|byte| [u32::from(byte.wrapping_shr(4)), u32::from(byte & 0xF)])
      .take(taken)
      .collect(),
    8 => packed
      .iter()
      .map(|byte| u32::from(*byte))
      .take(taken)
      .collect(),
    _sixteen => packed
      .as_chunks::<2>()
      .0
      .iter()
      .map(|pair| u32::from(u16::from_be_bytes(*pair)))
      .take(taken)
      .collect(),
  };
  tables.sample_count = count;

  Ok(())
}

fn sample_to_group(cursor: &mut Cursor<'_>, tables: &mut Tables) -> Result<(), Error> {
  let header = cursor.full_header()?;
  let grouping_type = cursor.fourcc()?;

  match header.version {
    1 => {
      let _parameter = cursor.u32()?;
    }
    _plain => {}
  }

  let count = cursor.count(8)?;
  let mut runs = Vec::new();

  for _ in 0..count {
    let samples = cursor.u32()?;
    let description = cursor.u32()?;

    runs.push((samples, description));
  }

  tables.sbgp.push((grouping_type, runs));

  Ok(())
}

#[derive(Clone, Copy)]
enum Lengths {
  EachWritten,
  Shared(u32),
}

fn group_descriptions(cursor: &mut Cursor<'_>, tables: &mut Tables) -> Result<(), Error> {
  let header = cursor.full_header()?;
  let grouping_type = cursor.fourcc()?;

  let lengths = match header.version {
    0 => Lengths::Shared(0),
    _versioned => {
      let shared = cursor.u32()?;

      match shared {
        0 => Lengths::EachWritten,
        shared_2 => Lengths::Shared(shared_2),
      }
    }
  };

  match header.version {
    0 | 1 => {}
    _with_default => {
      let _default_index = cursor.u32()?;
    }
  }

  let count = cursor.count(0)?;
  let described = descriptions(cursor, lengths, count)?;

  tables.sgpd.insert(grouping_type, described);

  Ok(())
}

fn descriptions(
  cursor: &mut Cursor<'_>,
  lengths: Lengths,
  count: u32,
) -> Result<Vec<Vec<u8>>, Error> {
  let mut found = Vec::new();

  for _ in 0..count {
    let length = match lengths {
      Lengths::EachWritten => cursor.u32()?,
      Lengths::Shared(length) => length,
    };

    match length {
      0 => {
        let Ok(rest) = cursor.rest();

        found.push(rest.to_vec());

        return Ok(found);
      }
      length_2 => {
        let description = cursor.bytes(u64::from(length_2))?;

        found.push(description.to_vec());
      }
    }
  }

  Ok(found)
}

struct Runs<'r> {
  runs: &'r [(u32, u32)],
  run: u32,
  used: u32,
}

impl<'r> Runs<'r> {
  fn new(runs: &'r [(u32, u32)]) -> Result<Self, Never> {
    Ok(Runs {
      runs,
      run: 0,
      used: 0,
    })
  }

  fn take(&mut self, wanted: u64) -> Result<u64, Never> {
    let runs = self.runs;
    let Ok(from) = index(self.run);
    let mut left = wanted;
    let mut total = 0_u64;

    for (count, delta) in runs.iter().skip(from) {
      match left {
        0 => break,
        _wanted => {}
      }

      let taken = u64::from(count.saturating_sub(self.used)).min(left);
      let Ok(narrow) = fitted::<u64, u32>(taken);

      total = total.saturating_add(taken.saturating_mul(u64::from(*delta)));
      left = left.saturating_sub(taken);
      self.used = self.used.saturating_add(narrow);

      match ::core::primitive::bool::then_some(self.used >= *count, ()) {
        ::core::option::Option::Some(()) => {
          self.run = self.run.saturating_add(1);
          self.used = 0;
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(total)
  }

  fn next_value(&mut self) -> Result<u32, Never> {
    let runs = self.runs;
    let Ok(from) = index(self.run);
    let next = runs
      .iter()
      .zip(0_u32..)
      .skip(from)
      .find(|((count, _), _)| *count != 0);

    Ok(match next {
      Some(((_, delta), at)) => {
        self.run = at;

        let Ok(_taken) = self.take(1);

        *delta
      }
      None => 0,
    })
  }
}

struct ChunkRuns<'r> {
  current: ChunkRun,
  upcoming: &'r [ChunkRun],
}

impl<'r> ChunkRuns<'r> {
  fn new(runs: &'r [ChunkRun]) -> Result<Self, Never> {
    Ok(match runs.split_first() {
      Some((current, upcoming)) => ChunkRuns {
        current: *current,
        upcoming,
      },
      None => ChunkRuns {
        current: ChunkRun::default(),
        upcoming: &[],
      },
    })
  }

  fn at(&mut self, chunk: u32) -> Result<ChunkRun, Never> {
    match self.upcoming.split_first() {
      Some((next, after)) => match ::core::primitive::bool::then_some(next.first_chunk <= chunk, ()) {
        ::core::option::Option::Some(()) => {
          self.current = *next;
          self.upcoming = after;
        }
        ::core::option::Option::None => {}
      },
      None => {}
    }

    Ok(self.current)
  }
}

struct PcmLayout {
  pcm: PcmConfig,
  audio: Option<AudioParams>,
}

fn frame_bytes(layout: &PcmLayout, tables: &Tables, frames: u64) -> Result<u64, Never> {
  let (version, per_frame, per_packet) = match &layout.audio {
    Some(audio) => (
      audio.qt_version,
      audio.bytes_per_frame,
      audio.samples_per_packet,
    ),
    None => (0, 0, 0),
  };

  let packets = (
    version,
    core::num::NonZeroU32::new(per_frame),
    core::num::NonZeroU32::new(per_packet),
  );

  Ok(match (packets, tables.const_size) {
    ((1, Some(per_frame_2), Some(per_packet_2)), _) => {
      let whole = frames / core::num::NonZeroU64::from(per_packet_2);

      whole.saturating_mul(u64::from(per_frame_2.get()))
    }
    ((_, _, _), 2..) => frames.saturating_mul(u64::from(tables.const_size)),
    ((_, _, _), _) => {
      let Ok(per_frame_3) = layout.pcm.bytes_per_frame();

      frames.saturating_mul(u64::from(per_frame_3))
    }
  })
}

fn pcm_samples(tables: &Tables, layout: PcmLayout) -> Result<Vec<Sample>, Error> {
  let Ok(chunk_count) = fitted::<_, u64>(tables.chunks.len());

  match ::core::primitive::bool::then_some(chunk_count > MAX_SAMPLES, ()) {
    ::core::option::Option::Some(()) => return Err(Error::TooLarge("chunk count")),
    ::core::option::Option::None => {}
  }

  let Ok(mut durations) = Runs::new(&tables.stts);
  let Ok(mut layout_runs) = ChunkRuns::new(&tables.stsc);
  let mut samples = Vec::new();
  let mut left = tables.sample_count;
  let mut dts = 0_i64;

  for (offset, chunk) in tables.chunks.iter().zip(1_u32..=u32::MAX) {
    let Ok(run) = layout_runs.at(chunk);
    let in_chunk = run.samples_per_chunk.min(left);

    match in_chunk {
      0 => continue,
      _some => {}
    }

    let Ok(duration) = durations.take(u64::from(in_chunk));
    let Ok(bytes) = frame_bytes(&layout, tables, u64::from(in_chunk));
    let Ok(size) = fitted::<u64, u32>(bytes);
    let Ok(narrow_duration) = fitted::<u64, u32>(duration);
    let Ok(advance) = fitted::<u64, i64>(duration);

    samples.push(Sample {
      offset: *offset,
      size,
      dts,
      pts: dts,
      duration: narrow_duration,
      is_sync: true,
      description_index: run.description_index,
    });

    dts = dts.saturating_add(advance);
    left = left.saturating_sub(in_chunk);
  }

  Ok(samples)
}

fn coded_samples(tables: &Tables) -> Result<Vec<Sample>, Error> {
  match ::core::primitive::bool::then_some(u64::from(tables.sample_count) > MAX_SAMPLES, ()) {
    ::core::option::Option::Some(()) => return Err(Error::TooLarge("sample count")),
    ::core::option::Option::None => {}
  }

  let Ok(mut durations) = Runs::new(&tables.stts);
  let Ok(mut offsets) = Runs::new(&tables.ctts);
  let Ok(mut layout_runs) = ChunkRuns::new(&tables.stsc);
  let mut sizes = tables.sizes.iter();
  let mut samples = Vec::new();
  let mut number = 0_u32;
  let mut dts = 0_i64;

  for (offset, chunk) in tables.chunks.iter().zip(1_u32..=u32::MAX) {
    let Ok(run) = layout_runs.at(chunk);
    let in_chunk = run
      .samples_per_chunk
      .min(tables.sample_count.saturating_sub(number));
    let mut position = *offset;

    for _ in 0..in_chunk {
      number = number.saturating_add(1);

      let size = match (tables.const_size, sizes.next()) {
        (0, Some(size)) => *size,
        (0, None) => 0,
        (constant, _) => constant,
      };

      let Ok(duration) = durations.next_value();
      let Ok(composition) = offsets.next_value();

      let is_sync = match &tables.stss {
        Some(sync) => sync.contains(&number),
        None => true,
      };

      samples.push(Sample {
        offset: position,
        size,
        dts,
        pts: dts.saturating_add(i64::from(composition.cast_signed())),
        duration,
        is_sync,
        description_index: run.description_index,
      });

      position = position.saturating_add(u64::from(size));
      dts = dts.saturating_add(i64::from(duration));
    }
  }

  Ok(samples)
}

struct Fragments<'a> {
  moofs: &'a [(u64, Vec<u8>)],
  defaults: &'a BTreeMap<u32, Trex>,
  file_length: u64,
}

fn fragments(file: &mut Mp4File, fragments: Fragments<'_>) -> Result<(), Error> {
  let mut next_dts: Vec<i64> = file
    .tracks
    .iter()
    .map(|track| match track.samples.last() {
      Some(last) => last.dts.saturating_add(i64::from(last.duration)),
      None => 0,
    })
    .collect();

  for (offset, moof) in fragments.moofs {
    parse_moof(
      Fragment {
        offset: *offset,
        moof,
        defaults: fragments.defaults,
        file_length: fragments.file_length,
      },
      &mut file.tracks,
      &mut next_dts,
    )?;
  }

  Ok(())
}

#[derive(Clone, Copy)]
struct Fragment<'a> {
  offset: u64,
  moof: &'a [u8],
  defaults: &'a BTreeMap<u32, Trex>,
  file_length: u64,
}

struct TrackFragment<'a> {
  header: &'a [u8],
  decode_time: Option<&'a [u8]>,
  runs: Vec<&'a [u8]>,
}

fn track_fragment(traf: &[u8]) -> Result<TrackFragment<'_>, Error> {
  let mut header = None;
  let mut decode_time = None;
  let mut runs = Vec::new();
  let Ok(children) = boxes(traf);

  for each in children {
    let child = each?;

    match &child.kind.0 {
      b"tfhd" => header = Some(child.payload),
      b"tfdt" => decode_time = Some(child.payload),
      b"trun" => runs.push(child.payload),
      _unread => {}
    }
  }

  let header_2 = header.ok_or(Error::Invalid(Invalid::FragmentWithoutHeader))?;

  Ok(TrackFragment {
    header: header_2,
    decode_time,
    runs,
  })
}

#[derive(Clone, Copy)]
struct Defaults {
  base: u64,
  description_index: u32,
  duration: u32,
  size: u32,
  flags: u32,
}

fn parse_moof(
  fragment: Fragment<'_>,
  tracks: &mut [Track],
  next_dts: &mut [i64],
) -> Result<(), Error> {
  let mut previous_end = None;
  let Ok(children) = boxes(fragment.moof);

  for each in children {
    let child = each?;

    match ::core::primitive::bool::then_some(child.kind == *b"traf", ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => continue,
    }

    let traf = track_fragment(child.payload)?;
    let Ok(mut cursor) = Cursor::new(traf.header, "tfhd");
    let header = cursor.full_header()?;
    let id = cursor.u32()?;

    let (track, decoded) = match tracks
      .iter_mut()
      .zip(next_dts.iter_mut())
      .find(|(track, _)| track.id == id)
    {
      Some(found) => found,
      None => continue,
    };

    let defaults = fragment_defaults(
      &mut cursor,
      header.flags,
      Defaulted {
        fragment,
        id,
        previous_end,
      },
    )?;

    match traf.decode_time {
      Some(payload) => {
        let time = decode_time(payload)?;

        *decoded = time;
      }
      None => {}
    }

    let mut position = defaults.base;

    for run in traf.runs {
      let end = track_run(
        run,
        Running {
          defaults,
          position,
          file_length: fragment.file_length,
        },
        track,
        decoded,
      )?;

      position = end;
    }

    previous_end = Some(position);
  }

  Ok(())
}

struct Defaulted<'a> {
  fragment: Fragment<'a>,
  id: u32,
  previous_end: Option<u64>,
}

fn fragment_defaults(
  cursor: &mut Cursor<'_>,
  flags: u32,
  defaulted: Defaulted<'_>,
) -> Result<Defaults, Error> {
  let movie = match defaulted.fragment.defaults.get(&defaulted.id) {
    Some(movie) => *movie,
    None => Trex::default(),
  };

  let base = match (flags & 1, flags & 0x20000, defaulted.previous_end) {
    (1, _, _) => cursor.u64()?,
    (_, 0x20000, _) | (_, _, None) => defaulted.fragment.offset,
    (_, _, Some(end)) => end,
  };

  let description_index = match flags & 2 {
    0 => movie.description_index,
    _written => cursor.u32()?,
  };

  let duration = match flags & 8 {
    0 => movie.duration,
    _written => cursor.u32()?,
  };

  let size = match flags & 0x10 {
    0 => movie.size,
    _written => cursor.u32()?,
  };

  let sample_flags = match flags & 0x20 {
    0 => movie.flags,
    _written => cursor.u32()?,
  };

  Ok(Defaults {
    base,
    description_index,
    duration,
    size,
    flags: sample_flags,
  })
}

fn decode_time(payload: &[u8]) -> Result<i64, Error> {
  let Ok(mut cursor) = Cursor::new(payload, "tfdt");
  let header = cursor.full_header()?;

  let time = match header.version {
    1 => cursor.u64()?,
    _compact => {
      let time = cursor.u32()?;

      u64::from(time)
    }
  };

  let Ok(time_2) = fitted::<u64, i64>(time);

  Ok(time_2)
}

#[derive(Clone, Copy)]
struct Running {
  defaults: Defaults,
  position: u64,
  file_length: u64,
}

fn track_run(
  run: &[u8],
  running: Running,
  track: &mut Track,
  decoded: &mut i64,
) -> Result<u64, Error> {
  let Running {
    defaults,
    mut position,
    file_length,
  } = running;
  let Ok(mut cursor) = Cursor::new(run, "trun");
  let header = cursor.full_header()?;
  let flags = header.flags;
  let per_sample = [0x100_u32, 0x200, 0x400, 0x800]
    .iter()
    .map(|bit| match flags & bit {
      0 => 0_u64,
      _present => 4,
    })
    .sum::<u64>();
  let count = cursor.u32()?;

  match per_sample {
    0 => match ::core::primitive::bool::then_some(u64::from(count).saturating_mul(u64::from(defaults.size.max(1))) > file_length, ()) {
      ::core::option::Option::Some(()) => return Err(Error::TooLarge("trun sample count")),
      ::core::option::Option::None => {}
    },
    per_sample_2 => cursor.check_count(count, per_sample_2)?,
  }

  let Ok(held) = fitted::<_, u64>(track.samples.len());

  match ::core::primitive::bool::then_some(held.saturating_add(u64::from(count)) > MAX_SAMPLES, ()) {
    ::core::option::Option::Some(()) => return Err(Error::TooLarge("sample count")),
    ::core::option::Option::None => {}
  }

  match flags & 1 {
    0 => {}
    _offset => {
      let offset = cursor.i32()?;

      position = defaults.base.wrapping_add_signed(i64::from(offset));
    }
  }

  let first_flags = match flags & 4 {
    0 => None,
    _written => {
      let first = cursor.u32()?;

      Some(first)
    }
  };

  for at in 0..count {
    let sample = run_sample(
      &mut cursor,
      RunSample {
        flags,
        version: header.version,
        defaults,
        first_flags,
        at,
      },
    )?;
    let dts = *decoded;

    track.samples.push(Sample {
      offset: position,
      size: sample.size,
      dts,
      pts: dts.saturating_add(sample.composition),
      duration: sample.duration,
      is_sync: sample.flags & 0x1_0000 == 0,
      description_index: defaults.description_index,
    });

    position = position.saturating_add(u64::from(sample.size));
    *decoded = dts.saturating_add(i64::from(sample.duration));
  }

  Ok(position)
}

#[derive(Clone, Copy)]
struct RunSample {
  flags: u32,
  version: u8,
  defaults: Defaults,
  first_flags: Option<u32>,
  at: u32,
}

struct ReadSample {
  duration: u32,
  size: u32,
  flags: u32,
  composition: i64,
}

fn run_sample(cursor: &mut Cursor<'_>, sample: RunSample) -> Result<ReadSample, Error> {
  let duration = match sample.flags & 0x100 {
    0 => sample.defaults.duration,
    _written => cursor.u32()?,
  };

  let size = match sample.flags & 0x200 {
    0 => sample.defaults.size,
    _written => cursor.u32()?,
  };

  let written_flags = match sample.flags & 0x400 {
    0 => sample.defaults.flags,
    _written => cursor.u32()?,
  };

  let flags = match (sample.at, sample.first_flags) {
    (0, Some(first)) => first,
    (_, _) => written_flags,
  };

  let composition = match sample.flags & 0x800 {
    0 => 0,
    _written => {
      let raw = cursor.u32()?;

      match sample.version {
        0 => i64::from(raw),
        _signed => i64::from(raw.cast_signed()),
      }
    }
  };

  Ok(ReadSample {
    duration,
    size,
    flags,
    composition,
  })
}

fn parse_udta(payload: &[u8], metadata: &mut Metadata) -> Result<(), Never> {
  let Ok(children) = whole_boxes(payload);

  for child in children {
    match (&child.kind.0, child.payload) {
      (b"meta", _) => {
        let Ok(()) = parse_meta(child.payload, metadata);
      }
      ([0xA9, _, _, _], [high, low, _, _, text @ ..]) => {
        let Ok(length) = index(u16::from_be_bytes([*high, *low]));

        match text.get(..length) {
          Some(said) => {
            let Ok(key) = child.kind.to_string_lossy();

            let Ok(label) = display_text(said);
            metadata.entries.push((key, label));
          }
          None => {}
        }
      }
      (_, _) => {}
    }
  }

  Ok(())
}

fn meta_keys(body: &[u8]) -> Result<Vec<Option<String>>, Never> {
  let Ok(found) = find(body, b"keys");

  let payload = match found {
    Some(payload) => payload,
    None => return Ok(Vec::new()),
  };

  let Ok(mut cursor) = Cursor::new(payload, "keys");

  let count = match (cursor.full_header(), cursor.count(8)) {
    (Ok(_header), Ok(count)) => count,
    (_, _) => return Ok(Vec::new()),
  };

  let mut keys = Vec::new();

  for _ in 0..count {
    match meta_key(&mut cursor) {
      Ok(key) => keys.push(key),
      Err(_short) => return Ok(keys),
    }
  }

  Ok(keys)
}

fn meta_key(cursor: &mut Cursor<'_>) -> Result<Option<String>, Error> {
  let size = cursor.u32()?;
  let _namespace = cursor.u32()?;
  let name = cursor.bytes(u64::from(size).saturating_sub(8))?;

  Ok(match std::str::from_utf8(name) {
    Ok(key) => Some(key.to_owned()),
    Err(_invalid_text) => None,
  })
}

fn parse_meta(payload: &[u8], metadata: &mut Metadata) -> Result<(), Never> {
  let body = match payload {
    [_, _, _, _, b'h', b'd', b'l', b'r', ..] => payload,
    [_, _, _, _, rest @ ..] => rest,
    _short => &[],
  };

  let Ok(keys) = meta_keys(body);
  let Ok(found) = find(body, b"ilst");

  let items = match found {
    Some(items) => items,
    None => return Ok(()),
  };

  let Ok(children) = whole_boxes(items);

  for item in children {
    let Ok(number) = item.kind.as_u32();
    let Ok(at) = index(number.saturating_sub(1));

    let key = match (number, keys.get(at)) {
      (1.., Some(Some(key))) => key.clone(),
      (_, _) => {
        let Ok(key) = item.kind.to_string_lossy();

        key
      }
    };

    let Ok(data) = find(item.payload, b"data");

    match data {
      Some(
        [
          kind_high,
          kind_middle,
          kind_low,
          kind_last,
          _,
          _,
          _,
          _,
          text @ ..,
        ],
      ) => {
        match u32::from_be_bytes([*kind_high, *kind_middle, *kind_low, *kind_last]) & 0xFF_FFFF {
          0 | 1 => {
            let Ok(value) = display_text(text);
            metadata.entries.push((key, value));
          }
          _binary => {}
        }
      }
      Some(_) | None => {}
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  use crate::bytes::building::{boxed, claiming, full, words};

  type Outcome = Result<(), Box<dyn std::error::Error>>;

  #[test]
  fn language_codes() {
    let Ok(undetermined) = parse_language(0x55C4);
    let Ok(unwritten) = parse_language(0);
    let Ok(english) = parse_language(0x15C7);

    assert_eq!(
      (undetermined.as_str(), unwritten.as_str(), english.as_str()),
      ("und", "eng", "eng")
    );
  }

  #[test]
  fn stts_cursor() {
    let runs = [(2, 10), (0, 99), (3, 5)];
    let Ok(mut cursor) = Runs::new(&runs);
    let Ok(first) = cursor.next_value();
    let Ok(three) = cursor.take(3);
    let Ok(last) = cursor.next_value();
    let Ok(past) = cursor.next_value();

    assert_eq!((first, three, last, past), (10, 20, 5, 0));
  }

  #[test]
  fn rejects_no_moov_and_cmov() {
    let Ok(free) = boxed(b"free", &[0; 4]);

    assert!(matches!(open(free.clone()), Err(Error::NoMoov)));

    let Ok(compressed) = boxed(b"cmov", &[]);
    let Ok(moov) = boxed(b"moov", &compressed);

    assert!(matches!(
      open([free, moov].concat()),
      Err(Error::Unsupported(Unsupported::CompressedMovieHeader))
    ));
  }

  #[test]
  fn metadata_qt_and_ilst() {
    let Ok(name) = boxed(&[0xA9, b'n', b'a', b'm'], &[0, 2, 0, 0, b'h', b'i']);
    let Ok(handler) = boxed(b"hdlr", &[0; 24]);
    let Ok(said) = boxed(b"data", &[0, 0, 0, 1, 0, 0, 0, 0, b'm', b'e']);
    let Ok(artist) = boxed(&[0xA9, b'A', b'R', b'T'], &said);
    let Ok(items) = boxed(b"ilst", &artist);
    let Ok(meta) = full(b"meta", 0, &[handler, items].concat());
    let mut metadata = Metadata::default();
    let Ok(()) = parse_udta(&[name, meta].concat(), &mut metadata);

    let Ok(title) = metadata.title();
    let Ok(artist_name) = metadata.get("©ART");

    assert_eq!((title, artist_name), (Some("hi"), Some("me")));
  }

  fn jpeg_entry() -> Result<Vec<u8>, Never> {
    let reference = [0_u8, 0, 0, 0, 0, 0, 0, 1];
    let Ok(header) = words(&[0, 0, 0, 0, 0x0140_00F0, 0x0048_0000, 0x0048_0000, 0]);
    let tail = [[0_u8, 1].as_slice(), &[0; 32], &[0, 24, 0xFF, 0xFF]].concat();

    boxed(b"jpeg", &[reference.as_slice(), &header, &tail].concat())
  }

  fn leading() -> Result<Vec<u8>, Never> {
    let Ok(brands) = boxed(b"ftyp", b"isom\0\0\0\0isom");
    let Ok(user) = boxed(b"uuid", &[[0xAB_u8; 16].as_slice(), b"payload"].concat());
    let Ok(wide) = boxed(b"wide", &[]);
    let Ok(skip) = boxed(b"skip", &[1, 2, 3]);

    Ok([brands, user, wide, skip].concat())
  }

  fn media_data() -> Result<Vec<u8>, Never> {
    Ok(
      [
        [0_u8, 0, 0, 1].as_slice(),
        b"mdat",
        &26_u64.to_be_bytes(),
        &[1, 1, 1, 2, 3, 3, 4, 4, 4, 4],
      ]
      .concat(),
    )
  }

  fn sample_table(data_start: u64, sizes: Option<(u32, u32)>) -> Result<Vec<u8>, Never> {
    let Ok(entry) = jpeg_entry();
    let Ok(described) = full(
      b"stsd",
      0,
      &[1_u32.to_be_bytes().as_slice(), &entry].concat(),
    );
    let Ok(durations) = words(&[1, 4, 10]);
    let Ok(durations_2) = full(b"stts", 0, &durations);
    let Ok(offsets) = words(&[2, 2, (-5_i32).cast_unsigned(), 2, 5]);
    let Ok(offsets_2) = full(b"ctts", 1, &offsets);
    let Ok(sync) = words(&[2, 3, 1]);
    let Ok(sync_2) = full(b"stss", 0, &sync);
    let Ok(chunks) = words(&[2, 1, 3, 1, 2, 1, 1]);
    let Ok(chunks_2) = full(b"stsc", 0, &chunks);

    let Ok(sizes_2) = match sizes {
      Some((size, count)) => {
        let Ok(said) = words(&[size, count]);

        full(b"stsz", 0, &said)
      }
      None => full(b"stz2", 0, &[0, 0, 0, 4, 0, 0, 0, 4, 0x31, 0x24]),
    };

    let Ok(placed) = full(
      b"co64",
      0,
      &[
        2_u32.to_be_bytes().as_slice(),
        &data_start.to_be_bytes(),
        &data_start.saturating_add(6).to_be_bytes(),
      ]
      .concat(),
    );
    let Ok(dependencies) = full(b"sdtp", 0, &[0x20, 0x10, 0x10, 0x10]);
    let Ok(grouped) = full(b"sbgp", 0, b"roll\0\0\0\x01\0\0\0\x04\0\0\0\x01");
    let Ok(groups) = full(b"sgpd", 1, b"roll\0\0\0\x02\0\0\0\x01\xFF\xFF");

    boxed(
      b"stbl",
      &[
        described,
        durations_2,
        offsets_2,
        sync_2,
        chunks_2,
        sizes_2,
        placed,
        dependencies,
        grouped,
        groups,
      ]
      .concat(),
    )
  }

  fn movie_box(tables: &[u8]) -> Result<Vec<u8>, Never> {
    let Ok(movie_header) = words(&[0, 0, 1000, 500]);
    let Ok(movie_header_2) = full(b"mvhd", 0, &[movie_header.as_slice(), &[0; 80]].concat());
    let Ok(track_header) = words(&[0, 0, 7, 0, 500]);
    let Ok(dimensions) = words(&[0x0140_0000, 0x00F0_0000]);
    let Ok(track_header_2) = full(
      b"tkhd",
      0,
      &[track_header.as_slice(), &[0; 52], &dimensions].concat(),
    );

    let edits = [
      2_u32.to_be_bytes().as_slice(),
      &100_u64.to_be_bytes(),
      &(-1_i64).to_be_bytes(),
      &0x10000_i32.to_be_bytes(),
      &400_u64.to_be_bytes(),
      &20_i64.to_be_bytes(),
      &0x10000_i32.to_be_bytes(),
    ]
    .concat();

    let Ok(edits_2) = full(b"elst", 1, &edits);
    let Ok(edits_3) = boxed(b"edts", &edits_2);
    let Ok(media_header) = words(&[0, 0, 100, 40, 0x15C7_0000]);
    let Ok(media_header_2) = full(b"mdhd", 0, &media_header);
    let Ok(handler) = full(
      b"hdlr",
      0,
      &[[0_u8; 4].as_slice(), b"vide", &[0; 12], b"Video\0"].concat(),
    );
    let Ok(information) = boxed(b"minf", tables);
    let Ok(media) = boxed(b"mdia", &[media_header_2, handler, information].concat());
    let Ok(track) = boxed(b"trak", &[track_header_2, edits_3, media].concat());

    boxed(b"moov", &[movie_header_2, track].concat())
  }

  fn handmade(sizes: Option<(u32, u32)>) -> Result<Vec<u8>, Never> {
    let Ok(leading) = leading();
    let Ok(before) = fitted::<_, u64>(leading.len());
    let data_start = before.saturating_add(16);
    let Ok(media) = media_data();
    let Ok(tables) = sample_table(data_start, sizes);
    let Ok(movie) = movie_box(&tables);
    let Ok(trailing) = claiming(b"free", &[9; 5], 0);

    Ok([leading, media, movie, trailing].concat())
  }

  #[test]
  fn handmade_sample_table() -> Outcome {
    let Ok(made) = handmade(None);
    let film = open(made.as_slice())?;
    let track = film.tracks.first().ok_or("no track")?;
    let first = track.samples.first().ok_or("no sample")?;

    assert_eq!(film.major_brand, *b"isom");
    assert_eq!(
      (
        track.id,
        track.kind,
        track.timescale,
        track.language.as_str()
      ),
      (7, TrackKind::Video, 100, "eng")
    );
    assert_eq!((track.width, track.height), (320, 240));
    assert_eq!(track.handler_name, "Video");

    let sizes: Vec<u32> = track.samples.iter().map(|sample| sample.size).collect();
    let offsets: Vec<u64> = track
      .samples
      .iter()
      .map(|sample| sample.offset.saturating_sub(first.offset))
      .collect();
    let shown: Vec<i64> = track.samples.iter().map(|sample| sample.pts).collect();
    let sync: Vec<bool> = track.samples.iter().map(|sample| sample.is_sync).collect();

    assert_eq!(sizes, [3, 1, 2, 4]);
    assert_eq!(offsets, [0, 3, 4, 6]);
    assert_eq!(shown, [-5, 5, 25, 35]);
    assert_eq!(sync, [true, false, true, false]);

    for (sample, wanted) in (0_u32..).zip([vec![1_u8, 1, 1], vec![2], vec![3, 3], vec![4; 4]]) {
      let read = track.read_sample(made.as_slice(), sample)?;

      assert_eq!(read, wanted);
    }

    let group = track.sample_groups.first().ok_or("no sample group")?;

    assert_eq!(track.edits.len(), 2);
    assert_eq!(track.edit_offset, -10);
    let Ok(presented) = track.presentation_pts(2);

    assert_eq!(presented, Some(15));
    assert_eq!(track.sdtp.len(), 4);
    assert_eq!(group.grouping_type, *b"roll");
    assert_eq!(group.descriptions, vec![vec![0xFF, 0xFF]]);
    assert!(matches!(
      track.read_sample(made.as_slice(), 4),
      Err(Error::NoSuchSample(4))
    ));

    Ok(())
  }

  #[derive(Clone, Copy)]
  enum Field {
    Size,
    Count,
  }

  fn overwritten(bytes: &mut [u8], kind: &[u8; 4], field: Field, with: u32) -> Outcome {
    let named = bytes
      .windows(4)
      .zip(0_u32..)
      .find(|(window, _)| *window == kind)
      .map(|(_, at)| at);

    let place = named
      .and_then(|at| match field {
        Field::Size => at.checked_sub(4),
        Field::Count => at.checked_add(8),
      })
      .ok_or("no such box")?;

    let Ok(start) = index(place);
    let target = bytes
      .get_mut(start..)
      .and_then(|rest| rest.first_chunk_mut::<4>())
      .ok_or("no such field")?;

    *target = with.to_be_bytes();

    Ok(())
  }

  #[test]
  fn absurd_counts_are_bounded() -> Outcome {
    let Ok(constant) = handmade(Some((1, u32::MAX)));
    let film = open(constant.as_slice())?;
    let track = film.tracks.first().ok_or("no track")?;

    assert!(track.samples.len() <= constant.len());

    let Ok(made) = handmade(None);
    let mut counted = made.clone();
    overwritten(&mut counted, b"stts", Field::Count, u32::MAX)?;

    assert!(matches!(open(counted.as_slice()), Err(Error::Truncated(_))));

    let mut oversized = made;
    overwritten(&mut oversized, b"stsc", Field::Size, 0x7FFF_FFFF)?;

    assert!(matches!(open(oversized.as_slice()), Err(Error::Framing(_))));

    Ok(())
  }
}
