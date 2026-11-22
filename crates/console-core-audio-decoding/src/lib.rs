//! Every container and codec the music player can open, registered in one
//! place.
//!
//! The family beside this crate is a decoder or a demuxer each, and none of
//! them knows the others. What a player needs is the two lists: a probe that
//! holds every format, so a file is recognized by what it starts with, and a
//! registry that holds every codec, so the track the probe found gets the
//! decoder that reads it. Both are made here and handed to whoever asked, and
//! the family's shared words are reachable as `core`, so a caller names one
//! crate.
//!
//! Each call makes its own list. Upstream kept one of each for the whole
//! process and handed out a reference to it; a list is a few entries in a map,
//! a caller opens one song at a time, and a value every caller shares is a
//! value nobody handed anybody.
//!
//! MPEG audio is layer three only. The decoder beside this crate can read
//! layers one and two, and the player has never asked it to.
//!
//! The crate began as symphonia, by the Project Symphonia developers under the
//! MPL-2.0, whose text is the `LICENSE` beside it. Upstream chose its formats
//! by feature; this desktop builds one player, which opens all of them.

use console_core_audio_decoding_advanced_audio_coding::{AacDecoder, AdtsReader};
use console_core_audio_decoding_apple_lossless::AlacDecoder;
use console_core_audio_decoding_flac::{FlacDecoder, FlacReader};
use console_core_audio_decoding_interfaces::codecs::{CODEC_TYPE_MP3, CodecRegistry, Decoder};
use console_core_audio_decoding_interfaces::probe::Probe;
use console_core_audio_decoding_matroska_container::MkvReader;
use console_core_audio_decoding_mp3::{MpaDecoder, MpaReader};
use console_core_audio_decoding_mpeg_4_container::IsoMp4Reader;
use console_core_audio_decoding_ogg_container::OggReader;
use console_core_audio_decoding_tags::id3v2::Id3v2Reader;
use console_core_audio_decoding_uncompressed::PcmDecoder;
use console_core_audio_decoding_vorbis::VorbisDecoder;
use console_core_audio_decoding_wave_container::{AiffReader, WavReader};
use console_core_never::Never;

pub use console_core_audio_decoding_interfaces as core;

pub fn codec_registry() -> Result<CodecRegistry, Never> {
  let Ok(mut registry) = CodecRegistry::new();

  let Ok(()) = registry.register_all::<AacDecoder>();
  let Ok(()) = registry.register_all::<AlacDecoder>();
  let Ok(()) = registry.register_all::<FlacDecoder>();
  let Ok(mpeg_codecs) = MpaDecoder::supported_codecs();

  for descriptor in mpeg_codecs
    .iter()
    .filter(|entry| entry.codec == CODEC_TYPE_MP3)
  {
    let Ok(()) = registry.register(descriptor);
  }

  let Ok(()) = registry.register_all::<PcmDecoder>();
  let Ok(()) = registry.register_all::<VorbisDecoder>();

  Ok(registry)
}

pub fn format_probe() -> Result<Probe, Never> {
  let mut probe = Probe::default();

  let Ok(()) = probe.register_all::<AdtsReader>();
  let Ok(()) = probe.register_all::<FlacReader>();
  let Ok(()) = probe.register_all::<IsoMp4Reader>();
  let Ok(()) = probe.register_all::<MpaReader>();
  let Ok(()) = probe.register_all::<AiffReader>();
  let Ok(()) = probe.register_all::<WavReader>();
  let Ok(()) = probe.register_all::<OggReader>();
  let Ok(()) = probe.register_all::<MkvReader>();
  let Ok(()) = probe.register_all::<Id3v2Reader>();

  Ok(probe)
}
