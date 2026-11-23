//! The files, drawn.
//!
//! What is here is the reading of the disk and the wiring of it to the panel.
//! What a listing comes to once it has been read is `console_files`, where it can
//! be asked without a disk to ask.
//!
//! Every tab is a place, stands somewhere inside it, and is looking either at
//! that folder or at a question about one thing in it. That is the one thing
//! this holds between one drawing and the next, and it is held behind a lock
//! because the panel reads its rows on a thread of its own: a folder on a stick
//! over USB takes long enough that reading it where the drawing happens would
//! stop the panel answering the buttons.
//!
//! The way back is the first row of every list, under the line to type in where
//! there is one. B does the same thing and a finger has no B: the panel's own
//! way out is the ×, which closes the whole thing, so without that row a
//! question opened by touch could only be left by putting the device down and
//! picking up the controller.
//!
//! The slow half is not done here either. Copying a film off a stick takes
//! seconds and the panel would be deaf for all of them, so anything that writes
//! is handed to the panel as a task, which runs it on a thread and draws the
//! folder again when it is done. An archive being unpacked and a picture being
//! made the wallpaper say so on their own rows for as long as that takes, and
//! so does the row that converts a folder, because each of those is a minute
//! in which the row looked like it had not been pressed. They are the commands rather than the library calls for the
//! same reason the settings ask `pactl` about the volume: the panel is a thing
//! that draws, and `mv` already knows what moving across two disks means.

use console_core_external_programs::Program;
use console_core_never::Never;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::doing::{self, Carrying, FileAction, Holding, ProgramUse};
use crate::listing::{self, Entry, Room, Shown, Still, Worth};
use crate::looking::{self, Found};
use crate::paths::{self, ChildName, InvalidName, NonTextPath};
use crate::places::{self, Place, WANTED};
use crate::standing::{
  self, Closes, Destination, Files, FilesEffect, FilesEvent, HERE_START, LINE, Line, Standing,
  WAYS_START, closes, first_thing,
};
use crate::thumbnails;
use crate::unzipping::Packed;
use console_actor::Actor;
use console_content_types::Table;
use console_core_internal_programs::InternalProgram;
use console_core_places::Folder;
use console_default_applications::editors::{self, Application};
use console_panel::card::Card;
use console_panel::icons::Icon;
use console_panel::page::{
  ActionOutcome, Answer, Aside, Confirmation, Handler, Heading, Initial, Page, Picture, Row,
  Rows, Showing, Subject,
};
use console_panel::tasks::Task;
use console_program_contract::Command;
use console_program_contract::{Effect, Event};

const NOTHING_SAYS_WHAT_IT_IS: &str = "";

type ActorAddress = Actor<Files>;

fn standing_of(held: &ActorAddress) -> Result<Standing, Never> {
  match held.get() {
    Ok(standing) => Ok(standing),
    Err(_the_actor_has_gone) => {
      console_core_logging::warn!("the panel's own state is missing, so it drew nothing");

      Standing::of(Vec::new())
    }
  }
}

fn decided(held: &ActorAddress, heard: FilesEvent) -> Result<Vec<Effect<FilesEffect>>, Never> {
  Ok(match held.send(Event::Custom(heard)) {
    Ok(effects) => effects,
    Err(_the_actor_has_gone) => {
      console_core_logging::warn!("the panel's own state is missing, so the press did nothing");

      Vec::new()
    }
  })
}

fn press(held: &ActorAddress, heard: FilesEvent, showing: &dyn Showing) -> Result<(), Never> {
  let effects = decided(held, heard)?;

  for effect in effects {
    match effect {
      Effect::Custom(FilesEffect::Replace(row)) => {
        let Ok(row_2) = console_core_number_conversion::fitted(row);

        showing.replace(row_2)
      }
      Effect::Custom(FilesEffect::ForgetTyping) => showing.forget_typing(),
      Effect::Custom(FilesEffect::WantingPictures(here)) => wanting_pictures(showing, &here)?,

      Effect::Run(_)
      | Effect::Stream(_)
      | Effect::Fork(_)
      | Effect::Interrupt(_)
      | Effect::Prompt(_)
      | Effect::Spawn(_)
      | Effect::Subscribe(_)
      | Effect::Unsubscribe(_)
      | Effect::Write(_)
      | Effect::Notify(_)
      | Effect::Print(_)
      | Effect::Stop(_) => {}
    }
  }

  Ok(())
}

fn quietly(held: &ActorAddress, heard: FilesEvent) -> Result<(), Never> {
  let _ = decided(held, heard)?;

  Ok(())
}

fn here_of(held: &ActorAddress, tab: u32) -> Result<PathBuf, Never> {
  let standing = standing_of(held)?;

  standing::here(&standing, tab)
}

fn called(held: &ActorAddress, tab: u32) -> Result<String, Never> {
  let standing = standing_of(held)?;

  standing::called(&standing, tab)
}

fn typed_in(held: &ActorAddress, tab: u32) -> Result<String, Never> {
  let standing = standing_of(held)?;

  standing::search_text(&standing, tab)
}

fn stand_where_asked(held: &ActorAddress, tab: u32, showing: &dyn Showing) -> Result<(), Never> {
  let here = here_of(held, tab)?;
  let things = read(&here)?;
  let names = things.into_iter().map(|thing| thing.name).collect();

  press(held, FilesEvent::Arrived { tab, names }, showing)
}

fn look(
  held: &ActorAddress,
  tab: u32,
  onto: Destination,
  showing: &dyn Showing,
  row: Line,
) -> Result<(), Never> {
  press(
    held,
    FilesEvent::Opened {
      tab,
      onto,
      row: row.0,
    },
    showing,
  )
}

