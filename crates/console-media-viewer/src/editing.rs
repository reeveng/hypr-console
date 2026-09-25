//! A picture turned, flipped or cropped, kept as a new file beside the one it
//! was made from.
//!
//! The file a person opened is never written. Every edit is a new picture
//! named after the old one -- `beach edited.jpg`, then `beach edited 2.jpg` --
//! and ffmpeg is told `-n`, so a name that turned up between choosing it and
//! writing it is refused rather than replaced. Editing an edit starts from the
//! name it was edited from, so a third turn of the beach is `beach edited 3`
//! rather than `beach edited edited edited`.
//!
//! There is no crop box. What is kept is what is on the screen: the zoom and
//! the pan already say which part of the picture someone is looking at, and on
//! a machine with sticks and a thumb that is a better way to choose a
//! rectangle than four corners dragged one at a time. [`kept`] is that
//! region in the picture's own pixels, worked out by the same
//! `console_panel::zoom` arithmetic the surface drew it with.
//!
//! The copy is written in the format it came in where ffmpeg can write that
//! format, and as a PNG where it cannot -- a HEIC off a phone, a camera's raw,
//! or a GIF, whose animation one frame of cannot keep anyway.

use std::path::{Path, PathBuf};

use console_core_external_programs::Program;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::whole_u32;
use console_core_words::Words;
use console_panel::zoom::{Framed, Zoomed};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    Is(String),
    ZoomInFirst,
}

