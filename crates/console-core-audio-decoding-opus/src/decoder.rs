//! One elementary stream: mode switching, hybrid summation, redundancy and transitions
//! (RFC 6716 section 4 and section 4.5).
//!
//! A frame is either read or concealed, and the two are separate paths rather than one path
//! that asks at every step whether there was data. Concealment is also what a transition
//! between modes cross-fades from, so the path that reads a frame calls the one that conceals
//! one, and the concealing path calls nothing back: a frame is never more than one call deep.
//! A loss longer than 20 ms is concealed 20 ms at a time, each piece shortened to the nearest
//! length the layer that was playing can conceal.
//!
//! The stream is always decoded at 48 kHz, so every frame length here is a count of samples at
//! that rate and the window the fades use is read one sample at a time.

use std::cmp::Ordering;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::Error;
use crate::celt::{CeltDecoder, Mixing};
use crate::packet::{Bandwidth, Channels, Mode, Packet, stretch};
use crate::range::{Bit, RangeDecoder};
use crate::silk::{SilkDecoder, Start};

const F20: u32 = 960;
const F10: u32 = 480;
const F5: u32 = 240;
const F2_5: u32 = 120;
const LONGEST: u32 = 5760;
const HYBRID_START_BAND: u32 = 17;
const GAIN_STEP: f32 = 6.488_141e-4;
const SILENCE: [u8; 2] = [0xff, 0xff];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Redundancy {
  Absent,
  CeltToSilk,
  SilkToCelt,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Crossing {
  Same,
  Different,
}

struct Redundant {
  kind: Redundancy,
  bytes: u32,
  length: u32,
}

struct Crossfade<'a> {
  from: &'a [f32],
  to: &'a [f32],
}

pub(crate) struct StreamDecoder {
  channels: Channels,
  celt: CeltDecoder,
  silk: SilkDecoder,
  stream_channels: Channels,
  bandwidth: Bandwidth,
  previous: Option<Mode>,
  previous_redundancy: Redundancy,
  frame_size: u32,
  last_packet_duration: u32,
  gain: Option<f32>,
}

fn start_band(mode: Mode) -> Result<u32, Never> {
  Ok(match mode {
    Mode::CeltOnly => 0,
    Mode::SilkOnly | Mode::Hybrid => HYBRID_START_BAND,
  })
}

fn tail(values: &mut [f32], from: u32) -> Result<&mut [f32], Never> {
  let Ok(at) = index(from);

  Ok(match values.get_mut(at..) {
    Some(rest) => rest,
    None => &mut [],
  })
}

fn after(values: &[f32], from: u32) -> Result<&[f32], Never> {
  let Ok(at) = index(from);

  Ok(match values.get(at..) {
    Some(rest) => rest,
    None => &[],
  })
}

fn front(values: &[f32], length: u32) -> Result<Vec<f32>, Never> {
  let Ok(taken) = index(length);

  Ok(values.iter().take(taken).copied().collect())
}

fn copied(into: &mut [f32], from: &[f32], length: u32) -> Result<(), Never> {
  let Ok(taken) = index(length);

  for (slot, value) in into.iter_mut().zip(from.iter()).take(taken) {
    *slot = *value;
  }

  Ok(())
}

fn smooth_fade(
  fade: Crossfade<'_>,
  out: &mut [f32],
  width: u32,
  window: &[f32; 120],
) -> Result<(), Never> {
  let Ok(lanes) = index(width.max(1));
  let Ok(overlap) = index(F2_5);

  for (((frame, from), to), weight) in out
    .chunks_exact_mut(lanes.max(1))
    .zip(fade.from.chunks_exact(lanes.max(1)))
    .zip(fade.to.chunks_exact(lanes.max(1)))
    .zip(window.iter())
    .take(overlap)
  {
    let square = weight * weight;

    for ((slot, earlier), later) in frame.iter_mut().zip(from.iter()).zip(to.iter()) {
      *slot = square * later + (1.0 - square) * earlier;
    }
  }

  Ok(())
}

