//! Pulse cache and bit allocation (RFC 6716 sections 4.3.3 and 4.3.4.1).
//!
//! The section states the allocation in 32-bit signed integers and in eighths of a bit, and
//! every quantity it computes stays within a frame's 1275 bytes shifted by a few bits, so the
//! arithmetic here wraps where the reference decoder's would and is never asked to. A
//! division is by a band's width or by a count of degrees of freedom, which the mode's tables
//! never make zero; where a division could only be by zero in a table that is not this one,
//! the quotient is zero. Both searches over the allocation table and the walk that gives the
//! highest bands back are bounded by the bands they search.

use std::cmp::Ordering;

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use super::mdct::Imdct;
use super::tables::{ALLOC, BANDS, EDGES, LOG2_FRAC_TABLE, MAX_LM, Place, entry};
use crate::range::{BITRES, Bit, RangeDecoder, significant_bits};

const MAX_PSEUDO: i32 = 40;
const LOG_MAX_PSEUDO: u32 = 6;
pub(crate) const MAX_FINE_BITS: i32 = 8;
const FINE_OFFSET: i32 = 21;
pub(crate) const QTHETA_OFFSET: i32 = 4;
pub(crate) const QTHETA_OFFSET_TWOPHASE: i32 = 16;
const ALLOC_STEPS: u32 = 6;
const ONE_BIT: i32 = 1 << BITRES;
const QUALITIES: u32 = 11;
const CACHE_ROWS: u32 = MAX_LM + 2;
const PVQ_CAP: u128 = u128::MAX >> 8;

pub(crate) fn edge(band: u32) -> Result<i32, Never> {
  let Ok(at) = index(band);

  Ok(match EDGES.get(at) {
    Some(edge) => *edge,
    None => 0,
  })
}

pub(crate) fn width(band: u32) -> Result<i32, Never> {
  let Ok(low) = edge(band);
  let Ok(high) = edge(band.wrapping_add(1));

  Ok(high.wrapping_sub(low))
}

pub(crate) fn of(values: &[i32; 21], band: u32) -> Result<i32, Never> {
  let Ok(at) = index(band);

  Ok(match values.get(at) {
    Some(value) => *value,
    None => 0,
  })
}

pub(crate) fn set(values: &mut [i32; 21], band: u32, value: i32) -> Result<(), Never> {
  let Ok(at) = index(band);

  match values.get_mut(at) {
    Some(slot) => *slot = value,
    None => {}
  }

  Ok(())
}

fn quality(level: i32, band: u32) -> Result<i32, Never> {
  let Ok(row) = fitted::<i32, u32>(level);

  entry(&ALLOC, Place { row, column: band })
}

fn quotient(
  crate::quantities::Numerator(numerator): crate::quantities::Numerator,
  crate::quantities::Denominator(denominator): crate::quantities::Denominator,
) -> Result<i32, Never> {
  Ok(match numerator.checked_div(denominator) {
    Some(quotient) => quotient,
    None => 0,
  })
}

pub(crate) fn get_pulses(step: i32) -> Result<i32, Never> {
  Ok(match step.cmp(&8) {
    Ordering::Less => step,
    Ordering::Equal | Ordering::Greater => (step & 7)
      .wrapping_add(8)
      .wrapping_shl(step.wrapping_shr(3).wrapping_sub(1).cast_unsigned()),
  })
}

pub(crate) fn log2_frac(
  crate::quantities::LogValue(value): crate::quantities::LogValue,
  crate::quantities::FractionalBits(frac): crate::quantities::FractionalBits,
) -> Result<i32, Never> {
  let Ok(bits) = significant_bits(value);
  let Ok(length) = fitted::<u32, i32>(bits);

  match value & value.wrapping_sub(1) {
    0 => Ok(length.wrapping_sub(1).wrapping_shl(frac)),
    _not_a_power => {
      let mut held = match bits.cmp(&16) {
        Ordering::Greater => value
          .wrapping_sub(1)
          .wrapping_shr(bits.wrapping_sub(16))
          .wrapping_add(1),
        Ordering::Less | Ordering::Equal => value.wrapping_shl(16u32.wrapping_sub(bits)),
      };
      let mut log = length.wrapping_sub(1).wrapping_shl(frac);

      for shift in (0..=frac).rev() {
        let carry = held.wrapping_shr(16);
        let Ok(signed) = fitted::<u32, i32>(carry);

        log = log.wrapping_add(signed.wrapping_shl(shift));
        held = held.wrapping_add(carry).wrapping_shr(carry);
        held = held
          .wrapping_mul(held)
          .wrapping_add(0x7FFF)
          .wrapping_shr(15);
      }

      Ok(match held.cmp(&0x8000) {
        Ordering::Greater => log.wrapping_add(1),
        Ordering::Less | Ordering::Equal => log,
      })
    }
  }
}

pub(crate) fn pvq_v(
  crate::quantities::VectorSize(dimensions): crate::quantities::VectorSize,
  crate::quantities::PulseCount(pulses): crate::quantities::PulseCount,
) -> Result<u128, Never> {
  match (pulses, dimensions) {
    (0, _) => Ok(1),
    (_, 0) => Ok(0),
    (_, _) => {
      let Ok(length) = index(pulses);
      let mut row = vec![0u128; length.min(128)];

      for _dimension in 0..dimensions {
        let mut left = 1u128;
        let mut diagonal = 1u128;

        for slot in &mut row {
          let up = *slot;

          *slot = up
            .saturating_add(left)
            .saturating_add(diagonal)
            .min(PVQ_CAP);
          diagonal = up;
          left = *slot;
        }
      }

      Ok(match row.last() {
        Some(count) => *count,
        None => 1,
      })
    }
  }
}

