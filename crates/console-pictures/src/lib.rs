//! A picture file, read into the pixels this desktop draws.
//!
//! Four programs wanted this and every one of them asked gdk-pixbuf: the menu's
//! icon store, the thumbnails behind a folder, the cover a song is played
//! under, and the viewer. None of them wanted a widget. What they wanted was a
//! width, a height and a run of RGBA, which is what goes in the buffer a
//! compositor reads, so the toolkit was carrying four call sites for a shape
//! that fits in one struct.
//!
//! **What decodes it is this process, and ffmpeg only for what is left.** The
//! formats a photograph, a screenshot and an icon arrive in are read by crates
//! in this tree, and a film's first picture by `console-films`, which
//! reads MPEG-4 and Matroska and the four codecs a film is written in. What
//! none of them reads -- HEIF and AVIF off a phone, TIFF, a film in another
//! container -- is handed to ffmpeg by `stills`, which asks ffprobe for the
//! size first so the run that comes back is a size this crate already knew.
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
//! measured lying on its side. What it refuses, arithmetic coding or twelve
//! bits a sample, goes to ffmpeg as everything did before. `console-core-png-files` reads a PNG
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
//! **A picture this desktop drew itself is a PPM.** A line of header and then
//! the pixels is what [`portable_pixmap`] writes -- a QR code of the Wi-Fi --
//! and [`from_portable_pixmap`] reads it back, so it goes the one way every
//! other file does.
//!
//! A file that neither of them can read is no picture rather than a failure. It
//! is the ordinary case -- a folder of two hundred things is walked for the few
//! that can be shown -- and the callers all had the same sentence for it
//! already. What is a fault is a program that will not run and a run of bytes
//! that is not the length this crate worked out, because both of those are the
//! machine rather than the file.

mod stills;

use std::io::{self, BufRead, Read};
use std::path::{Path, PathBuf};

use console_concurrency::Cores;
use console_core_geometry::{Point, Rectangle, Size};
use console_core_jpeg_files::{JpegError, Task};
use console_core_never::Never;
use console_core_number_conversion::whole_u32;
use console_core_picture_scaling::{Scaling, Source, scaled};
use console_core_shapes::Pixels;
use console_core_webp_files::WebpError;
use console_core_words::Words;
use console_films::FilmError;

pub const SVG: &str = "svg";

pub const SPREAD_FROM: u32 = 512;

#[derive(Debug)]
pub enum PictureError {
  Unread(PathBuf, std::io::Error),
  Unrendered(PathBuf, console_core_scalable_vector_graphics::Error),
  Film(FilmError),
  Query(&'static str, std::io::Error),
  Unmeasured(PathBuf, String),
  Short { of: PathBuf, wanted: u64, got: u64 },
}

impl std::fmt::Display for PictureError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PictureError::Unread(at, fault) => write!(to, "{}: {fault}", at.display()),
      PictureError::Unrendered(at, fault) => write!(to, "{}: drawing it: {fault}", at.display()),
      PictureError::Film(fault) => write!(to, "{fault}"),
      PictureError::Query(program, fault) => write!(to, "{program}: {fault}"),
      PictureError::Unmeasured(at, said) => write!(
        to,
        "{}: nothing says how big this is: {said:?}",
        at.display()
      ),
      PictureError::Short { of, wanted, got } => write!(
        to,
        "{}: {got} bytes of a picture that is {wanted}",
        of.display()
      ),
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

pub use console_core_pixel_buffers::PackedPixels as Bitmap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FullSize {
  Decoded(Format, Bitmap),
  Rejected(Format),
  Unrecognized,
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the fixed header read uses a descriptor checked to be a regular file; waiting on the filesystem is the kernel's responsibility"
  )
)]
fn file_header(at: &Path) -> std::io::Result<[u8; 12]> {
  let listed = std::fs::metadata(at)?;

  match ::core::primitive::bool::then_some(listed.is_file(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
    }
  }

  let mut file = std::fs::File::open(at)?;
  let opened = file.metadata()?;

  match ::core::primitive::bool::then_some(opened.is_file(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
    }
  }

  let mut opening = [0u8; 12];

  file.read_exact(&mut opening)?;

  Ok(opening)
}

fn opened(at: &Path) -> Result<Option<Opened>, Never> {
  let opening = match file_header(at) {
    Ok(opening) => opening,
    Err(_unreadable_so_ffmpeg_will_say_why) => return Ok(None),
  };

  let format = match opening {
    [0xFF, 0xD8, 0xFF, ..] => Format::Jpeg,
    [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n', ..] => Format::Png,
    [b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..] => Format::Gif,
    [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P'] => Format::Webp,
    _ => return Ok(None),
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
      let decoded = match ::core::primitive::bool::then_some(
        covering.width.max(covering.height) >= SPREAD_FROM,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let Ok(cores) = Cores::counted();

          console_core_jpeg_files::decoded_spread(bytes, covering, &|tasks| {
            on_every_core(cores, tasks)
          })
        }
        ::core::option::Option::None => console_core_jpeg_files::decoded(bytes, covering),
      };

      match decoded {
        Ok(picture) => Some(picture),
        Err(_unread) => None,
      }
    }
    Whole::Png => match console_core_png_files::decoded(bytes) {
      Ok(picture) => Some(picture),
      Err(_unread) => None,
    },
    Whole::Gif => match console_core_gif_files::decoded(bytes) {
      Ok(picture) => Some(picture),
      Err(_unread) => None,
    },
  })
}

