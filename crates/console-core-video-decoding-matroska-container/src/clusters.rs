//! A cluster's blocks, read as headers, and the samples they become.
//!
//! A cluster is walked to its end, or for one of unknown size to the first
//! level-1 element after it, and only the headers of its children are read:
//! the Timestamp every block is relative to, and of each block the track, the
//! relative timestamp, the flags and where each laced frame lies. A laced
//! block's lace header is read with its frames, because the sizes are in front
//! of them; nothing else of a frame is.
//!
//! Damage ends the cluster rather than the file. A child that does not decode
//! or claims more than the cluster holds is a `Warning`, and the walk goes on
//! from the next element that looks like a level-1 one -- except a block cut off
//! by the end of the file, which is the last thing in it and is dropped.
//!
//! A laced block's frames share its duration -- BlockDuration, or the track's
//! DefaultDuration once per frame -- and each frame's timestamp is the block's
//! plus its share, rounded half away from zero to a tick. CodecDelay is taken
//! from every timestamp (RFC 9559 §5.1.4.1.25), rounded to ticks once so the
//! tick timestamps stay on the block grid.

use std::collections::BTreeMap;

use console_core_byte_sources::ByteSource;
use console_core_extensible_binary_meta_language::{Size, uint};
use console_core_iteration::Step;
use console_core_matroska_blocks::{BlockHeader, Lacing, parse_lacing};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, wrapped};

use crate::demux::MkvFile;
use crate::error::{Error, Warning};
use crate::ids::{
  BLOCK, BLOCK_DURATION, BLOCK_GROUP, Level, REFERENCE_BLOCK, SIMPLE_BLOCK, TIMESTAMP, level,
};
use crate::meta::ClusterInfo;
use crate::states::{ElementBounds, FrameKind};
use crate::track::{Sample, Track, gcd};
use crate::window::{Header, Span, Window, walk};

const BLOCK_HEAD: u64 = 16;

const KEYFRAME_FLAG: u8 = 0x80;

const NANOSECONDS: u64 = 1_000_000_000;

pub(crate) type Numbers = BTreeMap<u64, u64>;

#[derive(Clone, Copy, Debug)]
struct Frame {
  offset: u64,
  size: u32,
}

#[derive(Clone, Debug)]
struct Parsed {
  track: u64,
  relative: i16,
  flags: u8,
  frames: Vec<Frame>,
}

#[derive(Clone, Debug)]
pub(crate) struct Block {
  track: u64,
  block_offset: u64,
  timestamp: i64,
  frames: Vec<Frame>,
  duration: Option<u64>,
  kind: FrameKind,
}

#[derive(Clone, Debug)]
struct Group {
  block: Option<Parsed>,
  duration: Option<u64>,
  kind: FrameKind,
}

#[derive(Clone, Copy)]
struct Cluster<'n> {
  header: Header,
  segment_end: u64,
  known_end: Option<u64>,
  end: u64,
  numbers: &'n Numbers,
}

struct Scanning<'w, 'a, S: ByteSource + ?Sized> {
  window: &'w mut Window<'a, S>,
  warnings: &'w mut Vec<Warning>,
  at: u64,
  timestamp: i64,
  blocks: Vec<Block>,
}

pub(crate) fn numbers(tracks: &[Track]) -> Result<Numbers, Never> {
  let mut numbers = Numbers::new();

  for (track, at) in tracks.iter().zip(0u64..) {
    let _first_kept = numbers.entry(track.number).or_insert(at);
  }

  Ok(numbers)
}

