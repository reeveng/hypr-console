//! The music: a song pressed is the library playing, and it stays the library.

use std::collections::BTreeSet;

use console_stage::checking::{Body, Check, Done, Why, cannot, failed};
use console_stage::device::Device;

/// A song pressed plays the library, and shuffling is only the order of it.
///
/// This is written because the thing it asks about broke twice without anything
/// saying so, and both times the player looked perfectly well: a song was
/// playing, the transport drew, the panel was right about everything on it. What
/// was wrong was what came *after* the song, which nothing here could see.
///
/// The first was a playlist of one. A song started by name is the only song in
/// the list, so next had nowhere to go and the evening ended when the song did.
/// The second was worse and looked identical: the list was the whole library
/// until shuffle was pressed off, at which point the player put the list back to
/// one that had been emptied, and the song playing was left with nothing behind
/// it -- or, depending on when the drawing thread looked, no player at all.
///
/// Neither is visible in one moment. Both are only visible in a sequence, which
/// is what this is: press next as many times as there are songs to walk, and ask
/// that they were that many different songs. Done three times, because the fault
/// was in the switching: as the press left it, with shuffling on, and with
/// shuffling turned off again.
pub const LIBRARY: Check = Check {
    name: "280-a-song-pressed-plays-the-library",
    about: "A song pressed plays, and what follows it is the rest of the library, once each.",
    feature: "music",
    since: "2026-09-03",
    bodies: &[Body::Device(there)],
};

/// How far to walk, at most. Short enough that the check is not the length of
/// an album, and the walk is the library's length when the library is shorter:
/// a pass that stops before the end says nothing about the end, and a pass
/// asked for more songs than there are would fail on the wrap it should make.
const WALK: usize = 6;


/// How long a song takes to become the song playing.
///
/// Next is a message, not a function: it returns before the decoder has opened
/// the file, and asking too early gets the song that was playing. Read from the
/// player rather than waited out, but a floor is still wanted.
const SWITCH: f64 = 1.5;

/// What a song is kept in, as far as this is concerned.
///
/// One string rather than a list of them because a list of quoted words that
/// begins with the name of a program on the machine reads, to the net in
/// `the_programs.rs`, exactly like a program being run. It is right to read it
/// that way, and this is not one: `flac` here is a file's ending.
const KINDS: &str = "flac mp3 opus m4a ogg wav";

const NAME: &str = "org.mpris.MediaPlayer2.kew";
const OBJECT: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

fn there(stage: &mut Device) -> Done {
    let songs = library(stage);

    if songs.is_empty() {
        return cannot("this machine has no music on it to play");
    }

    // The walk is the library when the library is short. Whatever is here is
    // what a pass over it means: one song is a pass of one, two songs is two
    // and then round, and neither is a smaller version of the fault -- a list
    // that has collapsed onto the song playing repeats inside a pass of two as
    // plainly as inside a pass of six. Asking for more presses than there are
    // songs would fail on the wrap, which is the player doing the right thing.
    let far = songs.len().min(WALK);

    let first = songs.first().ok_or(Why::Cannot("no songs".to_string()))?;

    playing_the_library(stage, first)?;

    // As the press leaves it, then with the button pressed, then pressed back.
    // The last of those is the one that was broken, and it is broken in a way
    // the first two cannot see.
    //
    // Both presses send the same call, and that is not a slip. Setting Shuffle
    // on this player is a press of the button and not a value being given: it
    // takes the property being written as the key going down and flips what it
    // has. So `true` twice is the button pressed and pressed again, and asking
    // for `false` the second time would be the same press either way.
    let asked = [
        ("as the press left it", false),
        ("with shuffle pressed", true),
        ("with shuffle pressed back", true),
    ];

    for (when, press) in asked {
        if press {
            stage.user(&format!(
                "busctl --user set-property {NAME} {OBJECT} {PLAYER} Shuffle b true"
            ));
            stage.settle(SWITCH);
        }

        let walked = walk(stage, far);
        let different: BTreeSet<&String> = walked.iter().collect();

        if walked.iter().any(String::is_empty) {
            ended(stage);
            return failed(format!("{when}, the player stopped saying what it was playing"));
        }

        if different.len() != walked.len() {
            ended(stage);
            return failed(format!(
                "{when}, {} presses of next played {} different songs: {walked:?}",
                walked.len(),
                different.len()
            ));
        }
    }

    ended(stage);
    Ok(())
}

/// Every song on the machine, as the panel's library would have them.
///
/// Asked of the folder rather than of the player, because the player is what is
/// being checked: a library read out of kew would agree with kew about a list
/// that had collapsed to one song.
fn library(stage: &mut Device) -> Vec<String> {
    let named = KINDS.split(' ').map(|kind| format!("-iname '*.{kind}'")).collect::<Vec<_>>().join(" -o ");
    let found = stage.user(&format!("find \"$HOME/Music\" -type f \\( {named} \\) 2>/dev/null | sort"));

    found.lines().map(str::trim).filter(|line| !line.is_empty()).map(String::from).collect()
}

