//! What the panel presses, answered where kew used to answer it.
//!
//! The interface is not this player's design and is not this desktop's either:
//! it is MPRIS, spelled the way `console_application_music::player` already reads it and
//! the way the checks already press it through busctl. So the surface came
//! first and the player was written to it, which is the opposite of how the
//! rest of this tree is built and is right here -- a panel that has to be
//! changed to meet its player would have made the swap unpressable.
//!
//! Everything is answered under one lock. A property read is short; a method
//! that starts a song is not, because it reads what the song says and the
//! picture on the front of it before any sound is made. Both used to be asked
//! of ffprobe and ffmpeg at once, because most of what either cost was ffmpeg
//! loading its libraries; they are read out of the file here now, one after the
//! other, since a read with no program to start is not worth a thread. That is
//! still a read of the song inside a bus call, and it is deliberate: the
//! panel wants the title and the sleeve at the same moment it wants the sound,
//! and a player that answered the call first and filled the card in afterwards
//! would draw an empty card every time a song is pressed.
//!
//! `PropertiesChanged` is emitted for the pair a listener actually acts on --
//! what is playing, and whether it is. Nothing here polls to find out, and the
//! panel is welcome to go on asking.
//!
//! The position is not one of them, because MPRIS says a listener works out
//! where the song is from the rate and never hears it change, and a panel
//! written here may not keep a timer to do that sum. So each whole second
//! played is `PositionChanged` on this player's own interface instead: the
//! card showing the clock listens for it, and the bars, which have no clock,
//! go on hearing only the pair.
//!
//! ## What answers, and what carries the answer
//!
//! A message goes in and what to say back comes out. Nothing here opens a
//! socket, so which property a name means, what a method does to the playlist
//! and what comes back when a caller asks for something this player has never
//! heard of are all questions a test can put with no bus anywhere -- which is
//! the shape `console-notifications` already answers its own name in, and the
//! reason both of them are readable at all.
//!
//! It was gio's before, and gio is glib, and glib is a main loop and a type
//! system and an object system carried on a device for one program that draws
//! nothing. `console-bus` is the wire this desktop already wrote to hold the
//! notification name, and a player is the second thing on it: what a service
//! needs beyond what a daemon needed is properties and introspection, which
//! are two more members rather than another library.

use console_bus::connection::{ConnectionError, Sender};
use console_bus::messages::{Message, Signal, Truth, ValidationError, Value};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_response_times::{Wait, Waiting};
use std::path::{Path, PathBuf};

use crate::answers::{self, Reply, Song, Status};
use crate::bookmark::{self, Bookmark, QueueSaved};
use crate::library;
use crate::playlist::{Context, Moved, Order, Playlist, SavedQueue};
use crate::sounding::{Sounding, Wanted};
use crate::{art, tags};

const CHANGED: &str = "org.freedesktop.DBus.Properties";

const LOOKING: &str = "org.freedesktop.DBus.Introspectable";

const PEER: &str = "org.freedesktop.DBus.Peer";
const SESSION: &str = "org.console.MusicPlayer.Session";

const ROOT_SAYS: [(&str, &str); 6] = [
  ("CanQuit", "b"),
  ("CanRaise", "b"),
  ("HasTrackList", "b"),
  ("Identity", "s"),
  ("SupportedUriSchemes", "as"),
  ("SupportedMimeTypes", "as"),
];

const PLAYER_SAYS: [(&str, &str); 15] = [
  ("PlaybackStatus", "s"),
  ("LoopStatus", "s"),
  ("Rate", "d"),
  ("Shuffle", "b"),
  ("Metadata", "a{sv}"),
  ("Volume", "d"),
  ("Position", "x"),
  ("MinimumRate", "d"),
  ("MaximumRate", "d"),
  ("CanGoNext", "b"),
  ("CanGoPrevious", "b"),
  ("CanPlay", "b"),
  ("CanPause", "b"),
  ("CanSeek", "b"),
  ("CanControl", "b"),
];

