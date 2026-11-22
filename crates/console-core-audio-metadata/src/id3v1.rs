//! The tag an MP3 ends with: 128 bytes of fixed fields, read as lofty read
//! them, and the genre list ID3v1 numbered that the later tags still point
//! into.
//!
//! A field is Latin-1 cut at its first NUL, and a field that opens with one is
//! not there. A year counts only when it is four digits. A comment gives up
//! its last two bytes to a track number when the first of them is a NUL and
//! the second is not, which is how ID3v1.1 fitted one in. A genre past the
//! list names nothing.

use std::io::{Read, Seek};

use console_core_never::Never;
use console_core_number_conversion::index;

use crate::bytes::Source;
use crate::fault::{Malformed, ParseError};
use crate::keys::ItemKey;
use crate::tag::{Tag, TagType};
use crate::text::latin1;

pub(crate) const GENRES: [&str; 192] = [
  "Blues",
  "Classic rock",
  "Country",
  "Dance",
  "Disco",
  "Funk",
  "Grunge",
  "Hip-Hop",
  "Jazz",
  "Metal",
  "New Age",
  "Oldies",
  "Other",
  "Pop",
  "R&B",
  "Rap",
  "Reggae",
  "Rock",
  "Techno",
  "Industrial",
  "Alternative",
  "Ska",
  "Death metal",
  "Pranks",
  "Soundtrack",
  "Euro-Techno",
  "Ambient",
  "Trip-Hop",
  "Vocal",
  "Jazz & Funk",
  "Fusion",
  "Trance",
  "Classical",
  "Instrumental",
  "Acid",
  "House",
  "Game",
  "Sound clip",
  "Gospel",
  "Noise",
  "Alternative Rock",
  "Bass",
  "Soul",
  "Punk",
  "Space",
  "Meditative",
  "Instrumental Pop",
  "Instrumental Rock",
  "Ethnic",
  "Gothic",
  "Darkwave",
  "Techno-Industrial",
  "Electronic",
  "Pop-Folk",
  "Eurodance",
  "Dream",
  "Southern Rock",
  "Comedy",
  "Cult",
  "Gangsta",
  "Top 40",
  "Christian Rap",
  "Pop/Funk",
  "Jungle",
  "Native US",
  "Cabaret",
  "New Wave",
  "Psychedelic",
  "Rave",
  "Show tunes",
  "Trailer",
  "Lo-Fi",
  "Tribal",
  "Acid Punk",
  "Acid Jazz",
  "Polka",
  "Retro",
  "Musical",
  "Rock \\u{2019}n\\u{2019} Roll",
  "Hard Rock",
  "Folk",
  "Folk-Rock",
  "National Folk",
  "Swing",
  "Fast Fusion",
  "Bebop",
  "Latin",
  "Revival",
  "Celtic",
  "Bluegrass",
  "Avantgarde",
  "Gothic Rock",
  "Progressive Rock",
  "Psychedelic Rock",
  "Symphonic Rock",
  "Slow rock",
  "Big Band",
  "Chorus",
  "Easy Listening",
  "Acoustic",
  "Humour",
  "Speech",
  "Chanson",
  "Opera",
  "Chamber music",
  "Sonata",
  "Symphony",
  "Booty bass",
  "Primus",
  "Porn groove",
  "Satire",
  "Slow jam",
  "Club",
  "Tango",
  "Samba",
  "Folklore",
  "Ballad",
  "Power Ballad",
  "Rhythmic Soul",
  "Freestyle",
  "Duet",
  "Punk Rock",
  "Drum Solo",
  "A cappella",
  "Euro-House",
  "Dance Hall",
  "Goa music",
  "Drum & Bass",
  "Club-House",
  "Hardcore Techno",
  "Terror",
  "Indie",
  "BritPop",
  "Negerpunk",
  "Polsk Punk",
  "Beat",
  "Christian Gangsta Rap",
  "Heavy Metal",
  "Black Metal",
  "Crossover",
  "Contemporary Christian",
  "Christian rock",
  "Merengue",
  "Salsa",
  "Thrash Metal",
  "Anime",
  "Jpop",
  "Synthpop",
  "Abstract",
  "Art Rock",
  "Baroque",
  "Bhangra",
  "Big beat",
  "Breakbeat",
  "Chillout",
  "Downtempo",
  "Dub",
  "EBM",
  "Eclectic",
  "Electro",
  "Electroclash",
  "Emo",
  "Experimental",
  "Garage",
  "Global",
  "IDM",
  "Illbient",
  "Industro-Goth",
  "Jam Band",
  "Krautrock",
  "Leftfield",
  "Lounge",
  "Math Rock",
  "New Romantic",
  "Nu-Breakz",
  "Post-Punk",
  "Post-Rock",
  "Psytrance",
  "Shoegaze",
  "Space Rock",
  "Trop Rock",
  "World Music",
  "Neoclassical",
  "Audiobook",
  "Audio theatre",
  "Neue Deutsche Welle",
  "Podcast",
  "Indie-Rock",
  "G-Funk",
  "Dubstep",
  "Garage Rock",
  "Psybient",
];

