//! Decoding one layer three frame into samples.
//!
//! A frame's side information says where its main data is, and the main data
//! may begin in an earlier frame: an encoder that did not need every byte of a
//! frame leaves them for a later one, so the bytes of each frame go into a
//! reservoir and the frame reads from `main_data_begin` bytes back. A stream
//! that is joined in the middle asks for bytes that were never held, and the
//! granules those bytes belonged to are silent rather than guessed at.
//!
//! Each granule is then requantized, joined back into left and right,
//! reordered, alias reduced, transformed and run through the polyphase
//! filterbank, which is [`crate::synthesis`].

use console_core_audio_decoding_interfaces::audio::{AudioBuffer, Signal};
use console_core_audio_decoding_interfaces::errors::{Error, Result, decode_error};
use console_core_byte_streams::{BitReaderLtr, BufReader, ReadBitsLtr, ReadBytes};
use console_core_number_conversion::{fitted, index};

use crate::common::{
  Bands, ChannelMode, FrameHeader, Infallible, Protection, Switch, Version, bands,
};
use crate::synthesis::{self, SynthesisState};

mod bitstream;
mod codebooks;
mod computed;
mod frame;
mod hybrid_synthesis;
mod imdct;
mod requantize;
mod stereo;
pub mod tables;

use codebooks::Codebooks;
use computed::Computed;
use frame::{FrameData, Granule, GranuleChannel, ScaleFactorGroup};

const EMPTY_RESERVOIR: [u8; 2048] = [0; 2048];

const GRANULE_SAMPLES: u32 = 576;

struct BitReservoir {
  buf: Vec<u8>,
  len: u32,
  consumed: u32,
}

impl BitReservoir {
  fn new() -> Infallible<Self> {
    Ok(BitReservoir {
      buf: EMPTY_RESERVOIR.to_vec(),
      len: 0,
      consumed: 0,
    })
  }

  fn keep(&mut self, kept: u32, data: &[u8]) -> Infallible<()> {
    let Ok(to) = index(self.len);
    let Ok(count) = index(kept);

    self.buf.copy_within(to.saturating_sub(count)..to, 0);

    for (slot, byte) in self.buf.iter_mut().skip(count).zip(data) {
      *slot = *byte;
    }

    Ok(())
  }

  fn fill(&mut self, data: &[u8], main_data_begin: u32) -> Result<u32> {
    let Ok(data_len) = fitted::<_, u32>(data.len());
    let Ok(capacity) = fitted::<_, u32>(self.buf.len());
    let main_data_end = main_data_begin.saturating_add(data_len);

    match ::core::primitive::bool::then_some(main_data_end > capacity, ()) {
      ::core::option::Option::Some(()) => return decode_error("mpa: invalid main_data length, will exceed resevoir buffer"),
      ::core::option::Option::None => {}
    }

    let unread = self.len.saturating_sub(self.consumed);

    let (kept, underflow) = match ::core::primitive::bool::then_some(main_data_begin <= unread, ()) {
      ::core::option::Option::Some(()) => (main_data_begin, 0),
      ::core::option::Option::None => (unread, main_data_begin.saturating_sub(unread)),
    };

    let Ok(()) = self.keep(kept, data);

    self.len = kept.saturating_add(data_len);
    self.consumed = 0;

    Ok(underflow)
  }

  fn consume(&mut self, len: u32) -> Infallible<()> {
    self.consumed = self.len.min(self.consumed.saturating_add(len));

    Ok(())
  }

  fn unread(&self) -> Infallible<&[u8]> {
    let Ok(from) = index(self.consumed);
    let Ok(to) = index(self.len);

    Ok(match self.buf.get(from..to) {
      Some(unread) => unread,
      None => &[],
    })
  }

  fn clear(&mut self) -> Infallible<()> {
    self.len = 0;
    self.consumed = 0;

    Ok(())
  }
}

type Spectrum = [f32; 576];

struct Position {
  begin: u32,
  skipped: u32,
  shared: [[ScaleFactorGroup; 4]; 2],
  previous: [Option<[u8; 39]>; 2],
}

struct Sources<'a> {
  main_data: &'a [u8],
  codebooks: &'a Codebooks,
  pow43: &'a [f32],
}

struct Lane<'a> {
  channel: &'a mut GranuleChannel,
  spectrum: &'a mut Spectrum,
  shared: [ScaleFactorGroup; 4],
  previous: Option<[u8; 39]>,
  intensity: Switch,
}

pub struct Layer3 {
  samples: [[Spectrum; 2]; 2],
  overlap: [[[f32; 18]; 32]; 2],
  synthesis: [SynthesisState; 2],
  reservoir: BitReservoir,
  codebooks: Codebooks,
  computed: Computed,
}

fn intensity_coded(header: &FrameHeader) -> Infallible<Switch> {
  Ok(match header.channel_mode {
    ChannelMode::JointStereo { intensity, .. } => intensity,
    ChannelMode::Mono | ChannelMode::DualMono | ChannelMode::Stereo => Switch::Off,
  })
}