const XML: &str = r#"<node>
  <interface name="org.console.MusicPlayer.Session">
    <method name="Checkpoint"/>
  </interface>
  <interface name="org.mpris.MediaPlayer2">
    <method name="Raise"/>
    <method name="Quit"/>
    <property name="CanQuit" type="b" access="read"/>
    <property name="CanRaise" type="b" access="read"/>
    <property name="HasTrackList" type="b" access="read"/>
    <property name="Identity" type="s" access="read"/>
    <property name="SupportedUriSchemes" type="as" access="read"/>
    <property name="SupportedMimeTypes" type="as" access="read"/>
  </interface>
  <interface name="org.mpris.MediaPlayer2.Player">
    <method name="Next"/>
    <method name="Previous"/>
    <method name="Pause"/>
    <method name="PlayPause"/>
    <method name="Stop"/>
    <method name="Play"/>
    <method name="Seek">
      <arg name="Offset" type="x" direction="in"/>
    </method>
    <method name="SetPosition">
      <arg name="TrackId" type="o" direction="in"/>
      <arg name="Position" type="x" direction="in"/>
    </method>
    <method name="OpenAlbum">
      <arg name="Uris" type="s" direction="in"/>
    </method>
    <method name="OpenUri">
      <arg name="Uri" type="s" direction="in"/>
    </method>
    <property name="PlaybackStatus" type="s" access="read"/>
    <property name="LoopStatus" type="s" access="readwrite"/>
    <property name="Rate" type="d" access="readwrite"/>
    <property name="Shuffle" type="b" access="readwrite"/>
    <property name="Metadata" type="a{sv}" access="read"/>
    <property name="Volume" type="d" access="readwrite"/>
    <property name="Position" type="x" access="read"/>
    <property name="MinimumRate" type="d" access="read"/>
    <property name="MaximumRate" type="d" access="read"/>
    <property name="CanGoNext" type="b" access="read"/>
    <property name="CanGoPrevious" type="b" access="read"/>
    <property name="CanPlay" type="b" access="read"/>
    <property name="CanPause" type="b" access="read"/>
    <property name="CanSeek" type="b" access="read"/>
    <property name="CanControl" type="b" access="read"/>
  </interface>
</node>"#;

pub struct PlayerState {
  pub list: Playlist,
  pub sounding: Sounding,
  pub playing: Song,
  pub folder: Option<PathBuf>,
  pub turn: u64,
  saved_queue: Option<SavedQueue>,
  album: Option<crate::loudness::AlbumGain>,
}

enum Restored {
  Yes,
  No,
}

impl PlayerState {
  pub fn new(sounding: Sounding, folder: Option<PathBuf>) -> Result<PlayerState, Never> {
    let Ok(list) = Playlist::of(Vec::new());
    let mut held = PlayerState {
      list,
      sounding,
      playing: Song::default(),
      folder: folder.map(|supplied| match std::path::absolute(&supplied) {
        Ok(absolute) => absolute,
        Err(_unrooted) => supplied,
      }),
      turn: 0,
      saved_queue: None,
      album: None,
    };

    let Ok(()) = held.remembered();

    Ok(held)
  }

