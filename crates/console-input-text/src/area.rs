//! Text typed across many lines, and where in it the next letter goes.
//!
//! An area is a field that lets Return in. Everything a field already answers
//! -- a letter, a backspace, a delete, a step left or right -- it answers the
//! same way, so it holds one rather than answering again. What a field cannot
//! answer is up and down, and Home and End once a line has more than one row:
//! those are about where the text was wrapped, which is a question about a
//! face, a size and a width. So the rows are handed in, worked out by whoever
//! draws them, and [`TextArea::rows`] is the arithmetic that turns what a
//! wrap said into letters of the whole text.
//!
//! A step up or down keeps the caret as many letters into the row as it was,
//! or at the end of a row shorter than that. Counted in letters rather than
//! measured, so a narrow letter above a wide one lands a little off where the
//! eye expected; that is the price of the arithmetic having no screen in it.
//! Up from the first row is the front of the text and down from the last is
//! its end, which is where every text editor puts them.
//!
//! Caret and row positions count extended grapheme clusters. CRLF is one line
//! separator, and a wrapped row never cuts an accent or joined emoji in half.
//! Row bounds include their start and exclude their end.

use std::ops::Range;

use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_draw_surface::Keysym;

use crate::boundaries;
use crate::field::{TextField, Typed};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextArea {
  field: TextField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
  pub start: u32,
  pub end: u32,
  pub paragraph: u32,
}

pub type Wrap<'a> = &'a dyn Fn(u32, &str) -> Vec<String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
  Up,
  Down,
}

impl TextArea {
  pub fn holding(text: &str) -> Result<Self, Never> {
    let Ok(field) = TextField::holding(text);

    Ok(TextArea { field })
  }

  pub fn text(&self) -> Result<&str, Never> {
    self.field.text()
  }

  pub fn caret(&self) -> Result<u32, Never> {
    self.field.caret()
  }

  pub fn placed(&mut self, caret: u32) -> Result<Typed, Never> {
    self.field.moved_to(caret)
  }

  pub fn typed(&mut self, key: Keysym, rows: &[Row]) -> Result<Typed, Never> {
    match key {
      Keysym::Return | Keysym::KP_Enter => self.field.insert('\n'),
      Keysym::Up => self.stepped(rows, Way::Up, 1),
      Keysym::Down => self.stepped(rows, Way::Down, 1),
      Keysym::Home => {
        let Ok(row) = self.row(rows);

        self.field.moved_to(match row {
          Some(row_2) => row_2.start,
          None => 0,
        })
      }
      Keysym::End => {
        let Ok(row) = self.row(rows);
        let Ok(last) = self.field.letters();

        self.field.moved_to(match row {
          Some(row_3) => row_3.end,
          None => last,
        })
      }
      key_2 => self.field.typed(key_2),
    }
  }

  pub fn stepped(&mut self, rows: &[Row], way: Way, by: u32) -> Result<Typed, Never> {
    let Ok(caret) = self.field.caret();
    let Ok(at) = row_at(rows, caret);

    let (at_3, row) = match at {
      Some(at_2) => at_2,
      None => return Ok(Typed::None),
    };

    let into = caret.saturating_sub(row.start);
    let Ok(count) = fitted::<_, u32>(rows.len());
    let Ok(last) = self.field.letters();

    let landing = match way {
      Way::Up => at_3.checked_sub(by),
      Way::Down => at_3.checked_add(by).filter(|landing| *landing < count),
    };

    let Ok(landing_3) = console_core_number_conversion::index(match landing {
      Some(landing_2) => landing_2,
      None => {
        return self.field.moved_to(match way {
          Way::Up => 0,
          Way::Down => last,
        });
      }
    });

    let to = match rows.get(landing_3) {
      Some(to) => to,
      None => return Ok(Typed::None),
    };

    self
      .field
      .moved_to(to.start.saturating_add(into).min(to.end))
  }

  pub fn row(&self, rows: &[Row]) -> Result<Option<Row>, Never> {
    let Ok(caret) = self.field.caret();
    let Ok(at) = row_at(rows, caret);

    Ok(at.map(|(_, row)| row))
  }

  pub fn rows(&self, wrap: Wrap<'_>) -> Result<Vec<Row>, Never> {
    let Ok(text) = self.field.text();

    rows(text, wrap)
  }
}

