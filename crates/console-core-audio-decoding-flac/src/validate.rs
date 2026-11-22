//! The MD5 a FLAC stream carries of its own decoded samples.
//!
//! FLAC hashes the samples as the encoder was handed them: interleaved, little endian, and as
//! many bytes each as the stream's bit depth rounds up to. The decoder works in planar 32 bit
//! samples, so what is hashed has to be rebuilt from those, one frame at a time.

use console_core_audio_decoding_interfaces::audio::{AudioBuffer, Signal};
use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_byte_streams::Monitor;
use console_core_checksums::md5::Md5;
use console_core_never::Never;
use console_core_number_conversion::index;

#[derive(Default)]
pub struct Validator {
  state: Md5,
  interleaved: Vec<u8>,
}

fn bytes_per_sample(bits_per_sample: u32) -> Result<Option<u32>> {
  match bits_per_sample {
    0 => Ok(None),
    1..=8 => Ok(Some(1)),
    9..=16 => Ok(Some(2)),
    17..=24 => Ok(Some(3)),
    25..=32 => Ok(Some(4)),
    _ => decode_error("flac: samples wider than 32 bits cannot be verified"),
  }
}

impl Validator {
  pub fn update(&mut self, buf: &AudioBuffer<i32>, bits_per_sample: u32) -> Result<()> {
    let sized = bytes_per_sample(bits_per_sample)?;

    let width = match sized {
      Some(width) => width,
      None => return Ok(()),
    };
    let Ok(kept) = index(width);

    self.interleaved.clear();

    let Ok(planes) = buf.planes();
    let Ok(channels) = planes.planes();
    let mut channels_2: Vec<_> = channels.iter().map(|plane| plane.iter()).collect();

    let Ok(value_1) = buf.frames();

    for _ in 0..value_1 {
      for channel in channels_2.iter_mut() {
        match channel.next() {
          Some(sample) => self
            .interleaved
            .extend(sample.to_le_bytes().into_iter().take(kept)),
          None => return decode_error("flac: a channel is shorter than the frame"),
        }
      }
    }

    let Ok(()) = self.state.process_buf_bytes(&self.interleaved);

    Ok(())
  }

  pub fn md5(&self) -> std::result::Result<[u8; 16], Never> {
    self.state.md5()
  }
}
