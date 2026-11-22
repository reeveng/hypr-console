//! A film file, read into what it is and the pictures in it.
//!
//! Every film this desktop showed was read by ffmpeg: ffprobe said how long it
//! was and whether it had sound, and a second ffmpeg wrote its frames down a
//! pipe as raw pixels for the viewer, the thumbnails and the wallpaper to read.
//! What it was doing for them is three things that are each somebody's
//! published work now, held in `crates/`: a container that says where each
//! picture is in the file and when it is shown -- MPEG-4 and QuickTime, or
//! Matroska and WebM -- a decoder for the codec those pictures are written
//! in, and the arithmetic that turns what a decoder hands back into red, green
//! and blue. The first two are filmcraft's and imazen's; the third is here.
//!
//! **Four codecs, because that is what a film is written in.** H.264 is every
//! phone and every camera until a few years ago, HEVC is every phone since,
//! and VP9 and AV1 are what the download panel fetches -- it asks for the
//! smallest file at the height worth having, and that is AV1 wherever a site
//! has it. The AV1 decoder is dav1d's own design carried into Rust with its
//! assembly written as safe SIMD, and it is the one of the four that has to
//! be fast to play a film at all, so it decodes several frames at once.
//!
//! **A picture is handed back as it was decoded.** Luma and two chroma planes,
//! eight bits or more, at the size the film was written; [`Picture::rgba`]
//! turns one into the pixels this desktop draws, upright, with the weights the
//! film says it was made with. Shrinking it is `console-core-picture-scaling`'s,
//! because a thumbnail and a frame of a playing film want the same filter.
//!
//! **Nothing is read before it is wanted.** The file stays on the disk and the
//! containers ask for each picture's bytes where it lies, so a two-hour film
//! costs its index and the pictures being shown rather than its length. A film
//! started from the middle starts at the key picture before that moment and
//! decodes forward to it, handing back nothing before it.

mod colour;
mod decoders;
mod tracks;

use std::fs::File;
use std::path::{Path, PathBuf};

use console_core_byte_sources::FileSource;
use console_core_geometry::Size;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_subtitle_files::Cue;
pub use console_core_subtitle_files::Seconds;

pub use colour::{Bitmap, Picture};
pub use decoders::DecodeFault;
pub use tracks::{ContainerFault, Language, Subtitles, Turn};

use decoders::Decoder;
use tracks::{Container, Video};

#[derive(Debug)]
pub enum FilmError {
  Unopened(PathBuf, std::io::Error),
  Unread(PathBuf, ContainerFault),
  Undecoded(PathBuf, DecodeFault),
}

impl std::fmt::Display for FilmError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      FilmError::Unopened(at, fault) => write!(to, "{} would not open: {fault}", at.display()),
      FilmError::Unread(at, fault) => write!(
        to,
        "{} is not a film that can be read: {fault}",
        at.display()
      ),
      FilmError::Undecoded(at, fault) => write!(
        to,
        "a picture in {} would not decode: {fault}",
        at.display()
      ),
    }
  }
}

impl std::error::Error for FilmError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
  Audible,
  Silent,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the inner fraction measures frames per second"
  )
)]
pub struct FramesPerSecond(pub f64);

#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
  pub seconds: Seconds,
  pub rate: FramesPerSecond,
  pub size: Size<u32>,
  pub sound: Sound,
  pub subtitles: Vec<Subtitles>,
}

pub struct Film {
  at: PathBuf,
  opened: FileSource,
  container: Container,
  video: Option<Video>,
  sound: Sound,
  subtitles: Vec<Subtitles>,
  seconds: Seconds,
}

pub fn open(at: &Path) -> Result<Option<Film>, FilmError> {
  let file = File::open(at).map_err(|fault| FilmError::Unopened(at.to_path_buf(), fault))?;
  let opened =
    FileSource::new(file).map_err(|fault| FilmError::Unopened(at.to_path_buf(), fault))?;
  let read = tracks::read(&opened);

  let read_3 = match read {
    Ok(Some(read_2)) => read_2,
    Ok(None) => return Ok(None),
    Err(fault) => return Err(FilmError::Unread(at.to_path_buf(), fault)),
  };

  Ok(Some(Film {
    at: at.to_path_buf(),
    opened,
    container: read_3.container,
    video: read_3.video,
    sound: read_3.sound,
    subtitles: read_3.subtitles,
    seconds: read_3.seconds,
  }))
}

