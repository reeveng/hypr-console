//! MPEG duration comes from a declared frame count, or the bytes between matching headers.

use std::cmp::Ordering;
use std::io::{Read, Seek};

use console_core_mpeg_audio_frames::{FrameTiming, frame_timing};

use crate::Contents;
use crate::bytes::Source;
use crate::fault::{Malformed, ParseError};

pub(crate) fn read<R: Read + Seek>(reader: &mut R) -> Result<Contents, ParseError> {
  let mut source = Source::new(reader)?;
  let mut id3 = crate::id3v2::leading(&mut source)?;
  let beginning = source.at()?;
  let Ok(length) = source.length();
  let id3v1 = crate::id3v1::trailer(&mut source, length)?;
  let audio_end = match &id3v1 {
    Some((_tag, start)) => *start,
    None => length,
  };
  let ape = crate::ape::trailer(&mut source, audio_end)?;
  let audio_end_2 = match &ape {
    Some((_tag, start)) => *start,
    None => audio_end,
  };
  let mut first = None;

  'headers: for offset in beginning..audio_end_2.saturating_sub(3) {
    source.seek_to(offset)?;

    let word = source.array::<4>()?;

    match (&id3, word) {
      (None, [b'I', b'D', b'3', _]) => {
        source.seek_to(offset)?;

        let value_1 = crate::id3v2::leading(&mut source)?;

        id3 = value_1;
      }
      (Some(_), _) | (None, _) => {}
    }

    let candidate = frame_timing(&word).map_err(|_invalid| Malformed::Frame)?;

    match candidate {
      Some(candidate_2) => {
        source.seek_to(offset)?;

        let frame = source.upto(u64::from(candidate_2.bytes))?;
        let timing = frame_timing(&frame).map_err(|_invalid| Malformed::Frame)?;

        first = timing.map(|timing_2| (offset, timing_2));

        break 'headers;
      }
      None => {}
    }
  }

  let (offset, first_2) = first.ok_or(Malformed::NoAudio)?;
  let duration = match first_2.declared_frames {
    Some(frames) => crate::duration::rounded(crate::duration::Interval {
      units: first_2.samples.saturating_mul(u64::from(frames)),
      per_second: u64::from(first_2.sample_rate),
    })
    .map_err(ParseError::from),
    None => estimated(&mut source, first_2, offset..audio_end_2),
  };
  let duration_2 = duration?;

  let mut tags = Vec::new();

  match id3 {
    Some(id3_2) => {
      let Ok(tag) = id3_2.tag();

      tags.push(tag);
    }
    None => {}
  }

  tags.extend(id3v1.map(|(tag, _start)| tag));
  tags.extend(ape.map(|(tag, _start)| tag));

  Ok(Contents {
    duration: duration_2,
    tags,
  })
}

fn estimated<R: Read + Seek>(
  source: &mut Source<'_, R>,
  first: FrameTiming,
  range: std::ops::Range<u64>,
) -> Result<std::time::Duration, ParseError> {
  let mut end = range.start.saturating_add(u64::from(first.bytes));

  'headers: for offset in (range.start..range.end.saturating_sub(3)).rev() {
    source.seek_to(offset)?;

    let word = source.array::<4>()?;
    let candidate = frame_timing(&word).map_err(|_invalid| Malformed::Frame)?;

    match candidate {
      Some(candidate_2) => match candidate_2.stream.cmp(&first.stream) {
        Ordering::Equal => {
          end = offset.saturating_add(u64::from(candidate_2.bytes));

          break 'headers;
        }
        Ordering::Less | Ordering::Greater => {}
      },
      None => {}
    }
  }

  let bytes = end.saturating_sub(range.start);
  let bits = bytes.saturating_mul(8);
  let rate = u64::from(first.bitrate).wrapping_div(1000);
  let milliseconds = bits
    .saturating_add(rate.wrapping_div(2))
    .checked_div(rate)
    .ok_or(Malformed::Properties)?;

  Ok(std::time::Duration::from_millis(milliseconds))
}
