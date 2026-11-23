//! Making what is already in a folder the one format this device keeps.
//!
//! The fetcher decides that for what it fetches -- opus for sound, mkv for a
//! film -- and this is the same decision applied to what arrived some other
//! way: transferred off a laptop, synced from a phone, copied off a stick. A
//! folder of nine extensions is a folder you need nine programs to be sure of,
//! and the point of one extension is that the thing you find that plays it
//! plays all of it.
//!
//! The two are not the same operation, and it matters which is which. A film
//! is remuxed: the streams are lifted out of one container and put in another,
//! nothing is decoded, nothing is lost, and a gigabyte takes a second. Sound is
//! re-encoded, which is a real loss -- an mp3 made opus has been through two
//! lossy encoders -- and it is done anyway because 128k opus off a 320k mp3 is
//! a thing no one can hear the bottom of, and because the alternative is the
//! folder staying nine formats forever. What is replaced goes to the
//! wastebasket rather than being unlinked, so a conversion someone regrets is
//! an hour's walk back rather than a loss.

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::store::Kind;

const NOTHING_SAID: &str = "";

const NOT_A_STILL: &str = "0";

pub const BITRATE: &str = "128k";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wants {
  None,
  Leave,
  Ask,
  Made(Kind),
}

const SOUNDS: [&str; 7] = ["aac", "flac", "m4a", "mp3", "oga", "wav", "wma"];

const FILMS: [&str; 10] = [
  "avi", "flv", "m4v", "mov", "mp4", "mpeg", "mpg", "ts", "webm", "wmv",
];

const EITHER: [&str; 2] = ["ogg", "webm"];

pub fn wants(name: &str) -> Result<Wants, Never> {
  let (_taken, end) = match name.rsplit_once('.') {
    Some((_taken, end)) => (_taken, end),
    None => return Ok(Wants::Leave),
  };

  want_extension(end)
}

pub fn wants_at(path: &Path) -> Result<Wants, Never> {
  let name = match path.file_name() {
    Some(name) => name.as_encoded_bytes(),
    None => return Ok(Wants::Leave),
  };
  let mut parts = name.rsplitn(2, |byte| *byte == b'.');

  match (parts.next(), parts.next()) {
    (Some(ending), Some(_)) => match std::str::from_utf8(ending) {
      Ok(text) => want_extension(text),
      Err(_unknown_extension) => Ok(Wants::Leave),
    },
    (None, _) | (Some(_), None) => Ok(Wants::Leave),
  }
}

