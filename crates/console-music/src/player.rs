//! The player, asked and told over MPRIS.
//!
//! kew runs headless with `--noui` and answers on the session bus as
//! `org.mpris.MediaPlayer2.kew`. Everything a surface needs is a property or a
//! method there, so nothing here reads a file kew wrote.


use console_number::{Float, toward_zero_i64};
use std::path::PathBuf;

use console_panel::running::said;

use crate::library::Kind;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum About {
    /// It is on the bus, so it can be asked and told.
    Yes,
    /// It is not running.
    No,
}

/// Whether the player is taking the songs in any order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Any order, which is what the shuffle row turns on.
    Any,
    /// The order they are in.
    AsListed,
}

/// Whether the player is there to be asked.
pub fn about() -> About {
    match said(&["busctl", "--user", "--no-legend", "list"]).contains(NAME) {
        true => About::Yes,
        false => About::No,
    }
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

    let Ok(held) = serde_json::from_str::<Value>(&said) else { return None };

    held.get("data").cloned()
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

            let Ok(byte) = u8::from_str_radix(&format!("{high}{low}"), 16) else { return None };

            Some(char::from(byte))
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
pub fn shuffling() -> Order {
    match property("Shuffle").as_ref().and_then(Value::as_bool).unwrap_or_default() {
        true => Order::Any,
        false => Order::AsListed,
    }
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
pub fn shuffle(any_order: Order) {
    if shuffling() != any_order {
        press("Shuffle", "b", &(any_order == Order::Any).to_string());
    }
}

/// Play this song over, or go on to the next one when it ends.
///
/// Asked before it is told, because the key is a round of three and where it
/// leaves the player depends on where the player was. Going on is `On` rather
/// than `Round`: the panel offers two modes, and the one that is not repeating
/// a song is the one where a list plays through.
pub fn repeat(wanted: Over) {
    for _ in 0..presses(over(), wanted) {
        press("LoopStatus", "s", wanted.said());
    }
}

/// How long a player that has just been started is given to answer.
///
/// It is a process being launched and a bus name being taken, and neither is
/// instant. Given up on rather than waited on for ever: the music is playing
/// either way by then, and the worst this can come to is a song that plays in
/// the order it was listed.
const COMES_UP: std::time::Duration = std::time::Duration::from_secs(6);

/// How often it is asked while it is coming up.
const BREATH: std::time::Duration = std::time::Duration::from_millis(150);

/// What choosing a song leaves the player playing: the whole library, in any
/// order, going round for ever, starting on the song that was chosen.
///
/// This is what a music player does when nobody has said otherwise. Choosing
/// one song and being handed silence four minutes later is the machine
/// stopping in the middle of the evening and waiting to be asked again; a
/// handheld that is being carried about is the last place anybody wants to go
/// back to the panel to hear a second song.
///
/// The two modes are pressed rather than set, which is what `shuffle` and
/// `repeat` already do about a player whose keys are flips. Either of them can
/// be turned off again from the transport, and turning one off is somebody
/// saying what they want rather than the machine having never decided.
///
/// They are pressed before the song is handed over, because the player builds
/// the list it is going to play at the moment it is given one: told to open a
/// song with shuffling on, the library falls in behind that song and every
/// other song in it plays once before any of them plays twice. Told with
/// shuffling off, the song is played where it stands and what is around it in
/// the folder is what comes next.
///
/// Then the song, again. The panel has already asked for it -- that is the
/// press answering at once -- and this asks a second time because the first
/// went to a player that was not there yet. Where it was there, the song
/// starts over a fraction of a second in, which is the cost of the two paths
/// being one.
pub fn onward(song: &std::path::Path) {
    if onward_only() == About::No {
        return;
    }

    open(song);
}

/// The two modes and no song, for a player that is already playing one.
///
/// Says whether there was a player there to press them on, so the caller
/// knows whether anything it does next has anybody to hear it.
pub fn onward_only() -> About {
    if waited_for() == About::No {
        return About::No;
    }

    shuffle(Order::Any);
    repeat(Over::Round);
    About::Yes
}

/// Hand the player a song, which is the player being told what to play.
///
/// One song to this end, and the whole library out of the other: the fork
/// answers `OpenUri` on a song by building the playlist out of the library
/// around it, so what a press of A means is *play this, and then everything
/// else*. The kew in the repositories has no `OpenUri` at all, and on that one
/// this says nothing and changes nothing.
///
/// The path is handed over as it is rather than as a URI. Ours takes an
/// absolute path as it stands, which saves escaping a filename here only to
/// unescape it there, and every filename in a music folder is somebody's
/// punctuation.
pub fn open(song: &std::path::Path) {
    said(&[
        "busctl", "--user", "call", NAME, OBJECT, PLAYER, "OpenUri",
        "s", &song.to_string_lossy(),
    ]);
}

/// What is run after a song is chosen, and the song it is run about.
///
/// Its own program rather than two more lines in the panel because it has to
/// wait, and a panel that waits is a panel that has stopped answering the
/// buttons.
pub fn onward_for(song: &std::path::Path) -> Vec<String> {
    vec!["music-onward".to_string(), song.to_string_lossy().to_string()]
}

/// Wait for the player to be there to be asked, or give up.
fn waited_for() -> About {
    let by = std::time::Instant::now() + COMES_UP;

    while std::time::Instant::now() < by {
        if about() == About::Yes {
            return About::Yes;
        }

        std::thread::sleep(BREATH);
    }

    About::No
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
    how_long(&property("Metadata").unwrap_or(Value::Null))
}

/// How long, out of the map the player answered with.
///
/// Apart from the asking so it can be tried against a map somebody wrote down.
/// It read one layer too deep for as long as it existed -- `property` has
/// already taken the `data` off what busctl said, and this took it off again,
/// which is a map that has no key of that name and so a length of nought. A
/// song of no length is a bar with nothing to divide by: the dot sat at the
/// start of every song, no song said how long it was, and seeking gave up
/// before it began. Nothing said any of that; it just looked like a player
/// that had only ever been told the time.
fn how_long(metadata: &Value) -> i64 {
    metadata
        .get("mpris:length")
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

    let at = toward_zero_i64(fraction.clamp(0.0, 1.0) * total.get().float());
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
    track(&property("Metadata").unwrap_or(Value::Null))
}

/// Which track, out of the map the player answered with. One layer too deep
/// in the same way [`how_long`] was, and wrong in the same quiet way: every
/// song was the track at `/`, so every seek was aimed at a song that is not
/// one.
fn track(metadata: &Value) -> String {
    metadata
        .get("mpris:trackid")
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
///
/// Which is the whole difference between the two halves of this line. Told
/// over the bus, the fork builds the playlist out of the library around the
/// song, so next and previous walk the library from wherever the thumb landed.
/// Started with a word instead -- which is what a player that is not running
/// has to be given -- kew looks that word up and plays what answers to it,
/// which for most songs is one song and a playlist of one. That is the state
/// [`onward`] is sent to undo the moment the player has a name on the bus.
pub fn opening(path: &std::path::Path, folder: Kind) -> Vec<String> {
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
pub fn sought(path: &std::path::Path, folder: Kind) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

    match folder {
        Kind::AFolder => name,
        Kind::ASong => {
            name.rsplit_once('.').map_or(name.as_str(), |(stem, _)| stem).to_string()
        }
    }
}

/// A word the shell takes as one word, whatever is in it.
fn single_quoted(said: &str) -> String {
    format!("'{}'", said.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The song goes to `music-onward` whole and unquoted: it is handed to a
    /// process rather than to a shell, and a name that had been made safe for
    /// a shell would arrive with the quoting still in it and open nothing.
    #[test]
    fn what_is_handed_to_a_program_is_the_path_itself() {
        let awkward = std::path::Path::new("/home/x/Don't Stop (Live) [a b].flac");
        assert_eq!(
            onward_for(awkward),
            vec!["music-onward".to_string(), awkward.display().to_string()],
        );
    }

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
            for folder in [Kind::AFolder, Kind::ASong] {
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
        let argv = opening(std::path::Path::new("/home/x/Don't Stop.mp3"), Kind::ASong);
        assert_eq!(argv[0], "sh");
        assert!(argv[2].contains(r"'/home/x/Don'\''t Stop.mp3'"));
        assert!(argv[2].contains(r"'Don'\''t Stop'"));
    }

    /// The player that will not take a path is replaced by one that has it,
    /// which is the kew in the repositories.
    #[test]
    fn a_player_that_will_not_be_told_is_started_again() {
        let argv = opening(std::path::Path::new("/music"), Kind::AFolder);
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
        assert_eq!(sought(song, Kind::ASong), "505 [qU9mHegkTc4]");
        assert_eq!(sought(std::path::Path::new("/m/Vol. 2"), Kind::AFolder), "Vol. 2");
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
            "xesam:url": {"type": "s", "data": "file:///nowhere/505.opus"},
            "mpris:length": {"type": "x", "data": 253_000_000_i64},
            "mpris:trackid": {"type": "o", "data": "/org/kew/track/7"}
        })
    }

    /// The two the bar is drawn from, read out of the same map every other
    /// part of the song is read out of.
    #[test]
    fn how_long_a_song_is_comes_out_of_the_map_the_rest_of_it_does() {
        assert_eq!(how_long(&metadata()), 253_000_000);
        assert_eq!(track(&metadata()), "/org/kew/track/7");
    }

    /// A player that says nothing about either. Nought is a bar that cannot
    /// be divided and so is not drawn, and `/` is the track that is no track,
    /// which is what seeking checks for.
    #[test]
    fn a_player_that_says_neither_leaves_the_bar_with_nothing_to_divide() {
        assert_eq!(how_long(&serde_json::json!({})), 0);
        assert_eq!(how_long(&Value::Null), 0);
        assert_eq!(track(&serde_json::json!({})), "/");
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
