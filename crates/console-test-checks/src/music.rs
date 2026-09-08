//! The music: a song pressed is the library playing, and it stays the library.
//!
//! It starts by asking whether anything is playing, and stands down when
//! something is. What it does to press this is stop whoever holds the player
//! and start one of its own on a song it chose, and a check cannot put a song
//! back where it was in somebody's afternoon -- the position is gone and so is
//! whatever they had queued. So the machine listening to something is the one
//! machine this is not asked of, which costs a skipped line and a sentence
//! saying why.
//!
//! What it asks for is the player's own vocabulary, imported rather than
//! spelled. The bus name moved when the player stopped being kew and this did
//! not, and a name nobody holds is answered by busctl with "not activatable"
//! -- which is a sentence about the bus and not about the music, so the check
//! went red naming the wrong thing twice: once when it asked whether anything
//! was already playing, and again when it asked the player it had just started
//! to say so.
//!
//! What it stands down for is a song being played and not a player being up.
//! The panel starts the player and leaves it standing when the song ends, so a
//! process is on every machine somebody has opened the Music panel on, and a
//! guard that asked for one would have stood every run of this down and gone
//! quiet rather than red -- which is the worse of the two. `PlaybackStatus` is
//! the question the sentence above promises: a stopped player is not somebody's
//! afternoon and this may take it.

use std::collections::BTreeSet;

use console_core_never::Never;
use console_music_player::answers::{NAME, OBJECT, PLAYER, Status};
use console_test_stages::checking::{Body, Check, Done, Why, cannot, failed};
use console_test_stages::device::{Device, PATIENCE, Seen};

pub const LIBRARY: Check = Check {
    name: "280-a-song-pressed-plays-the-library",
    about: "A song pressed plays, and what follows it is the rest of the library, once each.",
    feature: "music",
    since: "2026-09-03",
    bodies: &[Body::Device(there)],
};

pub const QUIET: Check = Check {
    name: "281-music-paused-lets-the-sound-go",
    about: "Paused, the player lets the sink go, so a machine holding a paused song is at rest.",
    feature: "music",
    since: "2026-09-08",
    bodies: &[Body::Device(quiet)],
};

const WALK: usize = 6;

const SWITCH: f64 = 1.5;

const SETTLING: f64 = 12.0;

const KINDS: &str = "flac mp3 opus m4a ogg wav";

const PROGRAM: &str = "music-player";

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

fn quiet(stage: &mut Device) -> Done {
    let Ok(playing) = playing(stage);

    match playing {
        Seen::Yes => {
            return cannot(
                "this machine is already playing something, and the check would have to stop it",
            );
        }
        Seen::NotYet => {},
    }

    let Ok(rested) = at_rest(stage);

    match rested {
        Seen::Yes => {},
        Seen::NotYet => {
            return cannot("something else on this machine is already holding the sound open");
        }
    }

    let Ok(songs) = library(stage);

    match songs.is_empty() {
        true => return cannot("this machine has no music on it to play"),
        false => {}
    }

    let first = songs.first().ok_or(Why::Cannot("no songs".to_string()))?;

    playing_the_library(stage, first)?;

    let Ok(_) = stage.until(sounding, PATIENCE);
    let Ok(heard) = sounding(stage);

    match heard {
        Seen::Yes => {}
        Seen::NotYet => {
            let Ok(()) = ended(stage);

            return cannot("nothing came out of this machine while a song was playing");
        }
    }

    let Ok(_) = stage.user(&format!("busctl --user call {NAME} {OBJECT} {PLAYER} Pause"));
    let Ok(_) = stage.until(at_rest, SETTLING);
    let Ok(rested) = at_rest(stage);

    let Ok(()) = ended(stage);

    match rested {
        Seen::Yes => Ok(()),
        Seen::NotYet => failed(format!(
            "the song was paused and a sink was still running {SETTLING} seconds later, \
             so the machine goes on driving the speakers with nothing to play"
        )),
    }
}

fn sounding(stage: &mut Device) -> Result<Seen, Never> {
    let Ok(said) = stage.user("pactl list short sinks");

    Ok(match said.contains("RUNNING") {
        true => Seen::Yes,
        false => Seen::NotYet,
    })
}

fn at_rest(stage: &mut Device) -> Result<Seen, Never> {
    let Ok(heard) = sounding(stage);

    Ok(match heard {
        Seen::Yes => Seen::NotYet,
        Seen::NotYet => Seen::Yes,
    })
}

fn playing(stage: &mut Device) -> Result<Seen, Never> {
    let Ok(word) = Status::Playing.said();
    let Ok(said) = stage.user(&format!(
        "busctl --user get-property {NAME} {OBJECT} {PLAYER} PlaybackStatus 2>&1"
    ));

    Ok(match said.trim() == format!("s \"{word}\"") {
        true => Seen::Yes,
        false => Seen::NotYet,
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

    let Ok(()) = ended(stage);

    let Ok(_) = stage.user(&format!(
        "systemd-run --user --collect --unit={UNIT} --quiet {PROGRAM} \"$HOME/Music\""
    ));
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

const UNIT: &str = "console-check-music-player";

fn ended(stage: &mut Device) -> Result<(), Never> {
    let Ok(_) =
        stage.user(&format!(
            "systemctl --user stop {UNIT}.service 2>/dev/null; pkill -x {PROGRAM}"
        ));

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

    title_in(&said)
}

fn title_in(said: &str) -> Result<String, Never> {
    let after = match said.split_once("\"xesam:title\"") {
        Some(after) => after,
        None => return Ok(String::new()),
    };

    let mut quoted = after.1.split('"');
    quoted.next();

    Ok(quoted.next().unwrap_or_default().to_string())
}

fn single_quoted(word: &str) -> Result<String, Never> {
    Ok(format!("'{}'", word.replace('\'', r"'\''")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_playing_is_read_out_of_what_the_bus_said() {
        let said = r#"a{sv} 2 "xesam:title" s "A Song" "mpris:length" x 1"#;
        assert_eq!(title_in(said), Ok("A Song".to_string()));
    }

    #[test]
    fn a_player_that_will_not_say_the_file_says_nothing() {
        assert_eq!(title_in(r#"a{sv} 1 "mpris:length" x 1"#), Ok(String::new()));
        assert_eq!(title_in(""), Ok(String::new()));
    }

    #[test]
    fn a_name_with_a_quote_in_it_is_still_one_word() {
        assert_eq!(single_quoted("don't"), Ok(r"'don'\''t'".to_string()));
    }
}
