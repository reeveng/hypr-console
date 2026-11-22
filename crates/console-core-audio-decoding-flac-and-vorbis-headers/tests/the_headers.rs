use console_core_audio_decoding_flac_and_vorbis_headers::flac::metadata::{
  MetadataBlockBoundary, MetadataBlockHeader, MetadataBlockType, StreamInfo,
  read_application_block, read_cuesheet_block, read_seek_table_block,
};
use console_core_audio_decoding_flac_and_vorbis_headers::vorbis::vorbis_channels_to_channels;
use console_core_audio_decoding_interfaces::audio::Channels;
use console_core_audio_decoding_interfaces::errors::Error;
use console_core_audio_decoding_interfaces::formats::Cue;
use console_core_audio_decoding_interfaces::formats::util::{
  SeekIndex, SeekPoint, SeekSearchResult,
};
use console_core_audio_decoding_interfaces::meta::StandardTagKey;
use console_core_byte_streams::{BufReader, ReadBytes};
use console_core_never::Never;

type Failure = Box<dyn std::error::Error>;

const CHECKSUM: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

const NO_CHECKSUM: [u8; 16] = [0; 16];

const STEREO_SIXTEEN_BIT: u64 = (44_100 << 44) | (1 << 41) | (15 << 36) | 1_234_567;

const LENGTH_UNKNOWN: u64 = (96_000 << 44) | (5 << 41) | (23 << 36);

const NO_SAMPLE_RATE: u64 = (1 << 41) | (15 << 36);

const SAMPLE_RATE_TOO_HIGH: u64 = (655_351 << 44) | (1 << 41) | (15 << 36);

const THREE_BITS_A_SAMPLE: u64 = (44_100 << 44) | (1 << 41) | (2 << 36);

const SECTOR: u64 = 588;

const COMPACT_DISC: u8 = 0x80;

const NOT_COMPACT_DISC: u8 = 0;

const LEAD_OUT: u8 = 170;

const RECORDING: &[u8; 12] = b"USRC17607839";

const NO_RECORDING: &[u8; 12] = &[0; 12];

const NO_FLAGS: u16 = 0;

struct Stream {
  shortest_block: u16,
  longest_block: u16,
  smallest_frame: u32,
  largest_frame: u32,
  packed: u64,
  checksum: [u8; 16],
}

const ORDINARY: Stream = Stream {
  shortest_block: 4096,
  longest_block: 4096,
  smallest_frame: 14,
  largest_frame: 14_892,
  packed: STEREO_SIXTEEN_BIT,
  checksum: CHECKSUM,
};

struct Sheet {
  catalog: &'static [u8],
  lead_in: u64,
  kind: u8,
  reserved: u16,
  tracks: u8,
}

const DISC: Sheet = Sheet {
  catalog: b"1234567890123",
  lead_in: 88_200,
  kind: COMPACT_DISC,
  reserved: 0,
  tracks: 2,
};

struct Track {
  offset: u64,
  number: u8,
  recording: &'static [u8; 12],
  flags: u16,
  reserved: u32,
  points: u8,
}

struct Point {
  offset: u64,
  packed: u32,
}

fn stream_info(stream: &Stream) -> Result<Vec<u8>, Never> {
  let mut bytes = Vec::new();

  bytes.extend(stream.shortest_block.to_be_bytes());
  bytes.extend(stream.longest_block.to_be_bytes());
  bytes.extend(stream.smallest_frame.to_be_bytes().iter().skip(1));
  bytes.extend(stream.largest_frame.to_be_bytes().iter().skip(1));
  bytes.extend(stream.packed.to_be_bytes());
  bytes.extend(stream.checksum);

  Ok(bytes)
}

fn read(stream: &Stream) -> Result<StreamInfo, Error> {
  let Ok(bytes) = stream_info(stream);

  let Ok(mut reader) = BufReader::new(&bytes);

  StreamInfo::read(&mut reader)
}

fn refused<Read, Fault>(read: Result<Read, Fault>) -> Result<String, Never>
where
  Error: From<Fault>,
{
  Ok(match read {
    Ok(_read) => String::from("read"),
    Err(fault) => Error::from(fault).to_string(),
  })
}

fn assert_refused<Read, Fault>(read: Result<Read, Fault>, what: &str) -> Result<(), Never>
where
  Error: From<Fault>,
{
  let Ok(said) = refused(read);

  assert_ne!(said, "read", "{what}");

  Ok(())
}