fn kinds() -> Result<Table, Never> {
  Ok(match Table::here() {
    Ok(kinds) => kinds,
    Err(why) => {
      console_core_logging::warn!("nothing says what a file is: {why}");

      Table::default()
    }
  })
}

fn read(path: &Path) -> Result<Vec<Entry>, Never> {
  let children = match std::fs::read_dir(path) {
    Ok(children) => children,
    Err(_nothing_to_walk) => return Ok(Vec::new()),
  };

  let Ok(kinds) = kinds();

  let mut things = Vec::new();

  for about in children.flatten() {
    let at = about.path();
    let name = match about.file_name().into_string() {
      Ok(name) => name,
      Err(_the_name_is_not_text) => {
        console_core_logging::warn!("{} cannot be shown as a file name", at.display());
        continue;
      }
    };
    let wanted = listing::visibility(&name)?;

    match wanted {
      Shown::No => continue,
      Shown::Yes => {}
    }

    let Ok(kind) = console_content_types::of(&kinds, &at);

    let (folder, size) = match at.metadata() {
      Ok(held) => (held.is_dir(), held.len()),
      Err(_nothing_says) => (false, 0),
    };

    things.push(Entry {
      folder,
      kind: match ::core::primitive::bool::then_some(folder, ()) {
        ::core::option::Option::Some(()) => NOTHING_SAYS_WHAT_IT_IS.to_string(),
        ::core::option::Option::None => kind,
      },
      size,
      name,
    });
  }

  listing::sorted(things)
}

fn home() -> Result<Vec<Place>, Never> {
  let each = [
    Folder::Documents,
    Folder::Downloads,
    Folder::Music,
    Folder::Pictures,
    Folder::Videos,
  ];

  let Ok(hers) = console_core_places::home();

  let home = match hers {
    Some(home) => home,
    None => return Ok(Vec::new()),
  };

  let (home_title, rest) = match WANTED.split_first() {
    Some((home_title, rest)) => (home_title, rest),
    None => return Ok(Vec::new()),
  };

  let mut said: Vec<(&str, Option<PathBuf>)> = vec![(*home_title, Some(home.clone()))];

  said.extend(rest.iter().copied().zip(each.map(|folder| {
    let Ok(at) = folder.under(&home);

    Some(at)
  })));

  let wanted = places::wanted_at(&home, &said)?;

  places::existing(wanted, |path| match path.metadata() {
    Ok(metadata) => metadata.is_dir(),
    Err(_unavailable) => false,
  })
}

fn cache_store() -> Result<PathBuf, Never> {
  let Ok(cache) = console_core_places::Base::Cache.user();

  match cache {
    Some(cache_2) => thumbnails::store(&cache_2),
    None => Ok(PathBuf::new()),
  }
}

fn plugged_in() -> Result<Vec<Place>, Never> {
  let Ok(held) = console_core_atomic_writes::read(Path::new(places::MOUNTS));

  let said = match held {
    console_core_atomic_writes::Stored::Text(said) => said,

    console_core_atomic_writes::Stored::Absent => String::new(),

    console_core_atomic_writes::Stored::Failed(fault) => {
      console_core_logging::warn!("{}: what is plugged in: {fault}", places::MOUNTS);

      String::new()
    }
  };

  places::plugged_in(&said)
}

fn wanting_pictures(showing: &dyn Showing, here: &Path) -> Result<(), Never> {
  let said = match paths::text(here) {
    Ok(said) => said,
    Err(_unavailable) => return Ok(()),
  };
  let Ok(making) = Command::internal(InternalProgram::FilesThumbnails, &[said]);
  let Ok(task) = Task::of(making);

  showing.run(task);

  Ok(())
}

fn unzipping(path: &str) -> Result<Command, Never> {
  Command::internal(InternalProgram::FilesUnzip, &[path])
}

fn converting(here: &str) -> Result<Command, Never> {
  Command::internal(InternalProgram::DownloadsFormat, &[here])
}

fn made_the_wallpaper(path: &str) -> Result<Command, Never> {
  Command::internal(
    InternalProgram::WallpaperRender,
    &[
      console_core_internal_programs::WALLPAPER_TAKE.spelling,
      path,
    ],
  )
}

fn in_progress_as(thing: &Entry, path: &str) -> Result<Option<(Command, String)>, Never> {
  let packed = thing.an_archive()?;
  let still = thing.a_picture()?;

  Ok(match (packed, still) {
    (Packed::AnArchive, Still::APicture | Still::NotOne) => {
      let Ok(unpacking) = unzipping(path);

      Some((unpacking, format!("{UNZIPPING} {}", thing.name)))
    }
    (Packed::NotOne, Still::APicture) => {
      let Ok(setting) = made_the_wallpaper(path);

      Some((setting, format!("{SETTING} {} {AS_WALLPAPER}", thing.name)))
    }
    (Packed::NotOne, Still::NotOne) => None,
  })
}

fn lit_while(row: Row, running: Option<(Command, String)>) -> Result<Row, Never> {
  match running {
    Some((command, says)) => row.while_running(&command, &says),
    None => Ok(row),
  }
}

