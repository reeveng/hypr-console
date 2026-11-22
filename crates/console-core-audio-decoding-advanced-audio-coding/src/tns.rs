//! Temporal noise shaping (4.6.9): an all-pole filter run along the spectrum so
//! the quantisation noise of a transient follows its envelope in time.
//!
//! The filters are sent as reflection coefficients and turned into predictor
//! coefficients here, by the recursion the standard gives, before they are
//! applied to the lines of the window they belong to.

use std::f32::consts;

use console_core_audio_decoding_interfaces::errors::Result;
use console_core_byte_streams::{FlagBit, ReadBitsLtr};
use console_core_number_conversion::fitted;

use crate::ics::IcsInfo;
use crate::lane::{Constraint, Grid, Lane, Sure};
use crate::subbands::{Spectrum, Windows};

const TNS_MAX_LONG_BANDS: [u32; 12] = [31, 31, 34, 40, 42, 51, 46, 46, 42, 42, 42, 39];

const TNS_MAX_SHORT_BANDS: [u32; 12] = [9, 9, 10, 14, 14, 14, 14, 14, 14, 14, 14, 14];

type Coefficients = [f32; 21];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
  Upward,
  Downward,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Resolution {
  Coarse,
  Fine,
}

#[derive(Copy, Clone)]
struct Filter {
  length: u32,
  order: u32,
  direction: Direction,
  coef: Coefficients,
}

impl Filter {
  fn empty() -> Sure<Self> {
    Ok(Self {
      length: 0,
      order: 0,
      direction: Direction::Upward,
      coef: [0.0; 21],
    })
  }

  fn read<B: ReadBitsLtr>(bs: &mut B, info: &IcsInfo, resolution: Resolution) -> Result<Self> {
    let Ok(length_bits) = info.pick(6, 4);
    let Ok(order_bits) = info.pick(5, 3);
    let Ok(max_order) = info.pick(12, 7);
    let length = bs.read_bits_leq32(length_bits)?;
    let order = bs.read_bits_leq32(order_bits)?;

    Constraint::from(order <= max_order).require()?;

    match order {
      0 => {
        let Ok(empty) = Self::empty();

        Ok(Self { length, ..empty })
      }
      _ => Self::read_coefficients(bs, Extent { length, order }, resolution),
    }
  }

  fn read_coefficients<B: ReadBitsLtr>(
    bs: &mut B,
    extent: Extent,
    resolution: Resolution,
  ) -> Result<Self> {
    let downward = bs.read_flag()?;
    let direction = match downward {
      FlagBit::One => Direction::Downward,
      FlagBit::Zero => Direction::Upward,
    };
    let compress = bs.read_flag()?;
    let (resolution_bits, factor_base) = match resolution {
      Resolution::Fine => (4u32, 8u8),
      Resolution::Coarse => (3, 4),
    };
    let bits = match compress {
      FlagBit::One => resolution_bits.saturating_sub(1),
      FlagBit::Zero => resolution_bits,
    };
    let reflection = Reflection {
      bits,
      order: extent.order,
      factor_base,
    }
    .read(bs)?;
    let coef = lpc(&reflection, extent.order)?;

    Ok(Self {
      length: extent.length,
      order: extent.order,
      direction,
      coef,
    })
  }
}

struct Extent {
  length: u32,
  order: u32,
}

struct Reflection {
  bits: u32,
  order: u32,
  factor_base: u8,
}

impl Reflection {
  fn read<B: ReadBitsLtr>(&self, bs: &mut B) -> Result<[f32; 20]> {
    let sign_mask = 1u8.wrapping_shl(self.bits.saturating_sub(1));
    let negative_mask = !(1u8.wrapping_shl(self.bits).saturating_sub(1));
    let base = f32::from(self.factor_base);
    let positive_factor = (base - 0.5) / consts::FRAC_PI_2;
    let negative_factor = (base + 0.5) / consts::FRAC_PI_2;
    let mut reflection = [0.0; 20];

    let filled = reflection.between_mut(0..self.order)?;

    for slot in filled.iter_mut() {
      let raw = bs.read_bits_leq32(self.bits)?;
      let Ok(raw_2) = fitted::<u32, u8>(raw);
      let extended = match raw_2 & sign_mask {
        0 => raw_2,
        _ => raw_2 | negative_mask,
      };
      let signed = i8::from_le_bytes([extended]);
      let coefficient = f32::from(signed);

      *slot = match signed {
        0.. => coefficient / positive_factor,
        ..=-1 => coefficient / negative_factor,
      }
      .sin();
    }

    Ok(reflection)
  }
}