fn sheet(sheet: &Sheet) -> Result<Vec<u8>, Never> {
  let mut bytes = Vec::new();

  bytes.extend(sheet.catalog);
  bytes.resize(128, 0);
  bytes.extend(sheet.lead_in.to_be_bytes());
  bytes.push(sheet.kind);
  bytes.extend(sheet.reserved.to_be_bytes());
  bytes.extend([0u8; 256]);
  bytes.push(sheet.tracks);

  Ok(bytes)
}

fn track(track: &Track) -> Result<Vec<u8>, Never> {
  let mut bytes = Vec::new();

  bytes.extend(track.offset.to_be_bytes());
  bytes.push(track.number);
  bytes.extend(track.recording);
  bytes.extend(track.flags.to_be_bytes());
  bytes.extend([0u8; 8]);
  bytes.extend(track.reserved.to_be_bytes());
  bytes.push(track.points);

  Ok(bytes)
}

fn point(point: &Point) -> Result<Vec<u8>, Never> {
  let mut bytes = Vec::new();

  bytes.extend(point.offset.to_be_bytes());
  bytes.extend(point.packed.to_be_bytes());

  Ok(bytes)
}

fn cues(bytes: &[u8]) -> Result<Vec<Cue>, Error> {
  let mut cues = Vec::new();
  let Ok(mut reader) = BufReader::new(bytes);

  read_cuesheet_block(&mut reader, &mut cues)?;

  Ok(cues)
}

fn one_track(kind: u8, track_read: &Track, point_read: &Point) -> Result<String, Never> {
  let Ok(mut bytes) = sheet(&Sheet {
    catalog: b"",
    lead_in: 0,
    kind,
    reserved: 0,
    tracks: 1,
  });
  let Ok(track) = track(track_read);
  let Ok(point) = point(point_read);

  bytes.extend(track);
  bytes.extend(point);

  refused(cues(&bytes))
}

fn header(bytes: [u8; 4]) -> Result<MetadataBlockHeader, Error> {
  let Ok(mut reader) = BufReader::new(&bytes);

  MetadataBlockHeader::read(&mut reader)
}

#[test]
fn a_stream_information_block_is_read_field_by_field() -> Result<(), Failure> {
  let info = read(&ORDINARY)?;

  assert_eq!(info.block_len_min, 4096);
  assert_eq!(info.block_len_max, 4096);
  assert_eq!(info.frame_byte_len_min, 14);
  assert_eq!(info.frame_byte_len_max, 14_892);
  assert_eq!(info.sample_rate, 44_100);
  assert_eq!(info.channels, Channels::FRONT_LEFT | Channels::FRONT_RIGHT);
  assert_eq!(info.bits_per_sample, 16);
  assert_eq!(info.n_samples, Some(1_234_567));
  assert_eq!(info.md5, Some(CHECKSUM));

  Ok(())
}

#[test]
fn a_stream_of_unknown_length_with_no_checksum_says_neither() -> Result<(), Failure> {
  let info = read(&Stream {
    shortest_block: 16,
    longest_block: 65_535,
    smallest_frame: 0,
    largest_frame: 0,
    packed: LENGTH_UNKNOWN,
    checksum: NO_CHECKSUM,
  })?;

  assert_eq!(info.block_len_min, 16);
  assert_eq!(info.block_len_max, 65_535);
  assert_eq!(info.sample_rate, 96_000);
  assert_eq!(
    info.channels,
    Channels::FRONT_LEFT
      | Channels::FRONT_RIGHT
      | Channels::FRONT_CENTRE
      | Channels::LFE1
      | Channels::REAR_LEFT
      | Channels::REAR_RIGHT
  );
  assert_eq!(info.bits_per_sample, 24);
  assert_eq!(info.n_samples, None);
  assert_eq!(info.md5, None);

  Ok(())
}

#[test]
fn a_frame_length_of_zero_is_unknown_rather_than_out_of_order() -> Result<(), Failure> {
  let info = read(&Stream {
    smallest_frame: 20,
    largest_frame: 0,
    ..ORDINARY
  })?;

  assert_eq!(info.frame_byte_len_min, 20);
  assert_eq!(info.frame_byte_len_max, 0);

  Ok(())
}