pub(crate) struct Mode {
  pub(crate) log_n: [i32; 21],
  cache_index: Vec<Option<u32>>,
  cache_bits: Vec<u8>,
  caps: Vec<u8>,
  pub(crate) window: [f32; 120],
  pub(crate) mdct: [Imdct; 4],
}

fn transforms() -> Result<[Imdct; 4], Never> {
  let sizes = [1920u32, 960, 480, 240].map(|size| match NonZeroU32::new(size) {
    Some(size_2) => size_2,
    None => NonZeroU32::MIN,
  });

  Ok(sizes.map(|size| {
    let Ok(transform) = Imdct::new(size);

    transform
  }))
}

impl Mode {
  pub(crate) fn new() -> Result<Mode, Never> {
    let mut log_n = [0i32; 21];

    for (band, slot) in (0u32..).zip(log_n.iter_mut()) {
      let Ok(span) = width(band);
      let Ok(log) = log2_frac(
        crate::quantities::LogValue(span.cast_unsigned()),
        crate::quantities::FractionalBits(BITRES),
      );

      *slot = log;
    }

    let Ok((cache_index, cache_bits)) = compute_pulse_cache();
    let Ok(mdct) = transforms();
    let mut mode = Mode {
      log_n,
      cache_index,
      cache_bits,
      caps: Vec::new(),
      window: [0.0; 120],
      mdct,
    };
    let Ok(caps) = compute_caps(&mode);

    mode.caps = caps;

    let Ok(()) = console_core_audio_windows::vorbis_half(&mut mode.window);

    Ok(mode)
  }

  pub(crate) fn cache(&self, lm: i32, band: u32) -> Result<&[u8], Never> {
    let row = lm
      .wrapping_add(1)
      .cast_unsigned()
      .wrapping_mul(BANDS)
      .wrapping_add(band);
    let Ok(at) = index(row);
    let start = match self.cache_index.get(at) {
      Some(Some(start)) => *start,
      Some(None) | None => 0,
    };
    let Ok(from) = index(start);

    Ok(match self.cache_bits.get(from..) {
      Some(cache) => cache,
      None => &[],
    })
  }

  pub(crate) fn bits2pulses(
    &self,
    band: u32,
    crate::quantities::BlockExponent(lm): crate::quantities::BlockExponent,
    crate::quantities::BitBudget(bits): crate::quantities::BitBudget,
  ) -> Result<i32, Never> {
    let Ok(cache) = self.cache(lm, band);
    let Ok(top) = cached(cache, 0);
    let mut low = 0i32;
    let mut high = top;
    let wanted = bits.wrapping_sub(1);

    for _step in 0..LOG_MAX_PSEUDO {
      let middle = low.wrapping_add(high).wrapping_add(1).wrapping_shr(1);
      let Ok(cost) = cached(cache, middle);

      match cost.cmp(&wanted) {
        Ordering::Equal | Ordering::Greater => high = middle,
        Ordering::Less => low = middle,
      }
    }

    let low_bits = match low {
      0 => -1,
      _some => {
        let Ok(cost) = cached(cache, low);

        cost
      }
    };
    let Ok(high_bits) = cached(cache, high);

    Ok(
      match wanted
        .wrapping_sub(low_bits)
        .cmp(&high_bits.wrapping_sub(wanted))
      {
        Ordering::Less | Ordering::Equal => low,
        Ordering::Greater => high,
      },
    )
  }

  pub(crate) fn pulses2bits(
    &self,
    band: u32,
    crate::quantities::BlockExponent(lm): crate::quantities::BlockExponent,
    crate::quantities::PulseIndex(pulses): crate::quantities::PulseIndex,
  ) -> Result<i32, Never> {
    match pulses {
      0 => Ok(0),
      _some => {
        let Ok(cache) = self.cache(lm, band);
        let Ok(cost) = cached(cache, pulses);

        Ok(cost.wrapping_add(1))
      }
    }
  }

  pub(crate) fn init_caps(
    &self,
    crate::quantities::BlockOrder(lm): crate::quantities::BlockOrder,
    crate::quantities::ChannelCount(channels): crate::quantities::ChannelCount,
  ) -> Result<[i32; 21], Never> {
    let mut cap = [0i32; 21];
    let row = lm
      .wrapping_mul(2)
      .wrapping_add(channels)
      .wrapping_sub(1)
      .wrapping_mul(BANDS);
    let Ok(from) = index(row);
    let Ok(wide) = fitted::<u32, i32>(channels);

    for ((band, slot), held) in (0u32..)
      .zip(cap.iter_mut())
      .zip(self.caps.iter().skip(from))
    {
      let Ok(span) = width(band);
      let size = span.wrapping_shl(lm);

      *slot = i32::from(*held)
        .wrapping_add(64)
        .wrapping_mul(wide)
        .wrapping_mul(size)
        .wrapping_shr(2);
    }

    Ok(cap)
  }
}

fn cached(cache: &[u8], place: i32) -> Result<i32, Never> {
  let Ok(at) = index(place);

  Ok(match cache.get(at) {
    Some(bits) => i32::from(*bits),
    None => 0,
  })
}

fn half_width(
  crate::quantities::BandIndex(band): crate::quantities::BandIndex,
  crate::quantities::CacheRow(row): crate::quantities::CacheRow,
) -> Result<u32, Never> {
  let Ok(span) = width(band);

  Ok(span.wrapping_shl(row).wrapping_shr(1).cast_unsigned())
}

fn pseudo_pulses(size: u32) -> Result<i32, Never> {
  let mut most = 0i32;

  for step in 0..MAX_PSEUDO {
    let Ok(pulses) = get_pulses(step.wrapping_add(1));
    let Ok(count) = pvq_v(
      crate::quantities::VectorSize(size),
      crate::quantities::PulseCount(pulses.cast_unsigned()),
    );

    match count.cmp(&u128::from(u32::MAX)) {
      Ordering::Less | Ordering::Equal => most = step.wrapping_add(1),
      Ordering::Greater => break,
    }
  }

  Ok(most)
}

