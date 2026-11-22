//! The player, asked and told over MPRIS.
//!
//! `music-player` runs with the library folder on its command line and answers
//! on the session bus. Everything a surface needs is a property or a method
//! there, so nothing here reads a file the player wrote.
//!
//! This half did not change when the player stopped being kew, and that was the
//! point of leaving MPRIS where it was: a panel that had to be rewritten to
//! meet its own player would have made the swap something no one could press
//! twice and compare. What did change is one member. `xesam:url` said which
//! file was playing so the card could offer to show it in Files, and both the
//! answer and the offer are gone.
//!
//! A call to `busctl` is one signature and then the arguments it describes,
//! never a type beside each argument. `SetPosition` takes two, and written as
//! `o <id> x <at>` it is a call with one argument and two words too many:
//! busctl refuses it, says so on a stream nothing here reads, and exits
//! non-zero into a helper that returns what was printed. So the seek was dead
//! for as long as it had existed -- the tap on the bar and the d-pad both,
//! because both are this one function -- and the panel went on drawing a bar
//! that moved with the song and answered nothing. `seeking` is the words, so
//! the shape of the call is a thing a test can hold rather than a string only
//! the bus ever sees.
//!
//! **What the bar is woken by is a signal and not its own asking.** The player
//! says `PropertiesChanged` for the pair anything acts on -- what is playing,
//! and whether it is -- and `EventGroup::Player` is a bus monitor over the
//! MPRIS path, which hears that signal and hears every `Get` this file makes to
//! answer it. A watch that took the monitor's lines whole would ask because it
//! had asked, which is the fault the bar's sound reading was, so what is worth
//! asking after is said here where what the reading comes from is known.
//!
//! The card under a song asks after one thing more, which is the player saying
//! a second has been played: the clock and the bar move on that, and nothing
//! else does, since the bars have no clock to move.

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::{Float, fixed_index, toward_zero_i64};
use console_core_words::Words;
use console_events::again::Worth;
use console_events::bus;
use console_music_player::answers;
use console_waiting::{Outcome, Ready, Schedule, until};
use std::path::{Path, PathBuf};

use console_panel::running::run_output;

use serde_json::Value;

use console_core_walking::{Between, Ring, where_it_is};

const NOTHING_SAID: &str = "";

const THE_FIRST_WAY_ROUND: u32 = 0;

const NO_PRESSES: u32 = 0;

const AT_THE_START: i64 = 0;

const NO_LENGTH_SAID: i64 = 0;

const NO_TRACK: &str = "/";
const MAX_PROPERTY_BYTES: u64 = 1024 * 1024;

pub const NAME: &str = "org.mpris.MediaPlayer2.console";