fn from_opened(opened: Option<&Opened>, shape: Shape) -> Result<Option<Pixels>, Never> {
  let opened_3 = match opened {
    Some(opened_2) => opened_2,
    None => return Ok(None),
  };

  let (made, covering) = match shape {
    Shape::Fitted(within) => match measured(opened_3) {
      Ok(Some(upright)) => {
        let Ok(made) = fit_within(upright, within);

        (made, made)
      }
      Ok(None) => return Ok(None),
    },
    Shape::Squared(size) => {
      let longest = size.width.max(size.height);

      (
        size,
        Size {
          width: longest,
          height: longest,
        },
      )
    }
  };

  let bytes = opened_3.bytes.as_slice();
  let wanted = (covering, shape, made);

  let shrunk = match opened_3.format {
    Format::Jpeg => whole_shrunk((Whole::Jpeg, bytes), wanted),
    Format::Png => whole_shrunk((Whole::Png, bytes), wanted),
    Format::Gif => whole_shrunk((Whole::Gif, bytes), wanted),
    Format::Webp => webp_shrunk(bytes, wanted),
  };

  let bytes_3 = match shrunk {
    Ok(Some(bytes_2)) => bytes_2,
    Ok(None) => return Ok(None),
  };

  Pixels::packed(made, bytes_3).map(Some)
}

fn region(had: Size<u32>, shape: Shape) -> Result<Rectangle<u32>, Never> {
  let side = had.width.min(had.height);

  Ok(match shape {
    Shape::Fitted(_) => Rectangle {
      origin: Point { x: 0, y: 0 },
      size: had,
    },
    Shape::Squared(_) => Rectangle {
      origin: Point {
        x: had.width.saturating_sub(side).saturating_div(2),
        y: had.height.saturating_sub(side).saturating_div(2),
      },
      size: Size {
        width: side,
        height: side,
      },
    },
  })
}

fn whole_shrunk(
  opened: (Whole, &[u8]),
  wanted: (Size<u32>, Shape, Size<u32>),
) -> Result<Option<Vec<u8>>, Never> {
  let (covering, shape, made) = wanted;

  let picture = match drawn(opened, covering) {
    Ok(Some(picture)) => picture,
    Ok(None) => return Ok(None),
  };

  let Ok(region) = region(picture.size, shape);
  let Ok(rgba) = scaled(
    Source {
      rgba: &picture.bytes,
      width: picture.size.width,
      region,
    },
    made,
  );

  Ok(Some(rgba))
}

struct Bands(Scaling);

impl console_core_webp_files::Receiver for Bands {
  fn received(&mut self, rgba: &[u8]) -> Result<(), Never> {
    self.0.received(rgba)
  }
}

fn webp_shrunk(
  bytes: &[u8],
  wanted: (Size<u32>, Shape, Size<u32>),
) -> Result<Option<Vec<u8>>, Never> {
  let (covering, shape, made) = wanted;

  let had = match console_core_webp_files::measured(bytes) {
    Ok(had) => had,
    Err(_unread) => return Ok(None),
  };

  let Ok(region) = region(had, shape);
  let kept = Rectangle {
    origin: Point { x: 0, y: 0 },
    size: region.size,
  };
  let Ok(scaling) = Scaling::new(region.size.width, kept, made);
  let mut bands = Bands(scaling);

  let decoded = match ::core::primitive::bool::then_some(
    covering.width.max(covering.height) >= SPREAD_FROM,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(cores) = Cores::counted();

      console_core_webp_files::decoded_in_bands(
        bytes,
        &|tasks| webp_on_every_core(cores, tasks),
        (region, &mut bands),
      )
    }
    ::core::option::Option::None => console_core_webp_files::decoded_in_bands(
      bytes,
      &|tasks| {
        tasks
          .iter()
          .try_for_each(console_core_webp_files::Task::done)
      },
      (region, &mut bands),
    ),
  };

  match decoded {
    Ok(drawn) => match ::core::primitive::bool::then_some(drawn == had, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(None),
    },
    Err(_unread) => return Ok(None),
  }

  let Ok(rgba) = bands.0.finished();

  Ok(Some(rgba))
}

