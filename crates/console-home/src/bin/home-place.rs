//! The card that says which applications are on the home screen.
//!
//! Y anywhere on the home screen opens it. It is the machine's whole list, in
//! the order the menu shows it, with a word beside the ones that are on the
//! home screen already; A puts one on and takes one off, and the card stays up
//! while somebody goes down the list deciding. B closes it.
//!
//! Where a thing goes is not asked here. A card that asked "which square"
//! after "which application" would be two questions for one decision, and the
//! second of them is a question about a grid nobody is looking at while the
//! card is over it. So this puts it in the first square that is free, and the
//! home screen is where it is moved to wherever it is wanted: hold a finger on
//! it -- or hold A -- and it is picked up, and the next press puts it down.
//!
//! A card of its own rather than something the home screen draws, because the
//! home screen is not a chooser. It is the desktop: while it is up the
//! shoulders change workspace and the Legion button leaves for Steam. A list
//! walked with A and backed out of with B is a chooser, and that is what this
//! is for the few seconds it takes to decide.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use console_home::shape::Shape;
use console_home::{Holding, Home};
use console_menu::{counts, entry, found};
use console_panel::page::{Does, Page, Picture, Row, Rows};
use console_panel::{chooser, panel};

/// What is said beside the ones that are on it.
const ON: &str = "on the home screen";

/// What the card is for, where the home screen is empty.
///
/// Said out loud because an empty home screen and a home screen this card
/// cannot read look the same from the front, and because the first thing
/// somebody does with a machine is put something on it.
const ABOUT: &str = "Choose what is on the home screen";

fn file() -> PathBuf {
    console_home::file(&found::home())
}

/// The grid as it is set, so that a square found free here is a square the
/// home screen actually has.
///
/// Read every time rather than held, for the same reason the placements are:
/// the settings tab can change it while this card is up, and a card that put
/// something in the sixth column of a five-column grid would be a press that
/// appears to do nothing.
fn shape() -> Shape {
    match std::fs::read_to_string(console_home::shape::at(&found::home())) {
        Ok(said) => Shape::read(&said),
        Err(_) => Shape::USUAL,
    }
}

/// What is on the home screen now, read fresh each time the card draws.
///
/// The home screen is running the whole time this is up and writes the same
/// file, so what was true when the card opened is not what to answer with.
fn home() -> Home {
    match std::fs::read_to_string(file()) {
        Ok(said) => Home::read(&said),
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Home::default(),
        Err(fault) => {
            eprintln!("home-place: {}: {fault}", file().display());

            Home::default()
        }
    }
}

/// Put it on, or take it off, whichever it is not.
///
/// The whole of what A does here. A row that is a switch is a row somebody can
/// press twice and be back where they were, which is what a list of things to
/// tick has to be.
fn turned(name: &str) {
    let mut home = home();

    match home.where_(name) {
        Some(_) => home.forget(name),
        // There is always a first free square -- a home screen whose every
        // pane is full grows another -- so putting something on is a thing
        // that cannot fail.
        None => {
            let spot = home.first_free(shape());
            home.place(spot, name);
        },
    }

    let at = file();

    if let Some(above) = at.parent() {
        let _ = std::fs::create_dir_all(above);
    }

    if let Err(fault) = std::fs::write(&at, home.written()) {
        eprintln!("home-place: {}: {fault}", at.display());
    }
}

/// Every application, in the order the menu shows them.
///
/// Read once, on the thread the panel reads its rows on, so the card is on the
/// screen while the desktop files are being opened.
fn everything() -> &'static Vec<(entry::Application, String)> {
    static ALL: OnceLock<Vec<(entry::Application, String)>> = OnceLock::new();
    ALL.get_or_init(|| listed(found::machine()))
}

/// The same list as it was written down last time, which is what this opens on.
fn before() -> &'static Vec<(entry::Application, String)> {
    static KEPT: OnceLock<Vec<(entry::Application, String)>> = OnceLock::new();
    KEPT.get_or_init(|| listed(found::remembered()))
}

fn listed(found: found::Found) -> Vec<(entry::Application, String)> {
    let names: Vec<String> = found.apps.keys().cloned().collect();

    counts::order(&names, &found::counted())
        .into_iter()
        .filter_map(|name| {
            let app = found.apps.get(&name)?;
            let picture = found.icon.get(&name).cloned().unwrap_or_default();

            Some((app.clone(), picture))
        })
        .collect()
}

fn rows(every: &'static [(entry::Application, String)]) -> Vec<Row> {
    let home = home();

    if every.is_empty() {
        return vec![Row::nothing("This machine has no applications on it")];
    }

    every
        .iter()
        .map(|(app, picture)| {
            let picture = match picture.is_empty() {
                true => Picture::Space,
                false => Picture::At(PathBuf::from(picture)),
            };
            let aside = match home.where_(&app.name) {
                Some(_) => ON,
                None => "",
            };
            let name = app.name.clone();

            Row::new(&app.name, aside, Does::and_stay(move |showing| {
                turned(&name);
                showing.refresh();
            }))
            .picturing(picture)
        })
        .collect()
}

fn main() {
    if chooser::alone("home-place", chooser::Again::Closes) == chooser::Alone::No {
        return;
    }

    // Said only where there is nothing on the home screen yet, which is the
    // one time the card has to explain itself.
    let heading = match home().holding() {
        Holding::Nothing => ABOUT,
        Holding::Something => "The home screen",
    };

    panel::show(
        Arc::new(move || {
            vec![Page::new(heading, Rows::asked(|| rows(everything())))
                .meanwhile(|| rows(before()))]
        }),
        0,
        None,
    );
}
