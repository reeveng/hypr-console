//! The FLAC reader: the metadata blocks up front, then frames until the stream ends.
//!
//! Every block is read through a `ScopedStream` the length of the block, so a reader that
//! wants more than the block says it has fails there rather than eating the next block's
//! header. Seeking is a bisection over the bytes of the file, narrowed by the seek table
//! when there is one, that stops when the range is too small for a bisection to be worth
//! its reads and finishes by walking frame headers forward. A stream that cannot be seeked
//! backwards can still be walked forward to a timestamp, which is the walk alone.

use std::cmp::Ordering;
use std::io::{Seek, SeekFrom};

use console_core_audio_decoding_flac_and_vorbis_headers::flac::metadata::{
  MetadataBlockBoundary, MetadataBlockHeader, MetadataBlockType, StreamInfo,
  read_application_block, read_comment_block, read_cuesheet_block, read_picture_block,
  read_seek_table_block,
};
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_FLAC, CodecParameters, PacketIntegrity, VerificationCheck,
};
use console_core_audio_decoding_interfaces::errors::{
  Error, Result, SeekErrorKind, decode_error, seek_error, unsupported_error,
};
use console_core_audio_decoding_interfaces::formats::util::{SeekIndex, SeekSearchResult};
use console_core_audio_decoding_interfaces::formats::{
  Cue, FormatOptions, FormatReader, Packet, SeekMode, SeekTo, SeekedTo, Track,
};
use console_core_audio_decoding_interfaces::meta::{Metadata, MetadataBuilder, MetadataLog};
use console_core_audio_decoding_interfaces::probe::{Descriptor, Instantiate, QueryDescriptor};
use console_core_audio_decoding_interfaces::support_format;
use console_core_audio_decoding_interfaces::units::TimeBase;
use console_core_byte_streams::{
  BufReader, FiniteStream, MediaSource, MediaSourceStream, ReadBytes, ScopedStream, Seeking,
};
use console_core_iteration::Step;
use console_core_never::Never;

use super::parser::{PacketParser, SyncInfo};
use super::steps::{halted, run};

const FLAC_STREAM_MARKER: [u8; 4] = *b"fLaC";

const BISECTION_FLOOR: u64 = 2 * 8096;

pub struct FlacReader {
  reader: MediaSourceStream,
  metadata: MetadataLog,
  tracks: Vec<Track>,
  cues: Vec<Cue>,
  index: Option<SeekIndex>,
  first_frame_offset: u64,
  parser: PacketParser,
}

struct Blocks {
  reader: MediaSourceStream,
  metadata_builder: MetadataBuilder,
  tracks: Vec<Track>,
  cues: Vec<Cue>,
  index: Option<SeekIndex>,
  parser: Option<PacketParser>,
}

fn read_body(blocks: &mut Blocks, header: &MetadataBlockHeader) -> Result<()> {
  let Ok(mut block_stream) = ScopedStream::new(&mut blocks.reader, u64::from(header.block_len));

  match header.block_type {
    MetadataBlockType::Application => {
      let _unstored = read_application_block(&mut block_stream, header.block_len)?;
    }
    MetadataBlockType::SeekTable => match blocks.index {
      None => {
        let Ok(mut new_index) = SeekIndex::new();

        read_seek_table_block(&mut block_stream, header.block_len, &mut new_index)?;

        blocks.index = Some(new_index);
      }
      Some(_) => return decode_error("flac: found more than one seek table block"),
    },
    MetadataBlockType::VorbisComment => {
      read_comment_block(&mut block_stream, &mut blocks.metadata_builder)?;
    }
    MetadataBlockType::Cuesheet => {
      read_cuesheet_block(&mut block_stream, &mut blocks.cues)?;
    }
    MetadataBlockType::Picture => {
      read_picture_block(&mut block_stream, &mut blocks.metadata_builder)?;
    }
    MetadataBlockType::StreamInfo => {
      read_stream_info_block(&mut block_stream, &mut blocks.tracks, &mut blocks.parser)?;
    }
    MetadataBlockType::Padding | MetadataBlockType::Unknown(_) => {
      block_stream.ignore_bytes(u64::from(header.block_len))?;
    }
  }

  let block_unread_len = block_stream.bytes_available();

  block_stream.ignore_bytes(block_unread_len)?;

  Ok(())
}

