//! The decoder: a packet of ADTS or MP4 payload in, a frame of samples out.
//!
//! An AAC payload is a run of syntactic elements, each led by a three bit id,
//! until one says it is the last. The channel elements are decoded to spectra,
//! the filler and data elements are stepped over, and then every channel that
//! was decoded is turned into samples. The elements that carry something this
//! decoder does not do (coupling channels and program configs) are refused
//! rather than skipped, because skipping one leaves the bits after it unread.

use console_core_audio_decoding_interfaces::audio::{
  AsAudioBufferRef, AudioBuffer, AudioBufferRef, Signal, SignalSpec,
};
use core::cmp::Ordering;

use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_AAC, CodecDescriptor, CodecParameters, Decoder, DecoderOptions, FinalizeResult,
};
use console_core_audio_decoding_interfaces::errors::{Result, unsupported_error};
use console_core_audio_decoding_interfaces::formats::Packet;
use console_core_audio_decoding_interfaces::support_codec;
use console_core_audio_decoding_interfaces::units::Duration;
use console_core_byte_streams::{BitReaderLtr, FiniteBitStream, FlagBit, ReadBitsLtr};
use console_core_number_conversion::index;

use crate::config::{Configuration, map_channels};
use crate::lane::{Constraint, Lane, Sure};
use crate::pair::{ChannelPair, Layout};
use crate::subbands::Subbands;
use crate::synthesis::Dsp;
use crate::tables::Tables;

const SAMPLES_PER_FRAME: Duration = 1024;

#[derive(Clone, Copy)]
enum Element {
  SingleChannel,
  ChannelPair,
  Coupling,
  Lfe,
  DataStream,
  ProgramConfig,
  Fill,
  End,
}

