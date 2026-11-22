use std::fmt::Write;
use std::io::Cursor;

use console_core_audio_decoding_interfaces::errors::Error;
use console_core_audio_decoding_interfaces::meta::{
  MetadataBuilder, MetadataOptions, MetadataReader, MetadataRevision,
};
use console_core_audio_decoding_tags::id3v2::Id3v2Reader;
use console_core_audio_decoding_tags::{flac, id3v1, itunes, riff, vorbis};
use console_core_byte_streams::{BufReader, MediaSourceStream};

type Failure = Box<dyn std::error::Error>;

type Bytes = Result<Vec<u8>, Failure>;

struct Header {
  major: u8,
  flags: u8,
}

struct Picture<'a> {
  kind: u32,
  media: &'a [u8],
  description: &'a [u8],
  width: u32,
  height: u32,
  depth: u32,
  colours: u32,
  data: &'a [u8],
}

const FRONT: Picture<'static> = Picture {
  kind: 3,
  media: b"image/png",
  description: b"front",
  width: 640,
  height: 480,
  depth: 24,
  colours: 0,
  data: &[0x89, b'P', b'N', b'G'],
};

fn said(revision: &MetadataRevision) -> Result<String, Failure> {
  let mut lines = String::new();

  let Ok(value_1) = revision.tags();

  for tag in value_1 {
    writeln!(lines, "{:?} {:?} {:?}", tag.std_key, tag.key, tag.value)?;
  }

  let Ok(value_2) = revision.visuals();

  for visual in value_2 {
    writeln!(
      lines,
      "visual {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
      visual.media_type,
      visual.dimensions,
      visual.bits_per_pixel,
      visual.color_mode,
      visual.usage,
      visual.tags,
      visual.data,
    )?;
  }

  Ok(lines)
}

fn length(bytes: &[u8]) -> Result<u32, Failure> {
  u32::try_from(bytes.len()).map_err(Failure::from)
}

fn synchsafe(size: u32) -> Result<[u8; 4], Failure> {
  let seven = |shift: u32| u8::try_from(size.wrapping_shr(shift) & 0x7f);
  let first = seven(21)?;
  let second = seven(14)?;
  let third = seven(7)?;
  let fourth = seven(0)?;

  Ok([first, second, third, fourth])
}

fn tag(header: Header, body: &[u8]) -> Bytes {
  let mut bytes = b"ID3".to_vec();
  let length = length(body)?;
  let size = synchsafe(length)?;

  bytes.extend([header.major, 0, header.flags]);
  bytes.extend(size);
  bytes.extend_from_slice(body);
  bytes.extend(b"what follows the tag");

  Ok(bytes)
}

fn tagged(header: Header, body: Bytes) -> Bytes {
  let body_2 = body?;

  tag(header, &body_2)
}

fn legacy_frame(id: &[u8; 3], body: &[u8]) -> Bytes {
  let size = length(body)?;
  let mut bytes = id.to_vec();

  bytes.extend(size.to_be_bytes().into_iter().skip(1));
  bytes.extend_from_slice(body);

  Ok(bytes)
}

fn frame(id: &[u8; 4], flags: u16, body: &[u8]) -> Bytes {
  let size = length(body)?;
  let mut bytes = id.to_vec();

  bytes.extend(size.to_be_bytes());
  bytes.extend(flags.to_be_bytes());
  bytes.extend_from_slice(body);

  Ok(bytes)
}

fn synchsafe_frame(id: &[u8; 4], flags: u16, body: &[u8]) -> Bytes {
  let length = length(body)?;
  let size = synchsafe(length)?;
  let mut bytes = id.to_vec();

  bytes.extend(size);
  bytes.extend(flags.to_be_bytes());
  bytes.extend_from_slice(body);

  Ok(bytes)
}

fn joined(parts: Vec<Bytes>) -> Bytes {
  let collected = parts.into_iter().collect::<Result<Vec<_>, _>>()?;

  Ok(collected.concat())
}

fn read(bytes: Vec<u8>) -> Result<MetadataRevision, Error> {
  let Ok(mut stream) = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
  let Ok(mut reader) = Id3v2Reader::new(&MetadataOptions::default());

  reader.read_all(&mut stream)
}

fn read_and_said(bytes: Bytes) -> Result<String, Failure> {
  let bytes_2 = bytes?;
  let revision = read(bytes_2)?;

  said(&revision)
}

