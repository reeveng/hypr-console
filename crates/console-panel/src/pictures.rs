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


use console_number::fitted;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;

/// What this file is, and which version of it.
///
/// Read before anything else and matched whole. A store written by a version
/// that laid the pixels out differently is not a store to be read carefully,
/// it is a store to be ignored and made again.
pub const MAGIC: &[u8] = b"panel-pictures 1\n";

/// One picture, as it is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// The file it was made out of, which is what it is found by.
    pub of: String,
    pub wide: u32,
    pub tall: u32,
    /// Bytes per row, which is not always four times the width.
    pub stride: u32,
    /// Non-premultiplied RGBA, which is what a pixbuf gives and what GDK takes.
    pub pixels: Vec<u8>,
}

/// Where the store is.
pub fn store() -> PathBuf {
    // The fallbacks are deliberate and in this order: a session that says
    // where its cache goes, then one that only says where its home is, then a
    // unit with neither, which is the root the desktop's own services run as.
    let cache = match (std::env::var("XDG_CACHE_HOME"), std::env::var("HOME")) {
        (Ok(cache), _) if !cache.is_empty() => PathBuf::from(cache),
        (_, Ok(home)) => PathBuf::from(home).join(".cache"),
        (_, Err(_)) => PathBuf::from("/root").join(".cache"),
    };

    cache.join("console").join("pictures")
}

/// The store, written.
pub fn written(pictures: &[Picture]) -> Vec<u8> {
    let mut head: Vec<u8> = Vec::new();
    let mut body: Vec<u8> = Vec::new();
    head.extend_from_slice(MAGIC);
    head.extend_from_slice(&fitted::<usize, u32>(pictures.len()).to_le_bytes());

    for picture in pictures {
        let name = picture.of.as_bytes();
        head.extend_from_slice(&fitted::<usize, u32>(name.len()).to_le_bytes());
        head.extend_from_slice(name);

        for number in [
            picture.wide,
            picture.tall,
            picture.stride,
            fitted(body.len()),
            fitted(picture.pixels.len()),
        ] {
            head.extend_from_slice(&number.to_le_bytes());
        }

        body.extend_from_slice(&picture.pixels);
    }

    head.extend_from_slice(&body);
    head
}

/// Where one picture's pixels are in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Where {
    pub wide: u32,
    pub tall: u32,
    pub stride: u32,
    /// Into the whole file, so a reader holding the bytes can cut them out
    /// without being told where the pictures begin.
    pub at: usize,
    pub long: usize,
}

/// The store, read: what is in it and where.
///
/// Anything that is not this file is nothing rather than a failure, and so is
/// anything in it that points outside it. A cache is written by a program that
/// can be killed halfway through writing, and every panel on the machine reads
/// this one before it draws.
pub fn read(bytes: &[u8]) -> Option<BTreeMap<String, Where>> {
    let after = bytes.strip_prefix(MAGIC)?;
    let (count, mut rest) = number(after)?;
    let mut held: BTreeMap<String, Where> = BTreeMap::new();
    let mut entries = Vec::new();

    for _ in 0..count {
        let (long, after) = number(rest)?;
        let long: usize = fitted(long);

        if after.len() < long {
            return None;
        }

        let (name, after) = after.split_at(long);

        let Ok(of) = String::from_utf8(name.to_vec()) else { return None };

        let (wide, after) = number(after)?;
        let (tall, after) = number(after)?;
        let (stride, after) = number(after)?;
        let (at, after) = number(after)?;
        let (len, after) = number(after)?;
        entries.push((of, wide, tall, stride, fitted::<u32, usize>(at), fitted::<u32, usize>(len)));
        rest = after;
    }

    // Where the pixels start, which is everything the entries did not use.
    let began = bytes.len().checked_sub(rest.len())?;

    for (of, wide, tall, stride, at, long) in entries {
        let at = began.checked_add(at)?;
        let end = at.checked_add(long)?;

        if end > bytes.len() || long == 0 {
            return None;
        }

        // A picture whose rows do not fit in its own pixels would be read past
        // the end of the file by whatever draws it. The last row is measured as
        // the pixels it actually has rather than as a whole stride, because
        // that is how a pixbuf lays one out and this is where they come from.
        let rows = fitted::<u32, usize>(stride)
            .checked_mul(fitted::<u32, usize>(tall.saturating_sub(1)))?;
        let last = fitted::<u32, usize>(wide).checked_mul(4)?;

        if rows.checked_add(last)? > long {
            return None;
        }

        held.insert(of, Where { wide, tall, stride, at, long });
    }

    Some(held)
}

/// The store as it is held: the bytes it was read out of, and where each
/// picture is in them.
///
/// The two travel together because one is only meaningful with the other: the
/// index says where to cut and the bytes are what is cut.
type Held = (Vec<u8>, BTreeMap<String, Where>);

/// The store, held for as long as this panel is up.
///
/// Read once. A panel is one opening and then a few minutes of somebody using
/// it, and re-reading a file that cannot have changed in a way this process
/// cares about would be a read per row. What is written while a panel is up is
/// picked up by the next panel, which is the same rule the remembered rows and
/// the icon index already keep.
fn held() -> Option<&'static Held> {
    static HELD: OnceLock<Option<Held>> = OnceLock::new();
    HELD.get_or_init(|| {
        let Ok(bytes) = std::fs::read(store()) else { return None };

        let index = read(&bytes)?;
        Some((bytes, index))
    })
    .as_ref()
}

