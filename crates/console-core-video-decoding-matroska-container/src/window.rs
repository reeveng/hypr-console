//! The file read through one window of bytes, a header at a time.
//!
//! Opening a file reads element headers and never frame payloads, so what is
//! asked of the source is many short reads close together. They are answered
//! out of one buffer of `WINDOW` bytes that is refilled from wherever the next
//! read begins, and a read too long for half of it goes to the source
//! directly.
//!
//! Every walk here is a position moving forward through a span: siblings to
//! the end of their parent, a cluster's children to its end, the scan for the
//! next element that looks like a level-1 one after damage. Each step lands
//! past where it began, so a walk ends at the end of its span or at the first
//! header that does not decode, and it is written as `console-core-iteration`'s
//! state stepped again rather than a `loop` whose end is a `return` somewhere
//! in it. `walk` is that, with a step that may fail.
//!
//! What looks like a level-1 element after damage is decided the way upstream
//! decided it: a known level-1 id whose header decodes and fits, with a first
//! child that decodes and fits -- and for a Cluster, a Timestamp among its
//! first children once a CRC-32 or a Void is stepped over.

use std::io;

use console_core_byte_sources::{ByteSource, resize_read_buffer};
use console_core_extensible_binary_meta_language::{Size, read_id, read_size};
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::ebml::width;
use crate::error::Error;
use crate::ids::{CLUSTER, CRC32, EBML, Level, SEGMENT, TIMESTAMP, VOID, level};
use crate::states::ElementBounds;

pub(crate) const WINDOW: u64 = 64 * 1024;

const HALF_WINDOW: u64 = WINDOW / 2;

const LONGEST_HEADER: u64 = 12;

const ID_WIDTH: u64 = 4;

const CHILDREN_ASKED: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
  pub(crate) start: u64,
  pub(crate) end: u64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Header {
  pub(crate) id: u32,
  pub(crate) at: u64,
  pub(crate) size: Size,
  pub(crate) header_len: u64,
}

impl Header {
  pub(crate) fn data_start(&self) -> Result<u64, Never> {
    Ok(self.at.saturating_add(self.header_len))
  }

  pub(crate) fn end(&self, limit: u64) -> Result<u64, Never> {
    let Ok(start) = self.data_start();

    Ok(match self.size {
      Size::Known(size) => start.saturating_add(size).min(limit),
      Size::Unknown => limit,
    })
  }

  pub(crate) fn body(&self, limit: u64) -> Result<Span, Never> {
    let Ok(start) = self.data_start();
    let Ok(end) = self.end(limit);

    Ok(Span { start, end })
  }