fn on_every_core(cores: Cores, tasks: &[Task<'_>]) -> Result<(), JpegError> {
  spread_out(cores, tasks, (Task::done, JpegError::Corrupt))
}

fn webp_on_every_core(
  cores: Cores,
  tasks: &[console_core_webp_files::Task<'_>],
) -> Result<(), WebpError> {
  spread_out(
    cores,
    tasks,
    (console_core_webp_files::Task::done, WebpError::Corrupt),
  )
}

type Worker<T, E> = fn(&T) -> Result<(), E>;

fn spread_out<T: Sync, E: Send>(
  cores: Cores,
  tasks: &[T],
  doing: (Worker<T, E>, E),
) -> Result<(), E> {
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
    None => {}
  }

  let filmed = film_measured(at)?;

  match filmed {
    Some(upright) => Ok(Some(upright)),
    None => stills::measured(at),
  }
}

fn film_measured(at: &Path) -> Result<Option<Size<u32>>, PictureError> {
  let film = console_films::open(at);

  let film_3 = match film {
    Ok(Some(film_2)) => film_2,
    Ok(None) => return Ok(None),
    Err(fault) => return unless_decoding(fault),
  };

  let Ok(facts) = film_3.facts();

  Ok(facts.map(|facts_2| facts_2.size))
}

fn unless_decoding<T>(fault: FilmError) -> Result<Option<T>, PictureError> {
  match fault {
    FilmError::Unopened(_, _) | FilmError::Unread(_, _) => Ok(None),
    FilmError::Undecoded(_, _) => Err(PictureError::Film(fault)),
  }
}

pub fn fit_within(had: Size<u32>, within: Size<u32>) -> Result<Size<u32>, Never> {
  let (wide, tall) = (f64::from(had.width), f64::from(had.height));

  match ::core::primitive::bool::then_some(wide > 0.0 && tall > 0.0, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      return Ok(Size {
        width: 1,
        height: 1,
      });
    }
  }

  let across = f64::from(within.width) / wide;
  let down = f64::from(within.height) / tall;
  let by = across.min(down);
  let Ok(made_wide) = whole_u32(wide * by);
  let Ok(made_tall) = whole_u32(tall * by);

  Ok(Size {
    width: made_wide.max(1),
    height: made_tall.max(1),
  })
}

fn first_picture(at: &Path) -> Result<Option<console_films::Picture>, PictureError> {
  let film = console_films::open(at);

  let film_3 = match film {
    Ok(Some(film_2)) => film_2,
    Ok(None) => return Ok(None),
    Err(fault) => return unless_decoding(fault),
  };

  match film_3.first_picture() {
    Ok(picture) => Ok(picture),
    Err(fault) => unless_decoding(fault),
  }
}

fn shrunk(picture: &Bitmap, shape: Shape, made: Size<u32>) -> Result<Pixels, Never> {
  let Ok(region) = region(picture.size, shape);
  let Ok(bytes) = scaled(
    Source {
      rgba: &picture.bytes,
      width: picture.size.width,
      region,
    },
    made,
  );

  Pixels::packed(made, bytes)
}

