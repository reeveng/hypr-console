use alloc::vec::Vec;
use unicode_bidi::{BidiClass, BidiInfo, ParagraphInfo, bidi_class};

#[derive(Debug)]
pub struct BidiParagraphs<'text> {
  text: &'text str,
  info: alloc::vec::IntoIter<ParagraphInfo>,
}

impl<'text> BidiParagraphs<'text> {
  pub fn new(text: &'text str) -> Result<Self, console_core_never::Never> {
    let simple = text.is_ascii()
      && !text
        .chars()
        .any(|c| c.is_ascii_control() && c != '\n' && c != '\r' && c != '\t');
    let paragraphs = match simple.then_some(()) {
      Some(()) => {
        let Ok(paragraphs) = ascii_paragraphs(text);
        paragraphs
      }
      None => {
        let Ok(info) = BidiInfo::new(text, None);
        let Ok(paragraphs) = info.into_paragraphs();
        paragraphs
      }
    };

    Ok(Self {
      text,
      info: paragraphs.into_iter(),
    })
  }
}

fn ascii_paragraphs(text: &str) -> Result<Vec<ParagraphInfo>, console_core_never::Never> {
  let mut paragraphs = Vec::new();
  let mut start = 0;

  for (position, _) in text.match_indices('\n') {
    let Ok(end) = console_core_number_conversion::fitted::<_, u32>(position);
    paragraphs.push(ParagraphInfo {
      range: start..end,
      level: unicode_bidi::Level::LEFT_TO_RIGHT,
    });
    start = end.saturating_add(1);
  }

  let Ok(length) = console_core_number_conversion::fitted::<_, u32>(text.len());

  match (start < length).then_some(()) {
    Some(()) => paragraphs.push(ParagraphInfo {
      range: start..length,
      level: unicode_bidi::Level::LEFT_TO_RIGHT,
    }),
    None => {}
  }

  Ok(paragraphs)
}

impl<'text> Iterator for BidiParagraphs<'text> {
  type Item = &'text str;

  fn next(&mut self) -> Option<Self::Item> {
    let para = self.info.next()?;
    let Ok(indices) = console_core_number_conversion::indices(para.range);
    let paragraph = self.text.get(indices)?;

    let mut char_indices = paragraph.char_indices();
    char_indices
      .next_back()
      .and_then(|(i, c)| (bidi_class(c) == Ok(BidiClass::B)).then_some(i))
      .map_or(Some(paragraph), |i| {
        Some(match (paragraph).get(0..i) {
          Some(value) => value,
          None => return None,
        })
      })
  }
}
