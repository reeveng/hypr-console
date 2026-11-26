//! Album identity and track order as presented by Music Library.

use std::collections::BTreeMap;
use std::path::PathBuf;

use console_core_never::Never;
use serde::{Deserialize, Serialize};

use crate::looking::Song;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
pub enum Key {
  Release(String),
  Named { artist: String, title: String },
}

#[derive(Debug)]
pub struct Album<'a> {
  pub key: Key,
  pub artist: &'a str,
  pub title: &'a str,
  pub tracks: Vec<&'a Song>,
}

pub fn groups(songs: &[Song]) -> Result<Vec<Album<'_>>, Never> {
  let mut groups = BTreeMap::new();

  for song in songs {
    let title = song.tags.album.title.trim();

    match title {
      "" => continue,
      _ => {}
    }

    let artist = match song.tags.album.artist.trim() {
      "" => song.tags.artist.trim(),
      credited => credited,
    };
    let Ok(release) = release_id(&song.tags.album.release_id);
    let key = match release {
      Some(id) => Key::Release(id.to_ascii_lowercase()),
      None => Key::Named {
        artist: artist.to_lowercase(),
        title: title.to_lowercase(),
      },
    };
    let album = groups.entry(key.clone()).or_insert_with(|| Album {
      key,
      artist,
      title,
      tracks: Vec::new(),
    });

    album.tracks.push(song);
  }

  let mut albums: Vec<_> = groups.into_values().collect();
  albums.sort_by_cached_key(|album| (album.artist.to_lowercase(), album.title.to_lowercase()));

  for album in &mut albums {
    album.tracks.sort_by_cached_key(|song| {
      let Ok(title) = song.says();
      let track = &song.tags.album;

      (
        match track.disc {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => 1,
        },
        match track.track {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => u32::MAX,
        },
        title.to_lowercase(),
      )
    });
  }

  Ok(albums)
}

pub fn release_id(said: &str) -> Result<Option<&str>, Never> {
  let mut parts = said.split('-');

  for length in [8, 4, 4, 4, 12] {
    match parts.next() {
      Some(part) => {
        let Ok(bytes) = console_core_number_conversion::fitted::<_, u32>(part.len());

        match (bytes == length && part.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(())
        {
          Some(()) => {}
          None => return Ok(None),
        }
      }
      None => return Ok(None),
    }
  }

  Ok(match parts.next() {
    None => Some(said),
    Some(_extra) => None,
  })
}

pub fn paths(album: &Album<'_>) -> Result<Vec<PathBuf>, Never> {
  Ok(album.tracks.iter().map(|song| song.path.clone()).collect())
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_music_player::tags::{self, Tags};

  #[test]
  fn compilation_tracks_follow_disc_and_track_numbers() {
    let track = |artist: &str, n: u32, disc: u32| {
      let path = PathBuf::from(format!("/music/{artist}.flac"));
      let Ok(mut song) = Song::of(&path);
      song.tags = Tags {
        title: artist.to_owned(),
        artist: artist.to_owned(),
        album: tags::Album {
          title: "Compilation".to_owned(),
          artist: "Various Artists".to_owned(),
          disc: Some(disc),
          track: Some(n),
          ..tags::Album::default()
        },
        ..Tags::default()
      };

      song
    };
    let library = [track("A", 1, 2), track("B", 2, 1), track("C", 1, 1)];
    let Ok(albums) = groups(&library);

    match albums.as_slice() {
      [album] => {
        assert_eq!(album.artist, "Various Artists");
        let artists: Vec<_> = album
          .tracks
          .iter()
          .map(|song| song.tags.artist.as_str())
          .collect();
        assert_eq!(artists, ["C", "B", "A"]);
      }
      _ => assert_eq!(albums.len(), 1),
    }
  }
}
