//! The pictures a listing shows, and where the desktop keeps them.
//!
//! By the rule every other desktop uses, so a picture made here is one Dolphin
//! finds and a picture Dolphin made is one this finds. The store is a folder in
//! the cache, a picture is named for the address of the thing it is of, and one
//! made before the thing last changed is out of date.
//!
//! Nothing here runs over a folder. That is `files-thumbnails`, which runs off
//! the panel: a folder of two hundred photographs takes seconds to work through
//! and a panel that waited for it would show nothing at all until it was done.
//! What is here is one picture made: a photograph or a drawing through
//! `console-pictures`, and a film from three seconds in, because the first
//! moment of a film is a black screen or a title more often than it is the film.
//! A film shorter than that is drawn from its start.
//!
//! What the picture is of is supplied as text to `console-core-png-files`:
//! the address of the thing and when it last changed. The thumbnail standard
//! names those fields; the PNG crate owns how they are written into a file.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use console_concurrency::Cores;
use console_films::{FilmError, open};
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_png_files::{Picture, PngError, Text, encoded_with_text};
use console_pictures::{PictureError, decoded, fit_within};

use console_core_checksums::md5;

pub const SIDE: u32 = 128;

const INTO_A_FILM: f64 = 3.0;

#[derive(Debug)]
pub enum ThumbnailError {
    Unread(PictureError),
    Unplayed(FilmError),
    Unwritten(PngError),
}

impl std::fmt::Display for ThumbnailError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThumbnailError::Unread(fault) => write!(to, "{fault}"),
            ThumbnailError::Unplayed(fault) => write!(to, "{fault}"),
            ThumbnailError::Unwritten(fault) => write!(to, "writing the picture down: {fault}"),
        }
    }
}

impl std::error::Error for ThumbnailError {}

pub fn store(cache: &Path) -> Result<PathBuf, Never> {
    Ok(cache.join("thumbnails").join("normal"))
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

    let Ok(url) = console_core_file_urls::url(&real);

    Ok(Some(url))
}

pub fn of(store: &Path, address: &str) -> Result<Option<PathBuf>, Never> {
    let digest = md5::of(address)?;

    Ok(Some(store.join(format!("{digest}.png"))))
}

pub fn drawn(thing: &Path, kind: &str, stamp: Option<Stamp<'_>>) -> Result<Option<Vec<u8>>, ThumbnailError> {
    let within = Size { width: SIDE, height: SIDE };

    let into_it = match kind.starts_with("video/") {
        true => into_a_film(thing, within),
        false => Ok(None),
    };

    let into_it = into_it.map_err(ThumbnailError::Unplayed)?;

    let picture = match into_it {
        Some(picture) => Some(picture),
        None => {
            let read = decoded(thing, within).map_err(ThumbnailError::Unread)?;

            read.map(|pixels| Picture { size: Size { width: pixels.width, height: pixels.height }, rgba: pixels.bytes.to_vec() })
        },
    };

    let text = match stamp {
        Some(Stamp { address, changed }) => vec![
            Text { keyword: URI, value: address },
            Text { keyword: CHANGED, value: changed },
        ],
        None => Vec::new(),
    };

    match picture {
        Some(picture) => encoded_with_text(&picture, &text).map(Some).map_err(ThumbnailError::Unwritten),
        None => Ok(None),
    }
}

fn into_a_film(thing: &Path, within: Size<u32>) -> Result<Option<Picture>, FilmError> {
    let opened = open(thing)?;

    let film = match opened {
        Some(film) => film,
        None => return Ok(None),
    };

    let pictures = film.pictures(INTO_A_FILM)?;

    let mut pictures = match pictures {
        Some(pictures) => pictures,
        None => return Ok(None),
    };

    let shown = pictures.next_picture()?;

    let shown = match shown {
        Some(shown) => shown,
        None => return Ok(None),
    };

    let (Ok(had), Ok(one)) = (shown.size(), Cores::one());
    let Ok(made) = fit_within(had, within);
    let Ok(drawn) = shown.rgba_at(made, one);

    Ok(Some(Picture { size: drawn.size, rgba: drawn.rgba }))
}

pub const URI: &str = "Thumb::URI";

pub const CHANGED: &str = "Thumb::MTime";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp<'a> {
    pub address: &'a str,
    pub changed: &'a str,
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

