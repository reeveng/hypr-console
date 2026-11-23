//! What a caller asks of a text that the annex's cases do not: several paragraphs at once, a line
//! that is only part of one, a level asked for at the very top, and a line that is not one.
//!
//! The conformance files hold one paragraph to a case and one line to a paragraph, and never ask
//! past level 1, so what happens between paragraphs, at the end of a line and at level 126 is
//! held here. A paragraph asked for at 126 stays there whatever is in it, and a line all at 126 is
//! shown in the order it was written; unicode-bidi stopped the program at both.

use console_core_never::Never;
use console_core_text_direction::{BidiInfo, Level, LevelFault, LineFault};

enum Error {
  Level(LevelFault),
  Line(LineFault),
  NoParagraph,
}

impl std::fmt::Debug for Error {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Error::Level(fault) => write!(to, "{fault}"),
      Error::Line(fault) => write!(to, "{fault}"),
      Error::NoParagraph => write!(to, "the text came out with no paragraph"),
    }
  }
}

fn numbers(levels: &[Level]) -> Result<Vec<u8>, Never> {
  levels.iter().map(|level| level.number()).collect()
}

fn levels(numbers: &[u8]) -> Result<Vec<Level>, Error> {
  numbers
    .iter()
    .map(|&number| Level::new(number))
    .collect::<Result<Vec<Level>, LevelFault>>()
    .map_err(Error::Level)
}

#[test]
fn a_text_is_split_into_paragraphs_each_with_its_own_direction() {
  let Ok(info) = BidiInfo::new("abc\nאבג", None);
  let Ok(paragraphs) = info.paragraphs();
  let found: Vec<_> = paragraphs
    .iter()
    .map(|paragraph| (paragraph.range.clone(), paragraph.level))
    .collect();

  assert_eq!(
    found,
    vec![(0..4, Level::LEFT_TO_RIGHT), (4..10, Level::RIGHT_TO_LEFT)]
  );
}

#[test]
fn a_paragraph_takes_its_direction_from_what_is_outside_its_isolates() {
  let Ok(isolated_hebrew) = BidiInfo::new("\u{2068}אב\u{2069}abc", None);
  let Ok(isolated_latin) = BidiInfo::new("\u{2068}abc\u{2069}אב", None);
  let first = |info: &BidiInfo<'_>| {
    let Ok(paragraphs) = info.paragraphs();

    paragraphs.first().map(|paragraph| paragraph.level)
  };

  assert_eq!(first(&isolated_hebrew), Some(Level::LEFT_TO_RIGHT));
  assert_eq!(first(&isolated_latin), Some(Level::RIGHT_TO_LEFT));
}

#[test]
fn a_right_to_left_paragraph_shows_its_runs_from_the_end() -> Result<(), Error> {
  let Ok(info) = BidiInfo::new("אב abc גד", None);
  let Ok(paragraphs) = info.paragraphs();
  let paragraph = paragraphs.first().ok_or(Error::NoParagraph)?;
  let (levels, runs) = info
    .visual_runs(paragraph, paragraph.range.clone())
    .map_err(Error::Line)?;

  assert_eq!(
    numbers(&levels),
    Ok(vec![1, 1, 1, 1, 1, 2, 2, 2, 1, 1, 1, 1, 1])
  );
  assert_eq!(runs, vec![8..13, 5..8, 0..5]);

  Ok(())
}

#[test]
fn whitespace_before_a_tab_is_put_back_at_the_paragraph_level_on_its_line() -> Result<(), Error> {
  let Ok(info) = BidiInfo::new("אב \tגד", Some(Level::LEFT_TO_RIGHT));
  let Ok(paragraphs) = info.paragraphs();
  let paragraph = paragraphs.first().ok_or(Error::NoParagraph)?;
  let (levels, _runs) = info
    .visual_runs(paragraph, paragraph.range.clone())
    .map_err(Error::Line)?;
  let Ok(original) = info.levels();

  assert_eq!(numbers(original), Ok(vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1]));
  assert_eq!(numbers(&levels), Ok(vec![1, 1, 1, 1, 0, 0, 1, 1, 1, 1]));

  Ok(())
}

#[test]
fn a_line_that_is_not_whole_characters_of_the_text_is_refused() -> Result<(), Error> {
  let Ok(info) = BidiInfo::new("אב", None);
  let Ok(paragraphs) = info.paragraphs();
  let paragraph = paragraphs.first().ok_or(Error::NoParagraph)?;

  assert_eq!(
    info.visual_runs(paragraph, 0..1),
    Err(LineFault::NotWholeCharacters {
      line: 0..1,
      length: 4
    })
  );
  assert_eq!(
    info.visual_runs(paragraph, 2..9),
    Err(LineFault::NotWholeCharacters {
      line: 2..9,
      length: 4
    })
  );

  Ok(())
}

#[test]
fn levels_are_put_in_the_order_they_are_shown() -> Result<(), Error> {
  let flat = levels(&[0, 0, 0, 0])?;
  let nested = levels(&[0, 0, 0, 1, 1, 1, 2, 2])?;

  assert_eq!(BidiInfo::reorder_visual(&flat), Ok(vec![0, 1, 2, 3]));
  assert_eq!(
    BidiInfo::reorder_visual(&nested),
    Ok(vec![0, 1, 2, 6, 7, 5, 4, 3])
  );
  assert_eq!(BidiInfo::reorder_visual(&[]), Ok(vec![]));

  Ok(())
}

#[test]
fn a_level_is_at_most_126() {
  assert_eq!(Level::new(126).map(|level| level.number()), Ok(Ok(126)));
  assert_eq!(Level::new(127), Err(LevelFault::TooDeep { number: 127 }));
}

#[test]
fn a_paragraph_asked_for_at_the_deepest_level_stays_there() -> Result<(), Error> {
  let deepest = Level::new(126).map_err(Error::Level)?;
  let Ok(info) = BidiInfo::new("aא1", Some(deepest));
  let Ok(paragraphs) = info.paragraphs();
  let paragraph = paragraphs.first().ok_or(Error::NoParagraph)?;
  let (levels, runs) = info
    .visual_runs(paragraph, paragraph.range.clone())
    .map_err(Error::Line)?;
  let Ok(original) = info.levels();

  assert_eq!(numbers(original), Ok(vec![126, 126, 126, 126]));
  assert_eq!(numbers(&levels), Ok(vec![126, 126, 126, 126]));
  assert_eq!(runs, vec![0..4]);
  assert_eq!(
    BidiInfo::reorder_visual(&[deepest, deepest]),
    Ok(vec![0, 1])
  );

  Ok(())
}
