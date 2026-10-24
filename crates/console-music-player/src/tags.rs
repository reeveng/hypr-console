//! What a song says about itself.
//!
//! This was `console-music`'s until the player stopped being kew's. Both need
//! it now -- the panel to list a library it is not playing, the player to say
//! on the bus what is playing -- and a second reading of one file is the thing
//! `CLAUDE.md` warns drifts quietly. So the reading is here, once, for both.
//!
//! `console-core-audio-metadata` reads the file. It used to be ffprobe, asked
//! once per song, which made a library of a thousand songs a thousand programs
//! started and their JSON read back; this is the same answer in the same
//! process, for every kind of file the player plays, from source this tree
//! carries rather than a package it installs.
//!
//! Where a tag lives is the whole reason this is not one line. An mp3 says its
//! title in an ID3 frame, an opus in a Vorbis comment, an m4a in an atom, and
//! one file can carry two of those that disagree. The reader names each item the
//! same way whatever it was kept in, and [`NAMED`] says which of those names
//! are about the music and what this tree calls them -- the words ffprobe used,
//! because they are the ones the rest of this file was written against. An
//! item it does not name is an identifier, a sort order or a gain, and is
//! nobody's business on a screen. The primary tag is read first, and the first
//! answer to a name is the one kept.

use std::collections::BTreeSet;
use std::path::Path;

use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_audio_metadata::error::FileParseError;
use console_core_audio_metadata::prelude::{AudioFile, ItemKey, TaggedFileExt};
use console_core_audio_metadata::tag::Tag;

pub const BETWEEN: &str = " \u{00b7} ";

pub const AS_MUCH: u32 = 300;

const NAMED: [(ItemKey, &str); 15] = [
    (ItemKey::TrackTitle, "title"),
    (ItemKey::TrackArtist, "artist"),
    (ItemKey::AlbumArtist, "album_artist"),
    (ItemKey::AlbumTitle, "album"),
    (ItemKey::TrackSubtitle, "subtitle"),
    (ItemKey::RecordingDate, "date"),
    (ItemKey::Year, "date"),
    (ItemKey::Genre, "genre"),
    (ItemKey::Comment, "comment"),
    (ItemKey::Description, "description"),
    (ItemKey::Composer, "composer"),
    (ItemKey::Performer, "performer"),
    (ItemKey::Publisher, "publisher"),
    (ItemKey::Label, "label"),
    (ItemKey::CopyrightMessage, "copyright"),
];

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub rest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tagged {
    Some,
    None,
}

impl Tags {
    pub fn tagged(&self) -> Result<Tagged, Never> {
        Ok(match self.title.is_empty() && self.artist.is_empty() && self.rest.is_empty() {
            true => Tagged::None,
            false => Tagged::Some,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metadata {
    pub items: Vec<(String, String)>,
    pub length: f64,
}

#[derive(Debug)]
pub enum Unread {
    File(FileParseError),
}

impl std::fmt::Display for Unread {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unread::File(fault) => write!(formatter, "reading its tags: {fault}"),
        }
    }
}

impl std::error::Error for Unread {}

pub fn metadata(path: &Path) -> Result<Metadata, Unread> {
    let file = console_core_audio_metadata::read_from_path(path).map_err(Unread::File)?;
    let mut items: Vec<(String, String)> = Vec::new();
    let primary = file.primary_tag();

    let first: Vec<&Tag> = primary.into_iter().chain(file.tags()).collect();

    for tag in first {
        let Ok(()) = gathered(tag, &mut items);
    }

    Ok(Metadata { items, length: file.properties().duration().as_secs_f64() })
}

fn heard(path: &Path) -> Result<Metadata, Never> {
    Ok(match metadata(path) {
        Ok(metadata) => metadata,
        Err(fault) => {
            eprintln!("music: {}: {fault}", path.display());

            Metadata::default()
        },
    })
}

pub fn of(path: &Path) -> Result<Tags, Never> {
    let Ok(heard) = heard(path);

    read(&heard.items)
}

pub fn playing(path: &Path) -> Result<Playing, Never> {
    let Ok(heard) = heard(path);

    played(&heard)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Playing {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub length: f64,
}

pub fn played(metadata: &Metadata) -> Result<Playing, Never> {
    let Ok(tags) = read(&metadata.items);

    let first = |wanted: &str| {
        metadata.items.iter().find(|(name, _said)| name == wanted).map(|(_name, said)| said.clone())
    };

    Ok(Playing {
        title: tags.title,
        artist: tags.artist,
        album: match first("album") {
            Some(album) => album,
            None => String::new(),
        },
        length: metadata.length,
    })
}

pub fn read(all: &[(String, String)]) -> Result<Tags, Never> {
    #[cfg_attr(
        dylint_lib = "explicit028_no_search_in_a_loop",
        allow(
            explicit028_no_search_in_a_loop,
            reason = "the tags of one song, searched for the few names that mean a title: neither list grows with anything"
        )
    )]
    let first = |wanted: &[&str]| {
        wanted.iter().find_map(|want| {
            all.iter().find(|(name, _)| name == want).map(|(_, said)| said.clone())
        })
    };
    let title = match first(&["title"]) {
        Some(title) => title,
        None => String::new(),
    };

    let artist = match first(&["artist", "album_artist"]) {
        Some(artist) => artist,
        None => String::new(),
    };
    let rest = rest(all, Song { title: &title, artist: &artist })?;

    Ok(Tags { rest, title, artist })
}

fn named(key: ItemKey) -> Result<Option<&'static str>, Never> {
    Ok(NAMED.iter().find(|(named, _name)| *named == key).map(|(_key, name)| *name))
}

