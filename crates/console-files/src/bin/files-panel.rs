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

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gtk4::gio;
use gtk4::glib::{self, UserDirectory};
use gtk4::prelude::*;
use console_files::doing::{self, Deed, Holding};
use console_files::listing::{self, Entry};
use console_files::looking::{self, Found};
use console_files::places::{self, Place, WANTED};
use console_files::thumbs;
use console_files::walk::Walk;
use console_panel::page::{Answer, Does, Page, Picture, Row, Rows, Showing, Taken};
use console_panel::{chooser, panel};

/// What a tab is looking at.
///
/// A question is a place you are rather than a thing drawn over the folder,
/// which is what makes B mean one thing everywhere: out of the question, then
/// out of the folder, then out of the place, then out of the panel. Each one
/// remembers the row it was opened from, so leaving it puts the highlight back
/// on the thing it was about.
#[derive(Clone)]
enum Onto {
    Folder,
    /// What can be done with the folder being stood in, rather than with
    /// anything in it.
    Here { from: usize },
    Programs { thing: Entry, from: usize },
    Ways { thing: Entry, from: usize },
}

/// Where every tab is standing, and what it is holding.
///
/// One walk per place, kept side by side, so turning the shoulders leaves each
/// tab where it was rather than sending it back to the top. Looking at
/// something in Pictures, going to Downloads for a moment and coming back is a
/// thing a person does, and a browser that forgets is one that has to be walked
/// down again each time.
struct Standing {
    /// One for the whole panel rather than one per tab. Carrying a photograph
    /// from Pictures to a stick is the reason it exists, and those are two tabs.
    holding: Option<Holding>,
    onto: Vec<Onto>,
    places: Vec<Place>,
    /// What has been typed into each place's line, where anything has. One per
    /// place like the walks, so a word typed in Pictures is still there when the
    /// shoulders come back to it.
    typed: Vec<String>,
    walks: Vec<Walk>,
}

impl Standing {
    fn of(places: Vec<Place>) -> Self {
        let walks = places.iter().map(|place| Walk::of(&place.path)).collect();
        let onto = places.iter().map(|_| Onto::Folder).collect();
        let typed = places.iter().map(|_| String::new()).collect();
        Standing { holding: None, onto, places, typed, walks }
    }
}

type Held = Arc<Mutex<Standing>>;

/// The lock, taken and given back before anything is drawn or run.
fn standing<T>(held: &Held, then: impl FnOnce(&mut Standing) -> T) -> T {
    then(&mut held.lock().expect("the standing"))
}

/// Where a tab is, and what it is looking at.
fn at(held: &Held, tab: usize) -> (PathBuf, Onto) {
    standing(held, |standing| {
        (standing.walks[tab].here().to_path_buf(), standing.onto[tab].clone())
    })
}

/// What is being looked for in this place, if anything.
fn typed_in(held: &Held, tab: usize) -> String {
    standing(held, |standing| standing.typed[tab].clone())
}

/// Look for something else, or for nothing.
fn look_for(held: &Held, tab: usize, word: &str) {
    standing(held, |standing| standing.typed[tab] = word.to_string());
}

/// The line to type in is row nought of a folder, so every row of one is
/// counted from after it.
///
/// A question is not a folder and has no line, which is why this is added where
/// a folder's rows are numbered and nowhere else.
const LINE: usize = 1;

/// Where the first thing in a folder walked into stands.
///
/// Past the line to type in, past the way out, past the folder's own name, and
/// past what is being carried where anything is. A folder is walked into to see
/// what is in it, and a highlight left on the way back is a second press of A
/// taking her straight out of the folder the first one opened.
fn first_thing(standing: &Standing) -> usize {
    LINE + 2 + usize::from(standing.holding.is_some())
}

/// Look at something else, and stand on a given row of it.
fn look(held: &Held, tab: usize, onto: Onto, showing: &dyn Showing, row: usize) {
    standing(held, |standing| standing.onto[tab] = onto);
    showing.replace(row);
}

