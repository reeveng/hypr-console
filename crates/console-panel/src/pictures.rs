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
//! work was new: the same sixty icons, at the same size, every time anyone
//! opened the menu.
//!
//! So they are decoded once, into one file, at the size a row draws them. An
//! opening reads that file once and hands out slices of it. No format is worked
//! out, nothing is scaled, and sixty opens become one.
//!
//! ## The size is half of what a picture is called
//!
//! A row draws an icon at the size a row is, and the home screen draws the
//! same icon at a third of the width of the screen. One store holding one size
//! answers the second of those with the first one's picture scaled up, which is
//! the blurred square this was written to avoid -- and a second store beside it
//! is two files, two makers and two stale answers. So what a picture is called
//! here is the file it was made from and the side it was made at, and a size
//! nobody has asked for yet is simply a picture that is not in the store. The
//! whole of that is [`keyed`], and it is why the store carries a version: the
//! first one's names were paths, and a path is now half a name.
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
//! ## Who reads it
//!
//! Two drawings now: the panel on GTK, which wants a texture, and the surface
//! this desktop draws itself, which wants the bytes. `ready` and `pixels` are
//! the same lookup ending in the two answers, and neither copies more than the
//! one picture asked for. What a surface must never do is decode: the store is
//! a file read once, and a picture that is not in it is asked for from the
//! maker and drawn on the next frame rather than in the middle of this one.
//!
//! ## Read again when it has been written again
//!
//! The store used to be read once per process and that was true for as long as
//! a panel was a process: it came up, read what was there, drew and went away.
//! `console-panels` holds every panel now, so the process that reads the store
//! is the one that was already running when the maker wrote it -- a menu opened
//! for the first time asks for sixty pictures, and the opening after it drew
//! none of them, because what the first opening had cached was the absence.
//! So what is held is the file's own stamp beside the bytes, and a store
//! written since is read again. Nothing above here knows the file was read
//! twice.
//!
//! It is a cache, under `~/.cache/console`, and it says what it was made from:
//! a picture is found by the path it was made out of, and the store is rebuilt
//! when the list it was made for is newer than it is. A picture that is not in
//! it is opened the old way, so a store that is missing, stale in part, or
//! written by an older version of this is slower and never wrong.


use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_places::Base;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use console_core_shapes::Pixels;


pub const MAGIC: &[u8] = b"panel-pictures 2\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Side(pub u32);

pub fn keyed(of: &str, side: Side) -> Result<String, Never> {
    Ok(format!("{} {of}", side.0))
}

pub fn unkeyed(key: &str) -> Result<Option<(&str, Side)>, Never> {
    let (side, of) = match key.split_once(' ') {
        Some((side, of)) => (side, of),
        None => return Ok(None),
    };

    Ok(match side.parse() {
        Ok(side) => Some((of, Side(side))),
        Err(_not_a_size) => None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub of: String,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixels: Vec<u8>,
}

pub const SIDE: &str = "--side";

pub fn store() -> Result<Option<PathBuf>, Never> {
    let ours = Base::Cache.ours()?;

    Ok(ours.map(|ours| ours.join("pictures")))
}

pub fn written(pictures: &[Picture]) -> Result<Vec<u8>, Never> {
    let mut head: Vec<u8> = Vec::new();
    let mut body: Vec<u8> = Vec::new();
    let Ok(many) = fitted::<_, u32>(pictures.len());
    head.extend_from_slice(MAGIC);
    head.extend_from_slice(&many.to_le_bytes());

    for picture in pictures {
        let name = picture.of.as_bytes();
        let Ok(called) = fitted::<_, u32>(name.len());
        head.extend_from_slice(&called.to_le_bytes());
        head.extend_from_slice(name);

        let Ok(at) = fitted(body.len());
        let Ok(long) = fitted(picture.pixels.len());

        for number in [picture.width, picture.height, picture.stride, at, long] {
            head.extend_from_slice(&number.to_le_bytes());
        }

        body.extend_from_slice(&picture.pixels);
    }

    head.extend_from_slice(&body);

    Ok(head)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Where {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub at: u64,
    pub long: u64,
}

impl Where {
    pub fn in_store<'a>(&self, bytes: &'a [u8]) -> Result<Option<&'a [u8]>, Never> {
        let Ok(from) = index(self.at);
        let Ok(end) = index(self.at.saturating_add(self.long));

        Ok(bytes.get(from..end))
    }
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

        let Ok(long) = index(long);

        let (name, after) = match after.split_at_checked(long) {
            Some(split) => split,
            None => return Ok(None),
        };

        let of = match String::from_utf8(name.to_vec()) {
            Ok(of) => of,
            Err(_not_text) => return Ok(None),
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

        let (length, after) = match number(after) {
            Ok(Some((length, after))) => (length, after),
            Ok(None) | Err(_) => return Ok(None),
        };

        entries.push((of, wide, tall, stride, at, length));
        rest = after;
    }

    let Ok(stored) = fitted::<_, u64>(bytes.len());
    let Ok(after_the_list) = fitted::<_, u64>(rest.len());
    let began = stored.saturating_sub(after_the_list);

    for (of, wide, tall, stride, at, long) in entries {
        let long = u64::from(long);

        let at = match began.checked_add(u64::from(at)) {
            Some(at) => at,
            None => return Ok(None),
        };

        let end = match at.checked_add(long) {
            Some(end) => end,
            None => return Ok(None),
        };

        match end > stored || long == 0 {
            true => return Ok(None),
            false => {},
        }

        let rows = match u64::from(stride).checked_mul(u64::from(tall.saturating_sub(1))) {
            Some(rows) => rows,
            None => return Ok(None),
        };

        let last = match u64::from(wide).checked_mul(4) {
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

        held.insert(of, Where { width: wide, height: tall, stride, at, long });
    }

    Ok(Some(held))
}

type Cache = (Vec<u8>, BTreeMap<String, Where>);

fn held() -> Result<Option<Arc<Cache>>, Never> {
    #[cfg_attr(
        dylint_lib = "explicit044_no_ambient_value",
        allow(
            explicit044_no_ambient_value,
            reason = "one file, read for as long as the process draws, and the process that draws is now the one holding every panel: what it read at the first opening has to be given up when the maker writes a newer store, so the holder is a lock beside the stamp it was read at and nothing above it knows the file was read twice"
        )
    )]
    static HELD: Mutex<Option<(SystemTime, Arc<Cache>)>> = Mutex::new(None);

    let Ok(store) = store();

    let store = match store {
        Some(store) => store,
        None => return Ok(None),
    };

    let written = match std::fs::metadata(&store).and_then(|of| of.modified()) {
        Ok(written) => written,
        Err(_nothing_has_been_made_yet) => return Ok(None),
    };

    let mut holding = match HELD.lock() {
        Ok(holding) => holding,
        Err(_a_reader_gave_up_holding_it) => return Ok(None),
    };

    match holding.as_ref() {
        Some((read_at, cache)) => match *read_at == written {
            true => return Ok(Some(Arc::clone(cache))),
            false => {},
        },
        None => {},
    }

    let bytes = match std::fs::read(&store) {
        Ok(bytes) => bytes,
        Err(_gone_between_the_stamp_and_the_read) => return Ok(None),
    };

    let index = match read(&bytes) {
        Ok(Some(index)) => index,
        Ok(None) | Err(_) => return Ok(None),
    };

    let cache = Arc::new((bytes, index));

    *holding = Some((written, Arc::clone(&cache)));

    Ok(Some(cache))
}

