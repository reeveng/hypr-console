//! The pages an Ogg stream is cut into, read off a file and written back.
//!
//! An Ogg file is a run of pages, each a header and a little content. The
//! header says where in the stream the page falls and lists the content's
//! lacing: a byte per segment of up to 255, where a segment shorter than 255
//! ends a packet and one of 255 says the packet goes on. So a codec's packets
//! are what the content spells once the lacing is read across pages, and
//! writing a changed packet means cutting it into pages again, numbering them
//! and checking each with the CRC every page carries.
//!
//! The metadata editor is what asks: it reads a stream's first packets to
//! find its tags, replaces the comment packet, writes the header packets out
//! as new pages and renumbers the pages after them. A packet read stops where
//! the last one asked for ends, mid-page if that is where it is, because the
//! editor keeps the rest of the file from exactly there.
//!
//! The crate began as ogg_pager, by Serial under MIT or Apache-2.0, carried in
//! `forks/` for lofty. What the editor never asked was left behind: reading
//! every packet in a file, a page's content handed out, a list of pages
//! returned rather than written, and the end-of-stream flag on the last of
//! them. The CRC is `console_core_checksums::ogg`, where the playback
//! demuxer's copy of the same table can ask for it too.
//!
//! File readers and in-memory streams can be passed directly. A caller using
//! a device reader supplies its I/O deadline before handing it to this crate.

mod packets;
mod page;
mod pagination;

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::io::{self, Read};

pub use packets::{Each, Packets};
pub use page::{Page, PageHeader};

pub const MAX_CONTENT_SIZE: u64 = 255 * 255;

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the caller sets the Read implementation's deadline, and Ogg reads only the requested number of bytes"
  )
)]
fn fill(data: &mut impl Read, into: &mut [u8]) -> io::Result<()> {
  data.read_exact(into)
}

fn read_content(data: &mut impl Read, declared: u64) -> Result<Vec<u8>, PageError> {
  match declared.cmp(&MAX_CONTENT_SIZE) {
    Ordering::Less | Ordering::Equal => {}
    Ordering::Greater => {
      return Err(PageError::Io(io::Error::new(
        io::ErrorKind::InvalidData,
        "Ogg page content exceeds its format limit",
      )));
    }
  }

  let mut content: Vec<u8> = (0..declared).map(|_| 0).collect();

  fill(data, &mut content)?;

  Ok(content)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
  BeginningOfStream,
  InsideStream,
}

#[derive(Debug)]
pub enum PageError {
  MissingCapturePattern,
  UnknownVersion,
  NoSegments,
  TooFewPackets,
  NoSuchPacket,
  Io(std::io::Error),
}

impl fmt::Display for PageError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      PageError::MissingCapturePattern => write!(to, "the page does not begin with OggS"),
      PageError::UnknownVersion => write!(
        to,
        "the page is written in a version of Ogg other than the only one there is"
      ),
      PageError::NoSegments => write!(to, "the page's lacing lists no segments"),
      PageError::TooFewPackets => write!(to, "the stream ends before the packets asked for"),
      PageError::NoSuchPacket => write!(to, "there is no packet at that place to replace"),
      PageError::Io(fault) => write!(to, "{fault}"),
    }
  }
}

impl Error for PageError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      PageError::Io(fault) => Some(fault),
      PageError::MissingCapturePattern
      | PageError::UnknownVersion
      | PageError::NoSegments
      | PageError::TooFewPackets
      | PageError::NoSuchPacket => None,
    }
  }
}

impl From<std::io::Error> for PageError {
  fn from(fault: std::io::Error) -> PageError {
    PageError::Io(fault)
  }
}