// ------------------------------------------------------------------ the disk

/// What is in a folder, or nothing if it will not be read.
///
/// A folder that cannot be read is a folder with nothing in it as far as this
/// is concerned. There is one on this device for every mount point a stick was
/// pulled out of, and a panel that shows an error where a listing goes is one
/// that has to be got out of before it can be used again.
fn read(path: &Path) -> Vec<Entry> {
    let asked = gio::File::for_path(path).enumerate_children(
        "standard::name,standard::type,standard::size,standard::fast-content-type",
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    );
    let Ok(children) = asked else { return Vec::new() };
    let mut things = Vec::new();
    for about in children.flatten() {
        let name = about.name().to_string_lossy().to_string();
        if !listing::wanted(&name) {
            continue;
        }
        things.push(Entry {
            folder: about.file_type() == gio::FileType::Directory,
            // The fast one, which goes by the name. The other reads the
            // beginning of every file in the folder, and this is asked of a
            // stick over USB while somebody waits for the listing.
            kind: kind_said(&about),
            size: about.size().max(0) as u64,
            name,
        });
    }
    listing::sorted(things)
}

/// What kind of thing the listing says this is.
///
/// The fast answer, which goes by the name. `content_type` on a file info is
/// the other one, and asking for it reads the beginning of every file in the
/// folder, which is a folder on a stick over USB being read twice while
/// somebody waits for it. Asked for one and read with the other, every kind
/// came back empty and nothing was ever worth a picture.
fn kind_said(about: &gio::FileInfo) -> String {
    about
        .attribute_string("standard::fast-content-type")
        .map(|kind| kind.to_string())
        .unwrap_or_default()
}

