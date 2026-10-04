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
//! **A JPEG, a PNG, a GIF or a WebP is read here, and only handed on if it
//! cannot be.**
//! Most of what the viewer, the thumbnails and the covers meet is a photograph
//! or a screenshot, and two programs started for every one of them is most of
//! the time a folder takes to show. `console-core-jpeg-files` reads a JPEG in
//! this process, at the smallest of its own sizes that covers the room, and the
//! way the camera was held -- which ffprobe does not say, so a portrait was
//! measured lying on its side. What it refuses, CMYK or arithmetic coding, goes
//! to ffmpeg as everything did before. `console-core-png-files` reads a PNG
//! whole, whatever kind it is, `console-core-gif-files` the first frame of a
//! GIF and `console-core-webp-files` the first frame of a WebP, lossy or
//! lossless, and each is drawn down from there. The first twelve bytes of a
//! file say which of them it is.
//!
//! **A picture somebody edits is read at its own size, here or not at all.**
//! [`full_size`] is the same four decoders with nothing behind them: the
//! viewer turns, flips and crops the pixels and writes the copy itself, so a
//! file only ffmpeg reads is one it says it cannot edit yet rather than one it
//! hands to a second program. Which of the four the file was comes back with
//! the pixels, because the copy is written in the format it came in.
//!
//! A photograph drawn into a room a screen wide is drawn on every core, a JPEG
//! and a lossy WebP alike: its bits are read on one, and what comes after the
//! reading of each round of rows -- the blocks, a WebP's loop filter, the
//! colour -- is spread with `console_concurrency::map` while the next is read. The
//! cores are counted once for the picture and handed to every round, because a
//! count is a walk over `/sys` and there are dozens of rounds to a photograph.
//! One drawn smaller than [`SPREAD_FROM`] a side is a thumbnail or an icon, and
//! those arrive by the folder and are already spread a picture to a core, so
//! each of them is drawn on the one it was given, and nothing is counted.
//!
//! A lossy WebP is never held whole on its way to a thumbnail. Its decoder
//! hands over the rows of the part that is wanted -- the square middle, or
//! all of it -- as each is coloured, and they are drawn down as they arrive,
//! so what is shrunk is what libwebp draws, row for row, without the
//! photograph ever being in memory at once. vipsthumbnail, which draws a WebP
//! smaller while it decodes it, is the measure this is held to: no slower, and
//! in no more memory.
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

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use console_concurrency::Cores;
use console_core_external_programs::Program;
use console_core_geometry::{Point, Rectangle, Size};
use console_core_jpeg_files::{JpegError, Task};
use console_core_webp_files::WebpError;
use console_core_never::Never;
use console_core_number_conversion::whole_u32;
use console_core_picture_scaling::{Scaling, Source, scaled};
use console_core_shapes::Pixels;
use console_core_words::Words;

pub const SVG: &str = "svg";

const BYTES_A_PIXEL: u32 = 4;