fn redundancy(dec: &mut RangeDecoder<'_>, mode: Mode, length: u32) -> Result<Redundant, Never> {
  let none = Redundant {
    kind: Redundancy::Absent,
    bytes: 0,
    length,
  };
  let hybrid = match mode {
    Mode::Hybrid => 20,
    Mode::SilkOnly | Mode::CeltOnly => 0,
  };
  let Ok(told) = dec.tell();
  let needed = told.wrapping_add(17).wrapping_add(hybrid);
  let room = needed.cmp(&length.wrapping_mul(8).cast_signed());

  let present = match (mode, room) {
    (Mode::CeltOnly, _) | (_, Ordering::Greater) => Bit::Zero,
    (Mode::Hybrid, Ordering::Less | Ordering::Equal) => {
      let Ok(bit) = dec.bit_logp(12);

      bit
    }
    (Mode::SilkOnly, Ordering::Less | Ordering::Equal) => Bit::One,
  };

  match present {
    Bit::Zero => Ok(none),
    Bit::One => {
      let Ok(to_silk) = dec.bit_logp(1);
      let bytes = match mode {
        Mode::Hybrid => {
          let Ok(coded) = dec.uint(256);

          coded.wrapping_add(2)
        }
        Mode::SilkOnly | Mode::CeltOnly => {
          let Ok(told_2) = dec.tell();

          length.wrapping_sub(told_2.cast_unsigned().wrapping_add(7).wrapping_shr(3))
        }
      };
      let Ok(told_3) = dec.tell();
      let kind = match to_silk {
        Bit::One => Redundancy::CeltToSilk,
        Bit::Zero => Redundancy::SilkToCelt,
      };
      let payload = length.wrapping_sub(bytes);
      let found = match (
        bytes.cmp(&length),
        payload.wrapping_mul(8).cmp(&told_3.cast_unsigned()),
      ) {
        (Ordering::Greater, _) | (_, Ordering::Less) => Redundant {
          kind: Redundancy::Absent,
          bytes: 0,
          length: 0,
        },
        (Ordering::Less | Ordering::Equal, Ordering::Equal | Ordering::Greater) => Redundant {
          kind,
          bytes,
          length: payload,
        },
      };
      let Ok(()) = dec.shrink_storage(found.bytes);

      Ok(found)
    }
  }
}

impl StreamDecoder {
  pub(crate) fn new(channels: Channels, output_gain: i16) -> Result<StreamDecoder, Never> {
    let Ok(celt) = CeltDecoder::new(channels);
    let Ok(silk) = SilkDecoder::new(channels);
    let gain = match output_gain {
      0 => None,
      _gained => Some((GAIN_STEP * f32::from(output_gain)).exp2()),
    };

    Ok(StreamDecoder {
      channels,
      celt,
      silk,
      stream_channels: channels,
      bandwidth: Bandwidth::Fullband,
      previous: None,
      previous_redundancy: Redundancy::Absent,
      frame_size: F2_5,
      last_packet_duration: 0,
      gain,
    })
  }

  pub(crate) fn last_packet_duration(&self) -> Result<u32, Never> {
    Ok(self.last_packet_duration)
  }

  pub(crate) fn conceal(&mut self, duration: u32, lane: &mut Vec<f32>) -> Result<u32, Error> {
    let Ok(width) = self.channels.count();
    let Ok(start) = fitted::<_, u32>(lane.len());
    let Ok(additional) = index(duration.wrapping_mul(width));
    let mut done = 0u32;

    lane.resize(lane.len().saturating_add(additional.min(11_520)), 0.0);

    for _piece in 0..duration {
      match done.cmp(&duration) {
        Ordering::Less => {}
        Ordering::Equal | Ordering::Greater => break,
      }

      let Ok(rest) = tail(lane, start.wrapping_add(done.wrapping_mul(width)));
      let made = self.concealed(rest, duration.wrapping_sub(done))?;

      done = done.wrapping_add(made);
    }

    self.last_packet_duration = duration;

    Ok(duration)
  }

  pub(crate) fn decode(&mut self, data: &[u8], lane: &mut Vec<f32>) -> Result<u32, Error> {
    let packet = Packet::parse(data)?;
    let Ok(size) = packet.toc.frame_samples();
    let Ok(width) = self.channels.count();
    let Ok(frames) = fitted::<_, u32>(packet.frames.len());
    let Ok(start) = fitted::<_, u32>(lane.len());
    let Ok(additional) = index(size.wrapping_mul(frames).wrapping_mul(width));
    let mut made = 0u32;

    self.bandwidth = packet.toc.bandwidth;
    self.frame_size = size;
    self.stream_channels = packet.toc.layout;
    lane.resize(lane.len().saturating_add(additional.min(11_520)), 0.0);

    for frame in &packet.frames {
      let Ok(rest) = tail(lane, start.wrapping_add(made.wrapping_mul(width)));
      let decoded = match frame.len().cmp(&1) {
        Ordering::Greater => self.coded(frame, packet.toc.mode, rest),
        Ordering::Less | Ordering::Equal => self.concealed(rest, size),
      };
      let produced = decoded?;

      made = made.wrapping_add(produced);
    }

    let Ok(kept) = index(start.wrapping_add(made.wrapping_mul(width)));

    lane.truncate(kept);
    self.last_packet_duration = made;

    Ok(made)
  }