pub fn kept(framed: &Framed, of: Size<u32>) -> Result<Option<Region>, Never> {
    let Ok(zoomed) = framed.zoom.zoomed();

    match zoomed {
        Zoomed::Whole => return Ok(None),
        Zoomed::ZoomedIn => {},
    }

    let Ok(placed) = framed.zoom.placed(of, framed.room);
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

pub fn filter(edit: Edit, region: Option<Region>) -> Result<Filter, Never> {
    Ok(match (edit, region) {
        (Edit::RotateLeft, _) => Filter::Is("transpose=cclock".to_string()),
        (Edit::RotateRight, _) => Filter::Is("transpose=clock".to_string()),
        (Edit::Flip, _) => Filter::Is("hflip".to_string()),
        (Edit::Crop, Some(Region { from, size })) => {
            Filter::Is(format!("crop={}:{}:{}:{}", size.width, size.height, from.x, from.y))
        },
        (Edit::Crop, None) => Filter::ZoomInFirst,
    })
}

const WRITTEN_AS_IT_CAME: [&str; 7] = ["jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff"];

const WRITTEN_OTHERWISE: &str = "png";

const EDITED: &str = " edited";

fn ending(at: &Path) -> Result<String, Never> {
    let came = at.extension().map(|came| came.to_string_lossy().to_string());

    Ok(match came {
        Some(came) => match WRITTEN_AS_IT_CAME.contains(&came.to_lowercase().as_str()) {
            true => came,
            false => WRITTEN_OTHERWISE.to_string(),
        },
        None => WRITTEN_OTHERWISE.to_string(),
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

pub fn beside(at: &Path, taken: impl Fn(&Path) -> bool) -> Result<Option<PathBuf>, Never> {
    let stem = match at.file_stem() {
        Some(stem) => stem.to_string_lossy().to_string(),
        None => return Ok(None),
    };
    let Ok(from) = edited_from(&stem);
    let Ok(ending) = ending(at);

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
    Unnamed(PathBuf),
    Query(std::io::Error),
    Rejected(PathBuf, String),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::Unnamed(at) => write!(to, "{}: no name is left beside it", at.display()),
            EditError::Query(fault) => write!(to, "ffmpeg: {fault}"),
            EditError::Rejected(into, said) => write!(to, "{}: ffmpeg wrote nothing: {said}", into.display()),
        }
    }
}

impl std::error::Error for EditError {}

fn compressed(into: &Path) -> Result<&'static [&'static str], Never> {
    let ending = into.extension().map(|ending| ending.to_string_lossy().to_lowercase());

    Ok(match ending.as_deref() {
        Some("jpg" | "jpeg") => &["-q:v", "2"],
        Some(_) | None => &[],
    })
}

pub fn saved(at: &Path, filter: &str) -> Result<PathBuf, EditError> {
    let Ok(named) = beside(at, Path::exists);

    let into = match named {
        Some(into) => into,
        None => return Err(EditError::Unnamed(at.to_path_buf())),
    };

    let Ok(mut asking) = Program::Ffmpeg.command();
    let Ok(quality) = compressed(&into);

    let said = asking
        .args(["-v", "error", "-n", "-i"])
        .arg(at)
        .args(["-vf", filter, "-frames:v", "1", "-update", "1"])
        .args(quality)
        .arg(&into)
        .output()
        .map_err(EditError::Query)?;

    match said.status.success() {
        true => Ok(into),
        false => Err(EditError::Rejected(into, String::from_utf8_lossy(&said.stderr).trim().to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_panel::zoom::Zoom;

    fn named(at: &str, taken: &[&str]) -> Option<String> {
        let Ok(named) = beside(Path::new(at), |asked| taken.iter().any(|taken| Path::new(taken) == asked));

        named.map(|named| named.to_string_lossy().to_string())
    }

    #[test]
    fn an_edit_is_named_after_the_picture_and_never_takes_a_name_that_is_there() {
        assert_eq!(named("/p/beach.jpg", &[]), Some("/p/beach edited.jpg".to_string()));
        assert_eq!(
            named("/p/beach.jpg", &["/p/beach edited.jpg", "/p/beach edited 2.jpg"]),
            Some("/p/beach edited 3.jpg".to_string())
        );
    }

    #[test]
    fn an_edit_of_an_edit_counts_on_from_the_picture_it_came_from() {
        assert_eq!(
            named("/p/beach edited.jpg", &["/p/beach edited.jpg"]),
            Some("/p/beach edited 2.jpg".to_string())
        );
        assert_eq!(
            named("/p/beach edited 2.jpg", &["/p/beach edited.jpg", "/p/beach edited 2.jpg"]),
            Some("/p/beach edited 3.jpg".to_string())
        );
        assert_eq!(named("/p/room 2.jpg", &[]), Some("/p/room 2 edited.jpg".to_string()));
    }

    #[test]
    fn a_format_ffmpeg_cannot_write_comes_back_as_a_png() {
        assert_eq!(named("/p/phone.HEIC", &[]), Some("/p/phone edited.png".to_string()));
        assert_eq!(named("/p/wave.gif", &[]), Some("/p/wave edited.png".to_string()));
        assert_eq!(named("/p/shot.PNG", &[]), Some("/p/shot edited.PNG".to_string()));
    }

    fn framed(zoom: Zoom) -> Framed {
        Framed { of: PathBuf::from("/p/beach.jpg"), zoom, room: Size { width: 1280, height: 800 } }
    }

    #[test]
    fn nothing_is_cropped_from_a_picture_that_is_all_on_the_screen() {
        assert_eq!(kept(&framed(Zoom::default()), Size { width: 4000, height: 3000 }), Ok(None));
        assert_eq!(filter(Edit::Crop, None), Ok(Filter::ZoomInFirst));
    }

    #[test]
    fn what_is_kept_is_what_the_screen_showed_and_not_a_square_of_the_zoom() {
        let Ok(twice) = Zoom::default().times(2.0);
        let region = kept(&framed(twice), Size { width: 4000, height: 3000 });

        assert_eq!(
            region,
            Ok(Some(Region { from: Point { x: 800, y: 750 }, size: Size { width: 2400, height: 1500 } }))
        );
    }

    #[test]
    fn a_crop_never_reaches_past_an_edge_of_the_picture() {
        let Ok(close) = Zoom::default().times(8.0);
        let Ok(far) = close.panned(Point { x: -99_999.0, y: -99_999.0 }, Size { width: 1280, height: 800 });
        let Ok(region) = kept(&framed(far), Size { width: 4001, height: 2999 });
        let Some(Region { from, size }) = region else { panic!("a zoomed picture keeps something") };

        assert!(from.x.saturating_add(size.width) <= 4001, "{region:?}");
        assert!(from.y.saturating_add(size.height) <= 2999, "{region:?}");
    }

    fn made(at: &Path, said: &str) {
        let Ok(mut asking) = Program::Ffmpeg.command();
        let done = asking.args(["-v", "error", "-y", "-f", "lavfi", "-i", said, "-frames:v", "1"]).arg(at).status();

        assert!(done.is_ok_and(|how| how.success()), "ffmpeg made no picture to edit");
    }

    #[test]
    fn an_edit_is_a_new_file_and_the_picture_it_came_from_is_untouched() {
        let folder = std::env::temp_dir().join(format!("console-viewer-editing-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&folder);
        let at = folder.join("wide.png");

        made(&at, "color=c=red:s=40x20");

        let before = std::fs::read(&at).ok();
        let Ok(Filter::Is(turned)) = filter(Edit::RotateRight, None) else { panic!("a turn is a filter") };
        let once = saved(&at, &turned);
        let twice = saved(&at, &turned);
        let after = std::fs::read(&at).ok();
        let shape = once.as_ref().ok().map(|once| console_pictures::measured(once));

        let _ = std::fs::remove_dir_all(&folder);

        assert_eq!(before, after, "the picture that was edited was written over");
        assert_eq!(once.ok(), Some(folder.join("wide edited.png")));
        assert_eq!(twice.ok(), Some(folder.join("wide edited 2.png")));

        match shape {
            Some(Ok(Some(shape))) => assert_eq!(shape, Size { width: 20, height: 40 }, "the turn was not written"),
            Some(Ok(None) | Err(_)) | None => panic!("the edit is not a picture: {shape:?}"),
        }
    }
}
