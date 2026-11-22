//! Pulse data (4.6.3): up to four lines of the spectrum that are corrected
//! after the rest of it has been dequantised.

use core::cmp::Ordering;

use console_core_audio_decoding_interfaces::errors::Result;
use console_core_byte_streams::{FlagBit, ReadBitsLtr};
use console_core_number_conversion::{fitted, index};

use crate::lane::{Grid, Lane, Sure};
use crate::spectrum::Scale;
use crate::subbands::{Scales, Spectrum};

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped value is a signed quantized spectral coefficient before inverse quantization"
  )
)]
struct Quantized(f32);

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped value is a dequantized line of the audio spectrum"
  )
)]
struct SpectralLine(f32);

fn iquant(Quantized(value): Quantized) -> Sure<f32> {
  Ok(match value.partial_cmp(&0.0) {
    Some(Ordering::Less) => -((-value).powf(4.0 / 3.0)),
    Some(Ordering::Equal | Ordering::Greater) | None => value.powf(4.0 / 3.0),
  })
}

struct Correction {
  line: SpectralLine,
  scale: Scale,
  amplitude: u8,
}

impl Correction {
  fn requant(&self) -> Sure<Quantized> {
    let line = self.line.0;

    match self.scale.0.abs().to_bits() {
      0 => Ok(Quantized(0.0)),
      _nonzero => Ok(Quantized(match (line / self.scale.0).partial_cmp(&0.0) {
        Some(Ordering::Equal | Ordering::Greater) => line.powf(3.0 / 4.0),
        Some(Ordering::Less) | None => -((-line).powf(3.0 / 4.0)),
      })),
    }
  }

  fn restored(&self) -> Sure<f32> {
    let Ok(Quantized(base)) = match self.line.0.abs().to_bits() {
      0 => Ok(Quantized(self.line.0)),
      _nonzero => self.requant(),
    };
    let amplitude = f32::from(self.amplitude);
    let base_2 = match base.partial_cmp(&0.0) {
      Some(Ordering::Greater) => base + amplitude,
      Some(Ordering::Equal | Ordering::Less) | None => base - amplitude,
    };
    let Ok(restored) = iquant(Quantized(base_2));

    Ok(restored * self.scale.0)
  }
}

#[derive(Clone, Copy)]
pub(crate) struct Pulse {
  count: u32,
  start_sfb: u32,
  offsets: [u32; 4],
  amplitudes: [u8; 4],
}

impl Pulse {
  pub(crate) fn read<B: ReadBitsLtr>(bs: &mut B) -> Result<Option<Self>> {
    let present = bs.read_flag()?;

    match present {
      FlagBit::Zero => Ok(None),
      FlagBit::One => {
        let pulse = Self::read_fields(bs)?;

        Ok(Some(pulse))
      }
    }
  }

  fn read_fields<B: ReadBitsLtr>(bs: &mut B) -> Result<Self> {
    let extra = bs.read_bits_leq32(2)?;
    let count = extra.saturating_add(1);
    let start_sfb = bs.read_bits_leq32(6)?;
    let mut offsets = [0; 4];
    let mut amplitudes = [0; 4];

    for pulse in 0..count {
      let offset = bs.read_bits_leq32(5)?;
      let amplitude = bs.read_bits_leq32(4)?;
      let Ok(amplitude_2) = fitted::<u32, u8>(amplitude);

      offsets.put(pulse, offset)?;
      amplitudes.put(pulse, amplitude_2)?;
    }

    Ok(Self {
      count,
      start_sfb,
      offsets,
      amplitudes,
    })
  }

  pub(crate) fn synth(
    &self,
    bands: &[u32],
    scales: &Scales,
    coeffs: &mut Spectrum,
  ) -> Result<()> {
    let Ok(band_count) = fitted::<_, u32>(bands.len());

    match self.start_sfb.cmp(&band_count.saturating_sub(1)) {
      Ordering::Equal | Ordering::Greater => Ok(()),
      Ordering::Less => self.apply(bands, scales, coeffs),
    }
  }

  fn apply(&self, bands: &[u32], scales: &Scales, coeffs: &mut Spectrum) -> Result<()> {
    let mut position = bands.at(self.start_sfb)?;
    let mut band = self.start_sfb;

    for pulse in 0..self.count {
      let offset = self.offsets.at(pulse)?;

      position = position.saturating_add(offset);

      match position {
        1024.. => return Ok(()),
        0..=1023 => {}
      }

      let Ok(from) = index(band.saturating_add(1));
      let Ok(passed) = fitted::<_, u32>(
        bands
          .iter()
          .skip(from)
          .take_while(|edge| **edge <= position)
          .count(),
      );

      band = band.saturating_add(passed);

      let _next_edge = bands.at(band.saturating_add(1))?;
      let scale = scales.cell((0, band))?;
      let amplitude = self.amplitudes.at(pulse)?;
      let line = coeffs.at(position)?;
      let Ok(restored) = Correction {
        line: SpectralLine(line),
        scale: Scale(scale),
        amplitude,
      }
      .restored();

      coeffs.put(position, restored)?;
    }

    Ok(())
  }
}
