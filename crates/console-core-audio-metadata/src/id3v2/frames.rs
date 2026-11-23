//! The frames of an ID3v2 tag, read one after another out of its bytes.
//!
//! A frame is a header -- an id, a size and flags -- and a body read the way
//! its id says. How far the next frame is from this one is lofty's answer
//! rather than the size the header gave: a body read as it stands ends where
//! its reader stopped, so a URL that ends at its terminator leaves what
//! follows to be read as the next frame, while a body that had to be
//! unsynchronised or inflated is taken whole. A frame with an id that cannot
//! be one is stepped over by its size. Which faults end the whole tag and
//! which only lose the frame is the other half of what is kept: a text in an
//! encoding that does not exist ends the read, as it did.
//!
//! The frames nobody reads an item out of -- an identifier, a rating, a
//! chapter, the involved people -- are kept as the bytes they were written
//! with, so they still take their place in the tag, replace each other and
//! count as one per tag where the standard says so, without their contents
//! being taken apart.

use console_core_never::Never;

use crate::bytes::{Cursor, length};
use crate::fault::{Malformed, ParseError};
use crate::id3v2::header::{Unsynchronised, Version, resynchronised, synchsafe};
use crate::id3v2::timestamp::Timestamp;
use crate::id3v2::{Identity, Validity};
use crate::picture::{MimeType, Picture, PictureType};
use crate::text::{Decoded, Encoding, Ending, decoded, order_of, terminated_maybe_marked};

const INFLATED_LIMIT: u32 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Flags {
  pub(crate) bits: u16,
  pub(crate) group: Option<u8>,
  pub(crate) method: Option<u8>,
  pub(crate) data_length: Option<u32>,
}

const GROUPED: u16 = 0x0040;

const COMPRESSED: u16 = 0x0008;

const ENCRYPTED: u16 = 0x0004;

const UNSYNCHRONISED: u16 = 0x0002;