impl Element {
  fn from_bits(bits: u32) -> Sure<Self> {
    Ok(match bits {
      0 => Element::SingleChannel,
      1 => Element::ChannelPair,
      2 => Element::Coupling,
      3 => Element::Lfe,
      4 => Element::DataStream,
      5 => Element::ProgramConfig,
      6 => Element::Fill,
      _ => Element::End,
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
  More,
  End,
}

struct Cursor {
  pairs: u32,
  channel: u32,
}

pub struct AacDecoder {
  config: Configuration,
  pairs: Vec<ChannelPair>,
  dsp: Dsp,
  subbands: Subbands,
  tables: Tables,
  params: CodecParameters,
  buf: AudioBuffer<f32>,
}

impl AacDecoder {
  fn set_pair(&mut self, cursor: &Cursor, layout: Layout) -> Result<()> {
    let Ok(next) = index(cursor.pairs);

    match self.pairs.len().cmp(&next) {
      Ordering::Less | Ordering::Equal => {
        let Ok(pair) = ChannelPair::new(layout, cursor.channel, self.subbands);

        self.pairs.push(pair);
      }
      Ordering::Greater => {
        let existing = self.pairs.borrowed(cursor.pairs)?;

        Constraint::from(existing.channel == cursor.channel).require()?;
        Constraint::from(existing.layout == layout).require()?;
      }
    }

    let last = match layout {
      Layout::Single => cursor.channel,
      Layout::Pair => cursor.channel.saturating_add(1),
    };

    Constraint::from(last < self.config.channels).require()
  }

  fn decode_element<B: ReadBitsLtr + FiniteBitStream>(
    &mut self,
    bs: &mut B,
    element: Element,
    cursor: &mut Cursor,
  ) -> Result<Step> {
    match element {
      Element::SingleChannel | Element::Lfe => {
        let _tag = bs.read_bits_leq32(4)?;

        self.set_pair(cursor, Layout::Single)?;

        let pair = self.pairs.borrowed_mut(cursor.pairs)?;

        pair.decode_ga_sce(bs, &self.tables)?;
        cursor.pairs = cursor.pairs.saturating_add(1);
        cursor.channel = cursor.channel.saturating_add(1);

        Ok(Step::More)
      }
      Element::ChannelPair => {
        let _tag = bs.read_bits_leq32(4)?;

        self.set_pair(cursor, Layout::Pair)?;

        let pair = self.pairs.borrowed_mut(cursor.pairs)?;

        pair.decode_ga_cpe(bs, &self.tables)?;
        cursor.pairs = cursor.pairs.saturating_add(1);
        cursor.channel = cursor.channel.saturating_add(2);

        Ok(Step::More)
      }
      Element::Coupling => unsupported_error("aac: coupling channel element"),
      Element::ProgramConfig => unsupported_error("aac: program config"),
      Element::DataStream => {
        skip_data_stream(bs)?;

        Ok(Step::More)
      }
      Element::Fill => {
        skip_fill(bs)?;

        Ok(Step::More)
      }
      Element::End => Ok(Step::End),
    }
  }

  fn decode_ga<B: ReadBitsLtr + FiniteBitStream>(&mut self, bs: &mut B) -> Result<()> {
    let mut cursor = Cursor {
      pairs: 0,
      channel: 0,
    };
    let elements = bs.bits_left().wrapping_div(3).saturating_add(1);

    for _ in 0..elements {
      match bs.bits_left() {
        0..=3 => break,
        4.. => {}
      }

      let id = bs.read_bits_leq32(3)?;
      let Ok(element) = Element::from_bits(id);
      let step = self.decode_element(bs, element, &mut cursor)?;

      match step {
        Step::End => break,
        Step::More => {}
      }
    }

    let Ok(count) = index(cursor.pairs);

    for pair in self.pairs.iter_mut().take(count) {
      pair.synth_audio(&mut self.dsp, &mut self.buf)?;
    }

    Ok(())
  }

  fn decode_inner(&mut self, packet: &Packet) -> Result<()> {
    let Ok(()) = self.buf.clear();
    self.buf.render_reserved(None)?;

    let Ok(value_1) = packet.buf();

    let Ok(mut bs) = BitReaderLtr::new(value_1);

    self.decode_ga(&mut bs)
  }
}

fn skip_data_stream<B: ReadBitsLtr>(bs: &mut B) -> Result<()> {
  let _instance = bs.read_bits_leq32(4)?;
  let align = bs.read_flag()?;
  let mut count = bs.read_bits_leq32(8)?;

  match count {
    255 => {
      let more = bs.read_bits_leq32(8)?;

      count = count.saturating_add(more);
    }
    _ => {}
  }

  match align {
    FlagBit::One => {
      let Ok(()) = bs.realign();
    }
    FlagBit::Zero => {}
  }

  bs.ignore_bits(count.saturating_mul(8))?;

  Ok(())
}

fn skip_fill<B: ReadBitsLtr>(bs: &mut B) -> Result<()> {
  let mut count = bs.read_bits_leq32(4)?;

  match count {
    15 => {
      let more = bs.read_bits_leq32(8)?;

      count = count.saturating_add(more).saturating_sub(1);
    }
    _ => {}
  }

  for _ in 0..count {
    bs.ignore_bits(8)?;
  }

  Ok(())
}

impl Decoder for AacDecoder {
  fn try_new(params: &CodecParameters, _: &DecoderOptions) -> Result<Self> {
    match params.codec {
      CODEC_TYPE_AAC => {}
      _unsupported => return unsupported_error("aac: invalid codec type"),
    }

    let config = match &params.extra_data {
      Some(extra_data) => Configuration::from_audio_specific_config(extra_data)?,
      None => Configuration::from_parameters(params)?,
    };
    let Ok(layout) = map_channels(config.channels);
    let channels = match layout {
      Some(channels) => channels,
      None => return unsupported_error("aac: channels or channel layout is required"),
    };
    let Ok(spec) = SignalSpec::new(config.sample_rate, channels);
    let Ok(subbands) = Subbands::find(config.sample_rate);
    let tables = Tables::new()?;
    let dsp = Dsp::new()?;
    let buf = AudioBuffer::new(SAMPLES_PER_FRAME, spec)?;

    Ok(AacDecoder {
      config,
      pairs: Vec::new(),
      dsp,
      subbands,
      tables,
      params: params.clone(),
      buf,
    })
  }

  fn reset(&mut self) -> std::result::Result<(), console_core_never::Never> {
    for pair in self.pairs.iter_mut() {
      let Ok(()) = pair.reset();
    }

    Ok(())
  }

  fn supported_codecs()
  -> std::result::Result<&'static [CodecDescriptor], console_core_never::Never> {
    Ok(&[support_codec!(
      CODEC_TYPE_AAC,
      "aac",
      "Advanced Audio Coding"
    )])
  }

  fn codec_params(&self) -> std::result::Result<&CodecParameters, console_core_never::Never> {
    Ok(&self.params)
  }

  fn decode(&mut self, packet: &Packet) -> Result<AudioBufferRef<'_>> {
    match self.decode_inner(packet) {
      Err(fault) => {
        let Ok(()) = self.buf.clear();

        Err(fault)
      }
      Ok(()) => {
        let Ok(value_2) = self.buf.as_audio_buffer_ref();

        Ok(value_2)
      }
    }
  }

  fn finalize(&mut self) -> std::result::Result<FinalizeResult, console_core_never::Never> {
    Ok(Default::default())
  }

  fn last_decoded(&self) -> std::result::Result<AudioBufferRef<'_>, console_core_never::Never> {
    let Ok(value_3) = self.buf.as_audio_buffer_ref();

    Ok(value_3)
  }
}
