//! Two makers at once both land in the store.
//!
//! Every list that wants a picture starts `panel-pictures` for it, and each
//! one writes the whole store: the books asking for their covers and the menu
//! asking for its icons are two makers that know nothing about each other. A
//! maker that read the store before it decoded wrote back what was there when
//! it began, and the other's pictures were gone -- for good, because a list asks
//! for a picture once. The library sat on blank covers until it was closed.

use std::path::Path;
use std::process::Command;

use console_core_geometry::Size;
use console_core_places::APPLICATION;
use console_panel::pictures::{self, Side};

type Failure = Box<dyn std::error::Error>;

const MAKER: &str = env!("CARGO_BIN_EXE_panel-pictures");

const SIDE: Side = Side(16);

const GREY: [u8; 72] = [200; 72];

fn a_picture(at: &Path) -> Result<(), Failure> {
    let size = Size { width: 4, height: 6 };
    let Ok(pixmap) = console_pictures::portable_pixmap(size, &GREY);

    console_core_atomic_writes::whole(at, &pixmap)?;

    Ok(())
}

#[test]
fn two_makers_at_once_keep_each_others_pictures() -> Result<(), Failure> {
    let room = console_core_temporary_directories::fresh("picture-store")?;
    let cache = room.join("cache");
    let one = room.join("one.ppm");
    let other = room.join("other.ppm");

    a_picture(&one)?;
    a_picture(&other)?;

    let side = SIDE.0.to_string();
    let started: Vec<_> = [&one, &other]
        .into_iter()
        .map(|of| Command::new(MAKER).args([pictures::SIDE.spelling, &side]).arg(of).env("XDG_CACHE_HOME", &cache).spawn())
        .collect::<Result<_, _>>()?;

    for mut maker in started {
        let ended = maker.wait()?;

        assert!(ended.success(), "a maker failed");
    }

    let bytes = std::fs::read(cache.join(APPLICATION).join("pictures"))?;
    let Ok(held) = pictures::read(&bytes);
    let held = held.ok_or("the store does not read")?;

    for of in [&one, &other] {
        let Ok(named) = pictures::key(&of.display().to_string(), SIDE);

        assert!(held.contains_key(&named), "{} was written over by the other maker", of.display());
    }

    std::fs::remove_dir_all(&room)?;

    Ok(())
}
