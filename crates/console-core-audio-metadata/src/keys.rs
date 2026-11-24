//! The things a tag says that anybody here asks for, and the word each
//! format spells each one with.
//!
//! Every format names a field its own way: ID3v2 by a frame id, a Vorbis
//! comment and an APE item by a word, an iTunes atom by four bytes or by a
//! domain and a name. Each table below is the one lofty kept for that format,
//! cut to the keys this desktop reads, and every one is matched without
//! regard to ASCII case, as lofty matched them. A word that names nothing here
//! is not an item, so a tag only ever holds what somebody reads. ID3v2 is the
//! one format written, so it is the one table read in both directions, and a
//! key with no frame of its own -- a year, a description, a performer -- is
//! left out of what is written rather than spelled as something else.

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemKey {
  TrackTitle,
  TrackArtist,
  AlbumArtist,
  AlbumTitle,
  TrackNumber,
  DiscNumber,
  MusicBrainzAlbumId,
  TrackSubtitle,
  RecordingDate,
  Year,
  Genre,
  Comment,
  Description,
  Composer,
  Performer,
  Publisher,
  Label,
  CopyrightMessage,
}

pub(crate) fn named(spellings: &[(&str, ItemKey)], word: &str) -> Result<Option<ItemKey>, Never> {
  Ok(
    spellings
      .iter()
      .find(|(spelling, _key)| spelling.eq_ignore_ascii_case(word))
      .map(|(_spelling, key)| *key),
  )
}

pub(crate) const APE: &[(&str, ItemKey)] = &[
  ("ALBUM", ItemKey::AlbumTitle),
  ("TITLE", ItemKey::TrackTitle),
  ("SUBTITLE", ItemKey::TrackSubtitle),
  ("ALBUM ARTIST", ItemKey::AlbumArtist),
  ("ALBUMARTIST", ItemKey::AlbumArtist),
  ("TRACK", ItemKey::TrackNumber),
  ("DISC", ItemKey::DiscNumber),
  ("MUSICBRAINZ_ALBUMID", ItemKey::MusicBrainzAlbumId),
  ("ARTIST", ItemKey::TrackArtist),
  ("COMPOSER", ItemKey::Composer),
  ("PERFORMER", ItemKey::Performer),
  ("LABEL", ItemKey::Label),
  ("YEAR", ItemKey::RecordingDate),
  ("GENRE", ItemKey::Genre),
  ("COPYRIGHT", ItemKey::CopyrightMessage),
  ("COMMENT", ItemKey::Comment),
];

pub(crate) const ID3V2: &[(&str, ItemKey)] = &[
  ("TALB", ItemKey::AlbumTitle),
  ("TIT2", ItemKey::TrackTitle),
  ("TIT3", ItemKey::TrackSubtitle),
  ("TRCK", ItemKey::TrackNumber),
  ("TPOS", ItemKey::DiscNumber),
  ("MUSICBRAINZ ALBUM ID", ItemKey::MusicBrainzAlbumId),
  ("TPE2", ItemKey::AlbumArtist),
  ("ALBUMARTIST", ItemKey::AlbumArtist),
  ("ALBUM ARTIST", ItemKey::AlbumArtist),
  ("TPE1", ItemKey::TrackArtist),
  ("TCOM", ItemKey::Composer),
  ("TPUB", ItemKey::Publisher),
  ("TDRC", ItemKey::RecordingDate),
  ("TCON", ItemKey::Genre),
  ("TCOP", ItemKey::CopyrightMessage),
  ("COMM", ItemKey::Comment),
];

pub(crate) const ILST: &[(&str, ItemKey)] = &[
  ("\u{a9}ALB", ItemKey::AlbumTitle),
  ("\u{a9}NAM", ItemKey::TrackTitle),
  ("trkn", ItemKey::TrackNumber),
  ("disk", ItemKey::DiscNumber),
  (
    "----:COM.APPLE.ITUNES:MUSICBRAINZ ALBUM ID",
    ItemKey::MusicBrainzAlbumId,
  ),
  ("----:COM.APPLE.ITUNES:SUBTITLE", ItemKey::TrackSubtitle),
  ("AART", ItemKey::AlbumArtist),
  ("\u{a9}ART", ItemKey::TrackArtist),
  ("\u{a9}WRT", ItemKey::Composer),
  ("----:COM.APPLE.ITUNES:LABEL", ItemKey::Label),
  ("\u{a9}DAY", ItemKey::RecordingDate),
  ("\u{a9}GEN", ItemKey::Genre),
  ("CPRT", ItemKey::CopyrightMessage),
  ("\u{a9}CMT", ItemKey::Comment),
  ("DESC", ItemKey::Description),
];

pub(crate) const RIFF_INFO: &[(&str, ItemKey)] = &[
  ("IPRD", ItemKey::AlbumTitle),
  ("INAM", ItemKey::TrackTitle),
  ("IART", ItemKey::TrackArtist),
  ("IMUS", ItemKey::Composer),
  ("ICRD", ItemKey::RecordingDate),
  ("IGNR", ItemKey::Genre),
  ("ICOP", ItemKey::CopyrightMessage),
  ("ICMT", ItemKey::Comment),
];

pub(crate) const VORBIS_COMMENTS: &[(&str, ItemKey)] = &[
  ("ALBUM", ItemKey::AlbumTitle),
  ("TRACKNUMBER", ItemKey::TrackNumber),
  ("DISCNUMBER", ItemKey::DiscNumber),
  ("MUSICBRAINZ_ALBUMID", ItemKey::MusicBrainzAlbumId),
  ("TITLE", ItemKey::TrackTitle),
  ("SUBTITLE", ItemKey::TrackSubtitle),
  ("ALBUMARTIST", ItemKey::AlbumArtist),
  ("ALBUM ARTIST", ItemKey::AlbumArtist),
  ("ARTIST", ItemKey::TrackArtist),
  ("COMPOSER", ItemKey::Composer),
  ("PERFORMER", ItemKey::Performer),
  ("PUBLISHER", ItemKey::Publisher),
  ("LABEL", ItemKey::Label),
  ("ORGANIZATION", ItemKey::Label),
  ("DATE", ItemKey::RecordingDate),
  ("YEAR", ItemKey::Year),
  ("GENRE", ItemKey::Genre),
  ("COPYRIGHT", ItemKey::CopyrightMessage),
  ("COMMENT", ItemKey::Comment),
  ("DESCRIPTION", ItemKey::Description),
];

pub(crate) fn id3v2_frame(key: ItemKey) -> Result<Option<&'static str>, Never> {
  Ok(match key {
    ItemKey::AlbumTitle => Some("TALB"),
    ItemKey::TrackNumber => Some("TRCK"),
    ItemKey::DiscNumber => Some("TPOS"),
    ItemKey::TrackTitle => Some("TIT2"),
    ItemKey::TrackSubtitle => Some("TIT3"),
    ItemKey::AlbumArtist => Some("TPE2"),
    ItemKey::TrackArtist => Some("TPE1"),
    ItemKey::Composer => Some("TCOM"),
    ItemKey::Publisher | ItemKey::Label => Some("TPUB"),
    ItemKey::RecordingDate => Some("TDRC"),
    ItemKey::Genre => Some("TCON"),
    ItemKey::CopyrightMessage => Some("TCOP"),
    ItemKey::Comment => Some("COMM"),
    ItemKey::Year | ItemKey::Description | ItemKey::Performer | ItemKey::MusicBrainzAlbumId => {
      None
    }
  })
}