fn parse_block<S: ByteSource + ?Sized>(
  window: &mut Window<'_, S>,
  body: Span,
  numbers: &Numbers,
) -> Result<Option<Parsed>, Error> {
  let head = window.peek(Span {
    start: body.start,
    end: body.start.saturating_add(BLOCK_HEAD).min(body.end),
  })?;
  let Ok(read) = BlockHeader::parse(head);
  let header = match read {
    Some(header) => header,
    None => return Ok(None),
  };
  let track = match numbers.get(&header.track) {
    Some(at) => *at,
    None => return Ok(None),
  };
  let frames_at = body.start.saturating_add(u64::from(header.header_len));
  let Ok(lacing) = Lacing::from_flags(header.flags);

  let frames = match lacing {
    Lacing::None => {
      let Ok(size) = wrapped::<u64, u32>(body.end.saturating_sub(frames_at));

      vec![Frame {
        offset: frames_at,
        size,
      }]
    }
    Lacing::Xiph | Lacing::FixedSize | Lacing::Ebml => {
      let laced = window.read_vec(Span {
        start: frames_at,
        end: body.end,
      })?;

      match parse_lacing(lacing, &laced) {
        Ok(found) => found
          .iter()
          .map(|frame| {
            let Ok(size) = wrapped::<u64, u32>(frame.len);

            Frame {
              offset: frames_at.saturating_add(frame.offset),
              size,
            }
          })
          .collect(),
        Err(_damaged) => return Ok(None),
      }
    }
  };

  Ok(Some(Parsed {
    track,
    relative: header.relative_timestamp,
    flags: header.flags,
    frames,
  }))
}

fn read_group_child<S: ByteSource + ?Sized>(
  window: &mut Window<'_, S>,
  child: Header,
  mut group: Group,
  limit: u64,
  numbers: &Numbers,
) -> Result<Group, Error> {
  match child.id {
    BLOCK => {
      let Ok(body) = child.body(limit);
      let parsed = parse_block(window, body, numbers)?;

      group.block = parsed;
    }
    BLOCK_DURATION => {
      let value = window.value(&child, limit)?;
      let Ok(duration) = uint(value);

      group.duration = Some(duration);
    }
    REFERENCE_BLOCK => group.kind = FrameKind::Delta,
    _ => {}
  }

  Ok(group)
}

fn parse_block_elem<S: ByteSource + ?Sized>(
  window: &mut Window<'_, S>,
  header: &Header,
  limit: u64,
  cluster_timestamp: i64,
  numbers: &Numbers,
) -> Result<Option<Block>, Error> {
  let Ok(body) = header.body(limit);
  let started = |parsed: Parsed, duration: Option<u64>, kind: FrameKind| Block {
    track: parsed.track,
    block_offset: header.at,
    timestamp: cluster_timestamp.wrapping_add(i64::from(parsed.relative)),
    frames: parsed.frames,
    duration,
    kind,
  };

  match header.id {
    SIMPLE_BLOCK => {
      let parsed = parse_block(window, body, numbers)?;

      Ok(parsed.map(|parsed_2| {
        let kind = match parsed_2.flags & KEYFRAME_FLAG {
          0 => FrameKind::Delta,
          _keyframe => FrameKind::Keyframe,
        };

        started(parsed_2, None, kind)
      }))
    }
    BLOCK_GROUP => {
      let empty = Group {
        block: None,
        duration: None,
        kind: FrameKind::Keyframe,
      };
      let group = window.siblings(body, empty, |window_2, child, group| {
        read_group_child(window_2, child, group, body.end, numbers)
      })?;

      Ok(
        group
          .block
          .map(|parsed| started(parsed, group.duration, group.kind)),
      )
    }
    _ => Ok(None),
  }
}

fn resynced<S: ByteSource + ?Sized>(
  window: &mut Window<'_, S>,
  span: Span,
) -> Result<u64, Error> {
  let found = window.resync(span)?;

  Ok(match found {
    Some(at) => at,
    None => span.end,
  })
}

fn finished<S: ByteSource + ?Sized>(
  scanning: Scanning<'_, '_, S>,
  end: u64,
) -> Result<(u64, i64, Vec<Block>), Never> {
  Ok((end, scanning.timestamp, scanning.blocks))
}

type Scanned = (u64, i64, Vec<Block>);

