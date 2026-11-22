//! The magic cookie an Apple Lossless entry carries for its decoder.

use console_core_audio_decoding_interfaces::codecs::{CODEC_TYPE_ALAC, CodecParameters};
use console_core_audio_decoding_interfaces::errors::{Result, decode_error, unsupported_error};
use console_core_byte_streams::ReadBytes;
use console_core_never::Never;

use crate::atoms::{Atom, AtomHeader};

pub struct AlacAtom {
  extra_data: Box<[u8]>,
}

impl Atom for AlacAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let full_box = AtomHeader::read_full_box(reader)?;

    match full_box.version {
      0 => {}
      _ => return unsupported_error("isomp4 (alac): unsupported alac version"),
    }

    match full_box.flags {
      0 => {}
      _ => return decode_error("isomp4 (alac): flags not zero"),
    }

    let parsed_cookie_len = header.payload_after_full_box()?;
    let cookie_len = match parsed_cookie_len {
      0 => return decode_error("isomp4 (alac): invalid alac atom length"),
      len @ (24 | 48) => len,
      _ => return decode_error("isomp4 (alac): invalid magic cookie length"),
    };
    let extra_data = reader.read_boxed_slice_exact(cookie_len)?;
    Ok(AlacAtom { extra_data })
  }
}

impl AlacAtom {
  pub fn fill_codec_params(
    &self,
    codec_params: &mut CodecParameters,
  ) -> std::result::Result<(), Never> {
    let Ok(_configured_for_codec) = codec_params.for_codec(CODEC_TYPE_ALAC);
    let Ok(_configured_with_extra_data) = codec_params.with_extra_data(self.extra_data.clone());

    Ok(())
  }
}