/// What kind of thing a file is, as the machine says it.
fn kind_of(path: &Path) -> Option<String> {
    let about = gio::File::for_path(path)
        .query_info("standard::content-type", gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
        .ok()?;
    about.content_type().map(|kind| kind.to_string())
}

/// What can open a thing of that kind, each by the name it is known as and the
/// name it is found by.
///
/// The identifier is carried rather than the application, because these rows
/// are built on a thread of its own and a thing GTK made is not one that can be
/// sent off the thread that made it. It is looked up again where it is used,
/// which is the drawing thread and the only place anything can be started from.
fn programs(kind: &str) -> Vec<(String, String)> {
    gio::AppInfo::recommended_for_type(kind)
        .iter()
        .filter_map(|app| Some((app.name().to_string(), app.id()?.to_string())))
        .collect()
}

/// The places this home has, by the machine's answer rather than by name.
///
/// A home directory keeps where its folders are in a file of its own, and on a
/// machine set up in another language they are called something else. Asking
/// glib is asking the same file everything else on the desktop asks, so the
/// Pictures tab is the folder a camera writes into rather than one that happens
/// to be spelt that way.
fn home() -> Vec<Place> {
    let each = [
        UserDirectory::Documents,
        UserDirectory::Downloads,
        UserDirectory::Music,
        UserDirectory::Pictures,
        UserDirectory::Videos,
    ];
    let home = glib::home_dir();
    let mut said: Vec<(&str, Option<PathBuf>)> = vec![(WANTED[0], Some(home.clone()))];
    said.extend(WANTED[1..].iter().copied().zip(each.map(glib::user_special_dir)));
    places::kept(places::wanted_at(&home, &said), |path| path.is_dir())
}

/// Anything plugged in, as tabs after the places.
///
/// Kept to what can be unmounted, which is what a stick and a card are and what
/// the disk this is running from is not. Without that the strip carries a tab
/// for the boot partition, which is a place to arrive at by accident and
/// nothing anybody came here to look at.
fn plugged_in() -> Vec<Place> {
    gio::VolumeMonitor::get()
        .mounts()
        .iter()
        .filter(|mount| mount.can_unmount())
        .filter_map(|mount| Some(Place::new(&mount.name(), mount.root().path()?)))
        .collect()
}

// ------------------------------------------------------------------ the rows

/// Ask for the pictures this folder wants, behind the panel.
///
/// The listing is already on the screen by now, drawn with whatever the store
/// had. This fills in the rest and the panel draws again when it is done, so
/// the first visit to a folder of photographs is names and then names with
/// pictures, and every visit after it is both at once.
fn wanting_pictures(showing: &dyn Showing, here: &Path) {
    showing.later(vec!["files-thumbs".to_string(), said(here)]);
}

/// The folder a tab is standing in, or what a word has found under it.
fn folder_rows(held: &Held, tab: usize, here: &Path) -> Vec<Row> {
    let word = typed_in(held, tab);
    if !word.trim().is_empty() {
        return found_rows(held, tab, here, &word);
    }

    let mut rows = Vec::new();
    let (above, called, holding) = standing(held, |standing| {
        (
            standing.walks[tab].above(&standing.places[tab].title),
            standing.walks[tab].called(&standing.places[tab].title),
            standing.holding.clone(),
        )
    });

    if let Some(above) = above {
        let leaving = Arc::clone(held);
        rows.push(here_too(held, tab, Row::back(&above, move |showing| {
            went_up(&leaving, tab, showing)
        })));
        // Where you are standing, which below the top of a place nothing else
        // on the panel says. The strip names the place and the way back names
        // the folder above it, so three folders down inside Home the one name
        // written nowhere was the folder in front of her. At the top there is
        // no such gap: the strip is already saying it, in pink, an inch above.
        rows.push(Row::naming(&called, ""));
    }
    if let Some(holding) = holding {
        rows.push(here_too(held, tab, put_down_row(held, &holding, here)));
    }

    let things = read(here);
    // Asked of the folder rather than of each row, so that a listing holding
    // photographs and folders together has its names starting in one place.
    let room = listing::wants_room(&things);
    let store = thumbs::store(&glib::user_cache_dir());
    for thing in things {
        // Where the row will stand once the panel has put its own line at the
        // top, because that is the number it is told to come back to.
        let at = rows.len() + LINE;
        rows.push(thing_row(held, tab, &thing, at, &picture(&store, here, &thing, room)));
    }
    // Y is where a new folder is asked for, and Y is asked of the row you are
    // standing on. An empty place has no row to stand on, so there it is a row
    // of its own: the alternative is a folder nothing can ever be put into.
    if !rows.iter().any(|row| !row.heading()) {
        rows.push(new_folder_row(held, tab, here, LINE));
    }

    match room {
        false => rows,
        true => rows.into_iter().map(with_room).collect(),
    }
}

/// What the line at the top of a folder says while nothing has been typed.
const ABOUT: &str = "Type to find, here and under here";

/// What a word found, nearest first.
///
/// The way back says the folder rather than the folder above, because what a
/// word puts on the screen is a list laid over the place she was standing in
/// and leaving it is arriving back where she was.
///
/// A found thing is opened or walked into. What else could be done with one is
/// not offered: everything behind Y is about a thing in the folder in front of
/// you, and these are not in it.
fn found_rows(held: &Held, tab: usize, here: &Path, word: &str) -> Vec<Row> {
    let folder = standing(held, |standing| standing.walks[tab].called(&standing.places[tab].title));
    let leaving = Arc::clone(held);
    let mut rows =
        vec![Row::back(&folder, move |showing| stopped_looking(&leaving, tab, showing))];

    let found = looking::under(here, word, &read);
    if found.is_empty() {
        rows.push(Row::said("Nothing here answers to that", ""));
        return rows;
    }
    let store = thumbs::store(&glib::user_cache_dir());
    let room = listing::wants_room(&found.iter().map(|one| one.thing.clone()).collect::<Vec<_>>());
    for one in found {
        let at = one.at(here);
        let holding = at.parent().unwrap_or(here).to_path_buf();
        let picture = picture(&store, &holding, &one.thing, room);
        rows.push(found_row(held, tab, &one, &picture));
    }
    match room {
        false => rows,
        true => rows.into_iter().map(with_room).collect(),
    }
}

/// One thing a word found: a file to open, or a folder to walk into from where
/// the search began.
fn found_row(held: &Held, tab: usize, one: &Found, picture: &Picture) -> Row {
    let row = match one.thing.folder {
        true => {
            let held = Arc::clone(held);
            let steps = one.steps();
            Row::new(&one.thing.name, &one.aside(), Does::and_stay(move |showing| {
                // The line is the panel's, and a folder arrived at is not a
                // search any more.
                showing.forget_typing();
                let (here, onto) = standing(&held, |standing| {
                    let onto = first_thing(standing);
                    for step in &steps {
                        standing.walks[tab].enter(step, onto);
                    }
                    (standing.walks[tab].here().to_path_buf(), onto)
                });
                showing.replace(onto);
                wanting_pictures(showing, &here);
            }))
            .opening()
        }
        false => {
            let at = one.at(&standing(held, |standing| standing.walks[tab].here().to_path_buf()));
            Row::new(&one.thing.name, &one.aside(), Does::run(&["xdg-open", &said(&at)]))
        }
    };
    row.picturing(picture.clone())
}

/// Stop looking, and stand at the top of the folder the search began in.
fn stopped_looking(held: &Held, tab: usize, showing: &dyn Showing) {
    showing.forget_typing();
    look_for(held, tab, "");
    showing.replace(LINE);
}

/// What a folder is, in the shape every icon theme has always drawn it.
/// Symbolic, so it is drawn in the ink of the row it sits on rather than
/// in a colour this palette never chose.
const FOLDER: &str = "folder-symbolic";

/// What is drawn in front of one thing, where there is room for anything.
///
/// A folder wears the mark that says it is one. A photograph and a film wear
/// themselves, once the picture has been made; everything else keeps the room
/// and puts nothing in it, because a page of documents each wearing a small
/// grey rectangle is harder to read than a page of names.
fn picture(store: &Path, here: &Path, thing: &Entry, room: bool) -> Picture {
    if !room {
        return Picture::None;
    }
    if thing.folder {
        return Picture::Named(FOLDER);
    }
    let found = thing.worth_a_picture().then(|| thumbs::found(store, &here.join(&thing.name)));
    match found.flatten() {
        None => Picture::Space,
        Some(at) => Picture::At(at),
    }
}

/// A row that keeps the room even though it is not a thing in the folder.
///
/// The way back, the folder's own name and what is being carried are rows like
/// any other, and a listing where three of them start an inch left of the rest
/// is one whose names do not line up.
fn with_room(row: Row) -> Row {
    match row.picture {
        Picture::None => row.picturing(Picture::Space),
        _ => row,
    }
}

/// One thing in the folder: a folder to walk into, or a file to open. Y asks it
/// what else it can be made to do.
fn thing_row(held: &Held, tab: usize, thing: &Entry, at: usize, picture: &Picture) -> Row {
    let aside = listing::aside(thing);
    let row = match thing.folder {
        true => {
            let held = Arc::clone(held);
            let name = thing.name.clone();
            Row::new(&thing.name, &aside, Does::and_stay(move |showing| {
                let (here, onto) = standing(&held, |standing| {
                    standing.walks[tab].enter(&name, at);
                    (standing.walks[tab].here().to_path_buf(), first_thing(standing))
                });
                // Onto the first thing in the folder rather than onto the row
                // that comes back out of it, which is what walking in was for.
                showing.replace(onto);
                wanting_pictures(showing, &here);
            }))
            .opening()
        }
        false => {
            let path = standing(held, |standing| standing.walks[tab].here().join(&thing.name));
            Row::new(&thing.name, &aside, Does::run(&["xdg-open", &path.to_string_lossy()]))
        }
    };
    let held = Arc::clone(held);
    let thing = thing.clone();
    row.picturing(picture.clone()).offering(move |showing| {
        look(&held, tab, Onto::Ways { thing: thing.clone(), from: at }, showing, WAYS_START);
        false
    })
}

/// Where the highlight lands in a question: past the way back and past the
/// heading, on the first row that does anything.
const WAYS_START: usize = 2;

/// What is being carried, and the folder it would land in.
fn put_down_row(held: &Held, holding: &Holding, here: &Path) -> Row {
    let argv = match holding.moving {
        true => vec!["mv".to_string(), "--".to_string()],
        false => vec!["cp".to_string(), "-r".to_string(), "--".to_string()],
    };
    let argv = [argv, vec![said(&holding.path), said(here)]].concat();
    let held = Arc::clone(held);
    Row::new(&holding.says(), "", Does::and_stay(move |showing| {
        standing(&held, |standing| standing.holding = None);
        showing.replace(LINE);
        showing.later(argv.clone());
    }))
}

/// What a new folder is called, asked, and then made.
///
/// The folder is the one being stood in rather than anything on the row Y was
/// pressed over, so the row it goes back to is that row: pressing Y over a
/// photograph, saying a name and coming back to the top of the listing is a
/// press that moves you somewhere you did not ask to go.
fn ask_for_a_folder(held: &Held, tab: usize, here: &Path, from: usize, showing: &dyn Showing) {
    let here = here.to_path_buf();
    let held = Arc::clone(held);
    showing.ask_aloud(NEW_FOLDER, answered(move |showing, word| {
        let Some(name) = doing::a_name(word) else { return };
        back_to_the_folder(&held, tab, showing, from);
        showing.later(vec!["mkdir".to_string(), "--".to_string(), said(&here.join(name))]);
    }));
}

/// What the row and the question are both called.
const NEW_FOLDER: &str = "New folder";

/// The one row a folder with nothing in it carries.
///
/// Everywhere else this is behind Y, which is asked of the row being stood on.
/// A place with nothing in it and nowhere above it has no such row, and without
/// this it would be a folder nothing could ever be put into.
fn new_folder_row(held: &Held, tab: usize, here: &Path, from: usize) -> Row {
    let here = here.to_path_buf();
    let held = Arc::clone(held);
    Row::new(NEW_FOLDER, "", Does::and_stay(move |showing| {
        ask_for_a_folder(&held, tab, &here, from, showing);
    }))
}

/// A row that is about the folder rather than about a thing in it, offering
/// under Y what can be done to the folder you are standing in.
///
/// The way back and what is being carried are the two of them. Neither is a
/// thing in the listing, so Y over either is about where you are, and they are
/// what an almost empty folder leaves under the thumb.
///
/// It asked for a folder's name at once while that was the only thing there was
/// to ask. It is a list now, because there are two.
fn here_too(held: &Held, tab: usize, row: Row) -> Row {
    let held = Arc::clone(held);
    row.offering(move |showing| {
        look(&held, tab, Onto::Here { from: LINE }, showing, HERE_START);
        false
    })
}

/// Where the highlight lands in the folder's own list: past the way back, on
/// the first thing that can be done.
const HERE_START: usize = 1;

/// What can be done with the folder being stood in.
///
/// No name over it. The way back is already saying which folder this is about,
/// an inch above, and a title repeating it is a row of the screen spent saying
/// a thing twice.
fn here_rows(held: &Held, tab: usize, here: &Path, from: usize) -> Vec<Row> {
    let folder = standing(held, |standing| standing.walks[tab].called(&standing.places[tab].title));
    let leaving = Arc::clone(held);
    vec![
        Row::back(&folder, move |showing| back_to_the_folder(&leaving, tab, showing, from)),
        new_folder_row(held, tab, here, from),
        one_format_row(held, tab, here, from),
    ]
}

/// What the row and its question are called.
const ONE_FORMAT: &str = "Make everything one format";
const ONE_FORMAT_ASKS: &str = "Make everything in here one format?";
const ONE_FORMAT_YES: &str = "Yes, songs to opus and films to mkv";

/// Make what is in the folder the one format this device keeps.
///
/// Asked first, because it rewrites every song and film in the folder at once
/// and there is no reading a listing to see what it did. What it replaces goes
/// to the wastebasket rather than being unlinked, which is what makes the
/// answer to that question a thing somebody can take back.
///
/// Handed to `later`: an hour of ffmpeg over somebody's music is not something
/// a card can wait for, and the corner says it has been set going.
fn one_format_row(held: &Held, tab: usize, here: &Path, from: usize) -> Row {
    let here = here.to_path_buf();
    let held = Arc::clone(held);
    Row::new(ONE_FORMAT, "", Does::and_stay(move |showing| {
        let here = here.clone();
        let held = Arc::clone(&held);
        let folder = here
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| said(&here));
        let said_as = folder.clone();
        showing.sure(ONE_FORMAT_ASKS, &said_as, &[ONE_FORMAT_YES], taken(move |showing, _| {
            back_to_the_folder(&held, tab, showing, from);
            showing.note(&format!("{folder} is being made one format, which takes a while"));
            showing.later(vec!["one-format".to_string(), said(&here)]);
        }));
    }))
}

