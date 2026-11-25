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

use console_core_never::Never;
use console_core_number_conversion::{fitted, fixed_index, index, indices};
use console_core_text_boundaries::previous_word_start;
use console_draw_surface::{KeyPress, Keysym};

use crate::boundaries;
use crate::input_range::{DeletionError, InputDeletion, deletion};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextField {
  text: String,
  caret: u32,
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

  pub fn typed(&mut self, key: Keysym) -> Result<Typed, Never> {
    let Ok(last) = self.letters();

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
      (press.modifiers & KeyPress::CONTROL != 0).then_some(()),
    ) {
      (Keysym::BackSpace, Some(())) => self.delete_previous_word(),
      (key, _) => self.typed(key),
    }
  }

  fn delete_previous_word(&mut self) -> Result<Typed, Never> {
    match self.caret {
      0 => Ok(Typed::None),
      _ => {
        let Ok(before) = self.before_caret();
        let Ok(start_byte) = previous_word_start(before);
        let Ok(start) = fitted(start_byte);
        let Ok(end) = fitted(before.len());

        self.remove_bytes(start..end)
      }
    }
  }

  pub fn insert(&mut self, letter: char) -> Result<Typed, Never> {
    let Ok(at) = boundaries::byte(&self.text, self.caret);
    let Ok(at_2) = index(at);

    self.text.insert(at_2, letter);
    let Ok(after) = fitted(at_2.saturating_add(letter.len_utf8()));
    let Ok(caret) = boundaries::at_or_after(&self.text, after);
    self.caret = caret;

    Ok(Typed::Edited)
  }

  pub fn apply_input(&mut self, bytes: InputDeletion, committed: &str) -> Result<Typed, Never> {
    let Ok(at) = boundaries::byte(&self.text, self.caret);
    let cursor = u64::from(at);
    let range = match deletion(&self.text, cursor, bytes) {
      Ok(removed) => removed,
      Err(DeletionError::Span) => cursor..cursor,
      Err(DeletionError::Caret) => return Ok(Typed::None),
    };
    let mut inserted = String::new();
    inserted.extend(committed.chars().filter(|letter| !letter.is_control()));

    match bool::then_some(range.is_empty() && inserted.is_empty(), ()) {
      Some(()) => return Ok(Typed::None),
      None => {}
    }

    let Ok(inserted_bytes) = fixed_index(inserted.len());
    let Ok(edit_end) = fitted(range.start.saturating_add(inserted_bytes));
    let Ok(native_range) = indices(range);
    self.text.replace_range(native_range, &inserted);
    let Ok(caret) = boundaries::at_or_after(&self.text, edit_end);
    self.caret = caret;

    Ok(Typed::Edited)
  }

  pub fn moved_to(&mut self, caret: u32) -> Result<Typed, Never> {
    let Ok(last) = self.letters();
    let caret_2 = caret.min(last);

    Ok(
      match ::core::primitive::bool::then_some(caret_2 == self.caret, ()) {
        ::core::option::Option::Some(()) => Typed::None,
        ::core::option::Option::None => {
          self.caret = caret_2;

          Typed::Moved
        }
      },
    )
  }

  fn remove(&mut self, range: std::ops::Range<u32>) -> Result<Typed, Never> {
    let Ok(from) = boundaries::byte(&self.text, range.start);
    let Ok(through) = boundaries::byte(&self.text, range.end);

    self.remove_bytes(from..through)
  }

  fn remove_bytes(&mut self, range: std::ops::Range<u32>) -> Result<Typed, Never> {
    let from = range.start;
    let Ok(native_range) = indices(range);
    self.text.replace_range(native_range, "");
    let Ok(caret) = boundaries::at_or_after(&self.text, from);
    self.caret = caret;

    Ok(Typed::Edited)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

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