fn refused(bytes: Bytes) -> Result<String, Failure> {
  let bytes_2 = bytes?;

  match read(bytes_2) {
    Ok(revision) => {
      let lines = said(&revision)?;

      Ok(format!("read as {lines:?}"))
    }
    Err(fault) => Ok(fault.to_string()),
  }
}

fn picture(block: &Picture<'_>) -> Bytes {
  let media_length = length(block.media)?;
  let description_length = length(block.description)?;
  let data_length = length(block.data)?;
  let mut bytes = block.kind.to_be_bytes().to_vec();

  bytes.extend(media_length.to_be_bytes());
  bytes.extend_from_slice(block.media);
  bytes.extend(description_length.to_be_bytes());
  bytes.extend_from_slice(block.description);
  bytes.extend(block.width.to_be_bytes());
  bytes.extend(block.height.to_be_bytes());
  bytes.extend(block.depth.to_be_bytes());
  bytes.extend(block.colours.to_be_bytes());
  bytes.extend(data_length.to_be_bytes());
  bytes.extend_from_slice(block.data);

  Ok(bytes)
}

fn comments(listed: &[&[u8]]) -> Bytes {
  let count = u32::try_from(listed.len())?;
  let mut block = 6u32.to_le_bytes().to_vec();

  block.extend(b"vendor");
  block.extend(count.to_le_bytes());

  for comment in listed {
    let size = length(comment)?;

    block.extend(size.to_le_bytes());
    block.extend_from_slice(comment);
  }

  Ok(block)
}

fn comments_said(listed: &[&[u8]]) -> Result<String, Failure> {
  let block = comments(listed)?;
  let Ok(value_3) = MetadataBuilder::new();

  let mut builder = value_3;
  let Ok(mut reader) = BufReader::new(&block);

  vorbis::read_comment_no_framing(&mut reader, &mut builder)?;

  let Ok(metadata) = builder.metadata();

  said(&metadata)
}

fn picture_said(block: &[u8]) -> Result<String, Failure> {
  let Ok(value_4) = MetadataBuilder::new();

  let mut builder = value_4;
  let Ok(mut reader) = BufReader::new(block);

  flac::read_picture_block(&mut reader, &mut builder)?;

  let Ok(metadata) = builder.metadata();

  said(&metadata)
}

fn picture_fault(block: &[u8]) -> Result<String, Failure> {
  let Ok(value_5) = MetadataBuilder::new();

  let mut builder = value_5;
  let Ok(mut reader) = BufReader::new(block);

  match flac::read_picture_block(&mut reader, &mut builder) {
    Ok(()) => Err(Failure::from("the block was read")),
    Err(fault) => Ok(fault.to_string()),
  }
}

const V3_FRAMES: &str = r##"Some(TrackTitle) "TIT2" String("A tïtle")
Some(Artist) "TPE1" String("Ab")
Some(Artist) "TPE1" String("Cd")
Some(Album) "TALB" String("Alb😀")
Some(ReplayGainTrackGain) "TXXX:REPLAYGAIN_TRACK_GAIN" String("-6.5 dB")
Some(ReplayGainTrackGain) "TXXX:REPLAYGAIN_TRACK_GAIN" String("-7 dB")
None "TXXX:mine" String("café�")
Some(Comment) "COMM!eng" String("short")
Some(Lyrics) "USLT" String("hi")
None "PCNT" UnsignedInt(258)
None "PCNT" UnsignedInt(1099511627778)
Some(Rating) "POPM:someone@example.org" UnsignedInt(196)
None "MCDI" Binary([1, 2, 3])
None "PRIV:owner" Binary([0, 1, 2])
Some(UrlArtist) "WOAR" String("http://example.org/artist")
Some(Url) "WXXX:a page" String("http://example.org/page")
Some(TrackNumber) "TRCK" String("3/12")
visual "image/jpeg" None None None Some(FrontCover) [Tag { std_key: Some(Description), key: "", value: String("cover") }] [255, 216, 255, 224]
visual "image/png" None None None None [Tag { std_key: Some(Description), key: "", value: String("") }] [137, 80, 78, 71]
"##;

