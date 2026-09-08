//! Every picture a list wants, in one file, already the size it is drawn at.
//!
//! A row keeps a square at its front and most rows have something to put in it:
//! an application's icon, a photograph's thumbnail. Opening one is a file
//! opened, a format worked out and an image decoded, and the menu does sixty of
//! them between the card existing and the card being on the screen -- on the
//! loop that draws, because that is where the rows are built. Nearly all of
//! them are SVG, which is a parser and a rasteriser per row.
//!
//! It was the slowest part of opening a panel by a long way, and none of the
//! work was new: the same sixty icons, at the same size, every time anybody
//! opened the menu.
//!
//! So they are decoded once, into one file, at the size a row draws them. An
//! opening reads that file once and hands out slices of it. No format is worked
//! out, nothing is scaled, and sixty opens become one.
//!
//! ## Why raw pixels and not sixty small PNGs
//!
//! Sixty PNGs is sixty opens and sixty decodes, which is the same shape as the
//! fault, cheaper. What is wanted here is one read: the pictures are small at
//! this size -- a square of thirty-two is four kilobytes -- and the whole store
//! is smaller than the icon index the menu already reads.
//!
//! ## What keeps it honest
//!
//! It is a cache, under `~/.cache/console`, and it says what it was made from:
//! a picture is found by the path it was made out of, and the store is rebuilt
//! when the list it was made for is newer than it is. A picture that is not in
//! it is opened the old way, so a store that is missing, stale in part, or
//! written by an older version of this is slower and never wrong.


use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_places::Base;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;

pub const MAGIC: &[u8] = b"panel-pictures 1\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub of: String,
    pub wide: u32,
    pub tall: u32,
    pub stride: u32,
    pub pixels: Vec<u8>,
}

pub fn store() -> Result<Option<PathBuf>, Never> {
    let ours = Base::Cache.ours()?;

    Ok(ours.map(|ours| ours.join("pictures")))
}

