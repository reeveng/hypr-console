//! The words MPRIS uses for what this player holds.
//!
//! None of this is the crate's own opinion. `console_music_panel::player` already
//! parses every one of these strings out of kew's answers, and the checks press
//! them through busctl, so what is spelled here is what a panel written against
//! another player expects to read. The names stay; only who answers changes.
//!
//! `mpris:trackid` is here because the panel reads it out of `Metadata` and
//! hands it straight back to `SetPosition`, which takes the track being sought
//! in as well as the place. A player that left it out would be asking the panel
//! to seek in whatever happened to be playing when the call landed.
//!
//! A path is spelled into a url by putting `file://` in front of it and nothing
//! else. That is not the encoding the specification asks for, and it is what
//! `console_music_panel::player::local` reads back: it strips the prefix and takes
//! what is left as a path, so a song with a space in its name survives this and
//! would not survive being percent-encoded. The reader and the writer are both
//! in this repository, which is the only reason that is allowed to be true.

use console_core_never::Never;
use console_core_number_conversion::{Float, toward_zero_i64};
use std::path::{Path, PathBuf};

use crate::playlist::Over;

pub const NAME: &str = "org.mpris.MediaPlayer2.console";

pub const OBJECT: &str = "/org/mpris/MediaPlayer2";

pub const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

pub const ROOT: &str = "org.mpris.MediaPlayer2";

pub const IDENTITY: &str = "Console";

pub const TRACK: &str = "/org/mpris/MediaPlayer2/Track/one";

const A_SECOND: f64 = 1_000_000.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Status {
    Playing,
    Paused,
    #[default]
    Stopped,
}

impl Status {
    pub fn said(self) -> Result<&'static str, Never> {
        Ok(match self {
            Status::Playing => "Playing",
            Status::Paused => "Paused",
            Status::Stopped => "Stopped",
        })
    }
}

pub fn over_said(over: Over) -> Result<&'static str, Never> {
    Ok(match over {
        Over::On => "None",
        Over::Again => "Track",
        Over::Round => "Playlist",
    })
}

pub fn over_read(said: &str) -> Result<Over, Never> {
    Ok(match said {
        "None" => Over::On,
        "Track" => Over::Again,
        "Playlist" => Over::Round,
        _unknown_to_this_player => Over::default(),
    })
}

pub fn micros(seconds: f64) -> Result<i64, Never> {
    toward_zero_i64(seconds * A_SECOND)
}

pub fn seconds(micros: i64) -> Result<f64, Never> {
    let Ok(held) = micros.float();

    Ok(held / A_SECOND)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Song {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art: Option<PathBuf>,
    pub length: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Told {
    Track(String),
    Word(String),
    Words(Vec<String>),
    Long(i64),
}

pub fn url(at: &Path) -> Result<String, Never> {
    Ok(format!("file://{}", at.display()))
}

pub fn metadata(song: &Song) -> Result<Vec<(String, Told)>, Never> {
    let Ok(length) = micros(song.length);

    let mut said = vec![
        ("mpris:trackid".to_string(), Told::Track(TRACK.to_string())),
        ("mpris:length".to_string(), Told::Long(length)),
        ("xesam:title".to_string(), Told::Word(song.title.clone())),
        ("xesam:artist".to_string(), Told::Words(vec![song.artist.clone()])),
        ("xesam:album".to_string(), Told::Word(song.album.clone())),
    ];

    match &song.art {
        Some(art) => {
            let Ok(url) = url(art);

            said.push(("mpris:artUrl".to_string(), Told::Word(url)));
        },
        None => {},
    }

    Ok(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_loop_status_the_panel_can_press_comes_back_as_itself() {
        for over in [Over::On, Over::Again, Over::Round] {
            let Ok(said) = over_said(over);
            let Ok(read) = over_read(said);

            assert_eq!(read, over);
        }
    }

    #[test]
    fn a_loop_status_this_player_does_not_know_is_the_ordinary_one() {
        let Ok(read) = over_read("Sideways");

        assert_eq!(read, Over::default());
    }

    #[test]
    fn a_position_goes_out_in_microseconds_and_comes_back_in_seconds() {
        let Ok(out) = micros(90.5);
        let Ok(back) = seconds(out);

        assert_eq!(out, 90_500_000);
        assert_eq!(back, 90.5);
    }

    #[test]
    fn a_song_with_a_space_in_its_name_survives_being_spelled_as_a_url() {
        let Ok(url) = url(&PathBuf::from("/music/The Last Shadow Puppets.opus"));

        assert_eq!(url, "file:///music/The Last Shadow Puppets.opus");
    }

    #[test]
    fn the_track_being_sought_in_is_named_and_the_file_playing_is_not() {
        let Ok(said) = metadata(&Song {
            title: "505".to_string(),
            artist: "Arctic Monkeys".to_string(),
            album: "Favourite Worst Nightmare".to_string(),
            art: None,
            length: 253.0,
        });

        let named: Vec<&str> = said.iter().map(|(name, _told)| name.as_str()).collect();

        assert!(named.contains(&"mpris:trackid"));
        assert!(!named.contains(&"xesam:url"));
    }
}
