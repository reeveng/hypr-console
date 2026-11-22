//! What a decoder looks up on every packet and computes once: the codebooks,
//! the power law of the quantiser, and the scalefactor gains.
//!
//! Each was a `lazy_static` in the code this was carried from, shared by every
//! decoder in the process. A decoder holds its own now, so none of them is a
//! value nobody handed anybody, and the cost is a few tens of kilobytes made
//! when a track is opened.

use console_core_audio_decoding_interfaces::errors::Result;

use crate::codebooks::Codebooks;

pub(crate) const INTENSITY_SCALE_MIN: i32 = -155;

pub(crate) const NORMAL_SCALE_MIN: i32 = -100;

pub(crate) struct Tables {
  pub(crate) pow43: Vec<f32>,
  pub(crate) normal_scales: Vec<f32>,
  pub(crate) intensity_scales: Vec<f32>,
  pub(crate) codebooks: Codebooks,
}

impl Tables {
  pub(crate) fn new() -> Result<Self> {
    let pow43 = (0u16..8192)
      .map(|value| f32::powf(f32::from(value), 4.0 / 3.0))
      .collect();
    let normal_scales = (0u8..=255)
      .map(|step| 2.0f32.powf(0.25 * f32::from(i16::from(step).saturating_sub(156))))
      .collect();
    let intensity_scales = (0u8..=255)
      .map(|step| 0.5f32.powf(0.25 * f32::from(i16::from(step).saturating_sub(155))))
      .collect();
    let codebooks = Codebooks::new()?;

    Ok(Self {
      pow43,
      normal_scales,
      intensity_scales,
      codebooks,
    })
  }
}
