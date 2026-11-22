//! What an iTunes free-form atom is called when it is one of the names other
//! taggers agreed on, so a name that is known comes out as a standard key and
//! one that is not stays under the name it was written with.

use console_core_audio_decoding_interfaces::meta::StandardTagKey;
use console_core_never::Never;

const KEYS: &[(&str, StandardTagKey)] = &[
  ("com.apple.iTunes:ARTISTS", StandardTagKey::Artist),
  ("com.apple.iTunes:ASIN", StandardTagKey::IdentAsin),
  ("com.apple.iTunes:BARCODE", StandardTagKey::IdentBarcode),
  (
    "com.apple.iTunes:CATALOGNUMBER",
    StandardTagKey::IdentCatalogNumber,
  ),
  ("com.apple.iTunes:CONDUCTOR", StandardTagKey::Conductor),
  (
    "com.apple.iTunes:DISCSUBTITLE",
    StandardTagKey::DiscSubtitle,
  ),
  ("com.apple.iTunes:DJMIXER", StandardTagKey::MixDj),
  ("com.apple.iTunes:ENGINEER", StandardTagKey::Engineer),
  ("com.apple.iTunes:ISRC", StandardTagKey::IdentIsrc),
  ("com.apple.iTunes:LABEL", StandardTagKey::Label),
  ("com.apple.iTunes:LANGUAGE", StandardTagKey::Language),
  ("com.apple.iTunes:LICENSE", StandardTagKey::License),
  ("com.apple.iTunes:LYRICIST", StandardTagKey::Lyricist),
  ("com.apple.iTunes:MEDIA", StandardTagKey::MediaFormat),
  ("com.apple.iTunes:MIXER", StandardTagKey::MixEngineer),
  ("com.apple.iTunes:MOOD", StandardTagKey::Mood),
  (
    "com.apple.iTunes:MusicBrainz Album Artist Id",
    StandardTagKey::MusicBrainzAlbumArtistId,
  ),
  (
    "com.apple.iTunes:MusicBrainz Album Id",
    StandardTagKey::MusicBrainzAlbumId,
  ),
  (
    "com.apple.iTunes:MusicBrainz Album Release Country",
    StandardTagKey::ReleaseCountry,
  ),
  (
    "com.apple.iTunes:MusicBrainz Album Status",
    StandardTagKey::MusicBrainzReleaseStatus,
  ),
  (
    "com.apple.iTunes:MusicBrainz Album Type",
    StandardTagKey::MusicBrainzReleaseType,
  ),
  (
    "com.apple.iTunes:MusicBrainz Artist Id",
    StandardTagKey::MusicBrainzArtistId,
  ),
  (
    "com.apple.iTunes:MusicBrainz Release Group Id",
    StandardTagKey::MusicBrainzReleaseGroupId,
  ),
  (
    "com.apple.iTunes:MusicBrainz Release Track Id",
    StandardTagKey::MusicBrainzReleaseTrackId,
  ),
  (
    "com.apple.iTunes:MusicBrainz Track Id",
    StandardTagKey::MusicBrainzTrackId,
  ),
  (
    "com.apple.iTunes:MusicBrainz Work Id",
    StandardTagKey::MusicBrainzWorkId,
  ),
  (
    "com.apple.iTunes:originaldate",
    StandardTagKey::OriginalDate,
  ),
  ("com.apple.iTunes:PRODUCER", StandardTagKey::Producer),
  ("com.apple.iTunes:REMIXER", StandardTagKey::Remixer),
  ("com.apple.iTunes:SCRIPT", StandardTagKey::Script),
  ("com.apple.iTunes:SUBTITLE", StandardTagKey::TrackSubtitle),
];

pub fn std_key_from_tag(key: &str) -> Result<Option<StandardTagKey>, Never> {
  Ok(
    KEYS
      .iter()
      .find(|(name, _)| *name == key)
      .map(|(_, standard)| *standard),
  )
}
