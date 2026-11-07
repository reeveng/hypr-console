//! Make the pictures one folder's listing wants, and stop.
//!
//! Off the panel and not in it. A folder of two hundred photographs takes
//! seconds to work through, and a panel that did it where it draws would show
//! nothing at all until it was done. So the listing draws with whatever the
//! store already has, this runs behind it, and the panel draws again when it
//! ends.
//!
//! Every picture wanted is made at once, one per core, through
//! `console_concurrency`. None of them reads another, each is a decode of its
//! own, and a folder's worth made one after another was a folder's worth of
//! cores waiting.
//!
//! Once per thing, ever. What is made goes into the store every desktop shares,
//! so the second visit to a folder is the listing and the pictures together.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_content_types::Table;
use console_core_arguments::{Command, Operands, ValidationError, read};
use console_core_never::Never;
use console_core_places::Base;
use console_files::listing::{Entry, Worth};
use console_files::thumbnails;

const FOLDER: [&str; 1] = ["FOLDER"];

const COMMAND: Command = Command {
    name: "files-thumbnails",
    about: "make the pictures one folder's listing wants, and stop",
    flags: &[],
    operands: Operands::Named(&FOLDER),
};

fn folder(words: &[String]) -> Result<String, ValidationError> {
    let read = read(&COMMAND, words);
    let line = read?;
    let operands = line.exactly(FOLDER);
    let [folder] = operands?;

    Ok(folder.clone())
}

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let Ok(code) = console_core_arguments::run_main(&COMMAND, &words, folder, |folder| make(&folder));

    code
}

fn make(folder: &str) -> Result<(), Never> {

    let Ok(cache) = Base::Cache.user();

    let cache = match cache {
        Some(cache) => cache,
        None => {
            eprintln!("files-thumbnails: no home, so there is nowhere to keep a picture");

            return Ok(());
        }
    };

    let Ok(store) = thumbnails::store(&cache);

    match std::fs::create_dir_all(&store) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!(
                "files-thumbnails: {}: making the store the pictures go in: {fault}",
                store.display()
            );

            return Ok(());
        }
    }

    let Ok(wanting) = wanting(Path::new(&folder), &store);

    let Ok(cores) = console_concurrency::Cores::counted();
    let every = console_concurrency::map(cores, &wanting, |(thing, kind)| make_thumbnail(thing, kind, &store));

    match every {
        Ok(_every_one_tried) => {},
        Err(fault) => eprintln!("files-thumbnails: {fault}"),
    }

    Ok(())
}

fn wanting(folder: &Path, store: &Path) -> Result<Vec<(PathBuf, String)>, Never> {
    let kinds = match Table::here() {
        Ok(kinds) => kinds,
        Err(why) => {
            eprintln!("files-thumbnails: nothing says what a file is: {why}");

            Table::default()
        }
    };

    let children = match std::fs::read_dir(folder) {
        Ok(children) => children,
        Err(_nothing_to_walk) => return Ok(Vec::new()),
    };

    let mut wanting: Vec<(PathBuf, String)> = Vec::new();

    for about in children.flatten() {
        let path = about.path();
        let Ok(kind) = console_content_types::of(&kinds, &path);

        let entry = Entry {
            folder: path.is_dir(),
            kind,
            name: String::new(),
            size: 0,
        };
        let worth = entry.worth_a_picture()?;
        let found = thumbnails::find_thumbnail(store, &path)?;

        match worth == Worth::APicture && found.is_none() {
            true => wanting.push((path, entry.kind)),
            false => {},
        }
    }

    Ok(wanting)
}

fn make_thumbnail(thing: &Path, kind: &str, store: &Path) -> Result<(), Never> {
    let address = thumbnails::address(thing)?;

    let address = match address {
        Some(address) => address,
        None => return Ok(()),
    };

    let kept = thumbnails::of(store, &address)?;

    let kept = match kept {
        Some(kept) => kept,
        None => return Ok(()),
    };

    let Ok(changed) = changed_at(thing);
    let stamp = thumbnails::Stamp { address: &address, changed: &changed };
    let png = match thumbnails::drawn(thing, kind, Some(stamp)) {
        Ok(Some(png)) => png,
        Ok(None) => return Ok(()),
        Err(fault) => {
            eprintln!("files-thumbnails: {}: {fault}", thing.display());

            return Ok(());
        }
    };

    match console_core_atomic_writes::whole(&kept, &png) {
        Ok(()) => {},
        Err(fault) => eprintln!("files-thumbnails: {}: writing the picture: {fault}", kept.display()),
    }

    Ok(())
}

fn changed_at(thing: &Path) -> Result<String, Never> {
    let about = match thing.metadata() {
        Ok(about) => about,
        Err(_unreadable) => return Ok(String::new()),
    };

    let when = match about.modified() {
        Ok(when) => when,
        Err(_unstamped) => return Ok(String::new()),
    };

    let since = match when.duration_since(std::time::UNIX_EPOCH) {
        Ok(since) => since,
        Err(_the_clock_is_before_the_epoch) => return Ok(String::new()),
    };

    Ok(since.as_secs().to_string())
}
