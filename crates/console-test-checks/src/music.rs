//! The music: a song pressed is the library playing, and it stays the library.
//!
//! It starts by asking whether anything is playing, and stands down when
//! something is. What it does to press this is stop whoever holds the player
//! and start one of its own on a song it chose, and a check cannot put a song
//! back where it was in somebody's afternoon -- the position is gone and so is
//! whatever they had queued. So the machine listening to something is the one
//! machine this is not asked of, which costs a skipped line and a sentence
//! saying why.

use std::collections::BTreeSet;

use console_core_never::Never;
use console_test_stages::checking::{Body, Check, Done, Why, cannot, failed};
use console_test_stages::device::{Device, PATIENCE, Seen};

pub const LIBRARY: Check = Check {
    name: "280-a-song-pressed-plays-the-library",
    about: "A song pressed plays, and what follows it is the rest of the library, once each.",
    feature: "music",
    since: "2026-09-03",
    bodies: &[Body::Device(there)],
};

const WALK: usize = 6;

const SWITCH: f64 = 1.5;

const KINDS: &str = "flac mp3 opus m4a ogg wav";

fn answering(seen: &mut Device) -> Result<Seen, Never> {
    let Ok(said) = shuffle(seen);

    Ok(match said.trim().starts_with("b ") {
        true => Seen::Yes,
        false => Seen::NotYet,
    })
}

fn shuffling(seen: &mut Device) -> Result<Seen, Never> {
    let Ok(said) = shuffle(seen);

    Ok(match said.trim() == "b true" {
        true => Seen::Yes,
        false => Seen::NotYet,
    })
}

fn shuffle(seen: &mut Device) -> Result<String, Never> {
    seen.user(&format!("busctl --user get-property {NAME} {OBJECT} {PLAYER} Shuffle"))
}

const NAME: &str = "org.mpris.MediaPlayer2.kew";
const OBJECT: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

fn there(stage: &mut Device) -> Done {
    let Ok(playing) = playing(stage);

    match playing {
        Seen::Yes => {
            return cannot(
                "this machine is already playing something, and the check would have to stop it",
            );
        }
        Seen::NotYet => {},
    }

    let Ok(songs) = library(stage);

    match songs.is_empty() {
        true => return cannot("this machine has no music on it to play"),
        false => {}
    }

    let far = songs.len().min(WALK);

    let first = songs.first().ok_or(Why::Cannot("no songs".to_string()))?;

    playing_the_library(stage, first)?;

    let asked = [
        ("as the press left it", false),
        ("with shuffle pressed", true),
        ("with shuffle pressed back", true),
    ];

    for (when, press) in asked {
        match press {
            true => {
                let Ok(_) = stage.user(&format!(
                    "busctl --user set-property {NAME} {OBJECT} {PLAYER} Shuffle b true"
                ));
                let Ok(_) = stage.until(shuffling, SWITCH);
            }
            false => {}
        }

        let Ok(walked) = walk(stage, far);
        let different: BTreeSet<&String> = walked.iter().collect();

        match walked.iter().any(String::is_empty) {
            true => {
                let Ok(()) = ended(stage);
                return failed(format!("{when}, the player stopped saying what it was playing"));
            }
            false => {}
        }

        match different.len() == walked.len() {
            true => {}
            false => {
                let Ok(()) = ended(stage);
                return failed(format!(
                    "{when}, {} presses of next played {} different songs: {walked:?}",
                    walked.len(),
                    different.len()
                ));
            }
        }
    }

    let Ok(()) = ended(stage);
    Ok(())
}

fn playing(stage: &mut Device) -> Result<Seen, Never> {
    let Ok(said) = stage.user("pgrep -x kew || true");

    Ok(match said.trim().is_empty() {
        true => Seen::NotYet,
        false => Seen::Yes,
    })
}