fn read_block(mut blocks: Blocks) -> std::result::Result<Step<Blocks, Result<Blocks>>, Never> {
  let header = match MetadataBlockHeader::read(&mut blocks.reader) {
    Ok(header) => header,
    Err(fault) => return halted(fault),
  };

  Ok(match read_body(&mut blocks, &header) {
    Err(fault) => Step::Halt(Err(fault)),
    Ok(()) => match header.boundary {
      MetadataBlockBoundary::LastBlock => Step::Halt(Ok(blocks)),
      MetadataBlockBoundary::MoreBlocks => Step::Again(blocks),
    },
  })
}

impl FlacReader {
  fn init_with_metadata(source: MediaSourceStream) -> Result<Self> {
    let Ok(value_1) = MetadataBuilder::new();

    let blocks = Blocks {
      reader: source,
      metadata_builder: value_1,
      tracks: Vec::new(),
      cues: Vec::new(),
      index: None,
      parser: None,
    };

    let Blocks {
      mut reader,
      metadata_builder,
      tracks,
      cues,
      index,
      parser: found,
    } = run(blocks, read_block)?;

    let mut parser = match found {
      Some(parser) => parser,
      None => return decode_error("flac: no stream info block"),
    };

    let mut metadata = MetadataLog::default();
    let Ok(value_2) = metadata_builder.metadata();

    let Ok(()) = metadata.push(value_2);

    let _synchronized = parser.resync(&mut reader)?;

    let first_frame_offset = reader.pos();

    Ok(FlacReader {
      reader,
      metadata,
      tracks,
      cues,
      index,
      first_frame_offset,
      parser,
    })
  }

  fn search_range(&self, ts: u64) -> Result<(u64, u64)> {
    let first = self.first_frame_offset;

    let end = match self.reader.byte_len() {
      Some(len) => len,
      None => return seek_error(SeekErrorKind::Unseekable),
    };

    let Ok(found) = match &self.index {
      Some(index) => index.search(ts),
      None => Ok(SeekSearchResult::Stream),
    };

    Ok(match found {
      SeekSearchResult::Upper(upper) => (first, first.saturating_add(upper.byte_offset)),
      SeekSearchResult::Lower(lower) => (first.saturating_add(lower.byte_offset), end),
      SeekSearchResult::Range(lower, upper) => (
        first.saturating_add(lower.byte_offset),
        first.saturating_add(upper.byte_offset),
      ),
      SeekSearchResult::Stream => (first, end),
    })
  }

  fn narrow(&mut self, ts: u64) -> Result<Narrowed> {
    let (start, end) = self.search_range(ts)?;

    let bisected = run(
      Bisection {
        reader: &mut self.reader,
        parser: &mut self.parser,
        start,
        end,
      },
      |state| bisect_step(ts, state),
    )?;

    match bisected {
      Narrowed::From(from) => {
        self.reader.seek(SeekFrom::Start(from))?;

        Ok(Narrowed::From(from))
      }
      landed @ Narrowed::Landed(_) => Ok(landed),
    }
  }
}

enum Narrowed {
  Landed(SeekedTo),
  From(u64),
}

struct Bisection<'a> {
  reader: &'a mut MediaSourceStream,
  parser: &'a mut PacketParser,
  start: u64,
  end: u64,
}

enum FrameLocation {
  Before,
  Inside,
  After,
}

impl SyncInfo {
  fn locate(&self, timestamp: u64) -> std::result::Result<FrameLocation, Never> {
    Ok(
      match (
        timestamp.cmp(&self.ts),
        timestamp.cmp(&self.ts.saturating_add(self.dur)),
      ) {
        (Ordering::Less, _) => FrameLocation::Before,
        (Ordering::Equal | Ordering::Greater, Ordering::Less) => FrameLocation::Inside,
        (Ordering::Equal | Ordering::Greater, Ordering::Equal | Ordering::Greater) => {
          FrameLocation::After
        }
      },
    )
  }
}

