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


use console_external_programs::Program;
use console_never::Never;
use console_number_conversion::fitted;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use console_panel::icons::Icon;
use gtk4::gio;
use gtk4::glib::{self, UserDirectory};
use gtk4::prelude::*;
use crate::doing::{self, Carrying, Deed, Holding};
use crate::listing::{self, Entry, Room, Shown, Worth};
use crate::looking::{self, Found};
use crate::places::{self, Place, WANTED};
use crate::thumbs;
use crate::standing::{
    self, Closes, Files, HERE_START, Heard, Its, LINE, Onto, Standing, WAYS_START, closes,
    first_thing,
};
use console_panel::actor::{self, Addr, Answer as Reply};
use console_program_contract::{Doing, Program as _, Turn, Word};
use console_panel::page::{Answer, Does, Heading, Page, Picture, Row, Rows, Showing, Taken};
use console_panel::card::{Card, Door};

enum Msg {
    Heard(Heard, Reply<Vec<Doing<Its>>>),
    At(Reply<Standing>),
}

struct Looking(Standing);

impl actor::Machine for Looking {
    type Msg = Msg;

    fn step(self, message: Msg) -> Self {
        match message {
            Msg::Heard(heard, answer) => {
                let Turn { now, doings } = Files::heard(&self.0, &Word::Its(heard));
                let _ = answer.say(doings);

                Looking(now)
            }
            Msg::At(answer) => {
                let _ = answer.say(self.0.clone());

                self
            },
        }
    }
}

type Held = Addr<Msg>;

fn standing_of(held: &Held) -> Result<Standing, Never> {
    match held.ask(Msg::At) {
        Ok(standing) => Ok(standing),
        Err(_) => {
            eprintln!("files-panel: the panel's own state has gone, so it drew nothing");

            Standing::of(Vec::new())
        }
    }
}

fn decided(held: &Held, heard: Heard) -> Result<Vec<Doing<Its>>, Never> {
    Ok(match held.ask(|answer| Msg::Heard(heard, answer)) {
        Ok(doings) => doings,
        Err(_) => {
            eprintln!("files-panel: the panel's own state has gone, so the press did nothing");

            Vec::new()
        }
    })
}

fn press(held: &Held, heard: Heard, showing: &dyn Showing) -> Result<(), Never> {
    let doings = decided(held, heard)?;

    for doing in doings {
        match doing {
            Doing::Its(Its::Replace(row)) => showing.replace(row),
            Doing::Its(Its::ForgetTyping) => showing.forget_typing(),
            Doing::Its(Its::WantingPictures(here)) => wanting_pictures(showing, &here)?,

            Doing::Ask(_)
            | Doing::Watch(_)
            | Doing::AskWhoever(_)
            | Doing::Start(_)
            | Doing::Listen(_)
            | Doing::Deafen(_)
            | Doing::Write(_)
            | Doing::Say(_)
            | Doing::Print(_)
            | Doing::Stop(_) => {},
        }
    }

    Ok(())
}

fn quietly(held: &Held, heard: Heard) -> Result<(), Never> {
    let _ = decided(held, heard)?;

    Ok(())
}

fn here_of(held: &Held, tab: usize) -> Result<PathBuf, Never> {
    let standing = standing_of(held)?;

    standing::here(&standing, tab)
}

fn called(held: &Held, tab: usize) -> Result<String, Never> {
    let standing = standing_of(held)?;

    standing::called(&standing, tab)
}

fn typed_in(held: &Held, tab: usize) -> Result<String, Never> {
    let standing = standing_of(held)?;

    standing::typed(&standing, tab)
}

fn stand_where_asked(held: &Held, tab: usize, showing: &dyn Showing) -> Result<(), Never> {
    let here = here_of(held, tab)?;
    let things = read(&here)?;
    let names = things.into_iter().map(|thing| thing.name).collect();

    press(held, Heard::Arrived { tab, names }, showing)
}

fn look(
    held: &Held,
    tab: usize,
    onto: Onto,
    showing: &dyn Showing,
    row: usize,
) -> Result<(), Never> {
    press(held, Heard::Opened { tab, onto, row }, showing)
}

