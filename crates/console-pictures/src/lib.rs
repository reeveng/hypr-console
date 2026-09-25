//! A picture file, read into the pixels this desktop draws.
//!
//! Four programs wanted this and every one of them asked gdk-pixbuf: the menu's
//! icon store, the thumbnails behind a folder, the cover a song is played
//! under, and the viewer. None of them wanted a widget. What they wanted was a
//! width, a height and a run of RGBA, which is what goes in the buffer a
//! compositor reads, so the toolkit was carrying four call sites for a shape
//! that fits in one struct.
//!
//! **What decodes it is what the device already has.** ffmpeg is in
//! `[packages]` because the viewer and the thumbnails have always wanted
//! frames out of a film, and it reads every raster format this desktop can
//! meet -- PNG, JPEG, WebP, HEIF, the lot -- and scales while it is there.
//! What it does not read is SVG, which is most of an icon theme, so an SVG is
//! drawn by the library gdk-pixbuf was calling for it all along: librsvg, as
//! `rsvg-convert`, into a PNG that the same one path then reads. That is one
//! new package to lose a toolkit, and it is the package the toolkit was using.
//!
//! **The size is asked for rather than guessed.** Raw video carries no header,
//! so a decode that scales and then reads the bytes does not know what shape
//! they are. ffprobe says what the file is before ffmpeg is told what to make
//! of it, the fitting is arithmetic here where it can be tested, and ffmpeg is
//! handed the exact width and height -- so the run that comes back is the size
//! this crate already said it would be, and a short one is a fault rather than
//! a picture with a torn last row.
//!
//! Two shapes are wanted and both are here. A picture drawn into a space is
//! fitted to it, which is every icon and every thumbnail; a cover under a song
//! is the square middle of the sleeve at exactly the size the characters make,
//! which is a crop and not a fit, and squashing the whole of a wide sleeve into
//! it is what a scale on its own would do.
//!
//! **One frame, whatever the file holds.** A film and an animated picture are
//! many frames, and a decode that writes every one of them is a run of bytes
//! as long as the film, arrived at after the film has been read end to end --
//! and then refused for not being the length of one picture. The first frame is
//! what a thumbnail and a still are made of, so it is the only one asked for.
//!
//! **A page of a PDF arrives already decoded.** poppler's `pdftoppm` writes
//! the page it drew as a PPM on its standard output, which is a line of header
//! and then the pixels, so [`from_portable_pixmap`] reads it here rather than handing a
//! file to ffmpeg to find out what it already says. The same shape is what
//! [`portable_pixmap`] writes, for a picture this desktop drew itself -- a QR
//! code of the Wi-Fi -- which then goes the one way every other file does.
//!
//! A file that neither of them can read is no picture rather than a failure. It
//! is the ordinary case -- a folder of two hundred things is walked for the few
//! that can be shown -- and the callers all had the same sentence for it
//! already. What is a fault is a program that will not run and a run of bytes
//! that is not the length this crate worked out, because both of those are the
//! machine rather than the file.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use console_core_external_programs::Program;
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::whole_u32;
use console_core_shapes::Pixels;

pub const SVG: &str = "svg";

const BYTES_A_PIXEL: u32 = 4;

#[derive(Debug)]
pub enum PictureError {
    Query(&'static str, std::io::Error),
    Unmeasured(PathBuf, String),
    Short { of: PathBuf, wanted: u64, got: u64 },
}

impl std::fmt::Display for PictureError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PictureError::Query(program, fault) => write!(to, "{program}: {fault}"),
            PictureError::Unmeasured(at, said) => {
                write!(to, "{}: nothing says how big this is: {said:?}", at.display())
            },
            PictureError::Short { of, wanted, got } => {
                write!(to, "{}: {got} bytes of a picture that is {wanted}", of.display())
            },
        }
    }
}

impl std::error::Error for PictureError {}