fn bisect_step(
  ts: u64,
  state: Bisection<'_>,
) -> std::result::Result<Step<Bisection<'_>, Result<Narrowed>>, Never> {
  let Bisection {
    reader,
    parser,
    start,
    end,
  } = state;

  match end.saturating_sub(start).cmp(&BISECTION_FLOOR) {
    Ordering::Less | Ordering::Equal => return Ok(Step::Halt(Ok(Narrowed::From(start)))),
    Ordering::Greater => (),
  }

  let mid = start.saturating_add(end).wrapping_shr(1);

  match reader.seek(SeekFrom::Start(mid)) {
    Ok(_) => (),
    Err(fault) => return halted(Error::from(fault)),
  }

  let sync = match parser.resync(reader) {
    Ok(sync) => sync,
    Err(fault) => return halted(fault),
  };

  let Ok(location) = sync.locate(ts);

  Ok(match location {
    FrameLocation::Before => Step::Again(Bisection {
      reader,
      parser,
      start,
      end: mid,
    }),
    FrameLocation::Inside => Step::Halt(Ok(Narrowed::Landed(SeekedTo {
      track_id: 0,
      actual_ts: sync.ts,
      required_ts: ts,
    }))),
    FrameLocation::After => Step::Again(Bisection {
      reader,
      parser,
      start: mid,
      end,
    }),
  })
}

struct Walk<'a> {
  reader: &'a mut MediaSourceStream,
  parser: &'a mut PacketParser,
}

fn walk_step(
  ts: u64,
  state: Walk<'_>,
) -> std::result::Result<Step<Walk<'_>, Result<SyncInfo>>, Never> {
  let Walk { reader, parser } = state;

  let sync = match parser.resync(reader) {
    Ok(sync) => sync,
    Err(fault) => return halted(fault),
  };

  let Ok(location) = sync.locate(ts);

  match (location, reader.seeking()) {
    (FrameLocation::Before, Seeking::ForwardOnly) => {
      halted(Error::SeekError(SeekErrorKind::ForwardOnly))
    }
    (FrameLocation::Before, Seeking::Anywhere) | (FrameLocation::Inside, _) => {
      Ok(Step::Halt(Ok(sync)))
    }
    (FrameLocation::After, _) => match reader.read_byte() {
      Ok(_) => Ok(Step::Again(Walk { reader, parser })),
      Err(fault) => halted(Error::from(fault)),
    },
  }
}

fn timestamp(sample_rate: Option<u32>, to: SeekTo) -> Result<u64> {
  match to {
    SeekTo::TimeStamp { ts, .. } => Ok(ts),
    SeekTo::Time { time, .. } => match sample_rate {
      Some(rate) => {
        let time_base = TimeBase::new(1, rate)?;

        let Ok(value_3) = time_base.calc_timestamp(time);

        Ok(value_3)
      }
      None => seek_error(SeekErrorKind::Unseekable),
    },
  }
}

impl QueryDescriptor for FlacReader {
  fn query() -> std::result::Result<&'static [Descriptor], console_core_never::Never> {
    Ok(&[support_format!(
      "flac",
      "Free Lossless Audio Codec Native",
      &["flac"],
      &["audio/flac"],
      &[b"fLaC"]
    )])
  }

  fn score(_context: &[u8]) -> std::result::Result<u8, console_core_never::Never> {
    Ok(255)
  }
}

impl FormatReader for FlacReader {
  fn try_new(mut source: MediaSourceStream, _options: &FormatOptions) -> Result<Self> {
    let marker = source.read_quad_bytes()?;

    match marker {
      FLAC_STREAM_MARKER => Self::init_with_metadata(source),
      _other => unsupported_error("flac: missing flac stream marker"),
    }
  }

  fn next_packet(&mut self) -> Result<Packet> {
    self.parser.parse(&mut self.reader)
  }

