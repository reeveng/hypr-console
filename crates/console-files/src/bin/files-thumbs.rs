//! Make the pictures one folder's listing wants, and stop.
//!
//! Off the panel and not in it. A folder of two hundred photographs takes
//! seconds to work through, and a panel that did it where it draws would show
//! nothing at all until it was done. So the listing draws with whatever the
//! store already has, this runs behind it, and the panel draws again when it
//! ends.
//!
//! Once per thing, ever. What is made goes into the store every desktop shares,
//! so the second visit to a folder is the listing and the pictures together.

use std::path::{Path, PathBuf};

use console_content_types::Table;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_places::Base;
use console_files::listing::{Entry, Worth};
use console_files::thumbs::{self, SIDE};

const INTO_IT: &str = "3";

const THE_START: &str = "0";

fn main() {
    let folder = match std::env::args().nth(1) {
        Some(folder) => folder,
        None => {
            eprintln!("which folder");
            return;
        }
    };

    let Ok(cache) = Base::Cache.hers();

    let cache = match cache {
        Some(cache) => cache,
        None => {
            eprintln!("files-thumbs: no home, so there is nowhere to keep a picture");

            return;
        }
    };

    let Ok(store) = thumbs::store(&cache);

    match std::fs::create_dir_all(&store) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!(
                "files-thumbs: {}: making the store the pictures go in: {fault}",
                store.display()
            );

            return;
        }
    }

    let Ok(wanting) = wanting(Path::new(&folder), &store);

    for (thing, kind) in wanting {
        let Ok(()) = made(&thing, &kind, &store);
    }
}

fn wanting(folder: &Path, store: &Path) -> Result<Vec<(PathBuf, String)>, Never> {
    let kinds = match Table::here() {
        Ok(kinds) => kinds,
        Err(why) => {
            eprintln!("files-thumbs: nothing says what a file is: {why}");

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
        let found = thumbs::found(store, &path)?;

        match worth == Worth::APicture && found.is_none() {
            true => wanting.push((path, entry.kind)),
            false => {},
        }
    }

    Ok(wanting)
}

fn made(thing: &Path, kind: &str, store: &Path) -> Result<(), Never> {
    let address = thumbs::address(thing)?;

    let address = match address {
        Some(address) => address,
        None => return Ok(()),
    };

    let kept = thumbs::of(store, &address)?;

    let kept = match kept {
        Some(kept) => kept,
        None => return Ok(()),
    };

    let part = kept.with_extension("part.png");

    let into_it: &[Option<&str>] = match kind.starts_with("video/") {
        true => &[Some(INTO_IT), Some(THE_START)],
        false => &[None],
    };

    let drawn = drawn(thing, &part, into_it)?;

    match drawn {
        Made::APicture => {
            let Ok(changed) = changed_at(thing);
            let Ok(stamped) = stamped(&part, thumbs::Stamp { address: &address, changed: &changed });

            match stamped {
                Made::APicture => {
                    let _ = std::fs::rename(&part, &kept);
                }
                Made::None => {
                    let _ = std::fs::remove_file(&part);
                }
            }
        }
        Made::None => {
            let _ = std::fs::remove_file(&part);
        }
    }

    Ok(())
}

fn changed_at(thing: &Path) -> Result<String, Never> {
    let about = match thing.metadata() {
        Ok(about) => about,
        Err(_fault) => return Ok(String::new()),
    };

    let when = match about.modified() {
        Ok(when) => when,
        Err(_fault) => return Ok(String::new()),
    };

    let since = match when.duration_since(std::time::UNIX_EPOCH) {
        Ok(since) => since,
        Err(_fault) => return Ok(String::new()),
    };

    Ok(since.as_secs().to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Made {
    APicture,
    None,
}

#[cfg_attr(
    dylint_lib = "explicit029_no_asking_per_item",
    allow(
        explicit029_no_asking_per_item,
        reason = "the list is where in a film to look, and each try runs only when the one before it drew nothing: ffmpeg cannot be asked for a second seek when the first lands past the end"
    )
)]
fn drawn(thing: &Path, part: &Path, into_it: &[Option<&str>]) -> Result<Made, Never> {
    for at in into_it {
        let Ok(mut asking) = Program::Ffmpeg.command();

        asking.args(["-loglevel", "error", "-y"]);

        match at {
            Some(at) => {
                asking.args(["-ss", at]);
            }
            None => {},
        }

        let done = asking
            .arg("-i")
            .arg(thing)
            .args(["-frames:v", "1", "-vf", &format!("scale={SIDE}:{SIDE}:force_original_aspect_ratio=decrease")])
            .arg(part)
            .status();

        match done.is_ok_and(|how| how.success()) && part.exists() {
            true => return Ok(Made::APicture),
            false => {},
        }
    }

    Ok(Made::None)
}

fn stamped(part: &Path, stamp: thumbs::Stamp<'_>) -> Result<Made, Never> {
    let png = match std::fs::read(part) {
        Ok(png) => png,
        Err(fault) => {
            eprintln!("files-thumbs: {}: {fault}", part.display());

            return Ok(Made::None);
        }
    };

    let said = thumbs::stamped(&png, stamp)?;

    let said = match said {
        Some(said) => said,
        None => {
            eprintln!("files-thumbs: {}: this is not the PNG ffmpeg was asked for", part.display());

            return Ok(Made::None);
        }
    };

    Ok(match console_core_atomic_writes::whole(part, &said) {
        Ok(()) => Made::APicture,

        Err(fault) => {
            eprintln!("files-thumbs: {}: writing the picture: {fault}", part.display());

            Made::None
        }
    })
}