pub const SPREAD_FROM: u32 = 512;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Fitted(Size<u32>),
    Squared(Size<u32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Format {
    #[words(says = "JPEG")]
    Jpeg,
    #[words(says = "PNG")]
    Png,
    #[words(says = "GIF")]
    Gif,
    #[words(says = "WebP")]
    Webp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Whole {
    Jpeg,
    Png,
    Gif,
}

struct Opened {
    format: Format,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    pub size: Size<u32>,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FullSize {
    Decoded(Format, Bitmap),
    Refused(Format),
    Unrecognized,
}

fn opened(at: &Path) -> Result<Option<Opened>, Never> {
    let mut opening = [0u8; 12];

    let begun = match std::fs::File::open(at) {
        Ok(mut file) => file.read_exact(&mut opening),
        Err(unopened) => Err(unopened),
    };

    let format = match (begun, opening) {
        (Ok(()), [0xFF, 0xD8, 0xFF, ..]) => Format::Jpeg,
        (Ok(()), [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n', ..]) => Format::Png,
        (Ok(()), [b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..]) => Format::Gif,
        (Ok(()), [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P']) => Format::Webp,
        (Ok(()), _) => return Ok(None),
        (Err(_unreadable_so_ffmpeg_will_say_why), _) => return Ok(None),
    };

    Ok(match std::fs::read(at) {
        Ok(bytes) => Some(Opened { format, bytes }),
        Err(_unreadable_so_ffmpeg_will_say_why) => None,
    })
}

fn measured(opened: &Opened) -> Result<Option<Size<u32>>, Never> {
    let bytes = opened.bytes.as_slice();

    Ok(match opened.format {
        Format::Jpeg => match console_core_jpeg_files::measured(bytes) {
            Ok(upright) => Some(upright),
            Err(_unread) => None,
        },
        Format::Png => match console_core_png_files::measured(bytes) {
            Ok(size) => Some(size),
            Err(_unread) => None,
        },
        Format::Gif => match console_core_gif_files::measured(bytes) {
            Ok(size) => Some(size),
            Err(_unread) => None,
        },
        Format::Webp => match console_core_webp_files::measured(bytes) {
            Ok(size) => Some(size),
            Err(_unread) => None,
        },
    })
}

fn drawn(opened: (Whole, &[u8]), covering: Size<u32>) -> Result<Option<Bitmap>, Never> {
    let (format, bytes) = opened;

    Ok(match format {
        Whole::Jpeg => {
            let decoded = match covering.width.max(covering.height) >= SPREAD_FROM {
                true => {
                    let Ok(cores) = Cores::counted();

                    console_core_jpeg_files::decoded_spread(bytes, covering, &|tasks| on_every_core(cores, tasks))
                },
                false => console_core_jpeg_files::decoded(bytes, covering),
            };

            match decoded {
                Ok(picture) => Some(Bitmap { size: picture.size, rgba: picture.rgba }),
                Err(_unread) => None,
            }
        },
        Whole::Png => match console_core_png_files::decoded(bytes) {
            Ok(picture) => Some(Bitmap { size: picture.size, rgba: picture.rgba }),
            Err(_unread) => None,
        },
        Whole::Gif => match console_core_gif_files::decoded(bytes) {
            Ok(picture) => Some(Bitmap { size: picture.size, rgba: picture.rgba }),
            Err(_unread) => None,
        },
    })
}

fn from_opened(opened: Option<&Opened>, shape: Shape) -> Result<Option<Pixels>, Never> {
    let opened = match opened {
        Some(opened) => opened,
        None => return Ok(None),
    };

    let (made, covering) = match shape {
        Shape::Fitted(within) => match measured(opened) {
            Ok(Some(upright)) => {
                let Ok(made) = fit_within(upright, within);

                (made, made)
            },
            Ok(None) => return Ok(None),
        },
        Shape::Squared(size) => {
            let longest = size.width.max(size.height);

            (size, Size { width: longest, height: longest })
        },
    };

    let bytes = opened.bytes.as_slice();
    let wanted = (covering, shape, made);

    let shrunk = match opened.format {
        Format::Jpeg => whole_shrunk((Whole::Jpeg, bytes), wanted),
        Format::Png => whole_shrunk((Whole::Png, bytes), wanted),
        Format::Gif => whole_shrunk((Whole::Gif, bytes), wanted),
        Format::Webp => webp_shrunk(bytes, wanted),
    };

    let rgba = match shrunk {
        Ok(Some(rgba)) => rgba,
        Ok(None) => return Ok(None),
    };

    let stride = made.width.saturating_mul(BYTES_A_PIXEL);

    Ok(Some(Pixels { width: made.width, height: made.height, stride, bytes: Arc::new(rgba) }))
}

fn region(had: Size<u32>, shape: Shape) -> Result<Rectangle<u32>, Never> {
    let side = had.width.min(had.height);

    Ok(match shape {
        Shape::Fitted(_) => Rectangle { origin: Point { x: 0, y: 0 }, size: had },
        Shape::Squared(_) => Rectangle {
            origin: Point { x: had.width.saturating_sub(side).saturating_div(2), y: had.height.saturating_sub(side).saturating_div(2) },
            size: Size { width: side, height: side },
        },
    })
}

fn whole_shrunk(opened: (Whole, &[u8]), wanted: (Size<u32>, Shape, Size<u32>)) -> Result<Option<Vec<u8>>, Never> {
    let (covering, shape, made) = wanted;

    let picture = match drawn(opened, covering) {
        Ok(Some(picture)) => picture,
        Ok(None) => return Ok(None),
    };

    let Ok(region) = region(picture.size, shape);
    let Ok(rgba) = scaled(Source { rgba: &picture.rgba, width: picture.size.width, region }, made);

    Ok(Some(rgba))
}

struct Bands(Scaling);

impl console_core_webp_files::Receiver for Bands {
    fn received(&mut self, rgba: &[u8]) -> Result<(), Never> {
        self.0.received(rgba)
    }
}

fn webp_shrunk(bytes: &[u8], wanted: (Size<u32>, Shape, Size<u32>)) -> Result<Option<Vec<u8>>, Never> {
    let (covering, shape, made) = wanted;

    let had = match console_core_webp_files::measured(bytes) {
        Ok(had) => had,
        Err(_unread) => return Ok(None),
    };

    let Ok(region) = region(had, shape);
    let kept = Rectangle { origin: Point { x: 0, y: 0 }, size: region.size };
    let Ok(scaling) = Scaling::new(region.size.width, kept, made);
    let mut bands = Bands(scaling);

    let decoded = match covering.width.max(covering.height) >= SPREAD_FROM {
        true => {
            let Ok(cores) = Cores::counted();

            console_core_webp_files::decoded_in_bands(bytes, &|tasks| webp_on_every_core(cores, tasks), (region, &mut bands))
        },
        false => console_core_webp_files::decoded_in_bands(bytes, &|tasks| tasks.iter().try_for_each(console_core_webp_files::Task::done), (region, &mut bands)),
    };

    match decoded {
        Ok(drawn) => match drawn == had {
            true => {},
            false => return Ok(None),
        },
        Err(_unread) => return Ok(None),
    }

    let Ok(rgba) = bands.0.finished();

    Ok(Some(rgba))
}

fn on_every_core(cores: Cores, tasks: &[Task<'_>]) -> Result<(), JpegError> {
    spread_out(cores, tasks, (Task::done, JpegError::Corrupt))
}

fn webp_on_every_core(cores: Cores, tasks: &[console_core_webp_files::Task<'_>]) -> Result<(), WebpError> {
    spread_out(cores, tasks, (console_core_webp_files::Task::done, WebpError::Corrupt))
}

type Worker<T, E> = fn(&T) -> Result<(), E>;

fn spread_out<T: Sync, E: Send>(cores: Cores, tasks: &[T], doing: (Worker<T, E>, E)) -> Result<(), E> {
    let (done, panicked) = doing;
    let answered = console_concurrency::map(cores, tasks, done);

    match answered {
        Ok(answers) => answers.into_iter().collect(),
        Err(_a_job_panicked) => Err(panicked),
    }
}

pub fn measure(at: &Path) -> Result<Option<Size<u32>>, PictureError> {
    let Ok(opened) = opened(at);
    let Ok(measured) = opened.as_ref().map_or(Ok(None), measured);

    match measured {
        Some(upright) => return Ok(Some(upright)),
        None => {},
    }

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

pub fn fit_within(had: Size<u32>, within: Size<u32>) -> Result<Size<u32>, Never> {
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

fn extract_frame(at: &Path, filter: &str) -> Result<Option<Vec<u8>>, PictureError> {
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

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "a file this process makes, reads once and deletes, on the device and not in a test, where a directory from `fresh` would be left behind after every picture"
    )
)]
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
    let Ok(opened) = opened(at);
    let Ok(read) = from_opened(opened.as_ref(), Shape::Fitted(within));

    match read {
        Some(pixels) => return Ok(Some(pixels)),
        None => {},
    }

    let measured = measure(at)?;

    let had = match measured {
        Some(had) => had,
        None => return Ok(None),
    };

    let Ok(made) = fit_within(had, within);
    let Size { width: wide, height: tall } = made;

    let drawn = extract_frame(at, &format!("scale={wide}:{tall}"))?;

    let bytes = match drawn {
        Some(bytes) => bytes,
        None => return Ok(None),
    };

    to_pixels(at, made, bytes)
}

fn to_pixels(at: &Path, made: Size<u32>, bytes: Vec<u8>) -> Result<Option<Pixels>, PictureError> {
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
    let (magic, remaining) = match bytes.split_first_chunk::<2>() {
        Some(split) => split,
        None => return Ok(None),
    };

    match magic == b"P6" {
        true => {},
        false => return Ok(None),
    }

    let read = (0..3).try_fold((Vec::new(), remaining), |(mut numbers, remaining), _each| {
        let mut parts = remaining.trim_ascii_start().splitn(2, u8::is_ascii_whitespace);

        let (word, after) = match (parts.next(), parts.next()) {
            (Some(word), Some(after)) => (word, after),
            (None, _) | (_, None) => return None,
        };

        let number = match std::str::from_utf8(word).map(str::parse::<u32>) {
            Ok(Ok(number)) => number,
            Ok(Err(_not_a_number)) => return None,
            Err(_not_text) => return None,
        };

        numbers.push(number);

        Some((numbers, after))
    });

    Ok(match read {
        Some((numbers, remaining)) => match numbers.as_slice() {
            [wide, tall, 255] => Some(ImageHeader { size: Size { width: *wide, height: *tall }, pixels: remaining }),
            _ => None,
        },
        None => None,
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

    for pixel in remaining.as_chunks::<3>().0 {
        rgba.extend_from_slice(pixel);
        rgba.push(u8::MAX);
    }

    let stride = size.width.saturating_mul(BYTES_A_PIXEL);

    Ok(Some(Pixels { width: size.width, height: size.height, stride, bytes: Arc::new(rgba) }))
}

pub fn square(at: &Path, size: Size<u32>) -> Result<Option<Pixels>, PictureError> {
    let Ok(opened) = opened(at);
    let Ok(read) = from_opened(opened.as_ref(), Shape::Squared(size));

    match read {
        Some(pixels) => return Ok(Some(pixels)),
        None => {},
    }

    let measured = measure(at)?;

    let had = match measured {
        Some(had) => had,
        None => return Ok(None),
    };

    let side = had.width.min(had.height);
    let left = had.width.saturating_sub(side).saturating_div(2);
    let top = had.height.saturating_sub(side).saturating_div(2);
    let Size { width: wide, height: tall } = size;

    let drawn = extract_frame(at, &format!("crop={side}:{side}:{left}:{top},scale={wide}:{tall}"))?;

    let bytes = match drawn {
        Some(bytes) => bytes,
        None => return Ok(None),
    };

    to_pixels(at, size, bytes)
}

pub fn full_size(at: &Path) -> Result<FullSize, Never> {
    let Ok(opened) = opened(at);

    let opened = match opened {
        Some(opened) => opened,
        None => return Ok(FullSize::Unrecognized),
    };

    let Ok(measured) = measured(&opened);
    let format = opened.format;

    let upright = match measured {
        Some(upright) => upright,
        None => return Ok(FullSize::Refused(format)),
    };

    let bytes = opened.bytes.as_slice();

    let drawn = match format {
        Format::Jpeg => drawn((Whole::Jpeg, bytes), upright),
        Format::Png => drawn((Whole::Png, bytes), upright),
        Format::Gif => drawn((Whole::Gif, bytes), upright),
        Format::Webp => webp_whole(bytes),
    };

    Ok(match drawn {
        Ok(Some(bitmap)) => FullSize::Decoded(format, bitmap),
        Ok(None) => FullSize::Refused(format),
    })
}

fn webp_whole(bytes: &[u8]) -> Result<Option<Bitmap>, Never> {
    let Ok(cores) = Cores::counted();

    Ok(match console_core_webp_files::decoded_spread(bytes, &|tasks| webp_on_every_core(cores, tasks)) {
        Ok(picture) => Some(Bitmap { size: picture.size, rgba: picture.rgba }),
        Err(_unread) => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

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
            fit_within(Size { width: 100, height: 50 }, Size { width: 64, height: 64 }),
            Ok(Size { width: 64, height: 32 })
        );
        assert_eq!(
            fit_within(Size { width: 50, height: 100 }, Size { width: 64, height: 64 }),
            Ok(Size { width: 32, height: 64 })
        );
    }

    #[test]
    fn a_picture_smaller_than_the_box_is_drawn_up_to_it() {
        assert_eq!(
            fit_within(Size { width: 16, height: 16 }, Size { width: 64, height: 64 }),
            Ok(Size { width: 64, height: 64 })
        );
    }

    #[test]
    fn nothing_is_ever_fitted_to_nothing() {
        assert_eq!(
            fit_within(Size { width: 4000, height: 1 }, Size { width: 64, height: 64 }),
            Ok(Size { width: 64, height: 1 })
        );
        assert_eq!(
            fit_within(Size { width: 0, height: 0 }, Size { width: 64, height: 64 }),
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

    fn make_picture(at: &Path, said: &str) -> Result<(), Never> {
        made_for(at, said, 1)
    }

    #[test]
    fn what_comes_back_is_the_size_this_crate_said_and_the_colour_that_was_written() -> Result<(), Box<dyn Error>> {
        let Ok(at) = beside(Path::new("red"));
        let Ok(()) = make_picture(&at, "color=c=red:s=100x50");

        let read = decoded(&at, Size { width: 20, height: 20 });
        let _ = std::fs::remove_file(&at);

        let read = read?;
        let held = read.ok_or("ffmpeg did not read what ffmpeg wrote")?;

        assert_eq!((held.width, held.height, held.stride), (20, 10, 80));
        assert_eq!(held.bytes.len(), 800);

        let first = held.bytes.get(0..4).map(<[u8]>::to_vec);

        match first {
            Some(said) => match said.as_slice() {
                [red, green, blue, opaque] => {
                    assert!(*red > 200, "red came back as {said:?}");
                    assert!(*green < 40 && *blue < 40, "red came back as {said:?}");
                    assert_eq!(*opaque, 255, "red came back as {said:?}");

                    Ok(())
                }
                _shorter => Err(Box::from(format!("four bytes of red: {said:?}"))),
            },
            None => Err(Box::from("no first pixel at all")),
        }
    }

    #[test]
    fn a_film_comes_back_as_its_first_frame_and_not_as_every_frame_in_it() -> Result<(), Box<dyn Error>> {
        let Ok(at) = beside(Path::new("film"));
        let at = at.with_extension("mkv");
        let Ok(()) = made_for(&at, "testsrc=s=64x48:d=2", 50);

        let read = decoded(&at, Size { width: 32, height: 32 });
        let _ = std::fs::remove_file(&at);

        let read = read?;
        let held = read.ok_or("ffmpeg did not read the film it wrote")?;

        assert_eq!((held.width, held.height), (32, 24));
        assert_eq!(held.bytes.len(), 32_usize.saturating_mul(24).saturating_mul(4));

        Ok(())
    }

    fn made_for(at: &Path, said: &str, frames: u32) -> Result<(), Never> {
        let Ok(mut asking) = Program::Ffmpeg.command();

        let done = asking
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", said, "-frames:v"])
            .arg(frames.to_string())
            .arg(at)
            .status();

        assert!(done.is_ok_and(|how| how.success()), "ffmpeg made no film to read back");

        Ok(())
    }

    #[test]
    fn a_drawing_comes_back_at_the_size_it_was_drawn_for() -> Result<(), Box<dyn Error>> {
        let Ok(at) = beside(Path::new("square"));
        let at = at.with_extension("svg");
        console_core_atomic_writes::whole(
            &at,
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#00ff00"/></svg>"##,
        )?;

        let read = decoded(&at, Size { width: 32, height: 32 });
        let _ = std::fs::remove_file(&at);

        let read = read?;
        let held = read.ok_or("rsvg-convert drew no picture")?;

        assert_eq!((held.width, held.height), (32, 32));
        assert_eq!(held.bytes.get(1), Some(&255), "green: {:?}", held.bytes.get(0..4));

        Ok(())
    }

    #[test]
    fn the_square_middle_is_the_middle_and_not_a_squashed_whole() -> Result<(), Box<dyn Error>> {
        let Ok(at) = beside(Path::new("halves"));
        let Ok(()) = make_picture(&at, "color=c=blue:s=40x20");

        let read = square(&at, Size { width: 10, height: 4 });
        let _ = std::fs::remove_file(&at);

        let read = read?;
        let held = read.ok_or("no square of a picture ffmpeg just wrote")?;

        assert_eq!((held.width, held.height, held.stride), (10, 4, 40));
        assert_eq!(held.bytes.len(), 160);

        Ok(())
    }

    fn photograph(named: &str, bytes: &[u8]) -> Result<PathBuf, Box<dyn Error>> {
        let Ok(at) = beside(Path::new(named));
        let at = at.with_extension("jpg");

        console_core_atomic_writes::whole(&at, bytes)?;

        Ok(at)
    }

    #[test]
    fn a_photograph_held_on_its_side_is_measured_and_drawn_upright() -> Result<(), Box<dyn Error>> {
        let at = photograph("turned", include_bytes!("../../console-core-jpeg-files/tests/pictures/turned.jpg"))?;

        let measured = measure(&at);
        let read = decoded(&at, Size { width: 53, height: 75 });
        let _ = std::fs::remove_file(&at);

        let measured = measured?;

        assert_eq!(measured, Some(Size { width: 53, height: 75 }));

        let read = read?;
        let held = read.ok_or("no photograph")?;
        let expected = include_bytes!("../../console-core-jpeg-files/tests/pictures/turned.rgb");
        let rgb = held.bytes.as_chunks::<4>().0.iter().flat_map(|[red, green, blue, _]| [*red, *green, *blue]);
        let most = rgb.zip(expected).map(|(got, wanted)| got.abs_diff(*wanted)).max();

        assert_eq!((held.width, held.height), (53, 75));
        assert!(most.is_some_and(|most| most <= 8), "{most:?} away from libjpeg");

        Ok(())
    }

    #[test]
    fn a_photograph_comes_back_at_the_size_it_was_fitted_or_squared_to() -> Result<(), Box<dyn Error>> {
        let at = photograph("small", include_bytes!("../../console-core-jpeg-files/tests/pictures/subsampled.jpg"))?;

        let fitted = decoded(&at, Size { width: 20, height: 20 });
        let spread = decoded(&at, Size { width: SPREAD_FROM, height: SPREAD_FROM });
        let squared = square(&at, Size { width: 10, height: 4 });
        let _ = std::fs::remove_file(&at);

        let fitted = fitted?;
        let spread = spread?;
        let squared = squared?;
        let fitted = fitted.ok_or("no fitted photograph")?;
        let spread = spread.ok_or("no photograph spread over the cores")?;
        let squared = squared.ok_or("no square of a photograph")?;

        assert_eq!((fitted.width, fitted.height, fitted.bytes.len()), (20, 14, 1120));
        assert_eq!((spread.width, spread.height), (SPREAD_FROM, 362));
        assert_eq!((squared.width, squared.height, squared.bytes.len()), (10, 4, 160));

        Ok(())
    }

    #[test]
    fn a_png_is_read_here_with_its_alpha_and_measured_from_its_header() -> Result<(), Box<dyn Error>> {
        let Ok(at) = beside(Path::new("see-through"));
        let at = at.with_extension("png");

        console_core_atomic_writes::whole(&at, include_bytes!("../../console-core-png-files/tests/pictures/rgba-8.png"))?;

        let measured = measure(&at);
        let read = decoded(&at, Size { width: 23, height: 17 });
        let squared = square(&at, Size { width: 8, height: 8 });
        let _ = std::fs::remove_file(&at);

        let measured = measured?;
        let read = read?;
        let held = read.ok_or("no PNG")?;
        let squared = squared?;
        let squared = squared.ok_or("no square of a PNG")?;

        assert_eq!(measured, Some(Size { width: 23, height: 17 }));
        assert_eq!((held.width, held.height), (23, 17));
        assert!(held.bytes.as_slice() == include_bytes!("../../console-core-png-files/tests/pictures/rgba-8.rgba"), "not what libpng draws");
        assert_eq!((squared.width, squared.height, squared.bytes.len()), (8, 8, 256));

        Ok(())
    }

    #[test]
    fn a_gif_is_read_here_as_its_first_frame() -> Result<(), Box<dyn Error>> {
        let Ok(at) = beside(Path::new("moving"));
        let at = at.with_extension("gif");

        console_core_atomic_writes::whole(&at, include_bytes!("../../console-core-gif-files/tests/pictures/two-frames.gif"))?;

        let measured = measure(&at);
        let read = decoded(&at, Size { width: 23, height: 17 });
        let _ = std::fs::remove_file(&at);

        let measured = measured?;
        let read = read?;
        let held = read.ok_or("no GIF")?;

        assert_eq!(measured, Some(Size { width: 23, height: 17 }));
        assert!(held.bytes.as_slice() == include_bytes!("../../console-core-gif-files/tests/pictures/two-frames.rgba"), "not the first frame");

        Ok(())
    }

    #[test]
    fn a_lossless_webp_is_read_here_with_its_alpha_and_measured_from_its_header() -> Result<(), Box<dyn Error>> {
        let Ok(at) = beside(Path::new("lossless"));
        let at = at.with_extension("webp");

        console_core_atomic_writes::whole(&at, include_bytes!("../../console-core-webp-files/tests/pictures/alpha.webp"))?;

        let measured = measure(&at);
        let read = decoded(&at, Size { width: 53, height: 29 });
        let _ = std::fs::remove_file(&at);

        let measured = measured?;
        let read = read?;
        let held = read.ok_or("no WebP")?;

        assert_eq!(measured, Some(Size { width: 53, height: 29 }));
        assert_eq!((held.width, held.height), (53, 29));
        assert!(held.bytes.as_slice() == include_bytes!("../../console-core-webp-files/tests/pictures/alpha.rgba"), "not what libwebp draws");

        Ok(())
    }

    #[test]
    fn a_lossy_webp_is_read_here_with_its_alpha_and_measured_from_its_header() -> Result<(), Box<dyn Error>> {
        let Ok(at) = beside(Path::new("lossy"));
        let at = at.with_extension("webp");

        console_core_atomic_writes::whole(&at, include_bytes!("../../console-core-webp-files/tests/pictures/lossy-alpha.webp"))?;

        let measured = measure(&at);
        let read = decoded(&at, Size { width: 45, height: 27 });
        let _ = std::fs::remove_file(&at);

        let measured = measured?;
        let read = read?;
        let held = read.ok_or("no lossy WebP")?;

        assert_eq!(measured, Some(Size { width: 45, height: 27 }));
        assert_eq!((held.width, held.height), (45, 27));
        assert!(held.bytes.as_slice() == include_bytes!("../../console-core-webp-files/tests/pictures/lossy-alpha.rgba"), "not what libwebp draws");

        Ok(())
    }

    #[test]
    fn a_lossy_webp_spread_over_the_cores_is_what_libwebp_draws() -> Result<(), Box<dyn Error>> {
        let Ok(at) = beside(Path::new("lossy-tall"));
        let at = at.with_extension("webp");

        console_core_atomic_writes::whole(&at, include_bytes!("../../console-core-webp-files/tests/pictures/lossy-tall.webp"))?;

        let read = decoded(&at, Size { width: SPREAD_FROM, height: SPREAD_FROM });
        let _ = std::fs::remove_file(&at);

        let read = read?;
        let held = read.ok_or("no lossy WebP spread over the cores")?;
        let drawn = include_bytes!("../../console-core-webp-files/tests/pictures/lossy-tall.rgba");
        let whole = Rectangle { origin: Point { x: 0, y: 0 }, size: Size { width: 48, height: 400 } };
        let Ok(expected) = scaled(Source { rgba: drawn, width: 48, region: whole }, Size { width: held.width, height: held.height });

        assert_eq!((held.width, held.height), (61, SPREAD_FROM));
        assert!(held.bytes.as_slice() == expected.as_slice(), "not what libwebp draws");

        Ok(())
    }

    #[test]
    fn a_lossy_webp_squared_is_the_middle_of_what_libwebp_draws() -> Result<(), Box<dyn Error>> {
        let Ok(at) = beside(Path::new("lossy-tall-squared"));
        let at = at.with_extension("webp");

        console_core_atomic_writes::whole(&at, include_bytes!("../../console-core-webp-files/tests/pictures/lossy-tall.webp"))?;

        let read = square(&at, Size { width: 32, height: 32 });
        let _ = std::fs::remove_file(&at);

        let read = read?;
        let held = read.ok_or("no lossy WebP squared")?;
        let drawn = include_bytes!("../../console-core-webp-files/tests/pictures/lossy-tall.rgba");
        let middle = Rectangle { origin: Point { x: 0, y: 176 }, size: Size { width: 48, height: 48 } };
        let Ok(expected) = scaled(Source { rgba: drawn, width: 48, region: middle }, Size { width: 32, height: 32 });

        assert_eq!((held.width, held.height), (32, 32));
        assert!(held.bytes.as_slice() == expected.as_slice(), "not the middle of what libwebp draws");

        Ok(())
    }

    #[test]
    fn a_jpeg_read_here_refuses_is_read_by_ffmpeg_instead() -> Result<(), Box<dyn Error>> {
        let at = photograph("cmyk", include_bytes!("../../console-core-jpeg-files/tests/pictures/cmyk.jpg"))?;

        let read = decoded(&at, Size { width: 20, height: 20 });
        let _ = std::fs::remove_file(&at);

        let read = read?;
        let held = read.ok_or("ffmpeg did not read a CMYK JPEG")?;

        assert_eq!((held.width, held.height), (20, 14));

        Ok(())
    }

    #[test]
    fn a_file_that_is_not_a_picture_is_no_picture_rather_than_a_fault() -> Result<(), Box<dyn Error>> {
        let Ok(at) = beside(Path::new("words"));
        console_core_atomic_writes::whole(&at, b"this is not a picture")?;

        let read = decoded(&at, Size { width: 64, height: 64 });
        let _ = std::fs::remove_file(&at);

        assert!(matches!(read, Ok(None)), "a file that is not a picture: {read:?}");

        Ok(())
    }
}