  pub(crate) fn overruns(&self, limit: u64) -> Result<ElementBounds, Never> {
    let Ok(start) = self.data_start();

    Ok(match self.size {
      Size::Known(size) => match ::core::primitive::bool::then_some(start.saturating_add(size) > limit, ()) {
        ::core::option::Option::Some(()) => ElementBounds::Overrun,
        ::core::option::Option::None => ElementBounds::Within,
      },
      Size::Unknown => ElementBounds::Within,
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Candidate {
  Plausible,
  Implausible,
}

pub(crate) fn walk<State, Exit>(
  state: State,
  step: impl Fn(State) -> Result<Step<State, Exit>, Error>,
) -> Result<Exit, Error> {
  let ended = iterate(state, |current| {
    Ok(match step(current) {
      Ok(Step::Again(next)) => Step::Again(next),
      Ok(Step::Halt(exit)) => Step::Halt(Ok(exit)),
      Err(fault) => Step::Halt(Err(fault)),
    })
  });

  match ended {
    Ok(walked) => walked,
    Err(endless) => Err(Error::from(endless)),
  }
}

pub(crate) struct Window<'a, S: ByteSource + ?Sized> {
  source: &'a S,
  pub(crate) len: u64,
  held: Vec<u8>,
  start: u64,
}

impl<'a, S: ByteSource + ?Sized> Window<'a, S> {
  pub(crate) fn new(source: &'a S) -> Result<Self, Error> {
    let len = source.len()?;

    Ok(Self {
      source,
      len,
      held: Vec::new(),
      start: 0,
    })
  }

  fn fill(&mut self, at: u64) -> Result<(), Error> {
    let wanted = self.len.saturating_sub(at).min(WINDOW);
    let mut held = std::mem::take(&mut self.held);

    resize_read_buffer(&mut held, wanted)?;
    self.source.read_at(at, &mut held)?;
    self.held = held;
    self.start = at;

    Ok(())
  }

  pub(crate) fn peek(&mut self, span: Span) -> Result<&[u8], Error> {
    let end = span
      .end
      .min(span.start.saturating_add(WINDOW))
      .min(self.len);

    match ::core::primitive::bool::then_some(span.start < end, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(&[]),
    }

    let Ok(held_len) = width(&self.held);
    let covered = span.start >= self.start && end <= self.start.saturating_add(held_len);

    match ::core::primitive::bool::then_some(covered, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => self.fill(span.start)?,
    }

    let Ok(from) = index(span.start.saturating_sub(self.start));
    let Ok(to) = index(end.saturating_sub(self.start));

    Ok(match self.held.get(from..to) {
      Some(bytes) => bytes,
      None => &[],
    })
  }

  pub(crate) fn read_vec(&mut self, span: Span) -> Result<Vec<u8>, Error> {
    match ::core::primitive::bool::then_some(span.end > self.len, ()) {
      ::core::option::Option::Some(()) => return Err(Error::Io(io::Error::from(io::ErrorKind::UnexpectedEof))),
      ::core::option::Option::None => {}
    }

    let wanted = span.end.saturating_sub(span.start);

    match ::core::primitive::bool::then_some(wanted <= HALF_WINDOW, ()) {
      ::core::option::Option::Some(()) => {
        let held = self.peek(span)?;

        Ok(held.to_vec())
      }
      ::core::option::Option::None => {
        let mut read = Vec::new();

        resize_read_buffer(&mut read, wanted)?;
        self.source.read_at(span.start, &mut read)?;

        Ok(read)
      }
    }
  }

  pub(crate) fn value(&mut self, header: &Header, limit: u64) -> Result<&[u8], Error> {
    let Ok(body) = header.body(limit);

    self.peek(body)
  }

  pub(crate) fn header(&mut self, span: Span) -> Result<Option<Header>, Error> {
    match ::core::primitive::bool::then_some(span.start < span.end, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(None),
    }

    let bytes = self.peek(Span {
      start: span.start,
      end: span.start.saturating_add(LONGEST_HEADER),
    })?;
    let Ok(identified) = read_id(bytes);
    let (id, id_len) = match identified {
      Some(identified_2) => identified_2,
      None => return Ok(None),
    };
    let Ok(after_id) = index(id_len);
    let Ok(sized) = read_size(match bytes.get(after_id..) {
      Some(rest) => rest,
      None => &[],
    });
    let (size, size_len) = match sized {
      Some(sized_2) => sized_2,
      None => return Ok(None),
    };
    let header_len = u64::from(id_len.saturating_add(size_len));

    Ok(match ::core::primitive::bool::then_some(span.start.saturating_add(header_len) > span.end, ()) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => Some(Header {
        id,
        at: span.start,
        size,
        header_len,
      }),
    })
  }

  pub(crate) fn resync(&mut self, span: Span) -> Result<Option<u64>, Error> {
    let Ok(stride) = index(WINDOW.saturating_sub(ID_WIDTH.saturating_sub(1)));

    for chunk_at in
      (span.start..span.end.saturating_sub(ID_WIDTH.saturating_sub(1))).step_by(stride.max(1))
    {
      let peeked = self.peek(Span {
        start: chunk_at,
        end: chunk_at.saturating_add(WINDOW),
      })?;
      let chunk = peeked.to_vec();
      let Ok(chunk_len) = width(&chunk);

      match ::core::primitive::bool::then_some(chunk_len < ID_WIDTH, ()) {
        ::core::option::Option::Some(()) => return Ok(None),
        ::core::option::Option::None => {}
      }

      let found = self.first_plausible(
        &chunk,
        Span {
          start: chunk_at,
          end: span.end,
        },
      )?;

      match found {
        Some(at) => return Ok(Some(at)),
        None => {}
      }
    }

    Ok(None)
  }

  fn first_plausible(&mut self, chunk: &[u8], span: Span) -> Result<Option<u64>, Error> {
    for (four, offset) in chunk.windows(4).zip(0u64..) {
      let id = match four {
        [a, b, c, d] => u32::from_be_bytes([*a, *b, *c, *d]),
        _ => continue,
      };
      let Ok(standing) = level(id);

      match (standing, id) {
        (Level::Top, EBML | SEGMENT) | (Level::Nested, _) => continue,
        (Level::Top, _) => {}
      }

      let at = span.start.saturating_add(offset);
      let candidate = self.plausible(
        Span {
          start: at,
          end: span.end,
        },
        id,
      )?;

      match candidate {
        Candidate::Plausible => return Ok(Some(at)),
        Candidate::Implausible => {}
      }
    }

    Ok(None)
  }

  fn plausible(&mut self, span: Span, id: u32) -> Result<Candidate, Error> {
    let found = self.header(span)?;
    let header = match found {
      Some(header) => header,
      None => return Ok(Candidate::Implausible),
    };
    let Ok(bounds) = header.overruns(span.end);

    match (header.id == id, bounds, header.size, id) {
      (false, _, _, _) | (true, ElementBounds::Overrun, _, _) => {
        return Ok(Candidate::Implausible);
      }
      (true, ElementBounds::Within, Size::Unknown, CLUSTER)
      | (true, ElementBounds::Within, Size::Known(_), _) => {}
      (true, ElementBounds::Within, Size::Unknown, _) => return Ok(Candidate::Implausible),
    }

    let Ok(body) = header.body(span.end);

    self.first_children(body, id)
  }

  fn first_children(&mut self, body: Span, id: u32) -> Result<Candidate, Error> {
    let mut child_at = body.start;

    for _asked in 0..CHILDREN_ASKED {
      let found = self.header(Span {
        start: child_at,
        end: body.end,
      })?;
      let child = match found {
        Some(child) => child,
        None => return Ok(Candidate::Implausible),
      };
      let Ok(bounds) = child.overruns(body.end);

      match (bounds, id, child.id) {
        (ElementBounds::Overrun, _, _) => return Ok(Candidate::Implausible),
        (ElementBounds::Within, CLUSTER, TIMESTAMP) => return Ok(Candidate::Plausible),
        (ElementBounds::Within, CLUSTER, CRC32 | VOID) => {
          let Ok(next) = child.end(body.end);

          child_at = next;
        }
        (ElementBounds::Within, CLUSTER, _) => return Ok(Candidate::Implausible),
        (ElementBounds::Within, _, _) => return Ok(Candidate::Plausible),
      }
    }

    Ok(Candidate::Implausible)
  }

  pub(crate) fn unknown_end(&mut self, span: Span) -> Result<u64, Error> {
    walk((self, span.start), |(window, at)| {
      let found = window.header(Span {
        start: at,
        end: span.end,
      })?;

      Ok(match found {
        None => Step::Halt(at),
        Some(header) => {
          let Ok(standing) = level(header.id);

          match (standing, header.size) {
            (Level::Top, _) | (Level::Nested, Size::Unknown) => Step::Halt(at),
            (Level::Nested, Size::Known(_)) => {
              let Ok(next) = header.end(span.end);

              Step::Again((window, next))
            }
          }
        }
      })
    })
  }

  pub(crate) fn siblings<Held>(
    &mut self,
    span: Span,
    start: Held,
    visit: impl Fn(&mut Self, Header, Held) -> Result<Held, Error>,
  ) -> Result<Held, Error> {
    walk((self, span.start, start), |(window, at, held)| {
      let found = window.header(Span {
        start: at,
        end: span.end,
      })?;

      Ok(match found {
        None => Step::Halt(held),
        Some(header) => {
          let visited = visit(window, header, held)?;
          let Ok(next) = header.end(span.end);

          Step::Again((window, next, visited))
        }
      })
    })
  }
}