#[test]
fn every_count_of_channels_is_the_layout_flac_assigns_it() -> Result<(), Failure> {
  let layouts = [
    0x0000_0001u32,
    0x0000_0003,
    0x0000_0007,
    0x0000_0033,
    0x0000_0037,
    0x0000_003f,
    0x0000_070f,
    0x0000_063f,
  ];

  for (fewer, layout) in (0u64..).zip(layouts) {
    let packed = (44_100u64.wrapping_shl(44)) | fewer.wrapping_shl(41) | 15u64.wrapping_shl(36);
    let info = read(&Stream { packed, ..ORDINARY })?;

    let Ok(bits) = info.channels.bits();

    assert_eq!(bits, layout, "one more than {fewer} channels");
  }

  Ok(())
}

#[test]
fn a_stream_information_block_that_contradicts_itself_is_refused() -> Result<(), Failure> {
  let Ok(short_block) = refused(read(&Stream {
    shortest_block: 15,
    ..ORDINARY
  }));
  let Ok(short_longest) = refused(read(&Stream {
    shortest_block: 16,
    longest_block: 15,
    ..ORDINARY
  }));
  let Ok(blocks_out_of_order) = refused(read(&Stream {
    shortest_block: 4096,
    longest_block: 2048,
    ..ORDINARY
  }));
  let Ok(frames_out_of_order) = refused(read(&Stream {
    smallest_frame: 20,
    largest_frame: 19,
    ..ORDINARY
  }));
  let Ok(no_rate) = refused(read(&Stream {
    packed: NO_SAMPLE_RATE,
    ..ORDINARY
  }));
  let Ok(rate_too_high) = refused(read(&Stream {
    packed: SAMPLE_RATE_TOO_HIGH,
    ..ORDINARY
  }));
  let Ok(narrow) = refused(read(&Stream {
    packed: THREE_BITS_A_SAMPLE,
    ..ORDINARY
  }));

  assert_eq!(
    short_block,
    "malformed stream: flac: minimum block length is 16 samples"
  );
  assert_eq!(
    short_longest,
    "malformed stream: flac: minimum block length is 16 samples"
  );
  assert_eq!(
    blocks_out_of_order,
    "malformed stream: flac: maximum block length is less than the minimum block length"
  );
  assert_eq!(
    frames_out_of_order,
    "malformed stream: flac: maximum frame length is less than the minimum frame length"
  );
  assert_eq!(
    no_rate,
    "malformed stream: flac: stream sample rate out of bounds"
  );
  assert_eq!(
    rate_too_high,
    "malformed stream: flac: stream sample rate out of bounds"
  );
  assert_eq!(
    narrow,
    "malformed stream: flac: stream bits per sample are out of bounds"
  );

  Ok(())
}

#[test]
fn a_stream_information_block_cut_short_is_a_fault() -> Result<(), Failure> {
  let Ok(bytes) = stream_info(&ORDINARY);

  for length in [0usize, 3, 9, 17, 33] {
    let cut: Vec<u8> = bytes.iter().copied().take(length).collect();
    let Ok(mut reader) = BufReader::new(&cut);
    let read = StreamInfo::read(&mut reader);

    let Ok(()) = assert_refused(read, &format!("{length} bytes were read as a whole block"));
  }

  Ok(())
}

#[test]
fn a_block_header_says_its_kind_its_length_and_whether_it_is_the_last() -> Result<(), Failure> {
  let first = header([0x00, 0x00, 0x00, 0x22])?;
  let last = header([0x84, 0x01, 0x02, 0x03])?;

  assert!(first.block_type == MetadataBlockType::StreamInfo);
  assert_eq!(first.block_len, 34);
  assert_eq!(first.boundary, MetadataBlockBoundary::MoreBlocks);
  assert!(last.block_type == MetadataBlockType::VorbisComment);
  assert_eq!(last.block_len, 0x01_0203);
  assert_eq!(last.boundary, MetadataBlockBoundary::LastBlock);

  Ok(())
}

#[test]
fn every_kind_of_block_has_its_number() -> Result<(), Failure> {
  let kinds = [
    (0u8, MetadataBlockType::StreamInfo),
    (1, MetadataBlockType::Padding),
    (2, MetadataBlockType::Application),
    (3, MetadataBlockType::SeekTable),
    (4, MetadataBlockType::VorbisComment),
    (5, MetadataBlockType::Cuesheet),
    (6, MetadataBlockType::Picture),
    (7, MetadataBlockType::Unknown(7)),
    (127, MetadataBlockType::Unknown(127)),
  ];

  for (number, kind) in kinds {
    let read = header([number, 0, 0, 0])?;

    assert!(read.block_type == kind, "block number {number}");
  }

  Ok(())
}