pub fn measured(at: &Path) -> Result<Option<Size<u32>>, PictureError> {
    let Ok(mut asking) = Program::Ffprobe.command();

    let said = asking
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height"])
        .args(["-of", "csv=p=0"])
        .arg(at)
        .output()
        .map_err(|fault| PictureError::Query("ffprobe", fault))?;

    match said.status.success() {
        true => {},
        false => return Ok(None),
    }

    let printed = String::from_utf8_lossy(&said.stdout).trim().to_string();

    let (wide, tall) = match printed.split_once(',') {
        Some((wide, tall)) => (wide.trim().to_string(), tall.trim().to_string()),
        None => return Err(PictureError::Unmeasured(at.to_path_buf(), printed)),
    };

    match (wide.parse::<u32>(), tall.parse::<u32>()) {
        (Ok(wide), Ok(tall)) => Ok(Some(Size { width: wide, height: tall })),
        (Err(_), _) | (_, Err(_)) => Err(PictureError::Unmeasured(at.to_path_buf(), printed)),
    }
}

pub fn fitted(had: Size<u32>, within: Size<u32>) -> Result<Size<u32>, Never> {
    let (wide, tall) = (f64::from(had.width), f64::from(had.height));

    match wide > 0.0 && tall > 0.0 {
        true => {},
        false => return Ok(Size { width: 1, height: 1 }),
    }

    let across = f64::from(within.width) / wide;
    let down = f64::from(within.height) / tall;
    let by = across.min(down);
    let Ok(made_wide) = whole_u32(wide * by);
    let Ok(made_tall) = whole_u32(tall * by);

    Ok(Size { width: made_wide.max(1), height: made_tall.max(1) })
}

fn drawn(at: &Path, filter: &str) -> Result<Option<Vec<u8>>, PictureError> {
    let Ok(mut asking) = Program::Ffmpeg.command();

    let said = asking
        .args(["-v", "error", "-i"])
        .arg(at)
        .args(["-vf", filter, "-frames:v", "1"])
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .stderr(Stdio::piped())
        .output()
        .map_err(|fault| PictureError::Query("ffmpeg", fault))?;

    Ok(match said.status.success() {
        true => Some(said.stdout),
        false => None,
    })
}

fn is_a_drawing(at: &Path) -> Result<Rendering, Never> {
    let ending = at.extension().map(|ending| ending.to_string_lossy().to_lowercase());

    Ok(match ending.as_deref() == Some(SVG) {
        true => Rendering::AsPaths,
        false => Rendering::AsPixels,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rendering {
    AsPaths,
    AsPixels,
}

fn rasterized(at: &Path, within: Size<u32>) -> Result<Option<PathBuf>, PictureError> {
    let Ok(into) = beside(at);
    let Ok(mut asking) = Program::RsvgConvert.command();
    let Size { width: wide, height: tall } = within;

    let said = asking
        .args(["-w", &wide.to_string(), "-h", &tall.to_string()])
        .args(["--keep-aspect-ratio", "-f", "png", "-o"])
        .arg(&into)
        .arg(at)
        .output()
        .map_err(|fault| PictureError::Query("rsvg-convert", fault))?;

    Ok(match said.status.success() {
        true => Some(into),
        false => {
            let _ = std::fs::remove_file(&into);

            None
        }
    })
}

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the name of a file this process is about to make and then delete, which nothing outside the process can hand it and nothing outside the process may share"
    )
)]
static ONE_AFTER_ANOTHER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn beside(at: &Path) -> Result<PathBuf, Never> {
    let mine = ONE_AFTER_ANOTHER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let named = match at.file_stem().map(|named| named.to_string_lossy().to_string()) {
        Some(named) => named,
        None => "picture".to_string(),
    };

    Ok(std::env::temp_dir().join(format!("console-picture-{}-{mine}-{named}.png", std::process::id())))
}

