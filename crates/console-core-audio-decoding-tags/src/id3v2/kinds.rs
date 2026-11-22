//! Which frame ids are read, as what, and under which standard key.
//!
//! An ID3v2.2 id is three characters and means the four-character id it is
//! listed against here; the frames nothing reads are not in either list, and
//! an id that is in neither is skipped by its size rather than refused.

use console_core_audio_decoding_interfaces::meta::StandardTagKey;
use console_core_never::Never;

#[derive(Clone, Copy)]
pub(super) enum Kind {
  CdIdentifier,
  Comment,
  PlayCount,
  Picture,
  Popularimeter,
  Private,
  Text,
  Url,
  UserText,
  UserUrl,
}

type Found = (Kind, Option<StandardTagKey>);

const LEGACY: &[(&[u8; 3], &[u8; 4])] = &[
  (b"BUF", b"RBUF"),
  (b"CNT", b"PCNT"),
  (b"COM", b"COMM"),
  (b"CRA", b"AENC"),
  (b"EQU", b"EQUA"),
  (b"ETC", b"ETCO"),
  (b"GEO", b"GEOB"),
  (b"IPL", b"IPLS"),
  (b"LNK", b"LINK"),
  (b"MCI", b"MCDI"),
  (b"MLL", b"MLLT"),
  (b"PCS", b"PCST"),
  (b"PIC", b"APIC"),
  (b"POP", b"POPM"),
  (b"REV", b"RVRB"),
  (b"RVA", b"RVAD"),
  (b"SLT", b"SYLT"),
  (b"STC", b"SYTC"),
  (b"TAL", b"TALB"),
  (b"TBP", b"TBPM"),
  (b"TCM", b"TCOM"),
  (b"TCO", b"TCON"),
  (b"TCR", b"TCOP"),
  (b"TDA", b"TDAT"),
  (b"TDY", b"TDLY"),
  (b"TEN", b"TENC"),
  (b"TFT", b"TFLT"),
  (b"TIM", b"TIME"),
  (b"TKE", b"TKEY"),
  (b"TLA", b"TLAN"),
  (b"TLE", b"TLEN"),
  (b"TMT", b"TMED"),
  (b"TOA", b"TOPE"),
  (b"TOF", b"TOFN"),
  (b"TOL", b"TOLY"),
  (b"TOR", b"TORY"),
  (b"TOT", b"TOAL"),
  (b"TP1", b"TPE1"),
  (b"TP2", b"TPE2"),
  (b"TP3", b"TPE3"),
  (b"TP4", b"TPE4"),
  (b"TPA", b"TPOS"),
  (b"TPB", b"TPUB"),
  (b"TRC", b"TSRC"),
  (b"TRD", b"TRDA"),
  (b"TRK", b"TRCK"),
  (b"TS2", b"TSO2"),
  (b"TSA", b"TSOA"),
  (b"TSC", b"TSOC"),
  (b"TSI", b"TSIZ"),
  (b"TSP", b"TSOP"),
  (b"TSS", b"TSSE"),
  (b"TST", b"TSOT"),
  (b"TT1", b"TIT1"),
  (b"TT2", b"TIT2"),
  (b"TT3", b"TIT3"),
  (b"TXT", b"TEXT"),
  (b"TXX", b"TXXX"),
  (b"TYE", b"TYER"),
  (b"UFI", b"UFID"),
  (b"ULT", b"USLT"),
  (b"WAF", b"WOAF"),
  (b"WAR", b"WOAR"),
  (b"WAS", b"WOAS"),
  (b"WCM", b"WCOM"),
  (b"WCP", b"WCOP"),
  (b"WPB", b"WPUB"),
  (b"WXX", b"WXXX"),
];

