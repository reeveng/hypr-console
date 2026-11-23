//! Joint stereo: mid-side and intensity coding, undone.
//!
//! A joint stereo frame codes each scale factor band one of three ways. Below
//! the intensity bound a band is left and right, or, when mid-side coding is
//! on, a sum and a difference that become left and right again; above it, the
//! second channel is silent and says only, as a position on a scale, how the
//! first is shared between the two. The bound is found from the top down, as
//! the first band whose second channel is not silent.

use std::ops::Range;

use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_number_conversion::{fitted, index};

use super::computed::Computed;
use super::frame::{BlockType, Granule};
use crate::common::{
  Bands as ScalefactorBands, ChannelMode, FrameHeader, Infallible, Switch, Version,
};

const INVALID_POSITION_MPEG1: u8 = 7;

const INVALID_POSITION_MPEG2: u8 = 31;

type Spectrum = [f32; 576];

#[derive(Copy, Clone, PartialEq, Eq)]
enum Band {
  Silent,
  Sounding,
}

struct Intensity<'a> {
  table: &'a [(f32, f32)],
  invalid: u8,
  mid_side: Switch,
  positions: [u8; 39],
}

struct Pair<'a> {
  left: &'a mut Spectrum,
  right: &'a mut Spectrum,
}

fn mid_side(pair: &mut Pair<'_>, range: &Range<u32>) -> Infallible<()> {
  let Ok(start) = index(range.start);
  let Ok(end) = index(range.end);

  for (mid, side) in pair
    .left
    .get_mut(start..end)
    .into_iter()
    .zip(pair.right.get_mut(start..end))
  {
    for (m, s) in mid.iter_mut().zip(side) {
      let left = (*m + *s) * std::f32::consts::FRAC_1_SQRT_2;
      let right = (*m - *s) * std::f32::consts::FRAC_1_SQRT_2;

      *m = left;
      *s = right;
    }
  }

  Ok(())
}

fn silence(right: &Spectrum, range: &Range<u32>) -> Infallible<Band> {
  let Ok(start) = index(range.start);
  let Ok(end) = index(range.end);

  Ok(match right.get(start..end) {
    Some(band) => {
      match ::core::primitive::bool::then_some(band.iter().all(|x| x.abs().to_bits() == 0), ()) {
        ::core::option::Option::Some(()) => Band::Silent,
        ::core::option::Option::None => Band::Sounding,
      }
    }
    None => Band::Silent,
  })
}

fn positions(source: impl Iterator<Item = u8>) -> Infallible<[u8; 39]> {
  let mut positions = [0u8; 39];

  for (slot, value) in positions.iter_mut().zip(source) {
    *slot = value;
  }

  Ok(positions)
}

impl Intensity<'_> {
  fn share(&self, at: u32, pair: &mut Pair<'_>, range: &Range<u32>) -> Result<()> {
    let Ok(position) = index(at);

    let is_pos = match self.positions.get(position) {
      Some(&is_pos) => is_pos,
      None => return decode_error("mpa: intensity position out of range"),
    };

    let Ok(table_position) = index(is_pos);

    match (
      is_pos < self.invalid,
      self.table.get(table_position),
      self.mid_side,
    ) {
      (true, Some(&(ratio_l, ratio_r)), _) => {
        let Ok(start) = index(range.start);
        let Ok(end) = index(range.end);

        for (left, right) in pair
          .left
          .get_mut(start..end)
          .into_iter()
          .zip(pair.right.get_mut(start..end))
        {
          for (l, r) in left.iter_mut().zip(right) {
            let is = *l;

            *l = ratio_l * is;
            *r = ratio_r * is;
          }
        }

        Ok(())
      }
      (_, _, Switch::On) => {
        let Ok(()) = mid_side(pair, range);

        Ok(())
      }
      (_, _, Switch::Off) => Ok(()),
    }
  }

  fn window(
    &self,
    state: Band,
    at: u32,
    pair: &mut Pair<'_>,
    range: &Range<u32>,
  ) -> Result<Band> {
    let Ok(now) = match state {
      Band::Sounding => Ok(Band::Sounding),
      Band::Silent => silence(pair.right, range),
    };

    match now {
      Band::Silent => self.share(at, pair, range)?,
      Band::Sounding => match self.mid_side {
        Switch::On => {
          let Ok(()) = mid_side(pair, range);
        }
        Switch::Off => {}
      },
    }

    Ok(now)
  }

  fn long_block(
    &self,
    bands: &[u32; 23],
    [rzero, end]: [u32; 2],
    pair: &mut Pair<'_>,
  ) -> Result<u32> {
    let edges = bands.iter().zip(bands.iter().skip(1)).zip(0..22u32).rev();
    let mut bound = end;

    for ((&start, &stop), at) in edges {
      let range = start..stop;

      let Ok(quiet) = silence(pair.right, &range);

      match (start >= rzero, quiet) {
        (true, _) | (false, Band::Silent) => self.share(at, pair, &range)?,
        (false, Band::Sounding) => break,
      }

      bound = start;
    }

    Ok(bound)
  }

  fn short_block(
    &self,
    bands: &Bands,
    short: &[u32],
    first: u32,
    pair: &mut Pair<'_>,
  ) -> Result<u32> {
    let mut sfi = first;

    let triples = short
      .iter()
      .zip(short.iter().skip(1))
      .zip(short.iter().skip(2))
      .zip(short.iter().skip(3))
      .step_by(3)
      .rev();

    let (mut bound, mut found) = (bands.end, false);
    let (mut w0, mut w1, mut w2) = (Band::Silent, Band::Silent, Band::Silent);

    for (((&s0, &s1), &s2), &s3) in triples {
      let third = self.window(w2, sfi.saturating_sub(1), pair, &(s2..s3))?;

      sfi = sfi.saturating_sub(1);

      let second = self.window(w1, sfi.saturating_sub(1), pair, &(s1..s2))?;

      sfi = sfi.saturating_sub(1);

      let first_2 = self.window(w0, sfi.saturating_sub(1), pair, &(s0..s1))?;

      sfi = sfi.saturating_sub(1);

      (w0, w1, w2) = (first_2, second, third);

      bound = s0;

      found = (w0, w1, w2) == (Band::Sounding, Band::Sounding, Band::Sounding);

      match ::core::primitive::bool::then_some(found, ()) {
        ::core::option::Option::Some(()) => break,
        ::core::option::Option::None => {}
      }
    }

    match (found, bands.long) {
      (false, Some(long)) => {
        let edges = long.iter().zip(long.iter().skip(1)).rev();

        for (&start, &stop) in edges {
          let range = start..stop;
          let Ok(quiet) = silence(pair.right, &range);

          match quiet {
            Band::Silent => self.share(sfi.saturating_sub(1), pair, &range)?,
            Band::Sounding => break,
          }

          sfi = sfi.saturating_sub(1);
          bound = start;
        }

        Ok(bound)
      }
      (_, _) => Ok(bound),
    }
  }
}