/// What can be done with one thing, and the one thing that can be done with the
/// folder it is in.
///
/// Two lists under two names, because they are about two different things. A
/// new folder is made where you are standing rather than out of the thing under
/// the highlight, and put among Rename and Delete it would read as one more
/// thing that could happen to the photograph.
fn way_rows(held: &Held, tab: usize, thing: &Entry, from: usize, here: &Path) -> Vec<Row> {
    let folder = standing(held, |standing| standing.walks[tab].called(&standing.places[tab].title));
    let leaving = Arc::clone(held);
    let mut rows = vec![
        Row::back(&folder, move |showing| back_to_the_folder(&leaving, tab, showing, from)),
        Row::naming(&thing.name, &listing::aside(thing)),
    ];
    let path = here.join(&thing.name);
    rows.extend(doing::ways(thing).into_iter().map(|deed| deed_row(held, tab, thing, from, &path, deed)));
    rows.push(Row::naming(&format!("In {folder}"), ""));
    rows.push(new_folder_row(held, tab, here, from));
    rows.push(one_format_row(held, tab, here, from));
    rows
}

/// Which program opens this one.
fn program_rows(held: &Held, tab: usize, thing: &Entry, from: usize, here: &Path) -> Vec<Row> {
    let path = here.join(&thing.name);
    let leaving = Arc::clone(held);
    let going_back = thing.clone();
    let mut rows = vec![
        Row::back(&going_back.name.clone(), move |showing| {
            look(&leaving, tab, Onto::Ways { thing: going_back.clone(), from }, showing, WAYS_START);
        }),
        Row::naming(Deed::OpenWith.says(), ""),
    ];

    let found = kind_of(&path).as_deref().map(programs).unwrap_or_default();
    if found.is_empty() {
        rows.push(Row::said("Nothing here opens this", ""));
        return rows;
    }
    for (says, id) in found {
        let path = path.clone();
        rows.push(Row::new(&says, "", Does::call(move |_| {
            started(&id, &path);
            true
        })));
    }
    rows
}

