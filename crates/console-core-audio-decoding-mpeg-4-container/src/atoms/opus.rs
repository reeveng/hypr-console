//! The `dOps` atom of an Opus entry, handed to the decoder as the identification header an Ogg
//! stream would have carried: the magic in front, and the fields little-endian where the atom
//! wrote them big-endian.

use console_core_audio_decoding_interfaces::codecs::{CODEC_TYPE_OPUS, CodecParameters};
use console_core_audio_decoding_interfaces::errors::{Result, decode_error, unsupported_error};
use console_core_byte_streams::ReadBytes;
use console_core_never::Never;

use crate::atoms::{Atom, AtomHeader};

const OPUS_MAGIC: &[u8] = b"OpusHead";

const MIN_DOPS_SIZE: u64 = 11;

const MAX_DOPS_SIZE: u64 = MIN_DOPS_SIZE + 257;

pub struct OpusAtom {
  extra_data: Box<[u8]>,
}

impl Atom for OpusAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    match ::core::primitive::bool::then_some(header.data_len < MIN_DOPS_SIZE, ()) {
      ::core::option::Option::Some(()) => {
        return decode_error("isomp4 (opus): opus identification header too short");
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(header.data_len > MAX_DOPS_SIZE, ()) {
      ::core::option::Option::Some(()) => {
        return decode_error("isomp4 (opus): opus identification header too large");
      }
      ::core::option::Option::None => {}
    }

    let dops = reader.read_boxed_slice_exact(header.data_len)?;

    match &*dops {
      [
        0,
        channels,
        pre_skip_high,
        pre_skip_low,
        rate_3,
        rate_2,
        rate_1,
        rate_0,
        gain_high,
        gain_low,
        mapping @ ..,
      ] => {
        let mut extra_data = Vec::from(OPUS_MAGIC);

        extra_data.extend_from_slice(&[0, *channels, *pre_skip_low, *pre_skip_high]);
        extra_data.extend_from_slice(&[*rate_0, *rate_1, *rate_2, *rate_3]);
        extra_data.extend_from_slice(&[*gain_low, *gain_high]);
        extra_data.extend_from_slice(mapping);

        Ok(OpusAtom {
          extra_data: extra_data.into_boxed_slice(),
        })
      }
      _ => unsupported_error("isomp4 (opus): unsupported opus version"),
    }
  }
}

impl OpusAtom {
  pub fn fill_codec_params(
    &self,
    codec_params: &mut CodecParameters,
  ) -> std::result::Result<(), Never> {
    let Ok(_configured_for_codec) = codec_params.for_codec(CODEC_TYPE_OPUS);
    let Ok(_configured_with_extra_data) = codec_params.with_extra_data(self.extra_data.clone());

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::atoms::Kind;
  use console_core_byte_streams::BufReader;

  #[test]
  fn identification_header_fields_use_the_byte_order_expected_by_the_decoder() -> Result<()> {
    let input = [0, 2, 0x01, 0x38, 0, 0, 0xbb, 0x80, 0xff, 0, 0];
    let header = AtomHeader {
      kind: Kind(*b"dOps"),
      header_len: 8,
      atom_len: 19,
      data_len: 11,
    };
    let Ok(mut stream) = BufReader::new(&input);
    let atom = OpusAtom::read(&mut stream, header)?;
    let expected = [
      b'O', b'p', b'u', b's', b'H', b'e', b'a', b'd', 0, 2, 0x38, 0x01, 0x80, 0xbb, 0, 0, 0,
      0xff, 0,
    ];
    assert_eq!(&*atom.extra_data, &expected);

    Ok(())
  }
}
