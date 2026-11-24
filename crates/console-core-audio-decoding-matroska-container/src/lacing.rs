//! A block cut into the frames it carries, each with its own timestamp.
//!
//! Where a block begins and where each laced frame inside it does is
//! `console-core-matroska-blocks`'s to say; what this adds is the clock: a
//! frame's place is the cluster's plus the block's own offset, and every frame
//! after the first in a laced block is one frame duration later than the one
//! before it.

use std::collections::HashMap;

use console_core_matroska_blocks::{BlockHeader, Frame as Laced, Lacing, parse_lacing};
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::fault::{DecodeError, Invalid};

pub(crate) struct Frame {
  pub(crate) track: u32,
  pub(crate) timestamp: u64,
  pub(crate) duration: u64,
  pub(crate) data: Box<[u8]>,
}

pub(crate) fn absolute_timestamp(cluster: u64, relative: i16) -> Result<u64, Never> {
  Ok(cluster.saturating_add_signed(i64::from(relative)))
}

pub(crate) fn extract_frames(
  block: &[u8],
  block_duration: Option<u64>,
  default_durations: &HashMap<u32, u64>,
  cluster: u64,
) -> Result<Vec<Frame>, DecodeError> {
  let Ok(parsed) = BlockHeader::parse(block);
  let header = parsed.ok_or(DecodeError::Invalid(Invalid::BlockHeader))?;
  let track = u32::try_from(header.track)
    .map_err(|_wider_than_a_track_id| DecodeError::Invalid(Invalid::TrackNumber))?;
  let Ok(payload_at) = index(header.header_len);
  let payload = block
    .get(payload_at..)
    .ok_or(DecodeError::Invalid(Invalid::BlockHeader))?;
  let Ok(lacing) = Lacing::from_flags(header.flags);
  let laced = parse_lacing(lacing, payload)
    .map_err(|_malformed_lace| DecodeError::Invalid(Invalid::Lacing))?;
  let count = u64::try_from(laced.len())
    .map_err(|_wider_than_a_count| DecodeError::Invalid(Invalid::Lacing))?;
  let duration = match block_duration.and_then(|whole| whole.checked_div(count)) {
    Some(each) => each,
    None => match default_durations.get(&track) {
      Some(default) => *default,
      None => 0,
    },
  };
  let Ok(first) = absolute_timestamp(cluster, header.relative_timestamp);

  laced
    .into_iter()
    .scan(first, |at, frame| {
      let timestamp = *at;

      *at = at.saturating_add(duration);

      Some((timestamp, frame))
    })
    .map(|(timestamp, frame)| {
      let Ok(found) = bytes_of(payload, &frame);
      let data = found.ok_or(DecodeError::Invalid(Invalid::Lacing))?;

      Ok(Frame {
        track,
        timestamp,
        duration,
        data: Box::from(data),
      })
    })
    .collect()
}

pub(crate) fn bytes_of<'a>(data: &'a [u8], frame: &Laced) -> Result<Option<&'a [u8]>, Never> {
  let Ok(start) = index(frame.offset);
  let Ok(len) = index(frame.len);

  Ok(start.checked_add(len).and_then(|end| data.get(start..end)))
}
