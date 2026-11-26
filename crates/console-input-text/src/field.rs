//! A line being typed, and where in it the next letter goes.
//!
//! The line used to be a string that only grew and shrank at its end, so the
//! arrows the keyboard draws reached a question and did nothing: a pair typed
//! as `()` could not be stepped back into, and a word misspelt at the front
//! was every letter after it typed again. The caret is counted in letters
//! as grapheme clusters, so combining accents and joined emoji stay whole.
//! Byte offsets are worked out only where the string is cut. An edit can join
//! adjacent clusters, so the caret is snapped to a boundary after every edit.
//! Input methods specify deletion counts in bytes on either side of the caret.
//! A deletion that splits a UTF-8 character is skipped. Committed characters
//! can still be inserted at a valid caret.

use std::collections::VecDeque;

use console_core_never::Never;
use console_core_number_conversion::{fitted, fixed_index, index, indices};
use console_core_text_boundaries::{next_word_end, previous_word_start};
use console_draw_surface::{KeyPress, Keysym};

use crate::boundaries;
use crate::input_range::{DeletionError, InputDeletion, deletion};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextField {
  text: String,
  caret: u32,
  anchor: Option<u32>,
  history: History,
}

const HISTORY_LIMIT: u32 = 64;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct History {
  undo: VecDeque<Edit>,
  redo: Vec<Edit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Edit {
  start: u64,
  removed: String,
  inserted: String,
  before: (u32, Option<u32>),
  after: u32,
}

#[derive(Clone, Copy)]
enum HistoryDirection {
  Undo,
  Redo,
}

const SHIFT: u32 = 1;
const COMMAND: u32 = KeyPress::CONTROL | (1 << 3) | (1 << 6);

#[derive(Clone, Copy)]
pub(crate) enum Movement {
  Place,
  Extend,
}

impl Movement {
  pub(crate) fn from_press(press: KeyPress) -> Result<Self, Never> {
    Ok(match (press.modifiers & SHIFT != 0).then_some(()) {
      Some(()) => Self::Extend,
      None => Self::Place,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
  Edited,
  Moved,
  None,
}

impl TextField {
  pub fn holding(text: &str) -> Result<Self, Never> {
    let Ok(caret) = boundaries::count(text);

    Ok(TextField {
      text: text.to_string(),
      caret,
      anchor: None,
      history: History::default(),
    })
  }

  pub fn text(&self) -> Result<&str, Never> {
    Ok(&self.text)
  }

  pub fn caret(&self) -> Result<u32, Never> {
    Ok(self.caret)
  }

  pub fn letters(&self) -> Result<u32, Never> {
    boundaries::count(&self.text)
  }

  pub fn before_caret(&self) -> Result<&str, Never> {
    let Ok(at) = boundaries::byte(&self.text, self.caret);
    let Ok(at_2) = index(at);

    Ok(match self.text.get(..at_2) {
      Some(before) => before,
      None => &self.text,
    })
  }

  pub fn selection(&self) -> Result<Option<std::ops::Range<u32>>, Never> {
    Ok(
      self
        .anchor
        .map(|anchor| anchor.min(self.caret)..anchor.max(self.caret)),
    )
  }

  pub fn selected_text(&self) -> Result<Option<&str>, Never> {
    let Ok(selection) = self.selection_bytes();

    Ok(match selection {
      Some(range) => {
        let Ok(native) = indices(range);
        self.text.get(native)
      }
      None => None,
    })
  }

  fn selection_bytes(&self) -> Result<Option<std::ops::Range<u64>>, Never> {
    let Ok(selection) = self.selection();

    Ok(selection.map(|range| {
      let Ok(from) = boundaries::byte(&self.text, range.start);
      let Ok(through) = boundaries::byte(&self.text, range.end);
      u64::from(from)..u64::from(through)
    }))
  }

  pub fn typed(&mut self, key: Keysym) -> Result<Typed, Never> {
    let Ok(last) = self.letters();
    let Ok(selection) = self.selection();

    match (key, selection) {
      (Keysym::BackSpace | Keysym::Delete, Some(range)) => return self.remove(range),
      (Keysym::Left, Some(range)) => return self.moved_to(range.start),
      (Keysym::Right, Some(range)) => return self.moved_to(range.end),
      _ => {}
    }

    match key {
      Keysym::BackSpace => {
        return Ok(match self.caret.checked_sub(1) {
          Some(before) => {
            let Ok(edited) = self.remove(before..self.caret);

            edited
          }
          None => Typed::None,
        });
      }
      Keysym::Delete => {
        return Ok(
          match ::core::primitive::bool::then_some(self.caret < last, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(edited) = self.remove(self.caret..self.caret.saturating_add(1));

              edited
            }
            ::core::option::Option::None => Typed::None,
          },
        );
      }
      Keysym::Left => return self.moved_to(self.caret.saturating_sub(1)),
      Keysym::Right => return self.moved_to(self.caret.saturating_add(1).min(last)),
      Keysym::Home => return self.moved_to(0),
      Keysym::End => return self.moved_to(last),
      _ => {}
    }

    let Ok(written) = key.char();
    let letter = match written {
      Some(letter) => letter,
      None => return Ok(Typed::None),
    };

    match ::core::primitive::bool::then_some(letter.is_control(), ()) {
      ::core::option::Option::Some(()) => Ok(Typed::None),
      ::core::option::Option::None => self.insert(letter),
    }
  }

  pub fn typed_press(&mut self, press: KeyPress) -> Result<Typed, Never> {
    match (
      press.key,
      press.modifiers & COMMAND,
      press.modifiers & SHIFT,
    ) {
      (Keysym::a | Keysym::A, KeyPress::CONTROL, 0) => self.select_all(),
      (Keysym::z | Keysym::Z, KeyPress::CONTROL, 0) => self.replay(HistoryDirection::Undo),
      (Keysym::z | Keysym::Z, KeyPress::CONTROL, SHIFT)
      | (Keysym::y | Keysym::Y, KeyPress::CONTROL, 0) => self.replay(HistoryDirection::Redo),
      (Keysym::BackSpace, KeyPress::CONTROL, _) => self.delete_previous_word(),
      (Keysym::Left | Keysym::Right, KeyPress::CONTROL, _) => self.move_word(press),
      (Keysym::Left, 0, SHIFT) => self.move_caret(self.caret.saturating_sub(1), Movement::Extend),
      (Keysym::Right, 0, SHIFT) => {
        self.move_caret(self.caret.saturating_add(1), Movement::Extend)
      }
      (Keysym::Home, 0, SHIFT) => self.move_caret(0, Movement::Extend),
      (Keysym::End, 0, SHIFT) => {
        let Ok(last) = self.letters();
        self.move_caret(last, Movement::Extend)
      }
      (key, 0, _) => self.typed(key),
      (_, _, _) => Ok(Typed::None),
    }
  }

  fn select_all(&mut self) -> Result<Typed, Never> {
    let Ok(end) = self.letters();
    self.set_caret(end, Some(0))
  }

  fn move_word(&mut self, press: KeyPress) -> Result<Typed, Never> {
    let Ok(before) = self.before_caret();
    let Ok(at) = fixed_index(before.len());
    let Ok(byte) = match press.key {
      Keysym::Left => previous_word_start(before),
      _ => {
        let remaining = match self.text.get(before.len()..) {
          Some(remaining) => remaining,
          None => return Ok(Typed::None),
        };
        let Ok(offset) = next_word_end(remaining);
        Ok(at.saturating_add(offset))
      }
    };
    let Ok(edge) = fitted(byte);
    let Ok(caret) = boundaries::at_or_after(&self.text, edge);
    let Ok(movement) = Movement::from_press(press);
    self.move_caret(caret, movement)
  }

  pub fn delete_selection(&mut self) -> Result<Typed, Never> {
    let Ok(selection) = self.selection();

    match selection {
      Some(range) => self.remove(range),
      None => Ok(Typed::None),
    }
  }

  fn delete_previous_word(&mut self) -> Result<Typed, Never> {
    let Ok(selection) = self.selection();

    match selection {
      Some(range) => return self.remove(range),
      None => {}
    }

    match self.caret {
      0 => Ok(Typed::None),
      _ => {
        let Ok(before) = self.before_caret();
        let Ok(start_byte) = previous_word_start(before);
        let Ok(start) = fitted(start_byte);
        let Ok(end) = fitted(before.len());

        self.replace_bytes(start..end, "")
      }
    }
  }

  pub fn insert(&mut self, letter: char) -> Result<Typed, Never> {
    self.insert_text(letter.encode_utf8(&mut [0; 4]))
  }

  pub(crate) fn insert_text(&mut self, text: &str) -> Result<Typed, Never> {
    match text {
      "" => return Ok(Typed::None),
      _ => {}
    }

    let Ok(selection) = self.selection_bytes();
    let range = match selection {
      Some(range) => range,
      None => {
        let Ok(at) = boundaries::byte(&self.text, self.caret);
        let byte = u64::from(at);
        byte..byte
      }
    };

    self.replace_bytes(range, text)
  }

  pub fn paste(&mut self, text: &str) -> Result<Typed, Never> {
    let first = match text.split(['\r', '\n']).next() {
      Some(line) => line,
      None => return Ok(Typed::None),
    };

    self.apply_input(
      InputDeletion {
        before: 0,
        after: 0,
      },
      first,
    )
  }

  pub fn apply_input(&mut self, bytes: InputDeletion, committed: &str) -> Result<Typed, Never> {
    let Ok(at) = boundaries::byte(&self.text, self.caret);
    let cursor = u64::from(at);
    let deletion = match deletion(&self.text, cursor, bytes) {
      Ok(removed) => removed,
      Err(DeletionError::Span) => cursor..cursor,
      Err(DeletionError::Caret) => return Ok(Typed::None),
    };
    let mut inserted = String::new();
    inserted.extend(committed.chars().filter(|letter| !letter.is_control()));
    let Ok(selection) = self.selection_bytes();
    let range = match (selection, inserted.is_empty(), deletion.is_empty()) {
      (Some(selected), false, _) | (Some(selected), _, false) => {
        deletion.start.min(selected.start)..deletion.end.max(selected.end)
      }
      _ => deletion,
    };

    match bool::then_some(range.is_empty() && inserted.is_empty(), ()) {
      Some(()) => return Ok(Typed::None),
      None => {}
    }

    self.replace_bytes(range, &inserted)
  }

  pub fn moved_to(&mut self, caret: u32) -> Result<Typed, Never> {
    self.move_caret(caret, Movement::Place)
  }

  pub fn extended_to(&mut self, caret: u32) -> Result<Typed, Never> {
    self.move_caret(caret, Movement::Extend)
  }

  pub fn select_word(&mut self, caret: u32) -> Result<Typed, Never> {
    let Ok(at) = boundaries::byte(&self.text, caret);
    let Ok(word) = console_core_text_boundaries::word_at(&self.text, u64::from(at));
    let Ok(start) = fitted(word.start);
    let Ok(end) = fitted(word.end);
    let Ok(anchor) = boundaries::at_or_after(&self.text, start);
    let Ok(through) = boundaries::at_or_after(&self.text, end);
    self.set_caret(through, Some(anchor))
  }

  pub(crate) fn move_caret(&mut self, caret: u32, movement: Movement) -> Result<Typed, Never> {
    let Ok(last) = self.letters();
    let anchor = match movement {
      Movement::Place => None,
      Movement::Extend => Some(match self.anchor {
        Some(anchor) => anchor,
        None => self.caret,
      }),
    };

    self.set_caret(caret.min(last), anchor)
  }

  fn set_caret(&mut self, caret: u32, anchor: Option<u32>) -> Result<Typed, Never> {
    let selection_start = anchor.filter(|from| *from != caret);

    Ok(
      match ::core::primitive::bool::then_some(
        (caret, selection_start) == (self.caret, self.anchor),
        (),
      ) {
        ::core::option::Option::Some(()) => Typed::None,
        ::core::option::Option::None => {
          self.caret = caret;
          self.anchor = selection_start;

          Typed::Moved
        }
      },
    )
  }

  fn remove(&mut self, range: std::ops::Range<u32>) -> Result<Typed, Never> {
    let Ok(from) = boundaries::byte(&self.text, range.start);
    let Ok(through) = boundaries::byte(&self.text, range.end);

    self.replace_bytes(u64::from(from)..u64::from(through), "")
  }

  fn replace_bytes(
    &mut self,
    range: std::ops::Range<u64>,
    inserted: &str,
  ) -> Result<Typed, Never> {
    let Ok(inserted_bytes) = fixed_index(inserted.len());
    let Ok(end) = fitted(range.start.saturating_add(inserted_bytes));
    let start = range.start;
    let before = (self.caret, self.anchor);
    let Ok(native_range) = indices(range);
    let removed = match self.text.get(native_range.clone()) {
      Some(removed) => removed.to_owned(),
      None => return Ok(Typed::None),
    };
    self.text.replace_range(native_range, inserted);
    let Ok(caret) = boundaries::at_or_after(&self.text, end);
    self.caret = caret;
    self.anchor = None;
    let Ok(()) = self.history.record(Edit {
      start,
      removed,
      inserted: inserted.to_owned(),
      before,
      after: caret,
    });

    Ok(Typed::Edited)
  }

  fn replay(&mut self, direction: HistoryDirection) -> Result<Typed, Never> {
    let pending = match direction {
      HistoryDirection::Undo => self.history.undo.pop_back(),
      HistoryDirection::Redo => self.history.redo.pop(),
    };
    let edit = match pending {
      Some(edit) => edit,
      None => return Ok(Typed::None),
    };
    let (current, replacement, (caret, anchor)) = match direction {
      HistoryDirection::Undo => (&edit.inserted, &edit.removed, edit.before),
      HistoryDirection::Redo => (&edit.removed, &edit.inserted, (edit.after, None)),
    };
    let Ok(bytes) = fixed_index(current.len());
    let Ok(range) = indices(edit.start..edit.start.saturating_add(bytes));
    self.text.replace_range(range, replacement);
    self.caret = caret;
    self.anchor = anchor;

    match direction {
      HistoryDirection::Undo => self.history.redo.push(edit),
      HistoryDirection::Redo => self.history.undo.push_back(edit),
    }

    Ok(Typed::Edited)
  }
}

impl History {
  fn record(&mut self, edit: Edit) -> Result<(), Never> {
    self.redo.clear();
    let Ok(limit) = index(HISTORY_LIMIT);

    match (self.undo.len() == limit).then_some(()) {
      Some(()) => {
        let _discarded = self.undo.pop_front();
      }
      None => {}
    }

    self.undo.push_back(edit);
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn shortcuts_select_graphemes_and_typing_replaces_the_selection() {
    let Ok(mut field) = TextField::holding("Cafe\u{301} 👩‍👩‍👧‍👦");
    let Ok(select) = KeyPress::with_modifiers(Keysym::a, KeyPress::CONTROL);
    let Ok(extend) = KeyPress::with_modifiers(Keysym::Left, SHIFT);
    let Ok(copy) = KeyPress::with_modifiers(Keysym::c, KeyPress::CONTROL);

    assert_eq!(field.typed_press(extend), Ok(Typed::Moved));
    assert_eq!(field.selected_text(), Ok(Some("👩‍👩‍👧‍👦")));
    assert_eq!(field.typed_press(copy), Ok(Typed::None));
    assert_eq!(field.typed_press(select), Ok(Typed::Moved));
    assert_eq!(field.selected_text(), Ok(Some("Cafe\u{301} 👩‍👩‍👧‍👦")));
    assert_eq!(field.typed(Keysym::x), Ok(Typed::Edited));
    assert_eq!(field.text(), Ok("x"));
    assert_eq!(field.selection(), Ok(None));
  }

  #[test]
  fn word_selection_and_undo_restore_the_edited_range() {
    let Ok(mut field) = TextField::holding("one cafe\u{301}");
    let Ok(select_word) = KeyPress::with_modifiers(Keysym::Left, KeyPress::CONTROL | SHIFT);
    let Ok(move_word) = KeyPress::with_modifiers(Keysym::Right, KeyPress::CONTROL);
    let Ok(undo) = KeyPress::with_modifiers(Keysym::z, KeyPress::CONTROL);
    let Ok(redo) = KeyPress::with_modifiers(Keysym::z, KeyPress::CONTROL | SHIFT);

    assert_eq!(field.typed_press(select_word), Ok(Typed::Moved));
    assert_eq!(field.selected_text(), Ok(Some("cafe\u{301}")));
    assert_eq!(field.paste("tea\r\nother line"), Ok(Typed::Edited));
    assert_eq!(field.text(), Ok("one tea"));
    assert_eq!(field.typed_press(undo), Ok(Typed::Edited));
    assert_eq!(field.selected_text(), Ok(Some("cafe\u{301}")));
    assert_eq!(field.typed_press(move_word), Ok(Typed::Moved));
    assert_eq!(field.selection(), Ok(None));
    assert_eq!(field.typed_press(redo), Ok(Typed::Edited));
    assert_eq!(field.text(), Ok("one tea"));
  }

  #[test]
  fn control_backspace_erases_the_previous_word_and_trailing_space() {
    let Ok(mut field) = TextField::holding("one cafe\u{301}   tail");
    let Ok(erase) = KeyPress::with_modifiers(Keysym::BackSpace, KeyPress::CONTROL);

    assert_eq!(field.moved_to(11), Ok(Typed::Moved));
    assert_eq!(field.typed_press(erase), Ok(Typed::Edited));
    assert_eq!(field.text(), Ok("one tail"));
    assert_eq!(field.caret(), Ok(4));
    assert_eq!(field.typed_press(erase), Ok(Typed::Edited));
    assert_eq!(field.text(), Ok("tail"));
    assert_eq!(field.typed_press(erase), Ok(Typed::None));
  }

  #[test]
  fn an_input_method_replaces_multibyte_text_and_leaves_the_caret_after_the_commit() {
    let Ok(mut field) = TextField::holding("a旧界z");
    let splits_character = InputDeletion {
      before: 1,
      after: 0,
    };

    assert_eq!(field.moved_to(2), Ok(Typed::Moved));
    assert_eq!(field.apply_input(splits_character, ""), Ok(Typed::None));
    assert_eq!(field.text(), Ok("a旧界z"));

    let two_characters = InputDeletion {
      before: 3,
      after: 3,
    };
    assert_eq!(
      field.apply_input(two_characters, "新しい"),
      Ok(Typed::Edited)
    );
    assert_eq!(field.text(), Ok("a新しいz"));
    assert_eq!(field.before_caret(), Ok("a新しい"));

    assert_eq!(
      field.apply_input(splits_character, "\n!"),
      Ok(Typed::Edited)
    );
    assert_eq!(field.text(), Ok("a新しい!z"));
    assert_eq!(field.before_caret(), Ok("a新しい!"));
  }

  fn typed(field: &mut TextField, keys: &[Keysym]) -> Result<Vec<Typed>, Never> {
    let mut told = Vec::new();

    for key in keys {
      let Ok(typed) = field.typed(*key);

      told.push(typed);
    }

    Ok(told)
  }

  #[test]
  fn a_letter_goes_into_the_line_and_a_backspace_takes_it_out_again() {
    let mut field = TextField::default();

    assert_eq!(
      typed(&mut field, &[Keysym::a, Keysym::b]),
      Ok(vec![Typed::Edited, Typed::Edited])
    );
    assert_eq!(field.text(), Ok("ab"));

    assert_eq!(field.typed(Keysym::BackSpace), Ok(Typed::Edited));
    assert_eq!(field.text(), Ok("a"));

    assert_eq!(
      typed(&mut field, &[Keysym::Return, Keysym::Down]),
      Ok(vec![Typed::None, Typed::None])
    );
    assert_eq!(
      field.text(),
      Ok("a"),
      "a key that means something to the panel is not a letter"
    );

    assert_eq!(
      TextField::default().typed(Keysym::BackSpace),
      Ok(Typed::None),
      "nothing to take out of an empty line"
    );
  }

  #[test]
  fn a_pair_is_typed_and_then_stepped_back_into() {
    let mut field = TextField::default();
    let Ok(_typed) = typed(
      &mut field,
      &[
        Keysym::parenleft,
        Keysym::parenright,
        Keysym::Left,
        Keysym::x,
      ],
    );

    assert_eq!(field.text(), Ok("(x)"));
    assert_eq!(field.before_caret(), Ok("(x"));
  }

  #[test]
  fn the_caret_stops_at_either_end_of_the_line() {
    let Ok(mut field) = TextField::holding("ab");

    assert_eq!(
      field.typed(Keysym::Right),
      Ok(Typed::None),
      "a line held starts with the caret after it"
    );
    assert_eq!(field.typed(Keysym::Home), Ok(Typed::Moved));
    assert_eq!(field.typed(Keysym::Left), Ok(Typed::None));
    assert_eq!(
      field.typed(Keysym::BackSpace),
      Ok(Typed::None),
      "nothing is before the front"
    );
    assert_eq!(field.typed(Keysym::Delete), Ok(Typed::Edited));
    assert_eq!(field.text(), Ok("b"));
    assert_eq!(field.typed(Keysym::End), Ok(Typed::Moved));
    assert_eq!(
      field.typed(Keysym::Delete),
      Ok(Typed::None),
      "nothing is after the end"
    );
  }

  #[test]
  fn a_step_is_a_letter_and_not_a_byte() {
    let Ok(mut field) = TextField::holding("café");
    let Ok(_typed) = typed(&mut field, &[Keysym::Left, Keysym::BackSpace]);

    assert_eq!(field.text(), Ok("caé"));
    assert_eq!(field.before_caret(), Ok("ca"));
  }

  #[test]
  fn accents_and_joined_emoji_are_one_step_and_one_deletion() {
    for cluster in ["e\u{301}", "👩‍👩‍👧‍👦", "🇧🇪", "क्\u{200d}ष"] {
      let text = format!("a{cluster}中");
      let Ok(mut field) = TextField::holding(&text);

      assert_eq!(field.letters(), Ok(3), "{cluster}");
      assert_eq!(field.typed(Keysym::Left), Ok(Typed::Moved));
      assert_eq!(field.caret(), Ok(2));
      assert_eq!(field.before_caret(), Ok(format!("a{cluster}").as_str()));
      assert_eq!(field.typed(Keysym::BackSpace), Ok(Typed::Edited));
      assert_eq!(field.text(), Ok("a中"));
      assert_eq!(field.caret(), Ok(1));

      let Ok(mut forward_delete) = TextField::holding(&text);
      let Ok(_moved) = forward_delete.moved_to(1);
      assert_eq!(forward_delete.typed(Keysym::Delete), Ok(Typed::Edited));
      assert_eq!(forward_delete.text(), Ok("a中"));
      assert_eq!(forward_delete.caret(), Ok(1));
    }
  }

  #[test]
  fn typed_combining_marks_and_emoji_joins_keep_the_caret_on_a_boundary() {
    let Ok(mut field) = TextField::holding("ex");
    let Ok(_moved) = field.moved_to(1);
    assert_eq!(field.insert('\u{301}'), Ok(Typed::Edited));
    assert_eq!(field.text(), Ok("e\u{301}x"));
    assert_eq!(field.caret(), Ok(1));
    assert_eq!(field.before_caret(), Ok("e\u{301}"));
    assert_eq!(field.insert('!'), Ok(Typed::Edited));
    assert_eq!(field.text(), Ok("e\u{301}!x"));
    assert_eq!(field.caret(), Ok(2));

    let Ok(mut emoji) = TextField::holding("👩👩");
    let Ok(_moved) = emoji.moved_to(1);
    assert_eq!(emoji.insert('\u{200d}'), Ok(Typed::Edited));
    assert_eq!(emoji.text(), Ok("👩‍👩"));
    assert_eq!(emoji.letters(), Ok(1));
    assert_eq!(emoji.caret(), Ok(1));
    assert_eq!(emoji.typed(Keysym::BackSpace), Ok(Typed::Edited));
    assert_eq!(emoji.text(), Ok(""));
  }

  #[test]
  fn deleting_a_separator_can_join_clusters_without_stranding_the_caret() {
    for key in [Keysym::BackSpace, Keysym::Delete] {
      let Ok(mut field) = TextField::holding("e\n\u{301}x");
      let Ok(_moved) = field.moved_to(match key {
        Keysym::BackSpace => 2,
        _ => 1,
      });

      assert_eq!(field.typed(key), Ok(Typed::Edited));
      assert_eq!(field.text(), Ok("e\u{301}x"));
      assert_eq!(field.caret(), Ok(1));
      assert_eq!(field.before_caret(), Ok("e\u{301}"));
    }
  }
}
