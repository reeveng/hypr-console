//! Vorbis comments as FLAC and Ogg both carry them: a vendor string, then
//! `NAME=value` lines, one of which may be a picture. A name is compared
//! without regard to case and keeps its own spelling in the tag it makes.

use console_core_audio_decoding_interfaces::errors::Error;
use console_core_audio_decoding_interfaces::meta::{MetadataBuilder, StandardTagKey, Tag, Value};
use console_core_byte_streams::{BufReader, ReadBytes};
use console_core_logging::warn;
use console_core_never::Never;

use crate::{flac, utf8_with_replacement};

const PICTURE_KEY: &str = "metadata_block_picture";

const KEYS: &[(&str, StandardTagKey)] = &[
  ("album artist", StandardTagKey::AlbumArtist),
  ("album", StandardTagKey::Album),
  ("albumartist", StandardTagKey::AlbumArtist),
  ("albumartistsort", StandardTagKey::SortAlbumArtist),
  ("albumsort", StandardTagKey::SortAlbum),
  ("arranger", StandardTagKey::Arranger),
  ("artist", StandardTagKey::Artist),
  ("artistsort", StandardTagKey::SortArtist),
  ("author", StandardTagKey::Writer),
  ("barcode", StandardTagKey::IdentBarcode),
  ("bpm", StandardTagKey::Bpm),
  ("catalog #", StandardTagKey::IdentCatalogNumber),
  ("catalog", StandardTagKey::IdentCatalogNumber),
  ("catalognumber", StandardTagKey::IdentCatalogNumber),
  ("catalogue #", StandardTagKey::IdentCatalogNumber),
  ("comment", StandardTagKey::Comment),
  ("compileation", StandardTagKey::Compilation),
  ("composer", StandardTagKey::Composer),
  ("conductor", StandardTagKey::Conductor),
  ("copyright", StandardTagKey::Copyright),
  ("date", StandardTagKey::Date),
  ("description", StandardTagKey::Description),
  ("disc", StandardTagKey::DiscNumber),
  ("discnumber", StandardTagKey::DiscNumber),
  ("discsubtitle", StandardTagKey::DiscSubtitle),
  ("disctotal", StandardTagKey::DiscTotal),
  ("disk", StandardTagKey::DiscNumber),
  ("disknumber", StandardTagKey::DiscNumber),
  ("disksubtitle", StandardTagKey::DiscSubtitle),
  ("disktotal", StandardTagKey::DiscTotal),
  ("djmixer", StandardTagKey::MixDj),
  ("ean/upn", StandardTagKey::IdentEanUpn),
  ("encoded-by", StandardTagKey::EncodedBy),
  ("encoder settings", StandardTagKey::EncoderSettings),
  ("encoder", StandardTagKey::Encoder),
  ("encoding", StandardTagKey::EncoderSettings),
  ("engineer", StandardTagKey::Engineer),
  ("ensemble", StandardTagKey::Ensemble),
  ("genre", StandardTagKey::Genre),
  ("isrc", StandardTagKey::IdentIsrc),
  ("language", StandardTagKey::Language),
  ("label", StandardTagKey::Label),
  ("license", StandardTagKey::License),
  ("lyricist", StandardTagKey::Lyricist),
  ("lyrics", StandardTagKey::Lyrics),
  ("media", StandardTagKey::MediaFormat),
  ("mixer", StandardTagKey::MixEngineer),
  ("mood", StandardTagKey::Mood),
  (
    "musicbrainz_albumartistid",
    StandardTagKey::MusicBrainzAlbumArtistId,
  ),
  ("musicbrainz_albumid", StandardTagKey::MusicBrainzAlbumId),
  ("musicbrainz_artistid", StandardTagKey::MusicBrainzArtistId),
  ("musicbrainz_discid", StandardTagKey::MusicBrainzDiscId),
  (
    "musicbrainz_originalalbumid",
    StandardTagKey::MusicBrainzOriginalAlbumId,
  ),
  (
    "musicbrainz_originalartistid",
    StandardTagKey::MusicBrainzOriginalArtistId,
  ),
  (
    "musicbrainz_recordingid",
    StandardTagKey::MusicBrainzRecordingId,
  ),
  (
    "musicbrainz_releasegroupid",
    StandardTagKey::MusicBrainzReleaseGroupId,
  ),
  (
    "musicbrainz_releasetrackid",
    StandardTagKey::MusicBrainzReleaseTrackId,
  ),
  ("musicbrainz_trackid", StandardTagKey::MusicBrainzTrackId),
  ("musicbrainz_workid", StandardTagKey::MusicBrainzWorkId),
  ("opus", StandardTagKey::Opus),
  ("organization", StandardTagKey::Label),
  ("originaldate", StandardTagKey::OriginalDate),
  ("part", StandardTagKey::Part),
  ("performer", StandardTagKey::Performer),
  ("producer", StandardTagKey::Producer),
  ("productnumber", StandardTagKey::IdentPn),
  ("publisher", StandardTagKey::Label),
  ("rating", StandardTagKey::Rating),
  ("releasecountry", StandardTagKey::ReleaseCountry),
  ("remixer", StandardTagKey::Remixer),
  ("replaygain_album_gain", StandardTagKey::ReplayGainAlbumGain),
  ("replaygain_album_peak", StandardTagKey::ReplayGainAlbumPeak),
  ("replaygain_track_gain", StandardTagKey::ReplayGainTrackGain),
  ("replaygain_track_peak", StandardTagKey::ReplayGainTrackPeak),
  ("script", StandardTagKey::Script),
  ("subtitle", StandardTagKey::TrackSubtitle),
  ("title", StandardTagKey::TrackTitle),
  ("titlesort", StandardTagKey::SortTrackTitle),
  ("totaldiscs", StandardTagKey::DiscTotal),
  ("totaltracks", StandardTagKey::TrackTotal),
  ("tracknumber", StandardTagKey::TrackNumber),
  ("tracktotal", StandardTagKey::TrackTotal),
  ("unsyncedlyrics", StandardTagKey::Lyrics),
  ("upc", StandardTagKey::IdentUpc),
  ("version", StandardTagKey::Version),
  ("writer", StandardTagKey::Writer),
  ("year", StandardTagKey::Date),
];

