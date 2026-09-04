//! Decode the pictures a list wants, once, into the one file it reads.
//!
//!     panel-pictures /usr/share/icons/.../firefox.svg /usr/share/pixmaps/x.png
//!
//! Off the panel, like `files-thumbs` and for the same reason: this is the work
//! that was making the menu slow to appear, and doing it where the panel draws
//! is doing it in the one place where nothing else can happen. The panel that
//! asks for it is already on the screen and goes on answering buttons; what
//! this is for is the next opening.
//!
//! What is asked for is decoded again whether or not the store already has it,
//! and everything else in the store whose file still exists is kept. So a
//! package that adds an application costs one rebuild of the pictures that
//! application's list wanted, and one that removes it leaves nothing behind.
//!
//! `console_panel::pictures` is the file's shape, who reads it and why.


use console_number::fitted;
use std::collections::BTreeMap;
use std::process::ExitCode;

use gtk4::gdk_pixbuf::{Colorspace, Pixbuf};

use console_panel::pictures::{self, Picture};
use console_panel::strip::PICTURE;

fn main() -> ExitCode {
    let wanted: Vec<String> = std::env::args().skip(1).collect();

    if wanted.is_empty() {
        eprintln!("usage: panel-pictures FILE...");
        return ExitCode::FAILURE;
    }

    let mut made: BTreeMap<String, Picture> = kept();

    for of in wanted {
        match drawn(&of) {
            Some(picture) => {
                made.insert(of, picture);
            },
            // A picture nothing can be made of is left out rather than written
            // as nothing: the row falls back to opening the file itself, which
            // is where an icon that is really a broken symlink belongs.
            None => {
                made.remove(&of);
            },
        }
    }

    let pictures: Vec<Picture> = made.into_values().collect();

    match written(&pictures) {
        Written::Yes => ExitCode::SUCCESS,
        Written::No => ExitCode::FAILURE,
    }
}

/// What the store already holds, minus anything whose file has gone.
///
/// Kept rather than thrown away, because two lists share one store: the menu
/// asks for its applications and the files ask for their thumbnails, and a
/// store written from one of those alone would take the other's pictures away
/// every time it was made.
fn kept() -> BTreeMap<String, Picture> {
    let Ok(bytes) = std::fs::read(pictures::store()) else { return BTreeMap::new() };

    let Some(index) = pictures::read(&bytes) else { return BTreeMap::new() };

    index
        .into_iter()
        .filter(|(of, _)| std::path::Path::new(of).exists())
        .filter_map(|(of, found)| {
            let pixels = bytes.get(found.at..found.at + found.long)?.to_vec();
            Some((
                of.clone(),
                Picture { of, wide: found.wide, tall: found.tall, stride: found.stride, pixels },
            ))
        })
        .collect()
}

/// One picture, at the size a row draws it.
///
/// Scaled here rather than at drawing time, which is the whole point: every
/// row on this device draws its picture in the same square, and working out
/// what an icon looks like in that square is the expensive half of an SVG.
fn drawn(of: &str) -> Option<Picture> {
    let Ok(held) = Pixbuf::from_file_at_scale(of, PICTURE, PICTURE, true) else { return None };

    // Four channels always, so the store is one layout and the reader hands
    // GDK the format it was told to expect. Most icons have an alpha channel
    // already; the few that do not are the photographs.
    let held = match held.has_alpha() {
        true => held,

        false => {
            let Ok(held) = held.add_alpha(false, 0, 0, 0) else { return None };

            held
        },
    };

    if held.colorspace() != Colorspace::Rgb || held.n_channels() != 4 || held.bits_per_sample() != 8
    {
        return None;
    }

    Some(Picture {
        of: of.to_string(),
        wide: fitted(held.width()),
        tall: fitted(held.height()),
        stride: fitted(held.rowstride()),
        pixels: held.read_pixel_bytes().to_vec(),
    })
}

/// Put the store down whole, or not at all.
///
/// Written beside itself and renamed over, because every panel on the machine
/// reads this file before it draws and a rename is the only way to change it
/// that no reader can arrive in the middle of.
/// Whether the store was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Written {
    /// It is on disk, whole, under its own name.
    Yes,
    /// Something in the way of writing it failed.
    No,
}

fn written(pictures: &[Picture]) -> Written {
    let at = pictures::store();

    let Some(above) = at.parent() else { return Written::No };

    if let Err(fault) = std::fs::create_dir_all(above) {
        eprintln!("panel-pictures: {}: {fault}", above.display());

        return Written::No;
    }

    let beside = at.with_extension("new");

    if let Err(fault) = std::fs::write(&beside, pictures::written(pictures)) {
        eprintln!("panel-pictures: {}: {fault}", beside.display());

        return Written::No;
    }

    match std::fs::rename(&beside, &at) {
        Ok(()) => Written::Yes,

        Err(fault) => {
            eprintln!("panel-pictures: {}: {fault}", at.display());
            Written::No
        }
    }
}