fn folder_rows(held: &ActorAddress, tab: u32, here: &Path) -> Result<Vec<Row>, Never> {
  let word = typed_in(held, tab)?;

  match ::core::primitive::bool::then_some(word.trim().is_empty(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return found_rows(held, tab, here, &word),
  }

  let mut rows = Vec::new();
  let held_now = standing_of(held)?;
  let above = standing::above(&held_now, tab)?;
  let called = standing::called(&held_now, tab)?;
  let holding = held_now.holding.clone();

  match above {
    Some(above_2) => {
      let leaving = held.clone();

      let Ok(back) = Row::back(&above_2, move |showing| {
        let Ok(()) = went_up(&leaving, tab, showing);
      });

      let row = here_too(held, tab, back)?;
      let Ok(naming) = Row::naming(&called, Aside(""));

      rows.push(row);
      rows.push(naming);
    }
    None => {}
  }

  match holding {
    Some(holding_2) => {
      let put_down = put_down_row(held, &holding_2, here)?;
      let row = here_too(held, tab, put_down)?;

      rows.push(row);
    }
    None => {}
  }

  let things = read(here)?;
  let room = listing::wants_room(&things)?;
  let Ok(store) = cache_store();

  for thing in things {
    let Ok(many) = console_core_number_conversion::fitted::<_, u32>(rows.len());
    let at = many.saturating_add(LINE);
    let picture = picture(&store, here, &thing, room)?;
    let row = thing_row(held, tab, here, &thing, Line(at), &picture)?;

    rows.push(row);
  }

  match ::core::primitive::bool::then_some(
    rows.iter().any(|row| {
      let Ok(heading) = row.heading();

      heading == Heading::No
    }),
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      let row = new_folder_row(held, tab, here, Line(LINE))?;

      rows.push(row);
    }
  }

  with_the_room(rows, room)
}

const ABOUT: &str = "Search this folder";

fn with_the_room(rows: Vec<Row>, room: Room) -> Result<Vec<Row>, Never> {
  match room {
    Room::Spared => return Ok(rows),
    Room::Retained => {}
  }

  let mut kept: Vec<Row> = Vec::new();

  for row in rows {
    let row_2 = with_room(row)?;

    kept.push(row_2);
  }

  Ok(kept)
}

fn found_rows(held: &ActorAddress, tab: u32, here: &Path, word: &str) -> Result<Vec<Row>, Never> {
  let folder = called(held, tab)?;
  let leaving = held.clone();

  let Ok(way_back) = Row::back(&folder, move |showing| {
    let Ok(()) = stopped_looking(&leaving, tab, showing);
  });
  let mut rows = vec![way_back];

  let found = looking::under(here, word, &read)?;

  match ::core::primitive::bool::then_some(found.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(row) = Row::placeholder("No Results");

      rows.push(row);

      return Ok(rows);
    }
    ::core::option::Option::None => {}
  }

  let Ok(store) = cache_store();
  let things: Vec<Entry> = found.iter().map(|one| one.thing.clone()).collect();
  let room = listing::wants_room(&things)?;

  for one in found {
    let at = match one.at(here) {
      Ok(at) => at,
      Err(InvalidName) => continue,
    };
    let holding = match at.parent() {
      Some(holding) => holding.to_path_buf(),
      None => here.to_path_buf(),
    };
    let picture = picture(&store, &holding, &one.thing, room)?;
    let row = found_row(held, tab, &one, &at, &picture)?;

    rows.push(row);
  }

  with_the_room(rows, room)
}

fn found_row(
  held: &ActorAddress,
  tab: u32,
  one: &Found,
  path: &Path,
  picture: &Picture,
) -> Result<Row, Never> {
  let aside = one.aside()?;

  let row = match ::core::primitive::bool::then_some(one.thing.folder, ()) {
    ::core::option::Option::Some(()) => {
      let held_2 = held.clone();
      let steps = match one.steps() {
        Ok(steps) => steps,
        Err(InvalidName) => return Row::placeholder("This folder name cannot be opened"),
      };

      let Ok(walks) = Handler::and_stay(move |showing| {
        showing.forget_typing();

        let steps_2 = steps.clone();

        let Ok(()) = press(
          &held_2,
          FilesEvent::Walked {
            tab,
            steps: steps_2,
          },
          showing,
        );
      });
      let Ok(row) = Row::new(&one.thing.name, Aside(&aside), walks);
      let Ok(opens) = row.opening();

      opens
    }
    ::core::option::Option::None => {
      let said = match paths::text(path) {
        Ok(said) => said,
        Err(_unavailable) => return Row::placeholder("This file name cannot be opened"),
      };
      let Ok(opens) = Program::XdgOpen.name();
      let Ok(runs) = Handler::run(&[opens, said]);
      let Ok(row) = Row::new(&one.thing.name, Aside(&aside), runs);

      row
    }
  };

  row.picturing(picture.clone())
}

fn stopped_looking(held: &ActorAddress, tab: u32, showing: &dyn Showing) -> Result<(), Never> {
  press(held, FilesEvent::Back { tab }, showing)
}

fn picture(store: &Path, here: &Path, thing: &Entry, room: Room) -> Result<Picture, Never> {
  match room {
    Room::Spared => return Ok(Picture::None),
    Room::Retained => {}
  }

  match ::core::primitive::bool::then_some(thing.folder, ()) {
    ::core::option::Option::Some(()) => return Ok(Picture::Named(Icon::Folder)),
    ::core::option::Option::None => {}
  }

  let worth = thing.worth_a_picture()?;

  let found = match worth {
    Worth::APicture => {
      let name = match ChildName::new(&thing.name) {
        Ok(name) => name,
        Err(InvalidName) => return Ok(Picture::Space),
      };

      thumbnails::find_thumbnail(store, &here.join(name))?
    }
    Worth::ItsNameAlone => None,
  };

  Ok(match found {
    None => Picture::Space,
    Some(at) => Picture::At(at),
  })
}

fn with_room(row: Row) -> Result<Row, Never> {
  Ok(match row.picture {
    Picture::None => {
      let Ok(spared) = row.picturing(Picture::Space);

      spared
    }
    Picture::Space
    | Picture::Named(_)
    | Picture::At(_)
    | Picture::Sleeve(_)
    | Picture::Showing(_)
    | Picture::Playing(_)
    | Picture::Written(_)
    | Picture::Bar(_) => row,
  })
}