fn damaged_end<S: ByteSource + ?Sized>(
  scanning: &mut Scanning<'_, '_, S>,
  child: &Header,
  bounds: ElementBounds,
  cluster: &Cluster<'_>,
) -> Result<u64, Error> {
  let Ok(start) = child.data_start();
  let after = Span {
    start: scanning.at.saturating_add(1),
    end: cluster.segment_end,
  };

  scanning.warnings.push(Warning::OverrunningBlock {
    cluster: cluster.header.at,
    id: child.id,
    at: scanning.at,
  });

  match (child.size, bounds, child.id) {
    (Size::Known(size), ElementBounds::Overrun, SIMPLE_BLOCK | BLOCK_GROUP) => {
      match ::core::primitive::bool::then_some(
        scanning.window.len <= start.saturating_add(size),
        (),
      ) {
        ::core::option::Option::Some(()) => Ok(cluster.end),
        ::core::option::Option::None => resynced(scanning.window, after),
      }
    }
    (Size::Known(_) | Size::Unknown, ElementBounds::Overrun | ElementBounds::Within, _) => {
      resynced(scanning.window, after)
    }
  }
}

fn scan_step<'w, 'a, S: ByteSource + ?Sized>(
  mut scanning: Scanning<'w, 'a, S>,
  cluster: &Cluster<'_>,
) -> Result<Step<Scanning<'w, 'a, S>, Scanned>, Error> {
  match ::core::primitive::bool::then_some(scanning.at < cluster.end, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      let Ok(done) = finished(scanning, cluster.end);

      return Ok(Step::Halt(done));
    }
  }

  let found = scanning.window.header(Span {
    start: scanning.at,
    end: cluster.end,
  })?;
  let child = match found {
    Some(child) => child,
    None => {
      scanning.warnings.push(Warning::DamagedCluster {
        cluster: cluster.header.at,
        at: scanning.at,
      });

      let end = resynced(
        scanning.window,
        Span {
          start: scanning.at.saturating_add(1),
          end: cluster.segment_end,
        },
      )?;
      let Ok(done) = finished(scanning, end);

      return Ok(Step::Halt(done));
    }
  };
  let Ok(standing) = level(child.id);
  let Ok(bounds) = child.overruns(cluster.end);

  match (cluster.known_end, standing, child.size, bounds) {
    (None, Level::Top, _, _) => {
      let at = scanning.at;
      let Ok(done) = finished(scanning, at);

      return Ok(Step::Halt(done));
    }
    (Some(_), _, Size::Unknown, _)
    | (None, Level::Nested, Size::Unknown, _)
    | (_, _, Size::Known(_), ElementBounds::Overrun) => {
      let end = damaged_end(&mut scanning, &child, bounds, cluster)?;
      let Ok(done) = finished(scanning, end);

      return Ok(Step::Halt(done));
    }
    (Some(_), _, Size::Known(_), ElementBounds::Within)
    | (None, Level::Nested, Size::Known(_), ElementBounds::Within) => {}
  }

  let Ok(child_end) = child.end(cluster.end);

  match child.id {
    TIMESTAMP => {
      let value = scanning.window.value(&child, cluster.end)?;
      let Ok(ticks) = uint(value);
      let Ok(timestamp) = wrapped::<u64, i64>(ticks);

      scanning.timestamp = timestamp;
    }
    SIMPLE_BLOCK | BLOCK_GROUP => {
      let read = parse_block_elem(
        scanning.window,
        &child,
        cluster.end,
        scanning.timestamp,
        cluster.numbers,
      )?;

      scanning.blocks.extend(read);
    }
    _ => {}
  }

  scanning.at = child_end;

  Ok(Step::Again(scanning))
}

pub(crate) fn scan_cluster<S: ByteSource + ?Sized>(
  window: &mut Window<'_, S>,
  header: &Header,
  segment_end: u64,
  numbers: &Numbers,
  warnings: &mut Vec<Warning>,
) -> Result<(ClusterInfo, Vec<Block>), Error> {
  let Ok(data_start) = header.data_start();
  let known_end = match header.size {
    Size::Known(_) => {
      let Ok(end) = header.end(segment_end);

      Some(end)
    }
    Size::Unknown => None,
  };
  let end = match known_end {
    Some(end) => end,
    None => segment_end,
  };
  let cluster = Cluster {
    header: *header,
    segment_end,
    known_end,
    end,
    numbers,
  };
  let scanning = Scanning {
    window,
    warnings,
    at: data_start,
    timestamp: 0,
    blocks: Vec::new(),
  };
  let (end_2, timestamp, blocks) = walk(scanning, |scanning_2| scan_step(scanning_2, &cluster))?;
  let Ok(timestamp_2) = wrapped::<i64, u64>(timestamp.max(0));

  Ok((
    ClusterInfo {
      offset: header.at,
      data_start,
      end: end_2,
      timestamp: timestamp_2,
    },
    blocks,
  ))
}