  fn metadata(&mut self) -> std::result::Result<Metadata<'_>, console_core_never::Never> {
    let Ok(value_4) = self.metadata.metadata();

    Ok(value_4)
  }

  fn cues(&self) -> std::result::Result<&[Cue], console_core_never::Never> {
    Ok(&self.cues)
  }

  fn tracks(&self) -> std::result::Result<&[Track], console_core_never::Never> {
    Ok(&self.tracks)
  }

  fn seek(&mut self, _mode: SeekMode, to: SeekTo) -> Result<SeekedTo> {
    let (sample_rate, n_frames) = match self.tracks.first() {
      Some(track) => (track.codec_params.sample_rate, track.codec_params.n_frames),
      None => return seek_error(SeekErrorKind::Unseekable),
    };

    let ts = timestamp(sample_rate, to)?;

    match n_frames {
      Some(known) => match ts.cmp(&known) {
        Ordering::Greater => return seek_error(SeekErrorKind::OutOfRange),
        Ordering::Less | Ordering::Equal => (),
      },
      None => (),
    }

    let narrowed = match self.reader.seeking() {
      Seeking::Anywhere => self.narrow(ts)?,
      Seeking::ForwardOnly => Narrowed::From(self.first_frame_offset),
    };

    match narrowed {
      Narrowed::Landed(seeked) => Ok(seeked),
      Narrowed::From(_) => {
        let packet = run(
          Walk {
            reader: &mut self.reader,
            parser: &mut self.parser,
          },
          |state| walk_step(ts, state),
        )?;

        Ok(SeekedTo {
          track_id: 0,
          actual_ts: packet.ts,
          required_ts: ts,
        })
      }
    }
  }

  fn into_inner(
    self: Box<Self>,
  ) -> std::result::Result<MediaSourceStream, console_core_never::Never> {
    Ok(self.reader)
  }
}

fn read_stream_info_block<B: ReadBytes + FiniteStream>(
  reader: &mut B,
  tracks: &mut Vec<Track>,
  parser: &mut Option<PacketParser>,
) -> Result<()> {
  match tracks.as_slice() {
    [] => (),
    [_, ..] => return decode_error("flac: found more than one stream info block"),
  }

  match reader.byte_len() {
    StreamInfo::BLOCK_SIZE => (),
    _ => return decode_error("flac: invalid stream info block size"),
  }

  let len = reader.byte_len();

  let extra_data = reader.read_boxed_slice_exact(len)?;

  let Ok(mut extra) = BufReader::new(&extra_data);
  let info = StreamInfo::read(&mut extra)?;

  let time_base = TimeBase::new(1, info.sample_rate)?;

  let Ok(mut codec_params) = CodecParameters::new();

  let Ok(_configured_for_codec) = codec_params.for_codec(CODEC_TYPE_FLAC);
  let Ok(_configured_with_packet_data_integrity) =
    codec_params.with_packet_data_integrity(PacketIntegrity::Guaranteed);
  let Ok(_configured_with_extra_data) = codec_params.with_extra_data(extra_data);
  let Ok(_configured_with_sample_rate) = codec_params.with_sample_rate(info.sample_rate);
  let Ok(_configured_with_time_base) = codec_params.with_time_base(time_base);
  let Ok(_configured_with_bits_per_sample) =
    codec_params.with_bits_per_sample(info.bits_per_sample);
  let Ok(_configured_with_channels) = codec_params.with_channels(info.channels);

  match info.md5 {
    Some(md5) => {
      let Ok(_configured_with_verification_code) =
        codec_params.with_verification_code(VerificationCheck::Md5(md5));
    }
    None => (),
  }

  match info.n_samples {
    Some(n_frames) => {
      let Ok(_configured_with_n_frames) = codec_params.with_n_frames(n_frames);
    }
    None => (),
  }

  let Ok(new_parser) = PacketParser::new(info);

  *parser = Some(new_parser);

  let Ok(value_5) = Track::new(0, codec_params);

  tracks.push(value_5);

  Ok(())
}
