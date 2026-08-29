//! The pictures a listing shows, and where the desktop keeps them.
//!
//! By the rule every other desktop uses, so a picture made here is one Dolphin
//! finds and a picture Dolphin made is one this finds. The store is a folder in
//! the cache, a picture is named for the address of the thing it is of, and one
//! made before the thing last changed is out of date.
//!
//! Nothing here makes one. That is `files-thumbs`, which runs off the panel:
//! a folder of two hundred photographs takes seconds to work through and a
//! panel that waited for it would show nothing at all until it was done.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use gtk4::glib;

/// How big a made picture is, on its longest side.
///
/// The size the shared store is specified at, so what is made here is what
/// anything else reading the store expects to find. The panel asks for less
/// than this and scales down, which costs nothing and means the store does not
/// have to be made again the day a row gets taller.
pub const SIDE: i32 = 128;

/// Where the desktop keeps them.
pub fn store(cache: &Path) -> PathBuf {
    cache.join("thumbnails").join("normal")
}

/// The address of a thing, as the store names it by.
///
/// The real path and not the one it was reached by. A folder reached through a
/// link has a second name for every file in it, and a store keyed by the name
/// used would keep a second picture of each: one made walking in one way and
/// never found walking in the other. Following the links first means a thing
/// has one picture however it was arrived at.
pub fn address(path: &Path) -> Option<String> {
    let real = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    glib::filename_to_uri(real, None).ok().map(|uri| uri.to_string())
}

/// The picture of that address, in the store.
///
/// The name is the digest of the address and not of the thing itself, which is
/// what makes this cheap: naming it takes no reading of a file that may be a
/// gigabyte of film.
pub fn of(store: &Path, address: &str) -> Option<PathBuf> {
    let digest = glib::compute_checksum_for_string(glib::ChecksumType::Md5, address)?;
    Some(store.join(format!("{digest}.png")))
}

/// Whether a made picture still says something true about the thing.
///
/// Made before the thing last changed, it is a picture of what that thing used
/// to be. A photograph edited on this device would go on showing the version
/// before the edit for as long as the store was believed.
pub fn fresh(made: SystemTime, changed: SystemTime) -> bool {
    made >= changed
}

/// The one on disk, if there is one and it is still true.
pub fn found(store: &Path, path: &Path) -> Option<PathBuf> {
    let picture = of(store, &address(path)?)?;
    let made = picture.metadata().ok()?.modified().ok()?;
    let changed = path.metadata().ok()?.modified().ok()?;
    fresh(made, changed).then_some(picture)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn cache() -> PathBuf {
        Path::new("/home/ada/.cache").to_path_buf()
    }

    #[test]
    fn the_store_is_where_every_other_desktop_looks() {
        assert_eq!(store(&cache()), Path::new("/home/ada/.cache/thumbnails/normal"));
    }

    /// The name the shared store gives a picture is the digest of the address,
    /// so anything else reading the store finds the same one.
    #[test]
    fn a_picture_is_named_for_the_address_of_the_thing_it_is_of() {
        let store = store(&cache());
        let one = of(&store, "file:///home/ada/Pictures/beach.jpg").expect("a name");
        let same = of(&store, "file:///home/ada/Pictures/beach.jpg").expect("a name");
        let other = of(&store, "file:///home/ada/Pictures/boat.jpg").expect("a name");
        assert_eq!(one, same);
        assert_ne!(one, other);
        assert!(one.starts_with(&store));
        assert_eq!(one.extension().and_then(|end| end.to_str()), Some("png"));
    }

    /// A name with a space in it is still one address, and one the store agrees
    /// with. Handed the path rather than the address, every one of those would
    /// be a picture nothing else could find.
    #[test]
    fn an_address_is_written_the_way_the_store_expects_it() {
        let said = address(Path::new("/home/ada/Pictures/a day out.jpg")).expect("an address");
        assert!(said.starts_with("file:///"));
        assert!(!said.contains(' '), "{said}");
    }

    /// One thing, one picture, whichever way it was walked to.
    #[test]
    fn a_thing_reached_through_a_link_has_the_address_of_the_thing() {
        let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/thumbs.rs");
        let roundabout = real.parent().expect("a folder").join("../src/thumbs.rs");
        assert_eq!(address(&roundabout), address(&real));
    }

    #[test]
    fn a_picture_made_before_the_thing_changed_is_out_of_date() {
        let then = SystemTime::UNIX_EPOCH;
        let now = then + Duration::from_secs(60);
        assert!(fresh(now, then));
        assert!(fresh(then, then));
        assert!(!fresh(then, now));
    }
}
