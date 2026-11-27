//! The decoder a file's `OpusHead` makes: mono, stereo or surround at 48 kHz.
//!
//! An `OpusHead` packet says how many elementary streams there are, which of
//! them are coupled stereo pairs, and which decoded channel each output channel
//! takes. Each packet of the file is then one packet per stream laid end to
//! end, every stream but the last self-delimited (RFC 6716 appendix B), and the
//! decoder answers planar samples, one list per output channel. The header's
//! output gain is applied. The pre-skip is not removed unless
//! [`Decoder::trim_pre_skip`] was asked, because a container usually says it
//! once for the whole track rather than for the stream.

use std::cmp::Ordering;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::Error;
use crate::decoder::StreamDecoder;
use crate::header::OpusHead;
use crate::packet::{Channels, Packet, stretch};

const SILENCE: u8 = 255;
const CONCEALED_DURATION: u32 = 960;

pub struct Decoder {
  head: OpusHead,
  streams: Vec<StreamDecoder>,
  skip_remaining: u32,
  scratch: Vec<Vec<f32>>,
}

fn byte(value: u32) -> Result<u8, Error> {
  match u8::try_from(value) {
    Ok(byte) => Ok(byte),
    Err(_unrepresentable) => Err(Error::MalformedFraming),
  }
}

fn reframe(packet: &Packet<'_>) -> Result<Vec<u8>, Error> {
  let stereo = match packet.toc.layout {
    Channels::Stereo => 4,
    Channels::Mono => 0,
  };
  let Ok(held) = fitted::<_, u32>(packet.frames.len());
  let Ok(apart) = index(held.saturating_sub(1));
  let toc = byte(u32::from(packet.toc.config).wrapping_shl(3) | stereo | 3)?;
  let count = byte(0x80 | held)?;
  let mut regular = vec![toc, count];

  for frame in packet.frames.iter().take(apart) {
    let Ok(size) = fitted::<_, u32>(frame.len());

    match size.cmp(&252) {
      Ordering::Less => {
        let only = byte(size)?;

        regular.push(only);
      }
      Ordering::Equal | Ordering::Greater => {
        let first = (size & 3).wrapping_add(252);
        let low = byte(first)?;
        let high = byte(size.wrapping_sub(first).wrapping_shr(2))?;

        regular.push(low);
        regular.push(high);
      }
    }
  }

  for frame in &packet.frames {
    regular.extend_from_slice(frame);
  }

  Ok(regular)
}

#[derive(Clone, Copy)]
enum Side {
  Left,
  Right,
}

#[derive(Clone, Copy)]
enum Source {
  Silent,
  Alone { stream: u32 },
  Coupled { stream: u32, side: Side },
}

fn source(wanted: u8, coupled: u32) -> Result<Source, Never> {
  let place = u32::from(wanted);

  Ok(
    match (wanted.cmp(&SILENCE), place.cmp(&coupled.wrapping_mul(2))) {
      (Ordering::Equal, _) => Source::Silent,
      (Ordering::Less | Ordering::Greater, Ordering::Less) => Source::Coupled {
        stream: place.wrapping_shr(1),
        side: match place & 1 {
          0 => Side::Left,
          _right => Side::Right,
        },
      },
      (Ordering::Less | Ordering::Greater, Ordering::Equal | Ordering::Greater) => {
        Source::Alone {
          stream: place.wrapping_sub(coupled),
        }
      }
    },
  )
}

fn copied<'a>(channel: &mut [f32], heard: impl Iterator<Item = &'a f32>) -> Result<(), Never> {
  for (slot, sample) in channel.iter_mut().zip(heard) {
    *slot = *sample;
  }

  Ok(())
}

impl Decoder {
  pub fn new(opus_head: &[u8]) -> Result<Decoder, Error> {
    let head = OpusHead::parse(opus_head)?;
    let Ok(streams) = Self::streams(&head);
    let scratch = streams.iter().map(|_stream| Vec::new()).collect();

    Ok(Decoder {
      head,
      streams,
      skip_remaining: 0,
      scratch,
    })
  }

  fn streams(head: &OpusHead) -> Result<Vec<StreamDecoder>, Never> {
    let mut streams = Vec::new();

    for number in 0..head.streams {
      let width = match number.cmp(&head.coupled_streams) {
        Ordering::Less => Channels::Stereo,
        Ordering::Equal | Ordering::Greater => Channels::Mono,
      };
      let Ok(stream) = StreamDecoder::new(width, head.output_gain);

      streams.push(stream);
    }

    Ok(streams)
  }

  pub fn channel_count(&self) -> Result<u32, Never> {
    Ok(self.head.channels)
  }