  fn concealed(&mut self, pcm: &mut [f32], asked: u32) -> Result<u32, Error> {
    let size = asked.min(LONGEST).min(self.frame_size);
    let Ok(width) = self.channels.count();

    let mode = match self.previous {
      None => {
        let Ok(length) = index(size.wrapping_mul(width));

        for value in pcm.iter_mut().take(length) {
          *value = 0.0;
        }

        return Ok(size);
      }
      Some(mode) => mode,
    };

    match size.cmp(&F20) {
      Ordering::Less | Ordering::Equal => return self.lost_piece(mode, pcm, size),
      Ordering::Greater => {}
    }

    let mut done = 0u32;

    for _piece in 0..size {
      match done.cmp(&size) {
        Ordering::Less => {}
        Ordering::Equal | Ordering::Greater => break,
      }

      let Ok(rest) = tail(pcm, done.wrapping_mul(width));
      let made = self.lost_piece(mode, rest, size.wrapping_sub(done).min(F20))?;

      done = done.wrapping_add(made);
    }

    Ok(size)
  }

  fn lost_piece(&mut self, mode: Mode, pcm: &mut [f32], asked: u32) -> Result<u32, Error> {
    let size = match (asked.cmp(&F20), asked.cmp(&F10), asked.cmp(&F5)) {
      (Ordering::Less, Ordering::Greater, _) => F10,
      (Ordering::Less, Ordering::Less, Ordering::Greater) => match mode {
        Mode::SilkOnly => asked,
        Mode::Hybrid | Mode::CeltOnly => F5,
      },
      _unchanged => asked,
    };

    self.lost_frame(mode, pcm, size)
  }

  fn lost_frame(&mut self, mode: Mode, pcm: &mut [f32], size: u32) -> Result<u32, Error> {
    let Ok(width) = self.channels.count();
    let Ok(silk) = self.silk_layer(None, mode, size);
    let Ok(stream) = self.stream_channels.count();
    let Ok(start) = start_band(mode);

    self.celt.stream_channels = stream;
    self.celt.start = start;

    self.celt_layer(None, None, mode, pcm, size, Redundancy::Absent)?;

    let Ok(()) = add(pcm, &silk, size.wrapping_mul(width));
    let Ok(()) = self.amplify(pcm, size);

    self.previous = Some(mode);
    self.previous_redundancy = Redundancy::Absent;

    Ok(size)
  }

  fn crossing(&self, mode: Mode) -> Result<Crossing, Never> {
    Ok(match (self.previous, mode, self.previous_redundancy) {
      (None, _, _)
      | (Some(Mode::CeltOnly), Mode::CeltOnly, _)
      | (Some(Mode::SilkOnly | Mode::Hybrid), Mode::SilkOnly | Mode::Hybrid, _) => Crossing::Same,
      (Some(Mode::SilkOnly | Mode::Hybrid), Mode::CeltOnly, Redundancy::SilkToCelt) => {
        Crossing::Same
      }
      (
        Some(Mode::SilkOnly | Mode::Hybrid),
        Mode::CeltOnly,
        Redundancy::Absent | Redundancy::CeltToSilk,
      ) => Crossing::Different,
      (Some(Mode::CeltOnly), Mode::SilkOnly | Mode::Hybrid, _) => Crossing::Different,
    })
  }

