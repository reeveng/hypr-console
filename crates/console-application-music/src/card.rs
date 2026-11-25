//! The player and Music Library share the songs and Now Playing views.
//!
//! The transport speaks MPRIS to one session player. The management app also
//! has album and recording review tabs whose actions go through the same
//! card actor. The panel by the bar has only Now Playing.

mod catalog;
mod metadata;

use catalog::{albums_page, manage_effect, review_page};
use console_core_arguments::{Command, Operands};
use console_core_iteration::{Step, iterate};
use console_core_localization::positional;
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted};
use metadata::suggestions_page;
use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;

use crate::ascii;
use crate::library::folder;
use crate::library::{self, Kind};
use crate::looking::{self, Song};
use crate::management::{Job, Request as Manage, Status, Work};
use crate::player::{self, Order, Over, Playing, Sound};
use crate::update::{Closes, Music, MusicEffect, MusicEvent, Standing, closes};
use crate::{album_art, albums, review};
use console_actor::Actor;
use console_music_player::selection::{Selection, Visibility};
use console_panel::card::{Card, Door};
use console_panel::icons::Icon;
use console_panel::page::{
  ActionOutcome, Active, Aside, Bar, ButtonPress, Handler, LoadedRows, Page, Picture, Row, Rows,
  Showing,
};
use console_panel::running;
use console_program_contract::{
  Command as RunCommand, Effect, Event, EventGroup, ExitStatus, Notification,
};

const TALL: u32 = 8;

const ABOUT: &str = "Search songs, artists or albums";

const MANY: u32 = 120;

const SCRUB: i64 = 5_000_000;

type Panel = Actor<Music>;

fn panel_state(held: &Panel) -> Result<Standing, Never> {
  Ok(match held.get() {
    Ok(standing) => standing,
    Err(_the_actor_has_gone) => {
      eprintln!("music-panel: the panel's own state is missing, so it drew as it opened");

      Standing::default()
    }
  })
}

fn decided(held: &Panel, heard: MusicEvent) -> Result<Vec<Effect<MusicEffect>>, Never> {
  Ok(match held.send(Event::Custom(heard)) {
    Ok(effects) => effects,
    Err(_the_actor_has_gone) => {
      eprintln!("music-panel: the panel's own state is missing, so the press did nothing");

      Vec::new()
    }
  })
}

fn press(held: &Panel, heard: MusicEvent, showing: &dyn Showing) -> Result<(), Never> {
  let Ok(effects) = decided(held, heard);
  let pending = VecDeque::from(effects);
  let drained = iterate(pending, |mut pending_2| {
    let effect = match pending_2.pop_front() {
      Some(effect) => effect,
      None => return Ok(Step::Halt(())),
    };
    let Ok(reply) = carry(held, &effect, showing);

    match reply {
      Some(request) => {
        let Ok(effects_2) = decided(held, MusicEvent::Manage(request));

        for effect_2 in effects_2.into_iter().rev() {
          pending_2.push_front(effect_2);
        }
      }
      None => {}
    }

    Ok(Step::Again(pending_2))
  });

  match drained {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("could not finish Music Library effects: {fault}"),
  }

  Ok(())
}

fn carry(
  held: &Panel,
  effect: &Effect<MusicEffect>,
  showing: &dyn Showing,
) -> Result<Option<Manage>, Never> {
  match effect {
    Effect::Custom(MusicEffect::Replace(row)) => {
      let Ok(row_2) = fitted(*row);

      showing.replace(row_2)
    }
    Effect::Custom(MusicEffect::Notify(said)) => {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Music Library"),
        said,
        None,
      );
    }
    Effect::Custom(MusicEffect::ForgetTyping) => showing.forget_typing(),
    Effect::Custom(MusicEffect::Shuffle(order)) => {
      player::shuffle(*order)?;
      showing.refresh();
    }
    Effect::Custom(MusicEffect::Repeat(over)) => {
      player::repeat(*over)?;
      showing.refresh();
    }

    Effect::Custom(MusicEffect::Manage(work)) => return manage_effect(held, work, showing),
    Effect::Custom(MusicEffect::Play(path)) => play_track(path, showing)?,
    Effect::Run(runs) => {
      let Ok(task) = console_panel::tasks::Task::of(runs.clone());

      showing.run(task);
    }
    Effect::Spawn(runs) => {
      let whole = running::words_of(runs)?;

      showing.leave_running(whole);
    }

    Effect::Stream(_)
    | Effect::Fork(_)
    | Effect::Interrupt(_)
    | Effect::Prompt(_)
    | Effect::Subscribe(_)
    | Effect::Unsubscribe(_)
    | Effect::Write(_)
    | Effect::Notify(_)
    | Effect::Print(_)
    | Effect::Stop(_) => {}
  }

  Ok(None)
}

