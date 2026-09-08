//! What a song says about itself.
//!
//! This was `console-music-panel`'s until the player stopped being kew's. Both need
//! it now -- the panel to list a library it is not playing, the player to say
//! on the bus what is playing -- and a second reading of one file is the thing
//! `CLAUDE.md` warns drifts quietly. So the reading is here and the running is
//! not: [`asking`] is the words to ask ffprobe, [`read`] is what the answer
//! means, and each caller runs the program the way its own crate runs programs.
//!
//! ffprobe is asked, and nothing here reads a file. It is on the machine
//! already -- the downloader puts the cover inside a song with ffmpeg and the
//! wallpaper is pressed with it -- and it reads every kind of file kew plays,
//! which is nine formats keeping their tags in four different places.
//!
//! Where a tag lives is the whole reason this is not one line. An mp3 says its
//! title in the format's tags; an opus says it in the audio stream's; and the
//! picture stapled to either is a stream of its own carrying tags that look
//! exactly like a song's and say "Album cover". So the format is read first,
//! then the streams that are sound, and the first answer to a name is the one
//! kept.

use std::path::Path;

use console_core_external_programs::Program;
use console_core_never::Never;
use serde_json::Value;

pub const BETWEEN: &str = " \u{00b7} ";

pub const AS_MUCH: usize = 300;

const NOT_THE_MUSIC: [&str; 8] = [
    "compatible_brands",
    "description",
    "encoder",
    "language",
    "major_brand",
    "minor_version",
    "purl",
    "synopsis",
];

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub rest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said {
    Something,
    Nothing,
}

impl Tags {
    pub fn anything(&self) -> Result<Said, Never> {
        Ok(match self.title.is_empty() && self.artist.is_empty() && self.rest.is_empty() {
            true => Said::Nothing,
            false => Said::Something,
        })
    }
}

pub fn asking(path: &Path) -> Result<Vec<String>, Never> {
    let word = |said: &str| said.to_string();

    Ok(vec![
        word("-v"),
        word("quiet"),
        word("-show_entries"),
        word("format=duration:format_tags:stream=codec_type:stream_tags"),
        word("-of"),
        word("json"),
        word("-i"),
        path.to_string_lossy().to_string(),
    ])
}

pub fn of(path: &Path) -> Result<Tags, Never> {
    let Ok(said) = ffprobe(path);

    read(&said)
}

pub fn playing(path: &Path) -> Result<Playing, Never> {
    let Ok(said) = ffprobe(path);

    played(&said)
}

