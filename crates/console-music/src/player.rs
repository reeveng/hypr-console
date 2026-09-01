//! The player, asked and told over MPRIS.
//!
//! kew runs headless with `--noui` and answers on the session bus as
//! `org.mpris.MediaPlayer2.kew`. Everything a surface needs is a property or a
//! method there, so nothing here reads a file kew wrote.

use std::path::PathBuf;

use console_panel::running::said;
use serde_json::Value;

/// The name kew answers to.
pub const NAME: &str = "org.mpris.MediaPlayer2.kew";

const OBJECT: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

/// What is playing, as far as the player will say.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Playing {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art: Option<PathBuf>,
    /// The file the song is, where the player says which one it is.
    ///
    /// The one thing about a song that cannot be worked out from the rest:
    /// two files can share a title, an artist and an album, and this library
    /// has several that do. Nothing draws it -- it is what Y hands to the
    /// files panel, so the song on now can be renamed or thrown away without
    /// going and finding it in a list of nine hundred.
    ///
    /// `xesam:url` is what MPRIS calls it. Ours says it; the kew in the
    /// repositories does not, and a row with nothing to offer offers nothing,
    /// which is why this is an option rather than a string.
    pub path: Option<PathBuf>,
    pub paused: bool,
    pub stopped: bool,
}

/// Nothing, which is what a player that will not answer amounts to.
impl Default for Playing {
    fn default() -> Self {
        Playing {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            art: None,
            path: None,
            paused: false,
            stopped: true,
        }
    }
}

/// Whether the player is there to be asked.
pub fn about() -> bool {
    said(&["busctl", "--user", "--no-legend", "list"]).contains(NAME)
}

/// What the player is playing.
pub fn playing() -> Option<Playing> {
    let status = property("PlaybackStatus")?;
    let metadata = property("Metadata")?;
    Some(read(&status, &metadata))
}

/// A property, as busctl prints it.
fn property(name: &str) -> Option<Value> {
    let said = said(&["busctl", "--user", "--json=short", "get-property", NAME, OBJECT, PLAYER, name]);
    serde_json::from_str::<Value>(&said).ok()?.get("data").cloned()
}

/// What those two properties come to.
pub fn read(status: &Value, metadata: &Value) -> Playing {
    let said = |key: &str| match metadata.get(key).and_then(|held| held.get("data")) {
        Some(Value::String(one)) => one.clone(),
        Some(Value::Array(many)) => many
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        _ => String::new(),
    };
    let state = status.as_str().unwrap_or_default();

    Playing {
        title: said("xesam:title"),
        artist: said("xesam:artist"),
        album: said("xesam:album"),
        art: local(&said("mpris:artUrl")),
        path: local(&said("xesam:url")),
        paused: state == "Paused",
        stopped: state == "Stopped" || state.is_empty(),
    }
}

/// A `file://` URI, as a path this machine can open.
pub fn local(url: &str) -> Option<PathBuf> {
    let path = url.strip_prefix("file://")?;
    let path = PathBuf::from(unescaped(path));
    path.exists().then_some(path)
}

/// A URI's percent escapes, put back.
fn unescaped(said: &str) -> String {
    let mut out = String::with_capacity(said.len());
    let mut letters = said.chars();

    while let Some(letter) = letters.next() {
        let escape = || {
            let (high, low) = (letters.clone().next()?, letters.clone().nth(1)?);
            let byte = u8::from_str_radix(&format!("{high}{low}"), 16).ok()?;
            Some(byte as char)
        };
        match letter {
            '%' => match escape() {
                Some(byte) => {
                    out.push(byte);
                    letters.next();
                    letters.next();
                }
                None => out.push(letter),
            },
            _ => out.push(letter),
        }
    }
    out
}

fn call(method: &str) {
    said(&["busctl", "--user", "call", NAME, OBJECT, PLAYER, method]);
}

/// What the player does when a song ends.
///
/// The three MPRIS has, under the names they mean here. Only two of them are
/// offered on the panel; the third is a state kew can be left in by its own
/// keyboard, and something read off the player has to be able to say so.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Over {
    /// Go on to the next song.
    #[default]
    On,
    /// Play this one again.
    Again,
    /// Go round the list again once it ends.
    Round,
}