fn play_track(path: &Path, showing: &dyn Showing) -> Result<(), Never> {
  let folder = folder()?;
  let words = player::opening_words(path, &folder)?;
  let args: Vec<_> = words.iter().map(String::as_str).collect();
  let Ok(command) = RunCommand::internal(
    console_core_internal_programs::InternalProgram::MusicPlayer,
    &args,
  );
  let Ok(start) = running::words_of(&command);
  showing.leave_running(start);

  let onward = player::onward_for(path)?;
  let args_2: Vec<_> = onward.iter().skip(1).map(String::as_str).collect();
  let Ok(command_2) = RunCommand::internal(crate::update::ONWARD, &args_2);
  let Ok(task) = console_panel::tasks::Task::of(command_2);
  showing.run(task);

  Ok(())
}

fn playing_rows(held: &Panel) -> Result<Vec<Row>, Never> {
  let asked = player::playing()?;

  match asked.as_ref() {
    Some(playing) => {
      match ::core::primitive::bool::then_some(playing.sound != Sound::Stopped, ()) {
        ::core::option::Option::Some(()) => playing_card(held, playing),
        ::core::option::Option::None => {
          let Ok(row) = Row::placeholder("Not Playing");

          Ok(vec![row])
        }
      }
    }
    None => {
      let Ok(row) = Row::placeholder("Not Playing");

      Ok(vec![row])
    }
  }
}

fn typed_in(held: &Panel) -> Result<String, Never> {
  let standing = panel_state(held)?;

  Ok(standing.typed.trim().to_string())
}

fn press_at(held: &Panel) -> Result<u32, Never> {
  let standing = panel_state(held)?;

  Ok(standing.press)
}

fn playing_card(held: &Panel, playing: &Playing) -> Result<Vec<Row>, Never> {
  let cover = match playing.art.as_deref() {
    Some(art) => ascii::read(art, TALL)?,
    None => None,
  };

  let drawn = match cover {
    Some(cover_2) => cover_2,
    None => ascii::room(TALL)?,
  };

  let under = match ::core::primitive::bool::then_some(playing.album.trim().is_empty(), ()) {
    ::core::option::Option::Some(()) => playing.artist.clone(),
    ::core::option::Option::None => format!("{} \u{2014} {}", playing.artist, playing.album),
  };

  let markup = drawn.markup()?;
  let head = head_row(Head {
    sleeve: &markup,
    title: &playing.title,
    under: under.trim(),
  })?;
  let scrub = scrub_row()?;
  let at = press_at(held)?;
  let transport = transport_row(held, playing, at)?;
  let walking = walking(held, transport)?;
  let Ok(chief) = walking.chief();
  let rows = vec![head, scrub, chief];

  Ok(rows)
}

