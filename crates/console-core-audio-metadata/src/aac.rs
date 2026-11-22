//! The average ADTS frame bitrate determines the length of a raw AAC stream.

use std::cmp::Ordering;
use std::io::{Read, Seek};
use std::time::Duration;

use console_core_audio_decoding_advanced_audio_coding::adts_frame;

use crate::Contents;
use crate::bytes::Source;
use crate::fault::{Malformed, ParseError};

pub(crate) fn read<R: Read + Seek>(reader: &mut R) -> Result<Contents, ParseError> {
  let mut source = Source::new(reader)?;
  let id3 = crate::id3v2::leading(&mut source)?;
  let beginning = source.at()?;
  let Ok(length) = source.length();
  let id3v1 = crate::id3v1::trailer(&mut source, length)?;
  let end = match &id3v1 {
    Some((_tag, start)) => *start,
    None => length,
  };
  let mut offset = beginning;
  let mut rates = 0u64;
  let mut frames = 0u64;

  'frames: for _frame in 0..end {
    match offset.saturating_add(7).cmp(&end) {
      Ordering::Greater => break 'frames,
      Ordering::Less | Ordering::Equal => {}
    }

    source.seek_to(offset)?;

    let header = source.array::<7>()?;
    let parsed = adts_frame(header).map_err(|_invalid| Malformed::Frame)?;

    match parsed {
      Some(frame) => {
        let next = offset.saturating_add(u64::from(frame.bytes));

        match next.saturating_add(7).cmp(&end) {
          Ordering::Greater => break 'frames,
          Ordering::Less | Ordering::Equal => {}
        }

        source.seek_to(next)?;

        let next_header = source.array::<7>()?;
        let following = adts_frame(next_header).map_err(|_invalid| Malformed::Frame)?;

        match following.filter(|candidate| candidate.stream == frame.stream) {
          None => {
            offset = offset.saturating_add(1);

            continue 'frames;
          }
          Some(_same_stream) => {}
        }

        let bits_per_second = u64::from(frame.bytes)
          .saturating_mul(u64::from(frame.sample_rate))
          .wrapping_div(1024)
          .saturating_mul(8);

        rates = rates.saturating_add(bits_per_second.wrapping_div(1024));
        frames = frames.saturating_add(1);
        offset = next;
      }
      None => offset = offset.saturating_add(1),
    }
  }

  let rate = rates.checked_div(frames).ok_or(Malformed::NoAudio)?;
  let stripped = match &id3 {
    Some(_tag) => beginning.saturating_sub(10),
    None => 0,
  };
  let stream_length = end.saturating_sub(stripped);
  let milliseconds = stream_length
    .saturating_mul(8)
    .checked_div(rate)
    .ok_or(Malformed::Properties)?;
  let mut tags = Vec::new();

  match id3 {
    Some(id3_2) => {
      let Ok(tag) = id3_2.tag();

      tags.push(tag);
    }
    None => {}
  }

  tags.extend(id3v1.map(|(tag, _start)| tag));

  Ok(Contents {
    duration: Duration::from_millis(milliseconds),
    tags,
  })
}
