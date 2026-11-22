//! Cutting a FLAC stream into packets, which is harder than it sounds because a frame does
//! not say how long it is.
//!
//! A frame ends where the next one's header begins, and the only way to know a header
//! from audio that happens to look like one is to read it: the sync word, the reserved bits,
//! the CRC-8 and then whether it is consistent with the stream information and the frame
//! before it. A fragment is what lies between two headers that passed. Usually that is a
//! packet, and its closing CRC-16 says so. When it is not, the header was a false one inside
//! the audio, so the fragment is held and the next one is checked as though it continued the
//! first, and the packet is the run of fragments whose CRC-16 comes out over all of them.

use std::cmp::Ordering;
use std::collections::VecDeque;

use console_core_audio_decoding_flac_and_vorbis_headers::flac::metadata::StreamInfo;
use console_core_audio_decoding_interfaces::errors::{Error, Result};
use console_core_audio_decoding_interfaces::formats::Packet;
use console_core_byte_streams::{BufReader, Monitor, ReadBytes, SeekBuffered};
use console_core_checksums::crc16::Crc16Ansi;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::frame::{
  BlockSequence, ChannelAssignment, FrameHeader, MAX_FRAME_HEADER_SIZE, MAX_FRAME_SIZE,
  Plausible, looks_like_frame_header, read_frame_header, starts_with_sync, sync_frame,
};
use crate::steps::{Infallible, halted, read_count, run};

const FRAGMENTS_KEPT: u32 = 4;

const READ_CHUNK: u32 = 1024;

const SEARCHED_ENOUGH: u32 = MAX_FRAME_SIZE.saturating_add(MAX_FRAME_HEADER_SIZE);

#[derive(Default)]
struct FrameSizes {
  recent: [u32; 4],
  count: u32,
}

impl FrameSizes {
  fn push(&mut self, value: u32) -> Infallible<()> {
    self.recent.rotate_left(1);

    let [.., newest] = &mut self.recent;

    *newest = value;
    self.count = self.count.saturating_add(1);

    Ok(())
  }

  fn reset(&mut self) -> Infallible<()> {
    self.count = 0;

    Ok(())
  }

  fn average(&self) -> Infallible<u32> {
    let Ok(kept) = fitted::<_, u32>(self.recent.len());
    let seen = self.count.min(kept);
    let Ok(newest) = index(seen);

    let total = self
      .recent
      .iter()
      .rev()
      .take(newest)
      .fold(0u32, |sum, size| sum.saturating_add(*size));

    Ok(match total.checked_div(seen) {
      Some(average) => average,
      None => 0,
    })
  }
}

pub struct SyncInfo {
  pub ts: u64,
  pub dur: u64,
}

struct ParsedPacket {
  buf: Box<[u8]>,
  sync: SyncInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Agreement {
  Matches,
  Differs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
  Accepted,
  Rejected,
}

impl From<bool> for Verdict {
  fn from(consistent: bool) -> Self {
    match bool::then_some(consistent, ()) {
      Some(()) => Self::Accepted,
      None => Self::Rejected,
    }
  }
}

struct Running {
  crc16: Crc16Ansi,
  total_len: u32,
}

struct Fragment {
  data: Box<[u8]>,
  footer: u16,
  agreement: Agreement,
  running: Running,
}

fn agreement(checks: [u16; 2]) -> Infallible<Agreement> {
  let [expected, computed] = checks;

  Ok(match expected.cmp(&computed) {
    Ordering::Equal => Agreement::Matches,
    Ordering::Less | Ordering::Greater => Agreement::Differs,
  })
}

impl Fragment {
  fn new(data: Box<[u8]>) -> Infallible<Option<Fragment>> {
    let (top, bottom) = match data.split_last_chunk::<2>() {
      Some(parts) => parts,
      None => return Ok(None),
    };

    let footer = u16::from_be_bytes(*bottom);

    let Ok(mut crc16) = Crc16Ansi::new(0);
    let Ok(()) = crc16.process_buf_bytes(top);
    let Ok(computed) = crc16.crc();
    let Ok(()) = crc16.process_buf_bytes(bottom);
    let Ok(total_len) = fitted::<_, u32>(data.len());

    let Ok(agreed) = agreement([footer, computed]);

    Ok(Some(Fragment {
      data,
      footer,
      agreement: agreed,
      running: Running { crc16, total_len },
    }))
  }