  fn remembered(&mut self) -> Result<(), Never> {
    let Ok(kept) = bookmark::read();

    let kept_3 = match kept {
      Some(kept_2) => kept_2,
      None => return Ok(()),
    };

    let Ok(restored) = self.restore_queue(&kept_3);

    match restored {
      Restored::Yes => return Ok(()),
      Restored::No => {}
    }

    let checked = match std::fs::metadata(&kept_3.song) {
      Ok(checked) => checked,
      Err(fault) => {
        console_core_logging::debug!("could not restore {}: {fault}", kept_3.song.display());

        return Ok(());
      }
    };

    match checked.is_file().then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let under = match &self.folder {
      Some(folder) => folder.clone(),
      None => match kept_3.song.parent() {
        Some(parent) => parent.to_path_buf(),
        None => return Ok(()),
      },
    };

    let Ok(mut waiting) = Waiting::here(Wait {
      who: "music-player",
      what: "remembered",
    });
    let Ok(songs) = library::songs_under(&under);

    let Ok(()) = waiting.mark("walked");
    let Ok(many) = fitted::<_, u64>(songs.len());
    let Ok(list) = Playlist::opened(songs, &kept_3.song);

    self.list = list;

    let Ok(()) = waiting.mark("listed");
    let Ok(()) = self.describing(&kept_3.song, &mut waiting);
    let Ok(()) = waiting.counted("songs", many);
    let Ok(()) = waiting.done_if_felt();

    self.sounding.ready(&kept_3.song, kept_3.at)
  }

  fn restore_queue(&mut self, kept: &Bookmark) -> Result<Restored, Never> {
    let Ok(saved) = bookmark::read_queue();
    let queue = match saved {
      Some(queue) => queue,
      None => return Ok(Restored::No),
    };
    let Ok(restored) = Playlist::restored(queue, &kept.song);
    let playlist = match restored {
      Some(playlist) => playlist,
      None => return Ok(Restored::No),
    };
    let Ok(song) = playlist.song();
    let path = match song {
      Some(song_path) => song_path.to_path_buf(),
      None => return Ok(Restored::No),
    };

    self.list = playlist;
    let Ok(checkpoint) = self.list.checkpoint();
    let Ok(context) = self.list.context();
    self.album = match context {
      Context::Library => None,
      Context::Album => {
        let Ok(gain) = crate::loudness::album_gain(&checkpoint.songs);

        gain
      }
    };
    self.saved_queue = Some(checkpoint);
    let at = match (path == kept.song).then_some(()) {
      Some(()) => kept.at,
      None => crate::Seconds(0.0),
    };
    let Ok(mut waiting) = Waiting::here(Wait {
      who: "music-player",
      what: "remembered",
    });
    let Ok(()) = self.describing(&path, &mut waiting);
    let Ok(gain) = self.gain();
    let Ok(()) = self.sounding.ready_with_gain(&path, at, gain);

    Ok(Restored::Yes)
  }

  pub fn remember(&mut self) -> Result<(), Never> {
    let Ok(queue) = self.list.checkpoint();

    match (self.saved_queue.as_ref() != Some(&queue)).then_some(()) {
      Some(()) => {
        let Ok(saved) = bookmark::write_queue(&queue);

        match saved {
          QueueSaved::Yes => self.saved_queue = Some(queue),
          QueueSaved::No => {}
        }
      }
      None => {}
    }

    self.remember_position()
  }

  pub fn remember_position(&self) -> Result<(), Never> {
    let Ok(song) = self.list.song();

    let song_3 = match song {
      Some(song_2) => song_2.to_path_buf(),
      None => return Ok(()),
    };

    let Ok(at) = self.sounding.position();

    bookmark::write(&song_3, crate::Seconds(at))
  }

  pub fn opened_album(&mut self, argument: &str) -> Result<(), Never> {
    let Ok(saved) = library::album_paths(argument);
    let library::AlbumQueue { mut tracks, first } = match saved {
      Some(queue) => queue,
      None => return Ok(()),
    };
    let Ok(index) = index(first);
    let path = match tracks.get(index) {
      Some(path) => path.clone(),
      None => return Ok(()),
    };
    tracks.retain(|track| library::queueable(track) == Ok(library::Playable::Yes));

    match tracks.contains(&path).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let Ok(gain) = crate::loudness::album_gain(&tracks);
    let Ok(list) = Playlist::album(tracks, &path);
    self.album = gain;
    self.list = list;

    self.started()
  }