pub fn a_row() -> Result<Side, Never> {
    let Ok(side) = fitted::<i32, u32>(crate::strip::PICTURE);

    Ok(Side(side))
}

pub fn pixels(of: &Path) -> Result<Option<Pixels>, Never> {
    let Ok(side) = a_row();

    pixels_at(of, side)
}

pub fn missing(wanted: &[String]) -> Result<Vec<String>, Never> {
    let Ok(side) = a_row();

    missing_at(wanted, side)
}

pub fn make(wanted: &[String]) -> Result<(), Never> {
    let Ok(side) = a_row();

    make_at(wanted, side)
}

pub fn pixels_at(of: &Path, side: Side) -> Result<Option<Pixels>, Never> {
    let held = match held() {
        Ok(Some(held)) => held,
        Ok(None) | Err(_) => return Ok(None),
    };
    let (bytes, index) = (&held.0, &held.1);

    let named = match of.to_str() {
        Some(named) => named,
        None => return Ok(None),
    };

    let Ok(named) = keyed(named, side);

    let found = match index.get(&named) {
        Some(found) => found,
        None => return Ok(None),
    };

    let Ok(held) = found.in_store(bytes);

    let held = match held {
        Some(held) => held,
        None => return Ok(None),
    };

    Ok(Some(Pixels {
        width: found.width,
        height: found.height,
        stride: found.stride,
        bytes: Arc::new(held.to_vec()),
    }))
}

pub fn missing_at(wanted: &[String], side: Side) -> Result<Vec<String>, Never> {
    let Ok(held) = held();

    let index = held.as_ref().map(|held| &held.1);
    let mut short: Vec<String> = Vec::new();

    for of in wanted {
        let Ok(named) = keyed(of, side);

        match index.is_some_and(|index| index.contains_key(&named)) {
            true => {},
            false => short.push(of.clone()),
        }
    }

    Ok(short)
}

pub fn make_at(wanted: &[String], side: Side) -> Result<(), Never> {
    #[cfg_attr(
        dylint_lib = "explicit044_no_ambient_value",
        allow(
            explicit044_no_ambient_value,
            reason = "what this process has already asked the maker for, so a second row wanting the same picture does not start a second maker; the rows are built in several places with nothing between them, which is the argument `opening`'s head makes about the same panel"
        )
    )]
    static ASKED: std::sync::Mutex<Option<std::collections::BTreeSet<String>>> =
        std::sync::Mutex::new(None);

    let mut asked = match ASKED.lock() {
        Ok(asked) => asked,
        Err(_the_lock_is_poisoned) => return Ok(()),
    };

    let asked = asked.get_or_insert_with(std::collections::BTreeSet::new);
    let mut fresh: Vec<String> = Vec::new();

    for of in wanted {
        let Ok(named) = keyed(of, side);

        match asked.insert(named) {
            true => fresh.push(of.clone()),
            false => {},
        }
    }

    let wanted = fresh.as_slice();

    match wanted.is_empty() {
        true => return Ok(()),
        false => {},
    }

    let Ok(mut drawing) = InternalProgram::PanelPictures.command();
    drawing
        .args([SIDE, &side.0.to_string()])
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
            width: side,
            height: side,
            stride: side * 4,
            pixels: vec![7; (side * side * 4).try_into().unwrap()],
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
        assert_eq!((one.width, one.height, one.stride), (32, 32, 128));
        assert_eq!(one.in_store(&bytes), Ok(Some(&vec![7u8; 32 * 32 * 4][..])));
        let two = held.get("/two.png").expect("the second picture");
        assert_eq!(two.in_store(&bytes), Ok(Some(&vec![7u8; 16 * 16 * 4][..])));
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
        for (slot, byte) in bytes.iter_mut().rev().skip(32 * 32 * 4 + 4).take(4).zip(far.iter().rev()) {
            *slot = *byte;
        }

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