const FRAMES: &[(&[u8; 4], Kind, Option<StandardTagKey>)] = &[
  (b"APIC", Kind::Picture, None),
  (b"COMM", Kind::Comment, Some(StandardTagKey::Comment)),
  (b"IPLS", Kind::Text, None),
  (b"MCDI", Kind::CdIdentifier, None),
  (b"PCNT", Kind::PlayCount, None),
  (b"POPM", Kind::Popularimeter, Some(StandardTagKey::Rating)),
  (b"PRIV", Kind::Private, None),
  (b"TALB", Kind::Text, Some(StandardTagKey::Album)),
  (b"TBPM", Kind::Text, Some(StandardTagKey::Bpm)),
  (b"TCOM", Kind::Text, Some(StandardTagKey::Composer)),
  (b"TCON", Kind::Text, Some(StandardTagKey::Genre)),
  (b"TCOP", Kind::Text, Some(StandardTagKey::Copyright)),
  (b"TDAT", Kind::Text, Some(StandardTagKey::Date)),
  (b"TDEN", Kind::Text, Some(StandardTagKey::EncodingDate)),
  (b"TDLY", Kind::Text, None),
  (b"TDOR", Kind::Text, Some(StandardTagKey::OriginalDate)),
  (b"TDRC", Kind::Text, Some(StandardTagKey::Date)),
  (b"TDRL", Kind::Text, Some(StandardTagKey::ReleaseDate)),
  (b"TDTG", Kind::Text, Some(StandardTagKey::TaggingDate)),
  (b"TENC", Kind::Text, Some(StandardTagKey::EncodedBy)),
  (b"TEXT", Kind::Text, Some(StandardTagKey::Writer)),
  (b"TFLT", Kind::Text, None),
  (b"TIME", Kind::Text, Some(StandardTagKey::Date)),
  (b"TIPL", Kind::Text, None),
  (b"TIT1", Kind::Text, Some(StandardTagKey::ContentGroup)),
  (b"TIT2", Kind::Text, Some(StandardTagKey::TrackTitle)),
  (b"TIT3", Kind::Text, Some(StandardTagKey::TrackSubtitle)),
  (b"TKEY", Kind::Text, None),
  (b"TLAN", Kind::Text, Some(StandardTagKey::Language)),
  (b"TLEN", Kind::Text, None),
  (b"TMCL", Kind::Text, None),
  (b"TMED", Kind::Text, Some(StandardTagKey::MediaFormat)),
  (b"TMOO", Kind::Text, Some(StandardTagKey::Mood)),
  (b"TOAL", Kind::Text, Some(StandardTagKey::OriginalAlbum)),
  (b"TOFN", Kind::Text, Some(StandardTagKey::OriginalFile)),
  (b"TOLY", Kind::Text, Some(StandardTagKey::OriginalWriter)),
  (b"TOPE", Kind::Text, Some(StandardTagKey::OriginalArtist)),
  (b"TORY", Kind::Text, Some(StandardTagKey::OriginalDate)),
  (b"TOWN", Kind::Text, None),
  (b"TPE1", Kind::Text, Some(StandardTagKey::Artist)),
  (b"TPE2", Kind::Text, Some(StandardTagKey::AlbumArtist)),
  (b"TPE3", Kind::Text, Some(StandardTagKey::Conductor)),
  (b"TPE4", Kind::Text, Some(StandardTagKey::Remixer)),
  (b"TPOS", Kind::Text, Some(StandardTagKey::DiscNumber)),
  (b"TPRO", Kind::Text, None),
  (b"TPUB", Kind::Text, Some(StandardTagKey::Label)),
  (b"TRCK", Kind::Text, Some(StandardTagKey::TrackNumber)),
  (b"TRDA", Kind::Text, Some(StandardTagKey::Date)),
  (b"TRSN", Kind::Text, None),
  (b"TRSO", Kind::Text, None),
  (b"TSIZ", Kind::Text, None),
  (b"TSOA", Kind::Text, Some(StandardTagKey::SortAlbum)),
  (b"TSOP", Kind::Text, Some(StandardTagKey::SortArtist)),
  (b"TSOT", Kind::Text, Some(StandardTagKey::SortTrackTitle)),
  (b"TSRC", Kind::Text, Some(StandardTagKey::IdentIsrc)),
  (b"TSSE", Kind::Text, Some(StandardTagKey::Encoder)),
  (b"TSST", Kind::Text, None),
  (b"TXXX", Kind::UserText, None),
  (b"TYER", Kind::Text, Some(StandardTagKey::Date)),
  (b"USLT", Kind::Comment, Some(StandardTagKey::Lyrics)),
  (b"WCOM", Kind::Url, Some(StandardTagKey::UrlPurchase)),
  (b"WCOP", Kind::Url, Some(StandardTagKey::UrlCopyright)),
  (b"WOAF", Kind::Url, Some(StandardTagKey::UrlOfficial)),
  (b"WOAR", Kind::Url, Some(StandardTagKey::UrlArtist)),
  (b"WOAS", Kind::Url, Some(StandardTagKey::UrlSource)),
  (b"WORS", Kind::Url, Some(StandardTagKey::UrlInternetRadio)),
  (b"WPAY", Kind::Url, Some(StandardTagKey::UrlPayment)),
  (b"WPUB", Kind::Url, Some(StandardTagKey::UrlLabel)),
  (b"WXXX", Kind::UserUrl, Some(StandardTagKey::Url)),
  (b"GRP1", Kind::Text, None),
  (b"MVIN", Kind::Text, Some(StandardTagKey::MovementNumber)),
  (b"MVNM", Kind::Text, Some(StandardTagKey::MovementName)),
  (b"TCAT", Kind::Text, Some(StandardTagKey::PodcastCategory)),
  (
    b"TDES",
    Kind::Text,
    Some(StandardTagKey::PodcastDescription),
  ),
  (b"TGID", Kind::Text, Some(StandardTagKey::IdentPodcast)),
  (b"TKWD", Kind::Text, Some(StandardTagKey::PodcastKeywords)),
  (b"TSO2", Kind::Text, Some(StandardTagKey::SortAlbumArtist)),
  (b"TSOC", Kind::Text, Some(StandardTagKey::SortComposer)),
  (b"WFED", Kind::Text, Some(StandardTagKey::UrlPodcast)),
];