pub fn written(pictures: &[Picture]) -> Result<Vec<u8>, Never> {
    let mut head: Vec<u8> = Vec::new();
    let mut body: Vec<u8> = Vec::new();
    let Ok(many) = fitted::<usize, u32>(pictures.len());
    head.extend_from_slice(MAGIC);
    head.extend_from_slice(&many.to_le_bytes());

    for picture in pictures {
        let name = picture.of.as_bytes();
        let Ok(called) = fitted::<usize, u32>(name.len());
        head.extend_from_slice(&called.to_le_bytes());
        head.extend_from_slice(name);

        let Ok(at) = fitted(body.len());
        let Ok(long) = fitted(picture.pixels.len());

        for number in [picture.wide, picture.tall, picture.stride, at, long] {
            head.extend_from_slice(&number.to_le_bytes());
        }

        body.extend_from_slice(&picture.pixels);
    }

    head.extend_from_slice(&body);

    Ok(head)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Where {
    pub wide: u32,
    pub tall: u32,
    pub stride: u32,
    pub at: usize,
    pub long: usize,
}

pub fn read(bytes: &[u8]) -> Result<Option<BTreeMap<String, Where>>, Never> {
    let after = match bytes.strip_prefix(MAGIC) {
        Some(after) => after,
        None => return Ok(None),
    };

    let (count, mut rest) = match number(after) {
        Ok(Some((count, rest))) => (count, rest),
        Ok(None) | Err(_) => return Ok(None),
    };

    let mut held: BTreeMap<String, Where> = BTreeMap::new();
    let mut entries = Vec::new();

    for _ in 0..count {
        let (long, after) = match number(rest) {
            Ok(Some((long, after))) => (long, after),
            Ok(None) | Err(_) => return Ok(None),
        };

        let Ok(long) = fitted::<u32, usize>(long);

        match after.len() < long {
            true => return Ok(None),
            false => {},
        }

        let (name, after) = after.split_at(long);

        let of = match String::from_utf8(name.to_vec()) {
            Ok(of) => of,
            Err(_fault) => return Ok(None),
        };

        let (wide, after) = match number(after) {
            Ok(Some((wide, after))) => (wide, after),
            Ok(None) | Err(_) => return Ok(None),
        };

        let (tall, after) = match number(after) {
            Ok(Some((tall, after))) => (tall, after),
            Ok(None) | Err(_) => return Ok(None),
        };

        let (stride, after) = match number(after) {
            Ok(Some((stride, after))) => (stride, after),
            Ok(None) | Err(_) => return Ok(None),
        };

        let (at, after) = match number(after) {
            Ok(Some((at, after))) => (at, after),
            Ok(None) | Err(_) => return Ok(None),
        };

        let (len, after) = match number(after) {
            Ok(Some((len, after))) => (len, after),
            Ok(None) | Err(_) => return Ok(None),
        };

        let Ok(at) = fitted::<u32, usize>(at);
        let Ok(len) = fitted::<u32, usize>(len);

        entries.push((of, wide, tall, stride, at, len));
        rest = after;
    }

    let began = match bytes.len().checked_sub(rest.len()) {
        Some(began) => began,
        None => return Ok(None),
    };

    for (of, wide, tall, stride, at, long) in entries {
        let at = match began.checked_add(at) {
            Some(at) => at,
            None => return Ok(None),
        };

        let end = match at.checked_add(long) {
            Some(end) => end,
            None => return Ok(None),
        };

        match end > bytes.len() || long == 0 {
            true => return Ok(None),
            false => {},
        }

        let Ok(step) = fitted::<u32, usize>(stride);
        let Ok(down) = fitted::<u32, usize>(tall.saturating_sub(1));

        let rows = match step.checked_mul(down) {
            Some(rows) => rows,
            None => return Ok(None),
        };

        let Ok(across) = fitted::<u32, usize>(wide);

        let last = match across.checked_mul(4) {
            Some(last) => last,
            None => return Ok(None),
        };

        let whole = match rows.checked_add(last) {
            Some(whole) => whole,
            None => return Ok(None),
        };

        match whole > long {
            true => return Ok(None),
            false => {},
        }

        held.insert(of, Where { wide, tall, stride, at, long });
    }

    Ok(Some(held))
}

type Held = (Vec<u8>, BTreeMap<String, Where>);

fn held() -> Result<Option<&'static Held>, Never> {
    static HELD: OnceLock<Option<Held>> = OnceLock::new();

    Ok(HELD
        .get_or_init(|| {
            let Ok(store) = store();

            let store = match store {
                Some(store) => store,
                None => return None,
            };

            let bytes = match std::fs::read(store) {
                Ok(bytes) => bytes,
                Err(_fault) => return None,
            };

            let index = match read(&bytes) {
                Ok(Some(index)) => index,
                Ok(None) | Err(_) => return None,
            };

            Some((bytes, index))
        })
        .as_ref())
}

pub fn ready(of: &Path) -> Result<Option<gdk::Texture>, Never> {
    let (bytes, index) = match held() {
        Ok(Some((bytes, index))) => (bytes, index),
        Ok(None) | Err(_) => return Ok(None),
    };

    let named = match of.to_str() {
        Some(named) => named,
        None => return Ok(None),
    };

    let found = match index.get(named) {
        Some(found) => found,
        None => return Ok(None),
    };

    let pixels = bytes.get(found.at..found.at.saturating_add(found.long));

    let pixels = match pixels {
        Some(pixels) => pixels,
        None => return Ok(None),
    };

    let Ok(wide) = fitted(found.wide);
    let Ok(tall) = fitted(found.tall);
    let Ok(stride) = fitted(found.stride);

    let texture = gdk::MemoryTexture::new(
        wide,
        tall,
        gdk::MemoryFormat::R8g8b8a8,
        &glib::Bytes::from(pixels),
        stride,
    );

    Ok(Some(texture.upcast()))
}

pub fn missing(wanted: &[String]) -> Result<Vec<String>, Never> {
    let Ok(held) = held();

    let index = held.map(|(_, index)| index);

    Ok(wanted
        .iter()
        .filter(|of| !index.is_some_and(|index| index.contains_key(*of)))
        .cloned()
        .collect())
}