fn compute_pulse_cache() -> Result<(Vec<Option<u32>>, Vec<u8>), Never> {
  let mut index_of: Vec<Option<u32>> = Vec::new();
  let mut first: BTreeMap<u32, Option<u32>> = BTreeMap::new();
  let mut entries = Vec::new();
  let mut next = 0u32;

  for row in 0..CACHE_ROWS {
    for band in 0..BANDS {
      let Ok(size) = half_width(
        crate::quantities::BandIndex(band),
        crate::quantities::CacheRow(row),
      );

      let entry = match (first.get(&size), size) {
        (Some(shared), _) => *shared,
        (None, 0) => None,
        (None, _) => {
          let Ok(most) = pseudo_pulses(size);
          let start = next;

          entries.push((size, most, start));
          next = next.wrapping_add(most.wrapping_add(1).cast_unsigned());

          Some(start)
        }
      };

      first.entry(size).or_insert(entry);
      index_of.push(entry);
    }
  }

  let Ok(length) = index(next);
  let mut bits = vec![0u8; length.min(const { 5 * 21 * 41 })];

  for (size, most, start) in entries {
    let Ok(()) = fill_cache(
      &mut bits,
      crate::quantities::VectorSize(size),
      most,
      crate::quantities::CacheOffset(start),
    );
  }

  Ok((index_of, bits))
}

fn fill_cache(
  bits: &mut [u8],
  crate::quantities::VectorSize(size): crate::quantities::VectorSize,
  most: i32,
  crate::quantities::CacheOffset(start): crate::quantities::CacheOffset,
) -> Result<(), Never> {
  let Ok(largest) = get_pulses(most);
  let Ok(required) = required_bits(
    crate::quantities::VectorSize(size),
    crate::quantities::MaxPulseCount(largest.cast_unsigned()),
  );
  let Ok(from) = index(start);
  let Ok(count) = index(most.wrapping_add(1));

  for (step, slot) in (0i32..).zip(bits.iter_mut().skip(from).take(count)) {
    let value = match step {
      0 => most,
      _some => {
        let Ok(pulses) = get_pulses(step);
        let Ok(at) = index(pulses);
        let cost = match required.get(at) {
          Some(cost) => *cost,
          None => 0,
        };

        cost.wrapping_sub(1)
      }
    };
    let Ok(byte) = fitted::<i32, u8>(value);

    *slot = byte;
  }

  Ok(())
}

fn required_bits(
  crate::quantities::VectorSize(size): crate::quantities::VectorSize,
  crate::quantities::MaxPulseCount(largest): crate::quantities::MaxPulseCount,
) -> Result<Vec<i32>, Never> {
  let mut out = vec![0i32];

  for pulses in 1..=largest {
    let bits = match size {
      1 => ONE_BIT,
      _more => {
        let Ok(count) = pvq_v(
          crate::quantities::VectorSize(size),
          crate::quantities::PulseCount(pulses),
        );
        let Ok(narrow) = fitted::<u128, u32>(count);
        let Ok(log) = log2_frac(
          crate::quantities::LogValue(narrow),
          crate::quantities::FractionalBits(BITRES),
        );

        log
      }
    };

    out.push(bits);
  }

  Ok(out)
}

#[derive(Clone, Copy)]
struct Shape {
  lm: i32,
  channels: i32,
  band: u32,
}

fn split_bits(
  mode: &Mode,
  shape: Shape,
  crate::quantities::BandWidth(size): crate::quantities::BandWidth,
  crate::quantities::BlockExponent(start_lm): crate::quantities::BlockExponent,
) -> Result<(i32, i32), Never> {
  let Ok(cache) = mode.cache(start_lm, shape.band);
  let Ok(top) = cached(cache, 0);
  let Ok(most) = cached(cache, top);
  let Ok(log_n) = of(&mode.log_n, shape.band);
  let mut bits = most.wrapping_add(1);
  let mut size_2 = size;

  for step in 0..shape.lm.wrapping_sub(start_lm) {
    bits = bits.wrapping_shl(1);

    let offset = log_n
      .wrapping_add(start_lm.wrapping_add(step).wrapping_shl(BITRES))
      .wrapping_shr(1)
      .wrapping_sub(QTHETA_OFFSET);
    let degrees = size_2.wrapping_mul(2).wrapping_sub(1);
    let numerator = degrees
      .wrapping_mul(offset)
      .wrapping_add(bits)
      .wrapping_mul(459);
    let denominator = degrees.wrapping_shl(9).wrapping_sub(459);
    let Ok(spent) = quotient(
      crate::quantities::Numerator(numerator.wrapping_add(denominator.wrapping_shr(1))),
      crate::quantities::Denominator(denominator),
    );

    bits = bits.wrapping_add(spent.min(57));
    size_2 = size_2.wrapping_shl(1);
  }

  Ok((bits, size_2))
}

fn stereo_bits(
  crate::quantities::LogDimensions(log_n): crate::quantities::LogDimensions,
  shape: Shape,
  crate::quantities::BitBudget(bits): crate::quantities::BitBudget,
  crate::quantities::BandWidth(size): crate::quantities::BandWidth,
) -> Result<i32, Never> {
  let doubled = bits.wrapping_shl(1);
  let (offset_by, removed, weight, ceiling) = match size {
    2 => (QTHETA_OFFSET_TWOPHASE, 1, 512, 64),
    _other => (QTHETA_OFFSET, 0, 487, 61),
  };
  let offset = log_n
    .wrapping_add(shape.lm.wrapping_shl(BITRES))
    .wrapping_shr(1)
    .wrapping_sub(offset_by);
  let degrees = size.wrapping_mul(2).wrapping_sub(1).wrapping_sub(removed);
  let numerator = degrees
    .wrapping_mul(offset)
    .wrapping_add(doubled)
    .wrapping_mul(weight);
  let denominator = degrees.wrapping_shl(9).wrapping_sub(weight);
  let Ok(spent) = quotient(
    crate::quantities::Numerator(numerator.wrapping_add(denominator.wrapping_shr(1))),
    crate::quantities::Denominator(denominator),
  );

  Ok(doubled.wrapping_add(spent.min(ceiling)))
}