pub fn decoded(at: &Path, within: Size<u32>) -> Result<Option<Pixels>, PictureError> {
    let Ok(drawn_as) = is_a_drawing(at);

    let standing = match drawn_as {
        Rendering::AsPixels => None,
        Rendering::AsPaths => {
            let rasterized = rasterized(at, within)?;

            match rasterized {
                Some(standing) => Some(standing),
                None => return Ok(None),
            }
        },
    };

    let reading = match &standing {
        Some(standing) => standing.as_path(),
        None => at,
    };

    let read = read(reading, within);

    match standing {
        Some(standing) => {
            let _ = std::fs::remove_file(standing);
        },
        None => {},
    }

    read
}

fn read(at: &Path, within: Size<u32>) -> Result<Option<Pixels>, PictureError> {
    let measured = measured(at)?;

    let had = match measured {
        Some(had) => had,
        None => return Ok(None),
    };

    let Ok(made) = fitted(had, within);
    let Size { width: wide, height: tall } = made;

    let drawn = drawn(at, &format!("scale={wide}:{tall}"))?;

    let bytes = match drawn {
        Some(bytes) => bytes,
        None => return Ok(None),
    };

    held(at, made, bytes)
}

fn held(at: &Path, made: Size<u32>, bytes: Vec<u8>) -> Result<Option<Pixels>, PictureError> {
    let wanted = u64::from(made.width)
        .saturating_mul(u64::from(BYTES_A_PIXEL))
        .saturating_mul(u64::from(made.height));
    let Ok(got) = console_core_number_conversion::fitted::<_, u64>(bytes.len());

    match got == wanted {
        true => {},
        false => {
            return Err(PictureError::Short { of: at.to_path_buf(), wanted, got });
        }
    }

    let stride = made.width.saturating_mul(BYTES_A_PIXEL);

    Ok(Some(Pixels { width: made.width, height: made.height, stride, bytes: Arc::new(bytes) }))
}

struct ImageHeader<'a> {
    size: Size<u32>,
    pixels: &'a [u8],
}

fn portable_pixmap_header(bytes: &[u8]) -> Result<Option<ImageHeader<'_>>, Never> {
    let mut remaining = bytes;
    let mut numbers: Vec<u32> = Vec::new();

    let magic = match remaining.split_first_chunk::<2>() {
        Some((magic, after)) => {
            remaining = after;
            magic
        },
        None => return Ok(None),
    };

    match magic == b"P6" {
        true => {},
        false => return Ok(None),
    }

    while numbers.len() < 3 {
        let mut parts = remaining.trim_ascii_start().splitn(2, u8::is_ascii_whitespace);

        let (word, after) = match (parts.next(), parts.next()) {
            (Some(word), Some(after)) => (word, after),
            (None, _) | (_, None) => return Ok(None),
        };

        let number = match std::str::from_utf8(word).map(str::parse::<u32>) {
            Ok(Ok(number)) => number,
            Ok(Err(_not_a_number)) => return Ok(None),
            Err(_not_text) => return Ok(None),
        };

        numbers.push(number);
        remaining = after;
    }

    Ok(match numbers.as_slice() {
        [wide, tall, 255] => Some(ImageHeader { size: Size { width: *wide, height: *tall }, pixels: remaining }),
        _ => None,
    })
}

pub fn portable_pixmap(size: Size<u32>, rgb: &[u8]) -> Result<Vec<u8>, Never> {
    let mut made = format!("P6\n{} {}\n255\n", size.width, size.height).into_bytes();

    made.extend_from_slice(rgb);

    Ok(made)
}

pub fn from_portable_pixmap(bytes: &[u8]) -> Result<Option<Pixels>, Never> {
    let header = portable_pixmap_header(bytes)?;

    let ImageHeader { size, pixels: remaining } = match header {
        Some(header) => header,
        None => return Ok(None),
    };

    let wanted = u64::from(size.width).saturating_mul(3).saturating_mul(u64::from(size.height));
    let Ok(got) = console_core_number_conversion::fitted::<_, u64>(remaining.len());

    match got >= wanted {
        true => {},
        false => return Ok(None),
    }

    let mut rgba: Vec<u8> = Vec::new();

    for pixel in remaining.chunks_exact(3) {
        rgba.extend_from_slice(pixel);
        rgba.push(u8::MAX);
    }

    let stride = size.width.saturating_mul(BYTES_A_PIXEL);

    Ok(Some(Pixels { width: size.width, height: size.height, stride, bytes: Arc::new(rgba) }))
}

