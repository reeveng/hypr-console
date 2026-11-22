//! The Opus identification header (`OpusHead`, RFC 7845 section 5.1).
//!
//! The first packet of an Ogg Opus stream and the `CodecPrivate` of a Matroska
//! track say how many channels there are, how many samples to drop from the
//! front, what gain to apply, and how the coded streams map onto channels.
//! Family 0 is mono or stereo in a single stream, family 1 is surround in
//! Vorbis order, family 255 leaves the order unspecified, and family 3
//! (projection) is not decoded.

use std::cmp::Ordering;

use console_core_number_conversion::fitted;

use crate::Error;
use crate::packet::stretch;

const MAGIC: &[u8] = b"OpusHead";
const SILENCE: u8 = 255;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OpusHead {
  pub channels: u32,
  pub pre_skip: u16,
  pub output_gain: i16,
  pub streams: u32,
  pub coupled_streams: u32,
  pub mapping: Vec<u8>,
}

struct Fixed {
  version: u8,
  channels: u8,
  pre_skip: u16,
  output_gain: i16,
  family: u8,
}

fn fixed(data: &[u8]) -> Result<(Fixed, &[u8]), Error> {
  let payload = data.strip_prefix(MAGIC).ok_or(Error::MissingMagic)?;
  let (bytes, rest) = payload
    .split_first_chunk::<11>()
    .ok_or(Error::MissingMagic)?;
  let [
    version,
    channels,
    skip0,
    skip1,
    _r0,
    _r1,
    _r2,
    _r3,
    gain0,
    gain1,
    family,
  ] = *bytes;

  Ok((
    Fixed {
      version,
      channels,
      pre_skip: u16::from_le_bytes([skip0, skip1]),
      output_gain: i16::from_le_bytes([gain0, gain1]),
      family,
    },
    rest,
  ))
}

fn single_stream(channels: u32) -> Result<(u32, u32, Vec<u8>), Error> {
  match channels.cmp(&2) {
    Ordering::Greater => Err(Error::TooManyChannelsForSingleStream),
    Ordering::Less | Ordering::Equal => Ok((
      1,
      channels.wrapping_sub(1),
      (0..channels)
        .map(|channel| {
          let Ok(place) = fitted::<u32, u8>(channel);

          place
        })
        .collect(),
    )),
  }
}

fn mapped(family: u8, channels: u32, rest: &[u8]) -> Result<(u32, u32, Vec<u8>), Error> {
  let Ok(length) = fitted::<_, u32>(rest.len());

  match (
    rest.split_first_chunk::<2>(),
    length.cmp(&channels.wrapping_add(2)),
  ) {
    (Some((counts, table)), Ordering::Equal | Ordering::Greater) => {
      counted(family, channels, counts.map(u32::from), table)
    }
    (Some(_), Ordering::Less) | (None, _) => Err(Error::TruncatedMapping),
  }
}

fn counted(
  family: u8,
  channels: u32,
  counts: [u32; 2],
  table: &[u8],
) -> Result<(u32, u32, Vec<u8>), Error> {
  let [streams, coupled] = counts;
  let coded_channels = streams.wrapping_add(coupled);

  match (streams, coupled.cmp(&streams), coded_channels) {
    (0, _, _) | (_, Ordering::Greater, _) | (_, _, 256..) => return Err(Error::BadStreamCounts),
    _valid => {}
  }

  match (family, channels) {
    (1, 0 | 9..) => return Err(Error::SurroundChannels),
    _valid => {}
  }

  let Ok(mapping) = stretch(
    table,
    crate::quantities::StartOffset(0),
    crate::quantities::ByteLength(channels),
  );

  match mapping
    .iter()
    .find(|&&place| place != SILENCE && u32::from(place) >= coded_channels)
  {
    Some(_out_of_range) => Err(Error::MappingOutOfRange),
    None => Ok((streams, coupled, mapping.to_vec())),
  }
}

impl OpusHead {
  pub(crate) fn parse(data: &[u8]) -> Result<OpusHead, Error> {
    let (fixed, rest) = fixed(data)?;
    let channels = u32::from(fixed.channels);

    match (fixed.version, channels) {
      (16.., _) => return Err(Error::MajorVersion),
      (_, 0) => return Err(Error::ZeroChannels),
      _valid => {}
    }

    let (streams, coupled_streams, mapping) = match fixed.family {
      0 => single_stream(channels)?,
      3 => return Err(Error::ProjectionFamily),
      family => mapped(family, channels, rest)?,
    };

    Ok(OpusHead {
      channels,
      pre_skip: fixed.pre_skip,
      output_gain: fixed.output_gain,
      streams,
      coupled_streams,
      mapping,
    })
  }
}