/// Open a thing with the one program named, rather than with the usual one.
///
/// The application is looked up here rather than carried from where the row was
/// built. Those are built on a thread of their own, a thing GTK made cannot be
/// sent off the thread that made it, and this is the drawing thread and the
/// only place anything can be started from.
fn started(id: &str, path: &Path) {
    let found = gio::AppInfo::all();
    let Some(app) = found.iter().find(|app| app.id().is_some_and(|its| its == id)) else { return };
    let _ = app.launch(&[gio::File::for_path(path)], gio::AppLaunchContext::NONE);
}

/// One thing that can be done, as a row.
fn deed_row(held: &Held, tab: usize, thing: &Entry, from: usize, path: &Path, deed: Deed) -> Row {
    if deed == Deed::Open {
        return Row::new(deed.says(), "", Does::run(&["xdg-open", &path.to_string_lossy()]));
    }
    let held = Arc::clone(held);
    let thing = thing.clone();
    let path = path.to_path_buf();
    let row = Row::new(deed.says(), "", Does::and_stay(move |showing| {
        done(&held, tab, &thing, from, &path, deed, showing);
    }));
    // Which program opens it is a list under this row. The rest of what can be
    // done to a thing happens where it stands, or asks a question.
    match deed == Deed::OpenWith {
        true => row.opening(),
        false => row,
    }
}