fn library(stage: &mut Device) -> Result<Vec<String>, Never> {
    let named =
        KINDS.split(' ').map(|kind| format!("-iname '*.{kind}'")).collect::<Vec<_>>().join(" -o ");
    let Ok(found) =
        stage.user(&format!("find \"$HOME/Music\" -type f \\( {named} \\) 2>/dev/null | sort"));

    Ok(found.lines().map(str::trim).filter(|line| !line.is_empty()).map(String::from).collect())
}

fn playing_the_library(stage: &mut Device, song: &str) -> Done {
    let Ok(quoted) = single_quoted(song);
    let Ok(stem) = stem(song);
    let Ok(name) = single_quoted(stem);

    let Ok(()) = ended(stage);

    let Ok(_) = stage
        .user(&format!("systemd-run --user --collect --unit={UNIT} --quiet kew --noui {name}"));
    let Ok(_) = stage.until(answering, PATIENCE);
    let Ok(answered) = shuffle(stage);

    match answered.trim().starts_with("b ") {
        true => {}
        false => {
            return failed(format!("the player did not start: it said {}", answered.trim()));
        }
    }

    let Ok(was) = song_playing(stage);
    let Ok(_) = stage.user(&format!("music-onward {quoted}"));
    let Ok(_) = stage.changed(song_playing, &was, SWITCH);

    Ok(())
}

const UNIT: &str = "console-check-kew";

fn ended(stage: &mut Device) -> Result<(), Never> {
    let Ok(_) =
        stage.user(&format!("systemctl --user stop {UNIT}.service 2>/dev/null; pkill -x kew"));

    Ok(())
}

fn walk(stage: &mut Device, far: usize) -> Result<Vec<String>, Never> {
    let mut played = Vec::new();

    for _ in 0..far {
        let Ok(playing) = song_playing(stage);

        played.push(playing.clone());

        let Ok(_) = stage.user(&format!("busctl --user call {NAME} {OBJECT} {PLAYER} Next"));
        let Ok(_) = stage.changed(song_playing, &playing, SWITCH);
    }

    Ok(played)
}

fn song_playing(stage: &mut Device) -> Result<String, Never> {
    let Ok(said) =
        stage.user(&format!("busctl --user get-property {NAME} {OBJECT} {PLAYER} Metadata"));

    url_in(&said)
}

fn url_in(said: &str) -> Result<String, Never> {
    let Some(after) = said.split_once("\"xesam:url\"") else { return Ok(String::new()) };

    let mut quoted = after.1.split('"');
    quoted.next();

    Ok(quoted.next().unwrap_or_default().to_string())
}

fn stem(path: &str) -> Result<&str, Never> {
    let name = path.rsplit('/').next().unwrap_or(path);

    Ok(match name.rsplit_once('.') {
        Some((before, _)) => before,
        None => name,
    })
}

fn single_quoted(word: &str) -> Result<String, Never> {
    Ok(format!("'{}'", word.replace('\'', r"'\''")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_playing_is_read_out_of_what_the_bus_said() {
        let said = r#"a{sv} 3 "xesam:title" s "A Song" "xesam:url" s "file:///home/a/b.flac" "mpris:length" x 1"#;
        assert_eq!(url_in(said), Ok("file:///home/a/b.flac".to_string()));
    }

    #[test]
    fn a_player_that_will_not_say_the_file_says_nothing() {
        assert_eq!(url_in(r#"a{sv} 1 "xesam:title" s "A Song""#), Ok(String::new()));
        assert_eq!(url_in(""), Ok(String::new()));
    }

    #[test]
    fn the_name_a_player_is_given_is_the_song_without_its_extension() {
        assert_eq!(stem("/home/a/Music/Album/1 - song.flac"), Ok("1 - song"));
        assert_eq!(stem("no-extension"), Ok("no-extension"));
        assert_eq!(stem("/a/b.c/song"), Ok("song"));
    }

    #[test]
    fn a_name_with_a_quote_in_it_is_still_one_word() {
        assert_eq!(single_quoted("don't"), Ok(r"'don'\''t'".to_string()));
    }
}