fn thing_row(
  held: &ActorAddress,
  tab: u32,
  here: &Path,
  thing: &Entry,
  at: Line,
  picture: &Picture,
) -> Result<Row, Never> {
  let aside = listing::aside(thing)?;
  let child = match ChildName::new(&thing.name) {
    Ok(child) => child,
    Err(InvalidName) => return Row::placeholder("Invalid file name"),
  };
  let path = here.join(child);
  let key = match paths::text(&path) {
    Ok(key) => key,
    Err(_unavailable) => return Row::placeholder("This path cannot be passed to a file action"),
  };

  let row = match ::core::primitive::bool::then_some(thing.folder, ()) {
    ::core::option::Option::Some(()) => {
      let held_2 = held.clone();
      let name = thing.name.clone();

      let Ok(enters) = Handler::and_stay(move |showing| {
        let name_2 = name.clone();

        let Ok(()) = press(
          &held_2,
          FilesEvent::Entered {
            tab,
            name: name_2,
            at: at.0,
          },
          showing,
        );
      });
      let Ok(row) = Row::new(&thing.name, Aside(&aside), enters);
      let Ok(opens) = row.opening();

      opens
    }
    ::core::option::Option::None => {
      let Ok(opens) = Program::XdgOpen.name();
      let Ok(runs) = Handler::run(&[opens, key]);
      let Ok(row) = Row::new(&thing.name, Aside(&aside), runs);
      let Ok(running) = in_progress_as(thing, key);
      let Ok(lit) = lit_while(row, running);

      lit
    }
  };

  let held_3 = held.clone();
  let thing_2 = thing.clone();
  let Ok(pictured) = row.picturing(picture.clone());
  let Ok(pictured_2) = pictured.selectable(key);

  pictured_2.offering(move |showing| {
    let onto = Destination::Ways {
      thing: thing_2.clone(),
      from: at.0,
    };

    let Ok(()) = look(&held_3, tab, onto, showing, Line(WAYS_START));

    ActionOutcome::Stay
  })
}

fn paste_command<'a>(holding: &'a Holding, here: &'a Path) -> Result<Command, NonTextPath<'a>> {
  let (program, mut arguments) = match holding.moving {
    Carrying::ToMove => (Program::Mv, vec!["--".to_string()]),
    Carrying::ToCopy => (Program::Cp, vec!["-r".to_string(), "--".to_string()]),
  };

  for path in &holding.paths {
    let native = paths::text(path)?;
    arguments.push(native.to_owned());
  }

  let into = paths::text(here)?;
  arguments.push(into.to_owned());

  Ok(Command {
    program: console_program_contract::Executable::External(program),
    arguments,
  })
}

fn put_down_row(held: &ActorAddress, holding: &Holding, here: &Path) -> Result<Row, Never> {
  let putting = match paste_command(holding, here) {
    Ok(putting) => putting,
    Err(_unavailable) => return Row::placeholder("These paths cannot be pasted"),
  };
  let held_2 = held.clone();
  let says = holding.says()?;

  let Ok(puts) = Handler::and_stay(move |showing| {
    let Ok(()) = press(&held_2, FilesEvent::Paste, showing);
    let Ok(task) = Task::of(putting.clone());

    showing.run(task);
  });

  Row::new(&says, Aside(""), puts)
}

fn ask_for_a_folder(
  held: &ActorAddress,
  tab: u32,
  here: &Path,
  from: Line,
  showing: &dyn Showing,
) -> Result<(), Never> {
  let here_2 = here.to_path_buf();
  let held_2 = held.clone();

  let answer = answer(move |showing_2, word| {
    let Ok(name) = console_core_file_names::a_name(word);

    let name_3 = match name {
      Some(name_2) => name_2,
      None => return,
    };

    let child = match ChildName::new(&name_3) {
      Ok(child) => child,
      Err(InvalidName) => return,
    };
    let at = here_2.join(child);
    let made = match paths::text(&at) {
      Ok(made) => made,
      Err(fault) => {
        showing_2.note(&fault.to_string());
        return;
      }
    };
    let Ok(()) = back_to_the_folder(&held_2, tab, showing_2, from);
    let Ok(making) = Command::external(Program::Mkdir, &["--", made]);
    let Ok(task) = Task::of(making);

    showing_2.run(task);
  })?;

  showing.ask_aloud(NEW_FOLDER, Initial(""), answer);

  Ok(())
}

const NEW_FOLDER: &str = "New Folder";

fn new_folder_row(held: &ActorAddress, tab: u32, here: &Path, from: Line) -> Result<Row, Never> {
  let here_2 = here.to_path_buf();
  let held_2 = held.clone();

  let Ok(asks) = Handler::and_stay(move |showing| {
    let Ok(()) = ask_for_a_folder(&held_2, tab, &here_2, from, showing);
  });

  Row::new(NEW_FOLDER, Aside(""), asks)
}

fn here_too(held: &ActorAddress, tab: u32, row: Row) -> Result<Row, Never> {
  let held_2 = held.clone();

  row.offering(move |showing| {
    let Ok(()) = look(
      &held_2,
      tab,
      Destination::Here { from: LINE },
      showing,
      Line(HERE_START),
    );

    ActionOutcome::Stay
  })
}

fn here_rows(held: &ActorAddress, tab: u32, here: &Path, from: Line) -> Result<Vec<Row>, Never> {
  let folder = called(held, tab)?;
  let leaving = held.clone();
  let new_folder = new_folder_row(held, tab, here, from)?;
  let downloads_format = one_format_row(held, tab, here, from)?;

  let Ok(way_back) = Row::back(&folder, move |showing| {
    let Ok(()) = back_to_the_folder(&leaving, tab, showing, from);
  });

  Ok(vec![way_back, new_folder, downloads_format])
}

