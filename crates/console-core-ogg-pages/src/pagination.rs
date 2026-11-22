//! Packets cut into pages, the way the fork cut them, byte for byte.
//!
//! Every packet starts a page of its own, and a page holds at most
//! thirty-two segments of 255, the 8160 bytes a packet is cut by, though Ogg
//! allows 255 segments: a header page
//! written small is a header page a player reads in one go. A packet that
//! fills its last page exactly is ended by a page with nothing on it but a
//! segment of nought, which is how Ogg says a packet that was a whole number
//! of full segments has stopped. A packet of no bytes is given no page, as
//! the fork gave it none.
//!
//! The first page of the first packet is marked as the beginning of the
//! stream when it is one, every page after the first of its packet is marked
//! as carrying a packet on, and a page on which no packet finishes says so
//! with a granule position of all ones. The fork counted the full page before
//! an empty ending page as one a packet finishes on, and so does this, since
//! the pages written are compared with the ones it wrote.

use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::{Packets, Page, PageHeader, Placement};

const CONTINUED_PACKET: u8 = 0x01;

const BEGINNING_OF_STREAM: u8 = 0x02;

const NO_PACKET_FINISHES: u64 = u64::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lacing {
  Continued,
  Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Finishing {
  Finishes,
  Continues,
}

struct Part<'a> {
  content: &'a [u8],
  lacing: Lacing,
  finishing: Finishing,
}

fn parts(packet: &[u8]) -> Result<Vec<Part<'_>>, Never> {
  let (whole, rest) = packet.as_chunks::<8160>();
  let mut parts = Vec::new();

  match whole.split_last() {
    None => match rest {
      [] => {}
      _short => parts.push(Part {
        content: rest,
        lacing: Lacing::Ended,
        finishing: Finishing::Finishes,
      }),
    },
    Some((last, before)) => {
      parts.extend(before.iter().map(|full| Part {
        content: full.as_slice(),
        lacing: Lacing::Continued,
        finishing: Finishing::Continues,
      }));

      let last_finishing = match rest {
        [] => Finishing::Finishes,
        _more => Finishing::Continues,
      };

      parts.push(Part {
        content: last.as_slice(),
        lacing: Lacing::Continued,
        finishing: last_finishing,
      });
      parts.push(Part {
        content: rest,
        lacing: Lacing::Ended,
        finishing: Finishing::Finishes,
      });
    }
  }

  Ok(parts)
}

fn lacing(part: &Part<'_>) -> Result<Vec<u8>, Never> {
  let (whole, short) = part.content.as_chunks::<255>();
  let mut segments: Vec<u8> = whole.iter().map(|_full| u8::MAX).collect();

  match part.lacing {
    Lacing::Continued => {}
    Lacing::Ended => {
      let Ok(last) = fitted::<_, u8>(short.len());

      segments.push(last);
    }
  }

  Ok(segments)
}

pub(crate) fn paginated(
  packets: &Packets,
  stream_serial: u32,
  granule_position: u64,
  placement: Placement,
) -> Result<Vec<Page>, Never> {
  let mut pages = Vec::new();
  let mut opening = placement;

  for packet in &packets.packets {
    let Ok(cut) = parts(packet);

    for (place, part) in cut.iter().enumerate() {
      let kind = match (place, opening) {
        (0, Placement::BeginningOfStream) => BEGINNING_OF_STREAM,
        (0, Placement::InsideStream) => 0,
        (_later, Placement::BeginningOfStream | Placement::InsideStream) => CONTINUED_PACKET,
      };

      let granule = match part.finishing {
        Finishing::Finishes => granule_position,
        Finishing::Continues => NO_PACKET_FINISHES,
      };

      let Ok(sequence_number) = fitted::<_, u32>(pages.len());
      let Ok(segments) = lacing(part);

      pages.push(Page {
        header: PageHeader {
          start: 0,
          granule_position: granule,
          stream_serial,
          sequence_number,
          kind,
          segments,
          checksum: 0,
        },
        content: part.content.to_vec(),
      });

      opening = Placement::InsideStream;
    }
  }

  Ok(pages)
}