const LENGTH: u64 = 128;

fn field(bytes: &[u8]) -> Result<Option<String>, Never> {
  Ok(match bytes.first() {
    Some(0) => None,
    Some(1..=255) | None => {
      let kept = match bytes.split(|byte| *byte == 0).next() {
        Some(kept) => kept,
        None => bytes,
      };
      let Ok(text) = latin1(kept);

      Some(text)
    }
  })
}

fn year(bytes: &[u8]) -> Result<Option<String>, Never> {
  let digits = bytes.iter().take_while(|byte| byte.is_ascii_digit()).fold(
    (0u8, 0u16),
    |(count, year), digit| {
      (
        count.saturating_add(1),
        year
          .saturating_mul(10)
          .saturating_add(u16::from(digit.saturating_sub(b'0'))),
      )
    },
  );

  Ok(match digits {
    (4, year) => Some(year.to_string()),
    (_count, _year) => None,
  })
}

pub(crate) fn tag(bytes: [u8; 128]) -> Result<Tag, Malformed> {
  let fields = bytes.strip_prefix(b"TAG").ok_or(Malformed::Signature)?;

  let part = |from: u64, to: u64| {
    let Ok(from_2) = index(from);
    let Ok(to_2) = index(to);

    match fields.get(from_2..to_2) {
      Some(part) => part,
      None => &[],
    }
  };

  let comment = match (fields.get(122), fields.get(123)) {
    (Some(0), Some(1..=255)) => part(94, 123),
    (Some(0), Some(0) | None) | (Some(1..=255) | None, Some(_) | None) => part(94, 124),
  };

  let genre = match fields.get(124) {
    Some(number) => {
      let Ok(at) = index(*number);

      GENRES.get(at).map(|named| named.to_string())
    }
    None => None,
  };

  let Ok(title) = field(part(0, 30));
  let Ok(artist) = field(part(30, 60));
  let Ok(album) = field(part(60, 90));
  let Ok(year) = year(part(90, 94));
  let Ok(comment_2) = field(comment);

  let Ok(mut tag) = Tag::new(TagType::Id3v1);

  for (key, text) in [
    (ItemKey::TrackTitle, title),
    (ItemKey::TrackArtist, artist),
    (ItemKey::AlbumTitle, album),
    (ItemKey::Year, year),
    (ItemKey::Comment, comment_2),
    (ItemKey::Genre, genre),
  ] {
    match text {
      Some(text_2) => {
        let Ok(()) = tag.insert_text(key, text_2);
      }
      None => {}
    }
  }

  Ok(tag)
}

pub(crate) fn trailer<R: Read + Seek>(
  source: &mut Source<'_, R>,
  end: u64,
) -> Result<Option<(Tag, u64)>, ParseError> {
  let start = match end.checked_sub(LENGTH) {
    Some(start) => start,
    None => return Ok(None),
  };

  source.seek_to(start)?;

  let bytes = source.array::<128>()?;

  match bytes.strip_prefix(b"TAG") {
    Some(_fields) => {}
    None => return Ok(None),
  }

  let tag = tag(bytes)?;

  Ok(Some((tag, start)))
}