fn max_bits(mode: &Mode, shape: Shape) -> Result<i32, Never> {
  let Ok(span) = width(shape.band);

  match span.wrapping_shl(shape.lm.cast_unsigned()) {
    1 => Ok(
      shape
        .channels
        .wrapping_mul(1i32.wrapping_add(MAX_FINE_BITS))
        .wrapping_shl(BITRES),
    ),
    _wider => {
      let (start_lm, size) = match (span.cmp(&2), span.cmp(&1)) {
        (Ordering::Greater, _) => (-1, span.wrapping_shr(1)),
        (Ordering::Less | Ordering::Equal, Ordering::Less | Ordering::Equal) => {
          let start_lm = shape.lm.min(1);

          (start_lm, span.wrapping_shl(start_lm.cast_unsigned()))
        }
        (Ordering::Less | Ordering::Equal, Ordering::Greater) => (0, span),
      };
      let Ok((bits, size_2)) = split_bits(
        mode,
        shape,
        crate::quantities::BandWidth(size),
        crate::quantities::BlockExponent(start_lm),
      );
      let Ok(log_n) = of(&mode.log_n, shape.band);
      let bits_2 = match shape.channels {
        2 => {
          let Ok(coupled) = stereo_bits(
            crate::quantities::LogDimensions(log_n),
            shape,
            crate::quantities::BitBudget(bits),
            crate::quantities::BandWidth(size_2),
          );

          coupled
        }
        _mono => bits,
      };
      let extra = match (shape.channels, size_2.cmp(&2)) {
        (2, Ordering::Greater) => 1,
        (_, _) => 0,
      };
      let degrees = shape.channels.wrapping_mul(size_2).wrapping_add(extra);
      let mut offset = log_n
        .wrapping_add(shape.lm.wrapping_shl(BITRES))
        .wrapping_shr(1)
        .wrapping_sub(FINE_OFFSET);

      match size_2 {
        2 => offset = offset.wrapping_add(ONE_BIT.wrapping_shr(2)),
        _other => {}
      }

      let numerator = degrees.wrapping_mul(offset).wrapping_add(bits_2);
      let denominator = degrees.wrapping_sub(1).wrapping_shl(BITRES);
      let Ok(fine) = quotient(
        crate::quantities::Numerator(numerator.wrapping_add(denominator.wrapping_shr(1))),
        crate::quantities::Denominator(denominator),
      );

      Ok(
        bits_2.wrapping_add(
          shape
            .channels
            .wrapping_mul(fine.min(MAX_FINE_BITS))
            .wrapping_shl(BITRES),
        ),
      )
    }
  }
}

fn compute_caps(mode: &Mode) -> Result<Vec<u8>, Never> {
  let mut caps = Vec::new();

  for lm in 0..=MAX_LM {
    for channels in 1..=2u32 {
      for band in 0..BANDS {
        let Ok(lm_2) = fitted::<u32, i32>(lm);
        let Ok(channels_2) = fitted::<u32, i32>(channels);
        let shape = Shape {
          lm: lm_2,
          channels: channels_2,
          band,
        };
        let Ok(bits) = max_bits(mode, shape);
        let Ok(span) = width(band);
        let wide = channels_2.wrapping_mul(span.wrapping_shl(lm_2.cast_unsigned()));
        let Ok(share) = quotient(
          crate::quantities::Numerator(bits.wrapping_mul(4)),
          crate::quantities::Denominator(wide),
        );
        let Ok(byte) = fitted::<i32, u8>(share.wrapping_sub(64));

        caps.push(byte);
      }
    }
  }

  Ok(caps)
}

pub(crate) struct Allocation {
  pub(crate) coded_bands: u32,
  pub(crate) intensity: u32,
  pub(crate) dual_stereo: Bit,
  pub(crate) balance: i32,
  pub(crate) pulses: [i32; 21],
  pub(crate) fine_quant: [i32; 21],
  pub(crate) fine_priority: [i32; 21],
}

pub(crate) struct Request<'a> {
  pub(crate) start: u32,
  pub(crate) end: u32,
  pub(crate) offsets: &'a [i32; 21],
  pub(crate) cap: &'a [i32; 21],
  pub(crate) alloc_trim: i32,
  pub(crate) total: i32,
  pub(crate) channels: u32,
  pub(crate) lm: u32,
}

struct Reserve {
  total: i32,
  skip: i32,
  intensity: i32,
  dual_stereo: i32,
}

struct Limits {
  thresh: [i32; 21],
  trim_offset: [i32; 21],
}

struct Interpolated {
  skip_start: u32,
  bits1: [i32; 21],
  bits2: [i32; 21],
}

fn reserved(request: &Request<'_>) -> Result<Reserve, Never> {
  let mut total = request.total.max(0);
  let skip = match total.cmp(&ONE_BIT) {
    Ordering::Equal | Ordering::Greater => ONE_BIT,
    Ordering::Less => 0,
  };

  total = total.wrapping_sub(skip);

  let (intensity, dual_stereo) = match request.channels {
    2 => {
      let Ok(at) = index(request.end.wrapping_sub(request.start));
      let wanted = match LOG2_FRAC_TABLE.get(at) {
        Some(bits) => *bits,
        None => 0,
      };

      match wanted.cmp(&total) {
        Ordering::Greater => (0, 0),
        Ordering::Less | Ordering::Equal => {
          total = total.wrapping_sub(wanted);

          let dual = match total.cmp(&ONE_BIT) {
            Ordering::Equal | Ordering::Greater => ONE_BIT,
            Ordering::Less => 0,
          };

          total = total.wrapping_sub(dual);

          (wanted, dual)
        }
      }
    }
    _mono => (0, 0),
  };

  Ok(Reserve {
    total,
    skip,
    intensity,
    dual_stereo,
  })
}

