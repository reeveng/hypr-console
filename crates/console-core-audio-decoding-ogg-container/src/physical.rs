//! The physical stream: where each logical stream in it starts and ends in time, and where the
//! physical stream ends in the file when more than one are chained.
//!
//! The first page of every logical stream gives its start time and the encoder delay before
//! it. The end comes from scanning the last pages of the file, and when none of them belongs to
//! the streams just read, the file is a chain and the end of this link is found by bisecting for
//! the first page that belongs to the next one.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Seek, SeekFrom};

use console_core_audio_decoding_interfaces::errors::{Error, Result};
use console_core_byte_streams::{MediaSourceStream, ReadBytes, ScopedStream, SeekBuffered};
use console_core_iteration::Step;
use console_core_number_conversion::fitted;

use crate::logical::{InspectState, LogicalStream};
use crate::page::{OGG_PAGE_MAX_SIZE, PageReader};
use crate::stepping::{Stepped, walk};

pub type Streams = BTreeMap<u32, LogicalStream>;

#[derive(Copy, Clone)]
pub struct ByteRange {
  pub start: u64,
  pub end: u64,
}

struct StartScan<'a> {
  pages: &'a mut PageReader,
  reader: ScopedStream<&'a mut MediaSourceStream>,
  streams: &'a mut Streams,
  probed: BTreeSet<u32>,
}

struct EndScan<'a> {
  pages: &'a mut PageReader,
  reader: ScopedStream<&'a mut MediaSourceStream>,
  streams: &'a mut Streams,
  state: InspectState,
  upper_pos: Option<u64>,
}

struct Bisection<'a> {
  reader: &'a mut MediaSourceStream,
  pages: &'a mut PageReader,
  streams: &'a Streams,
  range: ByteRange,
  linear_scan_len: u64,
}

fn start_step(mut scan: StartScan<'_>) -> Stepped<StartScan<'_>, StartScan<'_>> {
  let Ok(page) = scan.pages.page();
  let serial = page.header.serial;

  match scan.streams.get_mut(&serial) {
    Some(stream) => match scan.probed.get(&serial) {
      Some(_already_probed) => {}
      None => {
        let Ok(()) = stream.inspect_start_page(&page);

        let _first = scan.probed.insert(serial);
      }
    },
    None => return Ok(Step::Halt(Ok(scan))),
  }

  match scan.probed.len().cmp(&scan.streams.len()) {
    Ordering::Equal | Ordering::Greater => Ok(Step::Halt(Ok(scan))),
    Ordering::Less => match scan.pages.try_next_page(&mut scan.reader) {
      Ok(()) => Ok(Step::Again(scan)),
      Err(_ended) => Ok(Step::Halt(Ok(scan))),
    },
  }
}

pub fn probe_stream_start(
  reader: &mut MediaSourceStream,
  pages: &mut PageReader,
  streams: &mut Streams,
) -> Result<()> {
  let original_pos = reader.pos();

  let Ok(scoped) = ScopedStream::new(&mut *reader, OGG_PAGE_MAX_SIZE);
  let scan = StartScan {
    pages,
    reader: scoped,
    streams,
    probed: BTreeSet::new(),
  };

  let done = walk(scan, start_step)?;
  let Ok(inner) = done.reader.into_inner();
  let _position = inner.seek_buffered(original_pos);

  Ok(())
}

impl EndScan<'_> {
  fn step(mut self) -> Stepped<Self, Option<u64>> {
    let Ok(page) = self.pages.page();

    match self.streams.get_mut(&page.header.serial) {
      Some(stream) => {
        let state = std::mem::take(&mut self.state);

        let Ok(inspected) = stream.inspect_end_page(state, &page);

        self.state = inspected;
      }
      None => return Ok(Step::Halt(Ok(self.upper_pos))),
    }

    self.upper_pos = Some(self.reader.pos());

    match self.pages.next_page(&mut self.reader) {
      Ok(()) => Ok(Step::Again(self)),
      Err(_ended) => Ok(Step::Halt(Ok(self.upper_pos))),
    }
  }
}

fn scan_stream_end(
  reader: &mut MediaSourceStream,
  pages: &mut PageReader,
  streams: &mut Streams,
  byte_range_end: u64,
) -> Result<Option<u64>> {
  let scoped_len = byte_range_end.saturating_sub(reader.pos());

  let Ok(scoped) = ScopedStream::new(reader, scoped_len);
  let scan = EndScan {
    pages,
    reader: scoped,
    streams,
    state: InspectState::default(),
    upper_pos: None,
  };

  walk(scan, EndScan::step)
}

fn bisect_step(mut bisection: Bisection<'_>) -> Stepped<Bisection<'_>, ByteRange> {
  let mid = bisection.range.start.midpoint(bisection.range.end);

  match bisection.reader.seek(SeekFrom::Start(mid)) {
    Ok(_position) => {}
    Err(fault) => return Ok(Step::Halt(Err(Error::from(fault)))),
  }

  match bisection.pages.next_page(bisection.reader) {
    Ok(()) => {}
    Err(_ended) => return Ok(Step::Halt(Ok(bisection.range))),
  }

  let Ok(header) = bisection.pages.header();
  let serial = header.serial;

  match bisection.streams.get(&serial) {
    Some(_in_link) => bisection.range.start = mid,
    None => bisection.range.end = mid,
  }

  let span = bisection.range.end.saturating_sub(bisection.range.start);

  match span.cmp(&bisection.linear_scan_len) {
    Ordering::Less => Ok(Step::Halt(Ok(bisection.range))),
    Ordering::Equal | Ordering::Greater => Ok(Step::Again(bisection)),
  }
}

fn scan_chained_end(
  reader: &mut MediaSourceStream,
  pages: &mut PageReader,
  streams: &mut Streams,
  range: ByteRange,
  linear_scan_len: u64,
) -> Result<Option<u64>> {
  let bisection = Bisection {
    reader: &mut *reader,
    pages: &mut *pages,
    streams: &*streams,
    range,
    linear_scan_len,
  };
  let narrowed = walk(bisection, bisect_step)?;

  reader.seek(SeekFrom::Start(narrowed.start))?;
  pages.next_page(reader)?;

  scan_stream_end(reader, pages, streams, narrowed.end)
}

pub fn probe_stream_end(
  reader: &mut MediaSourceStream,
  pages: &mut PageReader,
  streams: &mut Streams,
  range: ByteRange,
) -> Result<Option<u64>> {
  let original_pos = reader.pos();
  let Ok(stream_count) = fitted::<_, u64>(streams.len());
  let linear_scan_len = stream_count.saturating_mul(OGG_PAGE_MAX_SIZE);

  let from = match range.end.checked_sub(linear_scan_len) {
    Some(tail) => tail.max(range.start),
    None => range.start,
  };

  reader.seek(SeekFrom::Start(from))?;
  pages.next_page(reader)?;

  let scanned = scan_stream_end(reader, pages, streams, range.end)?;

  let chained = match scanned {
    Some(end) => Ok(Some(end)),
    None => scan_chained_end(reader, pages, streams, range, linear_scan_len),
  };

  let result = chained?;

  reader.seek(SeekFrom::Start(original_pos))?;

  Ok(result)
}