  pub fn opened(&mut self, at: &Path) -> Result<(), Never> {
    self.album = None;
    let at_2 = match std::path::absolute(at) {
      Ok(path) => path,
      Err(_unrooted) => return Ok(()),
    };

    let Ok(mut waiting) = Waiting::here(Wait {
      who: "music-player",
      what: "library",
    });
    let under = match &self.folder {
      Some(folder) => folder.clone(),
      None => match at_2.parent() {
        Some(parent) => parent.to_path_buf(),
        None => return Ok(()),
      },
    };

    let Ok(mut songs) = library::songs_under(&under);

    match crate::selection::Selection::load() {
      Ok(selection) => songs.retain(|song| {
        song == &at_2 || selection.visibility(song) == Ok(crate::selection::Visibility::Included)
      }),
      Err(fault) => {
        console_core_logging::warn!("could not read Music Library selections: {fault}")
      }
    }

    match (!songs.contains(&at_2) && library::queueable(&at_2) == Ok(library::Playable::Yes))
      .then_some(())
    {
      Some(()) => songs.insert(0, at_2.clone()),
      None => {}
    }

    let Ok(()) = waiting.mark("walked");
    let Ok(many) = fitted::<_, u64>(songs.len());
    let Ok(list) = Playlist::opened(songs, &at_2);

    self.list = list;

    let Ok(()) = waiting.mark("listed");
    let Ok(()) = waiting.counted("songs", many);
    let Ok(()) = waiting.done_if_felt();

    self.started()
  }

  pub fn started(&mut self) -> Result<(), Never> {
    let Ok(song) = self.list.song();

    let song_3 = match song {
      Some(song_2) => song_2.to_path_buf(),
      None => return Ok(()),
    };

    let Ok(mut waiting) = Waiting::here(Wait {
      who: "music-player",
      what: "song",
    });
    let Ok(()) = self.describing(&song_3, &mut waiting);
    let Ok(()) = waiting.done_if_felt();

    let Ok(gain) = self.gain();

    self
      .sounding
      .play_album_track(&song_3, crate::Seconds(0.0), gain)
  }

  fn gain(&self) -> Result<Option<crate::loudness::AlbumGain>, Never> {
    let Ok(order) = self.list.ordering();

    Ok(match order {
      Order::Any => None,
      Order::AsListed => self.album,
    })
  }

  fn describing(&mut self, song: &Path, waiting: &mut Waiting) -> Result<(), Never> {
    self.turn = self.turn.saturating_add(1);

    let Ok(Described { said, art }) = described(song, self.turn);
    let Ok(()) = waiting.mark("described");
    let Ok(title) = title_or_filename(song, &said.title);

    self.playing = Song {
      title,
      artist: said.artist,
      album: said.album,
      art,
      length: said.length,
    };

    Ok(())
  }

  pub fn onward(&mut self) -> Result<(), Never> {
    let Ok(moved) = self.list.onward();

    self.moved(moved)
  }

  pub fn back(&mut self) -> Result<(), Never> {
    let Ok(moved) = self.list.back();

    self.moved(moved)
  }

  pub fn finished(&mut self) -> Result<(), Never> {
    let Ok(moved) = self.list.finished();

    self.moved(moved)
  }

  fn moved(&mut self, moved: Moved) -> Result<(), Never> {
    match moved {
      Moved::Yes => self.started(),
      Moved::No => self.sounding.wanting(Wanted::Stopped),
    }
  }

  pub fn play_pause(&mut self) -> Result<(), Never> {
    let Ok(wanted) = self.sounding.desired();

    match wanted {
      Wanted::Playing => self.sounding.wanting(Wanted::Paused),
      Wanted::Paused => self.sounding.wanting(Wanted::Playing),
      Wanted::Stopped => self.started(),
    }
  }

  pub fn status(&self) -> Result<Status, Never> {
    let Ok(wanted) = self.sounding.desired();

    Ok(match wanted {
      Wanted::Playing => Status::Playing,
      Wanted::Paused => Status::Paused,
      Wanted::Stopped => Status::Stopped,
    })
  }
}

