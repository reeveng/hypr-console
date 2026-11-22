//! One channel's individual channel stream (4.4.2.4): the window it is cut
//! with, how its bands are coded, their scalefactors and then their lines.
//!
//! What a channel keeps from one packet to the next is only the tail of its
//! last window, which the next one overlaps, and the shape that window had.

use core::cmp::Ordering;

use console_core_audio_decoding_interfaces::errors::{Result, decode_error, unsupported_error};
use console_core_byte_streams::{FlagBit, ReadBitsLtr};
use console_core_number_conversion::fitted;

use crate::lane::{Fields, Grid, Constraint, Lane, Sure, invalid};
use crate::pulse::Pulse;
use crate::spectrum::{BandType, Scale, decode_band};
use crate::subbands::{Bands, Lcg, Scales, Spectrum, Subbands, Windows};
use crate::synthesis::Dsp;
use crate::tables::{INTENSITY_SCALE_MIN, NORMAL_SCALE_MIN, Tables};
use crate::tns::Tns;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowSequence {
  OnlyLong,
  LongStart,
  EightShort,
  LongStop,
}

impl WindowSequence {
  fn from_bits(bits: u32) -> Sure<Self> {
    Ok(match bits & 3 {
      0 => WindowSequence::OnlyLong,
      1 => WindowSequence::LongStart,
      2 => WindowSequence::EightShort,
      _ => WindowSequence::LongStop,
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowShape {
  Sine,
  KaiserBessel,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Window {
  Own,
  Common,
}

#[derive(Clone, Copy)]
pub(crate) enum Grouping {
  NewGroup,
  SameGroup,
}

impl From<FlagBit> for Grouping {
  fn from(flag: FlagBit) -> Self {
    match flag {
      FlagBit::Zero => Self::NewGroup,
      FlagBit::One => Self::SameGroup,
    }
  }
}

#[derive(Clone)]
pub(crate) struct IcsInfo {
  pub(crate) window_sequence: WindowSequence,
  pub(crate) window_shape: WindowShape,
  pub(crate) prev_window_shape: WindowShape,
  pub(crate) scale_factor_grouping: Windows<Grouping>,
  group_starts: Windows<u32>,
  pub(crate) window_groups: u32,
  pub(crate) max_sfb: u32,
}

impl IcsInfo {
  pub(crate) fn new() -> Sure<Self> {
    Ok(Self {
      window_sequence: WindowSequence::OnlyLong,
      window_shape: WindowShape::Sine,
      prev_window_shape: WindowShape::Sine,
      scale_factor_grouping: [Grouping::NewGroup; 8],
      group_starts: [0; 8],
      window_groups: 0,
      max_sfb: 0,
    })
  }

  pub(crate) fn pick<T>(&self, long: T, short: T) -> Sure<T> {
    Ok(match self.window_sequence {
      WindowSequence::EightShort => short,
      WindowSequence::OnlyLong | WindowSequence::LongStart | WindowSequence::LongStop => long,
    })
  }

  pub(crate) fn num_windows(&self) -> Sure<u32> {
    self.pick(1, 8)
  }

  pub(crate) fn decode<B: ReadBitsLtr>(&mut self, bs: &mut B) -> Result<()> {
    self.prev_window_shape = self.window_shape;

    let reserved = bs.read_flag()?;

    match reserved {
      FlagBit::One => return decode_error("aac: ics reserved bit set"),
      FlagBit::Zero => {}
    }

    let bits = bs.read_bits_leq32(2)?;
    let Ok(sequence) = WindowSequence::from_bits(bits);
    let kaiser_bessel = bs.read_flag()?;

    self.window_sequence = sequence;
    self.window_shape = match kaiser_bessel {
      FlagBit::One => WindowShape::KaiserBessel,
      FlagBit::Zero => WindowShape::Sine,
    };
    self.window_groups = 1;

    match sequence {
      WindowSequence::EightShort => {
        let max_sfb = bs.read_bits_leq32(4)?;

        self.max_sfb = max_sfb;
        self.read_grouping(bs)
      }
      WindowSequence::OnlyLong | WindowSequence::LongStart | WindowSequence::LongStop => {
        let max_sfb = bs.read_bits_leq32(6)?;

        self.max_sfb = max_sfb;

        let predictor_data_present = bs.read_flag()?;

        match predictor_data_present {
          FlagBit::One => unsupported_error("aac: predictor data"),
          FlagBit::Zero => Ok(()),
        }
      }
    }
  }

  fn read_grouping<B: ReadBitsLtr>(&mut self, bs: &mut B) -> Result<()> {
    for window in 0..7 {
      let grouping = bs.read_flag().map(Grouping::from)?;

      self.scale_factor_grouping.put(window, grouping)?;

      match grouping {
        Grouping::SameGroup => {}
        Grouping::NewGroup => {
          self
            .group_starts
            .put(self.window_groups, window.saturating_add(1))?;
          self.window_groups = self.window_groups.saturating_add(1);
        }
      }
    }

    Ok(())
  }

  pub(crate) fn copy_from_common(&mut self, other: &IcsInfo) -> Sure<()> {
    let prev_window_shape = self.window_shape;

    *self = other.clone();
    self.prev_window_shape = prev_window_shape;

    Ok(())
  }

  fn group_start(&self, group: u32) -> Result<u32> {
    match group {
      0 => Ok(0),
      1.. => match group.cmp(&self.window_groups) {
        Ordering::Equal | Ordering::Greater => {
          let Ok(end) = self.pick(1, 8);

          Ok(end)
        }
        Ordering::Less => self.group_starts.at(group),
      },
    }
  }
}

#[derive(Clone, Copy)]
struct Running {
  normal: i32,
  noise: i32,
  intensity: i32,
  noise_started: NoiseStart,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NoiseStart {
  Pending,
  Started,
}

impl Running {
  fn new(global_gain: u8) -> Sure<Self> {
    let gain = i32::from(global_gain);

    Ok(Self {
      normal: gain,
      noise: gain.saturating_sub(90).saturating_sub(NORMAL_SCALE_MIN),
      intensity: INTENSITY_SCALE_MIN.saturating_neg(),
      noise_started: NoiseStart::Pending,
    })
  }

  fn scale_of<B: ReadBitsLtr>(
    &mut self,
    bs: &mut B,
    kind: BandType,
    tables: &Tables,
  ) -> Result<f32> {
    match kind {
      BandType::Zero => Ok(0.0),
      BandType::Intensity | BandType::IntensityOutOfPhase => {
        let step = scale_step(bs, tables)?;

        self.intensity = self.intensity.saturating_add(step);

        scale_at(&tables.intensity_scales, self.intensity)
      }
      BandType::Noise => {
        let step = self.noise_step(bs, tables)?;

        self.noise = self.noise.saturating_add(step);

        scale_at(&tables.normal_scales, self.noise)
      }
      BandType::SignedQuads(_)
      | BandType::UnsignedQuads(_)
      | BandType::SignedPairs(_)
      | BandType::UnsignedPairs(_)
      | BandType::Escape => {
        let step = scale_step(bs, tables)?;

        self.normal = self.normal.saturating_add(step);

        scale_at(&tables.normal_scales, self.normal)
      }
    }
  }

  fn noise_step<B: ReadBitsLtr>(&mut self, bs: &mut B, tables: &Tables) -> Result<i32> {
    match self.noise_started {
      NoiseStart::Pending => {
        self.noise_started = NoiseStart::Started;

        let bits = bs.read_bits_leq32(9)?;
        let Ok(raw) = fitted::<u32, i32>(bits);

        Ok(raw.saturating_sub(256))
      }
      NoiseStart::Started => scale_step(bs, tables),
    }
  }
}

fn scale_step<B: ReadBitsLtr>(bs: &mut B, tables: &Tables) -> Result<i32> {
  let (word, _) = bs.read_codebook(&tables.codebooks.scale_factors)?;

  Ok(i32::from(word).saturating_sub(60))
}

fn scale_at(table: &[f32], value: i32) -> Result<f32> {
  match u32::try_from(value) {
    Ok(at) => table.at(at),
    Err(_negative) => invalid(),
  }
}

#[derive(Clone)]
pub(crate) struct Ics {
  global_gain: u8,
  pub(crate) info: IcsInfo,
  pulse: Option<Pulse>,
  tns: Option<Tns>,
  band_types: Windows<Bands<BandType>>,
  pub(crate) scales: Scales,
  subbands: Subbands,
  pub(crate) coeffs: Spectrum,
  delay: Spectrum,
}

impl Ics {
  pub(crate) fn new(subbands: Subbands) -> Sure<Self> {
    let Ok(info) = IcsInfo::new();

    Ok(Self {
      global_gain: 0,
      info,
      pulse: None,
      tns: None,
      band_types: [[BandType::Zero; 64]; 8],
      scales: [[0.0; 64]; 8],
      subbands,
      coeffs: [0.0; 1024],
      delay: [0.0; 1024],
    })
  }

  pub(crate) fn reset(&mut self) -> Sure<()> {
    let Ok(info) = IcsInfo::new();

    self.info = info;
    self.delay = [0.0; 1024];

    Ok(())
  }

  pub(crate) fn band_type(&self, band: (u32, u32)) -> Result<BandType> {
    self.band_types.cell(band)
  }

  pub(crate) fn bands(&self) -> Sure<&'static [u32]> {
    self.info.pick(self.subbands.long, self.subbands.short)
  }

  fn decode_section_data<B: ReadBitsLtr>(&mut self, bs: &mut B) -> Result<()> {
    for group in 0..self.info.window_groups {
      self.read_group_sections(bs, group)?;
    }

    Ok(())
  }

  fn read_group_sections<B: ReadBitsLtr>(&mut self, bs: &mut B, group: u32) -> Result<()> {
    let Ok(width) = self.info.pick(5, 3);
    let max_sfb = self.info.max_sfb;
    let mut covered = 0u32;

    for _ in 0..max_sfb {
      match covered.cmp(&max_sfb) {
        Ordering::Equal | Ordering::Greater => break,
        Ordering::Less => {}
      }

      let bits = bs.read_bits_leq32(4)?;
      let kind = BandType::from_bits(bits)?;
      let length = section_length(bs, width)?;
      let end = covered.saturating_add(length);

      Constraint::from(end <= max_sfb).require()?;

      for sfb in covered..end {
        self.band_types.put_cell((group, sfb), kind)?;
      }

      covered = end;
    }

    Constraint::from(covered >= max_sfb).require()
  }

  fn decode_scale_factor_data<B: ReadBitsLtr>(
    &mut self,
    bs: &mut B,
    tables: &Tables,
  ) -> Result<()> {
    let Ok(mut running) = Running::new(self.global_gain);

    for group in 0..self.info.window_groups {
      for sfb in 0..self.info.max_sfb {
        let kind = self.band_type((group, sfb))?;
        let scale = running.scale_of(bs, kind, tables)?;

        self.scales.put_cell((group, sfb), scale)?;
      }
    }

    Ok(())
  }

  fn decode_spectrum<B: ReadBitsLtr>(
    &mut self,
    bs: &mut B,
    lcg: &mut Lcg,
    tables: &Tables,
  ) -> Result<()> {
    self.coeffs.fill(0.0);

    let Ok(bands) = self.bands();

    for group in 0..self.info.window_groups {
      let first = self.info.group_start(group)?;
      let next = self.info.group_start(group.saturating_add(1))?;

      for sfb in 0..self.info.max_sfb {
        let start = bands.at(sfb)?;
        let end = bands.at(sfb.saturating_add(1))?;
        let kind = self.band_type((group, sfb))?;
        let scale = self.scales.cell((group, sfb))?;

        for window in first..next {
          let offset = window.saturating_mul(128);
          let span = start.saturating_add(offset)..end.saturating_add(offset);
          let lines = self.coeffs.between_mut(span)?;

          decode_band(bs, lcg, tables, kind, Scale(scale), lines)?;
        }
      }
    }

    Ok(())
  }

  pub(crate) fn decode<B: ReadBitsLtr>(
    &mut self,
    bs: &mut B,
    lcg: &mut Lcg,
    tables: &Tables,
    window: Window,
  ) -> Result<()> {
    let gain = bs.read_bits_leq32(8)?;
    let Ok(gain_2) = fitted::<u32, u8>(gain);

    self.global_gain = gain_2;

    match window {
      Window::Own => self.info.decode(bs)?,
      Window::Common => {}
    }

    self.decode_section_data(bs)?;
    self.decode_scale_factor_data(bs, tables)?;
    let pulse = Pulse::read(bs)?;

    self.pulse = pulse;

    match (&self.pulse, self.info.window_sequence) {
      (Some(_), WindowSequence::EightShort) => return invalid(),
      (None, WindowSequence::EightShort)
      | (
        Some(_) | None,
        WindowSequence::OnlyLong | WindowSequence::LongStart | WindowSequence::LongStop,
      ) => {}
    }

    let tns = Tns::read(bs, &self.info)?;

    self.tns = tns;

    let gain_control_data_present = bs.read_flag()?;

    match gain_control_data_present {
      FlagBit::One => return invalid(),
      FlagBit::Zero => {}
    }

    self.decode_spectrum(bs, lcg, tables)
  }

  pub(crate) fn synth_channel(&mut self, dsp: &mut Dsp, dst: &mut [f32]) -> Result<()> {
    let Ok(bands) = self.bands();

    match &self.pulse {
      Some(pulse) => pulse.synth(bands, &self.scales, &mut self.coeffs)?,
      None => {}
    }

    match &self.tns {
      Some(tns) => tns.synth(&self.info, bands, self.subbands.rate, &mut self.coeffs)?,
      None => {}
    }

    dsp.synth(&self.coeffs, &mut self.delay, &self.info, dst)
  }
}

fn section_length<B: ReadBitsLtr>(bs: &mut B, width: u32) -> Result<u32> {
  let escape = 1u32.wrapping_shl(width).saturating_sub(1);
  let mut length = 0u32;
  let Ok(fields) = Fields::of(bs, width);

  for read in fields {
    let increment = read?;

    length = length.saturating_add(increment);

    match increment.cmp(&escape) {
      Ordering::Less => return Ok(length),
      Ordering::Equal | Ordering::Greater => {}
    }
  }

  invalid()
}
