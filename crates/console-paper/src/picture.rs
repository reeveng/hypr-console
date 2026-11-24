//! Bounded file loading and a single decoded canvas for all outputs.

use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_webp_files::{Animation, Picture, Playback, measured};
use console_paper::Motion;

use crate::play::{Load, Timing};

const MOST_FILE_BYTES: u32 = 64 * 1024 * 1024;
const MOST_PIXELS: u64 = 8 * 1024 * 1024;
const MOST_SIDE: u32 = 8192;

#[derive(PartialEq, Eq)]
struct FileVersion {
  device: u64,
  inode: u64,
  bytes: u64,
  updated: (i64, i64),
  changed: (i64, i64),
}

impl FileVersion {
  fn read(metadata: &Metadata) -> Result<Self, Never> {
    Ok(Self {
      device: metadata.dev(),
      inode: metadata.ino(),
      bytes: metadata.len(),
      updated: (metadata.mtime(), metadata.mtime_nsec()),
      changed: (metadata.ctime(), metadata.ctime_nsec()),
    })
  }
}

enum Content {
  Still,
  Animated {
    asset: Animation,
    delay: Option<Duration>,
  },
}

struct Named {
  at: PathBuf,
  version: FileVersion,
  motion: Motion,
  content: Content,
}

impl Named {
  fn timing(&self) -> Result<Timing, Never> {
    Ok(match (&self.content, self.motion) {
      (
        Content::Animated {
          delay: Some(delay), ..
        },
        Motion::Play,
      ) => Timing::Moving(*delay),
      (Content::Still, _)
      | (Content::Animated { delay: None, .. }, _)
      | (Content::Animated { delay: Some(_), .. }, Motion::Freeze) => Timing::Still,
    })
  }
}

pub struct Drawing {
  ground: [u8; 3],
  color: [u8; 3],
  named: Option<Named>,
  pub size: Size<u32>,
  pub bytes: Vec<u8>,
  pub revision: u64,
  pub content: u64,
}

impl Drawing {
  pub fn new(ground: [u8; 3]) -> Result<Self, Never> {
    let [red, green, blue] = ground;
    Ok(Self {
      ground,
      color: ground,
      named: None,
      size: Size {
        width: 1,
        height: 1,
      },
      bytes: vec![blue, green, red, 255],
      revision: 1,
      content: 1,
    })
  }

  pub fn clear(&mut self, color: [u8; 3]) -> Result<(), Never> {
    let [red, green, blue] = color;
    self.color = color;
    self.named = None;
    self.size = Size {
      width: 1,
      height: 1,
    };
    self.bytes = vec![blue, green, red, 255];
    self.revision = self.revision.wrapping_add(1);
    self.content = self.content.wrapping_add(1);
    Ok(())
  }

  pub fn load(&mut self, at: PathBuf, motion: Motion) -> Result<Load, Never> {
    Ok(match self.open(at, motion) {
      Ok(load) => load,
      Err(fault) => Load::Refused(fault.to_string()),
    })
  }

  fn open(&mut self, at: PathBuf, motion: Motion) -> io::Result<Load> {
    let (file, version) = open(&at)?;

    match self.named.as_mut() {
      Some(named) => match (named.at == at, named.version == version) {
        (true, true) => {
          let Ok(updated) = change_motion(named, motion);
          return Ok(updated);
        }
        _ => {}
      },
      None => {}
    }

    let file_bytes = console_core_bounded_reads::bytes(&file, MOST_FILE_BYTES)?;
    let size = measured(&file_bytes).map_err(io::Error::other)?;

    match (size.width, size.height) {
      (1..=MOST_SIDE, 1..=MOST_SIDE) => {}
      _ => {
        return Err(io::Error::other(
          "wallpaper dimensions exceed the playback limit",
        ));
      }
    }

    let pixels = u64::from(size.width).saturating_mul(u64::from(size.height));

    let metadata = file.metadata()?;
    let Ok(after) = FileVersion::read(&metadata);

    match (pixels, after == version) {
      (1..=MOST_PIXELS, true) => {}
      (1..=MOST_PIXELS, false) => {
        return Err(io::Error::other("wallpaper changed during reading"));
      }
      _ => {
        return Err(io::Error::other(
          "wallpaper dimensions exceed the playback limit",
        ));
      }
    }

    let content = self.decode(file_bytes)?;
    let named = Named {
      at,
      version,
      motion,
      content,
    };
    let Ok(timing) = named.timing();
    self.named = Some(named);
    self.revision = self.revision.wrapping_add(1);
    self.content = self.content.wrapping_add(1);
    Ok(Load::Ready(timing))
  }