  fn update(&mut self, frag: &Fragment) -> Infallible<Agreement> {
    let Ok(len) = fitted::<_, u32>(frag.data.len());

    self.running.total_len = self.running.total_len.saturating_add(len);

    match frag.data.split_last_chunk::<2>() {
      Some((top, bottom)) => {
        let Ok(()) = self.running.crc16.process_buf_bytes(top);
        let Ok(computed) = self.running.crc16.crc();
        let Ok(()) = self.running.crc16.process_buf_bytes(bottom);

        agreement([frag.footer, computed])
      }
      None => Ok(Agreement::Differs),
    }
  }

  fn parse_header(&self) -> Result<FrameHeader> {
    let Ok(mut reader) = BufReader::new(&self.data);
    let sync = reader.read_be_u16()?;

    read_frame_header(&mut reader, sync)
  }
}

#[derive(Default)]
struct PacketBuilder {
  frags: VecDeque<Fragment>,
  max_size: Option<u32>,
  avg_size: Option<u32>,
  last_header: Option<FrameHeader>,
}

impl PacketBuilder {
  fn max_frame_size(&self) -> Infallible<u32> {
    Ok(match self.max_size {
      Some(size) => size,
      None => MAX_FRAME_SIZE,
    })
  }

  fn max_average_frame_size(&self) -> Infallible<u32> {
    Ok(match self.avg_size {
      Some(size) => size.saturating_mul(4),
      None => MAX_FRAME_SIZE,
    })
  }

  fn push_fragment(&mut self, frag: Fragment) -> Infallible<()> {
    let Ok(max) = self.max_frame_size();
    let Ok(max_average) = self.max_average_frame_size();
    let Ok(held) = fitted::<_, u32>(self.frags.len());

    let oldest_when_full = self.frags.front().filter(|first| {
      first.running.total_len > max
        || first.running.total_len > max_average
        || held >= FRAGMENTS_KEPT
    });

    match oldest_when_full {
      Some(_) => {
        let _dropped = self.frags.pop_front();
      }
      None => (),
    }

    self.frags.push_back(frag);

    Ok(())
  }

  fn merged(&self, from: u32, frag: Fragment) -> Infallible<Box<[u8]>> {
    let Ok(skip) = index(from);

    let held = self.frags.iter().skip(skip).map(|held| held.data.as_ref());
    let parts: Vec<&[u8]> = held.chain(std::iter::once(frag.data.as_ref())).collect();

    Ok(parts.concat().into_boxed_slice())
  }

  fn try_build(
    &mut self,
    stream_info: &StreamInfo,
    frag: Fragment,
  ) -> Result<Option<ParsedPacket>> {
    let (header, data) = match frag.agreement {
      Agreement::Matches => {
        let header = frag.parse_header()?;

        (header, frag.data)
      }
      Agreement::Differs => {
        let located =
          self
            .frags
            .iter_mut()
            .zip(0u32..)
            .find_map(|(held, at)| match held.update(&frag) {
              Ok(Agreement::Matches) => Some((at, held.parse_header())),
              Ok(Agreement::Differs) => None,
            });

        match located {
          Some((at, header)) => {
            let header_2 = header?;
            let Ok(data) = self.merged(at, frag);

            (header_2, data)
          }
          None => {
            let Ok(()) = self.push_fragment(frag);

            return Ok(None);
          }
        }
      }
    };

    self.frags.clear();

    let Ok(sync) = calc_sync_info(stream_info, &header);

    self.last_header = Some(header);

    Ok(Some(ParsedPacket { buf: data, sync }))
  }