fn is_a_drawing(at: &Path) -> Result<Rendering, Never> {
  let drawing = at
    .extension()
    .and_then(std::ffi::OsStr::to_str)
    .is_some_and(|ending| ending.eq_ignore_ascii_case(SVG));

  Ok(match ::core::primitive::bool::then_some(drawing, ()) {
    ::core::option::Option::Some(()) => Rendering::AsPaths,
    ::core::option::Option::None => Rendering::AsPixels,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rendering {
  AsPaths,
  AsPixels,
}

fn drawn_from_paths(at: &Path, within: Size<u32>) -> Result<Option<Pixels>, PictureError> {
  let bytes = std::fs::read(at).map_err(|fault| PictureError::Unread(at.to_path_buf(), fault))?;
  let options = console_core_scalable_vector_graphics::Options::default();
  let tree = console_core_scalable_vector_graphics::Tree::from_data(&bytes, &options)
    .map_err(|fault| PictureError::Unrendered(at.to_path_buf(), fault))?;
  let Ok(drawn) = tree.size();
  let Ok(width) = drawn.width();
  let Ok(wide) = whole_u32(f64::from(width));
  let Ok(height) = drawn.height();
  let Ok(tall) = whole_u32(f64::from(height));
  let Ok(made) = fit_within(
    Size {
      width: wide,
      height: tall,
    },
    within,
  );
  let Ok(across) = console_core_number_conversion::fitted::<u32, u16>(made.width);
  let Ok(down) = console_core_number_conversion::fitted::<u32, u16>(made.height);
  let scale = console_core_path_rasterizer::Transform::from_scale_factors(
    ::console_core_path_rasterizer::ScaleFactors {
      x: f32::from(across)
        / ({
          let Ok(value) = drawn.width();
          value
        }),
      y: f32::from(down)
        / ({
          let Ok(value) = drawn.height();
          value
        }),
    },
  );

  let mut pixmap = match {
    let Ok(explicit_value) = console_core_path_rasterizer::Pixmap::new(made.width, made.height);
    explicit_value
  } {
    Some(pixmap) => pixmap,
    None => return Ok(None),
  };

  console_core_scalable_vector_graphics_rasterizer::render(
    &tree,
    {
      let Ok(explicit_value) = scale;
      explicit_value
    },
    &mut ({
      let Ok(value) = pixmap.as_mut();
      value
    }),
  );

  let Ok(bytes_2) = pixmap.take_demultiplied();
  let Ok(pixels) = Pixels::packed(made, bytes_2);

  Ok(Some(pixels))
}

#[cfg(test)]
fn fixture_path(extension: &str) -> Result<PathBuf, console_core_temporary_directories::Unmade> {
  let directory = console_core_temporary_directories::private("picture")?;

  Ok(directory.join("picture").with_extension(extension))
}

pub fn decoded(at: &Path, within: Size<u32>) -> Result<Option<Pixels>, PictureError> {
  let Ok(drawn_as) = is_a_drawing(at);

  match drawn_as {
    Rendering::AsPixels => read(at, within),
    Rendering::AsPaths => drawn_from_paths(at, within),
  }
}

fn read(at: &Path, within: Size<u32>) -> Result<Option<Pixels>, PictureError> {
  let Ok(opened) = opened(at);
  let Ok(read) = from_opened(opened.as_ref(), Shape::Fitted(within));

  match read {
    Some(pixels) => return Ok(Some(pixels)),
    None => {}
  }

  let first = first_picture(at)?;

  let picture = match first {
    Some(picture) => picture,
    None => return stills::fitted(at, within),
  };

  let Ok(had) = picture.size();
  let Ok(made) = fit_within(had, within);

  let Ok(cores) = match ::core::primitive::bool::then_some(
    within.width.max(within.height) >= SPREAD_FROM,
    (),
  ) {
    ::core::option::Option::Some(()) => Cores::counted(),
    ::core::option::Option::None => Cores::one(),
  };

  let Ok(drawn) = picture.color_pixels_at(made, cores);
  let Ok(pixels) = drawn.into_strided();

  Ok(Some(pixels))
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

  match ::core::primitive::bool::then_some(magic == b"P6", ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(None),
  }

  let read = (0..3).try_fold(
    (Vec::new(), remaining),
    |(mut numbers, remaining_2), _each| {
      let mut parts = remaining_2
        .trim_ascii_start()
        .splitn(2, u8::is_ascii_whitespace);

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
    },
  );

  Ok(match read {
    Some((numbers, remaining_3)) => match numbers.as_slice() {
      [wide, tall, 255] => Some(ImageHeader {
        size: Size {
          width: *wide,
          height: *tall,
        },
        pixels: remaining_3,
      }),
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

  let ImageHeader {
    size,
    pixels: remaining,
  } = match header {
    Some(header_2) => header_2,
    None => return Ok(None),
  };

  let wanted = u64::from(size.width)
    .saturating_mul(3)
    .saturating_mul(u64::from(size.height));
  let Ok(got) = console_core_number_conversion::fitted::<_, u64>(remaining.len());

  match ::core::primitive::bool::then_some(got >= wanted, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(None),
  }

  let mut pixels = Vec::new();

  for pixel in remaining.as_chunks::<3>().0 {
    pixels.extend_from_slice(pixel);
    pixels.push(u8::MAX);
  }

  Pixels::packed(size, pixels).map(Some)
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the caller supplies the buffered reader and the camera supplies a deadline for each pipe read"
  )
)]
pub fn read_portable_pixmap(
  reader: &mut impl BufRead,
  limit: Size<u32>,
) -> io::Result<Option<Pixels>> {
  let mut frame = Vec::new();

  for line in 0..3 {
    let Ok(before) = console_core_number_conversion::fixed_index(frame.len());
    let _ = reader.take(64).read_until(b'\n', &mut frame)?;
    let Ok(after) = console_core_number_conversion::fixed_index(frame.len());

    match (after.saturating_sub(before), line, frame.last()) {
      (0, 0, _) => return Ok(None),
      (0, _, _) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
      (_, _, Some(b'\n')) => {}
      (_, _, _) => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "invalid pixmap header line",
        ));
      }
    }
  }

  let Ok(header) = portable_pixmap_header(&frame);
  let header_2 =
    header.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid pixmap header"))?;
  let Size { width, height } = header_2.size;
  let size_is_valid = width != 0
    && height != 0
    && width <= limit.width
    && height <= limit.height
    && header_2.pixels.is_empty();

  match size_is_valid.then_some(()) {
    Some(()) => {}
    None => {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid pixmap dimensions or header",
      ));
    }
  }

  let pixel_count = u64::from(width).saturating_mul(u64::from(height));
  let bytes = pixel_count
    .checked_mul(3)
    .ok_or(io::ErrorKind::InvalidData)?;
  let mut payload = reader.take(bytes);
  let _ = payload.read_to_end(&mut frame)?;

  match payload.limit() {
    0 => {}
    _ => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
  }

  let Ok(pixels) = from_portable_pixmap(&frame);

  Ok(pixels)
}

pub fn square(at: &Path, size: Size<u32>) -> Result<Option<Pixels>, PictureError> {
  let Ok(opened) = opened(at);
  let Ok(read) = from_opened(opened.as_ref(), Shape::Squared(size));

  match read {
    Some(pixels) => return Ok(Some(pixels)),
    None => {}
  }

  let first = first_picture(at)?;

  let picture = match first {
    Some(picture) => picture,
    None => return stills::squared(at, size),
  };

  let Ok(whole) = picture.color_pixels();
  let Ok(pixels) = shrunk(&whole, Shape::Squared(size), size);

  Ok(Some(pixels))
}

