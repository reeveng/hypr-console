//! What a song says about itself.
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

use console_panel::running::said;
use serde_json::Value;

/// What is written between the things a song says that have no name of their
/// own.
pub const BETWEEN: &str = " \u{00b7} ";

/// How much of the rest is worth keeping.
///
/// A song fetched off YouTube carries the whole of the description somebody
/// wrote under it: a label, a producer, four links and a hashtag, a thousand
/// characters of it. It is worth searching and it is not worth a megabyte of
/// index, so this is where it is cut.
pub const AS_MUCH: usize = 300;

/// What a file says that is about the file rather than about the music.
///
/// `synopsis` and `description` are the same thing yt-dlp already wrote into
/// `comment`, word for word, so keeping them is the same sentence three times
/// in one row of the index.
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

/// What a song says, as much of it as is worth keeping.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    /// Everything else it says: the album, the year, whoever put it up.
    pub rest: String,
}

impl Tags {
    /// Whether the file said anything at all.
    pub fn anything(&self) -> bool {
        !(self.title.is_empty() && self.artist.is_empty() && self.rest.is_empty())
    }
}

/// The words that ask what one file says.
///
/// `-i` rather than a bare path, because a song whose name begins with a dash
/// is a song, and ffprobe would otherwise read it as a flag it does not have.
pub fn asking(path: &Path) -> Vec<String> {
    let word = |said: &str| said.to_string();
    vec![
        word("ffprobe"),
        word("-v"),
        word("quiet"),
        // The tags and what each stream is, and nothing else. Everything ffprobe
        // would otherwise work out about a file is time spent on a question
        // nobody here is asking.
        word("-show_entries"),
        word("format_tags:stream=codec_type:stream_tags"),
        word("-of"),
        word("json"),
        word("-i"),
        path.to_string_lossy().to_string(),
    ]
}

/// What one file says, asked of it.
pub fn of(path: &Path) -> Tags {
    let argv = asking(path);
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();
    read(&said(&words))
}

/// What ffprobe said, as what the song says.
pub fn read(said: &str) -> Tags {
    let mut all: Vec<(String, String)> = Vec::new();
    let Ok(held) = serde_json::from_str::<Value>(said) else { return Tags::default() };

    if let Some(tags) = held.get("format").and_then(|format| format.get("tags")) {
        gathered(tags, &mut all);
    }
    if let Some(streams) = held.get("streams").and_then(Value::as_array) {
        for stream in streams {
            // The cover is a stream too, and its tags say "Album cover" under
            // the same name a song says its title under.
            if stream.get("codec_type").and_then(Value::as_str) != Some("audio") {
                continue;
            }
            if let Some(tags) = stream.get("tags") {
                gathered(tags, &mut all);
            }
        }
    }
    let first = |wanted: &[&str]| {
        wanted.iter().find_map(|want| {
            all.iter().find(|(name, _)| name == want).map(|(_, said)| said.clone())
        })
    };
    // Whoever made it, or, where nobody says that, whoever the record is
    // filed under. A compilation says only the second and a song off one is
    // still worth finding by it.
    let title = first(&["title"]).unwrap_or_default();
    let artist = first(&["artist", "album_artist"]).unwrap_or_default();
    Tags { rest: rest(&all, &title, &artist), title, artist }
}

/// The names and what they said, in the order they were met, lower case and
/// on one line.
///
/// One line because a description is a paragraph with blank lines in it, and
/// what is being made here is a row of an index that a word is looked for in.
fn gathered(tags: &Value, into: &mut Vec<(String, String)>) {
    let Some(held) = tags.as_object() else { return };

    for (name, value) in held {
        let Some(said) = value.as_str() else { continue };
        let said = said.split_whitespace().collect::<Vec<&str>>().join(" ");
        let name = name.trim().to_lowercase();
        if said.is_empty() || into.iter().any(|(had, _)| *had == name) {
            continue;
        }
        into.push((name, said));
    }
}

/// Everything the song says that is not its title or whose it is.
///
/// Said once each: an album artist that is the artist, or a title said twice
/// under two names, is one word in the haystack rather than two.
fn rest(all: &[(String, String)], title: &str, artist: &str) -> String {
    let mut rest: Vec<&str> = Vec::new();

    for (name, said) in all {
        let known = said == title || said == artist || rest.contains(&said.as_str());
        if known || NOT_THE_MUSIC.contains(&name.as_str()) {
            continue;
        }
        rest.push(said);
    }
    cut(&rest.join(BETWEEN), AS_MUCH)
}

/// A string, no longer than that many characters.
fn cut(said: &str, to: usize) -> String {
    match said.char_indices().nth(to) {
        Some((at, _)) => said[..at].trim_end().to_string(),
        None => said.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What ffprobe answers for a song fetched off YouTube as an opus: the
    /// tags in the audio stream, and a cover stapled on carrying tags of its
    /// own.
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

    /// And for an mp3, where the tags are the format's and the cover is a
    /// stream saying it is called "Album cover".
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
        assert_eq!(read(&an_opus()).title, "505");
        assert_eq!(read(&an_opus()).artist, "Arctic Monkeys");
        assert_eq!(read(&an_mp3()).title, "2Pac - Changes ft. Talent");
        assert_eq!(read(&an_mp3()).artist, "2PacVEVO");
    }

    /// The picture is a stream with tags on it, and one of them is called
    /// title. A song with no title of its own is not a song called Album
    /// cover.
    #[test]
    fn what_the_cover_says_about_itself_is_not_what_the_song_says() {
        let tagless = serde_json::json!({
            "streams": [
                { "codec_type": "audio", "tags": { } },
                { "codec_type": "video", "tags": { "title": "Album cover" } }
            ],
            "format": { }
        });
        assert_eq!(read(&tagless.to_string()), Tags::default());
        assert!(!read(&tagless.to_string()).anything());
    }

    /// The album and the year are worth looking in; the encoder and the link
    /// the file came from are not.
    #[test]
    fn the_rest_is_what_is_about_the_music() {
        assert_eq!(read(&an_opus()).rest, format!("Favourite Worst Nightmare{BETWEEN}20141225"));
        assert_eq!(read(&an_mp3()).rest, "20110705");
    }

    /// A description is the same paragraph three times under three names, and
    /// it arrives with the blank lines still in it.
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
        assert_eq!(read(&said.to_string()).rest, "Provided to YouTube by Hydeout");
    }

    /// A thousand characters of somebody's description in every row of the
    /// index is a megabyte read on every letter typed.
    #[test]
    fn a_description_nobody_would_read_is_cut_where_it_stops_being_worth_it() {
        let said = serde_json::json!({
            "format": { "tags": { "comment": "x".repeat(1000) } }
        });
        assert_eq!(read(&said.to_string()).rest.chars().count(), AS_MUCH);
    }

    #[test]
    fn a_file_that_could_not_be_asked_says_nothing() {
        assert_eq!(read(""), Tags::default());
        assert_eq!(read("not json"), Tags::default());
    }

    /// A song whose name begins with a dash is a song, not a flag.
    #[test]
    fn a_name_ffprobe_would_read_as_a_flag_is_handed_to_it_as_a_file() {
        let argv = asking(Path::new("/home/x/-Rain.opus"));
        assert_eq!(argv.last().unwrap(), "/home/x/-Rain.opus");
        assert_eq!(argv[argv.len() - 2], "-i");
    }
}
