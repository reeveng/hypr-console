//! A single channel element or a channel pair (4.4.2.1, 4.4.2.2) and the joint
//! stereo that ties the two channels of a pair together.
//!
//! A pair either codes its channels as two streams with a window each, or shares
//! one window description across both and says per band whether the channels
//! were sent as left and right or as their sum and difference. A band of the
//! second channel may also be no spectrum at all but the first channel's,
//! scaled and possibly inverted.

use console_core_audio_decoding_interfaces::audio::{AudioBuffer, Signal};
use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_byte_streams::{FlagBit, ReadBitsLtr};

use crate::ics::{Grouping, Ics, Window};
use crate::lane::Lane;
use crate::lane::{Grid, Sure};
use crate::spectrum::BandType;
use crate::subbands::{Bands, Lcg, Subbands, Windows};
use crate::synthesis::Dsp;
use crate::tables::Tables;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layout {
  Single,
  Pair,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MidSide {
  Never,
  PerBand,
  Always,
}

#[derive(Clone, Copy)]
enum BandCoding {
  LeftRight,
  MidSide,
}

impl From<FlagBit> for BandCoding {
  fn from(flag: FlagBit) -> Self {
    match flag {
      FlagBit::Zero => Self::LeftRight,
      FlagBit::One => Self::MidSide,
    }
  }
}

#[derive(Clone, Copy)]
enum Role {
  Intensity(Phase),
  Noise,
  Plain,
}

#[derive(Clone, Copy)]
enum Phase {
  Together,
  Inverted,
}

fn role_of(kind: BandType) -> Sure<Role> {
  Ok(match kind {
    BandType::Intensity => Role::Intensity(Phase::Together),
    BandType::IntensityOutOfPhase => Role::Intensity(Phase::Inverted),
    BandType::Noise => Role::Noise,
    BandType::Zero
    | BandType::SignedQuads(_)
    | BandType::UnsignedQuads(_)
    | BandType::SignedPairs(_)
    | BandType::UnsignedPairs(_)
    | BandType::Escape => Role::Plain,
  })
}

#[derive(Clone)]
pub(crate) struct ChannelPair {
  pub(crate) layout: Layout,
  pub(crate) channel: u32,
  mid_side: MidSide,
  band_coding: Windows<Bands<BandCoding>>,
  ics0: Ics,
  ics1: Ics,
  lcg: Lcg,
}

impl ChannelPair {
  pub(crate) fn new(layout: Layout, channel: u32, subbands: Subbands) -> Sure<Self> {
    let Ok(ics0) = Ics::new(subbands);
    let Ok(ics1) = Ics::new(subbands);
    let Ok(lcg) = Lcg::new(0x1f2e3d4c);

    Ok(Self {
      layout,
      channel,
      mid_side: MidSide::Never,
      band_coding: [[BandCoding::LeftRight; 64]; 8],
      ics0,
      ics1,
      lcg,
    })
  }

  pub(crate) fn reset(&mut self) -> Sure<()> {
    let Ok(()) = self.ics0.reset();
    let Ok(()) = self.ics1.reset();

    Ok(())
  }

  pub(crate) fn decode_ga_sce<B: ReadBitsLtr>(
    &mut self,
    bs: &mut B,
    tables: &Tables,
  ) -> Result<()> {
    self.ics0.decode(bs, &mut self.lcg, tables, Window::Own)
  }

  pub(crate) fn decode_ga_cpe<B: ReadBitsLtr>(
    &mut self,
    bs: &mut B,
    tables: &Tables,
  ) -> Result<()> {
    let common = bs.read_flag()?;
    let window = match common {
      FlagBit::One => Window::Common,
      FlagBit::Zero => Window::Own,
    };

    match window {
      Window::Common => self.read_common(bs)?,
      Window::Own => {}
    }

    self.ics0.decode(bs, &mut self.lcg, tables, window)?;
    self.ics1.decode(bs, &mut self.lcg, tables, window)?;

    match window {
      Window::Common => self.joint_stereo(),
      Window::Own => Ok(()),
    }
  }

  fn read_common<B: ReadBitsLtr>(&mut self, bs: &mut B) -> Result<()> {
    self.ics0.info.decode(bs)?;

    let mask = bs.read_bits_leq32(2)?;

    self.mid_side = match mask {
      0 => MidSide::Never,
      1 => MidSide::PerBand,
      2 => MidSide::Always,
      _ => return decode_error("aac: invalid mid-side mask"),
    };

    for group in 0..self.ics0.info.window_groups {
      for sfb in 0..self.ics0.info.max_sfb {
        let coding = match self.mid_side {
          MidSide::Never => Ok(BandCoding::LeftRight),
          MidSide::Always => Ok(BandCoding::MidSide),
          MidSide::PerBand => bs.read_flag().map(BandCoding::from),
        }?;

        self.band_coding.put_cell((group, sfb), coding)?;
      }
    }

    let Ok(()) = self.ics1.info.copy_from_common(&self.ics0.info);

    Ok(())
  }

  fn joint_stereo(&mut self) -> Result<()> {
    let Ok(bands) = self.ics0.bands();
    let Ok(windows) = self.ics0.info.num_windows();
    let mut group = 0u32;

    for window in 0..windows {
      let grouping = match window {
        0 => Ok(Grouping::SameGroup),
        1.. => self
          .ics0
          .info
          .scale_factor_grouping
          .at(window.saturating_sub(1)),
      }?;

      match grouping {
        Grouping::NewGroup => group = group.saturating_add(1),
        Grouping::SameGroup => {}
      }

      for sfb in 0..self.ics0.info.max_sfb {
        let base = window.saturating_mul(128);
        let low = bands.at(sfb)?;
        let high = bands.at(sfb.saturating_add(1))?;

        self.stereo_band(
          (group, sfb),
          base.saturating_add(low)..base.saturating_add(high),
        )?;
      }
    }

    Ok(())
  }

  fn stereo_band(&mut self, band: (u32, u32), span: std::ops::Range<u32>) -> Result<()> {
    let kind1 = self.ics1.band_type(band)?;
    let kind0 = self.ics0.band_type(band)?;
    let Ok(role1) = role_of(kind1);
    let Ok(role0) = role_of(kind0);

    match role1 {
      Role::Intensity(phase) => self.intensity(band, phase, span),
      Role::Noise => Ok(()),
      Role::Plain => match role0 {
        Role::Noise => Ok(()),
        Role::Intensity(_) | Role::Plain => self.mid_side(band, span),
      },
    }
  }

  fn intensity(
    &mut self,
    band: (u32, u32),
    phase: Phase,
    span: std::ops::Range<u32>,
  ) -> Result<()> {
    let coding = self.band_coding.cell(band)?;
    let direction = match phase {
      Phase::Together => 1.0,
      Phase::Inverted => -1.0,
    };
    let factor = match (self.mid_side, coding) {
      (MidSide::PerBand, BandCoding::MidSide) => -1.0,
      (MidSide::PerBand, BandCoding::LeftRight)
      | (MidSide::Never | MidSide::Always, BandCoding::LeftRight | BandCoding::MidSide) => 1.0,
    };
    let band_scale = self.ics1.scales.cell(band)?;
    let scale = direction * factor * band_scale;
    let left = self.ics0.coeffs.between(span.clone())?;
    let right = self.ics1.coeffs.between_mut(span)?;

    for (l, r) in left.iter().zip(right) {
      *r = scale * *l;
    }

    Ok(())
  }

  fn mid_side(&mut self, band: (u32, u32), span: std::ops::Range<u32>) -> Result<()> {
    let coding = self.band_coding.cell(band)?;

    match coding {
      BandCoding::LeftRight => Ok(()),
      BandCoding::MidSide => {
        let mid = self.ics0.coeffs.between_mut(span.clone())?;
        let side = self.ics1.coeffs.between_mut(span)?;

        for (m, s) in mid.iter_mut().zip(side) {
          let difference = *m - *s;

          *m += *s;
          *s = difference;
        }

        Ok(())
      }
    }
  }

  pub(crate) fn synth_audio(&mut self, dsp: &mut Dsp, abuf: &mut AudioBuffer<f32>) -> Result<()> {
    let Ok(dst) = abuf.chan_mut(self.channel);

    self.ics0.synth_channel(dsp, dst)?;

    match self.layout {
      Layout::Single => Ok(()),
      Layout::Pair => {
        let Ok(second) = abuf.chan_mut(self.channel.saturating_add(1));

        self.ics1.synth_channel(dsp, second)
      }
    }
  }
}