fn title_or_filename(song: &Path, title: &str) -> Result<String, Never> {
  match ::core::primitive::bool::then_some(title.is_empty(), ()) {
    ::core::option::Option::None => return Ok(title.to_string()),
    ::core::option::Option::Some(()) => {}
  }

  match song.file_stem() {
    Some(stem) => match stem.to_str() {
      Some(text) => Ok(text.to_string()),
      None => console_core_file_urls::escaped(Path::new(stem)),
    },
    None => Ok(String::new()),
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Described {
  pub said: tags::Playing,
  pub art: Option<PathBuf>,
}

pub fn described(song: &Path, turn: u64) -> Result<Described, Never> {
  let Ok(said) = tags::playing(song);
  let Ok(art) = art::of(song, turn);

  Ok(Described { said, art })
}

fn reply_value(said: &Reply) -> Result<Value, Never> {
  Ok(match said {
    Reply::Track(path) => Value::variant("o", Value::Path(path.clone()))?,
    Reply::Word(word) => Value::variant("s", Value::Word(word.clone()))?,
    Reply::Strings(words) => {
      let listed = words.iter().map(|word| Value::Word(word.clone())).collect();

      Value::variant("as", Value::List(listed))?
    }
    Reply::Long(long) => Value::variant("x", Value::Signed64(*long))?,
  })
}

fn words(said: &[&str]) -> Result<Value, Never> {
  Ok(Value::List(
    said
      .iter()
      .map(|word| Value::Word((*word).to_string()))
      .collect(),
  ))
}

fn metadata(held: &PlayerState) -> Result<Value, Never> {
  let Ok(said) = answers::metadata(&held.playing);
  let mut listed: Vec<Value> = Vec::new();

  for (name, value) in said {
    let Ok(value_2) = reply_value(&value);

    listed.push(Value::Group(vec![Value::Word(name), value_2]));
  }

  Ok(Value::List(listed))
}

fn root_says(name: &str) -> Result<Option<Value>, Never> {
  Ok(match name {
    "Identity" => Some(Value::Word(answers::IDENTITY.to_string())),
    "SupportedUriSchemes" => {
      let Ok(said) = words(&["file"]);

      Some(said)
    }
    "SupportedMimeTypes" => {
      let Ok(said) = words(&[]);

      Some(said)
    }
    "CanQuit" => Some(Value::Truth(Truth::Yes)),
    "CanRaise" | "HasTrackList" => Some(Value::Truth(Truth::No)),
    _unknown_to_this_player => None,
  })
}

fn player_says(held: &PlayerState, name: &str) -> Result<Option<Value>, Never> {
  Ok(match name {
    "PlaybackStatus" => {
      let Ok(status) = held.status();
      let Ok(said) = status.said();

      Some(Value::Word(said.to_string()))
    }
    "LoopStatus" => {
      let Ok(over) = held.list.repeating();
      let Ok(said) = answers::over_said(over);

      Some(Value::Word(said.to_string()))
    }
    "Shuffle" => {
      let Ok(order) = held.list.ordering();

      Some(Value::Truth(match order {
        Order::Any => Truth::Yes,
        Order::AsListed => Truth::No,
      }))
    }
    "Metadata" => {
      let Ok(said) = metadata(held);

      Some(said)
    }
    "Position" => {
      let Ok(at) = held.sounding.position();
      let Ok(micros) = answers::micros(crate::Seconds(at));

      Some(Value::Signed64(micros))
    }
    "Rate" | "MinimumRate" | "MaximumRate" | "Volume" => Some(Value::Fraction(1.0)),
    "CanGoNext" | "CanGoPrevious" | "CanPlay" | "CanPause" | "CanSeek" | "CanControl" => {
      Some(Value::Truth(Truth::Yes))
    }
    _unknown_to_this_player => None,
  })
}

struct Property<'a> {
  on: &'a str,
  name: &'a str,
}

fn says(held: &PlayerState, asked: &Property<'_>) -> Result<Option<Value>, Never> {
  let Property { on, name } = *asked;
  let known = match on {
    answers::ROOT => ROOT_SAYS.iter().find(|(said, _)| *said == name),
    answers::PLAYER => PLAYER_SAYS.iter().find(|(said, _)| *said == name),
    _nothing_here_answers_for_that => None,
  };

  let shape = match known {
    Some((_, shape)) => *shape,
    None => return Ok(None),
  };

  let value = match on {
    answers::ROOT => root_says(name)?,
    answers::PLAYER => player_says(held, name)?,
    _nothing_here_answers_for_that => None,
  };

  Ok(match value {
    Some(value_2) => {
      let Ok(held_2) = Value::variant(shape, value_2);

      Some(held_2)
    }
    None => None,
  })
}

fn says_all(held: &PlayerState, on: &str) -> Result<Value, Never> {
  let every: &[(&str, &str)] = match on {
    answers::ROOT => &ROOT_SAYS,
    answers::PLAYER => &PLAYER_SAYS,
    _nothing_here_answers_for_that => &[],
  };

  let mut listed: Vec<Value> = Vec::new();

  for (name, _) in every {
    let said = says(held, &Property { on, name })?;

    match said {
      Some(said_2) => listed.push(Value::Group(vec![Value::Word((*name).to_string()), said_2])),
      None => {}
    }
  }

  Ok(Value::List(listed))
}

fn player_told(held: &mut PlayerState, name: &str, value: &Value) -> Result<(), Never> {
  match name {
    "LoopStatus" => {
      let Ok(said) = value.text();

      let said_3 = match said {
        Some(said_2) => said_2.to_string(),
        None => String::new(),
      };

      let Ok(over) = answers::over_read(&said_3);

      let Ok(()) = held.list.repeat(over);
      held.remember()
    }
    "Shuffle" => {
      let Ok(order) = shuffling(value);
      let Ok(seed) = seed();

      let Ok(()) = held.list.shuffling(order, seed);
      held.remember()
    }
    _nothing_else_here_can_be_set => Ok(()),
  }
}

fn shuffling(value: &Value) -> Result<Order, Never> {
  let Ok(wrapped) = Value::variant("b", Value::Truth(Truth::Yes));
  let asked = *value == wrapped || *value == Value::Truth(Truth::Yes);

  Ok(match ::core::primitive::bool::then_some(asked, ()) {
    ::core::option::Option::Some(()) => Order::Any,
    ::core::option::Option::None => Order::AsListed,
  })
}

#[cfg_attr(
  dylint_lib = "explicit039_no_reading_the_clock",
  allow(
    explicit039_no_reading_the_clock,
    reason = "a shuffle asked for twice in one run is meant to answer differently, and the nanoseconds are the only thing on this machine that differ between the two askings; `shuffling` itself takes the seed, so the decision is still one a test can press"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit077_no_ambient_randomness",
  allow(
    explicit077_no_ambient_randomness,
    reason = "a shuffle asked for twice in one run is meant to answer differently, and the clock is the seed `shuffling` is handed"
  )
)]
fn seed() -> Result<u64, Never> {
  Ok(
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
      Ok(since) => u64::from(since.subsec_nanos()),
      Err(_the_clock_is_before_the_epoch) => 1,
    },
  )
}

