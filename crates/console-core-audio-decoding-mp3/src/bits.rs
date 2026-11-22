//! Reading a field of a given width out of a bit stream, as the type it is.
//!
//! Every layer reads its side information and samples as fields of a few
//! bits, and each wants them as the narrowest type that holds them, so the
//! narrowing is written once here and a layer says only how wide.

use console_core_audio_decoding_interfaces::errors::Result;
use console_core_byte_streams::ReadBitsLtr;
use console_core_number_conversion::{Ends, fitted};

pub trait Fields: ReadBitsLtr {
  fn field<T>(&mut self, width: u32) -> Result<T>
  where
    T: TryFrom<u32> + Ends,
  {
    let raw = self.read_bits_leq32(width)?;
    let Ok(held) = fitted::<u32, T>(raw);

    Ok(held)
  }
}

impl<B: ReadBitsLtr> Fields for B {}