const V4_FRAMES: &str = r##"Some(Artist) "TPE1" String("one")
Some(Artist) "TPE1" String("two")
Some(Artist) "TPE1" String("three")
Some(TrackTitle) "TIT2" String("ÿ")
Some(TrackTitle) "TIT2" String("�")
Some(Date) "TDRC" String("2024")
Some(Mood) "TMOO" String("calm")
None "TSST" String("a�b�")
Some(Comment) "COMM" String("a comment")
Some(UrlPodcast) "WFED" String("http://example.org/feed")
"##;

const V2_FRAMES: &str = r##"Some(TrackTitle) "TT2" String("A title")
Some(Artist) "TP1" String("Ab�")
"##;

const V3_CHECKED: &str = r##"Some(TrackTitle) "TIT2" String("checked")
"##;

const V3_PLAIN: &str = r##"Some(TrackTitle) "TIT2" String("plain")
"##;

const V3_UNSYNCHRONISED: &str = r##"Some(TrackTitle) "TIT2" String("aÿbT")
"##;

const HEADER_AND_TEXT_REFUSALS: &str = r##"unsupported feature: id3v2: not an ID3v2 tag
unsupported feature: id3v2: unsupported ID3v2 version
unsupported feature: id3v2: unsupported ID3v2 version
malformed stream: id3v2: invalid version number(s)
unsupported feature: id3v2: ID3v2.2 compression is not supported
malformed stream: id3v2: unused flag bits are not cleared
unsupported feature: id3v2: compressed frames are not supported
read as "Some(TrackTitle) \"TIT2\" String(\"a\")\n"
malformed stream: id3v2: invalid text encoding
malformed stream: id3v2: invalid TXXX text encoding
malformed stream: id3v2: invalid WXXX URL description encoding
malformed stream: id3v2: invalid text encoding
buffer underrun
malformed stream: id3v2: invalid text encoding"##;

const COUNTER_AND_FRAME_REFUSALS: &str = r##"malformed stream: id3v2: play counters must be a minimum of 32bits
unsupported feature: id3v2: play counters greater than 64bits are not supported
buffer underrun
malformed stream: id3v2: invalid extended header size
malformed stream: id3v2: unused flag bits are not cleared
unsupported feature: id3v2: compressed frames are not supported
unsupported feature: id3v2: encrypted frames are not supported
malformed stream: id3v2: extended flags should have a length of 1
malformed stream: id3v2: is update extended flag has invalid size
malformed stream: id3v2: CRC32 extended flag has invalid size
malformed stream: id3v2: restrictions extended flag has invalid size
end of stream"##;

const VORBIS_COMMENTS: &str = r##"Some(TrackTitle) "TITLE" String("A title")
Some(AlbumArtist) "Album Artist" String("Somebody")
Some(ReplayGainTrackGain) "REPLAYGAIN_TRACK_GAIN" String("-6.5 dB")
Some(Version) "version" String("2")
None "mine" String("a=b")
None "empty" String("")
None "café" String("��")
Some(Date) "DATE" String("1999")
visual "image/png" Some(Size { width: 640, height: 480 }) Some(24) Some(Discrete) Some(FrontCover) [Tag { std_key: Some(Description), key: "DESCRIPTION", value: String("front") }] [137, 80, 78, 71]
visual "image/png" Some(Size { width: 640, height: 480 }) Some(24) Some(Discrete) Some(FrontCover) [Tag { std_key: Some(Description), key: "DESCRIPTION", value: String("front") }] [137, 80, 78, 71]
"##;

const VORBIS_CUT: &str = r##"Err("buffer underrun")"##;

const FLAC_FRONT: &str = r##"visual "image/png" Some(Size { width: 640, height: 480 }) Some(24) Some(Discrete) Some(FrontCover) [Tag { std_key: Some(Description), key: "DESCRIPTION", value: String("front") }] [137, 80, 78, 71]
"##;

const FLAC_WITHOUT_A_USAGE: &str = r##"visual "image/png" None Some(16) Some(Discrete) None [Tag { std_key: Some(Description), key: "DESCRIPTION", value: String("front") }] [137, 80, 78, 71]
"##;

const FLAC_INDEXED: &str = r##"visual "image/png" Some(Size { width: 640, height: 480 }) Some(24) Some(Indexed(16)) Some(FrontCover) [Tag { std_key: Some(Description), key: "DESCRIPTION", value: String("front") }] [137, 80, 78, 71]
"##;

