//! A RIFF INFO chunk read as a tag: the four characters it was written under
//! stay its key, and the same four characters in any case name the standard
//! key when they are one of the known ones.

use console_core_audio_decoding_interfaces::meta::{StandardTagKey, Tag, Value};
use console_core_never::Never;

use crate::utf8_with_replacement;

const KEYS: &[(&str, StandardTagKey)] = &[
  ("ages", StandardTagKey::Rating),
  ("cmnt", StandardTagKey::Comment),
  ("comm", StandardTagKey::Comment),
  ("dtim", StandardTagKey::OriginalDate),
  ("genr", StandardTagKey::Genre),
  ("iart", StandardTagKey::Artist),
  ("icmt", StandardTagKey::Comment),
  ("icop", StandardTagKey::Copyright),
  ("icrd", StandardTagKey::Date),
  ("idit", StandardTagKey::OriginalDate),
  ("ienc", StandardTagKey::EncodedBy),
  ("ieng", StandardTagKey::Engineer),
  ("ifrm", StandardTagKey::TrackTotal),
  ("ignr", StandardTagKey::Genre),
  ("ilng", StandardTagKey::Language),
  ("imus", StandardTagKey::Composer),
  ("inam", StandardTagKey::TrackTitle),
  ("iprd", StandardTagKey::Album),
  ("ipro", StandardTagKey::Producer),
  ("iprt", StandardTagKey::TrackNumber),
  ("irtd", StandardTagKey::Rating),
  ("isft", StandardTagKey::Encoder),
  ("isgn", StandardTagKey::Genre),
  ("isrf", StandardTagKey::MediaFormat),
  ("itch", StandardTagKey::EncodedBy),
  ("iwri", StandardTagKey::Writer),
  ("lang", StandardTagKey::Language),
  ("prt1", StandardTagKey::TrackNumber),
  ("prt2", StandardTagKey::TrackTotal),
  ("titl", StandardTagKey::TrackTitle),
  ("torg", StandardTagKey::Label),
  ("trck", StandardTagKey::TrackNumber),
  ("tver", StandardTagKey::Version),
  ("year", StandardTagKey::Date),
];

pub fn parse(tag: [u8; 4], buf: &[u8]) -> Result<Tag, Never> {
  let Ok(key) = utf8_with_replacement(&tag);
  let Ok(value) = utf8_with_replacement(buf);
  let lowered = key.to_lowercase();
  let standard = KEYS
    .iter()
    .find(|(name, _)| *name == lowered)
    .map(|(_, standard)| *standard);

  let Ok(value_1) = Tag::new(standard, &key, Value::from(value));

  Ok(value_1)
}