fn read_channel(
  sources: &Sources<'_>,
  header: &FrameHeader,
  begin: &mut u32,
  lane: Lane<'_>,
) -> Result<()> {
  let Lane {
    channel,
    spectrum,
    shared,
    previous,
    intensity,
  } = lane;
  let Ok(byte_index) = index(begin.wrapping_shr(3));

  let rest = match sources
    .main_data
    .get(byte_index..)
    .filter(|rest| !rest.is_empty())
  {
    Some(rest) => rest,
    None => return decode_error("mpa: invalid main_data offset"),
  };

  let Ok(mut bs) = BitReaderLtr::new(rest);

  bs.ignore_bits(*begin & 7)?;

  let scale_factors = match header.version {
    Version::Mpeg1 => {
      bitstream::read_scale_factors_mpeg1(&mut bs, channel, shared, previous.as_ref())
    }
    Version::Mpeg2 | Version::Mpeg2p5 => {
      bitstream::read_scale_factors_mpeg2(&mut bs, channel, intensity)
    }
  };

  let part2_len = scale_factors?;
  let part2_3_length = u32::from(channel.part2_3_length);

  match ::core::primitive::bool::then_some(part2_len > part2_3_length, ()) {
    ::core::option::Option::Some(()) => return decode_error("mpa: part2_3_length is not valid"),
    ::core::option::Option::None => {}
  }

  let part3_len = part2_3_length.saturating_sub(part2_len);

  let huffman = requantize::read_huffman_samples(
    &mut bs,
    channel,
    part3_len,
    sources.codebooks,
    sources.pow43,
    spectrum,
  );

  let rzero = match huffman {
    Ok(rzero) => Ok(rzero),
    Err(fault) => huffman_overrun(fault),
  };

  let rzero_2 = rzero?;

  channel.rzero = rzero_2;

  *begin = begin.saturating_add(part2_3_length);

  Ok(())
}

fn huffman_overrun(fault: Error) -> Result<u32> {
  match fault {
    Error::IoError(io) => match ::core::primitive::bool::then_some(io.kind() == std::io::ErrorKind::Other, ()) {
      ::core::option::Option::Some(()) => decode_error("mpa: huffman decode overrun"),
      ::core::option::Option::None => Err(Error::IoError(io)),
    },
    Error::DecodeError(_)
    | Error::SeekError(_)
    | Error::Unsupported(_)
    | Error::LimitError(_)
    | Error::ResetRequired => Err(fault),
  }
}

impl Layer3 {
  pub fn new() -> Result<Self> {
    let Ok(reservoir) = BitReservoir::new();
    let Ok(computed) = Computed::new();
    let codebooks = Codebooks::build()?;

    Ok(Layer3 {
      samples: [[[0f32; 576]; 2]; 2],
      overlap: [[[0f32; 18]; 32]; 2],
      synthesis: Default::default(),
      reservoir,
      codebooks,
      computed,
    })
  }

  pub fn reset(&mut self) -> Infallible<()> {
    self.samples = [[[0f32; 576]; 2]; 2];
    self.overlap = [[[0f32; 18]; 32]; 2];
    self.synthesis = Default::default();

    self.reservoir.clear()
  }

  fn read_main_data(
    &mut self,
    header: &FrameHeader,
    underflow_bits: u32,
    frame: &mut FrameData,
  ) -> Result<u32> {
    let Ok(main_data) = self.reservoir.unread();
    let Ok(pow43) = self.computed.pow43();
    let sources = Sources {
      main_data,
      codebooks: &self.codebooks,
      pow43,
    };
    let Ok(granules) = index(header.granules);
    let Ok(channels) = index(header.channels);
    let mut position = Position {
      begin: 0,
      skipped: 0,
      shared: frame.scfsi,
      previous: [None, None],
    };

    for (granule, samples) in frame
      .granules
      .iter_mut()
      .zip(self.samples.iter_mut())
      .take(granules)
    {
      match ::core::primitive::bool::then_some(position.skipped < underflow_bits, ()) {
        ::core::option::Option::Some(()) => {
          for (channel, spectrum) in granule
            .channels
            .iter()
            .zip(samples.iter_mut())
            .take(channels)
          {
            let Ok(()) = requantize::zero(spectrum);

            position.skipped = position
              .skipped
              .saturating_add(u32::from(channel.part2_3_length));
          }

          match ::core::primitive::bool::then_some(position.skipped > underflow_bits, ()) {
            ::core::option::Option::Some(()) => position.begin = position.skipped.saturating_sub(underflow_bits),
            ::core::option::Option::None => {}
          }
        }
        ::core::option::Option::None => {
          let Ok(intensity) = intensity_coded(header);
          let lanes = granule
            .channels
            .iter_mut()
            .zip(samples.iter_mut())
            .zip(position.shared)
            .zip(position.previous);

          for (at, (((channel, spectrum), shared), previous)) in
            (0u32..).zip(lanes).take(channels)
          {
            let intensity_2 = match ::core::primitive::bool::then_some(at > 0, ()) {
              ::core::option::Option::Some(()) => intensity,
              ::core::option::Option::None => Switch::Off,
            };

            read_channel(
              &sources,
              header,
              &mut position.begin,
              Lane {
                channel,
                spectrum,
                shared,
                previous,
                intensity: intensity_2,
              },
            )?;
          }
        }
      }

      position.previous = granule
        .channels
        .each_ref()
        .map(|channel| Some(channel.scalefacs));
    }

    Ok(position.begin.saturating_add(7).wrapping_shr(3))
  }