fn gathered(tag: &Tag, into: &mut Vec<(String, String)>) -> Result<(), Never> {
    let mut had: BTreeSet<String> = into.iter().map(|(name, _)| name.clone()).collect();
    let mut found: Vec<(String, String)> = Vec::new();

    for item in tag.items() {
        let Ok(name) = named(item.key());

        let (name, said) = match (name, item.value().text()) {
            (Some(name), Some(said)) => (name, said),
            (None, _) | (_, None) => continue,
        };

        let said = said.split_whitespace().collect::<Vec<&str>>().join(" ");

        match said.is_empty() || !had.insert(name.to_string()) {
            true => continue,
            false => {},
        }

        found.push((name.to_string(), said));
    }

    found.sort();
    into.extend(found);

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Song<'a> {
    title: &'a str,
    artist: &'a str,
}

fn rest(all: &[(String, String)], song: Song<'_>) -> Result<String, Never> {
    let Song { title, artist } = song;
    let mut rest: Vec<&str> = Vec::new();
    let mut already: BTreeSet<&str> = BTreeSet::new();

    for (_name, said) in all {
        let known = said == title || said == artist;

        match known {
            true => continue,
            false => {},
        }

        match already.insert(said.as_str()) {
            true => rest.push(said),
            false => {},
        }
    }

    cut(&rest.join(BETWEEN), AS_MUCH)
}