#[test]
fn a_block_header_cut_short_is_a_fault() -> Result<(), Failure> {
  let Ok(mut empty) = BufReader::new(&[]);
  let Ok(mut three) = BufReader::new(&[0, 0, 0]);
  let Ok(()) = assert_refused(MetadataBlockHeader::read(&mut empty), "no bytes");
  let Ok(()) = assert_refused(MetadataBlockHeader::read(&mut three), "three bytes");

  Ok(())
}

#[test]
fn only_a_block_of_the_one_length_is_stream_information() -> Result<(), Failure> {
  assert_eq!(StreamInfo::BLOCK_SIZE, 34);

  Ok(())
}

#[test]
fn a_seek_table_keeps_its_points_and_drops_its_placeholders() -> Result<(), Failure> {
  let mut bytes = Vec::new();

  bytes.extend(0u64.to_be_bytes());
  bytes.extend(0u64.to_be_bytes());
  bytes.extend(4096u16.to_be_bytes());
  bytes.extend(u64::MAX.to_be_bytes());
  bytes.extend([7u8; 10]);
  bytes.extend(4096u64.to_be_bytes());
  bytes.extend(1000u64.to_be_bytes());
  bytes.extend(2048u16.to_be_bytes());
  bytes.extend([9u8; 17]);

  let length = u32::try_from(bytes.len())?;
  let Ok(value_1) = SeekIndex::new();

  let mut index = value_1;
  let Ok(mut reader) = BufReader::new(&bytes);

  read_seek_table_block(&mut reader, length, &mut index)?;

  let first = SeekPoint {
    frame_ts: 0,
    byte_offset: 0,
    n_frames: 4096,
  };
  let second = SeekPoint {
    frame_ts: 4096,
    byte_offset: 1000,
    n_frames: 2048,
  };

  let Ok(value_2) = index.search(100);

  assert_eq!(value_2, SeekSearchResult::Range(first, second));
  let Ok(value_3) = index.search(5000);

  assert_eq!(value_3, SeekSearchResult::Lower(second));
  assert_eq!(reader.pos(), 54);

  Ok(())
}

#[test]
fn a_seek_table_cut_short_is_a_fault() -> Result<(), Failure> {
  let Ok(value_4) = SeekIndex::new();

  let mut index = value_4;
  let Ok(mut zeros) = BufReader::new(&[0u8; 17]);
  let Ok(mut placeholders) = BufReader::new(&[0xffu8; 17]);

  let Ok(()) = assert_refused(
    read_seek_table_block(&mut zeros, 18, &mut index),
    "a point of 17 bytes",
  );
  let Ok(()) = assert_refused(
    read_seek_table_block(&mut placeholders, 18, &mut index),
    "a placeholder of 17 bytes",
  );

  Ok(())
}

fn two_track_sheet() -> Result<Vec<u8>, Never> {
  let Ok(mut bytes) = sheet(&DISC);
  let Ok(first) = track(&Track {
    offset: 0,
    number: 1,
    recording: RECORDING,
    flags: 0xc000,
    reserved: 0,
    points: 2,
  });
  let Ok(pregap) = point(&Point {
    offset: 0,
    packed: 0,
  });
  let Ok(start) = point(&Point {
    offset: SECTOR,
    packed: 0x0100_0000,
  });
  let Ok(last) = track(&Track {
    offset: 1176,
    number: LEAD_OUT,
    recording: NO_RECORDING,
    flags: NO_FLAGS,
    reserved: 0,
    points: 0,
  });

  bytes.extend(first);
  bytes.extend(pregap);
  bytes.extend(start);
  bytes.extend(last);

  Ok(bytes)
}

#[test]
fn a_cuesheet_is_read_into_a_cue_for_each_track() -> Result<(), Failure> {
  let Ok(bytes) = two_track_sheet();
  let cues = cues(&bytes)?;

  let numbers: Vec<u32> = cues.iter().map(|cue| cue.index).collect();
  let starts: Vec<u64> = cues.iter().map(|cue| cue.start_ts).collect();
  let points: Vec<Vec<u64>> = cues
    .iter()
    .map(|cue| {
      cue
        .points
        .iter()
        .map(|point| point.start_offset_ts)
        .collect()
    })
    .collect();
  let tags: Vec<Vec<(Option<StandardTagKey>, String, String)>> = cues
    .iter()
    .map(|cue| {
      cue
        .tags
        .iter()
        .map(|tag| (tag.std_key, tag.key.clone(), tag.value.to_string()))
        .collect()
    })
    .collect();
  let untagged = cues
    .iter()
    .flat_map(|cue| cue.points.iter())
    .all(|point| point.tags.is_empty());

  assert_eq!(numbers, [1, 170]);
  assert_eq!(starts, [0, 1176]);
  assert_eq!(points, [vec![0, SECTOR], vec![]]);
  assert_eq!(
    tags,
    [
      vec![(
        Some(StandardTagKey::IdentIsrc),
        String::from("ISRC"),
        String::from("USRC17607839")
      )],
      vec![(
        Some(StandardTagKey::IdentIsrc),
        String::from("ISRC"),
        String::new()
      )],
    ]
  );
  assert!(untagged);

  Ok(())
}