fn read(path: &Path) -> Result<Vec<Entry>, Never> {
    let asked = gio::File::for_path(path).enumerate_children(
        "standard::name,standard::type,standard::size,standard::fast-content-type",
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    );

    let Ok(children) = asked else { return Ok(Vec::new()) };

    let mut things = Vec::new();

    for about in children.flatten() {
        let name = about.name().to_string_lossy().to_string();
        let wanted = listing::wanted(&name)?;

        match wanted {
            Shown::No => continue,
            Shown::Yes => {},
        }

        let kind = kind_said(&about)?;
        let Ok(size) = fitted(about.size().max(0));

        things.push(Entry {
            folder: about.file_type() == gio::FileType::Directory,
            kind,
            size,
            name,
        });
    }

    listing::sorted(things)
}

fn kind_said(about: &gio::FileInfo) -> Result<String, Never> {
    Ok(about
        .attribute_string("standard::fast-content-type")
        .map(|kind| kind.to_string())
        .unwrap_or_default())
}

fn kind_of(path: &Path) -> Result<Option<String>, Never> {
    let Ok(about) = gio::File::for_path(path).query_info(
        "standard::content-type",
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    ) else {
        return Ok(None);
    };

    Ok(about.content_type().map(|kind| kind.to_string()))
}

fn programs(kind: &str) -> Result<Vec<(String, String)>, Never> {
    Ok(gio::AppInfo::recommended_for_type(kind)
        .iter()
        .filter_map(|app| {
            let id = app.id()?;

            Some((app.name().to_string(), id.to_string()))
        })
        .collect())
}

fn home() -> Result<Vec<Place>, Never> {
    let each = [
        UserDirectory::Documents,
        UserDirectory::Downloads,
        UserDirectory::Music,
        UserDirectory::Pictures,
        UserDirectory::Videos,
    ];
    let home = glib::home_dir();

    let Some((home_title, rest)) = WANTED.split_first() else { return Ok(Vec::new()) };

    let mut said: Vec<(&str, Option<PathBuf>)> = vec![(*home_title, Some(home.clone()))];

    said.extend(rest.iter().copied().zip(each.map(glib::user_special_dir)));

    let wanted = places::wanted_at(&home, &said)?;

    places::kept(wanted, |path| path.is_dir())
}

fn plugged_in() -> Result<Vec<Place>, Never> {
    let mounted = gio::VolumeMonitor::get().mounts();
    let mut places: Vec<Place> = Vec::new();

    for mount in mounted.iter().filter(|mount| mount.can_unmount()) {
        let Some(at) = mount.root().path() else { continue };

        let place = Place::new(&mount.name(), at)?;

        places.push(place);
    }

    Ok(places)
}

fn wanting_pictures(showing: &dyn Showing, here: &Path) -> Result<(), Never> {
    let said = said(here)?;

    showing.later(vec!["files-thumbs".to_string(), said]);

    Ok(())
}

fn folder_rows(held: &Held, tab: usize, here: &Path) -> Result<Vec<Row>, Never> {
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
            let Ok(naming) = Row::naming(&called, "");

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
    let store = thumbs::store(&glib::user_cache_dir())?;

    for thing in things {
        let at = rows.len().saturating_add(LINE);
        let picture = picture(&store, here, &thing, room)?;
        let row = thing_row(held, tab, &thing, at, &picture)?;

        rows.push(row);
    }

    match rows.iter().any(|row| {
        let Ok(heading) = row.heading();

        heading == Heading::No
    }) {
        true => {},
        false => {
            let row = new_folder_row(held, tab, here, LINE)?;

            rows.push(row);
        }
    }

    with_the_room(rows, room)
}

const ABOUT: &str = "Type to find, here and under here";

fn with_the_room(rows: Vec<Row>, room: Room) -> Result<Vec<Row>, Never> {
    match room {
        Room::Spared => return Ok(rows),
        Room::Kept => {},
    }

    let mut kept: Vec<Row> = Vec::new();

    for row in rows {
        let row = with_room(row)?;

        kept.push(row);
    }

    Ok(kept)
}