pub fn between(text: &str, letters: Range<u32>) -> Result<&str, Never> {
  let Ok(start) = boundaries::byte(text, letters.start);
  let Ok(end) = boundaries::byte(text, letters.end);
  let Ok(start_2) = console_core_number_conversion::index(start);
  let Ok(end_2) = console_core_number_conversion::index(end);

  Ok(match text.get(start_2..end_2) {
    Some(said) => said,
    None => "",
  })
}

pub fn row_at(rows: &[Row], caret: u32) -> Result<Option<(u32, Row)>, Never> {
  Ok(
    (0_u32..)
      .zip(rows.iter().copied())
      .filter(|(_, row)| row.start <= caret)
      .last(),
  )
}

pub fn rows(text: &str, wrap: Wrap<'_>) -> Result<Vec<Row>, Never> {
  let mut found = Vec::new();
  let mut from = 0_u32;

  let trailing =
    match ::core::primitive::bool::then_some(text.is_empty() || text.ends_with('\n'), ()) {
      ::core::option::Option::Some(()) => Some(""),
      ::core::option::Option::None => None,
    };
  let paragraphs = text.split_inclusive('\n').chain(trailing);

  for (paragraph, line) in (0_u32..).zip(paragraphs) {
    let said = match line.strip_suffix('\n') {
      Some(content) => match content.strip_suffix('\r') {
        Some(content_2) => content_2,
        None => content,
      },
      None => line,
    };
    let Ok(long) = boundaries::count(said);
    let Ok(starts) = starts(said, &wrap(paragraph, said), long);
    let ends = starts.iter().copied().skip(1).chain(std::iter::once(long));

    for (start, end) in starts.iter().copied().zip(ends) {
      found.push(Row {
        start: from.saturating_add(start),
        end: from.saturating_add(end),
        paragraph,
      });
    }

    from = from.saturating_add(long).saturating_add(1);
  }

  Ok(found)
}

