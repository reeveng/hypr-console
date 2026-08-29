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

/// Play a song or a whole directory, whatever was playing before.
///
/// A player that cannot be told is started again holding the new thing. Every
/// kew answers to `--noui` and only ours answers to OpenUri, so the panel works
/// against the one in the repositories and gets a playlist that never stops
/// once the fork is on the machine.

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
pub fn opening(path: &std::path::Path) -> Vec<String> {
    let path = single_quoted(&path.to_string_lossy());
    let told = format!("busctl --user call {NAME} {OBJECT} {PLAYER} OpenUri s {path}");
    let again = format!("pkill -x kew; exec kew --noui {path}");
    vec!["sh".to_string(), "-c".to_string(), format!("{told} 2>/dev/null || {{ {again} }}")]
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
    #[test]
    fn a_name_the_shell_would_read_as_words_stays_one_word() {
        let argv = opening(std::path::Path::new("/home/x/Don't Stop.mp3"));
        assert_eq!(argv[0], "sh");
        assert!(argv[2].contains(r"'/home/x/Don'\''t Stop.mp3'"));
    }

    /// The player that will not take a path is replaced by one that has it,
    /// which is the kew in the repositories.
    #[test]
    fn a_player_that_will_not_be_told_is_started_again() {
        let argv = opening(std::path::Path::new("/music"));
        assert!(argv[2].contains("OpenUri"));
        assert!(argv[2].contains("pkill -x kew"));
        assert!(argv[2].contains("kew --noui"));
    }

    #[test]
    fn a_player_that_cannot_be_asked_is_not_playing() {
        assert!(Playing::default().stopped);
    }

    fn metadata() -> Value {
        serde_json::json!({
            "xesam:title": {"type": "s", "data": "505"},
            "xesam:artist": {"type": "as", "data": ["Arctic Monkeys"]},
            "xesam:album": {"type": "s", "data": "Favourite Worst Nightmare"},
            "mpris:artUrl": {"type": "s", "data": "file:///tmp/kew/cover.jpg"}
        })
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