const ONE_FORMAT: &str = "Convert All Media";
const ONE_FORMAT_RUNNING: &str = "Converting All Media";
const UNZIPPING: &str = "Unzipping";
const SETTING: &str = "Setting";
const AS_WALLPAPER: &str = "as Wallpaper";
const ONE_FORMAT_ASKS: &str = "Convert everything here? Songs become Opus, videos MKV.";
const ONE_FORMAT_DOES: &str = "Convert";

fn one_format_row(held: &ActorAddress, tab: u32, here: &Path, from: Line) -> Result<Row, Never> {
  let path = match paths::text(here) {
    Ok(path) => path,
    Err(_unavailable) => return Row::placeholder("This path cannot be converted"),
  };
  let Ok(changing) = converting(path);
  let waiting = changing.clone();
  let Ok(folder) = paths::display_name(here);
  let held_2 = held.clone();

  let Ok(asks) = Handler::and_stay(move |showing| {
    let held_3 = held_2.clone();
    let named = folder.clone();
    let changing_now = waiting.clone();

    let Ok(then) = on_chosen(move |showing_2, _| {
      let Ok(()) = back_to_the_folder(&held_3, tab, showing_2, from);

      showing_2.note(&format!("Converting {named}…"));

      let Ok(task) = Task::of(changing_now);
      showing_2.run(task);
    });

    showing.sure(ONE_FORMAT_ASKS, Subject(&folder), &[ONE_FORMAT_DOES], then);
  });
  let Ok(row) = Row::new(ONE_FORMAT, Aside(""), asks);

  row.while_running(&changing, ONE_FORMAT_RUNNING)
}

fn way_rows(
  held: &ActorAddress,
  tab: u32,
  thing: &Entry,
  from: Line,
  here: &Path,
) -> Result<Vec<Row>, Never> {
  let folder = called(held, tab)?;
  let leaving = held.clone();
  let aside = listing::aside(thing)?;

  let Ok(way_back) = Row::back(&folder, move |showing| {
    let Ok(()) = back_to_the_folder(&leaving, tab, showing, from);
  });
  let Ok(naming) = Row::naming(&thing.name, Aside(&aside));
  let mut rows = vec![way_back, naming];

  let name = match ChildName::new(&thing.name) {
    Ok(name) => name,
    Err(InvalidName) => return Ok(rows),
  };
  let path = here.join(name);
  let ways = doing::ways(thing)?;
  let editors = match ::core::primitive::bool::then_some(thing.folder, ()) {
    ::core::option::Option::Some(()) => Vec::new(),
    ::core::option::Option::None => editors::choices(&thing.kind)?,
  };
  let editor_kind = editors::Kind::of(&thing.kind)?;
  let preferred = match editor_kind {
    Some(kind) => kind.preferred(&thing.kind, &editors)?,
    None => None,
  };

  for deed in ways {
    let row = match (deed, editors.as_slice()) {
      (FileAction::Edit, _) => {
        let editor = match preferred {
          Some(matched_editor) => matched_editor,
          ::core::option::Option::None => continue,
        };

        let says = deed.says()?;
        let launch = launch_with(editor, &path)?;

        let edit = Row::new(says, Aside(&editor.says), launch)?;

        edit
      }
      (FileAction::EditWith, []) => continue,
      _ => deed_row(held, tab, thing, from, &path, deed)?,
    };

    rows.push(row);
  }

  let new_folder = new_folder_row(held, tab, here, from)?;
  let downloads_format = one_format_row(held, tab, here, from)?;

  let Ok(naming_2) = Row::naming(&format!("In {folder}"), Aside(""));

  rows.push(naming_2);
  rows.push(new_folder);
  rows.push(downloads_format);

  Ok(rows)
}

fn program_rows(
  held: &ActorAddress,
  tab: u32,
  thing: &Entry,
  from: Line,
  here: &Path,
  use_: ProgramUse,
) -> Result<Vec<Row>, Never> {
  let name = match ChildName::new(&thing.name) {
    Ok(name) => name,
    Err(InvalidName) => return Ok(Vec::new()),
  };
  let path = here.join(name);
  let leaving = held.clone();
  let going_back = thing.clone();
  let deed = match use_ {
    ProgramUse::Open => FileAction::OpenWith,
    ProgramUse::Edit => FileAction::EditWith,
  };
  let opens = deed.says()?;

  let Ok(way_back) = Row::back(&thing.name, move |showing| {
    let onto = Destination::Ways {
      thing: going_back.clone(),
      from: from.0,
    };

    let Ok(()) = look(&leaving, tab, onto, showing, Line(WAYS_START));
  });
  let Ok(naming) = Row::naming(opens, Aside(""));
  let mut rows = vec![way_back, naming];

  let found = match use_ {
    ProgramUse::Open => crate::open_with::programs(&thing.kind)?,
    ProgramUse::Edit => editors::choices(&thing.kind)?,
  };

  match ::core::primitive::bool::then_some(found.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(row) = Row::placeholder(match use_ {
        ProgramUse::Open => "No App Opens This",
        ProgramUse::Edit => "No editor installed for this file",
      });

      rows.push(row);

      return Ok(rows);
    }
    ::core::option::Option::None => {}
  }

  for app in found {
    let starts = launch_with(&app, &path)?;
    let row = Row::new(&app.says, Aside(""), starts)?;

    rows.push(row);
  }

  Ok(rows)
}

fn launch_with(app: &Application, path: &Path) -> Result<Handler, Never> {
  let desktop = app.path.clone();
  let path_2 = path.to_path_buf();

  Handler::call(move |_| {
    let Ok(()) = crate::open_with::started(&desktop, &path_2);

    ActionOutcome::Close
  })
}

