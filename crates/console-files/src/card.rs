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
//! is handed to `later`, which runs it on a thread and draws the folder again
//! when it is done. They are the commands rather than the library calls for the
//! same reason the settings ask `pactl` about the volume: the panel is a thing
//! that draws, and `mv` already knows what moving across two disks means.


use console_core_external_programs::Program;
use console_core_never::Never;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use console_core_places::Folder;
use console_content_types::Table;
use console_panel::icons::Icon;
use crate::doing::{self, Carrying, FileAction, Holding};
use crate::listing::{self, Entry, Room, Shown, Worth};
use crate::looking::{self, Found};
use crate::places::{self, Place, WANTED};
use crate::thumbnails;
use crate::standing::{
    self, Closes, Files, HERE_START, FilesEvent, FilesEffect, LINE, Line, Destination, Standing, WAYS_START, closes,
    first_thing,
};
use console_panel::actor::{self, Address, Answer as Reply};
use console_program_contract::{Effect, Program as _, Update, Event};
use console_panel::page::{Answer, Aside, Handler, Heading, Page, Picture, Row, Rows, Showing, OnChosen, Subject};
use console_panel::card::Card;

const NOTHING_SAYS_WHAT_IT_IS: &str = "";


enum Message {
    Event(FilesEvent, Reply<Vec<Effect<FilesEffect>>>),
    At(Reply<Standing>),
}

struct Looking(Standing);

impl actor::Machine for Looking {
    type Message = Message;

    fn step(self, message: Message) -> Self {
        match message {
            Message::Event(heard, answer) => {
                let Update { state, effects } = Files::update(&self.0, &Event::Custom(heard));
                let _ = answer.say(effects);

                Looking(state)
            }
            Message::At(answer) => {
                let _ = answer.say(self.0.clone());

                self
            },
        }
    }
}

type ActorAddress = Address<Message>;

fn standing_of(held: &ActorAddress) -> Result<Standing, Never> {
    match held.ask(Message::At) {
        Ok(standing) => Ok(standing),
        Err(_the_actor_has_gone) => {
            eprintln!("files: the panel's own state is missing, so it drew nothing");

            Standing::of(Vec::new())
        }
    }
}

fn decided(held: &ActorAddress, heard: FilesEvent) -> Result<Vec<Effect<FilesEffect>>, Never> {
    Ok(match held.ask(|answer| Message::Event(heard, answer)) {
        Ok(effects) => effects,
        Err(_the_actor_has_gone) => {
            eprintln!("files: the panel's own state is missing, so the press did nothing");

            Vec::new()
        }
    })
}