fn cut(said: &str, to: u32) -> Result<String, Never> {
    let Ok(to) = index(to);

    Ok(match said.char_indices().nth(to).and_then(|(at, _)| said.get(..at)) {
        Some(head) => head.trim_end().to_string(),
        None => said.to_string(),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use console_core_audio_metadata::config::WriteOptions;
    use console_core_audio_metadata::tag::{TagExt, TagType};
    use console_core_number_conversion::fitted;

    fn items(said: &[(&str, &str)]) -> Result<Vec<(String, String)>, Never> {
        Ok(said.iter().map(|(name, said)| (name.to_string(), said.to_string())).collect())
    }

    fn an_opus() -> Result<Vec<(String, String)>, Never> {
        items(&[
            ("album", "Favourite Worst Nightmare"),
            ("artist", "Arctic Monkeys"),
            ("date", "20141225"),
            ("title", "505"),
        ])
    }

    fn an_mp3() -> Result<Vec<(String, String)>, Never> {
        items(&[("artist", "2PacVEVO"), ("date", "20110705"), ("title", "2Pac - Changes ft. Talent")])
    }

    pub(crate) fn a_second_of_silence() -> Result<Vec<u8>, Never> {
        a_wave(&[0; 8000])
    }

    pub(crate) fn a_wave(samples: &[i16]) -> Result<Vec<u8>, Never> {
        let rate: u32 = 8000;
        let Ok(count) = fitted::<_, u32>(samples.len());
        let data = count.saturating_mul(2);
        let mut said: Vec<u8> = Vec::new();

        said.extend(b"RIFF");
        said.extend(data.saturating_add(36).to_le_bytes());
        said.extend(b"WAVEfmt ");
        said.extend(16_u32.to_le_bytes());
        said.extend(1_u16.to_le_bytes());
        said.extend(1_u16.to_le_bytes());
        said.extend(rate.to_le_bytes());
        said.extend(rate.saturating_mul(2).to_le_bytes());
        said.extend(2_u16.to_le_bytes());
        said.extend(16_u16.to_le_bytes());
        said.extend(b"data");
        said.extend(data.to_le_bytes());
        said.extend(samples.iter().flat_map(|sample| sample.to_le_bytes()));

        Ok(said)
    }

    #[test]
    fn a_song_on_disk_says_its_title_artist_and_length() -> Result<(), Box<dyn std::error::Error>> {
        let folder = console_core_temporary_directories::fresh("music-player-tags")?;
        let song = folder.join("song.wav");
        let Ok(silence) = a_second_of_silence();

        console_core_atomic_writes::whole(&song, &silence)?;

        let mut tag = Tag::new(TagType::Id3v2);

        tag.insert_text(ItemKey::TrackTitle, "Rain".to_string());
        tag.insert_text(ItemKey::TrackArtist, "Nujabes".to_string());
        tag.insert_text(ItemKey::AlbumTitle, "Modal Soul".to_string());
        tag.save_to_path(&song, WriteOptions::default())?;

        let Ok(playing) = playing(&song);

        assert_eq!(playing.title, "Rain");
        assert_eq!(playing.artist, "Nujabes");
        assert_eq!(playing.album, "Modal Soul");
        assert_eq!(playing.length.round(), 1.0);

        Ok(())
    }

    #[test]
    fn a_song_that_keeps_its_tags_in_one_place_is_read_the_same_as_one_that_keeps_them_in_another() {
        let Ok(opus_said) = an_opus();
        let Ok(opus) = read(&opus_said);
        let Ok(mp3_said) = an_mp3();
        let Ok(mp3) = read(&mp3_said);

        assert_eq!(opus.title, "505");
        assert_eq!(opus.artist, "Arctic Monkeys");
        assert_eq!(mp3.title, "2Pac - Changes ft. Talent");
        assert_eq!(mp3.artist, "2PacVEVO");
    }

    #[test]
    fn what_no_one_named_is_not_what_the_song_says() {
        let Ok(tags) = read(&[]);

        assert_eq!(tags, Tags::default());
        assert_eq!(tags.tagged(), Ok(Tagged::None));
    }

    #[test]
    fn the_rest_is_what_is_about_the_music() {
        let Ok(opus_said) = an_opus();
        let Ok(opus) = read(&opus_said);
        let Ok(mp3_said) = an_mp3();
        let Ok(mp3) = read(&mp3_said);

        assert_eq!(opus.rest, format!("Favourite Worst Nightmare{BETWEEN}20141225"));
        assert_eq!(mp3.rest, "20110705");
    }

    #[test]
    fn what_is_said_twice_is_kept_once() {
        let Ok(said) = items(&[
            ("album_artist", "Nujabes"),
            ("artist", "Nujabes"),
            ("comment", "Provided to YouTube by Hydeout"),
            ("description", "Provided to YouTube by Hydeout"),
        ]);
        let Ok(tags) = read(&said);

        assert_eq!(tags.rest, "Provided to YouTube by Hydeout");
    }

    #[test]
    fn a_description_no_one_would_read_is_cut_where_it_stops_being_worth_it() {
        let long = "x".repeat(1000);
        let Ok(said) = items(&[("comment", &long)]);
        let Ok(tags) = read(&said);

        assert_eq!(u32::try_from(tags.rest.chars().count()), Ok(AS_MUCH));
    }

    #[test]
    fn a_file_that_could_not_be_read_says_nothing() {
        let Ok(tags) = of(Path::new("/nowhere/at/all.opus"));

        assert_eq!(tags, Tags::default());
    }
}