impl Over {
    /// The three of them in the order the player walks them in.
    pub const ROUND: [Over; 3] = [Over::On, Over::Again, Over::Round];

    /// What MPRIS calls it.
    pub fn said(self) -> &'static str {
        match self {
            Over::On => "None",
            Over::Again => "Track",
            Over::Round => "Playlist",
        }
    }

    /// Which one the player means by a word.
    pub fn read(said: &str) -> Over {
        Over::ROUND.into_iter().find(|over| over.said() == said).unwrap_or_default()
    }

    /// Where it stands in the round.
    fn place(self) -> usize {
        Over::ROUND.into_iter().position(|over| over == self).unwrap_or_default()
    }
}

/// Whether the player is taking the songs in any order.
pub fn shuffling() -> bool {
    property("Shuffle").as_ref().and_then(Value::as_bool).unwrap_or_default()
}

/// What the player will do when this song ends.
pub fn over() -> Over {
    Over::read(property("LoopStatus").as_ref().and_then(Value::as_str).unwrap_or_default())
}

/// A property, pressed.
///
/// Not set. kew reads the name and never the value: setting LoopStatus is its
/// repeat key and setting Shuffle is its shuffle key, so a panel that said
/// "Track" to a player already repeating a track would turn repeating off.
/// This is written as the press it is, which is also why it works against the
/// kew in the repositories as well as against ours.
fn press(name: &str, kind: &str, value: &str) {
    said(&["busctl", "--user", "set-property", NAME, OBJECT, PLAYER, name, kind, value]);
}

/// How many presses of the repeat key it takes to get from one to the other.
pub fn presses(from: Over, to: Over) -> usize {
    (Over::ROUND.len() + to.place() - from.place()) % Over::ROUND.len()
}

/// Take the songs in any order, or in the order they are in.
///
/// Asked before it is told, for the same reason: the press is a flip, so a
/// player already taking them in any order would be put back in order by being
/// told to take them in any order. What is sent is still what is wanted, so a
/// player that does read the value gets it right in one.
pub fn shuffle(any_order: bool) {
    if shuffling() != any_order {
        press("Shuffle", "b", &any_order.to_string());
    }
}

/// Play this song over, or go on to the next one when it ends.
///
/// Asked before it is told, because the key is a round of three and where it
/// leaves the player depends on where the player was. Going on is `On` rather
/// than `Round`: the panel offers two modes, and the one that is not repeating
/// a song is the one where a list plays through.
pub fn repeat(one: bool) {
    let wanted = match one {
        true => Over::Again,
        false => Over::On,
    };
    for _ in 0..presses(over(), wanted) {
        press("LoopStatus", "s", wanted.said());
    }
}

/// Play what is loaded, or stop playing it.
pub fn play_pause() {
    call("PlayPause");
}

pub fn next() {
    call("Next");
}

pub fn previous() {
    call("Previous");
}

/// How many microseconds of the song have been played.
///
/// Zero is the honest answer to a song that has not started or one the player
/// does not know how long it is. The two numbers a bar wants -- how long the
/// song is, and where the dot is -- are the only ones worth asking for.
pub fn position() -> i64 {
    property("Position").as_ref().and_then(Value::as_i64).unwrap_or_default()
}

/// How long the song is, in microseconds.
///
/// Zero for a player that does not say -- which means the bar has nothing to
/// draw a fraction of, and the dot sits at the start until somebody asks.
pub fn length() -> i64 {
    property("Metadata")
        .as_ref()
        .and_then(|metadata| metadata.get("data"))
        .and_then(|data| data.get("mpris:length"))
        .and_then(|held| held.get("data"))
        .and_then(Value::as_i64)
        .unwrap_or_default()
}

/// Jump to this fraction of the song, where 0 is the start and 1 the end.
///
/// Asked in fractions rather than microseconds so the panel can hand the
/// same shape to the d-pad, to a tap on the bar, and to a future volume
/// rocker. The fraction is the only thing that survives a song change.
pub fn seek(fraction: f64) {
    let Some(total) = std::num::NonZeroI64::new(length()) else { return };
    let at = (fraction.clamp(0.0, 1.0) * total.get() as f64) as i64;
    let id = track_id();
    said(&[
        "busctl", "--user", "call", NAME, OBJECT, PLAYER, "SetPosition",
        "o", &id, "x", &at.to_string(),
    ]);
}