fn found_rows(held: &Held, tab: usize, here: &Path, word: &str) -> Result<Vec<Row>, Never> {
    let folder = called(held, tab)?;
    let leaving = held.clone();

    let Ok(way_back) = Row::back(&folder, move |showing| {
        let Ok(()) = stopped_looking(&leaving, tab, showing);
    });
    let mut rows = vec![way_back];

    let found = looking::under(here, word, &read)?;

    match found.is_empty() {
        true => {
            let Ok(row) = Row::nothing("Nothing here answers to that");

            rows.push(row);

            return Ok(rows);
        }
        false => {},
    }

    let store = thumbs::store(&glib::user_cache_dir())?;
    let things: Vec<Entry> = found.iter().map(|one| one.thing.clone()).collect();
    let room = listing::wants_room(&things)?;

    for one in found {
        let at = one.at(here)?;
        let holding = at.parent().unwrap_or(here).to_path_buf();
        let picture = picture(&store, &holding, &one.thing, room)?;
        let row = found_row(held, tab, &one, &picture)?;

        rows.push(row);
    }

    with_the_room(rows, room)
}

fn found_row(held: &Held, tab: usize, one: &Found, picture: &Picture) -> Result<Row, Never> {
    let aside = one.aside()?;

    let row = match one.thing.folder {
        true => {
            let held = held.clone();
            let steps = one.steps()?;

            let Ok(walks) = Does::and_stay(move |showing| {
                showing.forget_typing();

                let steps = steps.clone();

                let Ok(()) = press(&held, Heard::Walked { tab, steps }, showing);
            });
            let Ok(row) = Row::new(&one.thing.name, &aside, walks);
            let Ok(opens) = row.opening();

            opens
        }
        false => {
            let here = here_of(held, tab)?;
            let at = one.at(&here)?;
            let said = said(&at)?;
            let Ok(opens) = Program::XdgOpen.name();
            let Ok(runs) = Does::run(&[opens, &said]);
            let Ok(row) = Row::new(&one.thing.name, &aside, runs);

            row
        }
    };

    row.picturing(picture.clone())
}

fn stopped_looking(held: &Held, tab: usize, showing: &dyn Showing) -> Result<(), Never> {
    press(held, Heard::Back { tab }, showing)
}

