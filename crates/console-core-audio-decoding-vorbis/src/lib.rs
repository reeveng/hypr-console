//! A Vorbis decoder: the codec's extra data is its two headers, each packet is
//! audio for every channel, and what comes out is planar `f32` in the order
//! the channels are laid out in the stream's channel map.
//!
//! The crate began as symphonia-codec-vorbis, by the Project Symphonia
//! developers under the MPL-2.0, whose text is the `LICENSE` beside it. It
//! decodes by the sections of the Vorbis I specification in order: the packet's
//! mode and windows, a floor for each channel, the residue for each submap,
//! the inverse coupling of channel pairs, the dot product of the two, and the
//! inverse MDCT with its overlap-add (`dsp`). What was upstream's to guess at --
//! a stream that names a codebook, a floor or a channel that is not there --
//! is the same fault a stream with a bad header is, because nothing here
//! indexes a list a stream chose the position in.
//!
//! Streams with more than eight channels have no channel map in the
//! specification and are refused, as are streams of more than 32.

mod codebook;
mod dsp;
mod floor;
mod floor0;
mod floor1;
mod inverse_db;
mod reading;
mod residue;
mod setup;
mod window;

use console_core_audio_decoding_flac_and_vorbis_headers::vorbis::vorbis_channels_to_channels;
use console_core_audio_decoding_interfaces::audio::{
  AsAudioBufferRef, AudioBuffer, AudioBufferRef,
};
use console_core_audio_decoding_interfaces::audio::{Signal, SignalSpec};
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_VORBIS, CodecDescriptor, CodecParameters,
};
use console_core_audio_decoding_interfaces::codecs::{Decoder, DecoderOptions, FinalizeResult};
use console_core_audio_decoding_interfaces::errors::{Error, decode_error, unsupported_error};
use console_core_audio_decoding_interfaces::formats::Packet;
use console_core_audio_decoding_interfaces::support_codec;
use console_core_byte_streams::{BitReaderRtl, BufReader, FlagBit, ReadBitsRtl};
use console_core_frequency_transforms::{Fault as TransformFault, mdct::Imdct};
use console_core_number_conversion::fitted;

use codebook::{VorbisCodebook, ilog};
use dsp::{Decoding, Dsp, DspChannel, Lapping};
use floor::Floor;
use reading::{entry, entry_mut};
use residue::Residue;
use setup::{IdentHeader, Mapping, Mode, Setup, read_ident_header, read_setup};
use window::{Block, Blocks, Windows};

const CHANNEL_MAPS: [&[u8]; 8] = [
  &[0],
  &[0, 1],
  &[0, 2, 1],
  &[0, 1, 2, 3],
  &[0, 2, 1, 3, 4],
  &[0, 2, 1, 4, 5, 3],
  &[0, 2, 1, 5, 6, 4, 3],
  &[0, 2, 1, 6, 7, 4, 5, 3],
];

pub struct VorbisDecoder {
  params: CodecParameters,
  ident: IdentHeader,
  setup: Setup,
  dsp: Dsp,
  buf: AudioBuffer<f32>,
}

struct Work<'a> {
  codebooks: &'a [VorbisCodebook],
  floors: &'a mut [Floor],
  residues: &'a mut [Residue],
  dsp: &'a mut Dsp,
}

fn map_vorbis_channel(num_channels: u8, ch: u32) -> Result<u32, Error> {
  let map = entry(&CHANNEL_MAPS, u32::from(num_channels).saturating_sub(1))?;
  let mapped = entry(map, ch)?;

  Ok(u32::from(*mapped))
}

