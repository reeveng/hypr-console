//! Bytes read at an offset, whoever is reading their format.
//!
//! The Matroska and MPEG-4 readers carried the same trait and the same memory
//! and file implementations. A file passed to both had to implement both,
//! although the question was identical. This is that question once, with a
//! positioned file read and a seekable reader as different implementations.
//!
//! Length can fail for a file. Turning that failure into zero made an unreadable
//! file look empty to both containers, so measuring it carries the fault. A
//! seekable reader keeps its position only after a successful operation; a
//! failed read may already have moved it and the next read must seek again.
//! `FileSource` measures once, so reading each sample in a film does not ask
//! for the file's metadata again. The caller opens the file it means to read.
//! Callers of ReadSeekSource also supply any I/O deadline their reader needs.
//! Callers of [`resize_read_buffer`] bound the requested size by a measured
//! source extent or a fixed window, including any prefix they add.
//!
//! The implementations were ported from FilmCraft; `LICENSE-MIT` carries its
//! original notice alongside the workspace's license.

use std::cell::RefCell;
use std::io::{self, ErrorKind, Read, Seek, SeekFrom};

use console_core_never::Never;
use console_core_number_conversion::{checked_index, index};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Empty {
  Yes,
  No,
}

pub trait ByteSource {
  fn len(&self) -> io::Result<u64>;

  fn read_at(&self, offset: u64, into: &mut [u8]) -> io::Result<()>;

  fn is_empty(&self) -> io::Result<Empty> {
    let length = self.len()?;

    Ok(match length {
      0 => Empty::Yes,
      _some_bytes => Empty::No,
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit088_no_arriving_length",
  expect(
    explicit088_no_arriving_length,
    reason = "readers bound the requested length by a measured source or a fixed window before resizing"
  )
)]
pub fn resize_read_buffer(bytes: &mut Vec<u8>, length: u64) -> io::Result<()> {
  let count = checked_index(length)
    .map_err(|_outside_index_range| io::Error::from(ErrorKind::OutOfMemory))?;

  bytes
    .try_reserve_exact(count.saturating_sub(bytes.len()))
    .map_err(|_allocation| io::Error::from(ErrorKind::OutOfMemory))?;
  bytes.resize(count, 0);

  Ok(())
}

struct Extent {
  offset: u64,
  length: u64,
}

enum ReadBounds {
  WithinSource,
  PastEnd,
}

impl From<bool> for ReadBounds {
  fn from(within_source: bool) -> Self {
    match bool::then_some(within_source, ()) {
      Some(()) => Self::WithinSource,
      None => Self::PastEnd,
    }
  }
}

fn end(within: Extent, into: &[u8]) -> io::Result<u64> {
  let Extent { offset, length } = within;
  let count =
    u64::try_from(into.len()).map_err(|_too_wide| io::Error::from(ErrorKind::UnexpectedEof))?;
  let end = offset.checked_add(count).ok_or(ErrorKind::UnexpectedEof)?;

  match ReadBounds::from(end <= length) {
    ReadBounds::WithinSource => Ok(end),
    ReadBounds::PastEnd => Err(io::Error::from(ErrorKind::UnexpectedEof)),
  }
}

impl ByteSource for [u8] {
  fn len(&self) -> io::Result<u64> {
    u64::try_from(<[u8]>::len(self)).map_err(|_too_wide| io::Error::from(ErrorKind::InvalidInput))
  }

  fn read_at(&self, offset: u64, into: &mut [u8]) -> io::Result<()> {
    let length = ByteSource::len(self)?;
    let end = end(Extent { offset, length }, into)?;
    let Ok(from) = index(offset);
    let Ok(to) = index(end);
    let bytes = self.get(from..to).ok_or(ErrorKind::UnexpectedEof)?;

    for (target, byte) in into.iter_mut().zip(bytes) {
      *target = *byte;
    }

    Ok(())
  }
}

impl ByteSource for Vec<u8> {
  fn len(&self) -> io::Result<u64> {
    ByteSource::len(self.as_slice())
  }

  fn read_at(&self, offset: u64, into: &mut [u8]) -> io::Result<()> {
    self.as_slice().read_at(offset, into)
  }
}

impl<T: ByteSource + ?Sized> ByteSource for &T {
  fn len(&self) -> io::Result<u64> {
    T::len(self)
  }

  fn read_at(&self, offset: u64, into: &mut [u8]) -> io::Result<()> {
    T::read_at(self, offset, into)
  }
}

impl<T: ByteSource + ?Sized> ByteSource for Box<T> {
  fn len(&self) -> io::Result<u64> {
    T::len(self)
  }

  fn read_at(&self, offset: u64, into: &mut [u8]) -> io::Result<()> {
    T::read_at(self, offset, into)
  }
}

#[cfg(not(target_arch = "wasm32"))]
impl ByteSource for std::fs::File {
  fn len(&self) -> io::Result<u64> {
    self.metadata().map(|metadata| metadata.len())
  }

  #[cfg(unix)]
  fn read_at(&self, offset: u64, into: &mut [u8]) -> io::Result<()> {
    end(
      Extent {
        offset,
        length: u64::MAX,
      },
      into,
    )?;

    std::os::unix::fs::FileExt::read_exact_at(self, into, offset)
  }

  #[cfg(windows)]
  fn read_at(&self, offset: u64, into: &mut [u8]) -> io::Result<()> {
    end(
      Extent {
        offset,
        length: u64::MAX,
      },
      into,
    )?;

    Positioned { file: self, offset }.read_exact(into)
  }

  #[cfg(not(any(unix, windows)))]
  fn read_at(&self, offset: u64, into: &mut [u8]) -> io::Result<()> {
    end(
      Extent {
        offset,
        length: u64::MAX,
      },
      into,
    )?;

    let mut file = self;

    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(into)
  }
}

#[cfg(windows)]
struct Positioned<'a> {
  file: &'a std::fs::File,
  offset: u64,
}

#[cfg(not(target_arch = "wasm32"))]
pub struct FileSource {
  file: std::fs::File,
  length: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileSource {
  pub fn new(file: std::fs::File) -> io::Result<Self> {
    let length = ByteSource::len(&file)?;

    Ok(Self { file, length })
  }

  pub fn into_inner(self) -> Result<std::fs::File, Never> {
    Ok(self.file)
  }
}

#[cfg(not(target_arch = "wasm32"))]
impl ByteSource for FileSource {
  fn len(&self) -> io::Result<u64> {
    Ok(self.length)
  }