fn handle_call(held: &mut PlayerState, method: &str, values: &[Value]) -> Result<(), Never> {
  let Ok(()) = doing(held, method, values);

  held.remember()
}

fn doing(held: &mut PlayerState, method: &str, values: &[Value]) -> Result<(), Never> {
  match method {
    "Next" => held.onward(),
    "Previous" => held.back(),
    "PlayPause" => held.play_pause(),
    "Pause" => held.sounding.wanting(Wanted::Paused),
    "Stop" => held.sounding.wanting(Wanted::Stopped),
    "Play" => {
      let Ok(wanted) = held.sounding.desired();

      match wanted {
        Wanted::Paused => held.sounding.wanting(Wanted::Playing),
        Wanted::Playing => Ok(()),
        Wanted::Stopped => held.started(),
      }
    }
    "OpenAlbum" => {
      let Ok(argument) = word(values, 0);

      held.opened_album(&argument)
    }
    "OpenUri" => {
      let Ok(at) = word(values, 0);
      let Ok(at_2) = local(&at);

      held.opened(&at_2)
    }
    "SetPosition" => {
      let Ok(micros) = long(values, 1);
      let Ok(seconds) = answers::seconds(micros);

      held.sounding.seek(seconds)
    }
    "Seek" => {
      let Ok(micros) = long(values, 0);
      let Ok(by) = answers::seconds(micros);
      let Ok(at) = held.sounding.position();

      held.sounding.seek(at + by.0)
    }
    _nothing_else_is_offered => Ok(()),
  }
}