  fn reset(&mut self) -> Infallible<()> {
    self.frags.clear();
    self.last_header = None;

    Ok(())
  }
}

pub struct PacketParser {
  info: StreamInfo,
  fsma: FrameSizes,
  builder: PacketBuilder,
}

struct Scan<'reader, B> {
  reader: &'reader mut B,
  buf: Vec<u8>,
  end: u32,
  pos: u32,
}

fn truncated(mut buf: Vec<u8>, size: u32) -> Result<Option<Fragment>> {
  let Ok(at) = index(size);

  buf.truncate(at);

  let Ok(fragment) = Fragment::new(buf.into_boxed_slice());

  Ok(fragment)
}

fn read_more<B: ReadBytes + SeekBuffered>(
  mut scan: Scan<'_, B>,
) -> std::result::Result<Step<Scan<'_, B>, Result<Option<Fragment>>>, Never> {
  match scan.end.cmp(&SEARCHED_ENOUGH) {
    Ordering::Equal | Ordering::Greater => return Ok(Step::Halt(Ok(None))),
    Ordering::Less => (),
  }

  let next_end = scan.end.saturating_add(READ_CHUNK);
  let Ok(next) = index(next_end);
  let Ok(from) = index(scan.end);

  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "the scan returns at SEARCHED_ENOUGH; the next read grows the measured buffer by at most one READ_CHUNK beyond that ceiling"
    )
  )]
  scan.buf.resize(scan.buf.len().max(next), 0);
  scan.pos = scan.end.saturating_sub(MAX_FRAME_HEADER_SIZE);

  let read = match scan.buf.get_mut(from..next) {
    Some(chunk) => read_count(scan.reader, chunk),
    None => {
      return halted(Error::DecodeError(
        "flac: the read buffer is shorter than it was made",
      ));
    }
  };

  Ok(match read {
    Ok(got) => {
      scan.end = scan.end.saturating_add(got);

      Step::Again(scan)
    }
    Err(_ended) => Step::Halt(truncated(scan.buf, scan.end)),
  })
}

fn scan_step<'reader, B: ReadBytes + SeekBuffered>(
  parser: &PacketParser,
  scan: Scan<'reader, B>,
) -> std::result::Result<Step<Scan<'reader, B>, Result<Option<Fragment>>>, Never> {
  let Ok(end) = index(scan.end);

  let found = match scan.buf.get(..end) {
    Some(data) => {
      let Ok(found) = parser.next_frame(data, scan.pos);

      found
    }
    None => None,
  };

  match found {
    Some(size) => {
      let Ok(()) = scan
        .reader
        .seek_buffered_rev(u64::from(scan.end.saturating_sub(size)));

      Ok(Step::Halt(truncated(scan.buf, size)))
    }
    None => read_more(scan),
  }
}

fn pull_step<'parser, 'reader, B: ReadBytes + SeekBuffered>(
  average: u32,
  state: (&'parser mut PacketParser, &'reader mut B),
) -> std::result::Result<
  Step<(&'parser mut PacketParser, &'reader mut B), Result<ParsedPacket>>,
  Never,
> {
  let (parser, reader) = state;

  let fragment = match parser.read_fragment(reader, average) {
    Ok(fragment) => fragment,
    Err(fault) => return halted(fault),
  };

  Ok(match parser.builder.try_build(&parser.info, fragment) {
    Ok(Some(packet)) => Step::Halt(Ok(packet)),
    Ok(None) => Step::Again((parser, reader)),
    Err(fault) => Step::Halt(Err(fault)),
  })
}

fn fragment_step<'parser, 'reader, B: ReadBytes + SeekBuffered>(
  average: u32,
  state: (&'parser mut PacketParser, &'reader mut B),
) -> std::result::Result<Step<(&'parser mut PacketParser, &'reader mut B), Result<Fragment>>, Never>
{
  let (parser, reader) = state;

  Ok(match parser.try_read_fragment(reader, average) {
    Ok(Some(fragment)) => Step::Halt(Ok(fragment)),
    Ok(None) => match parser.resync(reader) {
      Ok(_resynchronized) => Step::Again((parser, reader)),
      Err(fault) => Step::Halt(Err(fault)),
    },
    Err(fault) => Step::Halt(Err(fault)),
  })
}

