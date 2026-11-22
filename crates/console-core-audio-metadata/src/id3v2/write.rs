//! An ID3v2.4 tag written from a `Tag`, the one tag this crate writes.
//!
//! Items become frames the way lofty made them. The texts that can hold
//! several values are gathered by frame and joined with the NUL 2.4 separates
//! values with, the comments into one comment, a recording date becomes a
//! date frame when it reads as one and a text frame when it does not, and a
//! label is a publisher. A frame made earlier wins over a later one the tag
//! would count as the same, which is how a label stands in front of a
//! publisher. A key with no frame is not written. Every text is UTF-8, the
//! pictures follow the frames, and the tag ends in the kilobyte of padding
//! lofty left for the next write. A tag with nothing to write is no bytes at
//! all, so whoever places it can take the old one away.

use console_core_never::Never;

use crate::fault::FileEncodingError;
use crate::id3v2::timestamp::Timestamp;
use crate::id3v2::{Identity, Validity};
use crate::keys::{self, ItemKey};
use crate::picture::Picture;
use crate::tag::Tag;

const PADDING: [u8; 1024] = [0; 1024];

const LARGEST: u32 = 0x0FFF_FFFF;

enum Written {
  Text(&'static str, String),
  Date(Timestamp),
  Comment(String),
}

impl Written {
  fn same(&self, other: &Written) -> Result<Identity, Never> {
    Ok(match (self, other) {
      (Written::Text(mine, _mine), Written::Text(theirs, _theirs)) => {
        Identity::from(mine == theirs)
      }
      (Written::Date(mine), Written::Date(theirs)) => Identity::from(mine == theirs),
      (Written::Comment(_mine), Written::Comment(_theirs)) => Identity::Same,
      (Written::Text(..), Written::Date(_) | Written::Comment(_))
      | (Written::Date(_), Written::Text(..) | Written::Comment(_))
      | (Written::Comment(_), Written::Text(..) | Written::Date(_)) => Identity::Distinct,
    })
  }

  fn id(&self) -> Result<&'static str, Never> {
    Ok(match self {
      Written::Text(id, _value) => id,
      Written::Date(_date) => "TDRC",
      Written::Comment(_comment) => "COMM",
    })
  }

  fn body(&self) -> Result<Vec<u8>, FileEncodingError> {
    let mut body = vec![3u8];

    match self {
      Written::Text(_, value) => body.extend_from_slice(value.as_bytes()),
      Written::Date(date) => {
        let Ok(validity) = date.verified();

        match validity {
          Validity::Valid => body.extend_from_slice(date.to_string().as_bytes()),
          Validity::Invalid => return Err(FileEncodingError::Timestamp),
        }
      }
      Written::Comment(comment) => {
        body.extend_from_slice(b"XXX\0");
        body.extend_from_slice(comment.as_bytes());
      }
    }

    Ok(body)
  }
}

fn kept(frames: &mut Vec<Written>, frame: Written) -> Result<(), Never> {
  let known = frames
    .iter()
    .find(|held| matches!(held.same(&frame), Ok(Identity::Same)));

  match known {
    Some(_held) => {}
    None => frames.push(frame),
  }

  Ok(())
}

fn joined(into: &mut String, value: &str) -> Result<(), Never> {
  into.push('\0');
  into.push_str(value);

  Ok(())
}

struct TextFrame<'a> {
  id: &'static str,
  value: &'a str,
}

