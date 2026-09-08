//! What the panel presses, answered where kew used to answer it.
//!
//! The interface is not this player's design and is not this desktop's either:
//! it is MPRIS, spelled the way `console_music_panel::player` already reads it and
//! the way the checks already press it through busctl. So the surface came
//! first and the player was written to it, which is the opposite of how the
//! rest of this tree is built and is right here -- a panel that has to be
//! changed to meet its player would have made the swap unpressable.
//!
//! Everything is answered under one lock. A property read is short; a method
//! that starts a song is not, because it asks ffprobe what the song says and
//! ffmpeg for the picture on the front of it before any sound is made. That is
//! tens of milliseconds inside a bus call, and it is deliberate: the panel
//! wants the title and the sleeve at the same moment it wants the sound, and a
//! player that answered the call first and filled the card in afterwards would
//! draw an empty card every time a song is pressed.
//!
//! `PropertiesChanged` is emitted for the pair a listener actually acts on --
//! what is playing, and whether it is. Nothing here polls to find out, and the
//! panel is welcome to go on asking.

use console_core_never::Never;
use gio::prelude::*;
use glib::Variant;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::answers::{self, Song, Status, Told};
use crate::library;
use crate::playlist::{Moved, Order, Playlist};
use crate::sounding::{Sounding, Wanted};
use crate::{art, tags};

const CHANGED: &str = "org.freedesktop.DBus.Properties";

const XML: &str = r#"<node>
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

pub struct Held {
    pub list: Playlist,
    pub sounding: Sounding,
    pub playing: Song,
    pub folder: Option<PathBuf>,
    pub turn: u64,
}

impl Held {
    pub fn new(sounding: Sounding, folder: Option<PathBuf>) -> Result<Held, Never> {
        let Ok(list) = Playlist::of(Vec::new());

        Ok(Held { list, sounding, playing: Song::default(), folder, turn: 0 })
    }

    pub fn opened(&mut self, at: &Path) -> Result<(), Never> {
        let under = match &self.folder {
            Some(folder) => folder.clone(),
            None => match at.parent() {
                Some(parent) => parent.to_path_buf(),
                None => return Ok(()),
            },
        };

        let Ok(songs) = library::songs_under(&under);
        let Ok(list) = Playlist::opened(songs, at);

        self.list = list;

        self.started()
    }

    pub fn started(&mut self) -> Result<(), Never> {
        let Ok(song) = self.list.song();

        let song = match song {
            Some(song) => song.to_path_buf(),
            None => return Ok(()),
        };

        self.turn = self.turn.saturating_add(1);

        let Ok(said) = tags::playing(&song);
        let Ok(art) = art::of(&song, self.turn);
        let Ok(title) = named(&song, &said.title);

        self.playing = Song {
            title,
            artist: said.artist,
            album: said.album,
            art,
            length: said.length,
        };

        self.sounding.play(&song, 0.0)
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
        let Ok(wanted) = self.sounding.wanted();

        match wanted {
            Wanted::Playing => self.sounding.wanting(Wanted::Paused),
            Wanted::Paused => self.sounding.wanting(Wanted::Playing),
            Wanted::Stopped => self.started(),
        }
    }

    pub fn status(&self) -> Result<Status, Never> {
        let Ok(wanted) = self.sounding.wanted();

        Ok(match wanted {
            Wanted::Playing => Status::Playing,
            Wanted::Paused => Status::Paused,
            Wanted::Stopped => Status::Stopped,
        })
    }
}

fn named(song: &Path, title: &str) -> Result<String, Never> {
    match title.is_empty() {
        false => return Ok(title.to_string()),
        true => {},
    }

    Ok(match song.file_stem().map(|stem| stem.to_string_lossy().to_string()) {
        Some(stem) => stem,
        None => String::new(),
    })
}

fn locked(held: &Arc<Mutex<Held>>) -> Result<MutexGuard<'_, Held>, Never> {
    Ok(match held.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    })
}