const OBJECT: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sound {
  Playing,
  Paused,
  Stopped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Playing {
  pub title: String,
  pub artist: String,
  pub album: String,
  pub art: Option<PathBuf>,
  pub sound: Sound,
}

impl Default for Playing {
  fn default() -> Self {
    Playing {
      title: String::new(),
      artist: String::new(),
      album: String::new(),
      art: None,
      sound: Sound::Stopped,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum About {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
  Any,
  AsListed,
}

pub fn about() -> Result<About, Never> {
  let Ok(listed) = run_output(Program::Busctl, &["--user", "--no-legend", "list"]);

  Ok(match ::core::primitive::bool::then_some(listed.contains(NAME), ()) {
    ::core::option::Option::Some(()) => About::Yes,
    ::core::option::Option::None => About::No,
  })
}

pub fn worth_asking_after(line: &str) -> Result<Worth, Never> {
  let Ok(said) = bus::message(line);

  let said_3 = match said {
    Some(said_2) => said_2,
    None => return Ok(Worth::Ignoring),
  };

  Ok(match said_3.member {
    "PropertiesChanged" => Worth::Querying,
    _asking_what_is_playing_is_not_it_changing => Worth::Ignoring,
  })
}

pub fn worth_moving_the_clock(line: &str) -> Result<Worth, Never> {
  let Ok(said) = bus::message(line);
  let Ok(the_song_changed) = worth_asking_after(line);

  Ok(match said.map(|said_2| (said_2.interface, said_2.member)) {
    Some((answers::APPLICATION, answers::POSITION_CHANGED)) => Worth::Querying,
    Some((_, _)) | None => the_song_changed,
  })
}

pub fn playing() -> Result<Option<Playing>, Never> {
  let status = property("PlaybackStatus")?;

  let status_3 = match status {
    Some(status_2) => status_2,
    None => return Ok(None),
  };

  let metadata = property("Metadata")?;

  let metadata_3 = match metadata {
    Some(metadata_2) => metadata_2,
    None => return Ok(None),
  };

  let playing = read(&status_3, &metadata_3)?;

  Ok(Some(playing))
}

fn property(name: &str) -> Result<Option<Value>, Never> {
  let Ok(said) = run_output(
    Program::Busctl,
    &[
      "--user",
      "--json=short",
      "get-property",
      NAME,
      OBJECT,
      PLAYER,
      name,
    ],
  );

  read_property(&said)
}

fn read_property(said: &str) -> Result<Option<Value>, Never> {
  let Ok(bytes) = fixed_index(said.len());

  match bool::then_some(bytes <= MAX_PROPERTY_BYTES, ()) {
    Some(()) => {}
    None => return Ok(None),
  }

  #[derive(serde::Deserialize)]
  #[serde(deny_unknown_fields)]
  struct Property {
    #[serde(rename = "type")]
    _kind: Option<serde::de::IgnoredAny>,
    data: Value,
  }

  Ok(match serde_json::from_str::<Property>(said) {
    Ok(held) => Some(held.data),
    Err(_not_json) => None,
  })
}

pub fn read(status: &Value, metadata: &Value) -> Result<Playing, Never> {
  let said = |key: &str| match metadata.get(key).and_then(|held| held.get("data")) {
    Some(Value::String(one)) => one.clone(),
    Some(Value::Array(many)) => match many.first().and_then(Value::as_str) {
      Some(one) => one.to_string(),
      None => String::new(),
    },
    Some(Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_)) | None => {
      String::new()
    }
  };
  let state = match status.as_str() {
    Some(state) => state,
    None => NOTHING_SAID,
  };
  let art = local(&said("mpris:artUrl"))?;

  Ok(Playing {
    title: said("xesam:title"),
    artist: said("xesam:artist"),
    album: said("xesam:album"),
    art,
    sound: match state {
      "Paused" => Sound::Paused,
      "Stopped" | "" => Sound::Stopped,
      _playing_or_something_this_player_made_up => Sound::Playing,
    },
  })
}

pub fn local(url: &str) -> Result<Option<PathBuf>, Never> {
  let Ok(read) = console_core_file_urls::path(url);
  let path = match read {
    Some(path) => path,
    None => return Ok(None),
  };

  Ok(match std::fs::metadata(&path) {
    Ok(_metadata) => Some(path),
    Err(_unavailable) => None,
  })
}

fn call(method: &str) -> Result<(), Never> {
  let Ok(_the_player_answers_on_its_own_bus) = run_output(
    Program::Busctl,
    &["--user", "call", NAME, OBJECT, PLAYER, method],
  );

  Ok(())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Words)]
pub enum Over {
  #[default]
  #[words(said = "None")]
  On,
  #[words(said = "Track")]
  Again,
  #[words(said = "Playlist")]
  Round,
}

impl Over {
  pub const ROUND: [Over; 3] = [Over::On, Over::Again, Over::Round];

  pub fn read(said: &str) -> Result<Over, Never> {
    for over in Over::ROUND {
      let name = over.said()?;

      match ::core::primitive::bool::then_some(name == said, ()) {
        ::core::option::Option::Some(()) => return Ok(over),
        ::core::option::Option::None => {}
      }
    }

    Ok(Over::default())
  }

  fn place(self) -> Result<u32, Never> {
    let Ok(found) = where_it_is(&Over::ROUND, &self);

    Ok(match found {
      Some(at) => at,
      None => THE_FIRST_WAY_ROUND,
    })
  }
}

pub fn shuffling() -> Result<Order, Never> {
  let shuffle = property("Shuffle")?;

  Ok(match shuffle.as_ref().and_then(Value::as_bool) {
    Some(true) => Order::Any,
    Some(false) | None => Order::AsListed,
  })
}

pub fn over() -> Result<Over, Never> {
  let status = property("LoopStatus")?;

  let said = match status.as_ref().and_then(Value::as_str) {
    Some(said) => said,
    None => NOTHING_SAID,
  };

  Over::read(said)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Property<'a> {
  name: &'a str,
  kind: &'a str,
  value: &'a str,
}

fn press(property: Property<'_>) -> Result<(), Never> {
  let Property { name, kind, value } = property;

  let Ok(_the_player_answers_on_its_own_bus) = run_output(
    Program::Busctl,
    &[
      "--user",
      "set-property",
      NAME,
      OBJECT,
      PLAYER,
      name,
      kind,
      value,
    ],
  );

  Ok(())
}

pub fn presses(from: Over, to: Over) -> Result<u32, Never> {
  let to_2 = to.place()?;
  let from_2 = from.place()?;

  let Ok(round) = Ring::of(&Over::ROUND);

  match round {
    Some(ring) => ring.steps(Between {
      from: from_2,
      to: to_2,
    }),
    None => Ok(NO_PRESSES),
  }
}

pub fn shuffle(any_order: Order) -> Result<(), Never> {
  let shuffling = shuffling()?;

  match ::core::primitive::bool::then_some(shuffling == any_order, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => press(Property {
      name: "Shuffle",
      kind: "b",
      value: &(any_order == Order::Any).to_string(),
    })?,
  }

  Ok(())
}

pub fn repeat(wanted: Over) -> Result<(), Never> {
  let over = over()?;
  let presses = presses(over, wanted)?;
  let name = wanted.said()?;

  for _ in 0..presses {
    press(Property {
      name: "LoopStatus",
      kind: "s",
      value: name,
    })?;
  }

  Ok(())
}

const COMES_UP: std::time::Duration = std::time::Duration::from_secs(6);

const BREATH: std::time::Duration = std::time::Duration::from_millis(150);

pub fn onward(song: &std::path::Path) -> Result<(), Never> {
  let about = onward_only()?;

  match about {
    About::No => return Ok(()),
    About::Yes => {}
  }

  open(song)
}

pub fn onward_only() -> Result<About, Never> {
  let about = waited_for()?;

  match about {
    About::No => return Ok(About::No),
    About::Yes => {}
  }

  shuffle(Order::Any)?;
  repeat(Over::Round)?;

  Ok(About::Yes)
}

pub fn open(song: &std::path::Path) -> Result<(), Never> {
  let location = on_bus(song)?;
  let Ok(_the_player_answers_on_its_own_bus) = run_output(
    Program::Busctl,
    &[
      "--user", "call", NAME, OBJECT, PLAYER, "OpenUri", "s", &location,
    ],
  );

  Ok(())
}

pub(crate) fn on_bus(path: &Path) -> Result<String, Never> {
  let plain = path
    .to_str()
    .filter(|text| !text.starts_with(console_core_file_urls::SCHEME));

  match plain {
    Some(text) => Ok(text.to_owned()),
    None => console_core_file_urls::url(path),
  }
}

pub fn onward_for(song: &Path) -> Result<Vec<String>, Never> {
  let mut words = vec!["music-onward".to_string()];

  match song.to_str() {
    Some(text) => words.push(text.to_owned()),
    None => {
      let url = console_core_file_urls::url(song)?;

      words.extend(["--song-uri".to_string(), url]);
    }
  }

  Ok(words)
}

fn waited_for() -> Result<About, Never> {
  let Ok(patience) = Schedule::asking_every(COMES_UP, BREATH);
  let Ok(came) = until(patience, || {
    let about = about()?;

    Ok(match about {
      About::Yes => Ready::Yes,
      About::No => Ready::NotYet,
    })
  });

  Ok(match came {
    Outcome::Happened => About::Yes,
    Outcome::RanOut => About::No,
  })
}

pub fn play_pause() -> Result<(), Never> {
  call("PlayPause")
}

pub fn pause() -> Result<(), Never> {
  call("Pause")
}

pub fn next() -> Result<(), Never> {
  call("Next")
}

pub fn previous() -> Result<(), Never> {
  call("Previous")
}

pub fn position() -> Result<i64, Never> {
  let at = property("Position")?;

  Ok(match at.as_ref().and_then(Value::as_i64) {
    Some(at_2) => at_2,
    None => AT_THE_START,
  })
}

pub fn length() -> Result<i64, Never> {
  let metadata = property("Metadata")?;

  let held = match metadata {
    Some(held) => held,
    None => Value::Null,
  };

  how_long(&held)
}

fn how_long(metadata: &Value) -> Result<i64, Never> {
  let told = metadata
    .get("mpris:length")
    .and_then(|held| held.get("data"))
    .and_then(Value::as_i64);

  Ok(match told {
    Some(how_long) => how_long,
    None => NO_LENGTH_SAID,
  })
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the inner value is a ratio of the requested position to the track duration"
  )
)]
pub struct TrackFraction(pub f64);

