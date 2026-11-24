//! In-memory clipboard history and its bounded panel preview.

use std::collections::VecDeque;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use serde::Deserialize;

const MAX_ENTRIES: u32 = 40;
const MAX_HISTORY_BYTES: u32 = 32 * 1024 * 1024;
const PREVIEW_CHARS: u32 = 96;
pub const MAX_TEXT_BYTES: u32 = 1024 * 1024;
const MAX_IMAGE_BYTES: u32 = 8 * 1024 * 1024;

pub const OWN_OFFER: &str = "application/x-console-clipboard-history";
pub const PNG: &str = "image/png";
const TEXT_TYPES: &[&str] = &["text/plain;charset=utf-8", "UTF8_STRING", "text/plain"];
const IMAGE_TYPES: &[&str] = &[PNG, "image/jpeg", "image/webp", "image/gif", "image/bmp"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
  Text,
  Image(&'static str),
}

impl Format {
  pub fn limit(self) -> Result<u32, Never> {
    Ok(match self {
      Self::Text => MAX_TEXT_BYTES,
      Self::Image(_) => MAX_IMAGE_BYTES,
    })
  }

  pub fn offered(self) -> Result<Vec<&'static str>, Never> {
    Ok(match self {
      Self::Text => TEXT_TYPES.to_vec(),
      Self::Image(mime) => vec![mime],
    })
  }
}

pub fn format(offered: &[String]) -> Result<Option<(Format, &str)>, Never> {
  let private = offered.iter().any(|mime| {
    mime.eq_ignore_ascii_case(OWN_OFFER)
      || mime.eq_ignore_ascii_case("x-kde-passwordManagerHint")
      || mime.eq_ignore_ascii_case("application/x-kde-passwordManagerHint")
  });

  match private.then_some(()) {
    Some(()) => return Ok(None),
    None => {}
  }

  let matching = |name: &str| offered.iter().find(|mime| mime.eq_ignore_ascii_case(name));

  Ok(
    TEXT_TYPES
      .iter()
      .find_map(|name| matching(name).map(|mime| (Format::Text, mime.as_str())))
      .or_else(|| {
        IMAGE_TYPES
          .iter()
          .find_map(|name| matching(name).map(|mime| (Format::Image(name), mime.as_str())))
      }),
  )
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preview {
  pub id: u64,
  pub label: PreviewLabel,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct PreviewLabel(String);

impl PreviewLabel {
  pub fn text(&self) -> Result<&str, Never> {
    Ok(&self.0)
  }
}

impl<'de> Deserialize<'de> for PreviewLabel {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    deserializer.deserialize_str(LabelVisitor)
  }
}

struct LabelVisitor;

impl serde::de::Visitor<'_> for LabelVisitor {
  type Value = PreviewLabel;

  fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str("a bounded clipboard preview label")
  }

  fn visit_str<E: serde::de::Error>(self, label: &str) -> Result<Self::Value, E> {
    let Ok(maximum) = index(PREVIEW_CHARS.saturating_add(1));

    match label.chars().nth(maximum) {
      Some(_excess) => Err(E::custom("clipboard preview is too long")),
      None => Ok(PreviewLabel(label.to_owned())),
    }
  }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Contents {
  pub format: Format,
  pub bytes: Vec<u8>,
}

impl Contents {
  pub fn length(&self) -> Result<u32, Never> {
    fitted(self.bytes.len())
  }

  fn preview(&self) -> Result<Option<String>, Never> {
    let text = match self.format {
      Format::Text => match std::str::from_utf8(&self.bytes) {
        Ok(text) => text,
        Err(_invalid_utf8) => return Ok(None),
      },
      Format::Image(mime) => {
        return Ok(Some(format!(
          "Image · {}",
          mime.trim_start_matches("image/").to_uppercase()
        )));
      }
    };
    let mut chars = text
      .split_whitespace()
      .flat_map(|word| word.chars().chain([' ']))
      .filter(|letter| !letter.is_control());
    let Ok(limit) = index(PREVIEW_CHARS);
    let mut label: String = chars.by_ref().take(limit).collect();
    label.truncate(label.trim_end().len());

    match chars.any(|letter| letter != ' ').then_some(()) {
      Some(()) => label.push('…'),
      None => {}
    }

    Ok(Some(match label.is_empty().then_some(()) {
      Some(()) => "Whitespace".to_string(),
      None => label,
    }))
  }
}

struct Entry {
  id: u64,
  label: PreviewLabel,
  contents: Contents,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(from = "bool", into = "bool")]
pub enum Capture {
  #[default]
  Recording,
  Paused,
}

impl From<bool> for Capture {
  fn from(paused: bool) -> Self {
    match paused {
      true => Self::Paused,
      false => Self::Recording,
    }
  }
}

impl From<Capture> for bool {
  fn from(capture: Capture) -> Self {
    match capture {
      Capture::Paused => true,
      Capture::Recording => false,
    }
  }
}

#[derive(Default)]
pub struct History {
  entries: VecDeque<Entry>,
  next: u64,
  bytes: u32,
  pub capture: Capture,
}

impl History {
  pub fn new(capture: Capture, start: u64) -> Result<Self, Never> {
    Ok(Self {
      next: start,
      capture,
      ..Self::default()
    })
  }

