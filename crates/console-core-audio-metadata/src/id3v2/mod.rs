//! An ID3v2 tag: its frames read in, kept one to a key the way lofty kept
//! them, and said as the items and pictures a `Tag` holds.
//!
//! A tag is read whole into memory, undone of its unsynchronisation when it
//! is older than 2.4, and walked frame by frame. A frame that repeats an
//! earlier one replaces it -- except that an empty frame never replaces one
//! with something in it, which is what a tagger that wrote a field twice
//! usually meant -- and the frames the standard allows once per tag are
//! swapped out the way lofty swapped them, because the order they are left in
//! is the order the items come out in. A 2.3 tag's year, day and time are
//! three frames, and they are put together into the one date 2.4 writes.
//!
//! Only what somebody reads becomes an item: the text frames with a key, the
//! user texts named by one, the comments, the dates that are dates, and the
//! pictures. A genre is the list a `TCON` frame spells, numbers and all.

mod frames;
mod header;
mod timestamp;
mod write;

use std::collections::VecDeque;
use std::io::{Read, Seek};

use console_core_audio_decoding_interfaces::id3v2::undo_unsynchronisation;
use console_core_never::Never;

use crate::bytes::{Cursor, Source};
use crate::fault::ParseError;
use crate::id3v1::genre_name;
use crate::keys::{self, ItemKey};
use crate::tag::{Tag, TagType};

use frames::{Body, Flags, Frame, FrameContent, Parsed};
use header::{Unsynchronisation, Version};
use timestamp::Timestamp;

pub(crate) use header::{Header, read as read_header};
pub(crate) use write::written;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Identity {
  Same,
  Distinct,
}