  fn silk_layer(
    &mut self,
    dec: Option<&mut RangeDecoder<'_>>,
    mode: Mode,
    size: u32,
  ) -> Result<Vec<f32>, Never> {
    match mode {
      Mode::CeltOnly => return Ok(Vec::new()),
      Mode::SilkOnly | Mode::Hybrid => {}
    }

    let Ok(width) = self.channels.count();
    let Ok(length) = index(size.max(F10).wrapping_mul(width));
    let mut silk = vec![0f32; length.min(11_520)];
    let mut dec_2 = dec;

    match self.previous {
      Some(Mode::CeltOnly) => {
        let Ok(()) = self.silk.reset();
      }
      Some(Mode::SilkOnly | Mode::Hybrid) | None => {}
    }

    match &dec_2 {
      Some(_dec) => {
        let rate = match (mode, self.bandwidth) {
          (Mode::SilkOnly, Bandwidth::Narrowband) => 8000,
          (Mode::SilkOnly, Bandwidth::Mediumband) => 12_000,
          (
            Mode::SilkOnly,
            Bandwidth::Wideband | Bandwidth::SuperWideband | Bandwidth::Fullband,
          )
          | (Mode::Hybrid | Mode::CeltOnly, _) => 16_000,
        };
        let Ok(()) = self.silk.set_stream(self.stream_channels, rate);
      }
      None => {}
    }

    let mut decoded = 0u32;

    for _call in 0..size {
      match decoded.cmp(&size) {
        Ordering::Less => {}
        Ordering::Equal | Ordering::Greater => break,
      }

      let start = match decoded {
        0 => Start::NewPacket,
        _later => Start::Continuing,
      };
      let Ok(rest) = tail(&mut silk, decoded.wrapping_mul(width));
      let Ok(made) = self.silk.decode(dec_2.as_deref_mut(), start, size, rest);

      match made {
        0 => break,
        _some => decoded = decoded.wrapping_add(made),
      }
    }

    Ok(silk)
  }

  fn celt_layer(
    &mut self,
    data: Option<&[u8]>,
    dec: Option<&mut RangeDecoder<'_>>,
    mode: Mode,
    pcm: &mut [f32],
    size: u32,
    kind: Redundancy,
  ) -> Result<(), Error> {
    match mode {
      Mode::SilkOnly => {
        let Ok(width) = self.channels.count();
        let Ok(length) = index(size.wrapping_mul(width));

        for value in pcm.iter_mut().take(length) {
          *value = 0.0;
        }

        match (self.previous, kind, self.previous_redundancy) {
          (Some(Mode::Hybrid), Redundancy::CeltToSilk, Redundancy::SilkToCelt) => {}
          (Some(Mode::Hybrid), Redundancy::Absent | Redundancy::SilkToCelt, _)
          | (
            Some(Mode::Hybrid),
            Redundancy::CeltToSilk,
            Redundancy::Absent | Redundancy::CeltToSilk,
          ) => {
            self.celt.start = 0;

            let _decoded = self
              .celt
              .decode(Some(&SILENCE), F2_5, pcm, Mixing::Add, None)?;
          }
          (Some(Mode::SilkOnly | Mode::CeltOnly) | None, _, _) => {}
        }
      }
      Mode::Hybrid | Mode::CeltOnly => {
        let changed = self.previous.filter(|previous| *previous != mode);

        match (changed, self.previous_redundancy) {
          (Some(_), Redundancy::Absent | Redundancy::CeltToSilk) => {
            let Ok(()) = self.celt.reset();
          }
          (Some(_), Redundancy::SilkToCelt) | (None, _) => {}
        }

        let _decoded = self
          .celt
          .decode(data, F20.min(size), pcm, Mixing::Replace, dec)?;
      }
    }

    Ok(())
  }

  fn amplify(&self, pcm: &mut [f32], size: u32) -> Result<(), Never> {
    let Ok(width) = self.channels.count();
    let Ok(length) = index(size.wrapping_mul(width));

    match self.gain {
      Some(gain) => {
        for value in pcm.iter_mut().take(length) {
          *value *= gain;
        }
      }
      None => {}
    }

    Ok(())
  }