#[test]
fn a_cuesheet_that_is_not_a_disc_is_held_to_less() -> Result<(), Failure> {
  let Ok(mut bytes) = sheet(&Sheet {
    catalog: b"",
    lead_in: 0,
    kind: NOT_COMPACT_DISC,
    reserved: 0,
    tracks: 1,
  });
  let Ok(only) = track(&Track {
    offset: 100,
    number: 200,
    recording: NO_RECORDING,
    flags: 0,
    reserved: 0,
    points: 1,
  });
  let Ok(start) = point(&Point {
    offset: 7,
    packed: 0,
  });

  bytes.extend(only);
  bytes.extend(start);

  let cues = cues(&bytes)?;
  let read: Vec<(u32, u64, Vec<u64>)> = cues
    .iter()
    .map(|cue| {
      (
        cue.index,
        cue.start_ts,
        cue
          .points
          .iter()
          .map(|point| point.start_offset_ts)
          .collect(),
      )
    })
    .collect();

  assert_eq!(read, [(200, 100, vec![7])]);

  Ok(())
}

#[test]
fn a_cuesheet_whose_head_is_wrong_is_refused() -> Result<(), Failure> {
  let Ok(unprintable) = sheet(&Sheet {
    catalog: b"12\x7f",
    ..DISC
  });
  let Ok(lead_in) = sheet(&Sheet {
    kind: NOT_COMPACT_DISC,
    ..DISC
  });
  let Ok(reserved) = sheet(&Sheet {
    reserved: 1,
    ..DISC
  });
  let Ok(no_tracks) = sheet(&Sheet { tracks: 0, ..DISC });
  let Ok(too_many) = sheet(&Sheet {
    tracks: 101,
    ..DISC
  });

  let Ok(unprintable_said) = refused(cues(&unprintable));
  let Ok(lead_in_said) = refused(cues(&lead_in));
  let Ok(reserved_said) = refused(cues(&reserved));
  let Ok(no_tracks_said) = refused(cues(&no_tracks));
  let Ok(too_many_said) = refused(cues(&too_many));

  assert_eq!(
    unprintable_said,
    "malformed stream: flac: cuesheet catalog number contains invalid characters"
  );
  assert_eq!(
    lead_in_said,
    "malformed stream: flac: cuesheet lead-in samples should be zero if not CD-DA"
  );
  assert_eq!(
    reserved_said,
    "malformed stream: flac: cuesheet reserved bits should be zero"
  );
  assert_eq!(
    no_tracks_said,
    "malformed stream: flac: cuesheet must have at-least one track"
  );
  assert_eq!(
    too_many_said,
    "malformed stream: flac: cuesheets for CD-DA must not have more than 100 tracks"
  );

  Ok(())
}

const VALID_TRACK: Track = Track {
  offset: SECTOR,
  number: 1,
  recording: RECORDING,
  flags: 0,
  reserved: 0,
  points: 1,
};

const VALID_POINT: Point = Point {
  offset: SECTOR,
  packed: 0,
};

#[test]
fn invalid_cuesheet_track_headers_are_refused() -> Result<(), Failure> {
  let Ok(off_sector) = one_track(
    COMPACT_DISC,
    &Track {
      offset: 587,
      ..VALID_TRACK
    },
    &VALID_POINT,
  );
  let Ok(numbered_zero) = one_track(
    NOT_COMPACT_DISC,
    &Track {
      number: 0,
      ..VALID_TRACK
    },
    &VALID_POINT,
  );
  let Ok(numbered_high) = one_track(
    COMPACT_DISC,
    &Track {
      number: 100,
      ..VALID_TRACK
    },
    &VALID_POINT,
  );
  let Ok(unprintable) = one_track(
    COMPACT_DISC,
    &Track {
      recording: b"USRC1760783\n",
      ..VALID_TRACK
    },
    &VALID_POINT,
  );
  let Ok(flags) = one_track(
    COMPACT_DISC,
    &Track {
      flags: 0x8001,
      ..VALID_TRACK
    },
    &VALID_POINT,
  );

  assert_eq!(
    off_sector,
    "malformed stream: flac: cuesheet track sample offset is not a multiple of 588 for CD-DA"
  );
  assert_eq!(
    numbered_zero,
    "malformed stream: flac: cuesheet track number of 0 not allowed"
  );
  assert_eq!(
    numbered_high,
    "malformed stream: flac: cuesheet track numbers greater than 99 are not allowed for CD-DA"
  );
  assert_eq!(
    unprintable,
    "malformed stream: flac: cuesheet track ISRC contains invalid characters"
  );
  assert_eq!(
    flags,
    "malformed stream: flac: cuesheet track reserved bits should be zero"
  );

  Ok(())
}