const USER_TEXT_KEYS: &[(&str, StandardTagKey)] = &[
  ("ACOUSTID FINGERPRINT", StandardTagKey::AcoustidFingerprint),
  ("ACOUSTID ID", StandardTagKey::AcoustidId),
  ("BARCODE", StandardTagKey::IdentBarcode),
  ("CATALOGNUMBER", StandardTagKey::IdentCatalogNumber),
  ("LICENSE", StandardTagKey::License),
  (
    "MUSICBRAINZ ALBUM ARTIST ID",
    StandardTagKey::MusicBrainzAlbumArtistId,
  ),
  ("MUSICBRAINZ ALBUM ID", StandardTagKey::MusicBrainzAlbumId),
  ("MUSICBRAINZ ARTIST ID", StandardTagKey::MusicBrainzArtistId),
  (
    "MUSICBRAINZ RELEASE GROUP ID",
    StandardTagKey::MusicBrainzReleaseGroupId,
  ),
  ("MUSICBRAINZ WORK ID", StandardTagKey::MusicBrainzWorkId),
  ("REPLAYGAIN_ALBUM_GAIN", StandardTagKey::ReplayGainAlbumGain),
  ("REPLAYGAIN_ALBUM_PEAK", StandardTagKey::ReplayGainAlbumPeak),
  ("REPLAYGAIN_TRACK_GAIN", StandardTagKey::ReplayGainTrackGain),
  ("REPLAYGAIN_TRACK_PEAK", StandardTagKey::ReplayGainTrackPeak),
  ("SCRIPT", StandardTagKey::Script),
];

pub(super) fn find(id: [u8; 4]) -> Result<Option<Found>, Never> {
  Ok(
    FRAMES
      .iter()
      .find(|(known, _, _)| **known == id)
      .map(|(_, kind, standard)| (*kind, *standard)),
  )
}

pub(super) fn find_legacy(id: [u8; 3]) -> Result<Option<Found>, Never> {
  let modern = LEGACY
    .iter()
    .find(|(old, _)| **old == id)
    .map(|(_, modern)| **modern);

  match modern {
    Some(modern_2) => find(modern_2),
    None => Ok(None),
  }
}

pub(super) fn user_text_key(description: &str) -> Result<Option<StandardTagKey>, Never> {
  Ok(
    USER_TEXT_KEYS
      .iter()
      .find(|(name, _)| *name == description)
      .map(|(_, standard)| *standard),
  )
}
