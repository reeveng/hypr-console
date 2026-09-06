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

use console_external_programs::Program;
use console_files::listing::{Entry, Worth};
use console_files::thumbs::{self, SIDE};
use console_never::Never;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

const INTO_IT: &str = "3";

fn main() {
    let Some(folder) = std::env::args().nth(1) else {
        eprintln!("which folder");
        return;
    };

    let Some(cache) = glib::user_cache_dir().into() else { return };

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
    let asked = gio::File::for_path(folder).enumerate_children(
        "standard::name,standard::type,standard::fast-content-type",
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    );

    let Ok(children) = asked else { return Ok(Vec::new()) };

    let mut wanting: Vec<(PathBuf, String)> = Vec::new();

    for about in children.flatten() {
        let entry = Entry {
            folder: about.file_type() == gio::FileType::Directory,
            kind: about
                .attribute_string("standard::fast-content-type")
                .map(|kind| kind.to_string())
                .unwrap_or_default(),
            name: String::new(),
            size: 0,
        };
        let path = folder.join(about.name());
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
    let Some(address) = thumbs::address(thing)? else { return Ok(()) };

    let Some(kept) = thumbs::of(store, &address)? else { return Ok(()) };

    let part = kept.with_extension("part.png");

    let drawn = match kind.starts_with("video/") {
        true => from_a_film(thing, &part)?,
        false => from_a_photograph(thing, &part, &address)?,
    };

    match drawn {
        Made::APicture => {
            let _ = std::fs::rename(&part, &kept);
        }
        Made::Nothing => {
            let _ = std::fs::remove_file(&part);
        }
    }

    Ok(())
}

fn from_a_photograph(thing: &Path, part: &Path, address: &str) -> Result<Made, Never> {
    let Ok(picture) = Pixbuf::from_file_at_scale(thing, SIDE, SIDE, true) else {
        return Ok(Made::Nothing);
    };

    let changed = changed_at(thing)?;

    Ok(match picture.savev(part, "png", &[
        ("tEXt::Thumb::URI", address),
        ("tEXt::Thumb::MTime", &changed),
    ]) {
        Ok(()) => Made::APicture,

        Err(fault) => {
            eprintln!("files-thumbs: {}: writing the picture: {fault}", part.display());
            Made::Nothing
        }
    })
}

fn changed_at(thing: &Path) -> Result<String, Never> {
    let Ok(about) = thing.metadata() else { return Ok(String::new()) };

    let Ok(when) = about.modified() else { return Ok(String::new()) };

    let Ok(since) = when.duration_since(std::time::UNIX_EPOCH) else { return Ok(String::new()) };

    Ok(since.as_secs().to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Made {
    APicture,
    Nothing,
}

fn from_a_film(thing: &Path, part: &Path) -> Result<Made, Never> {
    for at in [INTO_IT, "0"] {
        let Ok(mut asking) = Program::Ffmpeg.command();

        let done = asking
            .args(["-loglevel", "error", "-y", "-ss", at, "-i"])
            .arg(thing)
            .args(["-frames:v", "1", "-vf", &format!("scale={SIDE}:{SIDE}:force_original_aspect_ratio=decrease")])
            .arg(part)
            .status();

        match done.is_ok_and(|how| how.success()) && part.exists() {
            true => return Ok(Made::APicture),
            false => {},
        }
    }

    Ok(Made::Nothing)
}