type Hunted<'reader, B> = Step<&'reader mut B, Result<(FrameHeader, u64)>>;

fn hunt_step<'reader, B: ReadBytes + SeekBuffered>(
  info: &StreamInfo,
  reader: &'reader mut B,
) -> std::result::Result<Hunted<'reader, B>, Never> {
  let sync = match sync_frame(reader) {
    Ok(sync) => sync,
    Err(fault) => return halted(fault),
  };

  let frame_pos = reader.pos().saturating_sub(2);

  let accepted = match read_frame_header(reader, sync) {
    Ok(header) => match strict_frame_header_check(info, &header, None) {
      Ok(Verdict::Accepted) => Some(header),
      Ok(Verdict::Rejected) => None,
    },
    Err(_not_a_header) => None,
  };

  Ok(match accepted {
    Some(header) => Step::Halt(Ok((header, frame_pos))),
    None => {
      reader.seek_buffered(frame_pos.saturating_add(1));

      Step::Again(reader)
    }
  })
}

impl PacketParser {
  pub fn new(info: StreamInfo) -> Infallible<PacketParser> {
    let max_size = match info.frame_byte_len_max {
      0 => None,
      size => Some(size),
    };

    Ok(PacketParser {
      info,
      fsma: FrameSizes::default(),
      builder: PacketBuilder {
        max_size,
        ..PacketBuilder::default()
      },
    })
  }

  fn soft_reset(&mut self) -> Infallible<()> {
    let Ok(()) = self.builder.reset();
    let Ok(()) = self.fsma.reset();

    Ok(())
  }

  fn frame_at(&self, frame: Option<&[u8]>) -> Infallible<Verdict> {
    let frame_3 = match frame {
      Some(frame_2) => frame_2,
      None => return Ok(Verdict::Rejected),
    };

    match looks_like_frame_header(frame_3) {
      Ok(Plausible::Noise) | Err(_) => return Ok(Verdict::Rejected),
      Ok(Plausible::Header) => (),
    }

    let (sync, after_sync) = match frame_3.split_first_chunk::<2>() {
      Some((pair, after_sync)) => (u16::from_be_bytes(*pair), after_sync),
      None => return Ok(Verdict::Rejected),
    };

    let Ok(mut after) = BufReader::new(after_sync);

    match read_frame_header(&mut after, sync) {
      Ok(header) => {
        strict_frame_header_check(&self.info, &header, self.builder.last_header.as_ref())
      }
      Err(_not_a_header) => Ok(Verdict::Rejected),
    }
  }

  fn next_frame(&self, data: &[u8], from: u32) -> Infallible<Option<u32>> {
    let Ok(skip) = index(from);

    let found = data
      .windows(2)
      .zip(0u32..)
      .skip(skip)
      .filter(|(pair, _)| matches!(starts_with_sync(pair), Ok(Plausible::Header)))
      .find(|(_, at)| {
        let Ok(start) = index(*at);

        matches!(self.frame_at(data.get(start..)), Ok(Verdict::Accepted))
      });

    Ok(found.map(|(_, at)| at))
  }

