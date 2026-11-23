//! The Ogg reader a probe hands a stream to: it finds the logical streams in the first pages,
//! reads their headers, and from then on hands back packets in the order the file stores them.
//!
//! A page marked as the first of its stream, met after the packets have begun, is the start of
//! a chained link. The reader reads the new link's tracks and then asks to be reset, because the
//! caller's decoders belong to the old ones.

use console_core_audio_decoding_interfaces::codecs::CodecParameters;
use console_core_audio_decoding_interfaces::errors::{
  Error, Result, SeekErrorKind, reset_error, seek_error, unsupported_error,
};
use console_core_audio_decoding_interfaces::formats::prelude::*;
use console_core_audio_decoding_interfaces::meta::{Metadata, MetadataLog};
use console_core_audio_decoding_interfaces::probe::{Descriptor, Instantiate, QueryDescriptor};
use console_core_audio_decoding_interfaces::support_format;
use console_core_byte_streams::{
  MediaSource, MediaSourceStream, ReadBytes, SeekBuffered, Seeking,
};
use console_core_iteration::Step;
use std::cmp::Ordering;
use std::io::{Seek, SeekFrom};

use crate::logical::LogicalStream;
use crate::mappings;
use crate::page::{OGG_PAGE_MAX_SIZE, PageReader, StreamStart};
use crate::physical::{self, ByteRange, Streams};
use crate::stepping::{Certain, Stepped, walk};

const BISECTION_FLOOR: u64 = 2 * OGG_PAGE_MAX_SIZE;

pub struct OggReader {
  reader: MediaSourceStream,
  tracks: Vec<Track>,
  metadata: MetadataLog,
  gapless: GaplessPlayback,
  pages: PageReader,
  streams: Streams,
  phys_byte_range_start: u64,
  phys_byte_range_end: Option<u64>,
}

struct Search<'a> {
  demuxer: &'a mut OggReader,
  serial: u32,
  required_ts: u64,
  range: ByteRange,
}

struct Landing<'a> {
  demuxer: &'a mut OggReader,
  serial: u32,
  required_ts: u64,
}

struct Identification<'a> {
  pages: &'a mut PageReader,
  reader: &'a mut MediaSourceStream,
  gapless: GaplessPlayback,
  streams: Streams,
  byte_range_start: u64,
}

type Identified = (Streams, u64);

struct Headers<'a> {
  pages: &'a mut PageReader,
  reader: &'a mut MediaSourceStream,
  metadata: &'a mut MetadataLog,
  streams: Streams,
  byte_range_start: u64,
}

fn next_packet_step(demuxer: &mut OggReader) -> Stepped<&mut OggReader, Packet> {
  let Ok(buffered) = demuxer.take_buffered_packet();

  match buffered {
    Some(packet) => Ok(Step::Halt(Ok(packet))),
    None => Ok(match demuxer.read_page() {
      Ok(()) => Step::Again(demuxer),
      Err(fault) => Step::Halt(Err(fault)),
    }),
  }
}

