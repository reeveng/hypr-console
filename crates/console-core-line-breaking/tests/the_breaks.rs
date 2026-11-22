use console_core_never::Never;
use console_core_number_conversion::fitted;

use console_core_line_breaking::BreakOpportunity::{Allowed, Mandatory};
use console_core_line_breaking::{LineBreaks, linebreaks_iter};

type Characters = std::vec::IntoIter<(u32, char)>;

fn characters(text: &str) -> Result<Vec<(u32, char)>, Never> {
  Ok(
    text
      .char_indices()
      .map(|(at, character)| {
        let Ok(position) = fitted::<_, u32>(at);

        (position, character)
      })
      .collect(),
  )
}

fn breaks(text: &str) -> Result<LineBreaks<Characters, u32>, Never> {
  let Ok(held) = characters(text);
  let Ok(end) = fitted::<_, u32>(text.len());

  linebreaks_iter(held.into_iter(), end)
}

#[test]
fn a_space_lets_a_line_end_after_it_and_the_end_of_the_text_always_does() {
  let Ok(first) = breaks("a b \nc");
  let Ok(second) = breaks("Hello world!");

  assert!(first.eq([(2, Allowed), (5, Mandatory), (6, Mandatory)]));
  assert!(second.eq([(6, Allowed), (12, Mandatory)]));
}

#[test]
fn context_before_a_character_decides_whether_it_may_start_a_line() {
  let Ok(accent) = breaks("a\u{0301}b");
  let Ok(flags) = breaks("🇦🇧🇨🇩🇪");
  let Ok(family) = breaks("👩\u{200d}👩\u{200d}👧");
  let Ok(number) = breaks("123,456.789");
  let Ok(lines) = breaks("a\r\nb");
  let Ok(hangul) = breaks("\u{1100}\u{1161}\u{11A8}\u{1102}\u{1161}");
  let Ok(hyphen) = breaks("alpha\u{00ad}beta");

  assert!(accent.eq([(4, Mandatory)]));
  assert!(flags.eq([(8, Allowed), (16, Allowed), (20, Mandatory)]));
  assert!(family.eq([(18, Mandatory)]));
  assert!(number.eq([(11, Mandatory)]));
  assert!(lines.eq([(3, Mandatory), (4, Mandatory)]));
  assert!(hangul.eq([(9, Allowed), (15, Mandatory)]));
  assert!(hyphen.eq([(7, Allowed), (11, Mandatory)]));
}

#[test]
fn the_ends_of_the_unicode_range_are_classified() {
  let Ok(last) = breaks("a\u{10FFFF}b");
  let Ok(tag) = breaks("a\u{E01EF}b");
  let Ok(null) = breaks("\u{0}\u{0}");
  let Ok(coptic) = breaks("\u{2CF3}\u{2CF3}");

  assert!(last.eq([(6, Mandatory)]));
  assert!(tag.eq([(6, Mandatory)]));
  assert!(null.eq([(2, Mandatory)]));
  assert!(coptic.eq([(6, Mandatory)]));
}

#[test]
fn a_break_iterator_can_be_cloned_part_way() {
  let Ok(mut found) = breaks("a b c");

  assert_eq!(found.next(), Some((2, Allowed)));
  assert!(found.clone().eq(found));
}

struct Cancellable {
  characters: std::iter::Take<Characters>,
  progress: InputProgress,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum InputProgress {
  Reading,
  Interrupted,
}

impl Iterator for Cancellable {
  type Item = (u32, char);

  fn next(&mut self) -> Option<Self::Item> {
    let next = self.characters.next();

    self.progress = match next {
      Some(_) => InputProgress::Reading,
      None => InputProgress::Interrupted,
    };

    next
  }
}

#[test]
fn a_caller_that_stops_early_can_see_it_after_the_final_break() {
  let text = "an_unbroken_segment";
  let Ok(held) = characters(text);
  let Ok(end) = fitted::<_, u32>(text.len());
  let source = Cancellable {
    characters: held.into_iter().take(3),
    progress: InputProgress::Reading,
  };
  let Ok(mut found) = linebreaks_iter(source, end);

  assert_eq!(found.next(), Some((end, Mandatory)));
  assert_eq!(
    found.get_mut().map(|held_source| held_source.progress),
    Ok(InputProgress::Interrupted)
  );
  assert_eq!(found.next(), None);
}
