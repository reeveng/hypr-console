//! The picture on the front of a song, put where a panel can open it.
//!
//! MPRIS says where a cover is and never what it is, so something has to write
//! the picture out as a file. kew kept one path and rewrote it in place, which
//! is the arrangement that cannot be told apart from nothing happening: a panel
//! handed the same path twice has no way to know the picture behind it changed,
//! and draws the one it already read. So the name carries the turn it was
//! written on, and the one before it is taken away as the next is made.
//!
//! The picture is copied rather than converted. Whatever is stapled to the song
//! comes out as it went in -- the audio metadata reader hands over the bytes, and the front cover
//! is preferred to whatever else a file carries -- and the name it lands under
//! says jpg whether or not it is one, because everything on this desktop that
//! opens a picture reads what is inside the file rather than the end of its
//! name.
//!
//! Where they go is `console_core_places::Base` rather than the toolkit's own
//! answer. glib is in the room for the bus, and `glib::user_cache_dir` would
//! have been the shortest line here, but it answers something for a machine
//! with no `HOME` and this desktop would rather write no picture than write one
//! where no one asked for it.

use console_core_audio_metadata::{FileParseError, Picture, PictureType};
use console_core_never::Never;
use console_core_places::Base;
use std::fs::{create_dir_all, read_dir, remove_file};
use std::path::{Path, PathBuf};

fn where_covers_go() -> Result<Option<PathBuf>, Never> {
  let ours = Base::Cache.ours()?;

  Ok(ours.map(|ours_2| ours_2.join("covers")))
}

struct CoverName(String);

impl AsRef<Path> for CoverName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

pub fn of(song: &Path, turn: u64) -> Result<Option<PathBuf>, Never> {
  let covers = where_covers_go()?;

  let folder = match covers {
    Some(folder) => folder,
    None => {
      eprintln!("music-player: no one's home, so the cover is not written anywhere");

      return Ok(None);
    }
  };

  match create_dir_all(&folder) {
    Ok(()) => {}
    Err(fault) => {
      eprintln!("music-player: {}: {fault}", folder.display());

      return Ok(None);
    }
  }

  let at = folder.join(CoverName(format!("cover-{turn}.jpg")));
  let Ok(picture) = picture(song);

  let bytes = match picture {
    Some(bytes) => bytes,
    None => return Ok(None),
  };

  match console_core_atomic_writes::whole(&at, &bytes) {
    Ok(()) => {}
    Err(fault) => {
      eprintln!("music-player: {}: {fault}", at.display());

      return Ok(None);
    }
  }

  let Ok(()) = swept(&folder, &at);

  Ok(Some(at))
}

pub fn picture(song: &Path) -> Result<Option<Vec<u8>>, Never> {
  let read = std::fs::File::open(song)
    .map_err(FileParseError::Open)
    .and_then(|mut reader| console_core_audio_metadata::read(&mut reader, song));
  let file = match read {
    Ok(file) => file,
    Err(fault) => {
      eprintln!(
        "music-player: {}: asking for the cover: {fault}",
        song.display()
      );

      return Ok(None);
    }
  };

  let every: Vec<&Picture> = file.tags.iter().flat_map(|tag| &tag.pictures).collect();
  let front = every
    .iter()
    .find(|picture| picture.kind == PictureType::CoverFront);

  Ok(front.or(every.first()).map(|picture| picture.data.clone()))
}

fn swept(folder: &Path, keeping: &Path) -> Result<(), Never> {
  let held = match read_dir(folder) {
    Ok(held) => held,
    Err(_nothing_to_sweep) => return Ok(()),
  };

  for found in held.flatten() {
    let at = found.path();

    match ::core::primitive::bool::then_some(at == keeping, ()) {
      ::core::option::Option::Some(()) => continue,
      ::core::option::Option::None => {}
    }

    match remove_file(&at) {
      Ok(()) => {}
      Err(fault) => eprintln!("music-player: {}: {fault}", at.display()),
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_audio_metadata::{MimeType, Tag, TagType};

  #[test]
  fn the_front_cover_is_the_picture_copied_out_of_a_song_that_carries_two()
  -> Result<(), Box<dyn std::error::Error>> {
    let folder = console_core_temporary_directories::fresh("music-player-art")?;
    let song = folder.join("song.wav");
    let Ok(silence) = crate::tags::tests::a_second_of_silence();

    console_core_atomic_writes::whole(&song, &silence)?;

    let Ok(mut tag) = Tag::new(TagType::Id3v2);

    tag.pictures.push(Picture {
      kind: PictureType::CoverBack,
      mime_type: Some(MimeType::Png),
      data: b"the back".to_vec(),
    });
    tag.pictures.push(Picture {
      kind: PictureType::CoverFront,
      mime_type: Some(MimeType::Png),
      data: b"the front".to_vec(),
    });

    let tagged = console_core_audio_metadata::tagged_wave(&silence, &tag)?;

    console_core_atomic_writes::whole(&song, &tagged)?;

    assert_eq!(picture(&song), Ok(Some(b"the front".to_vec())));
    assert_eq!(picture(&folder.join("none.wav")), Ok(None));

    Ok(())
  }
}