fn deed_row(
  held: &ActorAddress,
  tab: u32,
  thing: &Entry,
  from: Line,
  path: &Path,
  deed: FileAction,
) -> Result<Row, Never> {
  let says = deed.says()?;

  match deed {
    FileAction::Open => {
      let target = match paths::text(path) {
        Ok(target) => target,
        Err(_unavailable) => return Row::placeholder("This file name cannot be opened"),
      };
      let Ok(opens) = Program::XdgOpen.name();
      let Ok(runs) = Handler::run(&[opens, target]);

      return Row::new(says, Aside(""), runs);
    }
    FileAction::Copy
    | FileAction::Delete
    | FileAction::Edit
    | FileAction::EditWith
    | FileAction::Move
    | FileAction::OpenWith
    | FileAction::Rename
    | FileAction::Select
    | FileAction::Unzip
    | FileAction::Wallpaper => {}
  }

  let held_2 = held.clone();
  let thing_2 = thing.clone();
  let path_2 = path.to_path_buf();

  let Ok(does) = Handler::and_stay(move |showing| {
    let Ok(()) = perform(&held_2, tab, &thing_2, from, &path_2, deed, showing);
  });
  let Ok(row) = Row::new(says, Aside(""), does);

  Ok(match deed {
    FileAction::OpenWith | FileAction::EditWith => {
      let Ok(opens) = row.opening();

      opens
    }
    FileAction::Copy
    | FileAction::Delete
    | FileAction::Edit
    | FileAction::Move
    | FileAction::Open
    | FileAction::Rename
    | FileAction::Select
    | FileAction::Unzip
    | FileAction::Wallpaper => row,
  })
}

fn perform(
  held: &ActorAddress,
  tab: u32,
  thing: &Entry,
  from: Line,
  path: &Path,
  deed: FileAction,
  showing: &dyn Showing,
) -> Result<(), Never> {
  let target = match paths::text(path) {
    Ok(target) => target,
    Err(unavailable) => {
      showing.note(&unavailable.to_string());
      return Ok(());
    }
  };

  match deed {
    FileAction::Open | FileAction::Edit => (),
    FileAction::OpenWith | FileAction::EditWith => {
      let use_ = match ::core::primitive::bool::then_some(deed == FileAction::EditWith, ()) {
        ::core::option::Option::Some(()) => ProgramUse::Edit,
        ::core::option::Option::None => ProgramUse::Open,
      };
      let onto = Destination::Programs {
        thing: thing.clone(),
        from: from.0,
        use_,
      };

      look(held, tab, onto, showing, Line(WAYS_START))?;
    }
    FileAction::Delete => {
      let held_2 = held.clone();
      let at = target.to_owned();
      let says = deed.says()?;

      let then = on_chosen(move |showing_2, _| {
        let Ok(()) = back_to_the_folder(&held_2, tab, showing_2, from);

        let Ok(trashing) = console_panel::page::trashing(&[at]);
        let Ok(task) = Task::of(trashing);

        showing_2.run(task);
      })?;

      showing.sure(doing::SURE, Subject(&thing.name), &[says], then);
    }
    FileAction::Copy | FileAction::Move => {
      let carrying = match ::core::primitive::bool::then_some(deed == FileAction::Move, ()) {
        ::core::option::Option::Some(()) => Carrying::ToMove,
        ::core::option::Option::None => Carrying::ToCopy,
      };
      let holding = Holding::of(thing, path.to_path_buf(), carrying)?;

      quietly(held, FilesEvent::Copy(holding))?;
      back_to_the_folder(held, tab, showing, from)?;
    }
    FileAction::Select => {
      back_to_the_folder(held, tab, showing, from)?;

      showing.select(vec![target.to_owned()]);
    }
    FileAction::Unzip => {
      back_to_the_folder(held, tab, showing, from)?;

      showing.note(&format!("{UNZIPPING} {}…", thing.name));

      let unpacking = unzipping(target)?;
      let task = Task::of(unpacking)?;

      showing.run(task);
    }
    FileAction::Wallpaper => {
      back_to_the_folder(held, tab, showing, from)?;

      showing.note(&format!("{SETTING} {} {AS_WALLPAPER}…", thing.name));

      let setting = made_the_wallpaper(target)?;
      let task = Task::of(setting)?;

      showing.run(task);
    }
    FileAction::Rename => {
      let held_3 = held.clone();
      let path_3 = path.to_path_buf();
      let was = target.to_owned();

      let answer = answer(move |showing_3, word| {
        let Ok(name) = console_core_file_names::a_name(word);

        let name_3 = match name {
          Some(name_2) => name_2,
          None => return,
        };

        let child = match ChildName::new(&name_3) {
          Ok(child) => child,
          Err(InvalidName) => return,
        };
        let beside = path_3.with_file_name(child.as_ref().as_os_str());
        let now = match paths::text(&beside) {
          Ok(now) => now,
          Err(fault) => {
            showing_3.note(&fault.to_string());
            return;
          }
        };
        let Ok(()) = back_to_the_folder(&held_3, tab, showing_3, from);
        let Ok(renaming) = Command::external(Program::Mv, &["--", &was, now]);
        let Ok(task) = Task::of(renaming);

        showing_3.run(task);
      })?;

      showing.ask_aloud(&format!("Rename {}", thing.name), Initial(""), answer);
    }
  }

  Ok(())
}

fn back_to_the_folder(
  held: &ActorAddress,
  tab: u32,
  showing: &dyn Showing,
  from: Line,
) -> Result<(), Never> {
  look(held, tab, Destination::Folder, showing, from)
}