/// Start the player on a song, the way pressing A on one does.
///
/// Two steps because the panel is two steps: the song is opened, which for a
/// player that is not running means starting it with a name, and then the modes
/// are set and the song is opened again over the bus so that the list is built
/// out of the library around it. The second is `music-onward`, which is the
/// console's own program and is exactly what the panel hands to `later`.
fn playing_the_library(stage: &mut Device, song: &str) -> Done {
    let quoted = single_quoted(song);
    let name = single_quoted(stem(song));

    ended(stage);

    // Started as a unit of its own rather than in the background of the shell
    // that asks for it. Each thing this check runs on the machine is its own
    // login session, and a process left running in one does not outlive it:
    // started with `&`, the player is gone by the time the next line asks it
    // anything, and the whole walk then reads as a player that will not speak.
    stage.user(&format!(
        "systemd-run --user --collect --unit={UNIT} --quiet kew --noui {name}"
    ));
    stage.settle(3.0);

    // Asked for an answer and not merely for output. busctl prints its refusal
    // on the same stream, so a player that is not there says "the name is not
    // activatable" and an emptiness test reads that as the player answering.
    let answered = stage.user(&format!("busctl --user get-property {NAME} {OBJECT} {PLAYER} Shuffle"));

    if !answered.trim().starts_with("b ") {
        return failed(format!("the player did not start: it said {}", answered.trim()));
    }

    stage.user(&format!("music-onward {quoted}"));
    stage.settle(SWITCH * 2.0);

    Ok(())
}

/// The unit the player is run as, so that it can be stopped by name whatever
/// state it is in and whatever else on the machine is called kew.
const UNIT: &str = "console-check-kew";

/// Put the player away again. A check that plays music on somebody's machine
/// leaves it playing otherwise.
fn ended(stage: &mut Device) {
    stage.user(&format!("systemctl --user stop {UNIT}.service 2>/dev/null; pkill -x kew"));
}

/// Press next as far as the walk goes, and say what played each time.
fn walk(stage: &mut Device, far: usize) -> Vec<String> {
    let mut played = Vec::new();

    for _ in 0..far {
        played.push(song_playing(stage));
        stage.user(&format!("busctl --user call {NAME} {OBJECT} {PLAYER} Next"));
        stage.settle(SWITCH);
    }

    played
}

/// The file the player says it is playing, or nothing if it will not say.
///
/// `xesam:url` and not the title: two songs can share a title, and a list that
/// has collapsed onto one song would look like a list of different songs if the
/// tags were what was compared.
fn song_playing(stage: &mut Device) -> String {
    let said =
        stage.user(&format!("busctl --user get-property {NAME} {OBJECT} {PLAYER} Metadata"));

    url_in(&said)
}

/// The `xesam:url` out of what busctl printed.
///
/// Its dump is one long line of `"key" s "value"` pairs, so this takes what is
/// quoted after the key and stops there. Written as a function of the text so
/// that it can be tested without a player.
fn url_in(said: &str) -> String {
    let Some(after) = said.split_once("\"xesam:url\"") else { return String::new() };

    let mut quoted = after.1.split('"');
    quoted.next();

    quoted.next().unwrap_or_default().to_string()
}

/// A song's name without the extension, which is what a kew that will not take
/// a path has to be given.
fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);

    match name.rsplit_once('.') {
        Some((before, _)) => before,
        None => name,
    }
}

/// A word the shell will hand on whole, quotes in it and all.
fn single_quoted(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_playing_is_read_out_of_what_the_bus_said() {
        let said = r#"a{sv} 3 "xesam:title" s "A Song" "xesam:url" s "file:///home/a/b.flac" "mpris:length" x 1"#;
        assert_eq!(url_in(said), "file:///home/a/b.flac");
    }

    /// A player that does not answer `xesam:url` is the packaged one, and the
    /// walk has to read as a stopped player rather than as six of the same
    /// song: six empty strings are six identical answers, and a check that only
    /// counted the different ones would call that a collapsed library.
    #[test]
    fn a_player_that_will_not_say_the_file_says_nothing() {
        assert_eq!(url_in(r#"a{sv} 1 "xesam:title" s "A Song""#), "");
        assert_eq!(url_in(""), "");
    }

    #[test]
    fn the_name_a_player_is_given_is_the_song_without_its_extension() {
        assert_eq!(stem("/home/a/Music/Album/1 - song.flac"), "1 - song");
        assert_eq!(stem("no-extension"), "no-extension");
        assert_eq!(stem("/a/b.c/song"), "song");
    }

    #[test]
    fn a_name_with_a_quote_in_it_is_still_one_word() {
        assert_eq!(single_quoted("don't"), r"'don'\''t'");
    }
}