fn lpc(reflection: &[f32; 20], order: u32) -> Result<Coefficients> {
  let mut coef = [0.0; 21];
  let mut scratch = [0.0; 21];

  for stage in 1..=order {
    let last = stage.saturating_sub(1);
    let latest = reflection.at(last)?;

    for i in 1..stage {
      let earlier = coef.at(i.saturating_sub(1))?;
      let mirrored = coef.at(stage.saturating_sub(i).saturating_sub(1))?;

      scratch.put(i, earlier + latest * mirrored)?;
    }

    for j in 0..last {
      let moved = scratch.at(j.saturating_add(1))?;

      coef.put(j, moved)?;
    }

    coef.put(last, latest)?;
  }

  Ok(coef)
}

#[derive(Copy, Clone)]
pub(crate) struct Tns {
  filter_counts: Windows<u32>,
  filters: Windows<[Filter; 4]>,
}

impl Tns {
  pub(crate) fn read<B: ReadBitsLtr>(bs: &mut B, info: &IcsInfo) -> Result<Option<Self>> {
    let present = bs.read_flag()?;

    match present {
      FlagBit::Zero => Ok(None),
      FlagBit::One => {
        let tns = Self::read_filters(bs, info)?;

        Ok(Some(tns))
      }
    }
  }

  fn read_filters<B: ReadBitsLtr>(bs: &mut B, info: &IcsInfo) -> Result<Self> {
    let Ok(count_bits) = info.pick(2, 1);
    let Ok(windows) = info.num_windows();
    let Ok(empty) = Filter::empty();
    let mut filter_counts = [0; 8];
    let mut filters = [[empty; 4]; 8];

    for window in 0..windows {
      let count = bs.read_bits_leq32(count_bits)?;
      let resolution = match count {
        0 => Resolution::Coarse,
        _ => {
          let fine = bs.read_flag()?;

          match fine {
            FlagBit::One => Resolution::Fine,
            FlagBit::Zero => Resolution::Coarse,
          }
        }
      };

      filter_counts.put(window, count)?;

      for slot in 0..count {
        let filter = Filter::read(bs, info, resolution)?;

        filters.put_cell((window, slot), filter)?;
      }
    }

    Ok(Self {
      filter_counts,
      filters,
    })
  }

  pub(crate) fn synth(
    &self,
    info: &IcsInfo,
    bands: &[u32],
    rate: u32,
    coeffs: &mut Spectrum,
  ) -> Result<()> {
    let Ok(table) = info.pick(&TNS_MAX_LONG_BANDS, &TNS_MAX_SHORT_BANDS);
    let ceiling = table.at(rate)?;
    let limit = ceiling.min(info.max_sfb);
    let Ok(windows) = info.num_windows();

    for window in 0..windows {
      let mut bottom = Self::edge_count(bands)?;

      let count = self.filter_counts.at(window)?;

      for slot in 0..count {
        let top = bottom;
        let filter = self.filters.cell((window, slot))?;

        bottom = top.saturating_sub(filter.length);

        match filter.order {
          0 => {}
          _ => {
            let base = window.saturating_mul(128);
            let low = bands.at(bottom.min(limit))?;
            let high = bands.at(top.min(limit))?;

            filter.apply(base.saturating_add(low)..base.saturating_add(high), coeffs)?;
          }
        }
      }
    }

    Ok(())
  }

  fn edge_count(bands: &[u32]) -> Result<u32> {
    let Ok(count) = fitted::<_, u32>(bands.len());

    Ok(count.saturating_sub(1))
  }
}

impl Filter {
  fn apply(&self, span: std::ops::Range<u32>, coeffs: &mut Spectrum) -> Result<()> {
    match self.direction {
      Direction::Upward => self.filter_upward(span, coeffs),
      Direction::Downward => self.filter_downward(span, coeffs),
    }
  }

  fn filter_upward(&self, span: std::ops::Range<u32>, coeffs: &mut Spectrum) -> Result<()> {
    for (done, line) in (0u32..).zip(span) {
      for tap in 0..self.order.min(done) {
        let earlier = coeffs.at(line.saturating_sub(tap).saturating_sub(1))?;
        let predictor = self.coef.at(tap)?;
        let current = coeffs.at(line)?;

        coeffs.put(line, current - earlier * predictor)?;
      }
    }

    Ok(())
  }

  fn filter_downward(&self, span: std::ops::Range<u32>, coeffs: &mut Spectrum) -> Result<()> {
    for (done, line) in (0u32..).zip(span.rev()) {
      for tap in 0..self.order.min(done) {
        let later = coeffs.at(line.saturating_add(tap).saturating_add(1))?;
        let predictor = self.coef.at(tap)?;
        let current = coeffs.at(line)?;

        coeffs.put(line, current - later * predictor)?;
      }
    }

    Ok(())
  }
}