  pub fn reset(&mut self) -> Result<(), Never> {
    let Ok(streams) = Self::streams(&self.head);
    self.streams = streams;
    self.skip_remaining = 0;

    for lane in &mut self.scratch {
      lane.clear();
    }

    Ok(())
  }

  pub fn trim_pre_skip(&mut self) -> Result<(), Never> {
    self.skip_remaining = u32::from(self.head.pre_skip);

    Ok(())
  }

  pub fn decode(&mut self, packet: Option<&[u8]>) -> Result<Vec<Vec<f32>>, Error> {
    for lane in &mut self.scratch {
      lane.clear();
    }

    let samples = match packet.filter(|data| !data.is_empty()) {
      Some(data) => self.decode_streams(data)?,
      None => self.conceal()?,
    };
    let Ok(planar) = self.planar(samples);

    Ok(planar)
  }

  fn decode_streams(&mut self, data: &[u8]) -> Result<u32, Error> {
    let Ok(count) = fitted::<_, u32>(self.streams.len());
    let Ok(length) = fitted::<_, u32>(data.len());
    let mut heard: Option<u32> = None;
    let mut offset = 0u32;

    for (number, (stream, lane)) in
      (0u32..).zip(self.streams.iter_mut().zip(self.scratch.iter_mut()))
    {
      let Ok(rest) = stretch(
        data,
        crate::quantities::StartOffset(offset),
        crate::quantities::ByteLength(length.wrapping_sub(offset)),
      );
      let Ok(left) = fitted::<_, u32>(rest.len());

      let (used, regular) = match (rest, number.wrapping_add(1).cmp(&count)) {
        ([], _) => return Err(Error::TruncatedMultistream),
        ([_, ..], Ordering::Equal) => (left, None),
        ([_, ..], Ordering::Less | Ordering::Greater) => {
          let (parsed, used) = Packet::parse_self_delimited(rest)?;
          let own = reframe(&parsed)?;

          (used, Some(own))
        }
      };
      let Ok(whole) = stretch(
        rest,
        crate::quantities::StartOffset(0),
        crate::quantities::ByteLength(used),
      );
      let part = match &regular {
        Some(own) => own.as_slice(),
        None => whole,
      };
      let made = stream.decode(part, lane)?;

      match heard {
        Some(earlier) => match earlier.cmp(&made) {
          Ordering::Less | Ordering::Greater => return Err(Error::DurationsDiffer),
          Ordering::Equal => {}
        },
        None => heard = Some(made),
      }

      offset = offset.wrapping_add(used);
    }

    Ok(match heard {
      Some(samples) => samples,
      None => 0,
    })
  }

  fn conceal(&mut self) -> Result<u32, Error> {
    let remembered = match self.streams.first() {
      Some(stream) => {
        let Ok(duration) = stream.last_packet_duration();

        duration
      }
      None => 0,
    };
    let duration = match remembered {
      0 => CONCEALED_DURATION,
      _remembered => remembered,
    };

    for (stream, lane) in self.streams.iter_mut().zip(self.scratch.iter_mut()) {
      let _made = stream.conceal(duration, lane)?;
    }

    Ok(duration)
  }

  fn planar(&mut self, samples: u32) -> Result<Vec<Vec<f32>>, Never> {
    let skipped = self.skip_remaining.min(samples);
    let Ok(length) = index(samples.wrapping_sub(skipped));
    let Ok(dropped) = index(skipped);
    let mut planar = Vec::new();

    for wanted in &self.head.mapping {
      let Ok(from) = source(*wanted, self.head.coupled_streams);
      let mut channel = vec![0f32; length.min(5760)];

      let (stream, side) = match from {
        Source::Silent => (None, None),
        Source::Alone { stream } => (Some(stream), None),
        Source::Coupled { stream, side } => (Some(stream), Some(side)),
      };
      let lane = match stream {
        Some(stream_2) => {
          let Ok(slot) = index(stream_2);

          self.scratch.get(slot)
        }
        None => None,
      };

      let Ok(()) = match (lane, side) {
        (None, _) => Ok(()),
        (Some(lane_2), None) => copied(&mut channel, lane_2.iter().skip(dropped)),
        (Some(lane_3), Some(Side::Left)) => copied(
          &mut channel,
          lane_3
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[left, _right]| left)
            .skip(dropped),
        ),
        (Some(lane_4), Some(Side::Right)) => copied(
          &mut channel,
          lane_4
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[_left, right]| right)
            .skip(dropped),
        ),
      };

      planar.push(channel);
    }

    self.skip_remaining = self.skip_remaining.wrapping_sub(skipped);

    Ok(planar)
  }
}