pub fn find_thumbnail(store: &Path, path: &Path) -> Result<Option<PathBuf>, Never> {
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
    use console_core_number_conversion::index;
    use std::error::Error;
    use std::time::Duration;

    #[test]
    fn what_the_picture_is_of_is_written_into_it_without_changing_the_pixels() -> Result<(), Box<dyn Error>> {
        let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("../console-core-png-files/tests/pictures/rgba-8.png");
        let stamp = Stamp { address: "file:///home/ada/beach.jpg", changed: "1700000000" };
        let stamped = drawn(&at, "image/png", Some(stamp))?;
        let png = stamped.ok_or("no thumbnail")?;
        let unstamped = drawn(&at, "image/png", None)?;
        let plain = unstamped.ok_or("no thumbnail")?;
        let picture = console_core_png_files::decoded(&png)?;
        let original = console_core_png_files::decoded(&plain)?;
        let uri = format!("{URI}\0{}", stamp.address);
        let changed = format!("{CHANGED}\0{}", stamp.changed);

        assert_eq!(picture, original);
        assert!(png.windows(uri.len()).any(|bytes| bytes == uri.as_bytes()));
        assert!(png.windows(changed.len()).any(|bytes| bytes == changed.as_bytes()));

        Ok(())
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Seen {
        Red,
        Green,
        Neither,
    }

    fn drawn_from(file: (&str, &str)) -> Result<console_core_png_files::Picture, Box<dyn Error>> {
        let (named, kind) = file;
        let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(named);
        let made = drawn(&at, kind, None)?;
        let png = made.ok_or(format!("nothing was drawn of {named}"))?;
        let picture = console_core_png_files::decoded(&png)?;

        Ok(picture)
    }

    fn middle_of(picture: &console_core_png_files::Picture) -> Result<Seen, Box<dyn Error>> {
        let across = u64::from(picture.size.width);
        let row = u64::from(picture.size.height.saturating_div(2));
        let Ok(pixel) = index(row.saturating_mul(across).saturating_add(across.saturating_div(2)));
        let said = picture.rgba.as_chunks::<4>().0.get(pixel).ok_or("the middle is past the end of the picture")?;

        Ok(match said {
            [red, green, blue, u8::MAX] => match (*red > 200, *green > 100, *blue < 60) {
                (true, false, true) => Seen::Red,
                (false, true, true) => Seen::Green,
                (_, _, _) => Seen::Neither,
            },
            _ => Seen::Neither,
        })
    }

    #[test]
    fn a_film_is_drawn_from_three_seconds_in_and_one_shorter_than_that_from_its_start() -> Result<(), Box<dyn Error>> {
        let long = drawn_from(("console-films/tests/films/dark-then-green.h264.mp4", "video/mp4"))?;
        let short = drawn_from(("console-films/tests/films/red-then-blue.h264.mp4", "video/mp4"))?;
        let side = Size { width: SIDE, height: 85 };
        let long_seen = middle_of(&long)?;
        let short_seen = middle_of(&short)?;

        assert_eq!((long.size, long_seen), (side, Seen::Green));
        assert_eq!((short.size, short_seen), (side, Seen::Red));

        Ok(())
    }

    #[test]
    fn a_photograph_is_drawn_no_wider_and_no_taller_than_the_store_keeps() -> Result<(), Box<dyn Error>> {
        let photograph = drawn_from(("console-core-jpeg-files/tests/pictures/full.jpg", "image/jpeg"))?;
        let measured = console_pictures::measure(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../console-core-jpeg-files/tests/pictures/full.jpg"))?;
        let had = measured.ok_or("the photograph has no size")?;
        let Ok(fitted) = fit_within(had, Size { width: SIDE, height: SIDE });

        assert_eq!(photograph.size, fitted);

        Ok(())
    }

    const CACHE: &str = "/home/ada/.cache";

    fn thumbnail_path(store: &Path, address: &str) -> Result<PathBuf, Box<dyn Error>> {
        let Ok(name) = of(store, address);
        let name = name.ok_or("a name")?;

        Ok(name)
    }

    #[test]
    fn the_store_is_where_every_other_desktop_looks() {
        assert_eq!(store(Path::new(CACHE)), Ok(PathBuf::from("/home/ada/.cache/thumbnails/normal")));
    }

    #[test]
    fn a_picture_is_named_for_the_address_of_the_thing_it_is_of() -> Result<(), Box<dyn Error>> {
        let Ok(store) = store(Path::new(CACHE));
        let one = thumbnail_path(&store, "file:///home/ada/Pictures/beach.jpg")?;
        let same = thumbnail_path(&store, "file:///home/ada/Pictures/beach.jpg")?;
        let other = thumbnail_path(&store, "file:///home/ada/Pictures/boat.jpg")?;

        assert_eq!(one, same);
        assert_ne!(one, other);
        assert!(one.starts_with(&store));
        assert_eq!(one.extension().and_then(|end| end.to_str()), Some("png"));

        Ok(())
    }

    #[test]
    fn an_address_is_written_the_way_the_store_expects_it() -> Result<(), Box<dyn Error>> {
        let Ok(said) = address(Path::new("/home/ada/Pictures/a day out.jpg"));
        let said = said.ok_or("an address")?;

        assert!(said.starts_with("file:///"));
        assert!(!said.contains(' '), "{said}");

        Ok(())
    }

    #[test]
    fn a_thing_reached_through_a_link_has_the_address_of_the_thing() -> Result<(), Box<dyn Error>> {
        let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/thumbnails.rs");
        let folder = real.parent().ok_or("a folder")?;
        let roundabout = folder.join("../src/thumbnails.rs");

        assert_eq!(address(&roundabout), address(&real));

        Ok(())
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
            let Ok(said) = console_core_file_urls::url(Path::new(path));

            let Ok(one) = of(Path::new("/store"), &said);

            assert_eq!(
                one,
                Some(PathBuf::from(format!("/store/{digest}.png"))),
                "{path} is not filed where the standard says it is"
            );
        }
    }

    #[test]
    fn a_picture_made_before_the_thing_changed_is_out_of_date() -> Result<(), Box<dyn Error>> {
        let then = SystemTime::UNIX_EPOCH;
        let now = then.checked_add(Duration::from_secs(60)).ok_or("a minute on from the epoch")?;

        assert_eq!(fresh(now, then), Ok(Fresh::Yes));
        assert_eq!(fresh(then, then), Ok(Fresh::Yes));
        assert_eq!(fresh(then, now), Ok(Fresh::Stale));

        Ok(())
    }
}