/// The track the player is on, said as the object path MPRIS calls it by.
///
/// `SetPosition` takes the track id and the microseconds; the track id is
/// what changes between songs, so it has to be asked each time. A player
/// without one is a player we cannot seek through, which is the same as a
/// seek that does nothing.
fn track_id() -> String {
    property("Metadata")
        .as_ref()
        .and_then(|metadata| metadata.get("data"))
        .and_then(|data| data.get("mpris:trackid"))
        .and_then(|held| held.get("data"))
        .and_then(Value::as_str)
        .unwrap_or("/")
        .to_string()
}

/// What to run to play this.
///
/// An argv rather than a thing done here, because the panel hands it to
/// `Showing::later` and everything below takes longer than a drawing thread
/// has: a player that is not running has to be started, and one that is
/// running has to be asked over D-Bus and killed when it will not answer.
///
/// Every kew answers to `--noui` and only ours answers to OpenUri, so this
/// works against the one in the repositories and gets a playlist that never
/// stops once the fork is on the machine.
pub fn opening(path: &std::path::Path, folder: bool) -> Vec<String> {
    let where_ = single_quoted(&path.to_string_lossy());
    let name = single_quoted(&sought(path, folder));
    let told = format!("busctl --user call {NAME} {OBJECT} {PLAYER} OpenUri s {where_}");
    let again = format!("pkill -x kew; exec kew --noui {name}");
    // The semicolon before the brace is the whole of whether this runs. A
    // brace group in sh ends at `; }` and not at ` }`: without it the closing
    // brace is read as another word for kew, the group is never closed, and
    // the line dies with "syntax error: unexpected end of file" before a note
    // of it is played. Nothing said so, because what this is handed to sends
    // its errors to /dev/null, so pressing A on a song did nothing and the
    // journal agreed that nothing had happened.
    vec!["sh".to_string(), "-c".to_string(), format!("{told} 2>/dev/null || {{ {again}; }}")]
}

/// The word for a kew that will not take a path.
///
/// Its argument is looked for in the library it has indexed rather than opened
/// as a file, so the path to a song sitting in the music folder is answered
/// with "Music not found" and nothing plays. The name is what it looks in, so
/// the name is what it is given: the whole of a folder's, and everything
/// before the extension of a song's. The id a download leaves at the end of a
/// name is kept rather than tidied away, because it is the half that tells two
/// songs of the same title apart.
pub fn sought(path: &std::path::Path, folder: bool) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    match folder {
        true => name,
        false => name.rsplit_once('.').map_or(name.as_str(), |(stem, _)| stem).to_string(),
    }
}

