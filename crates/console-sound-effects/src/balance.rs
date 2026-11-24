//! Attenuation relative to the PCM of the volume preview.
//!
//! Playback balance uses a shared gain for all channels in each interleaved
//! frame. Its state carries across writes without buffering the playback clock.

use std::num::NonZeroU32;

use console_core_never::Never;
use console_core_number_conversion::{index, narrow_f32, toward_zero_i32};

use crate::synthesis::{RATE, RenderError};
use crate::volume_samples;

const PCM16_SCALE: f64 = 32_768.0;
const LOUDNESS_WINDOW_SAMPLES: u32 = RATE * 2 / 5;
const PEAK_MARGIN: f64 = 0.5;
const POWER_MARGIN: f64 = 0.25;
const POWER_ATTACK_SECONDS: f64 = 0.010;
const RELEASE_SECONDS: f64 = 0.350;

struct Reference {
  peak: Amplitude,
  power: Power,
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "amplitude is a fraction of digital full scale"
  )
)]
struct Amplitude(f64);

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "power is the mean of squared normalized amplitudes"
  )
)]
struct Power(f64);

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "gain is the dimensionless multiplier applied to each sample"
  )
)]
struct Gain(f64);

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "response is the unitless interpolation fraction for one frame"
  )
)]
struct Response(f64);

impl Reference {
  fn measured(pcm: &[u8]) -> Result<Self, Never> {
    let (samples, _) = pcm.as_chunks::<2>();
    let Ok(window) = index(LOUDNESS_WINDOW_SAMPLES);
    let mut peak: f64 = 0.0;
    let mut power: f64 = 0.0;
    let mut sum = 0.0;
    let window_length = f64::from(LOUDNESS_WINDOW_SAMPLES);

    for (position, bytes) in samples.iter().enumerate() {
      let Ok(Amplitude(sample)) = amplitude(*bytes);
      peak = peak.max(sample.abs());
      sum += sample * sample;

      match position.checked_sub(window).and_then(|at| samples.get(at)) {
        Some(previous) => {
          let Ok(Amplitude(previous_2)) = amplitude(*previous);
          sum -= previous_2 * previous_2;
        }
        None => {}
      }

      power = power.max(sum / window_length);
    }

    Ok(Self {
      peak: Amplitude(peak),
      power: Power(power),
    })
  }
}

fn amplitude(bytes: [u8; 2]) -> Result<Amplitude, Never> {
  Ok(Amplitude(
    f64::from(i16::from_le_bytes(bytes)) / PCM16_SCALE,
  ))
}

fn finite(sample: Amplitude) -> Result<Amplitude, Never> {
  Ok(match sample.0.is_finite().then_some(sample) {
    Some(finite) => finite,
    None => Amplitude(0.0),
  })
}

pub struct PlaybackFormat {
  pub sample_rate_hz: u32,
  pub channels: u32,
}

pub struct Balance {
  channels: NonZeroU32,
  peak_limit: Amplitude,
  power_limit: Power,
  attack: Response,
  release: Response,
  power: Power,
  gain: Gain,
}

impl Balance {
  pub fn new(format: PlaybackFormat) -> Result<Self, RenderError> {
    let preview = volume_samples()?;
    let Ok(reference) = Reference::measured(&preview);
    let rate = f64::from(format.sample_rate_hz.max(1));
    let channels = match NonZeroU32::new(format.channels) {
      Some(count) => count,
      None => NonZeroU32::MIN,
    };

    Ok(Self {
      channels,
      peak_limit: Amplitude(reference.peak.0 * PEAK_MARGIN),
      power_limit: Power(reference.power.0 * POWER_MARGIN),
      attack: Response(-(-1.0 / (rate * POWER_ATTACK_SECONDS)).exp_m1()),
      release: Response(-(-1.0 / (rate * RELEASE_SECONDS)).exp_m1()),
      power: Power(0.0),
      gain: Gain(1.0),
    })
  }

  fn gain_for(&mut self, Amplitude(magnitude): Amplitude) -> Result<Gain, Never> {
    let power = magnitude * magnitude;
    let Response(response) = match (power > self.power.0).then_some(()) {
      Some(()) => self.attack,
      None => self.release,
    };
    self.power.0 += (power - self.power.0) * response;

    let peak_gain = match (magnitude > self.peak_limit.0).then_some(()) {
      Some(()) => self.peak_limit.0 / magnitude,
      None => 1.0,
    };
    let power_gain = match (self.power.0 > self.power_limit.0).then_some(()) {
      Some(()) => (self.power_limit.0 / self.power.0).sqrt(),
      None => 1.0,
    };
    let recovering = self.gain.0 + (1.0 - self.gain.0) * self.release.0;
    let gain = recovering.min(peak_gain).min(power_gain);
    self.gain = Gain(gain);

    Ok(Gain(gain))
  }

