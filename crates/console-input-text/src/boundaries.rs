//! Conversion between grapheme positions and the byte offsets used by strings.
//!
//! Edited byte offsets advance to the first following grapheme boundary.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_text_boundaries::{ClusterKind, UnicodeSegmentation};

pub fn count(text: &str) -> Result<u32, Never> {
  let Ok(graphemes) = text.graphemes(ClusterKind::Extended);

  fitted(graphemes.count())
}

pub(crate) fn byte(text: &str, grapheme: u32) -> Result<u32, Never> {
  let Ok(grapheme_2) = index(grapheme);
  let Ok(mut graphemes) = text.grapheme_indices(ClusterKind::Extended);

  match graphemes.nth(grapheme_2) {
    Some((at, _)) => fitted(at),
    None => fitted(text.len()),
  }
}

pub(crate) fn at_or_after(text: &str, byte: u32) -> Result<u32, Never> {
  let Ok(graphemes) = text.grapheme_indices(ClusterKind::Extended);

  fitted(
    graphemes
      .take_while(|&(start, _)| start < u64::from(byte))
      .count(),
  )
}
