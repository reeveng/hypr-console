//! The pictures a listing shows, and where the desktop keeps them.
//!
//! By the rule every other desktop uses, so a picture made here is one Dolphin
//! finds and a picture Dolphin made is one this finds. The store is a folder in
//! the cache, a picture is named for the address of the thing it is of, and one
//! made before the thing last changed is out of date.
//!
//! Nothing here makes one. That is `files-thumbnails`, which runs off the panel:
//! a folder of two hundred photographs takes seconds to work through and a
//! panel that waited for it would show nothing at all until it was done.
//!
//! What it does do is say what the picture is of. The standard asks for two
//! lines of text inside the PNG -- the address of the thing and when that thing
//! last changed -- and they are how every other file manager decides whether
//! what it found is still about the file it is looking at. gdk-pixbuf wrote
//! them as it wrote the file; ffmpeg writes a PNG and nothing else, so they are
//! put in here, as the one chunk the format was designed to carry and this
//! tree can write: a length, a name, the bytes, and a CRC32 of the two.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use console_core_never::Never;
use console_core_number_conversion::index;

use console_core_checksums::{crc32, md5};

pub const SIDE: i32 = 128;

pub fn store(cache: &Path) -> Result<PathBuf, Never> {
    Ok(cache.join("thumbnails").join("normal"))
}

pub const PLAIN: &str = "-_.!~*'()/&=:@+$,";

pub fn escaped(path: &Path) -> Result<String, Never> {
    let mut written = String::new();

    for byte in path.as_os_str().as_encoded_bytes() {
        let letter = char::from(*byte);

        match letter.is_ascii_alphanumeric() || PLAIN.contains(letter) {
            true => written.push(letter),
            false => written.push_str(&format!("%{byte:02X}")),
        }
    }

    Ok(written)
}

pub fn address(path: &Path) -> Result<Option<String>, Never> {
    let real = match path.canonicalize() {
        Ok(real) => real,
        Err(_not_resolved) => path.to_path_buf(),
    };

    match real.is_absolute() {
        true => {},
        false => return Ok(None),
    }

    let escaped = escaped(&real)?;

    Ok(Some(format!("file://{escaped}")))
}

pub fn of(store: &Path, address: &str) -> Result<Option<PathBuf>, Never> {
    let digest = md5::of(address)?;

    Ok(Some(store.join(format!("{digest}.png"))))
}

const HEAD: u32 = 8;

const SIZE: u32 = 4;

const NAME: u32 = 4;

const IMAGE_HEADER: &[u8] = b"IHDR";

const TEXT: &[u8] = b"tEXt";

pub const URI: &str = "Thumb::URI";

pub const CHANGED: &str = "Thumb::MTime";

fn chunk(name: &[u8], said: &[u8]) -> Result<Vec<u8>, Never> {
    let mut out = Vec::new();
    let Ok(long) = u32::try_from(said.len()).map_or_else(|_| Ok::<u32, Never>(0), Ok);

    out.extend_from_slice(&long.to_be_bytes());
    out.extend_from_slice(name);
    out.extend_from_slice(said);

    let mut over = name.to_vec();

    over.extend_from_slice(said);

    let Ok(check) = crc32::of(&over);

    out.extend_from_slice(&check.to_be_bytes());

    Ok(out)
}

struct Text<'a> {
    key: &'a str,
    value: &'a str,
}

