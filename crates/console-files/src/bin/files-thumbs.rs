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
use std::process::Command;

use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use console_files::listing::{Entry, Worth};
use console_files::thumbs::{self, SIDE};

/// Where in a film to take the frame from.
///
/// Not the first one. A film that opens on a black frame or a title card gets a
/// picture that says nothing about it, and most of them open on one or the
/// other. A film shorter than this has no such second and is taken from the
/// beginning instead.
const INTO_IT: &str = "3";

fn main() {
    let Some(folder) = std::env::args().nth(1) else {
        eprintln!("which folder");
        return;
    };

    let Some(cache) = glib::user_cache_dir().into() else { return };

    let store = thumbs::store(&cache);

    if let Err(fault) = std::fs::create_dir_all(&store) {
        eprintln!("files-thumbs: {}: making the store the pictures go in: {fault}", store.display());

        return;
    }

    for (thing, kind) in wanting(Path::new(&folder), &store) {
        made(&thing, &kind, &store);
    }
}

/// The things in a folder that are worth a picture and have not got one, each
/// with what the machine says it is.
///
/// The kind is carried rather than worked out again where it is used. Asked
/// twice, the second answer can be the other one: the listing goes by the name
/// and a second look reads the file, and a photograph saved under the wrong
/// ending would be drawn by one rule and skipped by the other.
fn wanting(folder: &Path, store: &Path) -> Vec<(PathBuf, String)> {
    let asked = gio::File::for_path(folder).enumerate_children(
        "standard::name,standard::type,standard::fast-content-type",
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    );

    let Ok(children) = asked else { return Vec::new() };

    children
        .flatten()
        .map(|about| {
            let entry = Entry {
                folder: about.file_type() == gio::FileType::Directory,
                kind: about
                    .attribute_string("standard::fast-content-type")
                    .map(|kind| kind.to_string())
                    .unwrap_or_default(),
                name: String::new(),
                size: 0,
            };
            (folder.join(about.name()), entry)
        })
        .filter(|(path, entry)| {
            entry.worth_a_picture() == Worth::APicture && thumbs::found(store, path).is_none()
        })
        .map(|(path, entry)| (path, entry.kind))
        .collect()
}

/// One picture, made and put where the store keeps it.
///
/// Written beside itself and moved into place, because a panel reading the
/// store while this runs would otherwise find a file that is half a picture and
/// draw the mark GTK has for a broken one.
fn made(thing: &Path, kind: &str, store: &Path) {
    let Some(address) = thumbs::address(thing) else { return };

    let Some(kept) = thumbs::of(store, &address) else { return };

    // Ending in .png, not merely beginning with it. ffmpeg reads the format
    // to write out of the name it is given, and a file called .png.part is one
    // it refuses rather than guesses at.
    let part = kept.with_extension("part.png");
    let drawn = match kind.starts_with("video/") {
        true => from_a_film(thing, &part),
        false => from_a_photograph(thing, &part, &address),
    };

    match drawn {
        Made::APicture => {
            let _ = std::fs::rename(&part, &kept);
        }
        Made::Nothing => {
            let _ = std::fs::remove_file(&part);
        }
    }
}

/// A photograph, read and scaled down.
///
/// The two notes the store expects are written into the picture as it is saved,
/// so anything else reading the store can tell what it is of and whether that
/// thing has changed since.
fn from_a_photograph(thing: &Path, part: &Path, address: &str) -> Made {
    let Ok(picture) = Pixbuf::from_file_at_scale(thing, SIDE, SIDE, true) else {
        return Made::Nothing;
    };

    let changed = changed_at(thing);

    match picture.savev(part, "png", &[("tEXt::Thumb::URI", address), ("tEXt::Thumb::MTime", &changed)])
    {
        Ok(()) => Made::APicture,

        Err(fault) => {
            eprintln!("files-thumbs: {}: writing the picture: {fault}", part.display());
            Made::Nothing
        }
    }
}

/// When the thing last changed, in seconds, or nothing where it will not say.
///
/// Nothing rather than a guess at three separate points: a thing whose
/// metadata will not open, a clock that will not give a moment, and a moment
/// before the epoch. A picture stamped with nothing is one that gets made
/// again next time rather than one that is wrong, so none of the three is
/// worth a sentence in a run that walks a whole folder.
fn changed_at(thing: &Path) -> String {
    let Ok(about) = thing.metadata() else { return String::new() };

    let Ok(when) = about.modified() else { return String::new() };

    let Ok(since) = when.duration_since(std::time::UNIX_EPOCH) else { return String::new() };

    since.as_secs().to_string()
}

/// A film, by one frame out of it.
///
/// ffmpeg, because nothing here decodes video and the wallpapers already have
/// it on the machine for the same reason. A film shorter than the moment asked
/// for gives nothing back, so it is asked again from the beginning.
/// Whether a picture came out of the attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Made {
    /// One was written, and the store has it.
    APicture,
    /// Nothing was, and the thing is drawn by its name.
    Nothing,
}

fn from_a_film(thing: &Path, part: &Path) -> Made {
    for at in [INTO_IT, "0"] {
        let done = Command::new("ffmpeg")
            .args(["-loglevel", "error", "-y", "-ss", at, "-i"])
            .arg(thing)
            .args(["-frames:v", "1", "-vf", &format!("scale={SIDE}:{SIDE}:force_original_aspect_ratio=decrease")])
            .arg(part)
            .status();

        if done.is_ok_and(|how| how.success()) && part.exists() {
            return Made::APicture;
        }
    }

    Made::Nothing
}