  fn read_at(&self, offset: u64, into: &mut [u8]) -> io::Result<()> {
    ByteSource::read_at(&self.file, offset, into)
  }
}

#[cfg(windows)]
impl Read for Positioned<'_> {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    allow(
      explicit051_no_machine_width,
      reason = "std::io::Read requires a usize byte count; the file offset is held as u64"
    )
  )]
  fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
    let count = std::os::windows::fs::FileExt::seek_read(self.file, into, self.offset)?;
    let wide =
      u64::try_from(count).map_err(|_too_wide| io::Error::from(ErrorKind::InvalidInput))?;

    self.offset = self.offset.saturating_add(wide);

    Ok(count)
  }
}

pub struct ReadSeekSource<R> {
  inner: RefCell<(R, Option<u64>)>,
  length: u64,
}

impl<R: Read + Seek> ReadSeekSource<R> {
  pub fn new(mut reader: R) -> io::Result<Self> {
    let length = reader.seek(SeekFrom::End(0))?;

    Ok(Self {
      inner: RefCell::new((reader, Some(length))),
      length,
    })
  }

  pub fn into_inner(self) -> Result<R, Never> {
    Ok(self.inner.into_inner().0)
  }
}

enum ReadPosition {
  AtOffset,
  NeedsSeek,
}

impl From<bool> for ReadPosition {
  fn from(at_offset: bool) -> Self {
    match bool::then_some(at_offset, ()) {
      Some(()) => Self::AtOffset,
      None => Self::NeedsSeek,
    }
  }
}

impl<R: Read + Seek> ByteSource for ReadSeekSource<R> {
  fn len(&self) -> io::Result<u64> {
    Ok(self.length)
  }

  fn read_at(&self, offset: u64, into: &mut [u8]) -> io::Result<()> {
    let end = end(
      Extent {
        offset,
        length: self.length,
      },
      into,
    )?;
    let mut held = self
      .inner
      .try_borrow_mut()
      .map_err(|_in_use| io::Error::from(ErrorKind::WouldBlock))?;
    let (reader, position) = &mut *held;

    match ReadPosition::from(*position == Some(offset)) {
      ReadPosition::AtOffset => {}
      ReadPosition::NeedsSeek => {
        *position = None;

        reader.seek(SeekFrom::Start(offset))?;
      }
    }

    *position = None;

    #[cfg_attr(
      dylint_lib = "explicit057_no_unbounded_wait",
      expect(
        explicit057_no_unbounded_wait,
        reason = "the caller owns the reader and sets any I/O deadline before constructing ReadSeekSource"
      )
    )]
    let read = reader.read_exact(into);

    read?;

    *position = Some(end);

    Ok(())
  }
}