impl Film {
  pub fn facts(&self) -> Result<Option<Facts>, Never> {
    let video = match &self.video {
      Some(video) => video,
      None => return Ok(None),
    };

    Ok(Some(Facts {
      seconds: self.seconds,
      rate: video.rate,
      size: video.shown,
      sound: self.sound,
      subtitles: self.subtitles.clone(),
    }))
  }

  pub fn pictures(&self, from: Seconds) -> Result<Option<Pictures<'_>>, FilmError> {
    let video = match &self.video {
      Some(video) => video,
      None => return Ok(None),
    };

    let decoder = Decoder::for_codec(&video.codec)
      .map_err(|fault| FilmError::Undecoded(self.at.clone(), fault))?;
    let Ok(start) = video.start_for(from);

    Ok(Some(Pictures {
      film: self,
      video,
      decoder,
      next: start,
      from,
      ready: Vec::new(),
      drained: Drained::No,
    }))
  }

  pub fn cues(&self, subtitles: &Subtitles) -> Result<Vec<Cue>, FilmError> {
    tracks::cues(&self.container, &self.opened, subtitles)
      .map_err(|fault| FilmError::Unread(self.at.clone(), fault))
  }

  pub fn first_picture(&self) -> Result<Option<Picture>, FilmError> {
    let pictures = self.pictures(Seconds(0.0))?;

    match pictures {
      Some(mut pictures_2) => pictures_2.next_picture(),
      None => Ok(None),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drained {
  Yes,
  No,
}

pub struct Pictures<'a> {
  film: &'a Film,
  video: &'a Video,
  decoder: Decoder,
  next: u32,
  from: Seconds,
  ready: Vec<Picture>,
  drained: Drained,
}

impl Pictures<'_> {
  pub fn next_picture(&mut self) -> Result<Option<Picture>, FilmError> {
    let walked = console_core_iteration::iterate(&mut *self, |pictures| {
      let Ok(step) = pictures.step();

      Ok(step)
    });

    match walked {
      Ok(answer) => answer,
      Err(_endless) => Ok(None),
    }
  }

  fn step(&mut self) -> Result<Step<&mut Self, Result<Option<Picture>, FilmError>>, Never> {
    match self.ready.pop() {
      Some(picture) => {
        let Ok(half_a_frame) = self.video.half_a_frame();
        let Ok(at) = picture.at();

        return Ok(match ::core::primitive::bool::then_some(at < self.from.0 - half_a_frame.0, ()) {
          ::core::option::Option::Some(()) => Step::Again(self),
          ::core::option::Option::None => Step::Halt(Ok(Some(picture))),
        });
      }
      None => {}
    }

    match self.drained {
      Drained::Yes => return Ok(Step::Halt(Ok(None))),
      Drained::No => {}
    }

    let fed = self.feed();

    Ok(match fed {
      Ok(()) => Step::Again(self),
      Err(fault) => Step::Halt(Err(fault)),
    })
  }

  fn feed(&mut self) -> Result<(), FilmError> {
    let at = &self.film.at;
    let unit = self
      .video
      .unit(&self.film.container, &self.film.opened, self.next);

    let decoded = match unit {
      Ok(Some((bytes, pts))) => {
        self.next = self.next.saturating_add(1);

        self.decoder.fed(&bytes, pts)
      }
      Ok(None) => {
        self.drained = Drained::Yes;

        self.decoder.drained()
      }
      Err(fault) => return Err(FilmError::Unread(at.clone(), fault)),
    };

    let decoded_2 = decoded.map_err(|fault| FilmError::Undecoded(at.clone(), fault))?;

    for one in decoded_2 {
      let Ok(picture) = one.placed(self.video);

      self.ready.push(picture);
    }

    self.ready.sort_by(|one, other| {
      let (Ok(one_2), Ok(other_2)) = (one.at(), other.at());

      other_2.total_cmp(&one_2)
    });

    Ok(())
  }
}
