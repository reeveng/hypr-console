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

use console_core_never::Never;
use gtk4::glib;

pub const SIDE: i32 = 128;

pub fn store(cache: &Path) -> Result<PathBuf, Never> {
    Ok(cache.join("thumbnails").join("normal"))
}

pub fn address(path: &Path) -> Result<Option<String>, Never> {
    let real = match path.canonicalize() {
        Ok(real) => real,
        Err(_) => path.to_path_buf(),
    };

    let uri = match glib::filename_to_uri(real, None) {
        Ok(uri) => uri,
        Err(_fault) => return Ok(None),
    };

    Ok(Some(uri.to_string()))
}

pub fn of(store: &Path, address: &str) -> Result<Option<PathBuf>, Never> {
    let digest = match glib::compute_checksum_for_string(glib::ChecksumType::Md5, address) {
        Some(digest) => digest,
        None => return Ok(None),
    };

    Ok(Some(store.join(format!("{digest}.png"))))
}

pub fn fresh(made: SystemTime, changed: SystemTime) -> Result<Fresh, Never> {
    Ok(match made >= changed {
        true => Fresh::Yes,
        false => Fresh::Stale,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fresh {
    Yes,
    Stale,
}

pub fn found(store: &Path, path: &Path) -> Result<Option<PathBuf>, Never> {
    let address = address(path)?;

    let address = match address {
        Some(address) => address,
        None => return Ok(None),
    };

    let picture = of(store, &address)?;

    let picture = match picture {
        Some(picture) => picture,
        None => return Ok(None),
    };

    let made = match picture.metadata().and_then(|held| held.modified()) {
        Ok(made) => made,
        Err(_fault) => return Ok(None),
    };

    let changed = match path.metadata().and_then(|held| held.modified()) {
        Ok(changed) => changed,
        Err(_fault) => return Ok(None),
    };

    let fresh = fresh(made, changed)?;

    Ok(match fresh {
        Fresh::Yes => Some(picture),
        Fresh::Stale => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn cache() -> PathBuf {
        Path::new("/home/ada/.cache").to_path_buf()
    }

    fn store_of(cache: &Path) -> PathBuf {
        let Ok(store) = store(cache);

        store
    }

    fn named(store: &Path, address: &str) -> PathBuf {
        let Ok(name) = of(store, address);

        name.expect("a name")
    }

    #[test]
    fn the_store_is_where_every_other_desktop_looks() {
        assert_eq!(store_of(&cache()), Path::new("/home/ada/.cache/thumbnails/normal"));
    }

    #[test]
    fn a_picture_is_named_for_the_address_of_the_thing_it_is_of() {
        let store = store_of(&cache());
        let one = named(&store, "file:///home/ada/Pictures/beach.jpg");
        let same = named(&store, "file:///home/ada/Pictures/beach.jpg");
        let other = named(&store, "file:///home/ada/Pictures/boat.jpg");

        assert_eq!(one, same);
        assert_ne!(one, other);
        assert!(one.starts_with(&store));
        assert_eq!(one.extension().and_then(|end| end.to_str()), Some("png"));
    }

    #[test]
    fn an_address_is_written_the_way_the_store_expects_it() {
        let Ok(said) = address(Path::new("/home/ada/Pictures/a day out.jpg"));

        let said = said.expect("an address");

        assert!(said.starts_with("file:///"));
        assert!(!said.contains(' '), "{said}");
    }

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
        assert_eq!(fresh(now, then), Ok(Fresh::Yes));
        assert_eq!(fresh(then, then), Ok(Fresh::Yes));
        assert_eq!(fresh(then, now), Ok(Fresh::Stale));
    }
}