/// What choosing one of them comes to.
fn done(
    held: &Held,
    tab: usize,
    thing: &Entry,
    from: usize,
    path: &Path,
    deed: Deed,
    showing: &dyn Showing,
) {
    match deed {
        // Handled where the row is made, because it leaves the panel.
        Deed::Open => (),
        Deed::OpenWith => {
            look(held, tab, Onto::Programs { thing: thing.clone(), from }, showing, 1)
        }
        Deed::Delete => {
            let held = Arc::clone(held);
            let path = path.to_path_buf();
            showing.sure(doing::SURE, &thing.name, &[deed.says()], taken(move |showing, _| {
                back_to_the_folder(&held, tab, showing, from);
                showing.later(vec!["gio".to_string(), "trash".to_string(), "--".to_string(), said(&path)]);
            }));
        }
        Deed::Copy | Deed::Move => {
            let holding = Holding::of(thing, path.to_path_buf(), deed == Deed::Move);
            standing(held, |standing| standing.holding = Some(holding));
            back_to_the_folder(held, tab, showing, from);
        }
        // It is decoded, brought into this palette, cut to the shape of this
        // screen and written out again, which is tens of seconds. Left running
        // rather than waited for, and the folder comes back at once.
        //
        // Which is the trouble: the folder coming back at once is the folder
        // exactly as it was, and nothing on it says the picture is being made.
        // So the corner says it, and says how long, because a minute of a
        // listing that looks untouched is a press somebody makes again.
        Deed::Wallpaper => {
            back_to_the_folder(held, tab, showing, from);
            showing.note(&format!(
                "{} is being made into a wallpaper, which takes about a minute",
                thing.name
            ));
            showing.later(vec![
                "sky-press".to_string(),
                "--take".to_string(),
                said(path),
            ]);
        }
        Deed::Rename => {
            let held = Arc::clone(held);
            let path = path.to_path_buf();
            showing.ask_aloud(&format!("Rename {}", thing.name), answered(move |showing, word| {
                let Some(name) = doing::a_name(word) else { return };
                back_to_the_folder(&held, tab, showing, from);
                let beside = path.with_file_name(name);
                showing.later(vec![
                    "mv".to_string(),
                    "--".to_string(),
                    said(&path),
                    said(&beside),
                ]);
            }));
        }
    }
}

