//! A synthetic moving pattern encoded with libvpx exercises inter frames at
//! 320×200 and 256×160 that reference both sizes.

use std::{fmt, hint::black_box};

use console_core_number_conversion::{checked_index, fixed_index};
use console_core_video_decoding_vp9::{DecodeStats, Decoder, Error};
use criterion::{Criterion, Throughput};

const CLIP: &[u8] = include_bytes!("fixtures/scaled-reference.ivf");
const FILE_HEADER: u32 = 32;
const PACKET_HEADER: u32 = 12;

#[derive(Debug)]
enum FixtureError {
  InvalidHeader,
  TruncatedPacket,
  PacketSize,
  TimestampOverflow,
  UnexpectedStatistics(DecodeStats),
  Decode(Error),
}

impl fmt::Display for FixtureError {
  fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::InvalidHeader => output.write_str("invalid VP9 IVF file header"),
      Self::TruncatedPacket => output.write_str("truncated IVF packet"),
      Self::PacketSize => output.write_str("IVF packet exceeds addressable memory"),
      Self::TimestampOverflow => output.write_str("frame index exceeds the timestamp range"),
      Self::UnexpectedStatistics(stats) => {
        write!(output, "unexpected fixture statistics: {stats:?}")
      }
      Self::Decode(error) => write!(output, "fixture decode failed: {error}"),
    }
  }
}

fn packets(bytes: &[u8]) -> Result<Vec<&[u8]>, FixtureError> {
  let Ok(file_header) = console_core_number_conversion::index(FILE_HEADER);
  let Ok(packet_header) = console_core_number_conversion::index(PACKET_HEADER);
  let (header, body) = bytes
    .split_at_checked(file_header)
    .ok_or(FixtureError::InvalidHeader)?;

  match (header.get(..4), header.get(8..12)) {
    (Some(b"DKIF"), Some(b"VP90")) => {}
    _ => return Err(FixtureError::InvalidHeader),
  }

  let Ok(capacity) = fixed_index(body.chunks_exact(packet_header.max(1)).len());
  let mut unread = body;
  let mut packets = Vec::new();

  for _ in 0..capacity {
    match unread {
      [] => break,
      [_, ..] => {}
    }

    let (record, payload) = unread
      .split_at_checked(packet_header)
      .ok_or(FixtureError::TruncatedPacket)?;
    let size = record
      .first_chunk::<4>()
      .ok_or(FixtureError::TruncatedPacket)?;
    let packet_size =
      checked_index(u32::from_le_bytes(*size)).map_err(|_| FixtureError::PacketSize)?;
    let (packet, next) = payload
      .split_at_checked(packet_size)
      .ok_or(FixtureError::TruncatedPacket)?;
    packets.push(packet);
    unread = next;
  }

  match unread {
    [] => Ok(packets),
    [_, ..] => Err(FixtureError::TruncatedPacket),
  }
}

fn decode(packets: &[&[u8]]) -> Result<DecodeStats, FixtureError> {
  let Ok(mut decoder) = Decoder::with_threads(1);

  for (frame, packet) in packets.iter().enumerate() {
    let timestamp = i64::try_from(frame).map_err(|_| FixtureError::TimestampOverflow)?;
    let pictures = decoder
      .decode(packet, timestamp)
      .map_err(FixtureError::Decode)?;
    drop(black_box(pictures));
  }

  let Ok(pending) = decoder.flush();
  drop(black_box(pending));
  let Ok(stats) = decoder.stats();
  Ok(stats)
}

fn main() -> Result<(), FixtureError> {
  let packets = packets(CLIP)?;
  let stats = decode(&packets)?;

  match (stats.shown, stats.scaled_ref_blocks) {
    (24, 1..) => {}
    _ => return Err(FixtureError::UnexpectedStatistics(stats)),
  }

  let mut criterion = Criterion::default().configure_from_args();
  let mut group = criterion.benchmark_group("vp9_scaled_reference");
  group.throughput(Throughput::Elements(stats.shown));
  group.bench_function("decode", |bencher| {
    bencher.iter(|| black_box(decode(black_box(&packets))));
  });
  group.finish();
  criterion.final_summary();
  Ok(())
}