fn limits(request: &Request<'_>) -> Result<Limits, Never> {
  let Ok(wide) = fitted::<u32, i32>(request.channels);
  let Ok(lm) = fitted::<u32, i32>(request.lm);
  let mut thresh = [0i32; 21];
  let mut trim_offset = [0i32; 21];

  for band in request.start..request.end {
    let Ok(span) = width(band);
    let Ok(above) = fitted::<u32, i32>(request.end.wrapping_sub(band).wrapping_sub(1));
    let floor = wide.wrapping_shl(BITRES);
    let threshold = span
      .wrapping_mul(3)
      .wrapping_shl(request.lm)
      .wrapping_shl(BITRES)
      .wrapping_shr(4);
    let scale = 1i32.wrapping_shl(request.lm.wrapping_add(BITRES));
    let trim = wide
      .wrapping_mul(span)
      .wrapping_mul(request.alloc_trim.wrapping_sub(5).wrapping_sub(lm))
      .wrapping_mul(above)
      .wrapping_mul(scale)
      .wrapping_shr(6);
    let trim_2 = match span.wrapping_shl(request.lm) {
      1 => trim.wrapping_sub(floor),
      _wider => trim,
    };
    let Ok(()) = set(&mut thresh, band, floor.max(threshold));
    let Ok(()) = set(&mut trim_offset, band, trim_2);
  }

  Ok(Limits {
    thresh,
    trim_offset,
  })
}

fn band_bits(request: &Request<'_>, limits: &Limits, row: i32, band: u32) -> Result<i32, Never> {
  let Ok(wide) = fitted::<u32, i32>(request.channels);
  let Ok(span) = width(band);
  let Ok(bits) = quality(row, band);
  let Ok(trim) = of(&limits.trim_offset, band);
  let raw = wide
    .wrapping_mul(span)
    .wrapping_mul(bits)
    .wrapping_shl(request.lm)
    .wrapping_shr(2);

  Ok(match raw.cmp(&0) {
    Ordering::Greater => raw.wrapping_add(trim).max(0),
    Ordering::Less | Ordering::Equal => raw,
  })
}

#[derive(Clone, Copy)]
enum BandThreshold {
  Pending,
  Reached,
}

fn quality_sum(request: &Request<'_>, limits: &Limits, row: i32) -> Result<i32, Never> {
  let Ok(wide) = fitted::<u32, i32>(request.channels);
  let floor = wide.wrapping_shl(BITRES);
  let mut threshold = BandThreshold::Pending;
  let mut sum = 0i32;

  for band in (request.start..request.end).rev() {
    let Ok(bits) = band_bits(request, limits, row, band);
    let Ok(offset) = of(request.offsets, band);
    let Ok(thresh) = of(&limits.thresh, band);
    let Ok(cap) = of(request.cap, band);
    let bits_2 = bits.wrapping_add(offset);

    match (threshold, bits_2.cmp(&thresh), bits_2.cmp(&floor)) {
      (BandThreshold::Reached, _, _) | (_, Ordering::Equal | Ordering::Greater, _) => {
        threshold = BandThreshold::Reached;
        sum = sum.wrapping_add(bits_2.min(cap));
      }
      (BandThreshold::Pending, Ordering::Less, Ordering::Equal | Ordering::Greater) => {
        sum = sum.wrapping_add(floor);
      }
      (BandThreshold::Pending, Ordering::Less, Ordering::Less) => {}
    }
  }

  Ok(sum)
}

fn chosen_quality(request: &Request<'_>, limits: &Limits, total: i32) -> Result<i32, Never> {
  let mut low = 1i32;
  let mut high = 10i32;

  for _row in 0..QUALITIES {
    let middle = low.wrapping_add(high).wrapping_shr(1);
    let Ok(sum) = quality_sum(request, limits, middle);

    match sum.cmp(&total) {
      Ordering::Greater => high = middle.wrapping_sub(1),
      Ordering::Less | Ordering::Equal => low = middle.wrapping_add(1),
    }

    match low.cmp(&high) {
      Ordering::Greater => break,
      Ordering::Less | Ordering::Equal => {}
    }
  }

  Ok(low)
}

fn interpolated(
  request: &Request<'_>,
  limits: &Limits,
  total: i32,
) -> Result<Interpolated, Never> {
  let Ok(high) = chosen_quality(request, limits, total);
  let low = high.wrapping_sub(1);
  let mut skip_start = request.start;
  let mut bits1 = [0i32; 21];
  let mut bits2 = [0i32; 21];

  for band in request.start..request.end {
    let Ok(lower) = band_bits(request, limits, low, band);
    let Ok(cap) = of(request.cap, band);
    let Ok(offset) = of(request.offsets, band);
    let upper = match high.cmp(&11) {
      Ordering::Equal | Ordering::Greater => match cap.cmp(&0) {
        Ordering::Greater => {
          let Ok(trim) = of(&limits.trim_offset, band);

          cap.wrapping_add(trim).max(0)
        }
        Ordering::Less | Ordering::Equal => cap,
      },
      Ordering::Less => {
        let Ok(upper) = band_bits(request, limits, high, band);

        upper
      }
    };
    let lower_2 = match low.cmp(&0) {
      Ordering::Greater => lower.wrapping_add(offset),
      Ordering::Less | Ordering::Equal => lower,
    };
    let upper_2 = upper.wrapping_add(offset);

    match offset.cmp(&0) {
      Ordering::Greater => skip_start = band,
      Ordering::Less | Ordering::Equal => {}
    }

    let Ok(()) = set(&mut bits1, band, lower_2);
    let Ok(()) = set(&mut bits2, band, upper_2.wrapping_sub(lower_2).max(0));
  }

  Ok(Interpolated {
    skip_start,
    bits1,
    bits2,
  })
}