#[test]
fn invalid_cuesheet_points_and_reserved_bits_are_refused() -> Result<(), Failure> {
  let Ok(reserved) = one_track(
    COMPACT_DISC,
    &Track {
      reserved: 1,
      ..VALID_TRACK
    },
    &VALID_POINT,
  );
  let Ok(too_many) = one_track(
    COMPACT_DISC,
    &Track {
      points: 101,
      ..VALID_TRACK
    },
    &VALID_POINT,
  );
  let Ok(point_off_sector) = one_track(
    COMPACT_DISC,
    &VALID_TRACK,
    &Point {
      offset: 589,
      packed: 0,
    },
  );
  let Ok(point_reserved) = one_track(
    COMPACT_DISC,
    &VALID_TRACK,
    &Point {
      offset: SECTOR,
      packed: 0x0100_0001,
    },
  );
  let Ok(read) = one_track(COMPACT_DISC, &VALID_TRACK, &VALID_POINT);

  assert_eq!(
    reserved,
    "malformed stream: flac: cuesheet track reserved bits should be zero"
  );
  assert_eq!(
    too_many,
    "malformed stream: flac: cuesheet track indicies cannot exceed 100 for CD-DA"
  );
  assert_eq!(
    point_off_sector,
    "malformed stream: flac: cuesheet track index point sample offset is not a multiple of 588 for CD-DA"
  );
  assert_eq!(
    point_reserved,
    "malformed stream: flac: cuesheet track index reserved bits should be 0"
  );
  assert_eq!(read, "read");

  Ok(())
}

#[test]
fn a_cuesheet_cut_short_is_a_fault() -> Result<(), Failure> {
  let Ok(bytes) = sheet(&DISC);

  let Ok(()) = assert_refused(cues(&bytes), "a sheet with no tracks after it");

  Ok(())
}

#[test]
fn an_application_block_is_its_name_and_what_follows_it() -> Result<(), Failure> {
  let Ok(mut riff) = BufReader::new(b"riffdata");
  let Ok(mut odd) = BufReader::new(&[0x00, b'"', 0xff, b'\n']);
  let named = read_application_block(&mut riff, 8)?;
  let unprintable = read_application_block(&mut odd, 4)?;

  assert_eq!(named.ident, "riff");
  assert_eq!(*named.data, *b"data");
  assert_eq!(unprintable.ident, "\\x00\\\"\\xff\\n");
  assert!(unprintable.data.is_empty());
  let Ok(mut short) = BufReader::new(b"riffdat");
  let Ok(mut cut) = BufReader::new(b"rif");
  let Ok(()) = assert_refused(
    read_application_block(&mut short, 8),
    "a block short of its length",
  );
  let Ok(()) = assert_refused(read_application_block(&mut cut, 4), "a name cut short");

  Ok(())
}

#[test]
fn every_count_of_channels_is_the_layout_vorbis_assigns_it() -> Result<(), Failure> {
  let layouts = [
    0x0000_0001u32,
    0x0000_0003,
    0x0000_0007,
    0x0000_0033,
    0x0000_0037,
    0x0000_003f,
    0x0000_070f,
    0x0000_063f,
  ];

  for (count, layout) in (1u8..).zip(layouts) {
    let read = vorbis_channels_to_channels(count)?;

    let Ok(bits) = read.bits();

    assert_eq!(bits, layout, "{count} channels");
  }

  let Ok(()) = assert_refused(vorbis_channels_to_channels(0), "no channels");
  let Ok(()) = assert_refused(vorbis_channels_to_channels(9), "9 channels");
  let Ok(()) = assert_refused(vorbis_channels_to_channels(255), "255 channels");

  Ok(())
}