pub fn seek(TrackFraction(fraction): TrackFraction) -> Result<(), Never> {
  let how_long = length()?;

  let total = match std::num::NonZeroI64::new(how_long) {
    Some(total) => total,
    None => return Ok(()),
  };

  let Ok(whole) = total.get().float();
  let Ok(at) = toward_zero_i64(fraction.clamp(0.0, 1.0) * whole);
  let id = track_id()?;
  let words = seeking(&id, at)?;

  let Ok(_the_player_answers_on_its_own_bus) = run_output(
    Program::Busctl,
    &words.iter().map(String::as_str).collect::<Vec<&str>>(),
  );

  Ok(())
}

pub fn seeking(id: &str, at: i64) -> Result<Vec<String>, Never> {
  Ok(vec![
    "--user".to_string(),
    "call".to_string(),
    NAME.to_string(),
    OBJECT.to_string(),
    PLAYER.to_string(),
    "SetPosition".to_string(),
    "ox".to_string(),
    id.to_string(),
    at.to_string(),
  ])
}

fn track_id() -> Result<String, Never> {
  let metadata = property("Metadata")?;

  let held = match metadata {
    Some(held) => held,
    None => Value::Null,
  };

  track(&held)
}

fn track(metadata: &Value) -> Result<String, Never> {
  let told = metadata
    .get("mpris:trackid")
    .and_then(|held| held.get("data"))
    .and_then(Value::as_str);

  Ok(match told {
    Some(track) => track.to_string(),
    None => NO_TRACK.to_string(),
  })
}