fn picture(store: &Path, here: &Path, thing: &Entry, room: Room) -> Result<Picture, Never> {
    match room {
        Room::Spared => return Ok(Picture::None),
        Room::Kept => {},
    }

    match thing.folder {
        true => return Ok(Picture::Named(Icon::Folder)),
        false => {},
    }

    let worth = thing.worth_a_picture()?;

    let found = match worth {
        Worth::APicture => thumbs::found(store, &here.join(&thing.name))?,
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
    held: &Held,
    tab: usize,
    thing: &Entry,
    at: usize,
    picture: &Picture,
) -> Result<Row, Never> {
    let aside = listing::aside(thing)?;

    let row = match thing.folder {
        true => {
            let held = held.clone();
            let name = thing.name.clone();

            let Ok(enters) = Does::and_stay(move |showing| {
                let name = name.clone();

                let Ok(()) = press(&held, Heard::Entered { tab, name, at }, showing);
            });
            let Ok(row) = Row::new(&thing.name, &aside, enters);
            let Ok(opens) = row.opening();

            opens
        }
        false => {
            let here = here_of(held, tab)?;
            let path = here.join(&thing.name);
            let Ok(opens) = Program::XdgOpen.name();
            let Ok(runs) = Does::run(&[opens, &path.to_string_lossy()]);
            let Ok(row) = Row::new(&thing.name, &aside, runs);

            row
        }
    };

    let held = held.clone();
    let thing = thing.clone();

    let Ok(pictured) = row.picturing(picture.clone());

    pictured.offering(move |showing| {
        let onto = Onto::Ways { thing: thing.clone(), from: at };

        let Ok(()) = look(&held, tab, onto, showing, WAYS_START);

        false
    })
}

fn put_down_row(held: &Held, holding: &Holding, here: &Path) -> Result<Row, Never> {
    let argv = match holding.moving {
        Carrying::ToMove => {
            let Ok(argv) = Program::Mv.argv(&["--"]);

            argv
        }
        Carrying::ToCopy => {
            let Ok(argv) = Program::Cp.argv(&["-r", "--"]);

            argv
        },
    };
    let what = said(&holding.path)?;
    let into = said(here)?;
    let argv = [argv, vec![what, into]].concat();
    let held = held.clone();
    let says = holding.says()?;

    let Ok(puts) = Does::and_stay(move |showing| {
        let Ok(()) = press(&held, Heard::PutDown, showing);

        showing.later(argv.clone());
    });

    Row::new(&says, "", puts)
}

fn ask_for_a_folder(
    held: &Held,
    tab: usize,
    here: &Path,
    from: usize,
    showing: &dyn Showing,
) -> Result<(), Never> {
    let here = here.to_path_buf();
    let held = held.clone();

    let answer = answered(move |showing, word| {
        let Ok(name) = doing::a_name(word);

        let Some(name) = name else { return };

        let Ok(()) = back_to_the_folder(&held, tab, showing, from);

        let Ok(made) = said(&here.join(name));

        let Ok(making) = Program::Mkdir.argv(&["--", &made]);

        showing.later(making);
    })?;

    showing.ask_aloud(NEW_FOLDER, answer);

    Ok(())
}

const NEW_FOLDER: &str = "New folder";

fn new_folder_row(held: &Held, tab: usize, here: &Path, from: usize) -> Result<Row, Never> {
    let here = here.to_path_buf();
    let held = held.clone();

    let Ok(asks) = Does::and_stay(move |showing| {
        let Ok(()) = ask_for_a_folder(&held, tab, &here, from, showing);
    });

    Row::new(NEW_FOLDER, "", asks)
}

fn here_too(held: &Held, tab: usize, row: Row) -> Result<Row, Never> {
    let held = held.clone();

    row.offering(move |showing| {
        let Ok(()) = look(&held, tab, Onto::Here { from: LINE }, showing, HERE_START);

        false
    })
}

fn here_rows(held: &Held, tab: usize, here: &Path, from: usize) -> Result<Vec<Row>, Never> {
    let folder = called(held, tab)?;
    let leaving = held.clone();
    let new_folder = new_folder_row(held, tab, here, from)?;
    let one_format = one_format_row(held, tab, here, from)?;

    let Ok(way_back) = Row::back(&folder, move |showing| {
        let Ok(()) = back_to_the_folder(&leaving, tab, showing, from);
    });

    Ok(vec![way_back, new_folder, one_format])
}

const UNZIPS: &str = "files-unzip";

const ONE_FORMAT: &str = "Make everything one format";
const ONE_FORMAT_ASKS: &str = "Make everything in here one format?";
const ONE_FORMAT_YES: &str = "Yes, songs to opus and films to mkv";

fn one_format_row(held: &Held, tab: usize, here: &Path, from: usize) -> Result<Row, Never> {
    let here = here.to_path_buf();
    let held = held.clone();

    let Ok(asks) = Does::and_stay(move |showing| {
        let here = here.clone();
        let held = held.clone();

        let Ok(whole) = said(&here);

        let folder = here
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or(whole);
        let said_as = folder.clone();

        let Ok(then) = taken(move |showing, _| {
            let Ok(()) = back_to_the_folder(&held, tab, showing, from);

            showing.note(&format!("{folder} is being made one format, which takes a while"));

            let Ok(at) = said(&here);

            showing.later(vec!["one-format".to_string(), at]);
        });

        showing.sure(ONE_FORMAT_ASKS, &said_as, &[ONE_FORMAT_YES], then);
    });

    Row::new(ONE_FORMAT, "", asks)
}

fn way_rows(
    held: &Held,
    tab: usize,
    thing: &Entry,
    from: usize,
    here: &Path,
) -> Result<Vec<Row>, Never> {
    let folder = called(held, tab)?;
    let leaving = held.clone();
    let aside = listing::aside(thing)?;

    let Ok(way_back) = Row::back(&folder, move |showing| {
        let Ok(()) = back_to_the_folder(&leaving, tab, showing, from);
    });
    let Ok(naming) = Row::naming(&thing.name, &aside);
    let mut rows = vec![way_back, naming];

    let path = here.join(&thing.name);
    let ways = doing::ways(thing)?;

    for deed in ways {
        let row = deed_row(held, tab, thing, from, &path, deed)?;

        rows.push(row);
    }

    let new_folder = new_folder_row(held, tab, here, from)?;
    let one_format = one_format_row(held, tab, here, from)?;

    let Ok(naming) = Row::naming(&format!("In {folder}"), "");

    rows.push(naming);
    rows.push(new_folder);
    rows.push(one_format);

    Ok(rows)
}

fn program_rows(
    held: &Held,
    tab: usize,
    thing: &Entry,
    from: usize,
    here: &Path,
) -> Result<Vec<Row>, Never> {
    let path = here.join(&thing.name);
    let leaving = held.clone();
    let going_back = thing.clone();
    let opens = Deed::OpenWith.says()?;

    let Ok(way_back) = Row::back(&going_back.name.clone(), move |showing| {
        let onto = Onto::Ways { thing: going_back.clone(), from };

        let Ok(()) = look(&leaving, tab, onto, showing, WAYS_START);
    });
    let Ok(naming) = Row::naming(opens, "");
    let mut rows = vec![way_back, naming];

    let kind = kind_of(&path)?;

    let found = match kind {
        Some(kind) => programs(&kind)?,
        None => Vec::new(),
    };

    match found.is_empty() {
        true => {
            let Ok(row) = Row::nothing("Nothing here opens this");

            rows.push(row);

            return Ok(rows);
        }
        false => {},
    }

    for (says, id) in found {
        let path = path.clone();

        let Ok(starts) = Does::call(move |_| {
            let Ok(()) = started(&id, &path);

            true
        });
        let Ok(row) = Row::new(&says, "", starts);

        rows.push(row);
    }

    Ok(rows)
}

fn started(id: &str, path: &Path) -> Result<(), Never> {
    let found = gio::AppInfo::all();

    let Some(app) = found.iter().find(|app| app.id().is_some_and(|its| its == id)) else {
        return Ok(());
    };

    let _ = app.launch(&[gio::File::for_path(path)], gio::AppLaunchContext::NONE);

    Ok(())
}

fn deed_row(
    held: &Held,
    tab: usize,
    thing: &Entry,
    from: usize,
    path: &Path,
    deed: Deed,
) -> Result<Row, Never> {
    let says = deed.says()?;

    match deed {
        Deed::Open => {
            let Ok(opens) = Program::XdgOpen.name();
            let Ok(runs) = Does::run(&[opens, &path.to_string_lossy()]);

            return Row::new(says, "", runs);
        }
        Deed::Copy | Deed::Delete | Deed::Move | Deed::OpenWith | Deed::Rename
        | Deed::Unzip | Deed::Wallpaper => {},
    }

    let held = held.clone();
    let thing = thing.clone();
    let path = path.to_path_buf();

    let Ok(does) = Does::and_stay(move |showing| {
        let Ok(()) = done(&held, tab, &thing, from, &path, deed, showing);
    });
    let Ok(row) = Row::new(says, "", does);

    Ok(match deed == Deed::OpenWith {
        true => {
            let Ok(opens) = row.opening();

            opens
        }
        false => row,
    })
}

fn done(
    held: &Held,
    tab: usize,
    thing: &Entry,
    from: usize,
    path: &Path,
    deed: Deed,
    showing: &dyn Showing,
) -> Result<(), Never> {
    match deed {
        Deed::Open => (),
        Deed::OpenWith => {
            let onto = Onto::Programs { thing: thing.clone(), from };

            look(held, tab, onto, showing, 1)?;
        }
        Deed::Delete => {
            let held = held.clone();
            let path = path.to_path_buf();
            let says = deed.says()?;

            let then = taken(move |showing, _| {
                let Ok(()) = back_to_the_folder(&held, tab, showing, from);

                let Ok(at) = said(&path);

                let Ok(trashing) = Program::Gio.argv(&["trash", "--", &at]);

                showing.later(trashing);
            })?;

            showing.sure(doing::SURE, &thing.name, &[says], then);
        }
        Deed::Copy | Deed::Move => {
            let carrying = match deed == Deed::Move {
                true => Carrying::ToMove,
                false => Carrying::ToCopy,
            };
            let holding = Holding::of(thing, path.to_path_buf(), carrying)?;

            quietly(held, Heard::Held(holding))?;
            back_to_the_folder(held, tab, showing, from)?;
        }
        Deed::Unzip => {
            back_to_the_folder(held, tab, showing, from)?;

            showing.note(&format!("{} is being unzipped", thing.name));

            let at = said(path)?;

            showing.later(vec![UNZIPS.to_string(), at]);
        }
        Deed::Wallpaper => {
            back_to_the_folder(held, tab, showing, from)?;

            showing.note(&format!(
                "{} is being made into a wallpaper, which takes about a minute",
                thing.name
            ));

            let at = said(path)?;

            showing.later(vec!["sky-press".to_string(), "--take".to_string(), at]);
        }
        Deed::Rename => {
            let held = held.clone();
            let path = path.to_path_buf();

            let answer = answered(move |showing, word| {
                let Ok(name) = doing::a_name(word);

                let Some(name) = name else { return };

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
    held: &Held,
    tab: usize,
    showing: &dyn Showing,
    from: usize,
) -> Result<(), Never> {
    look(held, tab, Onto::Folder, showing, from)
}

fn went_up(held: &Held, tab: usize, showing: &dyn Showing) -> Result<(), Never> {
    press(held, Heard::Up { tab }, showing)
}

fn said(path: &Path) -> Result<String, Never> {
    Ok(path.to_string_lossy().to_string())
}

fn answered(
    then: impl Fn(&dyn Showing, &str) + Send + Sync + 'static,
) -> Result<Answer, Never> {
    Ok(Arc::new(then))
}

fn taken(then: impl Fn(&dyn Showing, usize) + Send + Sync + 'static) -> Result<Taken, Never> {
    Ok(Arc::new(then))
}

fn rows(held: &Held, tab: usize) -> Result<Vec<Row>, Never> {
    let held_now = standing_of(held)?;
    let here = standing::here(&held_now, tab)?;
    let onto = standing::onto(&held_now, tab)?;

    match onto {
        Onto::Folder => folder_rows(held, tab, &here),
        Onto::Here { from } => here_rows(held, tab, &here, from),
        Onto::Programs { thing, from } => program_rows(held, tab, &thing, from, &here),
        Onto::Ways { thing, from } => way_rows(held, tab, &thing, from, &here),
    }
}

fn pages(held: &Held) -> Result<Vec<Page>, Never> {
    let standing = standing_of(held)?;
    let titles = standing::titles(&standing)?;
    let mut pages: Vec<Page> = Vec::new();

    for (tab, title) in titles.iter().enumerate() {
        let page = page(held, tab, title)?;

        pages.push(page);
    }

    Ok(pages)
}

fn page(held: &Held, tab: usize, title: &str) -> Result<Page, Never> {
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
    let Ok(page) = page.on_back(move |showing| {
        let Ok(was) = standing_of(&backing);

        let Ok(()) = press(&backing, Heard::Back { tab }, showing);

        let Ok(closes) = closes(&was, tab);

        match closes {
            Closes::Yes => true,
            Closes::No => false,
        }
    });

    let standing = standing_of(held)?;
    let onto = standing::onto(&standing, tab)?;

    match onto {
        Onto::Folder => {},
        Onto::Here { .. } | Onto::Programs { .. } | Onto::Ways { .. } => return Ok(page),
    }

    let typing = held.clone();

    page.searching(ABOUT, move |showing, word| {
        let heard = Heard::Typed { tab, word: word.to_string() };

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

    let Some(leading) = places::leading_to(&standing.places, path, is)? else { return Ok(None) };

    for step in &leading.steps {
        let onto = first_thing(standing, leading.place)?;

        let Some(walk) = standing.walks.get_mut(leading.place) else { return Ok(None) };

        walk.enter(step, onto)?;
    }

    standing.stand_on = leading.stand_on.map(|name| (leading.place, name));

    let Some(place) = standing.places.get(leading.place) else { return Ok(None) };

    Ok(Some(place.title.clone()))
}


pub const WHO: &str = "files-panel";

const DOOR: &str = "files";

pub fn door(_argv: &[String]) -> Result<Door, Never> {
    Door::closing(DOOR)
}

pub fn card(argv: &[String]) -> Result<Card, Never> {
    let asked = argv.first().cloned();
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

    let held = standing.addr.clone();

    let Ok(card) = Card::new(Arc::new(move || {
        let Ok(pages) = pages(&held);

        pages
    }));
    let Ok(card) = card.opening_at(opened_at.as_deref().or(asked.as_deref()));

    card.shutting(Box::new(move || standing.shutdown()))
}