  fn try_read_fragment<B: ReadBytes + SeekBuffered>(
    &self,
    reader: &mut B,
    avg_frame_size: u32,
  ) -> Result<Option<Fragment>> {
    let init_read_size = avg_frame_size
      .clamp(1024, 32768)
      .saturating_add(MAX_FRAME_HEADER_SIZE);
    let Ok(size) = index(init_read_size);

    #[cfg_attr(
      dylint_lib = "explicit088_no_arriving_length",
      expect(
        explicit088_no_arriving_length,
        reason = "the initial read is capped at 32768 bytes plus MAX_FRAME_HEADER_SIZE before converting to an allocation index"
      )
    )]
    let mut buf: Vec<u8> = vec![0; size];

    let end = read_count(reader, &mut buf)?;

    run(
      Scan {
        reader,
        buf,
        end,
        pos: 1,
      },
      |scan| scan_step(self, scan),
    )
  }

  fn read_fragment<B: ReadBytes + SeekBuffered>(
    &mut self,
    reader: &mut B,
    avg_frame_size: u32,
  ) -> Result<Fragment> {
    run((self, reader), |state| fragment_step(avg_frame_size, state))
  }

  pub fn parse<B: ReadBytes + SeekBuffered>(&mut self, reader: &mut B) -> Result<Packet> {
    let Ok(avg_frame_size) = self.fsma.average();

    self.builder.avg_size = Some(avg_frame_size);

    let parsed = run((&mut *self, reader), |state| {
      pull_step(avg_frame_size, state)
    })?;

    let Ok(len) = fitted::<_, u32>(parsed.buf.len());

    let Ok(()) = self.fsma.push(len);

    let Ok(value_1) =
      Packet::new_from_boxed_slice(0, parsed.sync.ts, parsed.sync.dur, parsed.buf);

    Ok(value_1)
  }

  pub fn resync<B: ReadBytes + SeekBuffered>(&mut self, reader: &mut B) -> Result<SyncInfo> {
    let init_pos = reader.pos();

    let (header, frame_pos) = run(&mut *reader, |reader_2| hunt_step(&self.info, reader_2))?;

    let Ok(sync) = calc_sync_info(&self.info, &header);

    reader.seek_buffered(frame_pos);

    match init_pos.cmp(&reader.pos()) {
      Ordering::Equal => (),
      Ordering::Less | Ordering::Greater => {
        let Ok(()) = self.soft_reset();
      }
    }

    Ok(sync)
  }
}

fn calc_sync_info(stream_info: &StreamInfo, header: &FrameHeader) -> Infallible<SyncInfo> {
  let block_sizes = stream_info.block_len_max.cmp(&stream_info.block_len_min);
  let dur = u64::from(header.block_num_samples);

  let ts = match (header.block_sequence, block_sizes) {
    (BlockSequence::BySample(sample), _) => sample,
    (BlockSequence::ByFrame(frame), Ordering::Equal) => {
      u64::from(frame).saturating_mul(u64::from(stream_info.block_len_min))
    }
    (BlockSequence::ByFrame(frame), Ordering::Less | Ordering::Greater) => {
      u64::from(frame).saturating_mul(dur)
    }
  };

  Ok(SyncInfo { ts, dur })
}

fn strict_frame_header_check(
  stream_info: &StreamInfo,
  header: &FrameHeader,
  last_header: Option<&FrameHeader>,
) -> Infallible<Verdict> {
  let sample_rate_agrees = header
    .sample_rate
    .is_none_or(|rate| rate == stream_info.sample_rate);
  let depth_agrees = header
    .bits_per_sample
    .is_none_or(|bits| bits == stream_info.bits_per_sample);

  let is_fixed = stream_info.block_len_min == stream_info.block_len_max;
  let within_bounds = header.block_num_samples <= stream_info.block_len_max;

  let last_seq = match last_header {
    Some(last) => match last.block_sequence {
      BlockSequence::BySample(sample) => sample,
      BlockSequence::ByFrame(frame) => u64::from(frame),
    },
    None => 0,
  };

  let is_monotonic = match header.block_sequence {
    BlockSequence::BySample(sample) => !is_fixed && (sample > last_seq || sample == 0),
    BlockSequence::ByFrame(frame) => is_fixed && (u64::from(frame) > last_seq || frame == 0),
  };

  let num_frame_channels = match header.channel_assignment {
    ChannelAssignment::Independent(num) => num,
    ChannelAssignment::LeftSide | ChannelAssignment::MidSide | ChannelAssignment::RightSide => 2,
  };

  let stream_channels = stream_info.channels.count();

  let consistent = sample_rate_agrees && depth_agrees && within_bounds && is_monotonic && {
    let Ok(value_2) = stream_channels;

    num_frame_channels == value_2
  };

  Ok(Verdict::from(consistent))
}