fn gathered(
  gathering: &mut Vec<(&'static str, String)>,
  frame: TextFrame<'_>,
) -> Result<(), Never> {
  let TextFrame { id, value } = frame;
  let held = gathering.iter_mut().find(|(held, _value)| *held == id);

  match held {
    Some((_, into)) => {
      let Ok(()) = joined(into, value);
    }
    None => gathering.push((id, value.to_string())),
  }

  Ok(())
}

fn frames(tag: &Tag) -> Result<Vec<Written>, Never> {
  let mut frames = Vec::new();
  let mut gathering: Vec<(&'static str, String)> = Vec::new();
  let mut comment: Option<String> = None;

  for item in &tag.items {
    let Ok(id) = keys::id3v2_frame(item.key);

    match (item.key, id) {
      (
        ItemKey::TrackArtist
        | ItemKey::AlbumArtist
        | ItemKey::TrackTitle
        | ItemKey::AlbumTitle
        | ItemKey::TrackSubtitle
        | ItemKey::Genre
        | ItemKey::Composer
        | ItemKey::CopyrightMessage
        | ItemKey::Publisher,
        Some(id_2),
      ) => {
        let Ok(()) = gathered(
          &mut gathering,
          TextFrame {
            id: id_2,
            value: &item.text,
          },
        );
      }
      (ItemKey::Comment, _) => match &mut comment {
        Some(into) => {
          let Ok(()) = joined(into, &item.text);
        }
        None => comment = Some(item.text.clone()),
      },
      (ItemKey::RecordingDate, _) => {
        let frame = match Timestamp::parse(item.text.as_bytes()) {
          Ok(Some(date)) => Written::Date(date),
          Ok(None) => Written::Text("TDRC", item.text.clone()),
          Err(_not_a_timestamp) => Written::Text("TDRC", item.text.clone()),
        };

        let Ok(()) = kept(&mut frames, frame);
      }
      (ItemKey::Label, Some(id_3)) => {
        let Ok(()) = kept(&mut frames, Written::Text(id_3, item.text.clone()));
      }
      (
        ItemKey::TrackArtist
        | ItemKey::AlbumArtist
        | ItemKey::TrackTitle
        | ItemKey::AlbumTitle
        | ItemKey::TrackSubtitle
        | ItemKey::Genre
        | ItemKey::Composer
        | ItemKey::CopyrightMessage
        | ItemKey::Publisher
        | ItemKey::Label,
        None,
      )
      | (ItemKey::Year | ItemKey::Description | ItemKey::Performer, _) => {}
    }
  }

  for (id, value) in gathering {
    let Ok(()) = kept(&mut frames, Written::Text(id, value));
  }

  match comment {
    Some(comment_2) => {
      let Ok(()) = kept(&mut frames, Written::Comment(comment_2));
    }
    None => {}
  }

  Ok(frames)
}

fn synchsafe(size: u32) -> Result<[u8; 4], FileEncodingError> {
  match size {
    0..=LARGEST => {}
    _oversized => return Err(FileEncodingError::TooLarge),
  }

  let seven = |shift: u32| {
    let [_high, _upper, _lower, low] = (size.wrapping_shr(shift) & 0x7F).to_be_bytes();

    low
  };

  Ok([seven(21), seven(14), seven(7), seven(0)])
}

fn framed(into: &mut Vec<u8>, id: &str, body: &[u8]) -> Result<(), FileEncodingError> {
  let size = match u32::try_from(body.len()) {
    Ok(size) => size,
    Err(_too_large) => return Err(FileEncodingError::TooLarge),
  };

  let size_2 = synchsafe(size)?;

  into.extend_from_slice(id.as_bytes());
  into.extend_from_slice(&size_2);
  into.extend_from_slice(&[0, 0]);
  into.extend_from_slice(body);

  Ok(())
}

fn pictured(picture: &Picture) -> Result<Vec<u8>, Never> {
  let mut body = vec![3u8];

  match &picture.mime_type {
    Some(mime_type) => {
      let Ok(text) = mime_type.as_text();

      body.extend_from_slice(text.as_bytes());
    }
    None => {}
  }

  let Ok(kind) = picture.kind.as_u8();

  body.extend_from_slice(&[0, kind, 0]);
  body.extend_from_slice(&picture.data);

  Ok(body)
}

pub(crate) fn written(tag: &Tag) -> Result<Vec<u8>, FileEncodingError> {
  let Ok(frames) = frames(tag);

  match (frames.as_slice(), tag.pictures.as_slice()) {
    ([], []) => return Ok(Vec::new()),
    ([_, ..], _) | ([], [_, ..]) => {}
  }

  let mut content = Vec::new();

  for frame in &frames {
    let body = frame.body()?;
    let Ok(id) = frame.id();

    framed(&mut content, id, &body)?;
  }

  for picture in &tag.pictures {
    let Ok(body) = pictured(picture);

    framed(&mut content, "APIC", &body)?;
  }

  content.extend_from_slice(&PADDING);

  let size = match u32::try_from(content.len()) {
    Ok(size) => size,
    Err(_too_large) => return Err(FileEncodingError::TooLarge),
  };

  let size_2 = synchsafe(size)?;
  let mut tag_2 = Vec::new();

  tag_2.extend_from_slice(b"ID3\x04\x00\x00");
  tag_2.extend_from_slice(&size_2);
  tag_2.extend_from_slice(&content);

  Ok(tag_2)
}