  fn decode(&mut self, file: Vec<u8>) -> io::Result<Content> {
    let playback = Playback::open(file, self.ground).map_err(io::Error::other)?;

    match playback {
      Playback::Still(picture) => {
        self.size = picture.size;
        let mut pixels = picture.bytes;

        for rgba in pixels.as_chunks_mut::<4>().0 {
          let [red, green, blue, alpha] = *rgba;
          *rgba = [blue, green, red, alpha];
        }

        self.bytes = pixels;
        Ok(Content::Still)
      }
      Playback::Animated(mut asset) => {
        let delay = asset.advance().map_err(io::Error::other)?;
        let Ok(picture) = asset.canvas();
        let bytes = pixels_for(picture, Vec::new())?;
        self.size = picture.size;
        self.bytes = bytes;
        Ok(Content::Animated { asset, delay })
      }
    }
  }

  pub fn advance(&mut self) -> Result<Timing, Never> {
    let (asset, delay, at) = match self.named.as_mut() {
      Some(Named {
        content: Content::Animated { asset, delay },
        at,
        ..
      }) => (asset, delay, at),
      Some(Named {
        content: Content::Still,
        ..
      })
      | None => return Ok(Timing::Still),
    };

    Ok(match asset.advance() {
      Ok(Some(long)) => {
        let Ok(picture) = asset.canvas();

        let Ok(()) = update_pixels(picture, &mut self.bytes);
        self.revision = self.revision.wrapping_add(1);
        *delay = Some(long);
        Timing::Moving(long)
      }
      Ok(None) => {
        *delay = None;
        Timing::Still
      }
      Err(fault) => {
        console_core_logging::warn!(
          "wallpaper {} stopped at its last good frame: {fault}",
          at.display()
        );
        *delay = None;
        Timing::Still
      }
    })
  }

  pub fn describing(&self) -> Result<String, Never> {
    Ok(match &self.named {
      Some(named) => format!("image: {}", named.at.display()),
      None => {
        let [red, green, blue] = self.color;
        format!("color: #{red:02x}{green:02x}{blue:02x}")
      }
    })
  }
}

fn open(at: &Path) -> io::Result<(File, FileVersion)> {
  let flags = rustix::fs::OFlags::NONBLOCK.bits();
  let Ok(nonblocking) = console_core_number_conversion::fitted::<_, i32>(flags);
  let file = OpenOptions::new()
    .read(true)
    .custom_flags(nonblocking)
    .open(at)?;
  let metadata = file.metadata()?;

  match (
    metadata.is_file(),
    metadata.len().checked_sub(1),
    metadata.len().cmp(&u64::from(MOST_FILE_BYTES)),
  ) {
    (true, Some(_), std::cmp::Ordering::Less | std::cmp::Ordering::Equal) => {
      let Ok(version) = FileVersion::read(&metadata);
      Ok((file, version))
    }
    _ => Err(io::Error::other(
      "wallpaper must be a nonempty, regular WebP within the playback limit",
    )),
  }
}

fn change_motion(named: &mut Named, motion: Motion) -> Result<Load, Never> {
  match (named.motion, motion) {
    (Motion::Play, Motion::Play) | (Motion::Freeze, Motion::Freeze) => Ok(Load::Unchanged),
    (Motion::Play, Motion::Freeze) | (Motion::Freeze, Motion::Play) => {
      named.motion = motion;
      let Ok(timing) = named.timing();
      Ok(Load::Ready(timing))
    }
  }
}

fn pixels_for(picture: &Picture, mut buffer: Vec<u8>) -> io::Result<Vec<u8>> {
  buffer
    .try_reserve_exact(picture.bytes.len())
    .map_err(io::Error::other)?;
  buffer.resize(picture.bytes.len(), 0);
  let Ok(()) = update_pixels(picture, &mut buffer);
  Ok(buffer)
}

fn update_pixels(picture: &Picture, buffer: &mut [u8]) -> Result<(), Never> {
  for (pixel, [red, green, blue, _]) in buffer
    .as_chunks_mut::<4>()
    .0
    .iter_mut()
    .zip(picture.bytes.as_chunks::<4>().0)
  {
    *pixel = [*blue, *green, *red, 255];
  }

  Ok(())
}