  fn coded(&mut self, data: &[u8], mode: Mode, pcm: &mut [f32]) -> Result<u32, Error> {
    let size = self.frame_size;
    let Ok(width) = self.channels.count();
    let Ok(mut dec) = RangeDecoder::new(data);
    let Ok(length) = fitted::<_, u32>(data.len());
    let Ok(crossing) = self.crossing(mode);
    let Ok(faded_length) = index(F5.wrapping_mul(width));
    let mut faded = vec![0f32; faded_length.min(480)];

    match (crossing, mode) {
      (Crossing::Different, Mode::CeltOnly) => {
        let _made = self.concealed(&mut faded, F5.min(size))?;
      }
      (Crossing::Different, Mode::SilkOnly | Mode::Hybrid) | (Crossing::Same, _) => {}
    }

    let Ok(silk) = self.silk_layer(Some(&mut dec), mode, size);
    let Ok(redundant) = redundancy(&mut dec, mode, length);
    let crossing_2 = match redundant.kind {
      Redundancy::Absent => crossing,
      Redundancy::CeltToSilk | Redundancy::SilkToCelt => Crossing::Same,
    };

    match (crossing_2, mode) {
      (Crossing::Different, Mode::SilkOnly | Mode::Hybrid) => {
        let _made = self.concealed(&mut faded, F5.min(size))?;
      }
      (Crossing::Different, Mode::CeltOnly) | (Crossing::Same, _) => {}
    }

    let Ok(end) = self.bandwidth.celt_end_band();
    let Ok(stream) = self.stream_channels.count();
    let Ok(extra) = stretch(
      data,
      crate::quantities::StartOffset(redundant.length),
      crate::quantities::ByteLength(redundant.bytes),
    );
    let Ok(main) = stretch(
      data,
      crate::quantities::StartOffset(0),
      crate::quantities::ByteLength(redundant.length),
    );
    let Ok(start) = start_band(mode);
    let Ok(redundant_length) = index(match redundant.kind {
      Redundancy::Absent => 0,
      Redundancy::CeltToSilk | Redundancy::SilkToCelt => F5.wrapping_mul(width),
    });
    let mut audio = vec![0f32; redundant_length.min(480)];

    self.celt.end = end;
    self.celt.stream_channels = stream;

    match redundant.kind {
      Redundancy::CeltToSilk => {
        self.celt.start = 0;

        let _decoded = self
          .celt
          .decode(Some(extra), F5, &mut audio, Mixing::Replace, None)?;
      }
      Redundancy::Absent | Redundancy::SilkToCelt => {}
    }

    self.celt.start = start;
    self.celt_layer(Some(main), Some(&mut dec), mode, pcm, size, redundant.kind)?;

    let Ok(()) = add(pcm, &silk, size.wrapping_mul(width));

    self.blend(
      pcm,
      Blend {
        kind: redundant.kind,
        extra,
        audio: &mut audio,
        crossing: crossing_2,
        faded: &faded,
        size,
      },
    )?;

    let Ok(()) = self.amplify(pcm, size);

    self.previous = Some(mode);
    self.previous_redundancy = redundant.kind;

    Ok(size)
  }

  fn blend(&mut self, pcm: &mut [f32], blend: Blend<'_>) -> Result<(), Error> {
    let Ok(width) = self.channels.count();
    let step = F2_5.wrapping_mul(width);

    match blend.kind {
      Redundancy::SilkToCelt => {
        let Ok(()) = self.celt.reset();

        self.celt.start = 0;

        let _decoded =
          self
            .celt
            .decode(Some(blend.extra), F5, blend.audio, Mixing::Replace, None)?;
        let Ok(window) = self.celt.window();
        let Ok(rest) = tail(pcm, blend.size.wrapping_sub(F2_5).wrapping_mul(width));
        let Ok(ending) = front(rest, step);
        let Ok(later) = after(blend.audio, step);
        let Ok(()) = smooth_fade(
          Crossfade {
            from: &ending,
            to: later,
          },
          rest,
          width,
          window,
        );
      }
      Redundancy::CeltToSilk => {
        let Ok(window) = self.celt.window();
        let Ok(()) = copied(pcm, blend.audio, step);
        let Ok(rest) = tail(pcm, step);
        let Ok(current) = front(rest, step);
        let Ok(later) = after(blend.audio, step);
        let Ok(()) = smooth_fade(
          Crossfade {
            from: later,
            to: &current,
          },
          rest,
          width,
          window,
        );
      }
      Redundancy::Absent => {}
    }

    let Ok(window) = self.celt.window();

    match (blend.crossing, blend.size.cmp(&F5)) {
      (Crossing::Different, Ordering::Equal | Ordering::Greater) => {
        let Ok(()) = copied(pcm, blend.faded, step);
        let Ok(rest) = tail(pcm, step);
        let Ok(current) = front(rest, step);
        let Ok(later) = after(blend.faded, step);
        let Ok(()) = smooth_fade(
          Crossfade {
            from: later,
            to: &current,
          },
          rest,
          width,
          window,
        );
      }
      (Crossing::Different, Ordering::Less) => {
        let Ok(current) = front(pcm, step);
        let Ok(()) = smooth_fade(
          Crossfade {
            from: blend.faded,
            to: &current,
          },
          pcm,
          width,
          window,
        );
      }
      (Crossing::Same, _) => {}
    }

    Ok(())
  }
}

struct Blend<'a> {
  kind: Redundancy,
  extra: &'a [u8],
  audio: &'a mut [f32],
  crossing: Crossing,
  faded: &'a [f32],
  size: u32,
}

fn add(pcm: &mut [f32], silk: &[f32], length: u32) -> Result<(), Never> {
  let Ok(taken) = index(length);

  for (value, layer) in pcm.iter_mut().take(taken).zip(silk.iter()) {
    *value += *layer;
  }

  Ok(())
}