pub fn make(wanted: &[String]) -> Result<(), Never> {
    static ASKED: std::sync::Mutex<Option<std::collections::BTreeSet<String>>> =
        std::sync::Mutex::new(None);

    let mut asked = match ASKED.lock() {
        Ok(asked) => asked,
        Err(_fault) => return Ok(()),
    };

    let asked = asked.get_or_insert_with(std::collections::BTreeSet::new);
    let wanted: Vec<String> =
        wanted.iter().filter(|of| asked.insert((*of).clone())).cloned().collect();
    let wanted = wanted.as_slice();

    match wanted.is_empty() {
        true => return Ok(()),
        false => {},
    }

    let mut drawing = std::process::Command::new("panel-pictures");
    drawing
        .args(wanted)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let Ok(()) = console_response_times::not_a_press(&mut drawing);

    let started = console_program_lifetime::let_go(&mut drawing);

    match started {
        Ok(drawing) => {
            let Ok(()) = crate::running::kept(drawing);
        },
        Err(fault) => {
            eprintln!("no pictures made: {fault}");
        }
    }

    Ok(())
}

fn number(bytes: &[u8]) -> Result<Option<(u32, &[u8])>, Never> {
    let (four, rest) = match bytes.split_at_checked(4) {
        Some((four, rest)) => (four, rest),
        None => return Ok(None),
    };

    let four: [u8; 4] = match four.try_into() {
        Ok(four) => four,
        Err(_not_four_bytes) => return Ok(None),
    };

    Ok(Some((u32::from_le_bytes(four), rest)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_picture(of: &str, side: u32) -> Picture {
        Picture {
            of: of.to_string(),
            wide: side,
            tall: side,
            stride: side * 4,
            pixels: vec![7; (side * side * 4) as usize],
        }
    }

    #[test]
    fn a_store_says_where_each_picture_is_and_how_big() {
        let pictures = vec![a_picture("/usr/share/icons/one.svg", 32), a_picture("/two.png", 16)];
        let Ok(bytes) = written(&pictures);
        let held = match read(&bytes) {
            Ok(Some(held)) => held,
            Ok(None) | Err(_) => panic!("a written store reads back"),
        };

        assert_eq!(held.len(), 2);
        let one = held.get("/usr/share/icons/one.svg").expect("the first picture");
        assert_eq!((one.wide, one.tall, one.stride), (32, 32, 128));
        assert_eq!(&bytes[one.at..one.at + one.long], &vec![7u8; 32 * 32 * 4]);
        let two = held.get("/two.png").expect("the second picture");
        assert_eq!(&bytes[two.at..two.at + two.long], &vec![7u8; 16 * 16 * 4]);
    }

    #[test]
    fn half_a_store_is_no_store_rather_than_half_the_pictures() {
        let Ok(bytes) = written(&[a_picture("/one.svg", 32)]);

        for cut in [0, 4, MAGIC.len(), MAGIC.len() + 4, bytes.len() - 1] {
            assert_eq!(read(&bytes[..cut]), Ok(None), "a store cut at {cut} was read as a store");
        }
    }

    #[test]
    fn a_file_that_is_not_this_file_is_not_read_at_all() {
        assert_eq!(read(b"not this at all"), Ok(None));
        assert_eq!(read(b"panel-pictures 2\n"), Ok(None));
    }

    #[test]
    fn a_picture_that_points_outside_the_store_is_refused() {
        let Ok(mut bytes) = written(&[a_picture("/one.svg", 32)]);
        let far = (1_000_000u32).to_le_bytes();
        let at = bytes.len() - 32 * 32 * 4 - 8;
        bytes[at..at + 4].copy_from_slice(&far);

        assert_eq!(read(&bytes), Ok(None));
    }

    #[test]
    fn a_picture_whose_rows_do_not_fit_its_own_pixels_is_refused() {
        let mut picture = a_picture("/one.svg", 32);
        picture.stride = 4096;
        let Ok(bytes) = written(&[picture]);

        assert_eq!(read(&bytes), Ok(None));
    }

    #[test]
    fn an_empty_store_is_a_store_with_nothing_in_it() {
        let Ok(bytes) = written(&[]);

        let held = match read(&bytes) {
            Ok(Some(held)) => held,
            Ok(None) | Err(_) => panic!("an empty store is still a store"),
        };

        assert!(held.is_empty());
    }
}
