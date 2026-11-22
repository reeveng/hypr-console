//! A file built by hand, to read what FFmpeg does not write: an unknown-size
//! Segment and Clusters, header stripping, a BlockGroup, Colour with mastering
//! metadata, a projection, Chapters, Tags, an Attachment and a CRC-32.
//!
//! The file is written with the shared EBML crate's own writers, a stage of it
//! to a function, and read back through the sample tables: what a frame is,
//! where it begins, when, and what reading it hands back.

use console_core_extensible_binary_meta_language::{write_id, write_size, write_unknown_size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::codec::Codec;
use crate::ebml;
use crate::error::{Error, Warning};
use crate::ids::*;
use crate::states::{ColourRange, FrameKind};
use crate::track::{Colour, ContentEncoding};
use crate::{MkvFile, OpenOptions, open_with};

fn el(out: &mut Vec<u8>, id: u32, payload: &[u8]) -> Result<(), Never> {
  let Ok(size) = fitted::<_, u64>(payload.len());
  let Ok(()) = write_id(out, id);
  let Ok(()) = write_size(out, size, 1);

  out.extend_from_slice(payload);

  Ok(())
}

fn el_uint(out: &mut Vec<u8>, id: u32, value: u64) -> Result<(), Never> {
  let bytes = value.to_be_bytes();
  let Ok(skipped) = index(value.leading_zeros().wrapping_shr(3).min(7));

  el(
    out,
    id,
    match bytes.get(skipped..) {
      Some(significant) => significant,
      None => &bytes,
    },
  )
}

fn el_str(out: &mut Vec<u8>, id: u32, text: &str) -> Result<(), Never> {
  el(out, id, text.as_bytes())
}

fn block(track: u8, relative: i16, kind: FrameKind, data: &[u8]) -> Result<Vec<u8>, Never> {
  let flags = match kind {
    FrameKind::Keyframe => 0x80,
    FrameKind::Delta => 0,
  };
  let mut written = vec![0x80 | track];

  written.extend_from_slice(&relative.to_be_bytes());
  written.push(flags);
  written.extend_from_slice(data);

  Ok(written)
}

fn head(file: &mut Vec<u8>) -> Result<(), Never> {
  let mut header = Vec::new();
  let mut info = Vec::new();
  let mut checked = Vec::new();

  let Ok(()) = el_str(&mut header, DOC_TYPE, "matroska");
  let Ok(()) = el_uint(&mut header, DOC_TYPE_VERSION, 4);
  let Ok(()) = el(file, EBML, &header);
  let Ok(()) = write_id(file, SEGMENT);
  let Ok(()) = write_unknown_size(file, 8);
  let Ok(()) = el_uint(&mut info, TIMESTAMP_SCALE, 1_000_000);
  let Ok(()) = el_str(&mut info, TITLE, "Synthetic");
  let Ok(()) = el(&mut info, VOID, &[0; 3]);
  let Ok(checksum) = ebml::crc32(&info);
  let Ok(()) = el(&mut checked, CRC32, &checksum.to_le_bytes());

  checked.extend_from_slice(&info);

  el(file, INFO, &checked)
}

fn colour() -> Result<Vec<u8>, Never> {
  let mut mastering = Vec::new();
  let mut colour = Vec::new();

  for (id, value) in [
    (PRIMARY_R_X, 0.708),
    (PRIMARY_R_Y, 0.292),
    (PRIMARY_G_X, 0.17),
    (PRIMARY_G_Y, 0.797),
    (PRIMARY_B_X, 0.131),
    (PRIMARY_B_Y, 0.046),
    (WHITE_POINT_X, 0.3127),
    (WHITE_POINT_Y, 0.329),
    (LUMINANCE_MAX, 1000.0),
    (LUMINANCE_MIN, 0.0001),
  ] {
    let Ok(()) = el(&mut mastering, id, &f64::to_be_bytes(value));
  }

  for (id, value) in [
    (MATRIX_COEFFICIENTS, 9),
    (RANGE, 2),
    (TRANSFER_CHARACTERISTICS, 16),
    (PRIMARIES, 9),
    (MAX_CLL, 1000),
    (MAX_FALL, 400),
  ] {
    let Ok(()) = el_uint(&mut colour, id, value);
  }

  let Ok(()) = el(&mut colour, MASTERING_METADATA, &mastering);

  Ok(colour)
}

fn video_track() -> Result<Vec<u8>, Never> {
  let mut track = Vec::new();
  let mut video = Vec::new();
  let mut projection = Vec::new();
  let mut compression = Vec::new();
  let mut encoding = Vec::new();
  let mut encodings = Vec::new();
  let Ok(colour) = colour();

  for (id, value) in [
    (TRACK_NUMBER, 1),
    (TRACK_UID, 11),
    (TRACK_TYPE, 1),
    (DEFAULT_DURATION, 40_000_000),
  ] {
    let Ok(()) = el_uint(&mut track, id, value);
  }

  for (id, value) in [
    (PIXEL_WIDTH, 1440),
    (PIXEL_HEIGHT, 1080),
    (DISPLAY_WIDTH, 1920),
    (DISPLAY_HEIGHT, 1080),
  ] {
    let Ok(()) = el_uint(&mut video, id, value);
  }

  let Ok(()) = el_str(&mut track, CODEC_ID, "V_MPEG4/ISO/AVC");
  let Ok(()) = el(&mut track, CODEC_PRIVATE, &[1, 0x64, 0, 0x1F]);
  let Ok(()) = el(&mut video, COLOUR, &colour);
  let Ok(()) = el_uint(&mut projection, PROJECTION_TYPE, 0);
  let Ok(()) = el(
    &mut projection,
    PROJECTION_POSE_ROLL,
    &(-90.0_f64).to_be_bytes(),
  );
  let Ok(()) = el(&mut video, PROJECTION, &projection);
  let Ok(()) = el(&mut track, VIDEO, &video);
  let Ok(()) = el_uint(&mut compression, CONTENT_COMP_ALGO, 3);
  let Ok(()) = el(&mut compression, CONTENT_COMP_SETTINGS, &[0, 0, 0, 1]);
  let Ok(()) = el(&mut encoding, CONTENT_COMPRESSION, &compression);
  let Ok(()) = el(&mut encodings, CONTENT_ENCODING, &encoding);
  let Ok(()) = el(&mut track, CONTENT_ENCODINGS, &encodings);

  Ok(track)
}

fn audio_track() -> Result<Vec<u8>, Never> {
  let mut track = Vec::new();
  let mut audio = Vec::new();

  let Ok(()) = el_uint(&mut track, TRACK_NUMBER, 2);
  let Ok(()) = el_uint(&mut track, TRACK_TYPE, 2);
  let Ok(()) = el_str(&mut track, CODEC_ID, "A_AAC/MPEG4/LC/SBR");
  let Ok(()) = el_str(&mut track, LANGUAGE, "ger");
  let Ok(()) = el(&mut audio, SAMPLING_FREQUENCY, &24_000.0_f64.to_be_bytes());
  let Ok(()) = el(
    &mut audio,
    OUTPUT_SAMPLING_FREQUENCY,
    &48_000.0_f64.to_be_bytes(),
  );
  let Ok(()) = el_uint(&mut audio, CHANNELS, 2);
  let Ok(()) = el(&mut track, AUDIO, &audio);

  Ok(track)
}

fn listed(file: &mut Vec<u8>) -> Result<(), Never> {
  let mut tracks = Vec::new();
  let mut display = Vec::new();
  let mut atom = Vec::new();
  let mut edition = Vec::new();
  let mut chapters = Vec::new();
  let mut attached = Vec::new();
  let mut attachments = Vec::new();
  let Ok(video) = video_track();
  let Ok(audio) = audio_track();

  let Ok(()) = el(&mut tracks, TRACK_ENTRY, &video);
  let Ok(()) = el(&mut tracks, TRACK_ENTRY, &audio);
  let Ok(()) = el(file, TRACKS, &tracks);
  let Ok(()) = el_str(&mut display, CHAP_STRING, "Intro");
  let Ok(()) = el_uint(&mut atom, CHAPTER_UID, 5);
  let Ok(()) = el_uint(&mut atom, CHAPTER_TIME_START, 0);
  let Ok(()) = el_uint(&mut atom, CHAPTER_TIME_END, 40_000_000);
  let Ok(()) = el(&mut atom, CHAPTER_DISPLAY, &display);
  let Ok(()) = el(&mut edition, CHAPTER_ATOM, &atom);
  let Ok(()) = el(&mut chapters, EDITION_ENTRY, &edition);
  let Ok(()) = el(file, CHAPTERS, &chapters);
  let Ok(()) = el_str(&mut attached, FILE_NAME, "font.ttf");
  let Ok(()) = el_str(&mut attached, FILE_MEDIA_TYPE, "font/ttf");
  let Ok(()) = el_uint(&mut attached, FILE_UID, 77);
  let Ok(()) = el(&mut attached, FILE_DATA, &[9; 100]);
  let Ok(()) = el(&mut attachments, ATTACHED_FILE, &attached);

  el(file, ATTACHMENTS, &attachments)
}

fn clusters(file: &mut Vec<u8>) -> Result<(), Never> {
  let mut group = Vec::new();
  let Ok(first) = block(1, 0, FrameKind::Keyframe, &[0x65, 1, 2]);
  let Ok(sound) = block(2, 5, FrameKind::Keyframe, &[0xAA; 10]);
  let Ok(grouped) = block(1, 40, FrameKind::Delta, &[0x41, 3]);
  let Ok(stray) = block(9, 0, FrameKind::Keyframe, &[1]);
  let Ok(later) = block(1, 0, FrameKind::Keyframe, &[0x65, 4]);

  let Ok(()) = write_id(file, CLUSTER);
  let Ok(()) = write_unknown_size(file, 1);
  let Ok(()) = el_uint(file, TIMESTAMP, 1000);
  let Ok(()) = el(file, SIMPLE_BLOCK, &first);
  let Ok(()) = el(file, SIMPLE_BLOCK, &sound);
  let Ok(()) = el(&mut group, BLOCK, &grouped);
  let Ok(()) = el(&mut group, REFERENCE_BLOCK, &[0xD8]);
  let Ok(()) = el_uint(&mut group, BLOCK_DURATION, 80);
  let Ok(()) = el(file, BLOCK_GROUP, &group);
  let Ok(()) = el(file, SIMPLE_BLOCK, &stray);
  let Ok(()) = write_id(file, CLUSTER);
  let Ok(()) = write_unknown_size(file, 8);
  let Ok(()) = el_uint(file, TIMESTAMP, 1120);

  el(file, SIMPLE_BLOCK, &later)
}

fn tags(file: &mut Vec<u8>) -> Result<(), Never> {
  let mut simple = Vec::new();
  let mut targets = Vec::new();
  let mut tag = Vec::new();
  let mut tags = Vec::new();

  let Ok(()) = el_str(&mut simple, TAG_NAME, "ARTIST");
  let Ok(()) = el_str(&mut simple, TAG_STRING, "Nobody");
  let Ok(()) = el_uint(&mut targets, TAG_TRACK_UID, 11);
  let Ok(()) = el(&mut tag, TARGETS, &targets);
  let Ok(()) = el(&mut tag, SIMPLE_TAG, &simple);
  let Ok(()) = el(&mut tags, TAG, &tag);

  el(file, TAGS, &tags)
}

fn synthetic() -> Result<Vec<u8>, Never> {
  let mut file = Vec::new();

  let Ok(()) = head(&mut file);
  let Ok(()) = listed(&mut file);
  let Ok(()) = clusters(&mut file);
  let Ok(()) = tags(&mut file);

  Ok(file)
}

const INDEXED: OpenOptions = OpenOptions {
  index: true,
  verify_crc: true,
};

const HEADERS_ONLY: OpenOptions = OpenOptions {
  index: false,
  verify_crc: true,
};

fn opened(bytes: &[u8], options: &OpenOptions) -> Result<MkvFile, Error> {
  open_with(bytes, options)
}

#[test]
fn the_segment_reads_whole_with_its_crc_checked() -> Result<(), Error> {
  let Ok(bytes) = synthetic();
  let file = opened(&bytes, &INDEXED)?;

  assert!(file.live);
  assert_eq!(file.warnings, Vec::new());
  assert_eq!((file.crc_checked, file.crc_errors.len()), (1, 0));
  assert_eq!(file.info.title.as_deref(), Some("Synthetic"));
  assert_eq!(file.clusters.len(), 2);

  Ok(())
}

#[test]
fn the_video_track_says_its_shape_colour_and_encoding() -> Result<(), Error> {
  let Ok(bytes) = synthetic();
  let file = opened(&bytes, &INDEXED)?;
  let video = file
    .tracks
    .first()
    .map(|track| (&track.codec, &track.encodings, &track.video));
  let (codec, encodings, shape) = match video {
    Some((codec, encodings, Some(shape))) => (codec, encodings, shape),
    Some((_, _, None)) | None => return Err(Error::NoSuchTrack(0)),
  };
  let (colour, mastering) = match &shape.colour {
    Some(
      colour @ Colour {
        mastering: Some(mastering),
        ..
      },
    ) => (colour, mastering),
    Some(Colour {
      mastering: None, ..
    })
    | None => return Err(Error::NoSuchTrack(0)),
  };
  let [_, green, _] = mastering.primaries;

  assert_eq!(
    codec,
    &Codec::Avc {
      avcc: vec![1, 0x64, 0, 0x1F]
    }
  );
  assert_eq!(
    encodings,
    &vec![ContentEncoding::HeaderStripping {
      bytes: vec![0, 0, 0, 1],
      scope: 1
    }]
  );
  assert_eq!(shape.pixel_aspect(), Ok((4, 3)));
  assert_eq!(
    shape
      .projection
      .as_ref()
      .map(|projection| projection.roll.0),
    Some(-90.0)
  );
  assert_eq!(shape.display_rotation(), Ok(Some(1)));
  assert_eq!(colour.full_range(), Ok(ColourRange::Full));
  assert_eq!(
    (
      colour.matrix_coefficients,
      colour.transfer_characteristics,
      colour.primaries,
      colour.max_cll,
      colour.max_fall
    ),
    (Some(9), Some(16), Some(9), Some(1000), Some(400))
  );
  assert_eq!(green, (0.17, 0.797));
  assert_eq!(mastering.white_point, (0.3127, 0.329));
  assert_eq!(
    (mastering.luminance_max.0, mastering.luminance_min.0),
    (1000.0, 0.0001)
  );

  Ok(())
}

#[test]
fn the_audio_track_and_the_segment_metadata_read_as_written() -> Result<(), Error> {
  let Ok(bytes) = synthetic();
  let file = opened(&bytes, &INDEXED)?;
  let audio = file
    .tracks
    .get(1)
    .map(|track| (&track.codec, track.language.as_str()));
  let chapter = file
    .chapters
    .first()
    .and_then(|edition| edition.chapters.first());
  let attachment = file.attachments.first().map(|found| {
    (
      found.name.as_str(),
      found.media_type.as_str(),
      found.uid,
      found.data_offset,
    )
  });
  let tag = file.tags.first();
  let payload = attachment.and_then(|(_, _, _, at)| {
    let Ok(from) = index(at);

    bytes.get(from..from.saturating_add(100))
  });

  assert_eq!(
    audio,
    Some((
      &Codec::Aac {
        asc: vec![0x13, 0x10, 0x56, 0xE5, 0x98]
      },
      "ger"
    ))
  );
  assert_eq!(
    chapter.map(|read| (read.title(), read.end_ns)),
    Some((Ok(Some("Intro")), Some(40_000_000)))
  );
  assert_eq!(file.attachments.len(), 1);
  assert_eq!(
    attachment.map(|(name, media_type, uid, _)| (name, media_type, uid)),
    Some(("font.ttf", "font/ttf", 77))
  );
  assert_eq!(payload, Some([9u8; 100].as_slice()));
  assert_eq!(
    tag.map(|read| read.targets.track_uids.clone()),
    Some(vec![11])
  );
  assert_eq!(
    tag
      .and_then(|read| read.simple.first())
      .and_then(|simple| simple.string.as_deref()),
    Some("Nobody")
  );

  Ok(())
}

#[test]
fn the_sample_tables_hold_every_frame_and_read_it_back() -> Result<(), Error> {
  let Ok(bytes) = synthetic();
  let file = opened(&bytes, &INDEXED)?;
  let video = file.tracks.first().map(|track| {
    track
      .samples
      .iter()
      .map(|sample| (sample.pts, sample.keyframe, sample.duration))
      .collect::<Vec<_>>()
  });
  let sound = file
    .tracks
    .get(1)
    .and_then(|track| track.samples.first())
    .map(|sample| (sample.pts, sample.size));
  let keyframes = file.tracks.first().map(|track| track.keyframes());
  let late = file.tracks.first().map(|track| track.ticks_to_ns(1120));
  let keyframe = file.read_sample(bytes.as_slice(), 0, 0)?;
  let grouped = file.read_sample(bytes.as_slice(), 0, 1)?;

  assert_eq!(
    video,
    Some(vec![(1000, true, 40), (1040, false, 80), (1120, true, 40)])
  );
  assert_eq!(sound, Some((1005, 10)));
  assert_eq!(keyframes, Some(Ok([0u32, 2].as_slice())));
  assert_eq!(late, Some(Ok(1_120_000_000)));
  assert_eq!(keyframe, vec![0, 0, 0, 1, 0x65, 1, 2]);
  assert_eq!(grouped, vec![0, 0, 0, 1, 0x41, 3]);
  assert_eq!(file.keyframe_before(0, 1_100_000_000), Ok(Some(0)));
  assert_eq!(file.keyframe_before(0, 1_130_000_000), Ok(Some(2)));
  assert!(matches!(
    file.read_sample(bytes.as_slice(), 0, 3),
    Err(Error::NoSuchSample(3))
  ));
  assert!(matches!(
    file.read_sample(bytes.as_slice(), 2, 0),
    Err(Error::NoSuchTrack(2))
  ));

  Ok(())
}

#[test]
fn a_damaged_title_is_a_crc_mismatch_and_not_a_fault() -> Result<(), Error> {
  let Ok(mut bytes) = synthetic();
  let title = bytes
    .windows(9)
    .zip(0u64..)
    .find(|(word, _)| *word == b"Synthetic")
    .map(|(_, at)| at);

  let letter = match title {
    Some(at) => {
      let Ok(position) = index(at);

      bytes.get_mut(position)
    }
    None => None,
  };

  match letter {
    Some(letter_2) => *letter_2 = b's',
    None => {}
  }

  let file = opened(&bytes, &INDEXED)?;

  assert_eq!(file.crc_errors.len(), 1);
  assert_eq!(file.warnings.len(), 1);
  assert!(matches!(
    file.warnings.first(),
    Some(Warning::CrcMismatch { .. })
  ));

  Ok(())
}

#[test]
fn damage_inside_a_cluster_ends_it_and_the_next_cluster_is_found() -> Result<(), Error> {
  let Ok(mut bytes) = synthetic();
  let cluster = CLUSTER.to_be_bytes();
  let second = bytes
    .windows(4)
    .zip(0u64..)
    .filter(|(four, _)| *four == cluster)
    .map(|(_, at)| at)
    .nth(1);

  match second {
    Some(at) => {
      let Ok(position) = index(at);
      let _kept = bytes.splice(position..position, [0, 0, 0]).count();
    }
    None => return Err(Error::Invalid(crate::error::Invalid::NoSegment)),
  }

  let file = opened(&bytes, &INDEXED)?;
  let video = file.tracks.first().map(|track| {
    track
      .samples
      .iter()
      .map(|sample| sample.pts)
      .collect::<Vec<_>>()
  });

  assert!(matches!(
    file.warnings.as_slice(),
    [Warning::DamagedCluster { .. }]
  ));
  assert_eq!(file.clusters.len(), 2);
  assert_eq!(video, Some(vec![1000, 1040, 1120]));

  Ok(())
}

#[test]
fn an_open_without_an_index_stops_at_the_first_cluster() -> Result<(), Error> {
  let Ok(bytes) = synthetic();
  let file = opened(&bytes, &HEADERS_ONLY)?;

  assert_eq!(file.tracks.len(), 2);
  assert!(!file.indexed);
  assert!(file.clusters.is_empty());
  assert!(file.first_cluster.is_some());
  assert!(file.tracks.iter().all(|track| track.samples.is_empty()));

  Ok(())
}