// ------------------------------------------------------------------- leaving

/// Back to the listing, standing on the thing the question was about.
fn back_to_the_folder(held: &Held, tab: usize, showing: &dyn Showing, from: usize) {
    look(held, tab, Onto::Folder, showing, from);
}

/// The question a tab was on before it went one deeper.
fn thing_of(held: &Held, tab: usize) -> Onto {
    standing(held, |standing| match &standing.onto[tab] {
        Onto::Programs { thing, from } => Onto::Ways { thing: thing.clone(), from: *from },
        onto => onto.clone(),
    })
}

/// Out of the folder and back onto it.
fn went_up(held: &Held, tab: usize, showing: &dyn Showing) {
    let (back_to, here) = standing(held, |standing| {
        (standing.walks[tab].up(), standing.walks[tab].here().to_path_buf())
    });
    if let Some(back_to) = back_to {
        showing.replace(back_to);
        wanting_pictures(showing, &here);
    }
}

// ------------------------------------------------------------------ the wire

fn said(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

/// A line of text, once it has been typed.
fn answered(then: impl Fn(&dyn Showing, &str) + Send + Sync + 'static) -> Answer {
    Arc::new(then)
}

/// What a question does once one of its answers is taken.
fn taken(then: impl Fn(&dyn Showing, usize) + Send + Sync + 'static) -> Taken {
    Arc::new(then)
}

fn rows(held: &Held, tab: usize) -> Vec<Row> {
    let (here, onto) = at(held, tab);
    match onto {
        Onto::Folder => folder_rows(held, tab, &here),
        Onto::Here { from } => here_rows(held, tab, &here, from),
        Onto::Programs { thing, from } => program_rows(held, tab, &thing, from, &here),
        Onto::Ways { thing, from } => way_rows(held, tab, &thing, from, &here),
    }
}

fn pages(held: &Held) -> Vec<Page> {
    let titles: Vec<String> =
        standing(held, |standing| standing.places.iter().map(|place| place.title.clone()).collect());
    titles.iter().enumerate().map(|(tab, title)| page(held, tab, title)).collect()
}

fn page(held: &Held, tab: usize, title: &str) -> Page {
    let reading = Arc::clone(held);
    let backing = Arc::clone(held);
    let arriving = Arc::clone(held);
    let page = Page::new(title, Rows::asked(move || rows(&reading, tab)))
        .on_arriving(move |showing| {
            let here = standing(&arriving, |standing| standing.walks[tab].here().to_path_buf());
            wanting_pictures(showing, &here);
        })
        .on_back(move |showing| {
        // Out of what a word found, then out of the question, then out of the
        // folder, then out of the place, and only then out of the panel.
        let (onto, at_top, word) = standing(&backing, |standing| {
            (
                standing.onto[tab].clone(),
                standing.walks[tab].at_top(),
                standing.typed[tab].clone(),
            )
        });
        match onto {
            Onto::Folder if !word.trim().is_empty() => stopped_looking(&backing, tab, showing),
            Onto::Folder if at_top => return true,
            Onto::Folder => went_up(&backing, tab, showing),
            Onto::Here { from } | Onto::Ways { from, .. } => {
                back_to_the_folder(&backing, tab, showing, from)
            }
            Onto::Programs { .. } => {
                let ways = thing_of(&backing, tab);
                look(&backing, tab, ways, showing, WAYS_START);
            }
        }
        false
    });

    // Only over a folder. A question is a list of answers and nothing about it
    // narrows, and a line to type into over "Yes, delete" is an invitation to
    // type at a question that is not asking for letters.
    if !matches!(at(held, tab).1, Onto::Folder) {
        return page;
    }
    let typing = Arc::clone(held);
    page.searching(ABOUT, move |showing, word| {
        let changed = standing(&typing, |standing| {
            let changed = standing.typed[tab] != word;
            standing.typed[tab] = word.to_string();
            changed
        });
        // Standing on the line, which is where the letters go. The row that was
        // being stood on is not the row standing there now.
        if changed {
            showing.replace(0);
        }
    })
}

fn main() {
    // A place may be named, so something that means Pictures can open on it.
    let place = std::env::args().nth(1);

    if !chooser::alone("files", chooser::Again::Closes) {
        return;
    }

    // The places are read once. A stick pulled out while this is open leaves a
    // tab that reads as empty, which is what it is.
    let mut places = home();
    places.extend(plugged_in());
    let held: Held = Arc::new(Mutex::new(Standing::of(places)));

    panel::show(Arc::new(move || pages(&held)), 0, place.as_deref());
}