fn went_up(held: &ActorAddress, tab: u32, showing: &dyn Showing) -> Result<(), Never> {
  press(held, FilesEvent::Up { tab }, showing)
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the panel owns the Answer callback and its API requires a shareable handler for redrawing the prompt"
  )
)]
fn answer(then: impl Fn(&dyn Showing, &str) + Send + Sync + 'static) -> Result<Answer, Never> {
  Ok(Arc::new(then))
}

fn on_chosen(
  then: impl FnOnce(&dyn Showing, u32) + Send + Sync + 'static,
) -> Result<Confirmation, Never> {
  Ok(Box::new(then))
}

fn rows(held: &ActorAddress, tab: u32) -> Result<Vec<Row>, Never> {
  let held_now = standing_of(held)?;
  let here = standing::here(&held_now, tab)?;
  let onto = standing::onto(&held_now, tab)?;

  match onto {
    Destination::Folder => folder_rows(held, tab, &here),
    Destination::Here { from } => here_rows(held, tab, &here, Line(from)),
    Destination::Programs { thing, from, use_ } => {
      program_rows(held, tab, &thing, Line(from), &here, use_)
    }
    Destination::Ways { thing, from } => way_rows(held, tab, &thing, Line(from), &here),
  }
}

fn pages(held: &ActorAddress) -> Result<Vec<Page>, Never> {
  let standing = standing_of(held)?;
  let titles = standing::titles(&standing)?;
  let mut pages: Vec<Page> = Vec::new();

  for (tab, title) in titles.iter().enumerate() {
    let Ok(tab_2) = console_core_number_conversion::fitted::<_, u32>(tab);
    let page = page(held, tab_2, title)?;

    pages.push(page);
  }

  Ok(pages)
}

fn carried_many(
  page: Page,
  held: &ActorAddress,
  deed: FileAction,
  moving: Carrying,
) -> Result<Page, Never> {
  let held_2 = held.clone();
  let Ok(says) = deed.says();

  page.selecting(says, move |_, keys| {
    let paths = keys.iter().map(PathBuf::from).collect();
    let Ok(holding) = Holding::many(paths, moving);
    let Ok(()) = quietly(&held_2, FilesEvent::Copy(holding));
  })
}

fn page(held: &ActorAddress, tab: u32, title: &str) -> Result<Page, Never> {
  let reading = held.clone();
  let backing = held.clone();
  let arriving = held.clone();

  let Ok(asking) = Rows::computed(move || {
    let Ok(rows) = rows(&reading, tab);

    rows
  });

  let Ok(page) = Page::new(title, asking);
  let Ok(page_2) = page.on_arriving(move |showing| {
    let Ok(here) = here_of(&arriving, tab);

    let Ok(()) = wanting_pictures(showing, &here);

    let Ok(()) = stand_where_asked(&arriving, tab, showing);
  });
  let Ok(page_3) = carried_many(page_2, held, FileAction::Copy, Carrying::ToCopy);
  let Ok(page_4) = carried_many(page_3, held, FileAction::Move, Carrying::ToMove);
  let Ok(page_5) = page_4.trashing_selected();
  let Ok(page_6) = page_5.on_back(move |showing| {
    let Ok(was) = standing_of(&backing);

    let Ok(()) = press(&backing, FilesEvent::Back { tab }, showing);

    let Ok(closes) = closes(&was, tab);

    match closes {
      Closes::Yes => ActionOutcome::Close,
      Closes::No => ActionOutcome::Stay,
    }
  });

  let standing = standing_of(held)?;
  let onto = standing::onto(&standing, tab)?;

  match onto {
    Destination::Folder => {}
    Destination::Here { .. } | Destination::Programs { .. } | Destination::Ways { .. } => {
      return Ok(page_6);
    }
  }

  let typing = held.clone();

  page_6.searching(ABOUT, move |showing, word| {
    let heard = FilesEvent::Typed {
      tab,
      word: word.to_string(),
    };

    let Ok(()) = press(&typing, heard, showing);
  })
}

fn went_to(standing: &mut Standing, said: &str) -> Result<Option<String>, Never> {
  let path = Path::new(said);

  let metadata = match path.metadata() {
    Ok(metadata) => metadata,
    Err(_unavailable) => return Ok(None),
  };
  let is = match metadata.is_dir().then_some(()) {
    Some(()) => places::Is::AFolder,
    None => places::Is::AFile,
  };

  let leading = places::leading_to(&standing.places, path, is)?;

  let leading_3 = match leading {
    Some(leading_2) => leading_2,
    None => return Ok(None),
  };

  for step in &leading_3.steps {
    let onto = first_thing(standing, leading_3.place)?;

    let Ok(slot) = console_core_number_conversion::index(leading_3.place);

    let walk = match standing.walks.get_mut(slot) {
      Some(walk) => walk,
      None => return Ok(None),
    };

    walk.enter(step, onto)?;
  }

  standing.stand_on = leading_3.stand_on.map(|name| (leading_3.place, name));

  let Ok(slot) = console_core_number_conversion::index(leading_3.place);

  let place = match standing.places.get(slot) {
    Some(place) => place,
    None => return Ok(None),
  };

  Ok(Some(place.title.clone()))
}

pub const WHO: &str = "files";