fn said(text: Text<'_>) -> Result<Vec<u8>, Never> {
    let Text { key, value } = text;
    let mut out = key.as_bytes().to_vec();

    out.push(0);
    out.extend_from_slice(value.as_bytes());

    chunk(TEXT, &out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp<'a> {
    pub address: &'a str,
    pub changed: &'a str,
}

pub fn stamped(png: &[u8], stamp: Stamp<'_>) -> Result<Option<Vec<u8>>, Never> {
    let Stamp { address, changed } = stamp;
    let Ok(from) = index(HEAD);
    let Ok(to) = index(HEAD.saturating_add(SIZE).saturating_add(NAME));
    let header = png.get(from..to);

    let long = match header {
        Some([one, two, three, four, name @ ..]) => match name == IMAGE_HEADER {
            true => u32::from_be_bytes([*one, *two, *three, *four]),
            false => return Ok(None),
        },
        Some(_shorter) => return Ok(None),
        None => return Ok(None),
    };

    let Ok(after) =
        index(HEAD.saturating_add(SIZE).saturating_add(NAME).saturating_add(long).saturating_add(SIZE));

    let (before, rest) = match (png.get(..after), png.get(after..)) {
        (Some(before), Some(rest)) => (before, rest),
        (None, _) | (_, None) => return Ok(None),
    };

    let mut out = before.to_vec();
    let Ok(uri) = said(Text { key: URI, value: address });
    let Ok(when) = said(Text { key: CHANGED, value: changed });

    out.extend_from_slice(&uri);
    out.extend_from_slice(&when);
    out.extend_from_slice(rest);

    Ok(Some(out))
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
        Err(_unstamped) => return Ok(None),
    };

    let changed = match path.metadata().and_then(|held| held.modified()) {
        Ok(changed) => changed,
        Err(_unstamped) => return Ok(None),
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

    const A_PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

    fn a_picture() -> Vec<u8> {
        let mut png = A_PNG.to_vec();
        let Ok(header) = chunk(IMAGE_HEADER, &[0; 13]);
        let Ok(end) = chunk(b"IEND", &[]);

        png.extend_from_slice(&header);
        png.extend_from_slice(&end);

        png
    }

    #[test]
    fn what_the_picture_is_of_is_written_into_it_where_the_standard_looks() {
        let png = a_picture();
        let said = stamped(&png, Stamp { address: "file:///home/ada/beach.jpg", changed: "1700000000" });

        let said = match said {
            Ok(Some(said)) => said,
            Ok(None) | Err(_) => panic!("a PNG this test just built"),
        };

        assert!(said.windows(URI.len()).any(|said| said == URI.as_bytes()));
        assert!(said.windows(CHANGED.len()).any(|said| said == CHANGED.as_bytes()));
        assert!(said.len() > png.len());
        assert_eq!(said.get(..8).map(<[u8]>::to_vec), Some(A_PNG.to_vec()));
    }

    #[test]
    fn the_two_lines_go_after_the_header_and_before_everything_else() {
        let png = a_picture();
        let Ok(said) = stamped(&png, Stamp { address: "file:///x", changed: "1" });

        let said = said.expect("a PNG this test just built");
        let before_the_end: Vec<&[u8]> = said.windows(4).take_while(|four| *four != b"IEND").collect();

        assert!(said.windows(4).any(|four| four == b"IEND"));
        assert!(before_the_end.contains(&b"tEXt".as_slice()));
    }

    #[test]
    fn a_file_that_is_not_a_png_is_left_alone() {
        assert_eq!(stamped(b"this is not a picture", Stamp { address: "file:///x", changed: "1" }), Ok(None));
        assert_eq!(stamped(&[], Stamp { address: "file:///x", changed: "1" }), Ok(None));
    }

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
        let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/thumbnails.rs");
        let roundabout = real.parent().expect("a folder").join("../src/thumbnails.rs");
        assert_eq!(address(&roundabout), address(&real));
    }

    #[test]
    fn a_name_is_the_one_every_other_file_manager_would_have_written() {
        let held = [
            ("/home/ada/Pictures/a b.png", "15f65b62306efc5662adef849c339d9b"),
            (
                "/home/ada/Pictures/\u{3a9}&x=1,y+2$z@w:v!~*()'-_.png",
                "21d5be1adab266d9c27ce68ede676ad6",
            ),
            ("/home/ada/Pictures/#q%20[1].jpg", "a6f55749645d74d8aebceaa701dd5820"),
        ];

        for (path, digest) in held {
            let Ok(said) = escaped(Path::new(path));

            let Ok(one) = of(Path::new("/store"), &format!("file://{said}"));

            assert_eq!(
                one,
                Some(PathBuf::from(format!("/store/{digest}.png"))),
                "{path} is not filed where the standard says it is"
            );
        }
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