/// A word the shell takes as one word, whatever is in it.
fn single_quoted(said: &str) -> String {
    format!("'{}'", said.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A name with a quote in it is a name somebody has, and the shell would
    /// otherwise read the rest of it as a command.
    /// The one test that would have caught it: the line is handed to a shell,
    /// so a shell is what says whether it is a line.
    ///
    /// Every other test here reads the string and agrees with itself about
    /// what is in it. This one asks `sh -n`, which parses and runs nothing,
    /// and it is the only kind of assertion that could tell that `{ ...; }`
    /// had been written `{ ... }`. That missing semicolon meant every press of
    /// A on a song died with "syntax error: unexpected end of file", quietly,
    /// because what runs this line throws its errors away.
    #[test]
    fn what_is_handed_to_a_shell_is_something_a_shell_can_read() {
        let awkward = [
            "/home/x/Don't Stop.mp3",
            "/home/x/Sweetness (Official Music Video) [0zzv0vYECWQ].opus",
            "/home/x/a; rm -rf $HOME/b.flac",
            "/home/x/quote\"and\"brace}.wav",
        ];
        for path in awkward {
            for folder in [true, false] {
                let argv = opening(std::path::Path::new(path), folder);
                let checked = std::process::Command::new("sh")
                    .arg("-n")
                    .arg("-c")
                    .arg(&argv[2])
                    .output()
                    .expect("a shell to ask");
                assert!(
                    checked.status.success(),
                    "sh cannot read the line for {path:?}: {}\n{}",
                    String::from_utf8_lossy(&checked.stderr),
                    argv[2]
                );
            }
        }
    }

    #[test]
    fn a_name_the_shell_would_read_as_words_stays_one_word() {
        let argv = opening(std::path::Path::new("/home/x/Don't Stop.mp3"), false);
        assert_eq!(argv[0], "sh");
        assert!(argv[2].contains(r"'/home/x/Don'\''t Stop.mp3'"));
        assert!(argv[2].contains(r"'Don'\''t Stop'"));
    }

    /// The player that will not take a path is replaced by one that has it,
    /// which is the kew in the repositories.
    #[test]
    fn a_player_that_will_not_be_told_is_started_again() {
        let argv = opening(std::path::Path::new("/music"), true);
        assert!(argv[2].contains("OpenUri"));
        assert!(argv[2].contains("pkill -x kew"));
        assert!(argv[2].contains("kew --noui"));
    }

    /// The one that takes a path takes the path, and the one that does not is
    /// given the name it can find. A folder keeps every letter of its name,
    /// because the dot in one of those is part of it.
    #[test]
    fn a_player_that_will_not_take_a_path_is_given_the_name() {
        let song = std::path::Path::new("/m/505 [qU9mHegkTc4].opus");
        assert_eq!(sought(song, false), "505 [qU9mHegkTc4]");
        assert_eq!(sought(std::path::Path::new("/m/Vol. 2"), true), "Vol. 2");
    }

    #[test]
    fn a_player_that_cannot_be_asked_is_not_playing() {
        assert!(Playing::default().stopped);
    }

    /// The player takes a set of either mode as a press of its own key and
    /// never looks at the value, so a mode is reached by counting presses.
    #[test]
    fn a_mode_is_as_many_presses_away_as_the_round_makes_it() {
        assert_eq!(presses(Over::On, Over::Again), 1);
        assert_eq!(presses(Over::Round, Over::Again), 2);
        assert_eq!(presses(Over::Round, Over::On), 1);
        assert_eq!(presses(Over::Again, Over::Again), 0);
    }

    /// Left repeating the whole list by kew's own keyboard, the panel says so
    /// by saying this song is not the one being repeated.
    #[test]
    fn what_the_player_says_about_the_end_of_a_song_is_read_back() {
        assert_eq!(Over::read("Track"), Over::Again);
        assert_eq!(Over::read("Playlist"), Over::Round);
        assert_eq!(Over::read("None"), Over::On);
        assert_eq!(Over::read(""), Over::On);
    }

    fn metadata() -> Value {
        serde_json::json!({
            "xesam:title": {"type": "s", "data": "505"},
            "xesam:artist": {"type": "as", "data": ["Arctic Monkeys"]},
            "xesam:album": {"type": "s", "data": "Favourite Worst Nightmare"},
            "mpris:artUrl": {"type": "s", "data": "file:///tmp/kew/cover.jpg"},
            "xesam:url": {"type": "s", "data": "file:///nowhere/505.opus"}
        })
    }

    /// The file itself, which is what Y over the song on now hands to the
    /// files. A player that does not say it leaves the row with nothing to
    /// offer rather than with a guess.
    #[test]
    fn a_player_that_says_which_file_it_is_playing_is_believed() {
        assert_eq!(read(&Value::Null, &metadata()).path, None, "a file that is not there");
        assert_eq!(read(&Value::Null, &serde_json::json!({})).path, None);
    }

    #[test]
    fn one_artist_is_taken_out_of_the_list_it_arrives_in() {
        let playing = read(&Value::String("Playing".into()), &metadata());
        assert_eq!(playing.artist, "Arctic Monkeys");
        assert_eq!(playing.title, "505");
        assert!(!playing.paused && !playing.stopped);
    }

    #[test]
    fn a_player_that_says_nothing_is_stopped() {
        let playing = read(&Value::Null, &serde_json::json!({}));
        assert!(playing.stopped);
        assert_eq!(playing.title, "");
    }

    #[test]
    fn a_name_with_a_space_in_it_survives_the_uri() {
        assert_eq!(unescaped("/home/a/505%20%5Bqu%5D.opus"), "/home/a/505 [qu].opus");
    }

    #[test]
    fn a_cover_that_is_not_there_is_no_cover() {
        assert_eq!(local("file:///nowhere/cover.jpg"), None);
        assert_eq!(local("https://example.com/cover.jpg"), None);
    }
}