struct Bands<'a> {
  long: Option<&'a [u32]>,
  end: u32,
}

pub fn stereo(
  header: &FrameHeader,
  scale_bands: &ScalefactorBands,
  granule: &mut Granule,
  spectra: &mut [Spectrum; 2],
  computed: &Computed,
) -> Result<()> {
  let (mid_side_coding, intensity_coding) = match header.channel_mode {
    ChannelMode::JointStereo {
      mid_side,
      intensity,
    } => (mid_side, intensity),
    ChannelMode::Mono | ChannelMode::DualMono | ChannelMode::Stereo => return Ok(()),
  };

  let [first, second] = &mut granule.channels;

  match ::core::primitive::bool::then_some(first.block_type == second.block_type, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      return decode_error("mpa: stereo channel pair block_type mismatch");
    }
  }

  let end = first.rzero.max(second.rzero);

  let [left, right] = spectra;
  let mut pair = Pair { left, right };

  let bound = match intensity_coding {
    Switch::Off => end,
    Switch::On => {
      let Ok([even, odd]) = computed.intensity_mpeg2();
      let Ok(mpeg1) = computed.intensity_mpeg1();

      let (table, invalid): (&[(f32, f32)], u8) =
        match (header.version, second.scalefac_compress & 1) {
          (Version::Mpeg1, _) => (mpeg1, INVALID_POSITION_MPEG1),
          (Version::Mpeg2 | Version::Mpeg2p5, 0) => (even, INVALID_POSITION_MPEG2),
          (Version::Mpeg2 | Version::Mpeg2p5, _) => (odd, INVALID_POSITION_MPEG2),
        };

      let scalefacs = second.scalefacs.iter().copied();

      match second.block_type {
        BlockType::Short => {
          let Ok(positions) =
            positions(scalefacs.clone().take(36).chain(scalefacs.skip(33).take(3)));
          let intensity = Intensity {
            table,
            invalid,
            mid_side: mid_side_coding,
            positions,
          };
          let bands = Bands { long: None, end };

          intensity.short_block(&bands, scale_bands.short, 39, &mut pair)?
        }
        BlockType::Mixed => {
          let Ok(positions) =
            positions(scalefacs.clone().take(36).chain(scalefacs.skip(33).take(3)));
          let intensity = Intensity {
            table,
            invalid,
            mid_side: mid_side_coding,
            positions,
          };
          let switch = scale_bands.mixed_switch;
          let Ok(switch_at) = index(switch);
          let Ok(plus_one) = index(switch.saturating_add(1));
          let Ok(mixed_len) = fitted::<_, u32>(scale_bands.mixed.len());
          let first_position = mixed_len.saturating_sub(1);
          let bands = Bands {
            long: scale_bands.mixed.get(..plus_one),
            end,
          };

          match scale_bands.mixed.get(switch_at..) {
            Some(short) => intensity.short_block(&bands, short, first_position, &mut pair)?,
            None => end,
          }
        }
        BlockType::Long | BlockType::Start | BlockType::End => {
          let Ok(positions) =
            positions(scalefacs.clone().take(21).chain(scalefacs.skip(20).take(1)));
          let intensity = Intensity {
            table,
            invalid,
            mid_side: mid_side_coding,
            positions,
          };

          intensity.long_block(scale_bands.long, [second.rzero, end], &mut pair)?
        }
      }
    }
  };

  match (mid_side_coding, bound > 0) {
    (Switch::On, true) => {
      let Ok(()) = mid_side(&mut pair, &(0..bound));
    }
    (_, _) => {}
  }

  match (intensity_coding, mid_side_coding) {
    (Switch::Off, Switch::Off) => {}
    (_, _) => {
      first.rzero = end;
      second.rzero = end;
    }
  }

  Ok(())
}
