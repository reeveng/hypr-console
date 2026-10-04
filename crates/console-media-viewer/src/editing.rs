//! A picture turned, flipped or cropped, kept as a new file beside the one it
//! was made from.
//!
//! The file a person opened is never written. Every edit is a new picture
//! named after the old one -- `beach edited.jpg`, then `beach edited 2.jpg` --
//! and the copy is given its name only if nobody has it yet, so a name that
//! turned up between choosing it and writing it is refused rather than
//! replaced. Editing an edit starts from the name it was edited from, so a
//! third turn of the beach is `beach edited 3` rather than
//! `beach edited edited edited`.
//!
//! There is no crop box. What is kept is what is on the screen: the zoom and
//! the pan already say which part of the picture someone is looking at, and on
//! a machine with sticks and a thumb that is a better way to choose a
//! rectangle than four corners dragged one at a time. [`crop_region`] is that
//! region in the picture's own pixels, worked out by the same
//! `console_panel::zoom` arithmetic the surface drew it with.
//!
//! **No second program.** The edit used to be an ffmpeg filter, which read
//! every format ffmpeg reads and wrote most of them back. Everything it was
//! handed in practice is a photograph or a screenshot, and this tree reads
//! both and writes both, so the picture is read by `console_pictures`, turned
//! here as rows of pixels, and written by the JPEG and PNG encoders. A JPEG
//! comes back a JPEG and anything else a PNG -- a GIF, whose animation one
//! frame of cannot keep anyway, and a WebP, which nothing here writes. A file
//! none of the decoders read -- a HEIC off a phone, a camera's raw -- is not
//! edited, and the viewer says so by name rather than starting ffmpeg for it;
//! a format worth editing is a decoder and an encoder added when somebody
//! meets it.

use std::path::{Path, PathBuf};

