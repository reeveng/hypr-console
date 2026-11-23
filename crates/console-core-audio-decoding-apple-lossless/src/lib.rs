//! The Apple Lossless decoder: a packet is a run of elements, each one a
//! single channel or a pair, and the layout in the cookie says which output
//! channels they land on.
//!
//! The elements are decoded in `element` into buffers of their own and copied
//! out here, so nothing a stream says about a frame's length can name a slice
//! of the output that is not there. What an element is made of is in
//! `predictor`, and what the cookie says in `cookie`.

mod cookie;
mod element;
mod fault;
mod predictor;

use console_core_audio_decoding_interfaces::audio::{
  AsAudioBufferRef, AudioBuffer, AudioBufferRef, Signal, SignalSpec,
};
use core::cmp::Ordering;

use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_ALAC, CodecDescriptor, CodecParameters, Decoder, DecoderOptions, FinalizeResult,
};
use console_core_audio_decoding_interfaces::errors::Error;
use console_core_audio_decoding_interfaces::formats::Packet;
use console_core_audio_decoding_interfaces::support_codec;
use console_core_byte_streams::{BitReaderLtr, BufReader, FlagBit, ReadBitsLtr};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::cookie::MagicCookie;
use crate::element::{Coupling, Scratch};
use crate::fault::DecodeError;

const TAG_SINGLE_CHANNEL: u32 = 0;
const TAG_CHANNEL_PAIR: u32 = 1;
const TAG_COUPLING_CHANNEL: u32 = 2;
const TAG_LOW_FREQUENCY: u32 = 3;
const TAG_DATA_STREAM: u32 = 4;
const TAG_PROGRAM_CONFIG: u32 = 5;
const TAG_FILL: u32 = 6;
const TAG_END: u32 = 7;

const BITS_PER_TAG: u32 = 3;

const SAMPLE_BITS: u32 = 32;

enum Step {
  Next,
  Finished,
}

struct Progress {
  next_channel: u32,
  frames: u32,
}

pub struct AlacDecoder {
  params: CodecParameters,
  scratch: Scratch,
  config: MagicCookie,
  buf: AudioBuffer<i32>,
}

fn skip_data_stream<B: ReadBitsLtr>(bs: &mut B) -> Result<Step, DecodeError> {
  let _tag = bs.read_bits_leq32(4)?;
  let align = bs.read_flag()?;
  let first = bs.read_bits_leq32(8)?;
  let count = match first {
    255 => {
      let extra = bs.read_bits_leq32(8)?;

      first.wrapping_add(extra)
    }
    _ => first,
  };

  match align {
    FlagBit::One => {
      let Ok(()) = bs.realign();
    }
    FlagBit::Zero => {}
  }

  bs.ignore_bits(count.wrapping_mul(8))?;

  Ok(Step::Next)
}

fn skip_fill<B: ReadBitsLtr>(bs: &mut B) -> Result<Step, DecodeError> {
  let first = bs.read_bits_leq32(4)?;
  let count = match first {
    15 => {
      let extra = bs.read_bits_leq32(8)?;

      first.wrapping_add(extra).wrapping_sub(1)
    }
    _ => first,
  };

  bs.ignore_bits(count.wrapping_mul(8))?;

  Ok(Step::Next)
}

impl AlacDecoder {
  fn copy_out(&mut self, channel: Option<&u32>, source: Option<&[i32]>) -> Result<(), DecodeError> {
    let (target, from) = match (channel, source) {
      (Some(&mapped), Some(samples)) => (mapped, samples),
      _ => return Err(DecodeError::Channel),
    };

    let Ok(output) = self.buf.chan_mut(target);

    for (slot, &sample) in output.iter_mut().zip(from) {
      *slot = sample;
    }

    Ok(())
  }

  fn element<B: ReadBitsLtr>(
    &mut self,
    bs: &mut B,
    progress: &mut Progress,
    coupling: Coupling,
  ) -> Result<Step, DecodeError> {
    let Ok(taken) = coupling.channels();
    let wanted = progress.next_channel.saturating_add(taken);

    match wanted.cmp(&u32::from(self.config.num_channels)) {
      Ordering::Greater => return Ok(Step::Finished),
      Ordering::Less | Ordering::Equal => {}
    }

    let frames = self.scratch.decode(bs, &self.config, coupling)?;
    let Ok(map) = self.config.channel_layout.channel_map();
    let Ok(next) = index(progress.next_channel);
    let Ok(after) = index(progress.next_channel.saturating_add(1));
    let Ok(first_samples) = self.scratch.first(frames);
    let Ok(second_samples) = self.scratch.second(frames);
    let first = first_samples.map(<[i32]>::to_vec);
    let second = second_samples.map(<[i32]>::to_vec);

    self.copy_out(map.get(next), first.as_deref())?;

    match coupling {
      Coupling::Single => {}
      Coupling::Pair => self.copy_out(map.get(after), second.as_deref())?,
    }

    progress.next_channel = wanted;
    progress.frames = frames;

    Ok(Step::Next)
  }