const FLAC_WITHOUT_A_WIDTH: &str = r##"visual "image/png" None Some(24) Some(Discrete) Some(FrontCover) [Tag { std_key: Some(Description), key: "DESCRIPTION", value: String("front") }] [137, 80, 78, 71]
"##;

const FLAC_ENDED_EARLY: &str = r##"visual "im" Some(Size { width: 640, height: 480 }) Some(24) Some(Discrete) Some(FrontCover) [Tag { std_key: Some(Description), key: "DESCRIPTION", value: String("front") }] [137, 80, 78, 71]
"##;

const FLAC_UNPRINTABLE: &str =
  r##"malformed stream: meta (flac): picture mime-type contains invalid characters"##;

const FLAC_CUT: &str = r##"buffer underrun"##;

const RIFF: &str = r##"[Tag { std_key: Some(TrackTitle), key: "INAM", value: String("A title") }, Tag { std_key: Some(Artist), key: "iart", value: String("Somebody�") }, Tag { std_key: Some(Date), key: "ICRD", value: String("1999") }, Tag { std_key: None, key: "IXYZ", value: String("unknown") }, Tag { std_key: Some(TrackNumber), key: "TRCK", value: String("") }, Tag { std_key: None, key: "I�RD", value: String("1999") }]"##;

const ITUNES: &str = r##"[Some(Artist), Some(ReleaseCountry), Some(OriginalDate), None, Some(TrackSubtitle), None, None]"##;

const GENRES: &str = r##"[Some("Blues"), Some("Other"), Some("Hard Rock"), Some("(133)"), Some("Synthpop"), Some("Psybient"), None, None]"##;

#[test]
fn an_id3v2_3_tag_is_read_a_frame_at_a_time() -> Result<(), Failure> {
  let body = joined(vec![
    frame(b"TIT2", 0, b"\x00A t\xeftle\x00"),
    frame(
      b"TPE1",
      0,
      b"\x01\xff\xfeA\x00b\x00\x00\x00\xff\xfeC\x00d\x00",
    ),
    frame(b"TALB", 0x0020, b"\x07\x02\x00A\x00l\x00b\xd8\x3d\xde\x00"),
    frame(b"TXXX", 0, b"\x00REPLAYGAIN_TRACK_GAIN\x00-6.5 dB\x00-7 dB"),
    frame(b"TXXX", 0, b"\x03mine\x00caf\xc3\xa9\xff"),
    frame(b"COMM", 0, b"\x00eng\x00short\x00the long of it"),
    frame(b"USLT", 0, b"\x01XXX\xff\xfe\x00\x00\xff\xfeh\x00i\x00"),
    frame(
      b"APIC",
      0,
      b"\x00image/jpeg\x00\x03cover\x00\xff\xd8\xff\xe0",
    ),
    frame(b"APIC", 0, b"\x03image/png\x00\x11\x00\x89PNG"),
    frame(b"PCNT", 0, b"\x00\x00\x01\x02"),
    frame(b"PCNT", 0, b"\x01\x00\x00\x00\x00\x02"),
    frame(b"POPM", 0, b"someone@example.org\x00\xc4\x00\x00\x00\x09"),
    frame(b"MCDI", 0, b"\x01\x02\x03"),
    frame(b"PRIV", 0, b"owner\x00\x00\x01\x02"),
    frame(b"WOAR", 0, b"http://example.org/artist"),
    frame(b"WXXX", 0, b"\x00a page\x00http://example.org/page"),
    frame(b"GEOB", 0, b"nothing reads this"),
    frame(b"TCON", 0, b""),
    frame(b"TRCK", 0, b"\x003/12"),
    Ok(vec![0u8; 32]),
  ])?;
  let said = read_and_said(tag(Header { major: 3, flags: 0 }, &body))?;

  assert_eq!(said, V3_FRAMES);

  Ok(())
}