fn press(held: &ActorAddress, heard: FilesEvent, showing: &dyn Showing) -> Result<(), Never> {
    let effects = decided(held, heard)?;

    for effect in effects {
        match effect {
            Effect::Custom(FilesEffect::Replace(row)) => {
                let Ok(row) = console_core_number_conversion::fitted(row);

                showing.replace(row)
            }
            Effect::Custom(FilesEffect::ForgetTyping) => showing.forget_typing(),
            Effect::Custom(FilesEffect::WantingPictures(here)) => wanting_pictures(showing, &here)?,

            Effect::Run(_)
            | Effect::Stream(_)
            | Effect::Prompt(_)
            | Effect::Spawn(_)
            | Effect::Subscribe(_)
            | Effect::Unsubscribe(_)
            | Effect::Write(_)
            | Effect::Notify(_)
            | Effect::Print(_)
            | Effect::Stop(_) => {},
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

    standing::typed(&standing, tab)
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
    press(held, FilesEvent::Opened { tab, onto, row: row.0 }, showing)
}

fn kinds() -> Result<Table, Never> {
    Ok(match Table::here() {
        Ok(kinds) => kinds,
        Err(why) => {
            eprintln!("files: nothing says what a file is: {why}");

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
        let name = about.file_name().to_string_lossy().to_string();
        let wanted = listing::wanted(&name)?;

        match wanted {
            Shown::No => continue,
            Shown::Yes => {},
        }

        let at = about.path();
        let Ok(kind) = console_content_types::of(&kinds, &at);

        let (folder, size) = match at.metadata() {
            Ok(held) => (held.is_dir(), held.len()),
            Err(_nothing_says) => (at.is_dir(), 0),
        };

        things.push(Entry {
            folder,
            kind: match folder {
                true => NOTHING_SAYS_WHAT_IT_IS.to_string(),
                false => kind,
            },
            size,
            name,
        });
    }

    listing::sorted(things)
}

fn kind_of(path: &Path) -> Result<Option<String>, Never> {
    let Ok(kinds) = kinds();
    let Ok(kind) = console_content_types::of(&kinds, path);

    Ok(match kind.is_empty() {
        true => None,
        false => Some(kind),
    })
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

    places::kept(wanted, |path| path.is_dir())
}

fn cache_store() -> Result<PathBuf, Never> {
    let Ok(cache) = console_core_places::Base::Cache.hers();

    match cache {
        Some(cache) => thumbnails::store(&cache),
        None => Ok(PathBuf::new()),
    }
}

fn plugged_in() -> Result<Vec<Place>, Never> {
    let Ok(held) = console_core_atomic_writes::read(Path::new(places::MOUNTS));

    let said = match held {
        console_core_atomic_writes::Stored::Text(said) => said,

        console_core_atomic_writes::Stored::Absent => String::new(),

        console_core_atomic_writes::Stored::Failed(fault) => {
            eprintln!("console-files: {}: what is plugged in: {fault}", places::MOUNTS);

            String::new()
        }
    };

    places::plugged_in(&said)
}

fn wanting_pictures(showing: &dyn Showing, here: &Path) -> Result<(), Never> {
    let said = said(here)?;

    showing.later(vec!["files-thumbnails".to_string(), said]);

    Ok(())
}

fn folder_rows(held: &ActorAddress, tab: u32, here: &Path) -> Result<Vec<Row>, Never> {
    let word = typed_in(held, tab)?;

    match word.trim().is_empty() {
        true => {},
        false => return found_rows(held, tab, here, &word),
    }

    let mut rows = Vec::new();
    let held_now = standing_of(held)?;
    let above = standing::above(&held_now, tab)?;
    let called = standing::called(&held_now, tab)?;
    let holding = held_now.holding.clone();

    match above {
        Some(above) => {
            let leaving = held.clone();

            let Ok(back) = Row::back(&above, move |showing| {
                let Ok(()) = went_up(&leaving, tab, showing);
            });

            let row = here_too(held, tab, back)?;
            let Ok(naming) = Row::naming(&called, Aside(""));

            rows.push(row);
            rows.push(naming);
        }
        None => {},
    }

    match holding {
        Some(holding) => {
            let put_down = put_down_row(held, &holding, here)?;
            let row = here_too(held, tab, put_down)?;

            rows.push(row);
        }
        None => {},
    }

    let things = read(here)?;
    let room = listing::wants_room(&things)?;
    let Ok(store) = cache_store();

    for thing in things {
        let Ok(many) = console_core_number_conversion::fitted::<_, u32>(rows.len());
        let at = many.saturating_add(LINE);
        let picture = picture(&store, here, &thing, room)?;
        let row = thing_row(held, tab, &thing, Line(at), &picture)?;

        rows.push(row);
    }

    match rows.iter().any(|row| {
        let Ok(heading) = row.heading();

        heading == Heading::No
    }) {
        true => {},
        false => {
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
        Room::Retained => {},
    }

    let mut kept: Vec<Row> = Vec::new();

    for row in rows {
        let row = with_room(row)?;

        kept.push(row);
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

    match found.is_empty() {
        true => {
            let Ok(row) = Row::nothing("No Results");

            rows.push(row);

            return Ok(rows);
        }
        false => {},
    }

    let Ok(store) = cache_store();
    let things: Vec<Entry> = found.iter().map(|one| one.thing.clone()).collect();
    let room = listing::wants_room(&things)?;

    for one in found {
        let at = one.at(here)?;
        let holding = match at.parent() {
            Some(holding) => holding.to_path_buf(),
            None => here.to_path_buf(),
        };
        let picture = picture(&store, &holding, &one.thing, room)?;
        let row = found_row(held, tab, &one, &picture)?;

        rows.push(row);
    }

    with_the_room(rows, room)
}

fn found_row(held: &ActorAddress, tab: u32, one: &Found, picture: &Picture) -> Result<Row, Never> {
    let aside = one.aside()?;

    let row = match one.thing.folder {
        true => {
            let held = held.clone();
            let steps = one.steps()?;

            let Ok(walks) = Handler::and_stay(move |showing| {
                showing.forget_typing();

                let steps = steps.clone();

                let Ok(()) = press(&held, FilesEvent::Walked { tab, steps }, showing);
            });
            let Ok(row) = Row::new(&one.thing.name, Aside(&aside), walks);
            let Ok(opens) = row.opening();

            opens
        }
        false => {
            let here = here_of(held, tab)?;
            let at = one.at(&here)?;
            let said = said(&at)?;
            let Ok(opens) = Program::XdgOpen.name();
            let Ok(runs) = Handler::run(&[opens, &said]);
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
        Room::Retained => {},
    }

    match thing.folder {
        true => return Ok(Picture::Named(Icon::Folder)),
        false => {},
    }

    let worth = thing.worth_a_picture()?;

    let found = match worth {
        Worth::APicture => thumbnails::found(store, &here.join(&thing.name))?,
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
    thing: &Entry,
    at: Line,
    picture: &Picture,
) -> Result<Row, Never> {
    let aside = listing::aside(thing)?;

    let row = match thing.folder {
        true => {
            let held = held.clone();
            let name = thing.name.clone();

            let Ok(enters) = Handler::and_stay(move |showing| {
                let name = name.clone();

                let Ok(()) = press(&held, FilesEvent::Entered { tab, name, at: at.0 }, showing);
            });
            let Ok(row) = Row::new(&thing.name, Aside(&aside), enters);
            let Ok(opens) = row.opening();

            opens
        }
        false => {
            let here = here_of(held, tab)?;
            let path = here.join(&thing.name);
            let Ok(opens) = Program::XdgOpen.name();
            let Ok(runs) = Handler::run(&[opens, &path.to_string_lossy()]);
            let Ok(row) = Row::new(&thing.name, Aside(&aside), runs);

            row
        }
    };

    let held = held.clone();
    let thing = thing.clone();
    let here = here_of(&held, tab)?;
    let key = said(&here.join(&thing.name))?;

    let Ok(pictured) = row.picturing(picture.clone());
    let Ok(pictured) = pictured.selectable(&key);

    pictured.offering(move |showing| {
        let onto = Destination::Ways { thing: thing.clone(), from: at.0 };

        let Ok(()) = look(&held, tab, onto, showing, Line(WAYS_START));

        false
    })
}

fn put_down_row(held: &ActorAddress, holding: &Holding, here: &Path) -> Result<Row, Never> {
    let arguments = match holding.moving {
        Carrying::ToMove => {
            let Ok(arguments) = Program::Mv.arguments(&["--"]);

            arguments
        }
        Carrying::ToCopy => {
            let Ok(arguments) = Program::Cp.arguments(&["-r", "--"]);

            arguments
        },
    };
    let what: Vec<String> = holding.paths.iter().map(|path| path.to_string_lossy().to_string()).collect();
    let into = said(here)?;
    let arguments = [arguments, what, vec![into]].concat();
    let held = held.clone();
    let says = holding.says()?;

    let Ok(puts) = Handler::and_stay(move |showing| {
        let Ok(()) = press(&held, FilesEvent::PutDown, showing);

        showing.later(arguments.clone());
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
    let here = here.to_path_buf();
    let held = held.clone();

    let answer = answered(move |showing, word| {
        let Ok(name) = doing::a_name(word);

        let name = match name {
            Some(name) => name,
            None => return,
        };

        let Ok(()) = back_to_the_folder(&held, tab, showing, from);

        let Ok(made) = said(&here.join(name));

        let Ok(making) = Program::Mkdir.arguments(&["--", &made]);

        showing.later(making);
    })?;

    showing.ask_aloud(NEW_FOLDER, answer);

    Ok(())
}

const NEW_FOLDER: &str = "New Folder";

fn new_folder_row(held: &ActorAddress, tab: u32, here: &Path, from: Line) -> Result<Row, Never> {
    let here = here.to_path_buf();
    let held = held.clone();

    let Ok(asks) = Handler::and_stay(move |showing| {
        let Ok(()) = ask_for_a_folder(&held, tab, &here, from, showing);
    });

    Row::new(NEW_FOLDER, Aside(""), asks)
}

fn here_too(held: &ActorAddress, tab: u32, row: Row) -> Result<Row, Never> {
    let held = held.clone();

    row.offering(move |showing| {
        let Ok(()) = look(&held, tab, Destination::Here { from: LINE }, showing, Line(HERE_START));

        false
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

const UNZIPS: &str = "files-unzip";

const ONE_FORMAT: &str = "Convert All Media";
const ONE_FORMAT_ASKS: &str =
    "Convert everything here? Songs become Opus, videos MKV.";
const ONE_FORMAT_DOES: &str = "Convert";

fn one_format_row(held: &ActorAddress, tab: u32, here: &Path, from: Line) -> Result<Row, Never> {
    let here = here.to_path_buf();
    let held = held.clone();

    let Ok(asks) = Handler::and_stay(move |showing| {
        let here = here.clone();
        let held = held.clone();

        let Ok(whole) = said(&here);

        let folder = match here.file_name() {
            Some(folder) => folder.to_string_lossy().to_string(),
            None => whole,
        };
        let said_as = folder.clone();

        let Ok(then) = taken(move |showing, _| {
            let Ok(()) = back_to_the_folder(&held, tab, showing, from);

            showing.note(&format!("Converting {folder}…"));

            let Ok(at) = said(&here);

            showing.later(vec!["downloads-format".to_string(), at]);
        });

        showing.sure(ONE_FORMAT_ASKS, Subject(&said_as), &[ONE_FORMAT_DOES], then);
    });

    Row::new(ONE_FORMAT, Aside(""), asks)
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

    let path = here.join(&thing.name);
    let ways = doing::ways(thing)?;

    for deed in ways {
        let row = deed_row(held, tab, thing, from, &path, deed)?;

        rows.push(row);
    }

    let new_folder = new_folder_row(held, tab, here, from)?;
    let downloads_format = one_format_row(held, tab, here, from)?;

    let Ok(naming) = Row::naming(&format!("In {folder}"), Aside(""));

    rows.push(naming);
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
) -> Result<Vec<Row>, Never> {
    let path = here.join(&thing.name);
    let leaving = held.clone();
    let going_back = thing.clone();
    let opens = FileAction::OpenWith.says()?;

    let Ok(way_back) = Row::back(&thing.name, move |showing| {
        let onto = Destination::Ways { thing: going_back.clone(), from: from.0 };

        let Ok(()) = look(&leaving, tab, onto, showing, Line(WAYS_START));
    });
    let Ok(naming) = Row::naming(opens, Aside(""));
    let mut rows = vec![way_back, naming];

    let kind = kind_of(&path)?;

    let found = match kind {
        Some(kind) => crate::open_with::programs(&kind)?,
        None => Vec::new(),
    };

    match found.is_empty() {
        true => {
            let Ok(row) = Row::nothing("No App Opens This");

            rows.push(row);

            return Ok(rows);
        }
        false => {},
    }

    for (says, id) in found {
        let path = path.clone();

        let Ok(starts) = Handler::call(move |_| {
            let Ok(()) = crate::open_with::started(&id, &path);

            true
        });
        let Ok(row) = Row::new(&says, Aside(""), starts);

        rows.push(row);
    }

    Ok(rows)
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
            let Ok(opens) = Program::XdgOpen.name();
            let Ok(runs) = Handler::run(&[opens, &path.to_string_lossy()]);

            return Row::new(says, Aside(""), runs);
        }
        FileAction::Copy | FileAction::Delete | FileAction::Move | FileAction::OpenWith | FileAction::Rename
        | FileAction::Select | FileAction::Unzip | FileAction::Wallpaper => {},
    }

    let held = held.clone();
    let thing = thing.clone();
    let path = path.to_path_buf();

    let Ok(does) = Handler::and_stay(move |showing| {
        let Ok(()) = done(&held, tab, &thing, from, &path, deed, showing);
    });
    let Ok(row) = Row::new(says, Aside(""), does);

    Ok(match deed == FileAction::OpenWith {
        true => {
            let Ok(opens) = row.opening();

            opens
        }
        false => row,
    })
}

fn done(
    held: &ActorAddress,
    tab: u32,
    thing: &Entry,
    from: Line,
    path: &Path,
    deed: FileAction,
    showing: &dyn Showing,
) -> Result<(), Never> {
    match deed {
        FileAction::Open => (),
        FileAction::OpenWith => {
            let onto = Destination::Programs { thing: thing.clone(), from: from.0 };

            look(held, tab, onto, showing, Line(1))?;
        }
        FileAction::Delete => {
            let held = held.clone();
            let path = path.to_path_buf();
            let says = deed.says()?;

            let then = taken(move |showing, _| {
                let Ok(()) = back_to_the_folder(&held, tab, showing, from);

                let Ok(at) = said(&path);

                let Ok(trashing) = Program::Gio.arguments(&["trash", "--", &at]);

                showing.later(trashing);
            })?;

            showing.sure(doing::SURE, Subject(&thing.name), &[says], then);
        }
        FileAction::Copy | FileAction::Move => {
            let carrying = match deed == FileAction::Move {
                true => Carrying::ToMove,
                false => Carrying::ToCopy,
            };
            let holding = Holding::of(thing, path.to_path_buf(), carrying)?;

            quietly(held, FilesEvent::PickedUp(holding))?;
            back_to_the_folder(held, tab, showing, from)?;
        }
        FileAction::Select => {
            back_to_the_folder(held, tab, showing, from)?;

            let at = said(path)?;

            showing.select(vec![at]);
        }
        FileAction::Unzip => {
            back_to_the_folder(held, tab, showing, from)?;

            showing.note(&format!("Unzipping {}…", thing.name));

            let at = said(path)?;

            showing.later(vec![UNZIPS.to_string(), at]);
        }
        FileAction::Wallpaper => {
            back_to_the_folder(held, tab, showing, from)?;

            showing.note(&format!("Setting {} as wallpaper…", thing.name));

            let at = said(path)?;

            showing.later(vec!["wallpaper-render".to_string(), "--take".to_string(), at]);
        }
        FileAction::Rename => {
            let held = held.clone();
            let path = path.to_path_buf();

            let answer = answered(move |showing, word| {
                let Ok(name) = doing::a_name(word);

                let name = match name {
                    Some(name) => name,
                    None => return,
                };

                let Ok(()) = back_to_the_folder(&held, tab, showing, from);

                let beside = path.with_file_name(name);

                let Ok(was) = said(&path);

                let Ok(now) = said(&beside);

                showing.later(vec!["mv".to_string(), "--".to_string(), was, now]);
            })?;

            showing.ask_aloud(&format!("Rename {}", thing.name), answer);
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

fn said(path: &Path) -> Result<String, Never> {
    Ok(path.to_string_lossy().to_string())
}

fn answered(
    then: impl Fn(&dyn Showing, &str) + Send + Sync + 'static,
) -> Result<Answer, Never> {
    Ok(Arc::new(then))
}

fn taken(then: impl Fn(&dyn Showing, u32) + Send + Sync + 'static) -> Result<OnChosen, Never> {
    Ok(Arc::new(then))
}

fn rows(held: &ActorAddress, tab: u32) -> Result<Vec<Row>, Never> {
    let held_now = standing_of(held)?;
    let here = standing::here(&held_now, tab)?;
    let onto = standing::onto(&held_now, tab)?;

    match onto {
        Destination::Folder => folder_rows(held, tab, &here),
        Destination::Here { from } => here_rows(held, tab, &here, Line(from)),
        Destination::Programs { thing, from } => program_rows(held, tab, &thing, Line(from), &here),
        Destination::Ways { thing, from } => way_rows(held, tab, &thing, Line(from), &here),
    }
}

fn pages(held: &ActorAddress) -> Result<Vec<Page>, Never> {
    let standing = standing_of(held)?;
    let titles = standing::titles(&standing)?;
    let mut pages: Vec<Page> = Vec::new();

    for (tab, title) in titles.iter().enumerate() {
        let Ok(tab) = console_core_number_conversion::fitted::<_, u32>(tab);
        let page = page(held, tab, title)?;

        pages.push(page);
    }

    Ok(pages)
}

fn carried_many(page: Page, held: &ActorAddress, deed: FileAction, moving: Carrying) -> Result<Page, Never> {
    let held = held.clone();
    let Ok(says) = deed.says();

    page.selecting(says, move |_, keys| {
        let paths = keys.iter().map(PathBuf::from).collect();
        let Ok(holding) = Holding::many(paths, moving);
        let Ok(()) = quietly(&held, FilesEvent::PickedUp(holding));
    })
}

fn page(held: &ActorAddress, tab: u32, title: &str) -> Result<Page, Never> {
    let reading = held.clone();
    let backing = held.clone();
    let arriving = held.clone();

    let Ok(asking) = Rows::asked(move || {
        let Ok(rows) = rows(&reading, tab);

        rows
    });

    let Ok(page) = Page::new(title, asking);
    let Ok(page) = page.on_arriving(move |showing| {
        let Ok(here) = here_of(&arriving, tab);

        let Ok(()) = wanting_pictures(showing, &here);

        let Ok(()) = stand_where_asked(&arriving, tab, showing);
    });
    let Ok(page) = carried_many(page, held, FileAction::Copy, Carrying::ToCopy);
    let Ok(page) = carried_many(page, held, FileAction::Move, Carrying::ToMove);
    let Ok(page) = page.trashing_selected();
    let Ok(page) = page.on_back(move |showing| {
        let Ok(was) = standing_of(&backing);

        let Ok(()) = press(&backing, FilesEvent::Back { tab }, showing);

        let Ok(closes) = closes(&was, tab);

        match closes {
            Closes::Yes => true,
            Closes::No => false,
        }
    });

    let standing = standing_of(held)?;
    let onto = standing::onto(&standing, tab)?;

    match onto {
        Destination::Folder => {},
        Destination::Here { .. } | Destination::Programs { .. } | Destination::Ways { .. } => return Ok(page),
    }

    let typing = held.clone();

    page.searching(ABOUT, move |showing, word| {
        let heard = FilesEvent::Typed { tab, word: word.to_string() };

        let Ok(()) = press(&typing, heard, showing);
    })
}

fn went_to(standing: &mut Standing, said: &str) -> Result<Option<String>, Never> {
    let path = Path::new(said);

    match path.exists() {
        true => {},
        false => return Ok(None),
    }

    let is = match path.is_dir() {
        true => places::Is::AFolder,
        false => places::Is::AFile,
    };

    let leading = places::leading_to(&standing.places, path, is)?;

    let leading = match leading {
        Some(leading) => leading,
        None => return Ok(None),
    };

    for step in &leading.steps {
        let onto = first_thing(standing, leading.place)?;

        let Ok(slot) = console_core_number_conversion::index(leading.place);

        let walk = match standing.walks.get_mut(slot) {
            Some(walk) => walk,
            None => return Ok(None),
        };

        walk.enter(step, onto)?;
    }

    standing.stand_on = leading.stand_on.map(|name| (leading.place, name));

    let Ok(slot) = console_core_number_conversion::index(leading.place);

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

    let Ok(mut first) = Standing::of(places.clone());

    let opened_at = match asked.as_deref() {
        Some(said) => {
            let Ok(opened) = went_to(&mut first, said);

            opened
        }
        None => None,
    };

    drop(first);

    let asked_again = asked.clone();

    let Ok(standing) = actor::supervise(move || {
        let Ok(mut standing) = Standing::of(places.clone());

        match asked_again.as_deref() {
            Some(said) => {
                let _ = went_to(&mut standing, said);
            }
            None => {},
        }

        Looking(standing)
    });

    let held = standing.address.clone();

    let Ok(card) = Card::new(Arc::new(move || {
        let Ok(pages) = pages(&held);

        pages
    }));
    let Ok(card) = card.opening_at(opened_at.as_deref().or(asked.as_deref()));

    card.shutting(Box::new(move || standing.shutdown()))
}