fn told(said: &Told) -> Result<Variant, Never> {
    Ok(match said {
        Told::Track(path) => match glib::variant::ObjectPath::try_from(path.clone()) {
            Ok(held) => held.to_variant(),
            Err(_not_an_object_path) => path.to_variant(),
        },
        Told::Word(word) => word.to_variant(),
        Told::Words(words) => words.to_variant(),
        Told::Long(long) => long.to_variant(),
    })
}

fn metadata(held: &Held) -> Result<Variant, Never> {
    let Ok(said) = answers::metadata(&held.playing);
    let dict = glib::VariantDict::new(None);

    for (name, value) in said {
        let Ok(value) = told(&value);

        dict.insert_value(&name, &value);
    }

    Ok(dict.end())
}

fn root_says(name: &str) -> Result<Variant, Never> {
    Ok(match name {
        "Identity" => answers::IDENTITY.to_variant(),
        "SupportedUriSchemes" => vec!["file".to_string()].to_variant(),
        "SupportedMimeTypes" => Vec::<String>::new().to_variant(),
        "CanQuit" => true.to_variant(),
        "CanRaise" | "HasTrackList" => false.to_variant(),
        _unknown_to_this_player => false.to_variant(),
    })
}

fn player_says(held: &Arc<Mutex<Held>>, name: &str) -> Result<Variant, Never> {
    let Ok(held) = locked(held);

    Ok(match name {
        "PlaybackStatus" => {
            let Ok(status) = held.status();
            let Ok(said) = status.said();

            said.to_variant()
        },
        "LoopStatus" => {
            let Ok(over) = held.list.repeating();
            let Ok(said) = answers::over_said(over);

            said.to_variant()
        },
        "Shuffle" => {
            let Ok(order) = held.list.ordering();

            matches!(order, Order::Any).to_variant()
        },
        "Metadata" => {
            let Ok(said) = metadata(&held);

            said
        },
        "Position" => {
            let Ok(at) = held.sounding.position();
            let Ok(micros) = answers::micros(at);

            micros.to_variant()
        },
        "Rate" | "MinimumRate" | "MaximumRate" | "Volume" => 1.0_f64.to_variant(),
        "CanGoNext" | "CanGoPrevious" | "CanPlay" | "CanPause" | "CanSeek" | "CanControl" => {
            true.to_variant()
        },
        _unknown_to_this_player => false.to_variant(),
    })
}

fn player_told(held: &Arc<Mutex<Held>>, name: &str, value: &Variant) -> Result<(), Never> {
    let Ok(mut held) = locked(held);

    match name {
        "LoopStatus" => {
            let said = value.get::<String>().unwrap_or_default();
            let Ok(over) = answers::over_read(&said);

            held.list.repeat(over)
        },
        "Shuffle" => {
            let order = match value.get::<bool>().unwrap_or_default() {
                true => Order::Any,
                false => Order::AsListed,
            };
            let Ok(seed) = seed();

            held.list.shuffling(order, seed)
        },
        _nothing_else_here_can_be_set => Ok(()),
    }
}

fn seed() -> Result<u64, Never> {
    Ok(match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(since) => u64::from(since.subsec_nanos()),
        Err(_the_clock_is_before_the_epoch) => 1,
    })
}

fn asked(held: &Arc<Mutex<Held>>, method: &str, params: &Variant) -> Result<(), Never> {
    let Ok(mut held) = locked(held);

    match method {
        "Next" => held.onward(),
        "Previous" => held.back(),
        "PlayPause" => held.play_pause(),
        "Pause" => held.sounding.wanting(Wanted::Paused),
        "Stop" => held.sounding.wanting(Wanted::Stopped),
        "Play" => {
            let Ok(wanted) = held.sounding.wanted();

            match wanted {
                Wanted::Paused => held.sounding.wanting(Wanted::Playing),
                Wanted::Playing => Ok(()),
                Wanted::Stopped => held.started(),
            }
        },
        "OpenUri" => {
            let Ok(at) = word(params, 0);
            let Ok(at) = local(&at);

            held.opened(&at)
        },
        "SetPosition" => {
            let Ok(micros) = long(params, 1);
            let Ok(seconds) = answers::seconds(micros);

            held.sounding.seek(seconds)
        },
        "Seek" => {
            let Ok(micros) = long(params, 0);
            let Ok(by) = answers::seconds(micros);
            let Ok(at) = held.sounding.position();

            held.sounding.seek(at + by)
        },
        _nothing_else_is_offered => Ok(()),
    }
}

