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


use console_never::Never;
use console_number_conversion::fitted;
use std::collections::BTreeMap;
use std::process::ExitCode;

use gtk4::gdk_pixbuf::{Colorspace, Pixbuf};

use console_panel::pictures::{self, Picture};
use console_panel::strip::PICTURE;

fn main() -> ExitCode {
    let wanted: Vec<String> = std::env::args().skip(1).collect();

    match wanted.is_empty() {
        true => {
            eprintln!("usage: panel-pictures FILE...");
            return ExitCode::FAILURE;
        }
        false => {},
    }

    let Ok(mut made) = kept();

    for of in wanted {
        let Ok(drawn) = drawn(&of);

        match drawn {
            Some(picture) => {
                made.insert(of, picture);
            },
            None => {
                made.remove(&of);
            },
        }
    }

    let pictures: Vec<Picture> = made.into_values().collect();

    let Ok(written) = written(&pictures);

    match written {
        Written::Yes => ExitCode::SUCCESS,
        Written::No => ExitCode::FAILURE,
    }
}

fn kept() -> Result<BTreeMap<String, Picture>, Never> {
    let Ok(store) = pictures::store();

    let Ok(bytes) = std::fs::read(store) else { return Ok(BTreeMap::new()) };

    let Ok(Some(index)) = pictures::read(&bytes) else { return Ok(BTreeMap::new()) };

    Ok(index
        .into_iter()
        .filter(|(of, _)| std::path::Path::new(of).exists())
        .filter_map(|(of, found)| {
            let held = bytes.get(found.at..found.at.saturating_add(found.long))?;
            let pixels = held.to_vec();
            Some((
                of.clone(),
                Picture { of, wide: found.wide, tall: found.tall, stride: found.stride, pixels },
            ))
        })
        .collect())
}

fn drawn(of: &str) -> Result<Option<Picture>, Never> {
    let Ok(held) = Pixbuf::from_file_at_scale(of, PICTURE, PICTURE, true) else {
        return Ok(None);
    };

    let held = match held.has_alpha() {
        true => held,

        false => {
            let Ok(held) = held.add_alpha(false, 0, 0, 0) else { return Ok(None) };

            held
        },
    };

    match held.colorspace() != Colorspace::Rgb
        || held.n_channels() != 4
        || held.bits_per_sample() != 8
    {
        true => return Ok(None),
        false => {},
    }

    let Ok(wide) = fitted(held.width());
    let Ok(tall) = fitted(held.height());
    let Ok(stride) = fitted(held.rowstride());

    Ok(Some(Picture {
        of: of.to_string(),
        wide,
        tall,
        stride,
        pixels: held.read_pixel_bytes().to_vec(),
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Written {
    Yes,
    No,
}

fn written(pictures: &[Picture]) -> Result<Written, Never> {
    let Ok(at) = pictures::store();

    let Some(above) = at.parent() else { return Ok(Written::No) };

    match std::fs::create_dir_all(above) {
        Ok(_) => {},
        Err(fault) => {
            eprintln!("panel-pictures: {}: {fault}", above.display());

            return Ok(Written::No);
        }
    }

    let beside = at.with_extension("new");
    let Ok(said) = pictures::written(pictures);

    match std::fs::write(&beside, said) {
        Ok(_) => {},
        Err(fault) => {
            eprintln!("panel-pictures: {}: {fault}", beside.display());

            return Ok(Written::No);
        }
    }

    Ok(match std::fs::rename(&beside, &at) {
        Ok(()) => Written::Yes,

        Err(fault) => {
            eprintln!("panel-pictures: {}: {fault}", at.display());

            Written::No
        }
    })
}