pub fn opening(path: &std::path::Path, under: &std::path::Path) -> Result<Vec<String>, Never> {
  let words = opening_words(path, under)?;

  Program::Sh.words(words)
}

pub fn opening_words(
  path: &std::path::Path,
  under: &std::path::Path,
) -> Result<Vec<String>, Never> {
  const OPEN_OR_RESTART: &str = "busctl --user call \"$1\" \"$2\" \"$3\" OpenUri s \"$4\" 2>/dev/null || { pkill -x music-player; shift 4; exec music-player \"$@\"; }";
  let location = on_bus(path)?;
  let mut words = vec![
    "-c".to_string(),
    OPEN_OR_RESTART.to_string(),
    "sh".to_string(),
    NAME.to_string(),
    OBJECT.to_string(),
    PLAYER.to_string(),
    location,
  ];

  match under.to_str() {
    Some(text) => words.push(text.to_owned()),
    None => {
      let url = console_core_file_urls::url(under)?;

      words.extend(["--folder-uri".to_owned(), url]);
    }
  }

  Ok(words)
}

#[cfg(test)]
mod tests {
  #[test]
  fn the_player_saying_what_changed_is_worth_asking_after() {
    let line = concat!(
      "  Sender=:1.65 Path=/org/mpris/MediaPlayer2 ",
      "Interface=org.freedesktop.DBus.Properties  Member=PropertiesChanged"
    );
    let Ok(worth) = worth_asking_after(line);

    assert_eq!(worth, Worth::Querying);
  }

  #[test]
  fn asking_what_is_playing_is_not_the_player_saying_it_changed() {
    let line = concat!(
      "  Sender=:1.92 Destination=org.mpris.MediaPlayer2.console ",
      "Path=/org/mpris/MediaPlayer2 ",
      "Interface=org.freedesktop.DBus.Properties  Member=Get"
    );
    let Ok(worth) = worth_asking_after(line);

    assert_eq!(worth, Worth::Ignoring);
  }

  #[test]
  fn a_second_played_moves_the_card_and_not_the_bar() {
    let line = concat!(
      "  Sender=:1.65 Path=/org/mpris/MediaPlayer2 ",
      "Interface=console.Player  Member=PositionChanged"
    );
    let Ok(card) = worth_moving_the_clock(line);
    let Ok(bar) = worth_asking_after(line);

    assert_eq!(card, Worth::Querying);
    assert_eq!(bar, Worth::Ignoring);
  }

  #[test]
  fn the_card_still_hears_the_song_changing() {
    let line = concat!(
      "  Sender=:1.65 Path=/org/mpris/MediaPlayer2 ",
      "Interface=org.freedesktop.DBus.Properties  Member=PropertiesChanged"
    );
    let Ok(worth) = worth_moving_the_clock(line);

    assert_eq!(worth, Worth::Querying);
  }

