//! The codec a track's id names, and what it takes for a depth to pick one.

use console_core_audio_decoding_interfaces::codecs;
use console_core_audio_decoding_interfaces::codecs::CodecType;
use console_core_never::Never;

use crate::segment::TrackElement;

fn by_depth(
  bit_depth: Option<u64>,
  table: &[(u64, CodecType)],
) -> Result<Option<CodecType>, Never> {
  Ok(
    table
      .iter()
      .find(|(bits, _)| Some(*bits) == bit_depth)
      .map(|(_, codec)| *codec),
  )
}

pub(crate) fn codec_id_to_type(track: &TrackElement) -> Result<Option<CodecType>, Never> {
  let bit_depth = track.audio.as_ref().and_then(|audio| audio.bit_depth);

  match track.codec_id.as_str() {
    "A_MPEG/L1" => Ok(Some(codecs::CODEC_TYPE_MP1)),
    "A_MPEG/L2" => Ok(Some(codecs::CODEC_TYPE_MP2)),
    "A_MPEG/L3" => Ok(Some(codecs::CODEC_TYPE_MP3)),
    "A_FLAC" => Ok(Some(codecs::CODEC_TYPE_FLAC)),
    "A_OPUS" => Ok(Some(codecs::CODEC_TYPE_OPUS)),
    "A_VORBIS" => Ok(Some(codecs::CODEC_TYPE_VORBIS)),
    "A_AAC/MPEG2/MAIN" | "A_AAC/MPEG2/LC" | "A_AAC/MPEG2/LC/SBR" | "A_AAC/MPEG2/SSR"
    | "A_AAC/MPEG4/MAIN" | "A_AAC/MPEG4/LC" | "A_AAC/MPEG4/LC/SBR" | "A_AAC/MPEG4/SSR"
    | "A_AAC/MPEG4/LTP" | "A_AAC" => Ok(Some(codecs::CODEC_TYPE_AAC)),
    "A_PCM/INT/BIG" => by_depth(
      bit_depth,
      &[
        (16, codecs::CODEC_TYPE_PCM_S16BE),
        (24, codecs::CODEC_TYPE_PCM_S24BE),
        (32, codecs::CODEC_TYPE_PCM_S32BE),
      ],
    ),
    "A_PCM/INT/LIT" => by_depth(
      bit_depth,
      &[
        (16, codecs::CODEC_TYPE_PCM_S16LE),
        (24, codecs::CODEC_TYPE_PCM_S24LE),
        (32, codecs::CODEC_TYPE_PCM_S32LE),
      ],
    ),
    "A_PCM/FLOAT/IEEE" => by_depth(
      bit_depth,
      &[
        (32, codecs::CODEC_TYPE_PCM_F32LE),
        (64, codecs::CODEC_TYPE_PCM_F64LE),
      ],
    ),
    _ => Ok(None),
  }
}
