//! Every retained Unicode 17 grapheme vector and the byte offsets of mixed-width clusters.

use console_core_never::Never;
use console_core_text_boundaries::{ClusterKind, UnicodeSegmentation};

#[rustfmt::skip]
mod graphemes;

#[test]
fn unicode_seventeen_clusters_match_the_upstream_vectors() -> Result<(), Never> {
  for &(text, expected) in graphemes::TEST_SAME {
    for kind in [ClusterKind::Extended, ClusterKind::Legacy] {
      let Ok(clusters) = text.graphemes(kind);

      assert!(clusters.eq(expected.iter().copied()), "{text:?} {kind:?}");
    }
  }

  for &(text, extended, legacy) in graphemes::TEST_DIFF {
    for (kind, expected) in [
      (ClusterKind::Extended, extended),
      (ClusterKind::Legacy, legacy),
    ] {
      let Ok(clusters) = text.graphemes(kind);

      assert!(clusters.eq(expected.iter().copied()), "{text:?} {kind:?}");
    }
  }

  Ok(())
}

#[test]
fn clusters_retain_their_byte_positions_and_empty_text_is_exhausted() -> Result<(), Never> {
  let text = "a̐éö̲\r\n";
  let Ok(clusters) = text.grapheme_indices(ClusterKind::Extended);
  let found: Vec<_> = clusters.collect();

  assert_eq!(found, [(0, "a̐"), (3, "é"), (6, "ö̲"), (11, "\r\n")]);

  let Ok(mut empty_clusters) = "".graphemes(ClusterKind::Extended);

  assert_eq!(empty_clusters.next(), None);
  assert_eq!(empty_clusters.next(), None);
  assert_eq!(empty_clusters.as_str(), Ok(""));

  Ok(())
}

#[test]
fn joining_context_stays_inside_the_correct_cluster() -> Result<(), Never> {
  for (text, expected) in [
    (
      "\u{20}\u{600}\u{600}\u{20}",
      ["\u{20}", "\u{600}\u{600}\u{20}"].as_slice(),
    ),
    (
      "\u{600}\u{20}\u{20}",
      ["\u{600}\u{20}", "\u{20}"].as_slice(),
    ),
    ("👨‍👧‍👦", ["👨‍👧‍👦"].as_slice()),
    ("🤸🏾🏾", ["🤸🏾🏾"].as_slice()),
    ("🇧🇪🇫🇷🇩", ["🇧🇪", "🇫🇷", "🇩"].as_slice()),
  ] {
    let Ok(clusters) = text.graphemes(ClusterKind::Extended);

    assert!(clusters.eq(expected.iter().copied()), "{text:?}");
  }

  Ok(())
}