pub fn full_size(at: &Path) -> Result<FullSize, Never> {
  let Ok(opened) = opened(at);

  let opened_3 = match opened {
    Some(opened_2) => opened_2,
    None => return Ok(FullSize::Unrecognized),
  };

  let Ok(measured) = measured(&opened_3);
  let format = opened_3.format;

  let upright = match measured {
    Some(upright) => upright,
    None => return Ok(FullSize::Rejected(format)),
  };

  let bytes = opened_3.bytes.as_slice();

  let drawn = match format {
    Format::Jpeg => drawn((Whole::Jpeg, bytes), upright),
    Format::Png => drawn((Whole::Png, bytes), upright),
    Format::Gif => drawn((Whole::Gif, bytes), upright),
    Format::Webp => webp_whole(bytes),
  };

  Ok(match drawn {
    Ok(Some(bitmap)) => FullSize::Decoded(format, bitmap),
    Ok(None) => FullSize::Rejected(format),
  })
}

fn webp_whole(bytes: &[u8]) -> Result<Option<Bitmap>, Never> {
  let Ok(cores) = Cores::counted();

  Ok(
    match console_core_webp_files::decoded_spread(bytes, &|tasks| {
      webp_on_every_core(cores, tasks)
    }) {
      Ok(picture) => Some(picture),
      Err(_unread) => None,
    },
  )
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
    let read_2 = read.map(|pixels| (pixels.width, pixels.height, pixels.bytes.to_vec()));

    assert_eq!(read_2, Some((2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])));
  }

  #[test]
  fn a_pixmap_written_here_is_read_back_as_the_same_picture() {
    let Ok(made) = portable_pixmap(
      Size {
        width: 1,
        height: 2,
      },
      &[0, 0, 0, 255, 255, 255],
    );
    let Ok(read) = from_portable_pixmap(&made);
    let read_2 = read.map(|pixels| (pixels.width, pixels.height, pixels.bytes.to_vec()));

    assert_eq!(read_2, Some((1, 2, vec![0, 0, 0, 255, 255, 255, 255, 255])));
  }

  #[test]
  fn a_page_cut_short_is_no_picture() {
    let Ok(read) = from_portable_pixmap(b"P6\n2 2\n255\n\x01\x02");

    assert_eq!(read, None);
  }

  #[test]
  fn streaming_pixmaps_consume_one_frame_at_a_time() -> io::Result<()> {
    let mut input: &[u8] = b"P6\n1 1\n255\n\x01\x02\x03P6\n1 1\n255\n\x04\x05\x06";
    let limit = Size {
      width: 1,
      height: 1,
    };

    for expected in [[1, 2, 3, 255], [4, 5, 6, 255]] {
      let frame = read_portable_pixmap(&mut input, limit)?;
      let bytes = frame.as_ref().map(|pixels| pixels.bytes.as_ref());

      assert_eq!(bytes, Some(expected.as_slice()));
    }

    let end = read_portable_pixmap(&mut input, limit)?;
    assert_eq!(end, None);

    Ok(())
  }

  #[test]
  fn streaming_pixmaps_reject_a_short_payload() {
    let mut input: &[u8] = b"P6\n1 1\n255\n\x01\x02";
    let result = read_portable_pixmap(
      &mut input,
      Size {
        width: 1,
        height: 1,
      },
    );

    assert_eq!(
      result.map_err(|error| error.kind()),
      Err(io::ErrorKind::UnexpectedEof)
    );
  }

  #[test]
  fn a_picture_fits_inside_the_box_it_was_given_without_stretching() {
    assert_eq!(
      fit_within(
        Size {
          width: 100,
          height: 50
        },
        Size {
          width: 64,
          height: 64
        }
      ),
      Ok(Size {
        width: 64,
        height: 32
      })
    );
    assert_eq!(
      fit_within(
        Size {
          width: 50,
          height: 100
        },
        Size {
          width: 64,
          height: 64
        }
      ),
      Ok(Size {
        width: 32,
        height: 64
      })
    );
  }

  #[test]
  fn a_picture_smaller_than_the_box_is_drawn_up_to_it() {
    assert_eq!(
      fit_within(
        Size {
          width: 16,
          height: 16
        },
        Size {
          width: 64,
          height: 64
        }
      ),
      Ok(Size {
        width: 64,
        height: 64
      })
    );
  }

  #[test]
  fn nothing_is_ever_fitted_to_nothing() {
    assert_eq!(
      fit_within(
        Size {
          width: 4000,
          height: 1
        },
        Size {
          width: 64,
          height: 64
        }
      ),
      Ok(Size {
        width: 64,
        height: 1
      })
    );
    assert_eq!(
      fit_within(
        Size {
          width: 0,
          height: 0
        },
        Size {
          width: 64,
          height: 64
        }
      ),
      Ok(Size {
        width: 1,
        height: 1
      })
    );
  }

  #[test]
  fn a_drawing_is_the_one_kind_that_is_drawn_before_it_is_read() {
    assert_eq!(
      is_a_drawing(Path::new("/x/firefox.svg")),
      Ok(Rendering::AsPaths)
    );
    assert_eq!(
      is_a_drawing(Path::new("/x/firefox.SVG")),
      Ok(Rendering::AsPaths)
    );
    assert_eq!(
      is_a_drawing(Path::new("/x/beach.jpg")),
      Ok(Rendering::AsPixels)
    );
    assert_eq!(is_a_drawing(Path::new("/x/beach")), Ok(Rendering::AsPixels));
  }

  fn make_picture(at: &Path, size: Size<u32>, colour: [u8; 4]) -> Result<(), Box<dyn Error>> {
    let area = u64::from(size.width).saturating_mul(u64::from(size.height));
    let Ok(area_2) = console_core_number_conversion::index(area);
    let rgba = colour.repeat(area_2);
    let written = console_core_png_files::encoded(&Bitmap { size, bytes: rgba })?;

    console_core_atomic_writes::whole(at, &written)?;

    Ok(())
  }

  fn remove_fixture(at: &Path) -> std::io::Result<()> {
    let directory = at.parent().ok_or(std::io::ErrorKind::InvalidInput)?;

    std::fs::remove_file(at)?;
    std::fs::remove_dir(directory)
  }

  #[test]
  fn what_comes_back_is_the_size_this_crate_said_and_the_colour_that_was_written()
  -> Result<(), Box<dyn Error>> {
    let at = fixture_path("png")?;
    make_picture(
      &at,
      Size {
        width: 100,
        height: 50,
      },
      [255, 0, 0, 255],
    )?;

    let read = decoded(
      &at,
      Size {
        width: 20,
        height: 20,
      },
    );
    remove_fixture(&at)?;

    let read_2 = read?;
    let held = read_2.ok_or("the picture just written did not read back")?;

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
  fn a_film_comes_back_as_its_first_frame_and_not_as_every_frame_in_it()
  -> Result<(), Box<dyn Error>> {
    let at = fixture_path("mkv")?;

    console_core_atomic_writes::whole(
      &at,
      include_bytes!("../../console-films/tests/films/red-then-blue.av1.mkv"),
    )?;

    let measured = measure(&at);
    let read = decoded(
      &at,
      Size {
        width: 48,
        height: 48,
      },
    );
    remove_fixture(&at)?;

    let measured_2 = measured?;
    let read_2 = read?;
    let held = read_2.ok_or("the film came back as no picture")?;

    assert_eq!(
      measured_2,
      Some(Size {
        width: 96,
        height: 64
      })
    );
    assert_eq!((held.width, held.height), (48, 32));
    assert_eq!(
      held.bytes.len(),
      48_usize.saturating_mul(32).saturating_mul(4)
    );

    let [red, _, blue, _] = held
      .bytes
      .first_chunk::<4>()
      .copied()
      .ok_or("no first pixel")?;

    assert!(
      red > 200 && blue < 60,
      "the first picture is red and came back as {red} red and {blue} blue"
    );

    Ok(())
  }

  #[test]
  fn a_drawing_comes_back_at_the_size_it_was_drawn_for() -> Result<(), Box<dyn Error>> {
    let at_2 = fixture_path("svg")?;
    console_core_atomic_writes::whole(
            &at_2,
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#00ff00"/></svg>"##,
        )?;

    let read = decoded(
      &at_2,
      Size {
        width: 32,
        height: 32,
      },
    );
    remove_fixture(&at_2)?;

    let read_2 = read?;
    let held = read_2.ok_or("the drawing came back as no picture")?;

    assert_eq!((held.width, held.height), (32, 32));
    assert_eq!(
      held.bytes.get(1),
      Some(&255),
      "green: {:?}",
      held.bytes.get(0..4)
    );

    Ok(())
  }

  #[test]
  fn a_tall_drawing_keeps_its_shape_inside_a_square() -> Result<(), Box<dyn Error>> {
    let at_2 = fixture_path("svg")?;
    console_core_atomic_writes::whole(
            &at_2,
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 20"><rect width="10" height="20" fill="#ff0000"/></svg>"##,
        )?;

    let read = decoded(
      &at_2,
      Size {
        width: 32,
        height: 32,
      },
    );
    remove_fixture(&at_2)?;

    let read_2 = read?;
    let held = read_2.ok_or("the drawing came back as no picture")?;

    assert_eq!((held.width, held.height), (16, 32));
    assert_eq!(held.bytes.first_chunk::<4>(), Some(&[255_u8, 0, 0, 255]));

    Ok(())
  }

  #[test]
  fn the_square_middle_is_the_middle_and_not_a_squashed_whole() -> Result<(), Box<dyn Error>> {
    let at = fixture_path("png")?;
    make_picture(
      &at,
      Size {
        width: 40,
        height: 20,
      },
      [0, 0, 255, 255],
    )?;

    let read = square(
      &at,
      Size {
        width: 10,
        height: 4,
      },
    );
    remove_fixture(&at)?;

    let read_2 = read?;
    let held = read_2.ok_or("no square of a picture just written")?;

    assert_eq!((held.width, held.height, held.stride), (10, 4, 40));
    assert_eq!(held.bytes.len(), 160);

    Ok(())
  }

  fn photograph(bytes: &[u8]) -> Result<PathBuf, Box<dyn Error>> {
    let at = fixture_path("jpg")?;

    console_core_atomic_writes::whole(&at, bytes)?;

    Ok(at)
  }

  #[test]
  fn a_photograph_held_on_its_side_is_measured_and_drawn_upright() -> Result<(), Box<dyn Error>> {
    let at = photograph(include_bytes!(
      "../../console-core-jpeg-files/tests/pictures/turned.jpg"
    ))?;

    let measured = measure(&at);
    let read = decoded(
      &at,
      Size {
        width: 53,
        height: 75,
      },
    );
    remove_fixture(&at)?;

    let measured_2 = measured?;

    assert_eq!(
      measured_2,
      Some(Size {
        width: 53,
        height: 75
      })
    );

    let read_2 = read?;
    let held = read_2.ok_or("no photograph")?;
    let expected = include_bytes!("../../console-core-jpeg-files/tests/pictures/turned.rgb");
    let rgb = held
      .bytes
      .as_chunks::<4>()
      .0
      .iter()
      .flat_map(|[red, green, blue, _]| [*red, *green, *blue]);
    let most = rgb
      .zip(expected)
      .map(|(got, wanted)| got.abs_diff(*wanted))
      .max();

    assert_eq!((held.width, held.height), (53, 75));
    assert!(
      most.is_some_and(|difference| difference <= 8),
      "{most:?} away from libjpeg"
    );

    Ok(())
  }

  #[test]
  fn a_photograph_comes_back_at_the_size_it_was_fitted_or_squared_to()
  -> Result<(), Box<dyn Error>> {
    let at = photograph(include_bytes!(
      "../../console-core-jpeg-files/tests/pictures/subsampled.jpg"
    ))?;

    let fitted = decoded(
      &at,
      Size {
        width: 20,
        height: 20,
      },
    );
    let spread = decoded(
      &at,
      Size {
        width: SPREAD_FROM,
        height: SPREAD_FROM,
      },
    );
    let squared = square(
      &at,
      Size {
        width: 10,
        height: 4,
      },
    );
    remove_fixture(&at)?;

    let fitted_2 = fitted?;
    let spread_2 = spread?;
    let squared_2 = squared?;
    let fitted_3 = fitted_2.ok_or("no fitted photograph")?;
    let spread_3 = spread_2.ok_or("no photograph spread over the cores")?;
    let squared_3 = squared_2.ok_or("no square of a photograph")?;

    assert_eq!(
      (fitted_3.width, fitted_3.height, fitted_3.bytes.len()),
      (20, 14, 1120)
    );
    assert_eq!((spread_3.width, spread_3.height), (SPREAD_FROM, 362));
    assert_eq!(
      (squared_3.width, squared_3.height, squared_3.bytes.len()),
      (10, 4, 160)
    );

    Ok(())
  }

  #[test]
  fn a_png_is_read_here_with_its_alpha_and_measured_from_its_header() -> Result<(), Box<dyn Error>>
  {
    let at = fixture_path("png")?;

    console_core_atomic_writes::whole(
      &at,
      include_bytes!("../../console-core-png-files/tests/pictures/rgba-8.png"),
    )?;

    let measured = measure(&at);
    let read = decoded(
      &at,
      Size {
        width: 23,
        height: 17,
      },
    );
    let squared = square(
      &at,
      Size {
        width: 8,
        height: 8,
      },
    );
    remove_fixture(&at)?;

    let measured_2 = measured?;
    let read_2 = read?;
    let held = read_2.ok_or("no PNG")?;
    let squared_2 = squared?;
    let squared_3 = squared_2.ok_or("no square of a PNG")?;

    assert_eq!(
      measured_2,
      Some(Size {
        width: 23,
        height: 17
      })
    );
    assert_eq!((held.width, held.height), (23, 17));
    assert!(
      held.bytes.as_slice()
        == include_bytes!("../../console-core-png-files/tests/pictures/rgba-8.rgba"),
      "not what libpng draws"
    );
    assert_eq!(
      (squared_3.width, squared_3.height, squared_3.bytes.len()),
      (8, 8, 256)
    );

    Ok(())
  }

  #[test]
  fn a_gif_is_read_here_as_its_first_frame() -> Result<(), Box<dyn Error>> {
    let at_2 = fixture_path("gif")?;

    console_core_atomic_writes::whole(
      &at_2,
      include_bytes!("../../console-core-gif-files/tests/pictures/two-frames.gif"),
    )?;

    let measured = measure(&at_2);
    let read = decoded(
      &at_2,
      Size {
        width: 23,
        height: 17,
      },
    );
    remove_fixture(&at_2)?;

    let measured_2 = measured?;
    let read_2 = read?;
    let held = read_2.ok_or("no GIF")?;

    assert_eq!(
      measured_2,
      Some(Size {
        width: 23,
        height: 17
      })
    );
    assert!(
      held.bytes.as_slice()
        == include_bytes!("../../console-core-gif-files/tests/pictures/two-frames.rgba"),
      "not the first frame"
    );

    Ok(())
  }

  #[test]
  fn a_lossless_webp_is_read_here_with_its_alpha_and_measured_from_its_header()
  -> Result<(), Box<dyn Error>> {
    let at = fixture_path("webp")?;

    console_core_atomic_writes::whole(
      &at,
      include_bytes!("../../console-core-webp-files/tests/pictures/alpha.webp"),
    )?;

    let measured = measure(&at);
    let read = decoded(
      &at,
      Size {
        width: 53,
        height: 29,
      },
    );
    remove_fixture(&at)?;

    let measured_2 = measured?;
    let read_2 = read?;
    let held = read_2.ok_or("no WebP")?;

    assert_eq!(
      measured_2,
      Some(Size {
        width: 53,
        height: 29
      })
    );
    assert_eq!((held.width, held.height), (53, 29));
    assert!(
      held.bytes.as_slice()
        == include_bytes!("../../console-core-webp-files/tests/pictures/alpha.rgba"),
      "not what libwebp draws"
    );

    Ok(())
  }

  #[test]
  fn a_lossy_webp_is_read_here_with_its_alpha_and_measured_from_its_header()
  -> Result<(), Box<dyn Error>> {
    let at = fixture_path("webp")?;

    console_core_atomic_writes::whole(
      &at,
      include_bytes!("../../console-core-webp-files/tests/pictures/lossy-alpha.webp"),
    )?;

    let measured = measure(&at);
    let read = decoded(
      &at,
      Size {
        width: 45,
        height: 27,
      },
    );
    remove_fixture(&at)?;

    let measured_2 = measured?;
    let read_2 = read?;
    let held = read_2.ok_or("no lossy WebP")?;

    assert_eq!(
      measured_2,
      Some(Size {
        width: 45,
        height: 27
      })
    );
    assert_eq!((held.width, held.height), (45, 27));
    assert!(
      held.bytes.as_slice()
        == include_bytes!("../../console-core-webp-files/tests/pictures/lossy-alpha.rgba"),
      "not what libwebp draws"
    );

    Ok(())
  }

  #[test]
  fn a_lossy_webp_spread_over_the_cores_is_what_libwebp_draws() -> Result<(), Box<dyn Error>> {
    let at = fixture_path("webp")?;

    console_core_atomic_writes::whole(
      &at,
      include_bytes!("../../console-core-webp-files/tests/pictures/lossy-tall.webp"),
    )?;

    let read = decoded(
      &at,
      Size {
        width: SPREAD_FROM,
        height: SPREAD_FROM,
      },
    );
    remove_fixture(&at)?;

    let read_2 = read?;
    let held = read_2.ok_or("no lossy WebP spread over the cores")?;
    let drawn = include_bytes!("../../console-core-webp-files/tests/pictures/lossy-tall.rgba");
    let whole = Rectangle {
      origin: Point { x: 0, y: 0 },
      size: Size {
        width: 48,
        height: 400,
      },
    };
    let Ok(expected) = scaled(
      Source {
        rgba: drawn,
        width: 48,
        region: whole,
      },
      Size {
        width: held.width,
        height: held.height,
      },
    );

    assert_eq!((held.width, held.height), (61, SPREAD_FROM));
    assert!(
      held.bytes.as_slice() == expected.as_slice(),
      "not what libwebp draws"
    );

    Ok(())
  }

  #[test]
  fn a_lossy_webp_squared_is_the_middle_of_what_libwebp_draws() -> Result<(), Box<dyn Error>> {
    let at = fixture_path("webp")?;

    console_core_atomic_writes::whole(
      &at,
      include_bytes!("../../console-core-webp-files/tests/pictures/lossy-tall.webp"),
    )?;

    let read = square(
      &at,
      Size {
        width: 32,
        height: 32,
      },
    );
    remove_fixture(&at)?;

    let read_2 = read?;
    let held = read_2.ok_or("no lossy WebP squared")?;
    let drawn = include_bytes!("../../console-core-webp-files/tests/pictures/lossy-tall.rgba");
    let middle = Rectangle {
      origin: Point { x: 0, y: 176 },
      size: Size {
        width: 48,
        height: 48,
      },
    };
    let Ok(expected) = scaled(
      Source {
        rgba: drawn,
        width: 48,
        region: middle,
      },
      Size {
        width: 32,
        height: 32,
      },
    );

    assert_eq!((held.width, held.height), (32, 32));
    assert!(
      held.bytes.as_slice() == expected.as_slice(),
      "not the middle of what libwebp draws"
    );

    Ok(())
  }

  #[test]
  fn a_jpeg_in_ink_is_read_here_too() -> Result<(), Box<dyn Error>> {
    let at = photograph(include_bytes!(
      "../../console-core-jpeg-files/tests/pictures/cmyk.jpg"
    ))?;

    let read = decoded(
      &at,
      Size {
        width: 20,
        height: 20,
      },
    );
    remove_fixture(&at)?;

    let read_2 = read?;
    let held = read_2.ok_or("a CMYK JPEG came back as no picture")?;

    assert_eq!((held.width, held.height), (20, 14));

    Ok(())
  }

  #[test]
  fn a_file_that_is_not_a_picture_is_no_picture_rather_than_a_fault() -> Result<(), Box<dyn Error>>
  {
    let at = fixture_path("png")?;
    console_core_atomic_writes::whole(&at, b"this is not a picture")?;

    let read = decoded(
      &at,
      Size {
        width: 64,
        height: 64,
      },
    );
    remove_fixture(&at)?;

    assert!(
      matches!(read, Ok(None)),
      "a file that is not a picture: {read:?}"
    );

    Ok(())
  }
}