fn select_mode<'a>(
  modes: &'a [Mode],
  mappings: &'a [Mapping],
  bs: &mut BitReaderRtl<'_>,
) -> Result<(Mode, &'a Mapping), Error> {
  let first = bs.read_flag()?;

  match first {
    FlagBit::One => return decode_error("vorbis: not an audio packet"),
    FlagBit::Zero => {}
  }

  let Ok(count) = fitted::<_, u32>(modes.len());
  let Ok(width) = ilog(count.saturating_sub(1));
  let mode_number = bs.read_bits_leq32(width)?;
  let mode = match entry(modes, mode_number) {
    Ok(mode) => *mode,
    Err(_missing) => return decode_error("vorbis: invalid packet mode number"),
  };
  let mapping = entry(mappings, u32::from(mode.mapping))?;

  Ok((mode, mapping))
}

impl Work<'_> {
  fn read_floors(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    mapping: &Mapping,
    block: Block,
    half: u32,
  ) -> Result<(), Error> {
    for (submap_number, channel) in mapping.multiplex.iter().zip(self.dsp.channels.iter_mut()) {
      let submap = entry(&mapping.submaps, u32::from(*submap_number))?;
      let floor = entry_mut(self.floors, u32::from(submap.floor))?;

      floor.read_channel(bs, self.codebooks)?;

      let Ok(decoding) = floor.decoding();

      channel.decoding = decoding;

      match decoding {
        Decoding::Needed => floor.synthesis(block, half, &mut channel.floor)?,
        Decoding::Skipped => {
          let Ok(()) = channel.silence_floor(half);
        }
      }
    }

    Ok(())
  }

  fn read_residues(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    mapping: &Mapping,
    half: u32,
  ) -> Result<(), Error> {
    for (submap_number, submap) in (0u8..).zip(mapping.submaps.iter()) {
      let mut members: Vec<&mut DspChannel> = self
        .dsp
        .channels
        .iter_mut()
        .zip(mapping.multiplex.iter())
        .filter(|(_, number)| **number == submap_number)
        .map(|(channel, _)| channel)
        .collect();
      let residue = entry_mut(self.residues, u32::from(submap.residue))?;

      residue.read_residue(bs, half, self.codebooks, &mut members)?;
    }

    Ok(())
  }
}

impl VorbisDecoder {
  fn decode_inner(&mut self, packet: &Packet) -> Result<(), Error> {
    let Ok(value_1) = packet.buf();

    let Ok(mut bs) = BitReaderRtl::new(value_1);
    let Setup {
      codebooks,
      floors,
      residues,
      mappings,
      modes,
    } = &mut self.setup;
    let (mode, mapping) = select_mode(modes, mappings, &mut bs)?;

    match mode.block {
      Block::Long => {
        let _previous_window_flag = bs.read_flag()?;
        let _next_window_flag = bs.read_flag()?;
      }
      Block::Short => {}
    }

    let Ok(half) = self.ident.blocks.half(mode.block);
    let mut work = Work {
      codebooks,
      floors,
      residues,
      dsp: &mut self.dsp,
    };

    work.read_floors(&mut bs, mapping, mode.block, half)?;

    for coupling in &mapping.couplings {
      work.dsp.propagate(*coupling)?;
    }

    work.read_residues(&mut bs, mapping, half)?;

    for coupling in &mapping.couplings {
      work.dsp.couple(*coupling, half)?;
    }

    for channel in &mut work.dsp.channels {
      let Ok(()) = channel.weigh(half);
    }

    self.synthesize(mode.block, packet)
  }

  fn synthesize(&mut self, block: Block, packet: &Packet) -> Result<(), Error> {
    let Ok(()) = self.buf.clear();

    let previous = self.dsp.previous;

    match previous {
      Some(previous_2) => {
        let Ok(before) = self.ident.blocks.size(previous_2);
        let Ok(now) = self.ident.blocks.size(block);

        self
          .buf
          .render_reserved(Some(before.saturating_add(now).wrapping_shr(2)))?;
      }
      None => {}
    }

    let Dsp {
      channels,
      imdct_short,
      imdct_long,
      windows,
      ..
    } = &mut self.dsp;
    let imdct = match block {
      Block::Long => imdct_long,
      Block::Short => imdct_short,
    };

    for (number, channel) in (0u32..).zip(channels.iter_mut()) {
      let target = map_vorbis_channel(self.ident.n_channels, number)?;

      let Ok(value_2) = self.buf.chan_mut(target);

      channel.synth(block, Lapping { previous, windows }, imdct, value_2)?;
    }

    let Ok(value_3) = packet.trim_start();
    let Ok(value_4) = packet.trim_end();

    let Ok(()) = self.buf.trim(value_3, value_4);
    self.dsp.previous = Some(block);

    Ok(())
  }
}