fn starts(said: &str, pieces: &[String], long: u32) -> Result<Vec<u32>, Never> {
  let mut found = vec![0_u32];
  let Ok(mut past) = fitted::<_, u32>(match pieces.first() {
    Some(first) => first.len(),
    None => 0,
  });

  for piece in pieces.iter().skip(1) {
    let Ok(from) = console_core_number_conversion::index(past);

    let rest = match said.get(from..) {
      Some(rest) => rest,
      None => "",
    };

    let Ok(after) = match rest.find(piece.as_str()) {
      Some(at) => fitted::<_, u32>(at).map(|at_2| past.saturating_add(at_2)),
      None => Ok(past),
    };

    let Ok(letters) = boundaries::at_or_after(said, after);
    let Ok(wide) = fitted::<_, u32>(piece.len());

    match ::core::primitive::bool::then_some(
      letters < long && found.last().is_some_and(|previous| *previous < letters),
      (),
    ) {
      ::core::option::Option::Some(()) => found.push(letters),
      ::core::option::Option::None => {}
    }

    past = after.saturating_add(wide);
  }

  Ok(found)
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_text_boundaries::{ClusterKind, UnicodeSegmentation};
  use std::num::{NonZero, NonZeroU8};

  const TWO: NonZeroU8 = NonZeroU8::MIN.saturating_add(1);
  const FOUR: NonZeroU8 = NonZeroU8::MIN.saturating_add(3);
  const TEN: NonZeroU8 = NonZeroU8::MIN.saturating_add(9);

  fn every(letters: NonZeroU8) -> Result<impl Fn(u32, &str) -> Vec<String>, Never> {
    Ok(move |_paragraph: u32, said: &str| -> Vec<String> {
      let Ok(graphemes) = said.graphemes(ClusterKind::Extended);
      let all: Vec<&str> = graphemes.collect();

      match ::core::primitive::bool::then_some(all.is_empty(), ()) {
        ::core::option::Option::Some(()) => vec![String::new()],
        ::core::option::Option::None => all
          .chunks(NonZero::from(letters).get())
          .map(|row| row.iter().copied().collect())
          .collect(),
      }
    })
  }

  fn area(text: &str, caret: u32) -> Result<TextArea, Never> {
    let Ok(mut area) = TextArea::holding(text);
    let Ok(_moved) = area.placed(caret);

    Ok(area)
  }

  #[test]
  fn return_starts_a_line_where_the_caret_is() {
    let Ok(mut area) = area("ab", 1);
    let Ok(wrap) = every(TEN);
    let Ok(rows) = area.rows(&wrap);

    assert_eq!(area.typed(Keysym::Return, &rows), Ok(Typed::Edited));
    assert_eq!(area.text(), Ok("a\nb"));
    assert_eq!(
      area.caret(),
      Ok(2),
      "the caret is at the front of the new line"
    );
  }

  #[test]
  fn a_row_is_where_a_wrap_broke_a_line_and_a_line_is_where_return_did() {
    let Ok(wrap) = every(FOUR);
    let Ok(rows) = rows("abcdef\n\nxy", &wrap);

    assert_eq!(
      rows,
      vec![
        Row {
          start: 0,
          end: 4,
          paragraph: 0
        },
        Row {
          start: 4,
          end: 6,
          paragraph: 0
        },
        Row {
          start: 7,
          end: 7,
          paragraph: 1
        },
        Row {
          start: 8,
          end: 10,
          paragraph: 2
        },
      ],
      "an empty line is a row the caret can stand on"
    );
  }

  #[test]
  fn a_row_a_wrap_trimmed_still_starts_where_its_words_do() {
    let wrap = |_paragraph: u32, _said: &str| vec!["one".to_string(), "two".to_string()];
    let Ok(rows) = rows("one two", &wrap);

    assert_eq!(
      rows,
      vec![
        Row {
          start: 0,
          end: 4,
          paragraph: 0
        },
        Row {
          start: 4,
          end: 7,
          paragraph: 0
        }
      ]
    );
  }

  #[test]
  fn up_and_down_keep_how_far_into_the_row_the_caret_was() {
    let Ok(mut area) = area("abcdef\nxy", 5);
    let Ok(wrap) = every(FOUR);
    let Ok(rows) = area.rows(&wrap);

    assert_eq!(area.typed(Keysym::Down, &rows), Ok(Typed::Moved));
    assert_eq!(
      area.caret(),
      Ok(8),
      "one letter into the row it left, one letter into the row below"
    );

    assert_eq!(area.typed(Keysym::Up, &rows), Ok(Typed::Moved));
    assert_eq!(area.caret(), Ok(5));

    assert_eq!(area.typed(Keysym::Up, &rows), Ok(Typed::Moved));
    assert_eq!(area.caret(), Ok(1));
  }

  #[test]
  fn a_shorter_row_holds_the_caret_at_its_end() {
    let Ok(mut area) = area("abcdef\nx", 3);
    let Ok(wrap) = every(TEN);
    let Ok(rows) = area.rows(&wrap);

    assert_eq!(area.typed(Keysym::Down, &rows), Ok(Typed::Moved));
    assert_eq!(area.caret(), Ok(8), "the end of `x`, not past it");
  }

  #[test]
  fn up_from_the_top_is_the_front_and_down_from_the_bottom_is_the_end() {
    let Ok(mut area) = area("abc\nde", 2);
    let Ok(wrap) = every(TEN);
    let Ok(rows) = area.rows(&wrap);

    assert_eq!(area.typed(Keysym::Up, &rows), Ok(Typed::Moved));
    assert_eq!(area.caret(), Ok(0));
    assert_eq!(area.typed(Keysym::Up, &rows), Ok(Typed::None));

    let Ok(_moved) = area.placed(4);

    assert_eq!(area.typed(Keysym::Down, &rows), Ok(Typed::Moved));
    assert_eq!(area.caret(), Ok(6));
  }

  #[test]
  fn home_and_end_are_the_row_and_not_the_whole_text() {
    let Ok(mut area) = area("abcdef\nxy", 5);
    let Ok(wrap) = every(FOUR);
    let Ok(rows) = area.rows(&wrap);

    assert_eq!(area.typed(Keysym::Home, &rows), Ok(Typed::Moved));
    assert_eq!(area.caret(), Ok(4));
    assert_eq!(area.typed(Keysym::End, &rows), Ok(Typed::Moved));
    assert_eq!(
      area.caret(),
      Ok(6),
      "the end of the line, before the line break"
    );
  }

  #[test]
  fn a_page_is_as_many_rows_as_it_holds() {
    let Ok(mut area) = area("a\nb\nc\nd\ne", 0);
    let Ok(wrap) = every(TEN);
    let Ok(rows) = area.rows(&wrap);

    assert_eq!(area.stepped(&rows, Way::Down, 3), Ok(Typed::Moved));
    assert_eq!(area.caret(), Ok(6));
    assert_eq!(area.stepped(&rows, Way::Down, 3), Ok(Typed::Moved));
    assert_eq!(area.caret(), Ok(9), "past the last row is the end");
  }

  #[test]
  fn a_row_says_the_letters_between_its_ends() {
    assert_eq!(
      between("café au lait", 2..6),
      Ok("fé a"),
      "counted in letters, cut in bytes"
    );
    assert_eq!(between("ab", 1..9), Ok("b"), "past the end is the end");
  }

  #[test]
  fn a_letter_typed_into_an_area_is_a_letter_typed_into_a_field() {
    let Ok(mut area) = area("", 0);
    let Ok(wrap) = every(TEN);
    let Ok(rows) = area.rows(&wrap);

    assert_eq!(area.typed(Keysym::a, &rows), Ok(Typed::Edited));
    assert_eq!(area.typed(Keysym::BackSpace, &rows), Ok(Typed::Edited));
    assert_eq!(area.text(), Ok(""));
  }

  #[test]
  fn wrapped_rows_and_vertical_movement_count_whole_graphemes() {
    let text = "a\u{301}👩‍👩‍👧‍👦中\néZ";
    let Ok(mut area) = area(text, 1);
    let Ok(wrap) = every(TWO);
    let Ok(rows) = area.rows(&wrap);

    assert_eq!(
      rows,
      vec![
        Row {
          start: 0,
          end: 2,
          paragraph: 0
        },
        Row {
          start: 2,
          end: 3,
          paragraph: 0
        },
        Row {
          start: 4,
          end: 6,
          paragraph: 1
        },
      ]
    );
    assert_eq!(between(text, 0..2), Ok("a\u{301}👩‍👩‍👧‍👦"));
    assert_eq!(area.typed(Keysym::Down, &rows), Ok(Typed::Moved));
    assert_eq!(area.caret(), Ok(3));
    assert_eq!(area.typed(Keysym::Down, &rows), Ok(Typed::Moved));
    assert_eq!(area.caret(), Ok(5));
    assert_eq!(area.typed(Keysym::Home, &rows), Ok(Typed::Moved));
    assert_eq!(area.caret(), Ok(4));
  }

  #[test]
  fn windows_newlines_are_one_grapheme_between_rows() {
    let text = "e\u{301}\r\n👩‍👩‍👧‍👦\n";
    let Ok(area) = area(text, 0);
    let Ok(wrap) = every(TEN);
    let Ok(rows) = area.rows(&wrap);

    assert_eq!(
      rows,
      vec![
        Row {
          start: 0,
          end: 1,
          paragraph: 0
        },
        Row {
          start: 2,
          end: 3,
          paragraph: 1
        },
        Row {
          start: 4,
          end: 4,
          paragraph: 2
        },
      ]
    );
    assert_eq!(between(text, 0..1), Ok("e\u{301}"));
    assert_eq!(between(text, 2..3), Ok("👩‍👩‍👧‍👦"));
  }

  #[test]
  fn a_wrap_boundary_inside_a_grapheme_moves_after_the_cluster() {
    for pieces in [vec!["e", "\u{301}x"], vec!["e", "\u{301}", "x"]] {
      let wrap =
        |_paragraph: u32, _said: &str| pieces.iter().map(|piece| piece.to_string()).collect();
      let Ok(rows) = rows("e\u{301}x", &wrap);

      assert_eq!(
        rows,
        vec![
          Row {
            start: 0,
            end: 1,
            paragraph: 0
          },
          Row {
            start: 1,
            end: 2,
            paragraph: 0
          }
        ]
      );
      let said = rows.first().map(|row| {
        let Ok(said) = between("e\u{301}x", row.start..row.end);

        said
      });
      assert_eq!(said, Some("e\u{301}"));
    }
  }

  #[test]
  fn wrapped_fragments_of_one_grapheme_do_not_add_an_empty_row() {
    let wrap = |_paragraph: u32, _said: &str| vec!["e".to_string(), "\u{301}".to_string()];
    let Ok(rows) = rows("e\u{301}", &wrap);

    assert_eq!(
      rows,
      vec![Row {
        start: 0,
        end: 1,
        paragraph: 0
      }]
    );
  }
}