pub(crate) fn compute_allocation(
  mode: &Mode,
  request: &Request<'_>,
  dec: &mut RangeDecoder<'_>,
) -> Result<Allocation, Never> {
  let Ok(reserve) = reserved(request);
  let Ok(limits) = limits(request);
  let Ok(curve) = interpolated(request, &limits, reserve.total);

  interp_bits2pulses(
    mode,
    request,
    &Shares {
      reserve,
      thresh: limits.thresh,
      curve,
    },
    dec,
  )
}

struct Shares {
  reserve: Reserve,
  thresh: [i32; 21],
  curve: Interpolated,
}

fn step_sum(
  request: &Request<'_>,
  shares: &Shares,
  crate::quantities::AllocationStep(step): crate::quantities::AllocationStep,
  crate::quantities::AllocationFloor(floor): crate::quantities::AllocationFloor,
) -> Result<i32, Never> {
  let mut sum = 0i32;
  let mut threshold = BandThreshold::Pending;

  for band in (request.start..request.end).rev() {
    let Ok(lower) = of(&shares.curve.bits1, band);
    let Ok(upper) = of(&shares.curve.bits2, band);
    let Ok(thresh) = of(&shares.thresh, band);
    let Ok(cap) = of(request.cap, band);
    let bits = lower.wrapping_add(step.wrapping_mul(upper).wrapping_shr(ALLOC_STEPS));

    match (threshold, bits.cmp(&thresh), bits.cmp(&floor)) {
      (BandThreshold::Reached, _, _) | (_, Ordering::Equal | Ordering::Greater, _) => {
        threshold = BandThreshold::Reached;
        sum = sum.wrapping_add(bits.min(cap));
      }
      (BandThreshold::Pending, Ordering::Less, Ordering::Equal | Ordering::Greater) => {
        sum = sum.wrapping_add(floor);
      }
      (BandThreshold::Pending, Ordering::Less, Ordering::Less) => {}
    }
  }

  Ok(sum)
}

fn first_bits(
  request: &Request<'_>,
  shares: &Shares,
  floor: i32,
) -> Result<([i32; 21], i32), Never> {
  let mut low = 0i32;
  let mut high = 1i32.wrapping_shl(ALLOC_STEPS);

  for _step in 0..ALLOC_STEPS {
    let middle = low.wrapping_add(high).wrapping_shr(1);
    let Ok(sum) = step_sum(
      request,
      shares,
      crate::quantities::AllocationStep(middle),
      crate::quantities::AllocationFloor(floor),
    );

    match sum.cmp(&shares.reserve.total) {
      Ordering::Greater => high = middle,
      Ordering::Less | Ordering::Equal => low = middle,
    }
  }

  let mut bits = [0i32; 21];
  let mut sum = 0i32;
  let mut threshold = BandThreshold::Pending;

  for band in (request.start..request.end).rev() {
    let Ok(lower) = of(&shares.curve.bits1, band);
    let Ok(upper) = of(&shares.curve.bits2, band);
    let Ok(thresh) = of(&shares.thresh, band);
    let Ok(cap) = of(request.cap, band);
    let raw = lower.wrapping_add(low.wrapping_mul(upper).wrapping_shr(ALLOC_STEPS));

    let held = match (threshold, raw.cmp(&thresh)) {
      (BandThreshold::Pending, Ordering::Less) => match raw.cmp(&floor) {
        Ordering::Equal | Ordering::Greater => floor,
        Ordering::Less => 0,
      },
      (BandThreshold::Reached, _) | (_, Ordering::Equal | Ordering::Greater) => {
        threshold = BandThreshold::Reached;

        raw
      }
    };
    let held_2 = held.min(cap);
    let Ok(()) = set(&mut bits, band, held_2);

    sum = sum.wrapping_add(held_2);
  }

  Ok((bits, sum))
}

struct Skipping {
  bits: [i32; 21],
  sum: i32,
  total: i32,
  intensity_rsv: i32,
  coded_bands: u32,
}

fn per_coefficient(
  crate::quantities::BitBudget(left): crate::quantities::BitBudget,
  crate::quantities::BandWidth(span): crate::quantities::BandWidth,
) -> Result<(i32, i32), Never> {
  let Ok(each) = quotient(
    crate::quantities::Numerator(left),
    crate::quantities::Denominator(span),
  );

  Ok((each, left.wrapping_sub(span.wrapping_mul(each))))
}