fn want_extension(end: &str) -> Result<Wants, Never> {
  let end_2 = end.to_lowercase();

  match ::core::primitive::bool::then_some(
    end_2 == crate::getting::SOUND || end_2 == crate::getting::FILM,
    (),
  ) {
    ::core::option::Option::Some(()) => return Ok(Wants::None),
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(EITHER.contains(&end_2.as_str()), ()) {
    ::core::option::Option::Some(()) => return Ok(Wants::Ask),
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(SOUNDS.contains(&end_2.as_str()), ()) {
    ::core::option::Option::Some(()) => return Ok(Wants::Made(Kind::Sound)),
    ::core::option::Option::None => {}
  }

  Ok(
    match ::core::primitive::bool::then_some(FILMS.contains(&end_2.as_str()), ()) {
      ::core::option::Option::Some(()) => Wants::Made(Kind::Film),
      ::core::option::Option::None => Wants::Leave,
    },
  )
}

pub fn beside(path: &Path, kind: Kind) -> Result<PathBuf, Never> {
  Ok(path.with_extension(match kind {
    Kind::Sound => crate::getting::SOUND,
    Kind::Film => crate::getting::FILM,
    Kind::Book => crate::getting::BOOK,
  }))
}

pub fn inside(said: &str) -> Result<Kind, Never> {
  let moving = said.lines().any(|line| {
    let mut said_2 = line.trim().split(',');
    let kind = match said_2.next() {
      Some(kind) => kind,
      None => NOTHING_SAID,
    };

    let still = match said_2.next() {
      Some(still) => still,
      None => NOT_A_STILL,
    };

    kind == "video" && still != "1"
  });

  Ok(match ::core::primitive::bool::then_some(moving, ()) {
    ::core::option::Option::Some(()) => Kind::Film,
    ::core::option::Option::None => Kind::Sound,
  })
}

pub fn about(path: &Path) -> Result<Vec<OsString>, Never> {
  let said = |word: &str| OsString::from(word);
  let Ok(ffprobe) = Program::Ffprobe.name();

  Ok(vec![
    said(ffprobe),
    said("-v"),
    said("error"),
    said("-select_streams"),
    said("v"),
    said("-show_entries"),
    said("stream=codec_type,disposition=attached_pic"),
    said("-of"),
    said("csv=p=0"),
    path.as_os_str().to_owned(),
  ])
}

fn ffmpeg(from: &Path, to: &Path, between: &[&str]) -> Result<Vec<OsString>, Never> {
  let said = |word: &&str| OsString::from(*word);

  Ok(
    ["ffmpeg", "-loglevel", "error", "-y", "-i"]
      .iter()
      .map(said)
      .chain(std::iter::once(from.as_os_str().to_owned()))
      .chain(between.iter().map(said))
      .chain(std::iter::once(to.as_os_str().to_owned()))
      .collect(),
  )
}

pub fn cover(from: &Path, to: &Path) -> Result<Vec<OsString>, Never> {
  ffmpeg(from, to, &["-map", "0:v", "-frames:v", "1"])
}

pub fn sound(from: &Path, to: &Path, picture: Option<&str>) -> Result<Vec<OsString>, Never> {
  let said = |word: &str| OsString::from(word);
  let mut arguments = vec![
    said("ffmpeg"),
    said("-loglevel"),
    said("error"),
    said("-y"),
    said("-i"),
    from.as_os_str().to_owned(),
    said("-map"),
    said("0:a"),
    said("-c:a"),
    said("libopus"),
    said("-b:a"),
    said(BITRATE),
    said("-map_metadata"),
    said("0"),
  ];

  match picture {
    Some(picture_2) => {
      arguments.push(said("-metadata"));
      arguments.push(OsString::from(format!(
        "METADATA_BLOCK_PICTURE={picture}",
        picture = picture_2
      )));
    }
    None => {}
  }

  arguments.push(to.as_os_str().to_owned());
  Ok(arguments)
}

pub fn film(from: &Path, to: &Path) -> Result<Vec<OsString>, Never> {
  ffmpeg(from, to, &["-map", "0", "-c", "copy"])
}

pub fn block(mime: &str, picture: &[u8]) -> Result<String, Never> {
  const FRONT_COVER: u32 = 3;
  let mut held = Vec::new();
  let Ok(wide) = fitted::<_, u32>(mime.len());

  held.extend_from_slice(&FRONT_COVER.to_be_bytes());
  held.extend_from_slice(&wide.to_be_bytes());
  held.extend_from_slice(mime.as_bytes());

  for _ in 0..5 {
    held.extend_from_slice(&0u32.to_be_bytes());
  }

  let Ok(many) = fitted::<_, u32>(picture.len());

  held.extend_from_slice(&many.to_be_bytes());
  held.extend_from_slice(picture);

  sixty_four(&held)
}

pub fn sixty_four(held: &[u8]) -> Result<String, Never> {
  const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut said = String::with_capacity(held.len().div_ceil(3).saturating_mul(4));

  for lot in held.chunks(3) {
    let Ok(pad) = fitted::<_, u32>(3usize.saturating_sub(lot.len()).saturating_mul(8));
    let held_3 = lot.iter().fold(0u32, |held_2, byte| {
      held_2.wrapping_shl(8) | u32::from(*byte)
    });
    let held_4 = held_3.wrapping_shl(pad);

    for at in 0..4 {
      match ::core::primitive::bool::then_some(at <= lot.len(), ()) {
        ::core::option::Option::Some(()) => {
          let Ok(down) = fitted::<_, u32>(18usize.saturating_sub(at.saturating_mul(6)));
          let Ok(which) = console_core_number_conversion::index(held_4.wrapping_shr(down) & 63);

          match ALPHABET.get(which) {
            Some(letter) => said.push(char::from(*letter)),
            None => said.push('='),
          }
        }
        ::core::option::Option::None => said.push('='),
      }
    }
  }

  Ok(said)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn what_is_already_the_one_format_is_left_where_it_is() {
    assert_eq!(wants("Africa [x].opus"), Ok(Wants::None));
    assert_eq!(wants("Africa [x].mkv"), Ok(Wants::None));
  }

  #[test]
  fn a_song_becomes_opus_and_a_film_becomes_mkv() {
    assert_eq!(wants("Africa.mp3"), Ok(Wants::Made(Kind::Sound)));
    assert_eq!(wants("Africa.FLAC"), Ok(Wants::Made(Kind::Sound)));
    assert_eq!(wants("holiday.mp4"), Ok(Wants::Made(Kind::Film)));
    assert_eq!(wants("holiday.MOV"), Ok(Wants::Made(Kind::Film)));
  }

  #[test]
  fn what_is_not_ours_is_not_touched() {
    assert_eq!(wants("cover.jpg"), Ok(Wants::Leave));
    assert_eq!(wants("notes.txt"), Ok(Wants::Leave));
    assert_eq!(wants("a film.kdenlive"), Ok(Wants::Leave));
    assert_eq!(wants("no extension at all"), Ok(Wants::Leave));
  }

  #[test]
  fn every_ending_this_calls_a_sound_is_one_the_player_lists() {
    let sounds = SOUNDS
      .iter()
      .chain(EITHER.iter())
      .chain([&crate::getting::SOUND]);

    for ending in sounds {
      let named = format!("a song.{ending}");

      assert_eq!(
        console_music_player::library::playable(Path::new(&named)),
        Ok(console_music_player::library::Playable::Yes),
        "downloads calls .{ending} a sound and the music player does not list it"
      );
    }
  }

  #[test]
  fn what_the_name_cannot_say_is_asked_of_the_file() {
    assert_eq!(wants("something.webm"), Ok(Wants::Ask));
    assert_eq!(wants("something.ogg"), Ok(Wants::Ask));
  }

  #[test]
  fn a_cover_is_not_what_makes_a_file_a_film() {
    assert_eq!(inside("video,0"), Ok(Kind::Film));
    assert_eq!(inside("video,1"), Ok(Kind::Sound));
    assert_eq!(inside(""), Ok(Kind::Sound));
  }

  #[test]
  fn the_new_name_is_the_old_one_under_the_new_extension() {
    let at = Path::new("/home/ada/Music/Africa [x].mp3");
    let Ok(beside) = beside(at, Kind::Sound);

    assert_eq!(beside, Path::new("/home/ada/Music/Africa [x].opus"));
  }

  #[test]
  fn a_film_is_moved_rather_than_decoded() {
    let Ok(arguments) = film(Path::new("/a/one.mp4"), Path::new("/a/one.mkv"));
    let coded = arguments.iter().skip_while(|word| *word != "-c").nth(1);

    assert_eq!(coded.and_then(|word| word.to_str()), Some("copy"));
  }

  #[test]
  fn a_song_carries_its_words_and_its_cover_over() {
    let (from, to) = (Path::new("/a/one.mp3"), Path::new("/a/one.opus"));
    let Ok(with) = sound(from, to, Some("Zm9v"));
    let Ok(without) = sound(from, to, None);

    assert!(with.contains(&OsString::from("METADATA_BLOCK_PICTURE=Zm9v")));
    assert!(with.contains(&OsString::from("-map_metadata")));
    assert!(
      !without
        .iter()
        .filter_map(|word| word.to_str())
        .any(|word| word.contains("PICTURE"))
    );
  }

  #[test]
  fn base_sixty_four_is_written_the_way_everyone_else_writes_it() {
    assert_eq!(sixty_four(b""), Ok(String::new()));
    assert_eq!(sixty_four(b"f"), Ok("Zg==".to_string()));
    assert_eq!(sixty_four(b"fo"), Ok("Zm8=".to_string()));
    assert_eq!(sixty_four(b"foo"), Ok("Zm9v".to_string()));
    assert_eq!(sixty_four(b"foobar"), Ok("Zm9vYmFy".to_string()));
    assert_eq!(sixty_four(&[0xff, 0xfe, 0xfd]), Ok("//79".to_string()));
  }

  #[test]
  fn the_picture_goes_in_as_the_block_every_writer_of_one_writes() {
    let Ok(said) = block("image/jpeg", b"\xff\xd8\xff");
    let held = said.as_bytes();
    assert!(!said.contains('\n'), "one line, because it is one argument");
    assert!(held.len() > 40);
  }
}