pub fn square(at: &Path, size: Size<u32>) -> Result<Option<Pixels>, PictureError> {
    let measured = measured(at)?;

    let had = match measured {
        Some(had) => had,
        None => return Ok(None),
    };

    let side = had.width.min(had.height);
    let left = had.width.saturating_sub(side).saturating_div(2);
    let top = had.height.saturating_sub(side).saturating_div(2);
    let Size { width: wide, height: tall } = size;

    let drawn = drawn(at, &format!("crop={side}:{side}:{left}:{top},scale={wide}:{tall}"))?;

    let bytes = match drawn {
        Some(bytes) => bytes,
        None => return Ok(None),
    };

    held(at, size, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_poppler_drew_is_read_as_what_its_header_says() {
        let mut bytes = b"P6\n2 1\n255\n".to_vec();
        bytes.extend_from_slice(&[255, 0, 0, 0, 0, 255]);

        let Ok(read) = from_portable_pixmap(&bytes);
        let read = read.map(|pixels| (pixels.width, pixels.height, pixels.bytes.to_vec()));

        assert_eq!(read, Some((2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])));
    }

    #[test]
    fn a_pixmap_written_here_is_read_back_as_the_same_picture() {
        let Ok(made) = portable_pixmap(Size { width: 1, height: 2 }, &[0, 0, 0, 255, 255, 255]);
        let Ok(read) = from_portable_pixmap(&made);
        let read = read.map(|pixels| (pixels.width, pixels.height, pixels.bytes.to_vec()));

        assert_eq!(read, Some((1, 2, vec![0, 0, 0, 255, 255, 255, 255, 255])));
    }

    #[test]
    fn a_page_cut_short_is_no_picture() {
        let Ok(read) = from_portable_pixmap(b"P6\n2 2\n255\n\x01\x02");

        assert_eq!(read, None);
    }

    #[test]
    fn a_picture_fits_inside_the_box_it_was_given_without_stretching() {
        assert_eq!(
            fitted(Size { width: 100, height: 50 }, Size { width: 64, height: 64 }),
            Ok(Size { width: 64, height: 32 })
        );
        assert_eq!(
            fitted(Size { width: 50, height: 100 }, Size { width: 64, height: 64 }),
            Ok(Size { width: 32, height: 64 })
        );
    }

    #[test]
    fn a_picture_smaller_than_the_box_is_drawn_up_to_it() {
        assert_eq!(
            fitted(Size { width: 16, height: 16 }, Size { width: 64, height: 64 }),
            Ok(Size { width: 64, height: 64 })
        );
    }

    #[test]
    fn nothing_is_ever_fitted_to_nothing() {
        assert_eq!(
            fitted(Size { width: 4000, height: 1 }, Size { width: 64, height: 64 }),
            Ok(Size { width: 64, height: 1 })
        );
        assert_eq!(
            fitted(Size { width: 0, height: 0 }, Size { width: 64, height: 64 }),
            Ok(Size { width: 1, height: 1 })
        );
    }

    #[test]
    fn a_drawing_is_the_one_kind_that_is_drawn_before_it_is_read() {
        assert_eq!(is_a_drawing(Path::new("/x/firefox.svg")), Ok(Rendering::AsPaths));
        assert_eq!(is_a_drawing(Path::new("/x/firefox.SVG")), Ok(Rendering::AsPaths));
        assert_eq!(is_a_drawing(Path::new("/x/beach.jpg")), Ok(Rendering::AsPixels));
        assert_eq!(is_a_drawing(Path::new("/x/beach")), Ok(Rendering::AsPixels));
    }

    fn made(at: &Path, said: &str) -> Result<(), Never> {
        made_for(at, said, "1")
    }

    #[test]
    fn what_comes_back_is_the_size_this_crate_said_and_the_colour_that_was_written() {
        let Ok(at) = beside(Path::new("red"));
        let Ok(()) = made(&at, "color=c=red:s=100x50");

        let read = decoded(&at, Size { width: 20, height: 20 }).expect("a picture ffmpeg just wrote");
        let _ = std::fs::remove_file(&at);

        let held = read.expect("ffmpeg reads what ffmpeg wrote");

        assert_eq!((held.width, held.height, held.stride), (20, 10, 80));
        assert_eq!(held.bytes.len(), 800);

        let first = held.bytes.get(0..4).map(<[u8]>::to_vec);

        match first {
            Some(said) => match said.as_slice() {
                [red, green, blue, opaque] => {
                    assert!(*red > 200, "red came back as {said:?}");
                    assert!(*green < 40 && *blue < 40, "red came back as {said:?}");
                    assert_eq!(*opaque, 255, "red came back as {said:?}");
                }
                _shorter => panic!("four bytes of red: {said:?}"),
            },
            None => panic!("no first pixel at all"),
        }
    }

    #[test]
    fn a_film_comes_back_as_its_first_frame_and_not_as_every_frame_in_it() {
        let Ok(at) = beside(Path::new("film"));
        let at = at.with_extension("mkv");
        let Ok(()) = made_for(&at, "testsrc=s=64x48:d=2", "50");

        let read = decoded(&at, Size { width: 32, height: 32 });
        let _ = std::fs::remove_file(&at);

        let held = read.expect("a film is one picture long").expect("ffmpeg reads the film it wrote");

        assert_eq!((held.width, held.height), (32, 24));
        assert_eq!(held.bytes.len(), 32 * 24 * 4);
    }

    fn made_for(at: &Path, said: &str, frames: &str) -> Result<(), Never> {
        let Ok(mut asking) = Program::Ffmpeg.command();

        let done = asking
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", said, "-frames:v", frames])
            .arg(at)
            .status();

        assert!(done.is_ok_and(|how| how.success()), "ffmpeg made no film to read back");

        Ok(())
    }

    #[test]
    fn a_drawing_comes_back_at_the_size_it_was_drawn_for() {
        let Ok(at) = beside(Path::new("square"));
        let at = at.with_extension("svg");
        let said = std::fs::write(
            &at,
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#00ff00"/></svg>"##,
        );

        assert!(said.is_ok(), "nowhere to write a drawing");

        let read = decoded(&at, Size { width: 32, height: 32 });
        let _ = std::fs::remove_file(&at);

        let held = read.expect("rsvg-convert draws it").expect("a drawing is a picture");

        assert_eq!((held.width, held.height), (32, 32));
        assert_eq!(held.bytes.get(1), Some(&255), "green: {:?}", held.bytes.get(0..4));
    }

    #[test]
    fn the_square_middle_is_the_middle_and_not_a_squashed_whole() {
        let Ok(at) = beside(Path::new("halves"));
        let Ok(()) = made(&at, "color=c=blue:s=40x20");

        let read = square(&at, Size { width: 10, height: 4 });
        let _ = std::fs::remove_file(&at);

        let held = read.expect("a picture ffmpeg just wrote").expect("a square of it");

        assert_eq!((held.width, held.height, held.stride), (10, 4, 40));
        assert_eq!(held.bytes.len(), 160);
    }

    #[test]
    fn a_file_that_is_not_a_picture_is_no_picture_rather_than_a_fault() {
        let Ok(at) = beside(Path::new("words"));
        let said = std::fs::write(&at, b"this is not a picture");

        assert!(said.is_ok(), "nowhere to write a file that is not a picture");

        let read = decoded(&at, Size { width: 64, height: 64 });
        let _ = std::fs::remove_file(&at);

        assert!(matches!(read, Ok(None)), "a file that is not a picture: {read:?}");
    }
}