fn word(params: &Variant, at: usize) -> Result<String, Never> {
    let held = match params.try_child_get::<String>(at) {
        Ok(held) => held,
        Err(_not_a_word_there) => None,
    };

    Ok(held.unwrap_or_default())
}

fn long(params: &Variant, at: usize) -> Result<i64, Never> {
    let held = match params.try_child_get::<i64>(at) {
        Ok(held) => held,
        Err(_not_a_number_there) => None,
    };

    Ok(held.unwrap_or_default())
}

fn local(said: &str) -> Result<PathBuf, Never> {
    Ok(match said.strip_prefix("file://") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(said),
    })
}

pub fn changed(connection: &gio::DBusConnection, held: &Arc<Mutex<Held>>) -> Result<(), Never> {
    let dict = glib::VariantDict::new(None);

    let Ok(status) = player_says(held, "PlaybackStatus");
    let Ok(said) = player_says(held, "Metadata");

    dict.insert_value("PlaybackStatus", &status);
    dict.insert_value("Metadata", &said);

    let body = (answers::PLAYER.to_string(), dict.end(), Vec::<String>::new()).to_variant();

    match connection.emit_signal(None, answers::OBJECT, CHANGED, "PropertiesChanged", Some(&body)) {
        Ok(()) => {},
        Err(fault) => eprintln!("music-player: saying what changed: {fault}"),
    }

    Ok(())
}

pub fn serve(held: &Arc<Mutex<Held>>) -> Result<Option<gio::DBusConnection>, Never> {
    let connection = match gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) {
        Ok(connection) => connection,
        Err(fault) => {
            eprintln!("music-player: no session bus, so nothing can ask for a song: {fault}");

            return Ok(None);
        },
    };

    let node = match gio::DBusNodeInfo::for_xml(XML) {
        Ok(node) => node,
        Err(fault) => {
            eprintln!("music-player: the interface will not parse: {fault}");

            return Ok(None);
        },
    };

    let root = match node.lookup_interface(answers::ROOT) {
        Some(root) => root,
        None => return Ok(None),
    };

    let player = match node.lookup_interface(answers::PLAYER) {
        Some(player) => player,
        None => return Ok(None),
    };

    let _the_root_answers_for_as_long_as_this_runs = connection
        .register_object(answers::OBJECT, &root)
        .method_call(|_connection, _sender, _path, _interface, _method, _params, invocation| {
            invocation.return_value(None);
        })
        .property(|_connection, _sender, _path, _interface, name| {
            let Ok(said) = root_says(name);

            said
        })
        .build();

    let saying = Arc::clone(held);
    let telling = Arc::clone(held);
    let calling = Arc::clone(held);

    let _the_player_answers_for_as_long_as_this_runs = connection
        .register_object(answers::OBJECT, &player)
        .method_call(move |connection, _sender, _path, _interface, method, params, invocation| {
            let Ok(()) = asked(&calling, method, &params);

            invocation.return_value(None);

            let Ok(()) = changed(&connection, &calling);
        })
        .property(move |_connection, _sender, _path, _interface, name| {
            let Ok(said) = player_says(&saying, name);

            said
        })
        .set_property(move |_connection, _sender, _path, _interface, name, value| {
            let Ok(()) = player_told(&telling, name, &value);

            true
        })
        .build();

    let _the_name_is_held_for_as_long_as_this_runs = gio::bus_own_name_on_connection(
        &connection,
        answers::NAME,
        gio::BusNameOwnerFlags::NONE,
        |_connection, _name| {},
        |_connection, _name| {
            eprintln!("music-player: another player has the name, so this one answers to nothing");
        },
    );

    Ok(Some(connection))
}