impl Decoder for VorbisDecoder {
  fn try_new(params: &CodecParameters, _: &DecoderOptions) -> Result<Self, Error> {
    match params.codec {
      CODEC_TYPE_VORBIS => {}
      _other => return unsupported_error("vorbis: invalid codec type"),
    }

    let extra_data = match params.extra_data.as_ref() {
      Some(buf) => buf,
      None => return unsupported_error("vorbis: missing extra data"),
    };
    let Ok(mut reader) = BufReader::new(extra_data);
    let ident = read_ident_header(&mut reader)?;
    let setup = read_setup(&mut reader, &ident)?;
    let blocks: Blocks = ident.blocks;
    let Ok(windows) = Windows::new(blocks);
    let mut channels = Vec::new();

    for _ in 0..ident.n_channels {
      let Ok(channel) = DspChannel::new(blocks);

      channels.push(channel);
    }

    let layout = vorbis_channels_to_channels(ident.n_channels)?;
    let Ok(spec) = SignalSpec::new(ident.sample_rate, layout);
    let Ok(short_half) = blocks.half(Block::Short);
    let Ok(long_half) = blocks.half(Block::Long);
    let Ok(long_size) = blocks.size(Block::Long);
    let imdct_short = match Imdct::new(short_half) {
      Ok(imdct) => imdct,
      Err(TransformFault::Size | TransformFault::Spectrum | TransformFault::Output) => {
        return unsupported_error("vorbis: no transform of the short block size");
      }
    };
    let imdct_long = match Imdct::new(long_half) {
      Ok(imdct) => imdct,
      Err(TransformFault::Size | TransformFault::Spectrum | TransformFault::Output) => {
        return unsupported_error("vorbis: no transform of the long block size");
      }
    };
    let dsp = Dsp {
      windows,
      channels,
      imdct_short,
      imdct_long,
      previous: None,
    };

    let buf = AudioBuffer::new(u64::from(long_size), spec)?;

    Ok(VorbisDecoder {
      params: params.clone(),
      ident,
      setup,
      dsp,
      buf,
    })
  }

  fn reset(&mut self) -> std::result::Result<(), console_core_never::Never> {
    let Ok(()) = self.dsp.reset();

    Ok(())
  }

  fn supported_codecs()
  -> std::result::Result<&'static [CodecDescriptor], console_core_never::Never> {
    Ok(&[support_codec!(CODEC_TYPE_VORBIS, "vorbis", "Vorbis")])
  }

  fn codec_params(&self) -> std::result::Result<&CodecParameters, console_core_never::Never> {
    Ok(&self.params)
  }

  fn decode(&mut self, packet: &Packet) -> Result<AudioBufferRef<'_>, Error> {
    match self.decode_inner(packet) {
      Ok(()) => {
        let Ok(value_5) = self.buf.as_audio_buffer_ref();

        Ok(value_5)
      }
      Err(fault) => {
        let Ok(()) = self.buf.clear();

        Err(fault)
      }
    }
  }

  fn finalize(&mut self) -> std::result::Result<FinalizeResult, console_core_never::Never> {
    Ok(FinalizeResult::default())
  }

  fn last_decoded(&self) -> std::result::Result<AudioBufferRef<'_>, console_core_never::Never> {
    let Ok(value_6) = self.buf.as_audio_buffer_ref();

    Ok(value_6)
  }
}