/// A picture out of the store, already the size it is drawn at.
///
/// Nothing if it is not in there, which is a row that opens its own file the
/// way every row used to. So a store that is missing or behind costs the time
/// it was made to save and never draws the wrong thing.
pub fn ready(of: &Path) -> Option<gdk::Texture> {
    let (bytes, index) = held()?;
    let found = index.get(of.to_str()?)?;
    let pixels = bytes.get(found.at..found.at + found.long)?;
    let texture = gdk::MemoryTexture::new(
        fitted(found.wide),
        fitted(found.tall),
        gdk::MemoryFormat::R8g8b8a8,
        &glib::Bytes::from(pixels),
        fitted(found.stride),
    );
    Some(texture.upcast())
}

/// Which of these the store has not got.
///
/// Asked by a panel once its real rows have arrived, so that what is made is
/// what a list actually asked for rather than everything on the machine.
pub fn missing(wanted: &[String]) -> Vec<String> {
    let index = held().map(|(_, index)| index);
    wanted
        .iter()
        .filter(|of| !index.is_some_and(|index| index.contains_key(*of)))
        .cloned()
        .collect()
}

/// Make the pictures a list wanted and did not find, behind whatever asked.
///
/// Left running and not waited for: the panel that asked is already drawn, and
/// what this is for is the opening after this one.
///
/// Asked for once per picture per panel, whatever happens to it. The store this
/// process read is the store it goes on reading, so what was missing when the
/// panel opened is missing to it for as long as it is up -- and without this,
/// every press of a shoulder would start another maker doing the work the last
/// one has just finished.
pub fn make(wanted: &[String]) {
    static ASKED: std::sync::Mutex<Option<std::collections::BTreeSet<String>>> =
        std::sync::Mutex::new(None);

    let Ok(mut asked) = ASKED.lock() else { return };

    let asked = asked.get_or_insert_with(std::collections::BTreeSet::new);
    let wanted: Vec<String> =
        wanted.iter().filter(|of| asked.insert((*of).clone())).cloned().collect();
    let wanted = &wanted[..];

    if wanted.is_empty() {
        return;
    }

    let started = std::process::Command::new("panel-pictures")
        .args(wanted)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    if let Err(fault) = started {
        eprintln!("no pictures made: {fault}");
    }
}

/// One little-endian number, and what is left after it.
fn number(bytes: &[u8]) -> Option<(u32, &[u8])> {
    let (four, rest) = bytes.split_at_checked(4)?;

    let Ok(four): Result<[u8; 4], _> = four.try_into() else { return None };

    Some((u32::from_le_bytes(four), rest))
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

    /// Written and read back is the same store, because everything that draws
    /// from it goes through both.
    #[test]
    fn a_store_says_where_each_picture_is_and_how_big() {
        let pictures = vec![a_picture("/usr/share/icons/one.svg", 32), a_picture("/two.png", 16)];
        let bytes = written(&pictures);
        let held = read(&bytes).expect("a written store reads back");
        assert_eq!(held.len(), 2);
        let one = held.get("/usr/share/icons/one.svg").expect("the first picture");
        assert_eq!((one.wide, one.tall, one.stride), (32, 32, 128));
        assert_eq!(&bytes[one.at..one.at + one.long], &vec![7u8; 32 * 32 * 4]);
        let two = held.get("/two.png").expect("the second picture");
        assert_eq!(&bytes[two.at..two.at + two.long], &vec![7u8; 16 * 16 * 4]);
    }

    /// The store is written by a program that can be killed halfway through it,
    /// and read by every panel before it draws.
    #[test]
    fn half_a_store_is_no_store_rather_than_half_the_pictures() {
        let bytes = written(&[a_picture("/one.svg", 32)]);
        for cut in [0, 4, MAGIC.len(), MAGIC.len() + 4, bytes.len() - 1] {
            assert_eq!(read(&bytes[..cut]), None, "a store cut at {cut} was read as a store");
        }
    }

    /// Something else entirely, and something written the way an older version
    /// of this laid it out, are both nothing.
    #[test]
    fn a_file_that_is_not_this_file_is_not_read_at_all() {
        assert_eq!(read(b"not this at all"), None);
        assert_eq!(read(b"panel-pictures 2\n"), None);
    }

    /// A picture pointing past the end of the file is what a reader would draw
    /// out of whatever memory came after it.
    #[test]
    fn a_picture_that_points_outside_the_store_is_refused() {
        let mut bytes = written(&[a_picture("/one.svg", 32)]);
        let far = (1_000_000u32).to_le_bytes();
        let at = bytes.len() - 32 * 32 * 4 - 8;
        bytes[at..at + 4].copy_from_slice(&far);
        assert_eq!(read(&bytes), None);
    }

    /// And so is one whose rows do not fit in the pixels it brought, which is
    /// the same read past the end one number further in.
    #[test]
    fn a_picture_whose_rows_do_not_fit_its_own_pixels_is_refused() {
        let mut picture = a_picture("/one.svg", 32);
        picture.stride = 4096;
        assert_eq!(read(&written(&[picture])), None);
    }

    #[test]
    fn an_empty_store_is_a_store_with_nothing_in_it() {
        let held = read(&written(&[])).expect("an empty store is still a store");
        assert!(held.is_empty());
    }
}