const LENGTHED: u16 = 0x0001;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Frame {
  pub(crate) id: String,
  pub(crate) flags: Flags,
  pub(crate) body: Body,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Body {
  Text {
    encoding: Encoding,
    value: String,
  },
  UserText {
    encoding: Encoding,
    description: String,
    content: String,
  },
  Url(String),
  UserUrl {
    encoding: Encoding,
    description: String,
    content: String,
  },
  Comment {
    encoding: Encoding,
    language: [u8; 3],
    description: String,
    content: String,
  },
  Lyrics {
    encoding: Encoding,
    language: [u8; 3],
    description: String,
    content: String,
  },
  Picture {
    encoding: Encoding,
    description: String,
    picture: Picture,
  },
  Timestamp {
    encoding: Encoding,
    timestamp: Timestamp,
  },
  Opaque(Vec<u8>),
  Binary(Vec<u8>),
}

#[derive(PartialEq)]
enum Key<'a> {
  Text(&'a str),
  Url(&'a str),
  UserText(&'a str),
  UserUrl(&'a str),
  Comment(&'a [u8; 3], &'a str),
  Lyrics(&'a [u8; 3], &'a str),
  Whole(&'a Frame),
}

pub(crate) enum Parsed {
  Next(Frame),
  Skip,
  End,
}

enum Wrapping {
  Plain,
  Wrapped,
}

pub(crate) enum FrameContent {
  Empty,
  Present,
  Unmeasured,
}

impl From<bool> for FrameContent {
  fn from(empty: bool) -> Self {
    match bool::then_some(empty, ()) {
      Some(()) => Self::Empty,
      None => Self::Present,
    }
  }
}

impl Frame {
  pub(crate) fn same(&self, other: &Frame) -> Result<Identity, Never> {
    let Ok(mine) = self.key();
    let Ok(theirs) = other.key();

    Ok(Identity::from(mine == theirs))
  }

  fn key(&self) -> Result<Key<'_>, Never> {
    Ok(match &self.body {
      Body::Text { .. } => Key::Text(&self.id),
      Body::Url(_) => Key::Url(&self.id),
      Body::UserText { description, .. } => Key::UserText(description),
      Body::UserUrl { description, .. } => Key::UserUrl(description),
      Body::Comment {
        language,
        description,
        ..
      } => Key::Comment(language, description),
      Body::Lyrics {
        language,
        description,
        ..
      } => Key::Lyrics(language, description),
      Body::Picture { .. } | Body::Timestamp { .. } | Body::Opaque(_) | Body::Binary(_) => {
        Key::Whole(self)
      }
    })
  }

  pub(crate) fn content_state(&self) -> Result<FrameContent, Never> {
    Ok(match &self.body {
      Body::Text { value, .. } => FrameContent::from(value.is_empty()),
      Body::UserText { content, .. }
      | Body::UserUrl { content, .. }
      | Body::Comment { content, .. }
      | Body::Lyrics { content, .. }
      | Body::Url(content) => FrameContent::from(content.is_empty()),
      Body::Picture { picture, .. } => FrameContent::from(picture.data.is_empty()),
      Body::Binary(data) => FrameContent::from(data.is_empty()),
      Body::Timestamp { .. } | Body::Opaque(_) => FrameContent::Unmeasured,
    })
  }
}

pub(crate) fn upgraded_v2(id: &str) -> Result<Option<&'static str>, Never> {
  Ok(match id {
    "BUF" => Some("RBUF"),
    "CNT" => Some("PCNT"),
    "COM" => Some("COMM"),
    "CRA" => Some("AENC"),
    "ETC" => Some("ETCO"),
    "GEO" => Some("GEOB"),
    "IPL" => Some("TIPL"),
    "MCI" => Some("MCDI"),
    "MLL" => Some("MLLT"),
    "PIC" => Some("APIC"),
    "POP" => Some("POPM"),
    "REV" => Some("RVRB"),
    "SLT" => Some("SYLT"),
    "STC" => Some("SYTC"),
    "TAL" => Some("TALB"),
    "TBP" => Some("TBPM"),
    "TCM" => Some("TCOM"),
    "TCO" => Some("TCON"),
    "TCP" => Some("TCMP"),
    "TCR" => Some("TCOP"),
    "TDY" => Some("TDLY"),
    "TEN" => Some("TENC"),
    "TFT" => Some("TFLT"),
    "TKE" => Some("TKEY"),
    "TLA" => Some("TLAN"),
    "TLE" => Some("TLEN"),
    "TMT" => Some("TMED"),
    "TOA" => Some("TOPE"),
    "TOF" => Some("TOFN"),
    "TOL" => Some("TOLY"),
    "TOR" => Some("TDOR"),
    "TOT" => Some("TOAL"),
    "TP1" => Some("TPE1"),
    "TP2" => Some("TPE2"),
    "TP3" => Some("TPE3"),
    "TP4" => Some("TPE4"),
    "TPA" => Some("TPOS"),
    "TPB" => Some("TPUB"),
    "TRC" => Some("TSRC"),
    "TRD" => Some("TDRC"),
    "TRK" => Some("TRCK"),
    "TS2" => Some("TSO2"),
    "TSA" => Some("TSOA"),
    "TSC" => Some("TSOC"),
    "TSP" => Some("TSOP"),
    "TSS" => Some("TSSE"),
    "TST" => Some("TSOT"),
    "TT1" => Some("TIT1"),
    "TT2" => Some("TIT2"),
    "TT3" => Some("TIT3"),
    "TXT" => Some("TEXT"),
    "TXX" => Some("TXXX"),
    "TYE" => Some("TDRC"),
    "UFI" => Some("UFID"),
    "ULT" => Some("USLT"),
    "WAF" => Some("WOAF"),
    "WAR" => Some("WOAR"),
    "WAS" => Some("WOAS"),
    "WCM" => Some("WCOM"),
    "WCP" => Some("WCOP"),
    "WPB" => Some("WPUB"),
    "WXX" => Some("WXXX"),
    "PCS" => Some("PCST"),
    "TCT" => Some("TCAT"),
    "TDS" => Some("TDES"),
    "TID" => Some("TGID"),
    "WFD" => Some("WFED"),
    "MVI" => Some("MVIN"),
    "MVN" => Some("MVNM"),
    "GP1" => Some("GRP1"),
    "TDR" => Some("TDRL"),
    _kept => None,
  })
}

fn upgraded_v3(id: &str) -> Result<Option<&'static str>, Never> {
  Ok(match id {
    "TORY" => Some("TDOR"),
    "TYER" => Some("TDRC"),
    "IPLS" => Some("TIPL"),
    _kept => None,
  })
}

fn valid(id: &str) -> Result<Validity, Never> {
  let characters = id
    .bytes()
    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit());

  Ok(Validity::from(characters && matches!(id.len(), 3 | 4)))
}

fn named(id: &str, upgrade: Option<&'static str>) -> Result<String, Never> {
  Ok(match upgrade {
    Some(upgraded) => upgraded.to_string(),
    None => id.to_string(),
  })
}

struct Opened {
  id: Option<String>,
  size: u32,
  flags: Flags,
}

fn opened(reader: &mut Cursor<'_>, version: Version) -> Result<Option<Opened>, Never> {
  match version {
    Version::Two => {
      let header = match reader.array::<6>() {
        Ok([0, ..]) => return Ok(None),
        Err(_short_header) => return Ok(None),
        Ok(header) => header,
      };
      let [a, b, c, s0, s1, s2] = header;
      let size = u32::from_be_bytes([0, s0, s1, s2]);

      let id = match std::str::from_utf8(&[a, b, c]) {
        Ok(id) => {
          let Ok(upgrade) = upgraded_v2(id);
          let Ok(id_2) = named(id, upgrade);

          Some(id_2)
        }
        Err(_not_utf8) => None,
      };

      Ok(Some(Opened {
        id,
        size,
        flags: Flags::default(),
      }))
    }
    Version::Three | Version::Four => {
      let header = match reader.array::<10>() {
        Ok([0, ..]) => return Ok(None),
        Err(_short_header) => return Ok(None),
        Ok(header) => header,
      };

      opened_v3_or_v4(header, version)
    }
  }
}

fn opened_v3_or_v4(header: [u8; 10], version: Version) -> Result<Option<Opened>, Never> {
  let [a, b, c, d, s0, s1, s2, s3, f0, f1] = header;

  let size = match version {
    Version::Four => {
      let Ok(size) = synchsafe([s0, s1, s2, s3]);

      size
    }
    Version::Two | Version::Three => u32::from_be_bytes([s0, s1, s2, s3]),
  };

  let id = match (version, d) {
    (Version::Three, 0) => match std::str::from_utf8(&[a, b, c]) {
      Ok(id) => {
        let Ok(upgrade) = upgraded_v2(id);
        let Ok(id_2) = named(id, upgrade);

        Some(id_2)
      }
      Err(_not_utf8) => None,
    },
    (Version::Three, _named) => match std::str::from_utf8(&[a, b, c, d]) {
      Ok(id) => {
        let trimmed = id.trim_end_matches('\0');
        let Ok(upgrade) = upgraded_v3(trimmed);
        let Ok(id_3) = named(trimmed, upgrade);

        Some(id_3)
      }
      Err(_not_utf8) => None,
    },
    (Version::Two | Version::Four, _) => match std::str::from_utf8(&[a, b, c, d]) {
      Ok(id) => Some(id.trim_end_matches('\0').to_string()),
      Err(_not_utf8) => None,
    },
  };

  let raw = u16::from_be_bytes([f0, f1]);
  let Ok(flags) = flags_of(raw, version);

  Ok(Some(Opened { id, size, flags }))
}

fn flags_of(raw: u16, version: Version) -> Result<Flags, Never> {
  let bits = match version {
    Version::Four => raw & 0x704F,
    Version::Two | Version::Three => {
      let moved = [
        (0x8000, 0x4000),
        (0x4000, 0x2000),
        (0x2000, 0x1000),
        (0x0020, GROUPED),
        (0x0080, COMPRESSED),
        (0x0040, ENCRYPTED),
      ];

      moved
        .iter()
        .fold(0u16, |bits, (from, to)| match raw & from {
          0 => bits,
          _set => bits | to,
        })
    }
  };

  let present = |bit: u16| match bits & bit {
    0 => None,
    _set => Some(0),
  };

  Ok(Flags {
    bits,
    group: present(GROUPED),
    method: present(ENCRYPTED),
    data_length: present(LENGTHED).map(u32::from),
  })
}

pub(crate) fn next(
  reader: &mut Cursor<'_>,
  version: Version,
  unsynchronised: Unsynchronised,
) -> Result<Parsed, ParseError> {
  let Ok(opened) = opened(reader, version);

  let Opened {
    id,
    size,
    mut flags,
  } = match opened {
    Some(opened_2) => opened_2,
    None => return Ok(Parsed::End),
  };

  let id_3 = match id {
    Some(id_2) => id_2,
    None => {
      let Ok(_skipped) = reader.upto(u64::from(size));

      return Ok(Parsed::Skip);
    }
  };

  let Ok(validity) = valid(&id_3);

  match (validity, size) {
    (Validity::Invalid, _) | (Validity::Valid, 0) => {
      let Ok(_skipped) = reader.upto(u64::from(size));

      return Ok(Parsed::Skip);
    }
    (Validity::Valid, _) => {}
  }

  match (unsynchronised, version) {
    (Unsynchronised::Yes, Version::Four) => flags.bits |= UNSYNCHRONISED,
    (Unsynchronised::Yes, Version::Two | Version::Three) | (Unsynchronised::No, _) => {}
  }

  let size_2 = announced(reader, &mut flags, size)?;

  body(
    reader,
    Announced {
      id: id_3,
      size: size_2,
      flags,
    },
    version,
  )
}

struct Announced {
  id: String,
  size: u32,
  flags: Flags,
}

fn announced(reader: &mut Cursor<'_>, flags: &mut Flags, size: u32) -> Result<u32, ParseError> {
  let after_method = match flags.method {
    Some(_method) => {
      let left = size.checked_sub(1).ok_or(Malformed::Frame)?;
      let method = reader.byte()?;

      flags.method = Some(method);

      left
    }
    None => size,
  };

  let after_group = match flags.group {
    Some(_group) => {
      let left = after_method.checked_sub(1).ok_or(Malformed::Frame)?;
      let group = reader.byte()?;

      flags.group = Some(group);

      left
    }
    None => after_method,
  };

  let after_length = match (flags.data_length, flags.bits & COMPRESSED) {
    (None, 0) => after_group,
    (Some(_), _compressed) | (None, _compressed) => {
      let left = after_group.checked_sub(4).ok_or(Malformed::Frame)?;
      let written = reader.array::<4>()?;
      let Ok(length) = synchsafe(written);

      flags.data_length = Some(length);

      left
    }
  };

  match after_length {
    0 => Err(ParseError::Parse(Malformed::Frame)),
    _ => Ok(after_length),
  }
}

fn body(
  reader: &mut Cursor<'_>,
  announced: Announced,
  version: Version,
) -> Result<Parsed, ParseError> {
  let Announced { id, size, flags } = announced;
  let Ok(rest) = reader.rest();
  let Ok(mut whole) = Cursor::new(rest);
  let Ok(within) = whole.upto(u64::from(size));

  let unwrapped = match (flags.bits & UNSYNCHRONISED, flags.bits & COMPRESSED) {
    (0, 0) => None,
    (0, _compressed) => {
      let inflated = inflated(within)?;

      Some(inflated)
    }
    (_unsynchronised, 0) => {
      let Ok(undone) = resynchronised(within);

      Some(undone)
    }
    (_unsynchronised, _compressed) => {
      let Ok(undone) = resynchronised(within);

      let inflated = inflated(&undone)?;

      Some(inflated)
    }
  };

  let (content, wrapping) = match &unwrapped {
    Some(unwrapped_2) => (unwrapped_2.as_slice(), Wrapping::Wrapped),
    None => (within, Wrapping::Plain),
  };

  let Ok(mut content_reader) = Cursor::new(content);
  let announced_2 = Announced { id, size, flags };

  let parsed = match flags.method {
    Some(_method) => {
      let frame = encrypted(&mut content_reader, announced_2)?;

      Ok(Some(frame))
    }
    None => parsed(&mut content_reader, announced_2, version),
  };
  let parsed_2 = parsed?;

  let Ok(whole_2) = length(within);

  let consumed = match (&parsed_2, wrapping) {
    (Some(_), Wrapping::Plain) => {
      let Ok(left) = content_reader.left();

      whole_2.saturating_sub(left)
    }
    (Some(_), Wrapping::Wrapped) | (None, Wrapping::Plain | Wrapping::Wrapped) => whole_2,
  };

  let Ok(_consumed) = reader.upto(consumed);

  Ok(match parsed_2 {
    Some(frame) => Parsed::Next(frame),
    None => Parsed::Skip,
  })
}

fn inflated(compressed: &[u8]) -> Result<Vec<u8>, ParseError> {
  match console_core_deflate::zlib::decode(compressed, INFLATED_LIMIT) {
    Ok(inflated) => Ok(inflated),
    Err(_damaged) => Err(ParseError::Parse(Malformed::Frame)),
  }
}

fn encrypted(reader: &mut Cursor<'_>, announced: Announced) -> Result<Frame, ParseError> {
  let Announced { id, size, flags } = announced;

  match flags.data_length {
    Some(_) => {}
    None => return Err(ParseError::Parse(Malformed::Frame)),
  }

  let data = reader.take(u64::from(size))?;

  Ok(Frame {
    id,
    flags,
    body: Body::Binary(data.to_vec()),
  })
}

fn verified(byte: u8, version: Version) -> Result<Encoding, Malformed> {
  match (version, byte) {
    (Version::Two, 0 | 1) | (Version::Three | Version::Four, _) => Encoding::from_byte(byte),
    (Version::Two, _other) => Err(Malformed::Encoding),
  }
}

fn parsed(
  reader: &mut Cursor<'_>,
  announced: Announced,
  version: Version,
) -> Result<Option<Frame>, ParseError> {
  let Announced { id, flags, .. } = announced;

  let body = match id.as_str() {
    "APIC" => picture(reader, version).map(Some),
    "TXXX" => user_text(reader, version),
    "WXXX" => user_url(reader, version),
    "COMM" => language(reader, version, Value::Comment),
    "USLT" => language(reader, version, Value::Lyrics),
    "TIPL" | "TMCL" => involved(reader, version),
    "UFID" | "RVA2" | "OWNE" | "ETCO" | "PRIV" | "POPM" | "CHAP" | "CTOC" => {
      rest(reader, Retained::Opaque).map(Some)
    }
    "TDEN" | "TDOR" | "TDRC" | "TDRL" | "TDTG" => timestamp(reader),
    "GRP1" | "MVNM" | "MVIN" => text(reader, version),
    other => match other.as_bytes() {
      [b'T', ..] => text(reader, version),
      [b'W', ..] => url(reader).map(Some),
      _binary => rest(reader, Retained::Binary).map(Some),
    },
  };

  let body_2 = body?;

  Ok(body_2.map(|body_3| Frame {
    id,
    flags,
    body: body_3,
  }))
}

enum Retained {
  Opaque,
  Binary,
}

enum Value {
  Comment,
  Lyrics,
}

fn rest(reader: &mut Cursor<'_>, kept: Retained) -> Result<Body, Malformed> {
  let Ok(bytes) = reader.rest();
  let Ok(()) = reader.finish();

  Ok(match kept {
    Retained::Opaque => Body::Opaque(bytes.to_vec()),
    Retained::Binary => Body::Binary(bytes.to_vec()),
  })
}

fn text(reader: &mut Cursor<'_>, version: Version) -> Result<Option<Body>, Malformed> {
  let byte = match reader.byte() {
    Ok(byte) => byte,
    Err(_missing_encoding) => return Ok(None),
  };
  let encoding = verified(byte, version)?;
  let Decoded { text: value, .. } = decoded(reader, encoding, Ending::Rest)?;

  Ok(Some(Body::Text { encoding, value }))
}

fn url(reader: &mut Cursor<'_>) -> Result<Body, Malformed> {
  let Decoded { text, .. } = decoded(reader, Encoding::Latin1, Ending::Terminated)?;

  Ok(Body::Url(text))
}

fn user_text(reader: &mut Cursor<'_>, version: Version) -> Result<Option<Body>, Malformed> {
  let byte = match reader.byte() {
    Ok(byte) => byte,
    Err(_missing_encoding) => return Ok(None),
  };
  let encoding = verified(byte, version)?;
  let described = decoded(reader, encoding, Ending::Terminated)?;

  let content = match encoding {
    Encoding::Latin1 | Encoding::Utf16BigEndian | Encoding::Utf8 => {
      let content = decoded(reader, encoding, Ending::Rest)?;

      content.text
    }
    Encoding::Utf16 => {
      let Ok(raw) = reader.rest();
      let Ok(()) = reader.finish();

      user_text_utf16(raw, described.mark)?
    }
  };

  Ok(Some(Body::UserText {
    encoding,
    description: described.text,
    content,
  }))
}

fn user_text_utf16(raw: &[u8], described: [u8; 2]) -> Result<String, Malformed> {
  let mark = match raw {
    [] => return Ok(String::new()),
    [0xFF, 0xFE, ..] => [0xFF, 0xFE],
    [0xFE, 0xFF, ..] => [0xFE, 0xFF],
    [_first, ..] => described,
  };

  let Ok(order) = order_of(mark);

  match order {
    Some(order_2) => crate::text::utf16(raw, order_2),
    None => Err(Malformed::Text),
  }
}

fn user_url(reader: &mut Cursor<'_>, version: Version) -> Result<Option<Body>, Malformed> {
  let byte = match reader.byte() {
    Ok(byte) => byte,
    Err(_missing_encoding) => return Ok(None),
  };
  let encoding = verified(byte, version)?;
  let described = decoded(reader, encoding, Ending::Terminated)?;
  let content = decoded(reader, Encoding::Latin1, Ending::Rest)?;

  Ok(Some(Body::UserUrl {
    encoding,
    description: described.text,
    content: content.text,
  }))
}

fn language(
  reader: &mut Cursor<'_>,
  version: Version,
  said: Value,
) -> Result<Option<Body>, Malformed> {
  let byte = match reader.byte() {
    Ok(byte) => byte,
    Err(_missing_encoding) => return Ok(None),
  };
  let encoding = verified(byte, version)?;
  let language = reader.array::<3>()?;
  let described = decoded(reader, encoding, Ending::Terminated)?;

  let Ok(order) = match encoding {
    Encoding::Utf16 => order_of(described.mark),
    Encoding::Latin1 | Encoding::Utf16BigEndian | Encoding::Utf8 => Ok(None),
  };

  let content = match order {
    Some(order_2) => terminated_maybe_marked(reader, order_2),
    None => {
      let content = decoded(reader, encoding, Ending::Rest)?;

      Ok(content.text)
    }
  };
  let content_2 = content?;

  let description = described.text;

  Ok(Some(match said {
    Value::Comment => Body::Comment {
      encoding,
      language,
      description,
      content: content_2,
    },
    Value::Lyrics => Body::Lyrics {
      encoding,
      language,
      description,
      content: content_2,
    },
  }))
}

fn involved(reader: &mut Cursor<'_>, version: Version) -> Result<Option<Body>, Malformed> {
  let byte = reader.byte()?;
  let _encoding = verified(byte, version)?;

  let Ok(pairs) = reader.rest();
  let Ok(()) = reader.finish();

  Ok(match pairs {
    [] => None,
    involved_people => Some(Body::Opaque(involved_people.to_vec())),
  })
}

fn picture(reader: &mut Cursor<'_>, version: Version) -> Result<Body, Malformed> {
  let byte = reader.byte()?;
  let encoding = Encoding::from_byte(byte)?;

  let mime_type = match version {
    Version::Two => {
      let value_3 = reader.array::<3>()?;

      match value_3 {
        [b'P', b'N', b'G'] => Some(MimeType::Png),
        [b'J', b'P', b'G'] => Some(MimeType::Jpeg),
        _other => return Err(Malformed::Picture),
      }
    }
    Version::Three | Version::Four => {
      let Decoded { text: mime, .. } = decoded(reader, Encoding::Latin1, Ending::Terminated)?;

      match mime.as_str() {
        "" => None,
        media_type => {
          let Ok(mime_type) = MimeType::from_text(media_type);

          Some(mime_type)
        }
      }
    }
  };

  let number = reader.byte()?;
  let Ok(kind) = PictureType::from_u8(number);
  let Decoded {
    text: description, ..
  } = decoded(reader, encoding, Ending::Terminated)?;
  let Ok(data) = reader.rest();
  let Ok(()) = reader.finish();

  Ok(Body::Picture {
    encoding,
    description,
    picture: Picture {
      kind,
      mime_type,
      data: data.to_vec(),
    },
  })
}

fn timestamp(reader: &mut Cursor<'_>) -> Result<Option<Body>, Malformed> {
  let byte = reader.byte()?;
  let encoding = Encoding::from_byte(byte)?;
  let Decoded { text: written, .. } = decoded(reader, encoding, Ending::Rest)?;
  let parsed = Timestamp::parse(written.as_bytes())?;

  Ok(parsed.map(|timestamp| Body::Timestamp {
    encoding,
    timestamp,
  }))
}