impl From<bool> for Identity {
  fn from(same: bool) -> Self {
    match bool::then_some(same, ()) {
      Some(()) => Self::Same,
      None => Self::Distinct,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Validity {
  Valid,
  Invalid,
}

impl From<bool> for Validity {
  fn from(valid: bool) -> Self {
    match bool::then_some(valid, ()) {
      Some(()) => Self::Valid,
      None => Self::Invalid,
    }
  }
}

pub(crate) fn leading<R: Read + Seek>(
  source: &mut Source<'_, R>,
) -> Result<Option<Id3v2>, ParseError> {
  let Ok(length) = source.length();
  let mut tag: Option<Id3v2> = None;

  'tags: for _tag in 0..length.wrapping_div(10).saturating_add(1) {
    let at = source.at()?;
    let signature = source.upto(3)?;

    source.seek_to(at)?;

    match signature.as_slice() {
      b"ID3" => {}
      _other => break 'tags,
    }

    let header = read_header(source)?;
    let parsed = Id3v2::read(source, &header)?;
    let Ok(footer) = header.footer();

    source.seek_forward(footer)?;

    match &mut tag {
      Some(existing) => {
        let Ok(()) = existing.merge(parsed);
      }
      None => tag = Some(parsed),
    }
  }

  Ok(tag)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Id3v2 {
  frames: Vec<Frame>,
}

enum Replacing {
  Restoring,
  Plain,
}

impl Id3v2 {
  pub(crate) fn read<R: Read + Seek>(
    source: &mut Source<'_, R>,
    header: &Header,
  ) -> Result<Id3v2, ParseError> {
    let Ok(size) = header.content_size();
    let content = source.upto(size)?;

    Id3v2::parsed(&content, header)
  }

  pub(crate) fn parsed(content: &[u8], header: &Header) -> Result<Id3v2, ParseError> {
    let Ok(size) = header.content_size();
    let Ok(mut whole) = Cursor::new(content);
    let Ok(taken) = whole.upto(size);
    let Ok(unsynchronised) = header.unsynchronised();

    let undone = match (unsynchronised, header.version) {
      (Unsynchronisation::Applied, Version::Two | Version::Three) => {
        let Ok(undone) = undo_unsynchronisation(taken);

        Some(undone)
      }
      (Unsynchronisation::Applied, Version::Four) | (Unsynchronisation::Absent, _) => None,
    };

    let content_2 = match &undone {
      Some(undone_2) => undone_2.as_slice(),
      None => taken,
    };

    let mut tag = Id3v2::default();
    let Ok(mut reader) = Cursor::new(content_2);

    'frames: for _frame in 0..=u32::MAX {
      let parsed = frames::next(&mut reader, header.version, unsynchronised)?;

      match parsed {
        Parsed::Next(frame) => {
          let Ok(()) = tag.insert(frame, Replacing::Restoring);
        }
        Parsed::Skip => {}
        Parsed::End => break 'frames,
      }
    }

    match header.version {
      Version::Three => {
        let Ok(()) = tag.dated();
      }
      Version::Two | Version::Four => {}
    }

    Ok(tag)
  }

  pub(crate) fn merge(&mut self, other: Id3v2) -> Result<(), Never> {
    for frame in other.frames {
      let Ok(()) = self.insert(frame, Replacing::Plain);
    }

    Ok(())
  }

  fn insert(&mut self, frame: Frame, replacing: Replacing) -> Result<(), Never> {
    let Ok(content) = frame.content_state();
    let Ok(replaced) = self.replaced_by(frame);

    let restored = match (replacing, content, replaced) {
      (Replacing::Restoring, FrameContent::Empty, Some(replaced_2)) => {
        let Ok(kept) = replaced_2.content_state();

        match kept {
          FrameContent::Present => Some(replaced_2),
          FrameContent::Empty | FrameContent::Unmeasured => None,
        }
      }
      (Replacing::Restoring, FrameContent::Empty, None)
      | (Replacing::Restoring, FrameContent::Present | FrameContent::Unmeasured, _)
      | (Replacing::Plain, _, _) => None,
    };

    match restored {
      Some(restored_2) => {
        let Ok(_) = self.replaced_by(restored_2);
      }
      None => {}
    }

    Ok(())
  }

  fn replaced_by(&mut self, frame: Frame) -> Result<Option<Frame>, Never> {
    let replaced = match frame.id.as_str() {
      "MCDI" | "ETCO" | "MLLT" | "SYTC" | "RVRB" | "PCNT" | "RBUF" | "POSS" | "OWNE" | "SEEK"
      | "ASPI" => {
        let Ok(removed) = self.removed(&frame.id);

        removed.into_iter().next()
      }
      _may_repeat => {
        let Ok(replaced) = self.without_first_same(&frame);

        replaced
      }
    };

    self.frames.push(frame);

    Ok(replaced)
  }

  fn without_first_same(&mut self, frame: &Frame) -> Result<Option<Frame>, Never> {
    let held = std::mem::take(&mut self.frames);
    let mut replaced = None;

    for kept in held {
      let Ok(identity) = kept.same(frame);

      match (&replaced, identity) {
        (None, Identity::Same) => replaced = Some(kept),
        (None, Identity::Distinct) | (Some(_), Identity::Same | Identity::Distinct) => {
          self.frames.push(kept)
        }
      }
    }

    Ok(replaced)
  }

  fn removed(&mut self, id: &str) -> Result<Vec<Frame>, Never> {
    let held = std::mem::take(&mut self.frames);
    let mut kept = VecDeque::with_capacity(held.len());
    let mut removed = Vec::new();

    for frame in held {
      match Identity::from(frame.id == id) {
        Identity::Same => {
          removed.push(frame);

          match kept.pop_front() {
            Some(front) => kept.push_back(front),
            None => {}
          }
        }
        Identity::Distinct => kept.push_back(frame),
      }
    }

    self.frames = Vec::from(kept);

    Ok(removed)
  }

  fn first_text(&self, id: &str) -> Result<Option<String>, Never> {
    let first = self.frames.iter().find(|frame| frame.id == id);

    Ok(match first {
      Some(Frame {
        body: Body::Text { value, .. },
        ..
      }) => Some(value.clone()),
      Some(_) | None => None,
    })
  }

  fn dated(&mut self) -> Result<(), Never> {
    let Ok(removed) = self.removed("TDRC");

    let year = match removed.into_iter().next() {
      Some(year) => year,
      None => return Ok(()),
    };

    let (encoding, written) = match &year.body {
      Body::Timestamp {
        encoding,
        timestamp,
      } => (*encoding, *timestamp),
      Body::Text { .. }
      | Body::UserText { .. }
      | Body::Url(_)
      | Body::UserUrl { .. }
      | Body::Comment { .. }
      | Body::Lyrics { .. }
      | Body::Picture { .. }
      | Body::Opaque(_)
      | Body::Binary(_) => {
        let Ok(()) = self.insert(year, Replacing::Plain);

        return Ok(());
      }
    };

    match written.month {
      Some(_month) => return Ok(()),
      None => {}
    }

    let mut date = Timestamp {
      year: written.year,
      ..Timestamp::default()
    };
    let Ok(day) = self.first_text("TDAT");
    let Ok(day_2) = pair(day.as_deref());

    let time = match day_2 {
      Some((day_3, month)) => {
        date.day = Some(day_3);
        date.month = Some(month);

        let Ok(time) = self.first_text("TIME");

        let Ok(time_2) = pair(time.as_deref());

        time_2
      }
      None => None,
    };

    match time {
      Some((hour, minute)) => {
        date.hour = Some(hour);
        date.minute = Some(minute);
      }
      None => {}
    }

    let Ok(()) = self.insert(
      Frame {
        id: String::from("TDRC"),
        flags: Flags::default(),
        body: Body::Timestamp {
          encoding,
          timestamp: date,
        },
      },
      Replacing::Plain,
    );

    match day_2 {
      Some(_used) => {
        let Ok(_removed) = self.removed("TDAT");
      }
      None => {}
    }

    match time {
      Some(_used) => {
        let Ok(_removed) = self.removed("TIME");
      }
      None => {}
    }

    Ok(())
  }

  pub(crate) fn tag(self) -> Result<Tag, Never> {
    let Ok(mut tag) = Tag::new(TagType::Id3v2);

    for frame in self.frames {
      let Ok(()) = said(&mut tag, frame);
    }

    Ok(tag)
  }
}

fn pair(written: Option<&str>) -> Result<Option<(u8, u8)>, Never> {
  let written_3 = match written {
    Some(written_2) => written_2,
    None => return Ok(None),
  };

  let halves = match written_3.as_bytes() {
    [0..=127, 0..=127, 0..=127, 0..=127] => written_3.split_at_checked(2),
    _other => None,
  };

  Ok(match halves {
    Some((first, second)) => match (first.parse::<u8>(), second.parse::<u8>()) {
      (Ok(first_2), Ok(second_2)) => Some((first_2, second_2)),
      (Err(_unread), _) | (_, Err(_unread)) => None,
    },
    None => None,
  })
}

fn said(tag: &mut Tag, frame: Frame) -> Result<(), Never> {
  let Frame { id, body, .. } = frame;

  match body {
    Body::Text { value, .. } => match id.as_str() {
      "TCON" => {
        let Ok(genres) = genres(&value);

        for genre in genres {
          let Ok(()) = tag.push_text(ItemKey::Genre, genre.to_string());
        }
      }
      _ => {
        let Ok(key) = keys::named(keys::ID3V2, &id);

        let Ok(()) = values(tag, key, &value);
      }
    },
    Body::UserText {
      description,
      content,
      ..
    } => {
      let Ok(key) = match description.as_str() {
        "" => Ok(None),
        field_name => keys::named(keys::ID3V2, field_name),
      };

      let Ok(()) = values(tag, key, &content);
    }
    Body::Comment { content, .. } | Body::Lyrics { content, .. } => {
      let Ok(key) = keys::named(keys::ID3V2, &id);

      let Ok(()) = values(tag, key, &content);
    }
    Body::Picture { picture, .. } => tag.pictures.push(picture),
    Body::Timestamp { timestamp, .. } => {
      let Ok(key) = keys::named(keys::ID3V2, &id);
      let Ok(validity) = timestamp.verified();

      match (key, validity) {
        (Some(key_2), Validity::Valid) => {
          let Ok(()) = tag.push_text(key_2, timestamp.to_string());
        }
        (Some(_), Validity::Invalid) | (None, _) => {}
      }
    }
    Body::Url(_) | Body::UserUrl { .. } | Body::Opaque(_) | Body::Binary(_) => {}
  }

  Ok(())
}

fn values(tag: &mut Tag, key: Option<ItemKey>, content: &str) -> Result<(), Never> {
  match key {
    Some(key_2) => {
      for value in content.split('\0') {
        let Ok(()) = tag.push_text(key_2, value.to_string());
      }
    }
    None => {}
  }

  Ok(())
}

fn genres(value: &str) -> Result<Vec<&str>, Never> {
  let mut said = Vec::new();
  let mut rest = value;

  'genres: for _genre in 0..=u32::MAX {
    match rest {
      "" => break 'genres,
      _more => {}
    }

    let Ok((genre, after)) = split_genre(rest);
    let Ok(genre_2) = parse_genre(genre);

    said.push(genre_2);
    rest = after;
  }

  Ok(said)
}

fn split_genre(rest: &str) -> Result<(&str, &str), Never> {
  match rest.split_once('\0') {
    Some(parts) => return Ok(parts),
    None => {}
  }

  let inner = match rest.strip_prefix('(') {
    Some(inner) => inner,
    None => return Ok((rest, "")),
  };
  let (inside, after) = match inner.split_once(')') {
    Some(parts) => parts,
    None => return Ok((rest, "")),
  };
  let genre = match (inside.as_bytes(), inner.split_inclusive(')').next()) {
    ([b'(', ..], Some(closed)) => closed,
    _ordinary => inside,
  };

  Ok((genre, after))
}

pub(crate) fn parse_genre(genre: &str) -> Result<&str, Never> {
  match genre.len() {
    4.. => return Ok(genre),
    _short => {}
  }

  Ok(match (genre.parse::<u8>(), genre) {
    (Ok(number), _genre) => {
      let Ok(name) = genre_name(number);

      match name {
        Some(standard) => standard,
        None => genre,
      }
    }
    (Err(_not_a_number), "RX") => "Remix",
    (Err(_not_a_number), "CR") => "Cover",
    (Err(_not_a_number), _genre) => genre,
  })
}
