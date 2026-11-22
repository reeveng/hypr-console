//! Attenuation relative to the PCM of the volume preview.

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
  peak: f64,
  power: f64,
}

impl Reference {
  fn measured(pcm: &[u8]) -> Self {
    let (samples, _) = pcm.as_chunks::<2>();
    let Ok(window) = index(LOUDNESS_WINDOW_SAMPLES);
    let mut peak: f64 = 0.0;
    let mut power: f64 = 0.0;
    let mut sum = 0.0;
    let window_length = f64::from(LOUDNESS_WINDOW_SAMPLES);

    for (position, bytes) in samples.iter().enumerate() {
      let sample = amplitude(*bytes);
      peak = peak.max(sample.abs());
      sum += sample * sample;

      match position.checked_sub(window).and_then(|at| samples.get(at)) {
        Some(previous) => {
          let previous = amplitude(*previous);
          sum -= previous * previous;
        }
        None => {}
      }

      power = power.max(sum / window_length);
    }

    Self { peak, power }
  }
}

fn amplitude(bytes: [u8; 2]) -> f64 {
  f64::from(i16::from_le_bytes(bytes)) / PCM16_SCALE
}

fn finite(sample: f32) -> f32 {
  match sample.is_finite() {
    true => sample,
    false => 0.0,
  }
}

/// Processes each interleaved frame with a gain shared by all its channels.
/// State carries across writes and adds no buffering to the playback clock.
pub struct Balance {
  channels: usize,
  peak_limit: f64,
  power_limit: f64,
  attack: f64,
  release: f64,
  power: f64,
  gain: f64,
}

impl Balance {
  pub fn new(rate: u32, channels: u32) -> Result<Self, RenderError> {
    let reference = Reference::measured(&volume_samples()?);
    let rate = f64::from(rate.max(1));
    let Ok(channels) = index(channels.max(1));

    Ok(Self {
      channels,
      peak_limit: reference.peak * PEAK_MARGIN,
      power_limit: reference.power * POWER_MARGIN,
      attack: -(-1.0 / (rate * POWER_ATTACK_SECONDS)).exp_m1(),
      release: -(-1.0 / (rate * RELEASE_SECONDS)).exp_m1(),
      power: 0.0,
      gain: 1.0,
    })
  }

  fn gain_for(&mut self, magnitude: f64) -> f64 {
    let power = magnitude * magnitude;
    let response = match power > self.power {
      true => self.attack,
      false => self.release,
    };
    self.power += (power - self.power) * response;

    let peak_gain = match magnitude > self.peak_limit {
      true => self.peak_limit / magnitude,
      false => 1.0,
    };
    let power_gain = match self.power > self.power_limit {
      true => (self.power_limit / self.power).sqrt(),
      false => 1.0,
    };
    let recovering = self.gain + (1.0 - self.gain) * self.release;

    self.gain = recovering.min(peak_gain).min(power_gain);

    self.gain
  }

  pub fn apply(&mut self, samples: &mut [f32]) -> Result<(), Never> {
    for frame in samples.chunks_mut(self.channels) {
      // The sum bounds any unity-weight downmix and keeps stereo gain linked.
      let magnitude = frame
        .iter()
        .map(|&sample| f64::from(finite(sample).abs()))
        .sum();
      let gain = self.gain_for(magnitude);

      for sample in frame {
        let Ok(heard) = narrow_f32(f64::from(finite(*sample)) * gain);
        *sample = heard;
      }
    }

    Ok(())
  }

  pub(crate) fn apply_mono_pcm16(&mut self, pcm: &mut [u8]) -> Result<(), Never> {
    let (samples, _) = pcm.as_chunks_mut::<2>();

    for bytes in samples {
      let sample = amplitude(*bytes);
      let gain = self.gain_for(sample.abs());
      let Ok(heard) = toward_zero_i32(sample * gain * PCM16_SCALE);
      let Ok(heard) = console_core_number_conversion::fitted::<_, i16>(heard);
      *bytes = heard.to_le_bytes();
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
    let reference = Reference::measured(&volume_samples()?);

    for (name, limit) in [
      ("sample-peak", reference.peak * PEAK_MARGIN),
      ("sustained-level", (reference.power * POWER_MARGIN).sqrt()),
    ] {
      let configured = settings
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == name)
        .ok_or("the system filter is missing a bell measurement")?
        .1
        .trim()
        .parse::<f64>()?;

      assert!(configured <= limit, "{name} is above the reference");
      assert!(limit - configured < 0.000_001, "{name} needs recalibration");
    }

    Ok(())
  }

  #[test]
  fn the_reference_is_audible_and_below_full_scale() -> Result<(), RenderError> {
    let reference = Reference::measured(&volume_samples()?);
    assert!(
      (0.10..0.20).contains(&reference.peak),
      "peak {}",
      reference.peak
    );
    assert!(
      (0.025..0.05).contains(&reference.power.sqrt()),
      "reference level {}",
      reference.power.sqrt()
    );
    Ok(())
  }

  #[test]
  fn initial_and_later_peaks_stay_below_the_reference() -> Result<(), RenderError> {
    let mut balance = Balance::new(RATE, 2)?;
    let mut initial = [1.0, -0.5];
    let Ok(()) = balance.apply(&mut initial);
    assert!(initial[0] > 0.0);
    assert!(f64::from(initial[0] - initial[1]) <= balance.peak_limit + 1e-8);
    assert_eq!(initial[0], -2.0 * initial[1]);

    let mut silence = vec![0.0; 48_000];
    let Ok(()) = balance.apply(&mut silence);
    assert!(silence.iter().all(|&sample| sample == 0.0));

    let mut burst = [-1.0, 1.0, f32::NAN, f32::INFINITY];
    let Ok(()) = balance.apply(&mut burst);
    assert!(f64::from(burst[1] - burst[0]) <= balance.peak_limit + 1e-8);
    assert_eq!(&burst[2..], &[0.0, 0.0]);
    Ok(())
  }

  #[test]
  fn sustained_playback_keeps_headroom_and_quiet_playback_has_no_boost() -> Result<(), RenderError>
  {
    let mut balance = Balance::new(44_100, 1)?;
    let mut quiet = [0.001, -0.001];
    let Ok(()) = balance.apply(&mut quiet);
    assert_eq!(quiet, [0.001, -0.001]);

    let mut loud = vec![0.9; 44_100];
    let Ok(()) = balance.apply(&mut loud);
    let (last, _) = loud.split_last().expect("one second of samples");
    assert!(f64::from(*last).powi(2) <= balance.power_limit + 1e-8);

    let Ok(()) = balance.apply(&mut quiet);
    assert!(quiet.iter().all(|sample| sample.abs() <= 0.001));
    Ok(())
  }
}