fn skip_bands(
  request: &Request<'_>,
  shares: &Shares,
  floor: i32,
  dec: &mut RangeDecoder<'_>,
  held: &mut Skipping,
) -> Result<(), Never> {
  let Ok(base) = edge(request.start);

  for _band in request.start..request.end {
    let band = held.coded_bands.wrapping_sub(1);

    match band.cmp(&shares.curve.skip_start) {
      Ordering::Less | Ordering::Equal => {
        held.total = held.total.wrapping_add(shares.reserve.skip);

        break;
      }
      Ordering::Greater => {}
    }

    let Ok(top) = edge(held.coded_bands);
    let Ok(bottom) = edge(band);
    let Ok((each, left)) = per_coefficient(
      crate::quantities::BitBudget(held.total.wrapping_sub(held.sum)),
      crate::quantities::BandWidth(top.wrapping_sub(base)),
    );
    let rem = left.wrapping_sub(bottom.wrapping_sub(base)).max(0);
    let Ok(bits) = of(&held.bits, band);
    let Ok(thresh) = of(&shares.thresh, band);
    let mut band_bits = bits
      .wrapping_add(each.wrapping_mul(top.wrapping_sub(bottom)))
      .wrapping_add(rem);

    match band_bits.cmp(&thresh.max(floor.wrapping_add(ONE_BIT))) {
      Ordering::Equal | Ordering::Greater => {
        let Ok(kept) = dec.bit_logp(1);

        match kept {
          Bit::One => break,
          Bit::Zero => {}
        }

        held.sum = held.sum.wrapping_add(ONE_BIT);
        band_bits = band_bits.wrapping_sub(ONE_BIT);
      }
      Ordering::Less => {}
    }

    held.sum = held.sum.wrapping_sub(bits.wrapping_add(held.intensity_rsv));

    match held.intensity_rsv.cmp(&0) {
      Ordering::Greater => {
        let Ok(at) = index(band.wrapping_sub(request.start));

        held.intensity_rsv = match LOG2_FRAC_TABLE.get(at) {
          Some(bits_2) => *bits_2,
          None => 0,
        };
      }
      Ordering::Less | Ordering::Equal => {}
    }

    held.sum = held.sum.wrapping_add(held.intensity_rsv);

    let kept = match band_bits.cmp(&floor) {
      Ordering::Equal | Ordering::Greater => floor,
      Ordering::Less => 0,
    };

    held.sum = held.sum.wrapping_add(kept);

    let Ok(()) = set(&mut held.bits, band, kept);

    held.coded_bands = band;
  }

  Ok(())
}

fn interp_bits2pulses(
  mode: &Mode,
  request: &Request<'_>,
  shares: &Shares,
  dec: &mut RangeDecoder<'_>,
) -> Result<Allocation, Never> {
  let Ok(wide) = fitted::<u32, i32>(request.channels);
  let floor = wide.wrapping_shl(BITRES);
  let Ok((bits, sum)) = first_bits(request, shares, floor);
  let mut held = Skipping {
    bits,
    sum,
    total: shares.reserve.total,
    intensity_rsv: shares.reserve.intensity,
    coded_bands: request.end,
  };
  let Ok(()) = skip_bands(request, shares, floor, dec, &mut held);

  let intensity = match held.intensity_rsv.cmp(&0) {
    Ordering::Greater => {
      let Ok(chosen) = dec.uint(held.coded_bands.wrapping_add(1).wrapping_sub(request.start));

      request.start.wrapping_add(chosen)
    }
    Ordering::Less | Ordering::Equal => 0,
  };
  let mut dual_stereo_rsv = shares.reserve.dual_stereo;

  match intensity.cmp(&request.start) {
    Ordering::Less | Ordering::Equal => {
      held.total = held.total.wrapping_add(dual_stereo_rsv);
      dual_stereo_rsv = 0;
    }
    Ordering::Greater => {}
  }

  let dual_stereo = match dual_stereo_rsv.cmp(&0) {
    Ordering::Greater => {
      let Ok(bit) = dec.bit_logp(1);

      bit
    }
    Ordering::Less | Ordering::Equal => Bit::Zero,
  };
  let Ok(()) = spread_left(request, &mut held);
  let placed = Placed {
    coded_bands: held.coded_bands,
    intensity,
    dual_stereo,
  };
  let Ok((fine_quant, fine_priority, balance)) = fine(mode, request, &placed, &mut held.bits);

  Ok(Allocation {
    coded_bands: held.coded_bands,
    intensity,
    dual_stereo,
    balance,
    pulses: held.bits,
    fine_quant,
    fine_priority,
  })
}

fn spread_left(request: &Request<'_>, held: &mut Skipping) -> Result<(), Never> {
  let Ok(base) = edge(request.start);
  let Ok(top) = edge(held.coded_bands);
  let Ok((each, mut left)) = per_coefficient(
    crate::quantities::BitBudget(held.total.wrapping_sub(held.sum)),
    crate::quantities::BandWidth(top.wrapping_sub(base)),
  );

  for band in request.start..held.coded_bands {
    let Ok(bits) = of(&held.bits, band);
    let Ok(span) = width(band);
    let Ok(()) = set(
      &mut held.bits,
      band,
      bits.wrapping_add(each.wrapping_mul(span)),
    );
  }

  for band in request.start..held.coded_bands {
    let Ok(bits) = of(&held.bits, band);
    let Ok(span) = width(band);
    let given = left.min(span);
    let Ok(()) = set(&mut held.bits, band, bits.wrapping_add(given));

    left = left.wrapping_sub(given);
  }

  Ok(())
}

struct Placed {
  coded_bands: u32,
  intensity: u32,
  dual_stereo: Bit,
}

struct Fine {
  bits: i32,
  fine: i32,
  priority: i32,
  excess: i32,
}

fn fine_offset(
  crate::quantities::DegreesOfFreedom(degrees): crate::quantities::DegreesOfFreedom,
  crate::quantities::BandWidth(size): crate::quantities::BandWidth,
  crate::quantities::BitBudget(bits): crate::quantities::BitBudget,
  crate::quantities::ChannelLogDimensions(nclogn): crate::quantities::ChannelLogDimensions,
) -> Result<i32, Never> {
  let mut offset = nclogn
    .wrapping_shr(1)
    .wrapping_sub(degrees.wrapping_mul(FINE_OFFSET));

  match size {
    2 => offset = offset.wrapping_add(degrees.wrapping_shl(BITRES).wrapping_shr(2)),
    _other => {}
  }

  let total = bits.wrapping_add(offset);

  Ok(
    match (
      total.cmp(&degrees.wrapping_mul(2).wrapping_shl(BITRES)),
      total.cmp(&degrees.wrapping_mul(3).wrapping_shl(BITRES)),
    ) {
      (Ordering::Less, _) => offset.wrapping_add(nclogn.wrapping_shr(2)),
      (Ordering::Equal | Ordering::Greater, Ordering::Less) => {
        offset.wrapping_add(nclogn.wrapping_shr(3))
      }
      (Ordering::Equal | Ordering::Greater, Ordering::Equal | Ordering::Greater) => offset,
    },
  )
}