fn word(values: &[Value], at: u32) -> Result<String, Never> {
  let Ok(at_2) = index(at);
  let held = match values.get(at_2) {
    Some(value) => {
      let Ok(text) = value.text();

      text.map(str::to_string)
    }
    None => None,
  };

  Ok(match held {
    Some(said) => said,
    None => String::new(),
  })
}

fn long(values: &[Value], at: u32) -> Result<i64, Never> {
  let Ok(at_2) = index(at);
  let held = match values.get(at_2) {
    Some(value) => {
      let Ok(counted) = value.counted();

      counted
    }
    None => None,
  };

  Ok(match held {
    Some(number) => number,
    None => NO_NUMBER_THERE,
  })
}

const NO_NUMBER_THERE: i64 = 0;

fn local(said: &str) -> Result<PathBuf, Never> {
  let Ok(read) = console_core_file_urls::path(said);

  Ok(match read {
    Some(path) => path,
    None => PathBuf::from(said),
  })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Modified {
  Yes,
  #[default]
  No,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Turn {
  pub say: Option<Message>,
  pub changed: Modified,
}

pub fn handle_message(held: &mut PlayerState, message: &Message) -> Result<Turn, Never> {
  let (on, member) = match (message.interface.as_deref(), message.member.as_deref()) {
    (Some(on), Some(member)) => (on, member),
    (None, Some(member)) => (LOOKING, member),
    (Some(_), None) | (None, None) => {
      let Ok(complaint) =
        message.complaining(ValidationError::UnknownMethod, "a call names a member");

      return Ok(Turn {
        say: Some(complaint),
        changed: Modified::No,
      });
    }
  };

  match (on, member) {
    (LOOKING, "Introspect") => {
      let Ok(answer) = message.answering();
      let Ok(value) = Value::word(XML);
      let Ok(answer_2) = answer.carrying("s", vec![value]);

      Ok(Turn {
        say: Some(answer_2),
        changed: Modified::No,
      })
    }
    (SESSION, "Checkpoint") => {
      let Ok(()) = held.remember();
      let Ok(answer) = message.answering();

      Ok(Turn {
        say: Some(answer),
        changed: Modified::No,
      })
    }
    (PEER, "Ping") => {
      let Ok(answer) = message.answering();

      Ok(Turn {
        say: Some(answer),
        changed: Modified::No,
      })
    }
    (CHANGED, "Get") => {
      let Ok(interface) = word(&message.values, 0);
      let Ok(name) = word(&message.values, 1);
      let said = says(
        held,
        &Property {
          on: &interface,
          name: &name,
        },
      )?;

      Ok(match said {
        Some(said_2) => {
          let Ok(answer) = message.answering();
          let Ok(answer_3) = answer.carrying("v", vec![said_2]);

          Turn {
            say: Some(answer_3),
            changed: Modified::No,
          }
        }
        None => {
          let Ok(complaint) = message.complaining(
            ValidationError::InvalidArgs,
            "this player has no such property",
          );

          Turn {
            say: Some(complaint),
            changed: Modified::No,
          }
        }
      })
    }
    (CHANGED, "GetAll") => {
      let Ok(interface) = word(&message.values, 0);
      let said = says_all(held, &interface)?;
      let Ok(answer) = message.answering();
      let Ok(answer_4) = answer.carrying("a{sv}", vec![said]);

      Ok(Turn {
        say: Some(answer_4),
        changed: Modified::No,
      })
    }
    (CHANGED, "Set") => {
      let Ok(name) = word(&message.values, 1);

      match message.values.get(2) {
        Some(value) => {
          let Ok(()) = player_told(held, &name, value);
        }
        None => {}
      }

      let Ok(answer) = message.answering();

      Ok(Turn {
        say: Some(answer),
        changed: Modified::Yes,
      })
    }
    (answers::ROOT, "Raise" | "Quit") => {
      let Ok(answer) = message.answering();

      Ok(Turn {
        say: Some(answer),
        changed: Modified::No,
      })
    }
    (answers::PLAYER, method) => {
      let Ok(()) = handle_call(held, method, &message.values);
      let Ok(answer) = message.answering();

      Ok(Turn {
        say: Some(answer),
        changed: Modified::Yes,
      })
    }
    (_on, _member) => {
      let Ok(complaint) = message.complaining(
        ValidationError::UnknownMethod,
        "nothing here answers to that",
      );

      Ok(Turn {
        say: Some(complaint),
        changed: Modified::No,
      })
    }
  }
}

pub fn changing(held: &PlayerState) -> Result<Message, Never> {
  let Ok(signal) = Message::signal(&Signal {
    at: answers::OBJECT,
    on: CHANGED,
    name: "PropertiesChanged",
  });
  let mut listed: Vec<Value> = Vec::new();

  for name in ["PlaybackStatus", "Metadata"] {
    let said = says(
      held,
      &Property {
        on: answers::PLAYER,
        name,
      },
    )?;

    match said {
      Some(said_2) => listed.push(Value::Group(vec![Value::Word(name.to_string()), said_2])),
      None => {}
    }
  }

  signal.carrying(
    "sa{sv}as",
    vec![
      Value::Word(answers::PLAYER.to_string()),
      Value::List(listed),
      Value::List(Vec::new()),
    ],
  )
}

pub fn changed(saying: &Sender, held: &PlayerState) -> Result<(), Never> {
  let Ok(signal) = changing(held);

  match saying.say(&signal) {
    Ok(_serial) => {}
    Err(fault) => {
      let Ok(()) = said_nothing(fault);
    }
  }

  Ok(())
}

pub fn moved(saying: &Sender) -> Result<(), Never> {
  let Ok(signal) = Message::signal(&Signal {
    at: answers::OBJECT,
    on: answers::APPLICATION,
    name: answers::POSITION_CHANGED,
  });

  match saying.say(&signal) {
    Ok(_serial) => {}
    Err(fault) => {
      let Ok(()) = said_nothing(fault);
    }
  }

  Ok(())
}

fn said_nothing(fault: ConnectionError) -> Result<(), Never> {
  console_core_logging::warn!("saying what changed: {fault}");

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::ffi::OsStr;
  use std::os::unix::ffi::OsStrExt;

  #[test]
  fn an_untitled_song_escapes_its_filename_when_the_bus_cannot_spell_it() {
    let song = Path::new(OsStr::from_bytes(b"/music/old\xff.opus"));

    assert_eq!(title_or_filename(song, ""), Ok("old%FF".to_string()));
    assert_eq!(
      title_or_filename(song, "A title"),
      Ok("A title".to_string())
    );
  }
}