  fn render(
    &mut self,
    header: &FrameHeader,
    scale_bands: &Bands,
    frame: &mut FrameData,
    out: &mut AudioBuffer<f32>,
  ) -> Result<()> {
    let Layer3 {
      samples,
      overlap,
      synthesis,
      computed,
      ..
    } = self;
    let Ok(granules) = index(header.granules);
    let mut filters = Filters {
      overlap,
      synthesis,
      computed,
    };

    for (at, (granule, samples_2)) in (0u32..)
      .zip(frame.granules.iter_mut().zip(samples.iter_mut()))
      .take(granules)
    {
      render_granule(
        header,
        scale_bands,
        &mut filters,
        granule,
        samples_2,
        Placing { at, out },
      )?;
    }

    Ok(())
  }

  pub fn decode(
    &mut self,
    reader: &mut BufReader<'_>,
    header: &FrameHeader,
    out: &mut AudioBuffer<f32>,
  ) -> Result<()> {
    let crc = match header.protection {
      Protection::Crc => reader.read_be_u16(),
      Protection::None => Ok(0),
    };

    crc?;

    let Ok(buf) = reader.read_buf_bytes_available_ref();
    let Ok(mut bs) = BitReaderLtr::new(buf);

    let Ok(scale_bands) = bands(header.rate);
    let side_info = bitstream::read_side_info(&mut bs, header, scale_bands.long);

    let mut frame = match side_info {
      Ok(frame) => frame,
      Err(fault) => {
        let Ok(()) = self.reservoir.clear();

        return Err(fault);
      }
    };

    let Ok(side_info_len) = index(header.side_info_len);

    let main_data = match buf.get(side_info_len..) {
      Some(main_data) => main_data,
      None => return decode_error("mpa: frame shorter than its side information"),
    };

    let underflow = self
      .reservoir
      .fill(main_data, u32::from(frame.main_data_begin))?;

    match self.read_main_data(header, underflow.saturating_mul(8), &mut frame) {
      Ok(len) => {
        let Ok(()) = self.reservoir.consume(len);
      }
      Err(fault) => {
        let Ok(()) = self.reservoir.clear();

        return Err(fault);
      }
    }

    self.render(header, &scale_bands, &mut frame, out)
  }
}

struct Filters<'a> {
  overlap: &'a mut [[[f32; 18]; 32]; 2],
  synthesis: &'a mut [SynthesisState; 2],
  computed: &'a Computed,
}

struct Placing<'a> {
  at: u32,
  out: &'a mut AudioBuffer<f32>,
}

fn render_granule(
  header: &FrameHeader,
  scale_bands: &Bands,
  filters: &mut Filters<'_>,
  granule: &mut Granule,
  samples: &mut [Spectrum; 2],
  placing: Placing<'_>,
) -> Result<()> {
  let Placing { at, out } = placing;
  let Filters {
    overlap,
    synthesis,
    computed,
  } = filters;
  let Ok(channels) = index(header.channels);
  let Ok(from) = index(at.saturating_mul(GRANULE_SAMPLES));

  for (channel, spectrum) in granule
    .channels
    .iter()
    .zip(samples.iter_mut())
    .take(channels)
  {
    let Ok(()) = requantize::requantize(scale_bands, channel, spectrum);
  }

  let joined = match header.channel_mode {
    ChannelMode::Mono => Ok(()),
    ChannelMode::DualMono | ChannelMode::Stereo | ChannelMode::JointStereo { .. } => {
      stereo::stereo(header, scale_bands, granule, samples, computed)
    }
  };

  joined?;

  out.render_reserved(Some(GRANULE_SAMPLES))?;

  let lanes = granule
    .channels
    .iter_mut()
    .zip(samples.iter_mut())
    .zip(overlap.iter_mut())
    .zip(synthesis.iter_mut());

  for (lane, (((channel, spectrum), overlap_2), state)) in (0u32..).zip(lanes).take(channels) {
    let Ok(()) = hybrid_synthesis::reorder(scale_bands, channel, spectrum);
    let Ok(()) = hybrid_synthesis::antialias(channel, spectrum, computed);
    let Ok(()) = hybrid_synthesis::hybrid_synthesis(channel, overlap_2, spectrum, computed);
    let Ok(()) = hybrid_synthesis::frequency_inversion(spectrum);

    let Ok(value_1) = out.chan_mut(lane);

    for block in value_1.get_mut(from..).into_iter() {
      let Ok(()) = synthesis::synthesis(state, 18, spectrum, block);
    }
  }

  Ok(())
}