fn fine_band(
  mode: &Mode,
  request: &Request<'_>,
  placed: &Placed,
  band: u32,
  bits: i32,
) -> Result<Fine, Never> {
  let Ok(wide) = fitted::<u32, i32>(request.channels);
  let Ok(lm) = fitted::<u32, i32>(request.lm);
  let stereo = match request.channels.cmp(&1) {
    Ordering::Greater => 1u32,
    Ordering::Less | Ordering::Equal => 0,
  };
  let Ok(span) = width(band);
  let Ok(cap) = of(request.cap, band);
  let size = span.wrapping_shl(request.lm);

  match size.cmp(&1) {
    Ordering::Greater => {
      let excess = bits.wrapping_sub(cap).max(0);
      let bits_2 = bits.wrapping_sub(excess);
      let extra = match (
        request.channels,
        size.cmp(&2),
        placed.dual_stereo,
        band.cmp(&placed.intensity),
      ) {
        (2, Ordering::Greater, Bit::Zero, Ordering::Less) => 1,
        (_, _, _, _) => 0,
      };
      let degrees = wide.wrapping_mul(size).wrapping_add(extra);
      let Ok(log_n) = of(&mode.log_n, band);
      let nclogn = degrees.wrapping_mul(log_n.wrapping_add(lm.wrapping_shl(BITRES)));
      let Ok(offset) = fine_offset(
        crate::quantities::DegreesOfFreedom(degrees),
        crate::quantities::BandWidth(size),
        crate::quantities::BitBudget(bits_2),
        crate::quantities::ChannelLogDimensions(nclogn),
      );
      let Ok(each) = quotient(
        crate::quantities::Numerator(
          bits_2
            .wrapping_add(offset)
            .wrapping_add(degrees.wrapping_shl(BITRES.wrapping_sub(1)))
            .max(0),
        ),
        crate::quantities::Denominator(degrees),
      );
      let fine = each.wrapping_shr(BITRES);
      let fine_2 = match wide.wrapping_mul(fine).cmp(&bits_2.wrapping_shr(BITRES)) {
        Ordering::Greater => bits_2.wrapping_shr(stereo).wrapping_shr(BITRES),
        Ordering::Less | Ordering::Equal => fine,
      };
      let fine_3 = fine_2.min(MAX_FINE_BITS);
      let priority = match fine_3
        .wrapping_mul(degrees.wrapping_shl(BITRES))
        .cmp(&bits_2.wrapping_add(offset))
      {
        Ordering::Equal | Ordering::Greater => 1,
        Ordering::Less => 0,
      };

      Ok(Fine {
        bits: bits_2.wrapping_sub(wide.wrapping_mul(fine_3).wrapping_shl(BITRES)),
        fine: fine_3,
        priority,
        excess,
      })
    }
    Ordering::Less | Ordering::Equal => {
      let excess = bits.wrapping_sub(wide.wrapping_shl(BITRES)).max(0);

      Ok(Fine {
        bits: bits.wrapping_sub(excess),
        fine: 0,
        priority: 1,
        excess,
      })
    }
  }
}

fn fine(
  mode: &Mode,
  request: &Request<'_>,
  placed: &Placed,
  bits: &mut [i32; 21],
) -> Result<([i32; 21], [i32; 21], i32), Never> {
  let Ok(wide) = fitted::<u32, i32>(request.channels);
  let stereo = match request.channels.cmp(&1) {
    Ordering::Greater => 1u32,
    Ordering::Less | Ordering::Equal => 0,
  };
  let mut fine_quant = [0i32; 21];
  let mut fine_priority = [0i32; 21];
  let mut balance = 0i32;

  for band in request.start..placed.coded_bands {
    let Ok(held) = of(bits, band);
    let Ok(mut answer) = fine_band(mode, request, placed, band, held.wrapping_add(balance));

    match answer.excess.cmp(&0) {
      Ordering::Greater => {
        let extra_fine = answer
          .excess
          .wrapping_shr(stereo.wrapping_add(BITRES))
          .min(MAX_FINE_BITS.wrapping_sub(answer.fine));
        let extra_bits = extra_fine.wrapping_mul(wide).wrapping_shl(BITRES);

        answer.fine = answer.fine.wrapping_add(extra_fine);
        answer.priority = match extra_bits.cmp(&answer.excess.wrapping_sub(balance)) {
          Ordering::Equal | Ordering::Greater => 1,
          Ordering::Less => 0,
        };
        answer.excess = answer.excess.wrapping_sub(extra_bits);
      }
      Ordering::Less | Ordering::Equal => {}
    }

    let Ok(()) = set(bits, band, answer.bits);
    let Ok(()) = set(&mut fine_quant, band, answer.fine);
    let Ok(()) = set(&mut fine_priority, band, answer.priority);

    balance = answer.excess;
  }

  for band in placed.coded_bands..request.end {
    let Ok(held) = of(bits, band);
    let fine = held.wrapping_shr(stereo).wrapping_shr(BITRES);
    let Ok(()) = set(&mut fine_quant, band, fine);
    let Ok(()) = set(bits, band, 0);
    let Ok(()) = set(
      &mut fine_priority,
      band,
      match fine.cmp(&1) {
        Ordering::Less => 1,
        Ordering::Equal | Ordering::Greater => 0,
      },
    );
  }

  Ok((fine_quant, fine_priority, balance))
}