fn add_picture(encoded: &str, metadata: &mut MetadataBuilder) -> Result<(), Never> {
  let Ok(decoded) = base64_decode(encoded);

  match decoded {
    Some(data) => {
      let Ok(mut block) = BufReader::new(&data);

      match flac::read_picture_block(&mut block, metadata) {
        Ok(()) => Ok(()),
        Err(fault) => {
          warn!("invalid picture block data: {fault}");

          Ok(())
        }
      }
    }
    None => {
      warn!("the base64 encoding of a picture block is invalid");

      Ok(())
    }
  }
}

fn add_comment(comment: &str, metadata: &mut MetadataBuilder) -> Result<(), Never> {
  match comment.split_once('=') {
    None => Ok(()),
    Some((key, value)) => {
      let lowered = key.to_lowercase();

      match lowered.as_str() {
        PICTURE_KEY => add_picture(value, metadata),
        _other => {
          let standard = KEYS
            .iter()
            .find(|(name, _)| *name == lowered)
            .map(|(_, standard)| *standard);

          let Ok(value_1) = Tag::new(standard, key, Value::from(value));

          let Ok(_configured_add_tag) = metadata.add_tag(value_1);

          Ok(())
        }
      }
    }
  }
}

pub fn read_comment_no_framing<B: ReadBytes>(
  reader: &mut B,
  metadata: &mut MetadataBuilder,
) -> Result<(), Error> {
  let vendor_length = reader.read_u32()?;

  reader.ignore_bytes(u64::from(vendor_length))?;

  let comments = reader.read_u32()?;

  for _ in 0..comments {
    let length = reader.read_u32()?;
    let bytes = reader.read_boxed_slice_exact(u64::from(length))?;
    let Ok(comment) = utf8_with_replacement(&bytes);

    let Ok(()) = add_comment(&comment, metadata);
  }

  Ok(())
}

fn base64_decode(encoded: &str) -> Result<Option<Vec<u8>>, Never> {
  let unpadded = encoded.trim_end_matches('=');

  Ok(
    match console_core_bytes::base64::decode_unpadded(unpadded.as_bytes()) {
      Ok(bytes) => {
        let Ok(decoded) = bytes.into_vec();

        Some(decoded)
      }
      Err(_not_base64) => None,
    },
  )
}

#[cfg(test)]
mod tests {
  use super::base64_decode;

  const CASES: &[(&str, Option<&[u8]>)] = &[
    ("", Some(b"")),
    ("Zg==", Some(b"f")),
    ("Zm8=", Some(b"fo")),
    ("Zm9v", Some(b"foo")),
    ("Zm9vYg==", Some(b"foob")),
    ("Zm9vYmE=", Some(b"fooba")),
    ("Zm9vYmFy", Some(b"foobar")),
    ("Zg", Some(b"f")),
    ("Zm8", Some(b"fo")),
    ("Zm9vYg", Some(b"foob")),
    ("Zm9vYmE", Some(b"fooba")),
    ("a", None),
    ("ab!c", None),
    ("ab=c", None),
  ];

  #[test]
  fn the_rfc_4648_vectors_decode_with_padding_and_without() {
    for (encoded, expected) in CASES {
      let Ok(decoded) = base64_decode(encoded);

      assert_eq!(decoded.as_deref(), *expected, "{encoded}");
    }
  }
}