fn to_ticks(nanoseconds: i128, scale: u64) -> Result<i64, Never> {
  let scale_2 = i128::from(scale);
  let half = scale_2.wrapping_div(2);
  let nudged = match ::core::primitive::bool::then_some(nanoseconds >= 0, ()) {
    ::core::option::Option::Some(()) => nanoseconds.wrapping_add(half),
    ::core::option::Option::None => nanoseconds.wrapping_sub(half),
  };
  let ticks = match nudged.checked_div(scale_2) {
    Some(ticks) => ticks,
    None => 0,
  };

  wrapped::<i128, i64>(ticks)
}

fn share(total: u128, part: u16, count: u64) -> Result<i128, Never> {
  let shared = match total
    .wrapping_mul(u128::from(part))
    .checked_div(u128::from(count))
  {
    Some(shared) => shared,
    None => 0,
  };

  wrapped::<u128, i128>(shared)
}

fn add_block(track: &mut Track, block: &Block, scale: u64, cluster: u32) -> Result<(), Never> {
  let Ok(count) = fitted::<_, u64>(block.frames.len());
  let total = match (block.duration, track.default_duration_ns) {
    (Some(ticks), _) => Some(u128::from(ticks).wrapping_mul(u128::from(scale))),
    (None, Some(each)) => Some(u128::from(each).wrapping_mul(u128::from(count))),
    (None, None) => None,
  };
  let Ok(delay_ticks) = to_ticks(i128::from(track.codec_delay_ns), scale);
  let base = i128::from(block.timestamp).wrapping_mul(i128::from(scale));
  let keyframe = matches!(block.kind, FrameKind::Keyframe);

  for (frame, lace) in block.frames.iter().zip(0u16..) {
    let (start, end) = match total {
      Some(total_2) => {
        let Ok(start) = share(total_2, lace, count);
        let Ok(end) = share(total_2, lace.saturating_add(1), count);

        (start, Some(end))
      }
      None => (0, None),
    };
    let Ok(at) = to_ticks(base.wrapping_add(start), scale);
    let Ok(start_ticks) = to_ticks(start, scale);
    let duration = match end {
      Some(end_2) => {
        let Ok(end_ticks) = to_ticks(end_2, scale);
        let Ok(duration) = wrapped::<i64, u64>(end_ticks.wrapping_sub(start_ticks).max(0));

        duration
      }
      None => 0,
    };
    let Ok(position) = fitted::<_, u32>(track.samples.len());

    match ::core::primitive::bool::then_some(keyframe, ()) {
      ::core::option::Option::Some(()) => track.sync.push(position),
      ::core::option::Option::None => {}
    }

    track.samples.push(Sample {
      offset: frame.offset,
      size: frame.size,
      pts: at.wrapping_sub(delay_ticks),
      duration,
      keyframe,
      cluster,
      block_offset: block.block_offset,
      lace,
    });
  }

  Ok(())
}

pub(crate) fn add_blocks(
  file: &mut MkvFile,
  cluster: u32,
  blocks: &[Block],
) -> Result<(), Never> {
  let scale = file.info.timestamp_scale;

  for block in blocks {
    let Ok(at) = index(block.track);

    match file.tracks.get_mut(at) {
      Some(track) => {
        let Ok(()) = add_block(track, block, scale, cluster);
      }
      None => {}
    }
  }

  Ok(())
}

pub(crate) fn finish_tracks(file: &mut MkvFile) -> Result<(), Never> {
  let scale = file.info.timestamp_scale;
  let Ok(common) = gcd([scale, NANOSECONDS]);

  for track in &mut file.tracks {
    track.timebase = (scale / common, NANOSECONDS / common);
  }

  Ok(())
}