#[test]
fn an_id3v2_4_tag_is_read_with_its_own_sizes_and_flags() -> Result<(), Failure> {
  let body = joined(vec![
    Ok(b"\x00\x00\x00\x0c\x01\x70\x01\x05\x00\x01\x02\x03\x04\x01\xbf".to_vec()),
    synchsafe_frame(b"TPE1", 0, b"\x03one\x00two\x00three"),
    synchsafe_frame(b"TIT2", 0x0002, b"\x02\x00\xff\x00\x00\x00A"),
    synchsafe_frame(b"TDRC", 0x0001, b"\x00\x00\x00\x05\x002024"),
    synchsafe_frame(b"TMOO", 0x0040, b"\x09\x00calm"),
    synchsafe_frame(
      b"TSST",
      0x0043,
      b"\x09\x00\x00\x00\x0a\x03a\xff\x00b\xff\x00",
    ),
    synchsafe_frame(b"COMM", 0, b"\x03en1\x00a comment"),
    synchsafe_frame(b"WFED", 0, b"\x00http://example.org/feed"),
    synchsafe_frame(b"XXXX", 0, b"unknown"),
    Ok(b"\x01\x00\x00\x00junk that is not a frame".to_vec()),
  ])?;
  let said = read_and_said(tag(
    Header {
      major: 4,
      flags: 0x40,
    },
    &body,
  ))?;

  assert_eq!(said, V4_FRAMES);

  Ok(())
}

#[test]
fn an_id3v2_2_tag_is_read_through_its_unsynchronisation() -> Result<(), Failure> {
  let body = joined(vec![
    legacy_frame(b"TT2", b"\x00A title"),
    legacy_frame(b"TP1", b"\x01\xff\x00\xfeA\x00b\x00"),
    legacy_frame(b"COM", b"\x00deu\x00Ein Kommentar\xff\x00"),
    legacy_frame(b"PIC", b"\x00PNG\x00\x04back\x00\x01\xff\x00\x00\x02"),
    legacy_frame(b"CNT", b"\x00\x00\x00\x07"),
    legacy_frame(b"XYZ", b"skipped"),
    legacy_frame(b"TAL", b""),
    legacy_frame(b"TYE", b"\x001999"),
    Ok(vec![0u8; 9]),
  ])?;
  let said = read_and_said(tag(
    Header {
      major: 2,
      flags: 0x80,
    },
    &body,
  ))?;

  assert_eq!(said, V2_FRAMES);

  Ok(())
}

#[test]
fn an_id3v2_3_tag_is_read_past_its_extended_header() -> Result<(), Failure> {
  let checked = joined(vec![
    Ok(b"\x00\x00\x00\x0a\x80\x00\x00\x00\x00\x00\x12\x34\x56\x78".to_vec()),
    frame(b"TIT2", 0, b"\x00checked"),
  ])?;
  let plain = joined(vec![
    Ok(b"\x00\x00\x00\x06\x00\x00\x00\x00\x00\x00".to_vec()),
    frame(b"TIT2", 0, b"\x00plain"),
    Ok(b"\x00\x00\x00".to_vec()),
  ])?;
  let checked_said = read_and_said(tag(
    Header {
      major: 3,
      flags: 0x40,
    },
    &checked,
  ))?;
  let plain_said = read_and_said(tag(
    Header {
      major: 3,
      flags: 0x60,
    },
    &plain,
  ))?;

  assert_eq!(checked_said, V3_CHECKED);
  assert_eq!(plain_said, V3_PLAIN);

  Ok(())
}

#[test]
fn an_id3v2_3_tag_is_unsynchronised_as_a_whole() -> Result<(), Failure> {
  let body = joined(vec![
    frame(b"TIT2", 0, b"\x00a\xff\x00b"),
    frame(b"TPE1", 0, b"\x00c"),
    Ok(vec![0u8; 12]),
  ])?;
  let said = read_and_said(tag(
    Header {
      major: 3,
      flags: 0x80,
    },
    &body,
  ))?;

  assert_eq!(said, V3_UNSYNCHRONISED);

  Ok(())
}

fn assert_refusals(
  cases: impl IntoIterator<Item = Bytes>,
  expected: &str,
) -> Result<(), Failure> {
  let answers = cases
    .into_iter()
    .map(refused)
    .collect::<Result<Vec<_>, _>>()?;

  assert_eq!(answers.join("\n"), expected);

  Ok(())
}