fn search_step(mut search: Search<'_>) -> Stepped<Search<'_>, ByteRange> {
  let span = search.range.end.saturating_sub(search.range.start);

  match span.cmp(&BISECTION_FLOOR) {
    Ordering::Less | Ordering::Equal => return Ok(Step::Halt(Ok(search.range))),
    Ordering::Greater => {}
  }

  let mid = search.range.start.midpoint(search.range.end);

  match search.demuxer.reader.seek(SeekFrom::Start(mid)) {
    Ok(_position) => {}
    Err(fault) => return Ok(Step::Halt(Err(Error::from(fault)))),
  }

  match search
    .demuxer
    .pages
    .next_page_for_serial(&mut search.demuxer.reader, search.serial)
  {
    Ok(()) => {}
    Err(_exhausted) => {
      search.range.end = mid;

      return Ok(Step::Again(search));
    }
  }

  let Ok(page) = search.demuxer.pages.page();

  let (start_ts, end_ts) = match search.demuxer.streams.get(&search.serial) {
    Some(stream) => {
      let Ok(inspected) = stream.inspect_page(&page);

      inspected
    }
    None => return Ok(Step::Halt(seek_error(SeekErrorKind::InvalidTrack))),
  };

  match (
    search.required_ts.cmp(&start_ts),
    search.required_ts.cmp(&end_ts),
  ) {
    (Ordering::Less, _) => search.range.end = mid,
    (Ordering::Equal | Ordering::Greater, Ordering::Greater) => search.range.start = mid,
    (Ordering::Equal | Ordering::Greater, Ordering::Less | Ordering::Equal) => {
      return Ok(Step::Halt(Ok(ByteRange {
        start: mid,
        end: mid,
      })));
    }
  }

  Ok(Step::Again(search))
}

fn landing_step(landing: Landing<'_>) -> Stepped<Landing<'_>, u64> {
  let Ok(peeked) = landing.demuxer.peek_logical_packet();

  match peeked {
    Some(packet) => {
      let Ok(id) = packet.track_id();
      let Ok(ts) = packet.ts();
      let Ok(dur) = packet.dur();
      let end = ts.saturating_add(dur);

      match (id.cmp(&landing.serial), end.cmp(&landing.required_ts)) {
        (Ordering::Equal, Ordering::Equal | Ordering::Greater) => Ok(Step::Halt(Ok(ts))),
        (Ordering::Less | Ordering::Greater, _) | (Ordering::Equal, Ordering::Less) => {
          let Ok(()) = landing.demuxer.discard_logical_packet();

          Ok(Step::Again(landing))
        }
      }
    }
    None => Ok(match landing.demuxer.read_page() {
      Ok(()) => Step::Again(landing),
      Err(fault) => Step::Halt(Err(fault)),
    }),
  }
}

fn identification_step(mut state: Identification<'_>) -> Stepped<Identification<'_>, Identified> {
  let Ok(header) = state.pages.header();

  match header.stream_start {
    StreamStart::Later => return Ok(Step::Halt(Ok((state.streams, state.byte_range_start)))),
    StreamStart::Beginning => {}
  }

  state.byte_range_start = state.reader.pos();

  let Ok(first) = state.pages.first_packet();

  let mapper = match first {
    Some(packet) => match mappings::detect(packet) {
      Ok(mapper) => Some(mapper),
      Err(fault) => return Ok(Step::Halt(Err(fault))),
    },
    None => None,
  };

  match mapper {
    Some(mapper_2) => {
      let Ok(stream) = LogicalStream::new(mapper_2, state.gapless);
      let _replaced = state.streams.insert(header.serial, stream);
    }
    None => {}
  }

  Ok(match state.pages.try_next_page(state.reader) {
    Ok(()) => Step::Again(state),
    Err(fault) => Step::Halt(Err(fault)),
  })
}

fn headers_step(mut state: Headers<'_>) -> Stepped<Headers<'_>, Identified> {
  let Ok(page) = state.pages.page();

  match state.streams.get_mut(&page.header.serial) {
    Some(stream) => {
      let side_data = match stream.read_page(&page) {
        Ok(side_data) => side_data,
        Err(fault) => return Ok(Step::Halt(Err(fault))),
      };

      for revision in side_data {
        let Ok(()) = state.metadata.push(revision);
      }

      let Ok(peeked) = stream.peek_packet();

      match peeked {
        ::core::option::Option::Some(_) => {
          return Ok(Step::Halt(Ok((state.streams, state.byte_range_start))));
        }
        ::core::option::Option::None => {}
      }
    }
    None => {}
  }

  state.byte_range_start = state.reader.pos();

  Ok(match state.pages.try_next_page(state.reader) {
    Ok(()) => Step::Again(state),
    Err(fault) => Step::Halt(Err(fault)),
  })
}

fn within_stream(params: &CodecParameters, ts: u64) -> Result<()> {
  match ts.cmp(&params.start_ts) {
    Ordering::Less => return seek_error(SeekErrorKind::OutOfRange),
    Ordering::Equal | Ordering::Greater => {}
  }

  match params.n_frames {
    Some(dur) => match ts.cmp(&dur.saturating_add(params.start_ts)) {
      Ordering::Greater => seek_error(SeekErrorKind::OutOfRange),
      Ordering::Less | Ordering::Equal => Ok(()),
    },
    None => Ok(()),
  }
}

impl OggReader {
  fn read_page(&mut self) -> Result<()> {
    self.pages.next_page(&mut self.reader)?;

    let Ok(page) = self.pages.page();

    match page.header.stream_start {
      StreamStart::Beginning => {
        self.start_new_physical_stream()?;

        reset_error()
      }
      StreamStart::Later => match self.streams.get_mut(&page.header.serial) {
        Some(stream) => {
          let _side_data = stream.read_page(&page)?;

          Ok(())
        }
        None => Ok(()),
      },
    }
  }

  #[must_use]
  fn peek_logical_packet(&self) -> Certain<Option<&Packet>> {
    let Ok(header) = self.pages.header();

    match self.streams.get(&header.serial) {
      Some(stream) => stream.peek_packet(),
      None => Ok(None),
    }
  }

  fn discard_logical_packet(&mut self) -> Certain<()> {
    let Ok(header) = self.pages.header();

    match self.streams.get_mut(&header.serial) {
      Some(stream) => stream.consume_packet(),
      None => Ok(()),
    }
  }

  fn take_buffered_packet(&mut self) -> Certain<Option<Packet>> {
    let Ok(header) = self.pages.header();

    match self.streams.get_mut(&header.serial) {
      Some(stream) => stream.next_packet(),
      None => Ok(None),
    }
  }

  fn next_logical_packet(&mut self) -> Result<Packet> {
    walk(self, next_packet_step)
  }

  fn bisect(&mut self, serial: u32, required_ts: u64) -> Result<()> {
    let end = match self.phys_byte_range_end {
      Some(end) => end,
      None => return seek_error(SeekErrorKind::Unseekable),
    };

    let range = ByteRange {
      start: self.phys_byte_range_start,
      end,
    };
    let converged = walk(
      Search {
        demuxer: &mut *self,
        serial,
        required_ts,
        range,
      },
      search_step,
    )?;

    match converged.start.cmp(&converged.end) {
      Ordering::Equal => {}
      Ordering::Less | Ordering::Greater => {
        self.reader.seek(SeekFrom::Start(converged.start))?;

        match self.pages.next_page_for_serial(&mut self.reader, serial) {
          Ok(()) => {}
          Err(_exhausted) => return seek_error(SeekErrorKind::OutOfRange),
        }
      }
    }

    for (&stream_serial, stream) in self.streams.iter_mut() {
      let Ok(()) = stream.reset();

      match stream_serial.cmp(&serial) {
        Ordering::Equal => {
          let Ok(page) = self.pages.page();
          let _side_data = stream.read_page(&page)?;
        }
        Ordering::Less | Ordering::Greater => {}
      }
    }

    Ok(())
  }

  fn do_seek(&mut self, serial: u32, required_ts: u64) -> Result<SeekedTo> {
    match self.reader.seeking() {
      Seeking::Anywhere => {
        self.bisect(serial, required_ts)?;
      }
      Seeking::ForwardOnly => {}
    }

    let actual_ts = walk(
      Landing {
        demuxer: &mut *self,
        serial,
        required_ts,
      },
      landing_step,
    )?;

    Ok(SeekedTo {
      track_id: serial,
      actual_ts,
      required_ts,
    })
  }

  fn stream_params(&self, serial: u32) -> Result<&CodecParameters> {
    match self.streams.get(&serial) {
      Some(stream) => {
        let Ok(params) = stream.codec_params();

        Ok(params)
      }
      None => seek_error(SeekErrorKind::InvalidTrack),
    }
  }

  fn probe_end(&mut self, streams: &mut Streams, byte_range_start: u64) -> Result<Option<u64>> {
    match self.reader.seeking() {
      Seeking::Anywhere => match self.reader.byte_len() {
        Some(total_len) => physical::probe_stream_end(
          &mut self.reader,
          &mut self.pages,
          streams,
          ByteRange {
            start: byte_range_start,
            end: total_len,
          },
        ),
        None => Ok(None),
      },
      Seeking::ForwardOnly => Ok(None),
    }
  }

  fn start_new_physical_stream(&mut self) -> Result<()> {
    let byte_range_start = self.reader.pos();

    let (identified_streams, identified_start) = walk(
      Identification {
        pages: &mut self.pages,
        reader: &mut self.reader,
        gapless: self.gapless,
        streams: Streams::new(),
        byte_range_start,
      },
      identification_step,
    )?;

    let (mut streams, byte_range_start_2) = walk(
      Headers {
        pages: &mut self.pages,
        reader: &mut self.reader,
        metadata: &mut self.metadata,
        streams: identified_streams,
        byte_range_start: identified_start,
      },
      headers_step,
    )?;

    physical::probe_stream_start(&mut self.reader, &mut self.pages, &mut streams)?;

    let byte_range_end = self.probe_end(&mut streams, byte_range_start_2)?;

    self.tracks.clear();

    for (&serial, stream) in streams.iter() {
      let Ok(params) = stream.codec_params();

      let Ok(value_4) = Track::new(serial, params.clone());

      self.tracks.push(value_4);
    }

    self.streams = streams;
    self.phys_byte_range_start = byte_range_start_2;
    self.phys_byte_range_end = byte_range_end;

    Ok(())
  }
}

impl QueryDescriptor for OggReader {
  fn query() -> std::result::Result<&'static [Descriptor], console_core_never::Never> {
    Ok(&[support_format!(
      "ogg",
      "OGG",
      &["ogg", "ogv", "oga", "ogx", "ogm", "spx", "opus"],
      &["video/ogg", "audio/ogg", "application/ogg"],
      &[b"OggS"]
    )])
  }

  fn score(_context: &[u8]) -> std::result::Result<u8, console_core_never::Never> {
    Ok(255)
  }
}

impl FormatReader for OggReader {
  fn try_new(mut source: MediaSourceStream, options: &FormatOptions) -> Result<Self> {
    source.ensure_seekback_buffer(OGG_PAGE_MAX_SIZE);

    let pages = PageReader::try_new(&mut source)?;

    let Ok(header) = pages.header();

    match header.stream_start {
      StreamStart::Beginning => {}
      StreamStart::Later => return unsupported_error("ogg: page is not marked as first"),
    }

    let mut ogg = OggReader {
      reader: source,
      tracks: Vec::new(),
      metadata: MetadataLog::default(),
      gapless: options.gapless,
      pages,
      streams: Streams::new(),
      phys_byte_range_start: 0,
      phys_byte_range_end: None,
    };

    ogg.start_new_physical_stream()?;

    Ok(ogg)
  }

  fn next_packet(&mut self) -> Result<Packet> {
    self.next_logical_packet()
  }

  fn metadata(&mut self) -> std::result::Result<Metadata<'_>, console_core_never::Never> {
    let Ok(value_5) = self.metadata.metadata();

    Ok(value_5)
  }

  fn cues(&self) -> std::result::Result<&[Cue], console_core_never::Never> {
    Ok(&[])
  }

  fn tracks(&self) -> std::result::Result<&[Track], console_core_never::Never> {
    Ok(&self.tracks)
  }

  fn seek(&mut self, _mode: SeekMode, to: SeekTo) -> Result<SeekedTo> {
    let (required_ts, serial) = match to {
      SeekTo::TimeStamp { ts, track_id } => {
        let params = self.stream_params(track_id)?;

        within_stream(params, ts)?;

        (ts, track_id)
      }
      SeekTo::Time { time, track_id } => {
        let serial = match track_id {
          Some(serial) => serial,
          None => match self.default_track() {
            Ok(Some(default_track)) => default_track.id,
            Ok(None) => return seek_error(SeekErrorKind::Unseekable),
          },
        };

        let params = self.stream_params(serial)?;

        let sample_rate = match params.sample_rate {
          Some(sample_rate) => sample_rate,
          None => return seek_error(SeekErrorKind::Unseekable),
        };

        let time_base = TimeBase::new(1, sample_rate)?;
        let Ok(ts) = time_base.calc_timestamp(time);

        within_stream(params, ts)?;

        (ts, serial)
      }
    };

    self.do_seek(serial, required_ts)
  }

  fn into_inner(
    self: Box<Self>,
  ) -> std::result::Result<MediaSourceStream, console_core_never::Never> {
    Ok(self.reader)
  }
}
