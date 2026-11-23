//! A stream's first packets, read across its pages, and written back as pages.
//!
//! Reading walks the lacing a segment at a time. A segment of 255 says the
//! packet goes on; anything shorter ends it, and the bytes the packet has
//! been given since the last read are read then, so a packet that ends
//! mid-page leaves the reader just past it. A page whose lacing ends on a
//! segment of 255 hands its packet's bytes over before the next page is read.
//! A header that cannot be read ends the stream, whatever the reason, and a
//! stream that ends before the packets asked for is a fault; a read that runs
//! short inside a page is the reader's own fault and says so.
//!
//! A stream numbers its pages in thirty-two bits, so that is as many pages as
//! a read walks.

use std::io::{Read, Seek, Write};

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::pagination::paginated;
use crate::{PageError, PageHeader, Placement, read_content};

const LACING_CONTINUES: u8 = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packets {
  pub(crate) packets: Vec<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct Each<'a> {
  rest: std::slice::Iter<'a, Vec<u8>>,
}

impl<'a> Iterator for Each<'a> {
  type Item = &'a [u8];

  fn next(&mut self) -> Option<&'a [u8]> {
    self.rest.next().map(Vec::as_slice)
  }
}

impl<'a> IntoIterator for &'a Packets {
  type Item = &'a [u8];
  type IntoIter = Each<'a>;

  fn into_iter(self) -> Each<'a> {
    Each {
      rest: self.packets.iter(),
    }
  }
}

struct Assembly {
  finished: Vec<Vec<u8>>,
  current: Vec<u8>,
  unread: u64,
}

impl Assembly {
  fn caught_up<R: Read>(&mut self, data: &mut R) -> Result<(), PageError> {
    let bytes = read_content(data, self.unread)?;

    self.current.extend(bytes);
    self.unread = 0;

    Ok(())
  }

  fn count(&self) -> Result<u64, Never> {
    fitted::<_, u64>(self.finished.len())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Request {
  Read,
  Wanted,
}

impl From<bool> for Request {
  fn from(complete: bool) -> Self {
    match bool::then_some(complete, ()) {
      Some(()) => Self::Read,
      None => Self::Wanted,
    }
  }
}

impl Packets {
  pub fn read_count<R: Read + Seek>(data: &mut R, count: u64) -> Result<Packets, PageError> {
    let mut assembly = Assembly {
      finished: Vec::new(),
      current: Vec::new(),
      unread: 0,
    };

    match count {
      0 => {
        return Ok(Packets {
          packets: assembly.finished,
        });
      }
      _some => {}
    }

    'pages: for _page in 0..=u32::MAX {
      let header = match PageHeader::read(data) {
        Ok(header) => header,
        Err(_after_the_last_page) => break 'pages,
      };

      'segments: for segment in header.segments {
        assembly.unread = assembly.unread.saturating_add(u64::from(segment));

        match segment {
          LACING_CONTINUES => continue 'segments,
          _ends => {
            assembly.caught_up(data)?;
            assembly
              .finished
              .push(std::mem::take(&mut assembly.current));
          }
        }

        let Ok(read) = assembly.count();

        match Request::from(read == count) {
          Request::Read => break 'pages,
          Request::Wanted => {}
        }
      }

      assembly.caught_up(data)?;
    }

    let Ok(read) = assembly.count();

    match Request::from(read == count) {
      Request::Read => Ok(Packets {
        packets: assembly.finished,
      }),
      Request::Wanted => Err(PageError::TooFewPackets),
    }
  }

  pub fn get(&self, packet: u64) -> Result<Option<&[u8]>, Never> {
    let Ok(at) = index(packet);

    Ok(self.packets.get(at).map(Vec::as_slice))
  }

  pub fn set(&mut self, packet: u64, content: Vec<u8>) -> Result<(), PageError> {
    let Ok(at) = index(packet);

    match self.packets.get_mut(at) {
      Some(slot) => {
        *slot = content;

        Ok(())
      }
      None => Err(PageError::NoSuchPacket),
    }
  }

  pub fn write_to<W: Write>(
    &self,
    writer: &mut W,
    stream_serial: u32,
    granule_position: u64,
    placement: Placement,
  ) -> Result<u64, PageError> {
    let Ok(pages) = paginated(self, stream_serial, granule_position, placement);
    let Ok(written) = fitted::<_, u64>(pages.len());

    for mut page in pages {
      let Ok(()) = page.update_checksum();
      let Ok(bytes) = page.bytes();

      writer.write_all(&bytes)?;
    }

    Ok(written)
  }
}