#[test]
fn invalid_headers_and_text_frames_say_why() -> Result<(), Failure> {
  let cases = [
    Ok(b"TAG\x03\x00\x00\x00\x00\x00\x00".to_vec()),
    tagged(Header { major: 5, flags: 0 }, Ok(Vec::new())),
    tagged(Header { major: 1, flags: 0 }, Ok(Vec::new())),
    tagged(
      Header {
        major: 0xff,
        flags: 0,
      },
      Ok(Vec::new()),
    ),
    tagged(
      Header {
        major: 2,
        flags: 0x40,
      },
      Ok(Vec::new()),
    ),
    tagged(
      Header { major: 3, flags: 0 },
      frame(b"TIT2", 0x0100, b"\x00a"),
    ),
    tagged(
      Header { major: 3, flags: 0 },
      frame(b"TIT2", 0x0080, b"\x00a"),
    ),
    tagged(
      Header { major: 3, flags: 0 },
      frame(b"TIT2", 0x0040, b"\x00a"),
    ),
    tagged(Header { major: 3, flags: 0 }, frame(b"TIT2", 0, b"\x04a")),
    tagged(Header { major: 3, flags: 0 }, frame(b"TXXX", 0, b"\x09a")),
    tagged(Header { major: 3, flags: 0 }, frame(b"WXXX", 0, b"\x09a")),
    tagged(Header { major: 3, flags: 0 }, frame(b"COMM", 0, b"\x09eng")),
    tagged(Header { major: 3, flags: 0 }, frame(b"COMM", 0, b"\x00en")),
    tagged(Header { major: 3, flags: 0 }, frame(b"APIC", 0, b"\x09")),
  ];

  assert_refusals(cases, HEADER_AND_TEXT_REFUSALS)
}

#[test]
fn invalid_counters_extended_headers_and_frames_say_why() -> Result<(), Failure> {
  let cases = [
    tagged(
      Header { major: 3, flags: 0 },
      frame(b"PCNT", 0, b"\x01\x02\x03"),
    ),
    tagged(
      Header { major: 3, flags: 0 },
      frame(b"PCNT", 0, b"123456789"),
    ),
    tagged(Header { major: 3, flags: 0 }, frame(b"POPM", 0, b"nobody")),
    tagged(
      Header {
        major: 3,
        flags: 0x40,
      },
      Ok(b"\x00\x00\x00\x07\x00\x00\x00\x00\x00\x00".to_vec()),
    ),
    tagged(
      Header { major: 4, flags: 0 },
      synchsafe_frame(b"TIT2", 0x0010, b"\x00a"),
    ),
    tagged(
      Header { major: 4, flags: 0 },
      synchsafe_frame(b"TIT2", 0x0008, b"\x00a"),
    ),
    tagged(
      Header { major: 4, flags: 0 },
      synchsafe_frame(b"TIT2", 0x0004, b"\x00a"),
    ),
    tagged(
      Header {
        major: 4,
        flags: 0x40,
      },
      Ok(b"\x00\x00\x00\x06\x02\x00".to_vec()),
    ),
    tagged(
      Header {
        major: 4,
        flags: 0x40,
      },
      Ok(b"\x00\x00\x00\x06\x01\x40\x02".to_vec()),
    ),
    tagged(
      Header {
        major: 4,
        flags: 0x40,
      },
      Ok(b"\x00\x00\x00\x06\x01\x20\x04".to_vec()),
    ),
    tagged(
      Header {
        major: 4,
        flags: 0x40,
      },
      Ok(b"\x00\x00\x00\x06\x01\x10\x02".to_vec()),
    ),
    Ok(b"ID3\x03\x00\x00\x00\x00\x00\x20TIT2\x00\x00\x00\x40\x00\x00\x00short".to_vec()),
  ];

  assert_refusals(cases, COUNTER_AND_FRAME_REFUSALS)
}

#[test]
fn vorbis_comments_are_read_into_tags_and_pictures() -> Result<(), Failure> {
  let said = comments_said(&[
        b"TITLE=A title",
        b"Album Artist=Somebody",
        b"REPLAYGAIN_TRACK_GAIN=-6.5 dB",
        b"version=2",
        b"mine=a=b",
        b"empty=",
        b"no separator",
        b"caf\xc3\xa9=\xff\xfe",
        b"METADATA_BLOCK_PICTURE=AAAAAwAAAAlpbWFnZS9wbmcAAAAFZnJvbnQAAAKAAAAB4AAAABgAAAAAAAAABIlQTkc=",
        b"metadata_block_picture=AAAAAwAAAAlpbWFnZS9wbmcAAAAFZnJvbnQAAAKAAAAB4AAAABgAAAAAAAAABIlQTkc",
        b"METADATA_BLOCK_PICTURE=not base64!",
        b"METADATA_BLOCK_PICTURE=AAAAAwAAAAlpbWFnZS9wbmcAAAAF",
        b"METADATA_BLOCK_PICTURE=AAAAA",
        b"DATE=1999",
    ])?;

  assert_eq!(said, VORBIS_COMMENTS);

  Ok(())
}