use console_core_atomic_writes::Unwritten;
use console_core_geometry::{Point, Size};
use console_core_jpeg_files::{JpegError, Quality};
use console_core_never::Never;
use console_core_number_conversion::{index, whole_u32};
use console_core_png_files::PngError;
use console_core_words::Words;
use console_panel::zoom::{Framed, Zoomed};
use console_pictures::{Bitmap, Format, FullSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Edit {
    #[words(says = "Rotate Left")]
    RotateLeft,
    #[words(says = "Rotate Right")]
    RotateRight,
    #[words(says = "Flip")]
    Flip,
    #[words(says = "Crop")]
    Crop,
}

pub const EDITS: [Edit; 4] = [Edit::RotateLeft, Edit::RotateRight, Edit::Flip, Edit::Crop];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub from: Point<u32>,
    pub size: Size<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    RotateLeft,
    RotateRight,
    Flip,
    Crop(Region),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Planned {
    Change(Change),
    ZoomInFirst,
}

pub fn crop_region(framed: &Framed, of: Size<u32>) -> Result<Option<Region>, Never> {
    let Ok(zoomed) = framed.zoom.zoomed();

    match zoomed {
        Zoomed::Whole => return Ok(None),
        Zoomed::ZoomedIn => {},
    }

    let Ok(placed) = framed.zoom.place(of, framed.room);
    let Ok(across) = whole_u32(placed.from.x);
    let Ok(down) = whole_u32(placed.from.y);
    let across = across.min(of.width.saturating_sub(1));
    let down = down.min(of.height.saturating_sub(1));
    let Ok(wide) = whole_u32(placed.seen.width);
    let Ok(tall) = whole_u32(placed.seen.height);
    let wide = wide.clamp(1, of.width.saturating_sub(across).max(1));
    let tall = tall.clamp(1, of.height.saturating_sub(down).max(1));

    Ok(Some(Region { from: Point { x: across, y: down }, size: Size { width: wide, height: tall } }))
}

pub fn planned(edit: Edit, region: Option<Region>) -> Result<Planned, Never> {
    Ok(match (edit, region) {
        (Edit::RotateLeft, _) => Planned::Change(Change::RotateLeft),
        (Edit::RotateRight, _) => Planned::Change(Change::RotateRight),
        (Edit::Flip, _) => Planned::Change(Change::Flip),
        (Edit::Crop, Some(region)) => Planned::Change(Change::Crop(region)),
        (Edit::Crop, None) => Planned::ZoomInFirst,
    })
}

pub fn changed(bitmap: &Bitmap, change: Change) -> Result<Bitmap, Never> {
    let Size { width, height } = bitmap.size;
    let Ok(across) = index(width);
    let rows: Vec<&[[u8; 4]]> = bitmap.rgba.as_chunks::<4>().0.chunks_exact(across.max(1)).collect();
    let rows = rows.as_slice();
    let turned = Size { width: height, height: width };

    let (size, pixels): (Size<u32>, Vec<[u8; 4]>) = match change {
        Change::RotateRight => {
            (turned, (0..across).flat_map(|column| rows.iter().rev().filter_map(move |row| row.get(column)).copied()).collect())
        },
        Change::RotateLeft => {
            (turned, (0..across).rev().flat_map(|column| rows.iter().filter_map(move |row| row.get(column)).copied()).collect())
        },
        Change::Flip => (bitmap.size, rows.iter().flat_map(|row| row.iter().rev().copied()).collect()),
        Change::Crop(Region { from, size }) => {
            let left = from.x.min(width);
            let top = from.y.min(height);
            let kept = Size { width: size.width.min(width.saturating_sub(left)), height: size.height.min(height.saturating_sub(top)) };
            let Ok(left) = index(left);
            let Ok(top) = index(top);
            let Ok(wide) = index(kept.width);
            let Ok(tall) = index(kept.height);

            (kept, rows.iter().skip(top).take(tall).flat_map(|row| row.iter().skip(left).take(wide).copied()).collect())
        },
    };

    Ok(Bitmap { size, rgba: pixels.into_flattened() })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Written {
    Jpeg,
    Png,
}

fn written(format: Format) -> Result<Written, Never> {
    Ok(match format {
        Format::Jpeg => Written::Jpeg,
        Format::Png | Format::Gif | Format::Webp => Written::Png,
    })
}

const QUALITY: u8 = 95;

const JPEG_ENDINGS: [&str; 2] = ["jpg", "jpeg"];

const PNG_ENDINGS: [&str; 1] = ["png"];

const EDITED: &str = " edited";

fn ending(at: &Path, written: Written) -> Result<String, Never> {
    let came = at.extension().map(|came| came.to_string_lossy().to_string());

    let (kept, otherwise): (&[&str], &str) = match written {
        Written::Jpeg => (&JPEG_ENDINGS, "jpg"),
        Written::Png => (&PNG_ENDINGS, "png"),
    };

    Ok(match came {
        Some(came) => match kept.contains(&came.to_lowercase().as_str()) {
            true => came,
            false => otherwise.to_string(),
        },
        None => otherwise.to_string(),
    })
}

fn edited_from(stem: &str) -> Result<&str, Never> {
    match stem.strip_suffix(EDITED) {
        Some(from) => return Ok(from),
        None => {},
    }

    Ok(match stem.rsplit_once(' ') {
        Some((before, count)) => match (count.parse::<u32>(), before.strip_suffix(EDITED)) {
            (Ok(_), Some(from)) => from,
            (Ok(_), None) => stem,
            (Err(_not_a_number), _) => stem,
        },
        None => stem,
    })
}

fn beside(at: &Path, written: Written, taken: impl Fn(&Path) -> bool) -> Result<Option<PathBuf>, Never> {
    let stem = match at.file_stem() {
        Some(stem) => stem.to_string_lossy().to_string(),
        None => return Ok(None),
    };
    let Ok(from) = edited_from(&stem);
    let Ok(ending) = ending(at, written);

    Ok((1..=u32::MAX)
        .map(|count| {
            let named = match count {
                1 => format!("{from}{EDITED}.{ending}"),
                _ => format!("{from}{EDITED} {count}.{ending}"),
            };

            at.with_file_name(named)
        })
        .find(|named| !taken(named)))
}

#[derive(Debug)]
pub enum EditError {
    Unsupported(PathBuf),
    Refused(PathBuf, Format),
    Unnamed(PathBuf),
    Jpeg(PathBuf, JpegError),
    Png(PathBuf, PngError),
    Unwritten(Unwritten),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::Unsupported(at) => write!(to, "{}: no decoder here reads this", at.display()),
            EditError::Refused(at, format) => {
                let Ok(says) = format.says();

                write!(to, "{}: the {says} decoder here will not read this one", at.display())
            },
            EditError::Unnamed(at) => write!(to, "{}: no name is left beside it", at.display()),
            EditError::Jpeg(into, fault) => write!(to, "{}: writing the JPEG: {fault}", into.display()),
            EditError::Png(into, fault) => write!(to, "{}: writing the PNG: {fault}", into.display()),
            EditError::Unwritten(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for EditError {}

fn encoded(bitmap: Bitmap, written: (Written, &Path)) -> Result<Vec<u8>, EditError> {
    let Bitmap { size, rgba } = bitmap;
    let (written, into) = written;

    match written {
        Written::Jpeg => {
            let Ok(quality) = Quality::percent(QUALITY);

            console_core_jpeg_files::encoded(&console_core_jpeg_files::Picture { size, rgba }, quality)
                .map_err(|fault| EditError::Jpeg(into.to_path_buf(), fault))
        },
        Written::Png => console_core_png_files::encoded(&console_core_png_files::Picture { size, rgba })
            .map_err(|fault| EditError::Png(into.to_path_buf(), fault)),
    }
}

pub fn saved(at: &Path, change: Change) -> Result<PathBuf, EditError> {
    let Ok(read) = console_pictures::full_size(at);

    let (format, bitmap) = match read {
        FullSize::Decoded(format, bitmap) => (format, bitmap),
        FullSize::Refused(format) => return Err(EditError::Refused(at.to_path_buf(), format)),
        FullSize::Unrecognized => return Err(EditError::Unsupported(at.to_path_buf())),
    };

    let Ok(written) = written(format);
    let Ok(named) = beside(at, written, Path::exists);

    let into = match named {
        Some(into) => into,
        None => return Err(EditError::Unnamed(at.to_path_buf())),
    };

    let Ok(changed) = changed(&bitmap, change);
    let bytes = encoded(changed, (written, &into))?;

    console_core_atomic_writes::whole_without_overwriting(&into, &bytes).map_err(EditError::Unwritten)?;

    Ok(into)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;
    use console_panel::zoom::Zoom;

    fn free_name(at: &str, written: Written, taken: &[&str]) -> Result<Option<String>, Never> {
        let Ok(named) = beside(Path::new(at), written, |asked| taken.iter().any(|taken| Path::new(taken) == asked));

        Ok(named.map(|named| named.to_string_lossy().into_owned()))
    }

    #[test]
    fn an_edit_is_named_after_the_picture_and_never_takes_a_name_that_is_there() {
        assert_eq!(free_name("/p/beach.jpg", Written::Jpeg, &[]), Ok(Some(String::from("/p/beach edited.jpg"))));
        assert_eq!(
            free_name("/p/beach.jpg", Written::Jpeg, &["/p/beach edited.jpg", "/p/beach edited 2.jpg"]),
            Ok(Some(String::from("/p/beach edited 3.jpg")))
        );
    }

    #[test]
    fn an_edit_of_an_edit_counts_on_from_the_picture_it_came_from() {
        assert_eq!(
            free_name("/p/beach edited.jpg", Written::Jpeg, &["/p/beach edited.jpg"]),
            Ok(Some(String::from("/p/beach edited 2.jpg")))
        );
        assert_eq!(
            free_name("/p/beach edited 2.jpg", Written::Jpeg, &["/p/beach edited.jpg", "/p/beach edited 2.jpg"]),
            Ok(Some(String::from("/p/beach edited 3.jpg")))
        );
        assert_eq!(free_name("/p/room 2.jpg", Written::Jpeg, &[]), Ok(Some(String::from("/p/room 2 edited.jpg"))));
    }

    #[test]
    fn a_copy_keeps_the_ending_it_came_with_when_it_is_written_in_the_same_format() {
        assert_eq!(free_name("/p/photo.JPEG", Written::Jpeg, &[]), Ok(Some(String::from("/p/photo edited.JPEG"))));
        assert_eq!(free_name("/p/shot.PNG", Written::Png, &[]), Ok(Some(String::from("/p/shot edited.PNG"))));
        assert_eq!(free_name("/p/wave.gif", Written::Png, &[]), Ok(Some(String::from("/p/wave edited.png"))));
        assert_eq!(free_name("/p/web.webp", Written::Png, &[]), Ok(Some(String::from("/p/web edited.png"))));
    }

    fn framed(zoom: Zoom) -> Result<Framed, Never> {
        Ok(Framed { of: PathBuf::from("/p/beach.jpg"), zoom, room: Size { width: 1280, height: 800 } })
    }

    #[test]
    fn nothing_is_cropped_from_a_picture_that_is_all_on_the_screen() {
        let Ok(whole) = framed(Zoom::default());

        assert_eq!(crop_region(&whole, Size { width: 4000, height: 3000 }), Ok(None));
        assert_eq!(planned(Edit::Crop, None), Ok(Planned::ZoomInFirst));
    }

    #[test]
    fn what_is_kept_is_what_the_screen_showed_and_not_a_square_of_the_zoom() {
        let Ok(twice) = Zoom::default().times(2.0);
        let Ok(twice) = framed(twice);
        let region = crop_region(&twice, Size { width: 4000, height: 3000 });

        assert_eq!(
            region,
            Ok(Some(Region { from: Point { x: 800, y: 750 }, size: Size { width: 2400, height: 1500 } }))
        );
    }

    #[test]
    fn a_crop_never_reaches_past_an_edge_of_the_picture() -> Result<(), &'static str> {
        let Ok(close) = Zoom::default().times(8.0);
        let Ok(far) = close.panned(Point { x: -99_999.0, y: -99_999.0 }, Size { width: 1280, height: 800 });
        let Ok(far) = framed(far);
        let Ok(region) = crop_region(&far, Size { width: 4001, height: 2999 });
        let kept = region.as_ref().ok_or("a zoomed picture keeps something")?;

        assert!(kept.from.x.saturating_add(kept.size.width) <= 4001, "{region:?}");
        assert!(kept.from.y.saturating_add(kept.size.height) <= 2999, "{region:?}");

        Ok(())
    }

    const A: [u8; 4] = [1, 1, 1, 255];
    const B: [u8; 4] = [2, 2, 2, 255];
    const C: [u8; 4] = [3, 3, 3, 255];
    const D: [u8; 4] = [4, 4, 4, 255];
    const E: [u8; 4] = [5, 5, 5, 255];
    const F: [u8; 4] = [6, 6, 6, 255];

    fn three_by_two() -> Result<Bitmap, Never> {
        Ok(Bitmap { size: Size { width: 3, height: 2 }, rgba: [A, B, C, D, E, F].concat() })
    }

    fn reads(size: (u32, u32), pixels: &[[u8; 4]]) -> Result<Bitmap, Never> {
        Ok(Bitmap { size: Size { width: size.0, height: size.1 }, rgba: pixels.concat() })
    }

    #[test]
    fn a_turn_to_the_right_puts_the_left_edge_along_the_top() {
        let Ok(picture) = three_by_two();

        assert_eq!(changed(&picture, Change::RotateRight), reads((2, 3), &[D, A, E, B, F, C]));
    }

    #[test]
    fn a_turn_to_the_left_puts_the_right_edge_along_the_top() {
        let Ok(picture) = three_by_two();

        assert_eq!(changed(&picture, Change::RotateLeft), reads((2, 3), &[C, F, B, E, A, D]));
    }

    #[test]
    fn a_flip_swaps_left_and_right_and_leaves_top_and_bottom() {
        let Ok(picture) = three_by_two();

        assert_eq!(changed(&picture, Change::Flip), reads((3, 2), &[C, B, A, F, E, D]));
    }

    #[test]
    fn a_crop_keeps_the_region_and_nothing_past_the_edge() {
        let Ok(picture) = three_by_two();
        let corner = Region { from: Point { x: 1, y: 1 }, size: Size { width: 9, height: 9 } };

        assert_eq!(changed(&picture, Change::Crop(corner)), reads((2, 1), &[E, F]));
    }

    fn red(size: Size<u32>) -> Result<Vec<u8>, Never> {
        let Ok(area) = index(u64::from(size.width).saturating_mul(u64::from(size.height)));

        Ok([255, 0, 0, 255].repeat(area))
    }

    #[test]
    fn an_edit_is_a_new_file_and_the_picture_it_came_from_is_untouched() -> Result<(), Box<dyn Error>> {
        let folder = console_core_temporary_directories::fresh("viewer-editing")?;
        let at = folder.join("wide.png");
        let size = Size { width: 40, height: 20 };
        let Ok(rgba) = red(size);
        let png = console_core_png_files::encoded(&console_core_png_files::Picture { size, rgba })?;

        console_core_atomic_writes::whole(&at, &png)?;

        let before = std::fs::read(&at)?;
        let once = saved(&at, Change::RotateRight)?;
        let twice = saved(&at, Change::RotateRight)?;
        let after = std::fs::read(&at)?;
        let written = std::fs::read(&once)?;
        let turned = console_core_png_files::measured(&written)?;

        let _ = std::fs::remove_dir_all(&folder);

        assert_eq!(before, after, "the picture that was edited was written over");
        assert_eq!(once, folder.join("wide edited.png"));
        assert_eq!(twice, folder.join("wide edited 2.png"));
        assert_eq!(turned, Size { width: 20, height: 40 }, "the turn was not written");

        Ok(())
    }

    #[test]
    fn a_jpeg_comes_back_a_jpeg() -> Result<(), Box<dyn Error>> {
        let folder = console_core_temporary_directories::fresh("viewer-editing-jpeg")?;
        let at = folder.join("beach.jpg");
        let size = Size { width: 32, height: 16 };
        let Ok(rgba) = red(size);
        let Ok(quality) = Quality::percent(90);
        let jpeg = console_core_jpeg_files::encoded(&console_core_jpeg_files::Picture { size, rgba }, quality)?;

        console_core_atomic_writes::whole(&at, &jpeg)?;

        let into = saved(&at, Change::RotateLeft)?;
        let written = std::fs::read(&into)?;
        let turned = console_core_jpeg_files::measured(&written)?;

        let _ = std::fs::remove_dir_all(&folder);

        assert_eq!(into, folder.join("beach edited.jpg"));
        assert_eq!(turned, Size { width: 16, height: 32 });

        Ok(())
    }

    #[test]
    fn a_format_nothing_here_reads_is_refused_and_nothing_is_written() -> Result<(), Box<dyn Error>> {
        let folder = console_core_temporary_directories::fresh("viewer-editing-heic")?;
        let at = folder.join("phone.HEIC");

        console_core_atomic_writes::whole(&at, b"\0\0\0\x18ftypheic\0\0\0\0mif1heic")?;

        let refused = saved(&at, Change::Flip);
        let listing = std::fs::read_dir(&folder)?;
        let left = listing.map(|entry| entry.map(|entry| entry.path())).collect::<Result<Vec<PathBuf>, std::io::Error>>();
        let left = left?;

        let _ = std::fs::remove_dir_all(&folder);

        assert!(matches!(refused, Err(EditError::Unsupported(_))), "{refused:?}");
        assert_eq!(left, vec![at], "something was written beside it");

        Ok(())
    }
}