fn ffprobe(path: &Path) -> Result<String, Never> {
    let Ok(argv) = asking(path);
    let Ok(mut asking) = Program::Ffprobe.command();

    asking.args(&argv);

    let Ok(()) = console_response_times::not_a_press(&mut asking);

    let done = match asking.output() {
        Ok(done) => done,
        Err(fault) => {
            eprintln!("music: {}: asking ffprobe: {fault}", path.display());

            return Ok(String::new());
        },
    };

    Ok(String::from_utf8_lossy(&done.stdout).to_string())
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Playing {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub length: f64,
}

pub fn played(said: &str) -> Result<Playing, Never> {
    let Ok(tags) = read(said);
    let Ok(all) = every(said);

    let first = |wanted: &str| {
        all.iter().find(|(name, _said)| name == wanted).map(|(_name, said)| said.clone())
    };

    let Ok(length) = length(said);

    Ok(Playing {
        title: tags.title,
        artist: tags.artist,
        album: first("album").unwrap_or_default(),
        length,
    })
}

fn length(said: &str) -> Result<f64, Never> {
    let held = match serde_json::from_str::<Value>(said) {
        Ok(held) => held,
        Err(_not_json) => return Ok(0.0),
    };

    let told = held.get("format").and_then(|format| format.get("duration"));

    Ok(match told.and_then(Value::as_str).map(str::parse::<f64>) {
        Some(Ok(length)) => length,
        Some(Err(_)) | None => 0.0,
    })
}

pub fn every(said: &str) -> Result<Vec<(String, String)>, Never> {
    let mut all: Vec<(String, String)> = Vec::new();

    let held = match serde_json::from_str::<Value>(said) {
        Ok(held) => held,
        Err(_not_json) => return Ok(all),
    };

    match held.get("format").and_then(|format| format.get("tags")) {
        Some(tags) => gathered(tags, &mut all)?,
        None => {},
    }

    match held.get("streams").and_then(Value::as_array) {
        Some(streams) => {
            for stream in streams {
                match stream.get("codec_type").and_then(Value::as_str) == Some("audio") {
                    true => {},
                    false => continue,
                }

                match stream.get("tags") {
                    Some(tags) => gathered(tags, &mut all)?,
                    None => {},
                }
            }
        }
        None => {},
    }

    Ok(all)
}

pub fn read(said: &str) -> Result<Tags, Never> {
    let Ok(all) = every(said);

    let first = |wanted: &[&str]| {
        wanted.iter().find_map(|want| {
            all.iter().find(|(name, _)| name == want).map(|(_, said)| said.clone())
        })
    };
    let title = first(&["title"]).unwrap_or_default();
    let artist = first(&["artist", "album_artist"]).unwrap_or_default();
    let rest = rest(&all, &title, &artist)?;

    Ok(Tags { rest, title, artist })
}

fn gathered(tags: &Value, into: &mut Vec<(String, String)>) -> Result<(), Never> {
    let held = match tags.as_object() {
        Some(held) => held,
        None => return Ok(()),
    };

    for (name, value) in held {
        let said = match value.as_str() {
            Some(said) => said,
            None => continue,
        };

        let said = said.split_whitespace().collect::<Vec<&str>>().join(" ");
        let name = name.trim().to_lowercase();

        match said.is_empty() || into.iter().any(|(had, _)| *had == name) {
            true => continue,
            false => {},
        }

        into.push((name, said));
    }

    Ok(())
}

fn rest(all: &[(String, String)], title: &str, artist: &str) -> Result<String, Never> {
    let mut rest: Vec<&str> = Vec::new();

    for (name, said) in all {
        let known = said == title || said == artist || rest.contains(&said.as_str());

        match known || NOT_THE_MUSIC.contains(&name.as_str()) {
            true => continue,
            false => {},
        }

        rest.push(said);
    }

    cut(&rest.join(BETWEEN), AS_MUCH)
}

fn cut(said: &str, to: usize) -> Result<String, Never> {
    Ok(match said.char_indices().nth(to).and_then(|(at, _)| said.get(..at)) {
        Some(head) => head.trim_end().to_string(),
        None => said.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn an_opus() -> String {
        serde_json::json!({
            "streams": [
                {
                    "codec_type": "audio",
                    "tags": {
                        "ALBUM": "Favourite Worst Nightmare",
                        "ARTIST": "Arctic Monkeys",
                        "DATE": "20141225",
                        "ENCODER": "Lavf62.3.100",
                        "TITLE": "505"
                    }
                },
                { "codec_type": "video", "tags": { "comment": "Cover (front)" } }
            ],
            "format": { }
        })
        .to_string()
    }

    fn an_mp3() -> String {
        serde_json::json!({
            "streams": [
                { "codec_type": "audio", "tags": { "encoder": "Lavc58.13" } },
                {
                    "codec_type": "video",
                    "tags": { "title": "Album cover", "comment": "Other" }
                }
            ],
            "format": {
                "tags": {
                    "artist": "2PacVEVO",
                    "title": "2Pac - Changes ft. Talent",
                    "date": "20110705",
                    "purl": "https://www.youtube.com/watch?v=eXvBjCO19QY"
                }
            }
        })
        .to_string()
    }

    #[test]
    fn a_song_that_keeps_its_tags_in_the_stream_is_read_the_same_as_one_that_does_not() {
        let Ok(opus) = read(&an_opus());

        let Ok(mp3) = read(&an_mp3());

        assert_eq!(opus.title, "505");
        assert_eq!(opus.artist, "Arctic Monkeys");
        assert_eq!(mp3.title, "2Pac - Changes ft. Talent");
        assert_eq!(mp3.artist, "2PacVEVO");
    }

    #[test]
    fn what_the_cover_says_about_itself_is_not_what_the_song_says() {
        let tagless = serde_json::json!({
            "streams": [
                { "codec_type": "audio", "tags": { } },
                { "codec_type": "video", "tags": { "title": "Album cover" } }
            ],
            "format": { }
        });
        let Ok(tags) = read(&tagless.to_string());

        assert_eq!(tags, Tags::default());
        assert_eq!(tags.anything(), Ok(Said::Nothing));
    }

    #[test]
    fn the_rest_is_what_is_about_the_music() {
        let Ok(opus) = read(&an_opus());

        let Ok(mp3) = read(&an_mp3());

        assert_eq!(opus.rest, format!("Favourite Worst Nightmare{BETWEEN}20141225"));
        assert_eq!(mp3.rest, "20110705");
    }

    #[test]
    fn what_is_said_twice_is_kept_once_and_on_one_line() {
        let said = serde_json::json!({
            "format": {
                "tags": {
                    "album_artist": "Nujabes",
                    "artist": "Nujabes",
                    "comment": "Provided to YouTube\n\nby Hydeout",
                    "synopsis": "Provided to YouTube\n\nby Hydeout"
                }
            }
        });
        let Ok(tags) = read(&said.to_string());

        assert_eq!(tags.rest, "Provided to YouTube by Hydeout");
    }

    #[test]
    fn a_description_nobody_would_read_is_cut_where_it_stops_being_worth_it() {
        let said = serde_json::json!({
            "format": { "tags": { "comment": "x".repeat(1000) } }
        });
        let Ok(tags) = read(&said.to_string());

        assert_eq!(tags.rest.chars().count(), AS_MUCH);
    }

    #[test]
    fn a_file_that_could_not_be_asked_says_nothing() {
        assert_eq!(read(""), Ok(Tags::default()));
        assert_eq!(read("not json"), Ok(Tags::default()));
    }

    #[test]
    fn a_name_ffprobe_would_read_as_a_flag_is_handed_to_it_as_a_file() {
        let Ok(argv) = asking(Path::new("/home/x/-Rain.opus"));

        assert_eq!(argv.last().unwrap(), "/home/x/-Rain.opus");
        assert_eq!(argv[argv.len() - 2], "-i");
    }
}