#[test]
fn vorbis_comments_cut_short_are_a_fault() -> Result<(), Failure> {
  let block = comments(&[b"TITLE=A title"])?;
  let cut = block
    .get(..block.len().saturating_sub(3))
    .ok_or("the block is shorter than its end")?;
  let Ok(value_6) = MetadataBuilder::new();

  let mut builder = value_6;
  let Ok(mut reader) = BufReader::new(cut);
  let answered = vorbis::read_comment_no_framing(&mut reader, &mut builder);

  assert_eq!(
    format!("{:?}", answered.map_err(|fault| fault.to_string())),
    VORBIS_CUT
  );

  Ok(())
}

#[test]
fn a_flac_picture_is_read_with_what_it_says_of_itself() -> Result<(), Failure> {
  let front = picture(&FRONT)?;
  let without_a_usage = picture(&Picture {
    kind: 0x11,
    height: 0,
    depth: 16,
    ..FRONT
  })?;
  let indexed = picture(&Picture {
    colours: 16,
    ..FRONT
  })?;
  let without_a_width = picture(&Picture { width: 0, ..FRONT })?;
  let ended_early = picture(&Picture {
    media: b"im\0ge/png",
    ..FRONT
  })?;
  let unprintable = picture(&Picture {
    media: b"\x7fmage/png",
    ..FRONT
  })?;
  let cut = front
    .get(..40)
    .ok_or("the block is shorter than its picture")?;

  let front_said = picture_said(&front)?;
  assert_eq!(front_said, FLAC_FRONT);
  let without_a_usage_said = picture_said(&without_a_usage)?;
  assert_eq!(without_a_usage_said, FLAC_WITHOUT_A_USAGE);
  let indexed_said = picture_said(&indexed)?;
  assert_eq!(indexed_said, FLAC_INDEXED);
  let without_a_width_said = picture_said(&without_a_width)?;
  assert_eq!(without_a_width_said, FLAC_WITHOUT_A_WIDTH);
  let ended_early_said = picture_said(&ended_early)?;
  assert_eq!(ended_early_said, FLAC_ENDED_EARLY);
  let unprintable_fault = picture_fault(&unprintable)?;
  assert_eq!(unprintable_fault, FLAC_UNPRINTABLE);
  let cut_fault = picture_fault(cut)?;
  assert_eq!(cut_fault, FLAC_CUT);

  Ok(())
}

#[test]
fn a_riff_info_chunk_is_a_tag_under_the_key_it_was_written_with() {
  let asked: [([u8; 4], &[u8]); 6] = [
    (*b"INAM", b"A title"),
    (*b"iart", b"Somebody\xff"),
    (*b"ICRD", b"1999"),
    (*b"IXYZ", b"unknown"),
    (*b"TRCK", b""),
    (*b"I\xffRD", b"1999"),
  ];
  let read = asked.map(|(key, value)| {
    let Ok(tag) = riff::parse(key, value);

    tag
  });

  assert_eq!(format!("{read:?}"), RIFF);
}

#[test]
fn an_itunes_name_is_a_standard_key_when_it_is_one_of_the_known_ones() {
  let asked = [
    "com.apple.iTunes:ARTISTS",
    "com.apple.iTunes:MusicBrainz Album Release Country",
    "com.apple.iTunes:originaldate",
    "com.apple.iTunes:ORIGINALDATE",
    "com.apple.iTunes:SUBTITLE",
    "ARTISTS",
    "",
  ];
  let found = asked.map(|name| {
    let Ok(key) = itunes::std_key_from_tag(name);

    key
  });

  assert_eq!(format!("{found:?}"), ITUNES);
}

#[test]
fn a_genre_number_is_a_name_while_the_table_has_one() {
  let asked = [0u8, 12, 79, 133, 147, 191, 192, 255];
  let named = asked.map(|number| {
    let Ok(name) = id3v1::util::genre_name(number);

    name
  });

  assert_eq!(format!("{named:?}"), GENRES);
}