fn walking(held: &Panel, row: Row) -> Result<Row, Never> {
  let Ok(of) = console_core_number_conversion::fitted::<_, u32>(
    row
      .buttons
      .as_ref()
      .map_or(0, |across| across.presses.len()),
  );
  let held_2 = held.clone();

  row.adjusting(move |by| {
    let ::core::result::Result::Ok(_) = decided(&held_2, MusicEvent::Along { by, of });
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Head<'a> {
  sleeve: &'a str,
  title: &'a str,
  under: &'a str,
}

fn head_row(head: Head<'_>) -> Result<Row, Never> {
  let Head {
    sleeve,
    title,
    under,
  } = head;

  Row::stacked(Picture::Written(sleeve.to_string()), title, Aside(under))
}

fn scrub_row() -> Result<Row, Never> {
  let position = player::position()?;
  let total = player::length()?;
  let done = clock(position)?;

  let whole = match ::core::primitive::bool::then_some(total > 0, ()) {
    ::core::option::Option::Some(()) => clock(total)?,
    ::core::option::Option::None => String::new(),
  };

  let scrub = Scrub {
    at: position,
    of: total,
  };
  let bar = scrub_bar(scrub)?;

  let Ok(nothing) = Handler::and_stay(|_| {});
  let Ok(row) = Row::new(&done, Aside(&whole), nothing);
  let Ok(row_2) = row.picturing(Picture::Bar(bar));
  let Ok(row_3) = row_2.adjusting(move |directory| {
    let Ok(step) = scrub_to(scrub, directory);
    let Ok(()) = player::seek(step);
  });

  row_3.seeking(|showing, frac| {
    let Ok(()) = player::seek(player::TrackFraction(frac));

    showing.refresh();
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Scrub {
  at: i64,
  of: i64,
}

fn scrub_bar(scrub: Scrub) -> Result<Bar, Never> {
  let Ok(at) = fitted(scrub.at.max(0));
  let Ok(of) = fitted(scrub.of.max(0));

  Ok(Bar { at, of })
}

fn clock(micros: i64) -> Result<String, Never> {
  let Ok(micros_2) = fitted::<i64, u64>(micros);

  positional(Duration::from_micros(micros_2))
}

fn scrub_to(scrub: Scrub, directory: i32) -> Result<player::TrackFraction, Never> {
  let Scrub { at, of } = scrub;
  let target = at
    .saturating_add(i64::from(directory).saturating_mul(SCRUB))
    .clamp(0, of);

  let Ok(along) = target.float();
  let Ok(whole) = of.max(1).float();

  Ok(player::TrackFraction(along / whole))
}

fn transport_row(held: &Panel, playing: &Playing, at: u32) -> Result<Row, Never> {
  let shuffling = player::shuffling()?;
  let over = player::over()?;
  let shuffle = held.clone();
  let repeat = held.clone();

  let Ok(shuffles) = ButtonPress::new(
    Icon::Shuffle,
    match shuffling {
      Order::Any => Active::Yes,
      Order::AsListed => Active::No,
    },
    move |showing| {
      let Ok(shuffling_2) = player::shuffling();

      let Ok(()) = press(&shuffle, MusicEvent::Shuffling(shuffling_2), showing);
    },
  );
  let Ok(previous) = ButtonPress::new(Icon::Previous, Active::No, |showing| {
    let Ok(()) = player::previous();

    showing.refresh();
  });
  let Ok(playing_2) = ButtonPress::new(
    match playing.sound {
      Sound::Paused | Sound::Stopped => Icon::Play,
      Sound::Playing => Icon::Pause,
    },
    Active::No,
    |showing| {
      let Ok(()) = player::play_pause();

      showing.refresh();
    },
  );
  let Ok(playing_3) = playing_2.chief();
  let Ok(next) = ButtonPress::new(Icon::Next, Active::No, |showing| {
    let Ok(()) = player::next();

    showing.refresh();
  });
  let Ok(repeats) = ButtonPress::new(
    match over {
      Over::Again => Icon::RepeatSong,
      Over::On | Over::Round => Icon::Repeat,
    },
    match over {
      Over::On => Active::No,
      Over::Again | Over::Round => Active::Yes,
    },
    move |showing| {
      let Ok(over_2) = player::over();

      let Ok(()) = press(&repeat, MusicEvent::Repeating(over_2), showing);
    },
  );
  let presses = vec![shuffles, previous, playing_3, next, repeats];
  let Ok(at_2) = fitted(at);

  Row::pressing(presses, at_2)
}

fn music_rows(held: &Panel, folder: &Path, songs: Vec<Song>) -> Result<Vec<Row>, Never> {
  let typed = typed_in(held)?;

  match ::core::primitive::bool::then_some(typed.is_empty(), ()) {
    ::core::option::Option::Some(()) => in_the_folder(held, folder, songs),
    ::core::option::Option::None => answering(held, folder, &songs, &typed),
  }
}

fn in_the_folder(held: &Panel, folder: &Path, mut listed: Vec<Song>) -> Result<Vec<Row>, Never> {
  match listed.is_empty().then_some(()) {
    Some(()) => {
      let Ok(row) = Row::placeholder(&format!("No Music in {}", folder.display()));

      return Ok(vec![row]);
    }
    None => {}
  }

  listed.sort_by_cached_key(|song| {
    let Ok(name) = song.says();
    let Ok(rank) = console_panel::page::sort_rank(name);

    (rank, name.to_lowercase())
  });

  let listed_rows: Result<Vec<Row>, Never> = listed
    .iter()
    .map(|song| played(held, song, folder))
    .collect();
  let Ok(rows) = listed_rows;

  console_panel::page::lettered(rows)
}

fn selection() -> Result<Selection, Never> {
  Ok(match Selection::load() {
    Ok(selection) => selection,
    Err(fault) => {
      console_core_logging::warn!("could not read Music Library selections: {fault}");

      Selection::default()
    }
  })
}

fn visible_songs(folder: &Path) -> Result<Vec<Song>, Never> {
  let listed = songs(folder)?;

  included(listed)
}

fn included(mut songs: Vec<Song>) -> Result<Vec<Song>, Never> {
  let Ok(()) = crate::suggestions::apply_saved(&mut songs);
  let selected = selection()?;
  songs.retain(|song| selected.visibility(&song.path) == Ok(Visibility::Included));

  Ok(songs)
}

fn answering(held: &Panel, folder: &Path, songs: &[Song], word: &str) -> Result<Vec<Row>, Never> {
  let found = looking::ranked(songs, word)?;

  match ::core::primitive::bool::then_some(found.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(row) = Row::placeholder(&format!("No Results for \u{201c}{word}\u{201d}"));

      return Ok(vec![row]);
    }
    ::core::option::Option::None => {}
  }

  let mut rows: Vec<Row> = Vec::new();
  let Ok(most) = console_core_number_conversion::index(MANY);

  for song in found.iter().take(most) {
    let row = played(held, song, folder)?;

    rows.push(row);
  }

  Ok(rows)
}

fn songs(folder: &Path) -> Result<Vec<Song>, Never> {
  let Ok(cache) = console_core_places::Base::Cache.user();

  let cache_3 = match cache {
    Some(cache_2) => cache_2,
    None => return looking::songs(folder, &library::things, &[]),
  };

  let at = looking::at(&cache_3)?;
  let known = looking::load(&at)?;

  looking::songs(folder, &library::things, &known)
}

fn played(held: &Panel, song: &Song, folder: &Path) -> Result<Row, Never> {
  let says = song.says()?;
  let aside = song.aside(folder)?;
  let plays = plays(held, &song.path, Kind::ASong)?;
  let shows = shown_in_the_files(held, &song.path)?;
  let Ok(offering) = console_panel::page::shown_or_selected(says, &song.path, shows);

  let Ok(row) = Row::new(says, Aside(&aside), plays);
  let Ok(key) = player::on_bus(&song.path);
  let Ok(row_2) = row.selectable(&key);

  row_2.offering(offering)
}

fn shown_in_the_files(
  held: &Panel,
  path: &Path,
) -> Result<impl Fn(&dyn Showing) + Send + Sync + 'static, Never> {
  let held_2 = held.clone();
  let path_2 = path.to_path_buf();

  Ok(move |_: &dyn Showing| {
    let Ok(effects) = decided(&held_2, MusicEvent::Shown(path_2.clone()));

    for effect in &effects {
      let Ok(runs) = effect.as_spawn();

      match runs {
        Some(runs_2) => {
          let Ok(whole) = running::words_of(runs_2);
          let Ok(()) = running::left_running(&whole);
        }
        None => {}
      }
    }
  })
}

fn plays(held: &Panel, path: &Path, folder: Kind) -> Result<Handler, Never> {
  let held_2 = held.clone();
  let path_2 = path.to_path_buf();

  Handler::and_stay(move |showing| {
    let Ok(()) = press(
      &held_2,
      MusicEvent::Chose {
        path: path_2.clone(),
        kind: folder,
      },
      showing,
    );
  })
}

fn playing_page(held: &Panel) -> Result<Page, Never> {
  let showing = held.clone();

  let Ok(asked) = Rows::computed(move || {
    let Ok(rows) = playing_rows(&showing);

    rows
  });
  let Ok(page) = Page::new("Now Playing", asked);
  let Ok(page_2) = page.in_the_middle();

  page_2.with_subscription(EventGroup::Player, player::worth_moving_the_clock)
}

fn music_page(held: &Panel, purpose: Holds) -> Result<Page, Never> {
  let reading = held.clone();
  let backing = held.clone();
  let typing = held.clone();

  let Ok(asked) = Rows::with_completion(move || {
    let Ok(loaded) = library_rows(&reading, purpose);

    loaded
  });
  let Ok(page) = Page::new("All Songs", asked);
  let Ok(opening) = page.reading_after_first_frame();
  let Ok(waiting) = opening.meanwhile(|| {
    let Ok(row) = Row::placeholder("Loading songs…");

    vec![row]
  });
  let Ok(page_2) = waiting.trashing_selected();
  let Ok(page_3) = page_2.on_back(move |showing| {
    let Ok(was) = panel_state(&backing);

    let Ok(()) = press(&backing, MusicEvent::Back, showing);

    let Ok(closes) = closes(&was);

    match closes {
      Closes::Yes => ActionOutcome::Close,
      Closes::No => ActionOutcome::Stay,
    }
  });

  let Ok(searchable) = page_3.searching(ABOUT, move |showing, word| {
    let Ok(()) = press(&typing, MusicEvent::Typed(word.to_string()), showing);
  });

  match purpose {
    Holds::Management => {
      searchable.with_subscription(EventGroup::Player, player::worth_asking_after)
    }
    Holds::Library | Holds::Playing => Ok(searchable),
  }
}

fn library_rows(held: &Panel, purpose: Holds) -> Result<LoadedRows, Never> {
  let folder = folder()?;
  let catalog = songs(&folder)?;
  let event = match purpose {
    Holds::Library | Holds::Playing => {
      let Ok(unread) = looking::unread(&catalog);

      MusicEvent::Arrived { unread }
    }
    Holds::Management => MusicEvent::Manage(Manage::Opened),
  };
  let visible = included(catalog)?;
  let mut rows = music_rows(held, &folder, visible)?;

  match purpose {
    Holds::Management => {
      let Ok(typed) = typed_in(held);

      match typed.is_empty().then_some(()) {
        Some(()) => {
          let Ok(convert) = convert_music_row();
          rows.insert(0, convert);
        }
        None => {}
      }

      let shortcut = now_playing_shortcut()?;

      match shortcut {
        Some(row) => rows.insert(0, row),
        None => {}
      }
    }
    Holds::Library | Holds::Playing => {}
  }

  let after = held.clone();
  let Ok(loaded) = LoadedRows::new(rows);

  loaded.then(move |showing| {
    let Ok(()) = press(&after, event, showing);
  })
}

fn convert_music_row() -> Result<Row, Never> {
  use console_core_internal_programs::InternalProgram::DownloadsFormat;
  use console_panel::page::Subject;
  use console_panel::tasks::Task;

  let Ok(command) = RunCommand::internal(DownloadsFormat, &["--music-library"]);
  let requested = command.clone();
  let Ok(does) = Handler::and_stay(move |showing| {
    let convert = requested.clone();

    showing.sure(
      "Convert the Music Library to Opus?",
      Subject("Includes album folders. Re-encoding is lossy. Originals go to Trash."),
      &["Convert"],
      Box::new(move |confirmed, _| {
        let Ok(task) = Task::of(convert);
        let Ok(attributed) =
          task.from_application(console_notifications::Application("Music Library"));
        let Ok(tracked) = attributed.then(|answer| match &answer.status {
          ExitStatus::Success | ExitStatus::Interrupt => Ok(None),
          ExitStatus::Failure(_) => {
            let Ok(note) = Notification::of("Music Library conversion could not finish");

            Ok(Some(note))
          }
        });
        confirmed.run(tracked);
      }),
    );
  });
  let Ok(row) = Row::new("Convert Music to Opus", Aside(""), does);

  row.while_running(&command, "Converting Music to Opus")
}

fn now_playing_shortcut() -> Result<Option<Row>, Never> {
  let playing = player::playing()?;
  let playing_3 = match playing {
    Some(playing_2) => match playing_2.sound {
      Sound::Playing | Sound::Paused => playing_2,
      Sound::Stopped => return Ok(None),
    },
    None => return Ok(None),
  };
  let Ok(does) = Handler::and_stay(|showing| showing.turn_to(4));
  let title = format!(
    "{} · {}",
    match playing_3.sound {
      Sound::Playing => "Now Playing",
      Sound::Paused => "Paused",
      Sound::Stopped => "Stopped",
    },
    playing_3.title
  );
  let Ok(row) = Row::new(&title, Aside(&playing_3.artist), does);
  let Ok(row_2) = row.opening();

  Ok(Some(row_2))
}

pub const WHO: &str = "music-panel";

pub const APP: &str = "music";

const DOOR: &str = "music";

pub const COMMAND: Command = Command {
  name: "music-panel",
  about: "what is playing, and what plays next",
  flags: &[],
  operands: Operands::None,
};

pub fn door(_argv: &[String]) -> Result<Door, Never> {
  Door::closing(DOOR)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holds {
  Playing,
  Library,
  Management,
}

fn held_as(holds: Holds) -> Result<Card, Never> {
  Card::supervised::<Music, _>((), move |held| {
    let Ok(playing) = playing_page(held);

    Ok(match holds {
      Holds::Playing => vec![playing],
      Holds::Library => {
        let Ok(music) = music_page(held, holds);

        vec![music, playing]
      }
      Holds::Management => {
        let Ok(music) = music_page(held, holds);
        let Ok(albums) = albums_page(held);
        let Ok(review) = review_page(held);
        let Ok(suggestions) = suggestions_page(held);

        vec![music, albums, review, suggestions, playing]
      }
    })
  })
}

pub fn card(_argv: &[String]) -> Result<Card, Never> {
  held_as(Holds::Playing)
}

pub fn library(_argv: &[String]) -> Result<Card, Never> {
  let card = held_as(Holds::Library)?;

  card.opening_at(Some("All Songs"))
}

pub fn management(_argv: &[String]) -> Result<Card, Never> {
  let card = held_as(Holds::Management)?;

  card.opening_at(Some("All Songs"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_panel::page::{Heading, Same};
  use std::error::Error;

  const SINGLE: i64 = 180_000_000;
  const MIX: i64 = 4_380_000_000;

  fn card() -> Result<Vec<Row>, Never> {
    let Ok(room) = ascii::room(TALL);
    let Ok(markup) = room.markup();

    let Ok(head) = head_row(Head {
      sleeve: &markup,
      title: "Blue Monday",
      under: "New Order \u{2014} Power, Corruption & Lies",
    });

    let Ok(scrub) = scrub_row();
    let Ok(play) = ButtonPress::new(Icon::Play, Active::No, |_| ());
    let Ok(pressing) = Row::pressing(vec![play], 0);

    Ok(vec![head, scrub, pressing])
  }

  #[test]
  fn nothing_on_the_card_that_cannot_be_pressed_is_stood_on() {
    let Ok(rows) = card();

    assert_eq!(
      rows.first().map(Row::heading),
      Some(Ok(Heading::Yes)),
      "the head is stood on for nothing",
    );
    assert_eq!(
      rows
        .iter()
        .filter(|row| row.heading() == Ok(Heading::Yes))
        .count(),
      1,
      "a card about one song reads more than its head",
    );
  }

  #[test]
  fn the_sleeve_keeps_its_room_before_there_is_a_cover_for_it() -> Result<(), Box<dyn Error>> {
    let Ok(room) = ascii::room(TALL);
    let Ok(markup) = room.markup();

    let Ok(empty) = head_row(Head {
      sleeve: &markup,
      title: "Blue Monday",
      under: "New Order",
    });

    let Ok(plain) = room.plain();

    let tall = u32::try_from(plain.lines().count())?;

    assert_eq!(tall, TALL);
    assert!(
      plain
        .lines()
        .all(|line| u32::try_from(line.chars().count()) == Ok(room.columns))
    );

    let Ok(bare) = Row::stacked(Picture::None, "Blue Monday", Aside("New Order"));

    assert_eq!(empty.looks_like(&bare), Ok(Same::No));

    Ok(())
  }

  #[test]
  fn the_bar_steps_by_the_same_few_seconds_whatever_the_song_is() {
    let moved = |total: i64| {
      let Ok(scrub_to) = scrub_to(
        Scrub {
          at: 30_000_000,
          of: total,
        },
        1,
      );
      let Ok(whole) = total.float();

      (scrub_to.0 * whole - 30_000_000.0) / 1_000_000.0
    };

    assert!(
      (moved(SINGLE) - 5.0).abs() < 0.001,
      "a single moved {}",
      moved(SINGLE)
    );
    assert!(
      (moved(MIX) - 5.0).abs() < 0.001,
      "a mix moved {}",
      moved(MIX)
    );
  }

  #[test]
  fn the_bar_stops_at_both_ends_of_the_song() {
    let Ok(back) = scrub_to(
      Scrub {
        at: 1_000_000,
        of: SINGLE,
      },
      -1,
    );
    let Ok(on) = scrub_to(
      Scrub {
        at: SINGLE.saturating_sub(1_000_000),
        of: SINGLE,
      },
      1,
    );

    assert!(back.0 < f64::EPSILON);
    assert!((on.0 - 1.0).abs() < f64::EPSILON);
  }
}