  pub fn apply(&mut self, samples: &mut [f32]) -> Result<(), Never> {
    let Ok(channels) = index(self.channels.get());

    for frame in samples.chunks_mut(channels.max(1)) {
      let magnitude = frame
        .iter()
        .map(|&sample| {
          let Ok(Amplitude(finite)) = finite(Amplitude(f64::from(sample)));

          finite.abs()
        })
        .sum();
      let Ok(Gain(gain)) = self.gain_for(Amplitude(magnitude));

      for sample in frame {
        let Ok(Amplitude(finite)) = finite(Amplitude(f64::from(*sample)));
        let Ok(heard) = narrow_f32(finite * gain);
        *sample = heard;
      }
    }

    Ok(())
  }

  pub(crate) fn apply_mono_pcm16(&mut self, pcm: &mut [u8]) -> Result<(), Never> {
    let (samples, _) = pcm.as_chunks_mut::<2>();

    for bytes in samples {
      let Ok(Amplitude(sample)) = amplitude(*bytes);
      let Ok(Gain(gain)) = self.gain_for(Amplitude(sample.abs()));
      let Ok(heard) = toward_zero_i32(sample * gain * PCM16_SCALE);
      let Ok(heard_2) = console_core_number_conversion::fitted::<_, i16>(heard);
      *bytes = heard_2.to_le_bytes();
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_system_filter_uses_the_rendered_bell() -> Result<(), Box<dyn std::error::Error>> {
    let settings = include_str!(
      "../../../files/etc/wireplumber/wireplumber.conf.d/60-console-balanced-playback.conf"
    );
    let preview = volume_samples()?;
    let Ok(reference) = Reference::measured(&preview);

    for (name, limit) in [
      ("sample-peak", reference.peak.0 * PEAK_MARGIN),
      ("sustained-level", (reference.power.0 * POWER_MARGIN).sqrt()),
    ] {
      let value = settings
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == name)
        .ok_or("the system filter is missing a bell measurement")?;
      let configured = value.1.trim().parse::<f64>()?;

      assert!(configured <= limit, "{name} is above the reference");
      assert!(limit - configured < 0.000_001, "{name} needs recalibration");
    }

    Ok(())
  }

  #[test]
  fn the_reference_is_audible_and_below_full_scale() -> Result<(), RenderError> {
    let preview = volume_samples()?;
    let Ok(reference) = Reference::measured(&preview);
    assert!(
      (0.10..0.20).contains(&reference.peak.0),
      "peak {}",
      reference.peak.0
    );
    assert!(
      (0.025..0.05).contains(&reference.power.0.sqrt()),
      "reference level {}",
      reference.power.0.sqrt()
    );
    Ok(())
  }

  #[test]
  fn initial_and_later_peaks_stay_below_the_reference() -> Result<(), RenderError> {
    let mut balance = Balance::new(PlaybackFormat {
      sample_rate_hz: RATE,
      channels: 2,
    })?;
    let mut initial = [1.0, -0.5];
    let Ok(()) = balance.apply(&mut initial);
    let [left, right] = initial;
    assert!(left > 0.0);
    assert!(f64::from(left - right) <= balance.peak_limit.0 + 1e-8);
    assert_eq!(left, -2.0 * right);

    let mut silence = vec![0.0; 48_000];
    let Ok(()) = balance.apply(&mut silence);
    assert!(
      silence
        .iter()
        .all(|sample| sample.classify() == std::num::FpCategory::Zero)
    );

    let mut burst = [-1.0, 1.0, f32::NAN, f32::INFINITY];
    let Ok(()) = balance.apply(&mut burst);
    let [left_burst, right_burst, invalid @ ..] = burst;
    assert!(f64::from(right_burst - left_burst) <= balance.peak_limit.0 + 1e-8);
    assert_eq!(invalid, [0.0, 0.0]);
    Ok(())
  }

  #[test]
  fn sustained_playback_keeps_headroom_and_quiet_playback_has_no_boost()
  -> Result<(), Box<dyn std::error::Error>> {
    let mut balance = Balance::new(PlaybackFormat {
      sample_rate_hz: 44_100,
      channels: 1,
    })?;
    let mut quiet = [0.001, -0.001];
    let Ok(()) = balance.apply(&mut quiet);
    assert_eq!(quiet, [0.001, -0.001]);

    let mut loud = vec![0.9; 44_100];
    let Ok(()) = balance.apply(&mut loud);
    let (last, _) = loud.split_last().ok_or("one second of samples")?;
    assert!(f64::from(*last).powi(2) <= balance.power_limit.0 + 1e-8);

    let Ok(()) = balance.apply(&mut quiet);
    assert!(quiet.iter().all(|sample| sample.abs() <= 0.001));
    Ok(())
  }
}