  pub fn remember(&mut self, contents: Contents) -> Result<(), Never> {
    match (self.capture, contents.bytes.as_slice()) {
      (Capture::Paused, _) | (Capture::Recording, []) => return Ok(()),
      (Capture::Recording, [_, ..]) => {}
    }

    let Ok(length) = contents.length();
    let Ok(limit) = contents.format.limit();

    match length.cmp(&limit) {
      std::cmp::Ordering::Greater => return Ok(()),
      std::cmp::Ordering::Equal | std::cmp::Ordering::Less => {}
    }

    let existing = self
      .entries
      .iter()
      .zip(0_u32..)
      .find_map(|(entry, position)| (entry.contents == contents).then_some(position));

    match existing {
      Some(position) => {
        let Ok(_promoted) = self.promote(position);
        return Ok(());
      }
      None => {}
    }

    let Ok(preview) = contents.preview();
    let label = match preview {
      Some(label) => label,
      None => return Ok(()),
    };

    self.next = self.next.wrapping_add(1);
    self.bytes = self.bytes.saturating_add(length);
    self.entries.push_front(Entry {
      id: self.next,
      label: PreviewLabel(label),
      contents,
    });

    self.trim()
  }

  fn trim(&mut self) -> Result<(), Never> {
    let Ok(count) = fitted::<_, u32>(self.entries.len());

    for _ in 0..count {
      let Ok(remaining) = fitted::<_, u32>(self.entries.len());

      match (remaining > MAX_ENTRIES || self.bytes > MAX_HISTORY_BYTES).then_some(()) {
        Some(()) => {}
        None => break,
      }

      match self.entries.pop_back() {
        Some(entry) => {
          let Ok(length) = entry.contents.length();
          self.bytes = self.bytes.saturating_sub(length);
        }
        None => break,
      }
    }

    Ok(())
  }

  pub fn previews(&self) -> Result<Vec<Preview>, Never> {
    match self.capture {
      Capture::Paused => return Ok(Vec::new()),
      Capture::Recording => {}
    }

    Ok(
      self
        .entries
        .iter()
        .map(|entry| Preview {
          id: entry.id,
          label: entry.label.clone(),
        })
        .collect(),
    )
  }

  pub fn restore(&mut self, id: u64) -> Result<Option<&Contents>, Never> {
    match self.capture {
      Capture::Paused => return Ok(None),
      Capture::Recording => {}
    }

    let selected = self
      .entries
      .iter()
      .zip(0_u32..)
      .find_map(|(entry, position)| (entry.id == id).then_some(position));

    match selected {
      Some(position) => self.promote(position),
      None => Ok(None),
    }
  }

  fn promote(&mut self, position: u32) -> Result<Option<&Contents>, Never> {
    let Ok(at) = index(position);

    match self.entries.remove(at) {
      Some(entry) => {
        self.entries.push_front(entry);
        Ok(self.entries.front().map(|entry_2| &entry_2.contents))
      }
      None => Ok(None),
    }
  }

  pub fn clear(&mut self) -> Result<(), Never> {
    self.entries.clear();
    self.bytes = 0;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use std::io;

  use super::*;

  fn text(bytes: &[u8]) -> Result<Contents, Never> {
    Ok(Contents {
      format: Format::Text,
      bytes: bytes.to_vec(),
    })
  }

  #[test]
  fn preview_and_restore_keep_distinct_payloads_with_the_same_label() -> io::Result<()> {
    let mut history = History::default();
    let Ok(multiline) = text(b"first\nline");
    let Ok(single_line) = text(b"first line");
    let Ok(()) = history.remember(multiline);
    let Ok(()) = history.remember(single_line.clone());
    let Ok(previews) = history.previews();

    let (newest, oldest) = match previews.as_slice() {
      [newest, oldest] => (newest, oldest),
      _unexpected_count => return Err(io::Error::other("expected exactly two previews")),
    };

    assert_eq!(newest.label, oldest.label);

    let Ok(restored) = history.restore(oldest.id);
    let contents = restored.ok_or_else(|| io::Error::other("missing saved multiline text"))?;
    assert_eq!(contents.bytes, b"first\nline");

    let Ok(()) = history.remember(single_line);
    let Ok(current) = history.previews();
    assert_eq!(current.first().map(|preview| preview.id), Some(newest.id));

    Ok(())
  }

  #[test]
  fn privacy_hides_older_entries_and_skips_new_copies() -> io::Result<()> {
    let mut history = History::default();
    let Ok(saved_text) = text(b"saved");
    let Ok(unrecorded) = text(b"secret");
    let Ok(next) = text(b"next");
    let Ok(()) = history.remember(saved_text);
    let Ok(initial) = history.previews();
    let checked_value = initial
      .first()
      .ok_or_else(|| io::Error::other("missing saved preview"))?;
    let saved = checked_value.id;
    history.capture = Capture::Paused;
    let Ok(()) = history.remember(unrecorded);
    let Ok(hidden) = history.previews();
    let Ok(not_restored) = history.restore(saved);
    assert!(hidden.is_empty());
    assert!(not_restored.is_none());

    history.capture = Capture::Recording;
    let Ok(visible) = history.previews();
    let Ok(restored) = history.restore(saved);
    let contents = restored.ok_or_else(|| io::Error::other("missing saved text"))?;
    assert_eq!(visible.len(), 1);
    assert_eq!(contents.bytes, b"saved");
    let Ok(()) = history.clear();
    let Ok(()) = history.remember(next);
    let Ok(cleared) = history.restore(saved);
    assert!(cleared.is_none());

    Ok(())
  }

  #[test]
  fn password_and_own_offers_are_ignored_before_receiving_any_data() {
    let mut offered = vec!["text/plain".to_string()];
    let Ok(ordinary) = format(&offered);
    assert_eq!(ordinary, Some((Format::Text, "text/plain")));

    for mime in [
      "x-kde-passwordManagerHint",
      "application/x-kde-passwordManagerHint",
      OWN_OFFER,
    ] {
      offered.push(mime.to_string());
      let Ok(ignored) = format(&offered);
      assert_eq!(ignored, None);
      offered.pop();
    }
  }
}