  fn step<B: ReadBitsLtr>(&mut self, bs: &mut B, progress: &mut Progress) -> Result<Step, DecodeError> {
    let tag = bs.read_bits_leq32(BITS_PER_TAG)?;
    let outcome = match tag {
      TAG_SINGLE_CHANNEL | TAG_LOW_FREQUENCY => self.element(bs, progress, Coupling::Single),
      TAG_CHANNEL_PAIR => self.element(bs, progress, Coupling::Pair),
      TAG_DATA_STREAM => skip_data_stream(bs),
      TAG_FILL => skip_fill(bs),
      TAG_END => Ok(Step::Finished),
      TAG_COUPLING_CHANNEL | TAG_PROGRAM_CONFIG => Err(DecodeError::Element),
      _ => Err(DecodeError::Element),
    };
    let step = outcome?;

    match step {
      Step::Finished => Ok(Step::Finished),
      Step::Next => match progress
        .next_channel
        .cmp(&u32::from(self.config.num_channels))
      {
        Ordering::Equal | Ordering::Greater => Ok(Step::Finished),
        Ordering::Less => Ok(Step::Next),
      },
    }
  }

  fn decode_inner(&mut self, packet: &Packet) -> Result<(), DecodeError> {
    let Ok(value_1) = packet.buf();

    let Ok(mut bs) = BitReaderLtr::new(value_1);
    let mut progress = Progress {
      next_channel: 0,
      frames: 0,
    };
    let Ok(value_2) = packet.buf();

    let Ok(bytes): Result<u32, Never> = fitted(value_2.len());
    let most_elements = bytes.saturating_mul(8).saturating_add(1);

    let Ok(()) = self.buf.clear();
    self.buf.render_silence(None)?;

    for _ in 0..most_elements {
      let step = self.step(&mut bs, &mut progress)?;

      match step {
        Step::Next => {}
        Step::Finished => break,
      }
    }

    let shift = SAMPLE_BITS.saturating_sub(u32::from(self.config.bit_depth));

    let Ok(()) = self.buf.truncate(progress.frames);
    let Ok(()) = self.buf.transform(|sample| sample.wrapping_shl(shift));

    Ok(())
  }
}

impl Decoder for AlacDecoder {
  fn try_new(params: &CodecParameters, _: &DecoderOptions) -> Result<Self, Error> {
    match params.codec {
      CODEC_TYPE_ALAC => {}
      _unsupported => return Err(Error::from(DecodeError::CodecType)),
    }

    let cookie = match &params.extra_data {
      Some(extra_data) => {
        let Ok(mut cookie_reader) = BufReader::new(extra_data);

        MagicCookie::try_read(&mut cookie_reader)
      }
      None => Err(DecodeError::MissingExtraData),
    };
    let config = cookie?;
    let Ok(channels) = config.channel_layout.channels();
    let Ok(spec) = SignalSpec::new(config.sample_rate, channels);
    let buf = AudioBuffer::new(u64::from(config.frame_length), spec)?;
    let Ok(scratch) = Scratch::new(&config);

    Ok(AlacDecoder {
      params: params.clone(),
      scratch,
      buf,
      config,
    })
  }

  fn reset(&mut self) -> std::result::Result<(), console_core_never::Never> {
    Ok(())
  }

  fn supported_codecs()
  -> std::result::Result<&'static [CodecDescriptor], console_core_never::Never> {
    Ok(&[support_codec!(
      CODEC_TYPE_ALAC,
      "alac",
      "Apple Lossless Audio Codec"
    )])
  }

  fn codec_params(&self) -> std::result::Result<&CodecParameters, console_core_never::Never> {
    Ok(&self.params)
  }

  fn decode(&mut self, packet: &Packet) -> Result<AudioBufferRef<'_>, Error> {
    match self.decode_inner(packet) {
      Ok(()) => {
        let Ok(value_3) = self.buf.as_audio_buffer_ref();

        Ok(value_3)
      }
      Err(fault) => {
        let Ok(()) = self.buf.clear();

        Err(Error::from(fault))
      }
    }
  }

  fn finalize(&mut self) -> std::result::Result<FinalizeResult, console_core_never::Never> {
    Ok(FinalizeResult::default())
  }

  fn last_decoded(&self) -> std::result::Result<AudioBufferRef<'_>, console_core_never::Never> {
    let Ok(value_4) = self.buf.as_audio_buffer_ref();

    Ok(value_4)
  }
}