pub fn card(arguments: &[String]) -> Result<Card, Never> {
  let asked = arguments.first().cloned();
  let Ok(mut places) = home();
  let Ok(plugged) = plugged_in();

  places.extend(plugged);

  let Ok(mut opening) = Standing::of(places);

  let opened_at = match asked.as_deref() {
    Some(said) => {
      let Ok(opened) = went_to(&mut opening, said);

      opened
    }
    None => None,
  };
  let Ok(card) = Card::supervised::<Files, _>(opening, pages);

  card.opening_at(opened_at.as_deref().or(asked.as_deref()))
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_program_contract::Executable;
  use std::cell::RefCell;
  use std::error::Error;
  use std::ffi::OsStr;
  use std::os::unix::ffi::OsStrExt;
  use std::os::unix::fs::symlink;

  #[derive(Default)]
  struct Tasks {
    pending: RefCell<Option<Confirmation>>,
    commands: RefCell<Vec<Command>>,
  }

  impl Showing for Tasks {
    fn refresh(&self) {}

    fn replace(&self, _standing_on: u32) {}

    fn forget_typing(&self) {}

    fn ask(&self, _question: &str, _then: Answer) {}

    fn ask_aloud(&self, _question: &str, _initial: Initial<'_>, _then: Answer) {}

    fn open_out(&self) {}

    fn toggle_full_screen(&self) {}

    fn zoom_by(&self, _factor: f64) {}

    fn select(&self, _keys: Vec<String>) {}

    fn turn_to(&self, _tab: u32) {}

    fn note(&self, _said: &str) {}

    fn interrupt(&self, _command: &Command) {}

    fn leave_running(&self, _arguments: Vec<String>) {}

    fn sure(&self, _question: &str, _about: Subject<'_>, _does: &[&str], then: Confirmation) {
      match self.pending.try_borrow_mut() {
        Ok(mut pending) => *pending = Some(then),
        Err(fault) => console_core_logging::warn!("capturing the confirmation: {fault}"),
      }
    }

    fn run(&self, task: Task) {
      match self.commands.try_borrow_mut() {
        Ok(mut commands) => commands.push(task.command),
        Err(fault) => console_core_logging::warn!("capturing the command: {fault}"),
      }
    }
  }

  fn confirmation(jobs: &Tasks) -> Result<Confirmation, Box<dyn Error>> {
    let mut pending = jobs.pending.try_borrow_mut()?;
    let answer = pending.take().ok_or("one confirmation")?;

    Ok(answer)
  }

  #[test]
  fn cancelling_conversion_drops_the_command_until_a_new_confirmation()
  -> Result<(), Box<dyn Error>> {
    let folder = Path::new("/media/fixture files");
    let Ok(place) = Place::new("Fixture", folder.to_path_buf());
    let Ok(standing) = Standing::of(vec![place]);
    let console_actor::Booted { actor, scope, .. } = console_actor::boot::<Files>(standing)?;
    let Ok(row) = one_format_row(&actor, 0, folder, Line(LINE));
    let operation = row
      .while_running
      .ok_or("the row should follow the conversion")?;
    let activate = match row.does {
      Some(Handler::Call(activate)) => activate,
      Some(Handler::Run(_)) | None => {
        return Err(Box::<dyn Error>::from("the row must ask before starting"));
      }
    };
    let jobs = Tasks::default();

    assert_eq!(activate(&jobs), ActionOutcome::Stay);
    let cancelled = confirmation(&jobs)?;
    drop(cancelled);
    let awaiting = jobs.commands.try_borrow()?;
    assert!(awaiting.is_empty());
    drop(awaiting);

    assert_eq!(activate(&jobs), ActionOutcome::Stay);
    let accepted = confirmation(&jobs)?;
    accepted(&jobs, 0);
    let queued = jobs.commands.try_borrow()?;
    assert_eq!(queued.as_slice(), [operation.command]);
    let Ok(()) = scope.close();

    Ok(())
  }

  fn pasted(holding: &Holding, destination: &Path) -> Result<(), Box<dyn Error>> {
    let command = paste_command(holding, destination).map_err(|fault| fault.to_string())?;
    let program = match command.program {
      Executable::External(program) => program,
      Executable::Internal(_other) => {
        return Err(Box::<dyn Error>::from(
          "pasting should run a system file command",
        ));
      }
    };
    let Ok(mut process) = program.command();
    let status = process.args(command.arguments).status()?;
    assert!(
      status.success(),
      "the fixture file command returned {status}"
    );

    Ok(())
  }

  #[test]
  fn paste_copies_and_moves_the_link_itself() -> Result<(), Box<dyn Error>> {
    let folder = console_core_temporary_directories::private("files-paste")?;
    let copies = folder.join("copies and spaces");
    let moves = folder.join("moves");
    std::fs::create_dir(&copies)?;
    std::fs::create_dir(&moves)?;
    let target = folder.join("original.txt");
    let source = folder.join("source link");
    console_core_atomic_writes::whole(&target, b"original fixture")?;
    symlink("original.txt", &source)?;

    let Ok(copy) = Holding::many(vec![source.clone()], Carrying::ToCopy);
    pasted(&copy, &copies)?;
    let copied = copies.join("source link");
    let copied_link = std::fs::read_link(&copied)?;
    assert_eq!(copied_link, Path::new("original.txt"));

    let Ok(moving) = Holding::many(vec![copied.clone()], Carrying::ToMove);
    pasted(&moving, &moves)?;
    let moved_link = std::fs::read_link(moves.join("source link"))?;
    assert_eq!(moved_link, Path::new("original.txt"));
    assert!(
      matches!(std::fs::symlink_metadata(copied), Err(fault) if fault.kind() == std::io::ErrorKind::NotFound)
    );
    let original = std::fs::read(&target)?;
    let source_link = std::fs::read_link(source)?;
    assert_eq!(original, b"original fixture");
    assert_eq!(source_link, Path::new("original.txt"));
    std::fs::remove_dir_all(folder)?;

    Ok(())
  }

  #[test]
  fn a_path_that_is_not_text_never_becomes_a_paste_command() {
    let source = PathBuf::from(OsStr::from_bytes(b"/fixture/name\xff"));
    let Ok(holding) = Holding::many(vec![source], Carrying::ToMove);

    assert!(matches!(
      paste_command(&holding, Path::new("/fixture/into")),
      Err(_not_text)
    ));
  }
}