  #[test]
  fn what_the_monitor_prints_around_a_message_does_not_wake_the_bar() {
    for line in ["‣ Type=method_call  Endian=l", "  };", ""] {
      let Ok(worth) = worth_asking_after(line);

      assert_eq!(worth, Worth::Ignoring, "{line}");
    }
  }

  use super::*;

  #[test]
  fn a_busctl_property_reply_carries_its_signature_and_value() {
    assert_eq!(
      read_property(r#"{"type":"s","data":"Paused"}"#),
      Ok(Some(Value::String("Paused".to_owned())))
    );
  }

  #[test]
  fn what_is_handed_to_a_program_is_the_path_itself() {
    let awkward = std::path::Path::new("/home/x/Don't Stop (Live) [a b].flac");
    let Ok(words) = onward_for(awkward);

    assert_eq!(
      words,
      vec!["music-onward".to_string(), awkward.display().to_string()]
    );
  }

  fn shell_line(arguments: &[String]) -> Result<Option<&str>, Never> {
    Ok(match arguments {
      [_shell, _told, line, ..] => Some(line.as_str()),
      _other => None,
    })
  }

  const NOT_A_SHELL_LINE: &str = "a player is opened as sh -c and a line";

  #[test]
  fn what_is_handed_to_a_shell_is_something_a_shell_can_read()
  -> Result<(), Box<dyn std::error::Error>> {
    let awkward = [
      "/home/x/Don't Stop.mp3",
      "/home/x/Sweetness (Official Music Video) [0zzv0vYECWQ].opus",
      "/home/x/a; rm -rf $HOME/b.flac",
      "/home/x/quote\"and\"brace}.wav",
    ];
    for path in awkward {
      let under = std::path::Path::new("/home/x/My Music (all of it)");
      let Ok(arguments) = opening(std::path::Path::new(path), under);
      let Ok(script) = shell_line(&arguments);
      let line = script.ok_or(NOT_A_SHELL_LINE)?;

      let Ok(mut asking) = Program::Sh.command();

      let checked = asking.arg("-n").args(arguments.iter().skip(1)).output()?;
      let stderr = std::str::from_utf8(&checked.stderr)?;

      assert!(
        checked.status.success(),
        "sh cannot read the line for {path:?}: {}\n{line}",
        stderr,
      );
    }

    Ok(())
  }

  #[test]
  fn a_name_the_shell_would_read_as_words_stays_one_word() -> Result<(), &'static str> {
    let under = std::path::Path::new("/home/x/Music");
    let Ok(arguments) = opening(std::path::Path::new("/home/x/Don't Stop.mp3"), under);
    let Ok(script) = shell_line(&arguments);
    let line = script.ok_or(NOT_A_SHELL_LINE)?;

    assert_eq!(arguments.first().map(String::as_str), Some("sh"));
    assert!(line.contains(r#"OpenUri s "$4""#));
    assert_eq!(
      arguments.iter().rev().nth(1).map(String::as_str),
      Some("/home/x/Don't Stop.mp3")
    );

    Ok(())
  }

  #[test]
  fn a_player_that_will_not_be_told_is_started_again() -> Result<(), &'static str> {
    let under = std::path::Path::new("/music");
    let Ok(arguments) = opening(std::path::Path::new("/music/Vol. 2"), under);
    let Ok(script) = shell_line(&arguments);
    let line = script.ok_or(NOT_A_SHELL_LINE)?;

    assert!(line.contains("OpenUri"));
    assert!(line.contains("pkill -x music-player"));
    assert!(line.contains(r#"shift 4; exec music-player "$@""#));
    assert_eq!(arguments.last().map(String::as_str), Some("/music"));

    Ok(())
  }

  #[test]
  fn the_player_is_started_on_the_library_rather_than_on_the_song() -> Result<(), &'static str> {
    let under = std::path::Path::new("/music");
    let Ok(arguments) = opening(std::path::Path::new("/music/b/505.opus"), under);
    let Ok(script) = shell_line(&arguments);
    let line = script.ok_or(NOT_A_SHELL_LINE)?;

    assert!(line.contains(r#"OpenUri s "$4""#));
    assert_eq!(
      arguments.iter().rev().nth(1).map(String::as_str),
      Some("/music/b/505.opus")
    );
    assert!(line.contains(r#"shift 4; exec music-player "$@""#));
    assert_eq!(arguments.last().map(String::as_str), Some("/music"));

    Ok(())
  }

  #[test]
  fn a_player_that_cannot_be_asked_is_not_playing() {
    assert_eq!(Playing::default().sound, Sound::Stopped);
  }

  #[test]
  fn a_mode_is_as_many_presses_away_as_the_round_makes_it() {
    assert_eq!(presses(Over::On, Over::Again), Ok(1));
    assert_eq!(presses(Over::Round, Over::Again), Ok(2));
    assert_eq!(presses(Over::Round, Over::On), Ok(1));
    assert_eq!(presses(Over::Again, Over::Again), Ok(0));
  }

  #[test]
  fn what_the_player_says_about_the_end_of_a_song_is_read_back() {
    assert_eq!(Over::read("Track"), Ok(Over::Again));
    assert_eq!(Over::read("Playlist"), Ok(Over::Round));
    assert_eq!(Over::read("None"), Ok(Over::On));
    assert_eq!(Over::read(""), Ok(Over::On));
  }

  fn metadata() -> Result<Value, Never> {
    Ok(serde_json::json!({
        "xesam:title": {"type": "s", "data": "505"},
        "xesam:artist": {"type": "as", "data": ["Arctic Monkeys"]},
        "xesam:album": {"type": "s", "data": "Favourite Worst Nightmare"},
        "mpris:artUrl": {"type": "s", "data": "file:///tmp/kew/cover.jpg"},
        "mpris:length": {"type": "x", "data": 253_000_000_i64},
        "mpris:trackid": {"type": "o", "data": "/org/kew/track/7"}
    }))
  }

  #[test]
  fn how_long_a_song_is_comes_out_of_the_map_the_rest_of_it_does() {
    let Ok(metadata) = metadata();

    assert_eq!(how_long(&metadata), Ok(253_000_000));
    assert_eq!(track(&metadata), Ok("/org/kew/track/7".to_string()));
  }

  #[test]
  fn a_player_that_says_neither_leaves_the_bar_with_nothing_to_divide() {
    assert_eq!(how_long(&serde_json::json!({})), Ok(0));
    assert_eq!(how_long(&Value::Null), Ok(0));
    assert_eq!(track(&serde_json::json!({})), Ok("/".to_string()));
  }

  #[test]
  fn one_artist_is_taken_out_of_the_list_it_arrives_in() {
    let Ok(metadata) = metadata();
    let Ok(playing) = read(&Value::String(String::from("Playing")), &metadata);

    assert_eq!(playing.artist, "Arctic Monkeys");
    assert_eq!(playing.title, "505");
    assert_eq!(playing.sound, Sound::Playing);
  }

  #[test]
  fn a_player_that_says_nothing_is_stopped() {
    let Ok(playing) = read(&Value::Null, &serde_json::json!({}));

    assert_eq!(playing.sound, Sound::Stopped);
    assert_eq!(playing.title, "");
  }

  #[test]
  fn a_cover_that_is_not_there_is_no_cover() {
    assert_eq!(local("file:///nowhere/cover.jpg"), Ok(None));
    assert_eq!(local("https://example.com/cover.jpg"), Ok(None));
  }

  #[test]
  fn a_seek_is_one_signature_and_the_two_things_it_describes() {
    let Ok(words) = seeking("/org/kew/tracklist/track0", 120_000_000);

    let told = words
      .iter()
      .skip_while(|word| *word != "SetPosition")
      .skip(1);

    assert_eq!(
      told.cloned().collect::<Vec<String>>(),
      ["ox", "/org/kew/tracklist/track0", "120000000"],
      "busctl reads the word after the method as the whole signature",
    );
  }

  #[test]
  fn a_seek_that_busctl_will_not_read_is_a_seek_that_never_happened()
  -> Result<(), std::str::Utf8Error> {
    let Ok(words) = seeking("/org/kew/tracklist/track0", 1);

    let Ok(mut asking) = Program::Busctl.command();
    let checked = asking.args(words.iter().map(String::as_str)).output();

    let checked_3 = match checked {
      Ok(checked_2) => checked_2,
      Err(_busctl_could_not_be_run) => return Ok(()),
    };

    let why = std::str::from_utf8(&checked_3.stderr)?;

    assert!(
      !why.contains("parameters for signature"),
      "busctl will not read the call this makes: {why}",
    );

    Ok(())
  }
}
