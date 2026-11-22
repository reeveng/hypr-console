//! Floor type 1: the outline as points joined by straight lines.
//!
//! A stream lists the x positions the points sit at, and each packet gives the
//! y of every one, most of them as a correction to what the neighbours on
//! either side already say. The curve is the line drawn through the points in
//! the order of their x, one amplitude per step of a table that is the same
//! for every stream.

use std::cmp::Ordering;
use std::collections::HashSet;

use console_core_audio_decoding_interfaces::errors::{Error, decode_error};
use console_core_byte_streams::{BitReaderRtl, FlagBit, ReadBitsRtl};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::codebook::{VorbisCodebook, book, ilog};
use crate::dsp::Decoding;
use crate::inverse_db::FLOOR1_INVERSE_DB_TABLE;
use crate::reading::{Fields, copied, entry, entry_mut, tolerate_end};

const MOST_POINTS: u32 = 65;

#[derive(Debug, Clone)]
struct Class {
  mainbook: Option<u8>,
  dimensions: u8,
  subclass_bits: u8,
  subbooks: Vec<Option<u8>>,
}

#[derive(Debug, Clone, Copy)]
struct Point {
  x: u32,
  y: i32,
}

#[derive(Debug, Clone, Copy)]
enum PointUse {
  Render,
  Skip,
}

#[derive(Debug)]
pub(crate) struct Floor1 {
  partitions: Vec<Class>,
  multiplier: i32,
  range: u32,
  x_list: Vec<u32>,
  sort_order: Vec<u32>,
  neighbors: Vec<(u32, u32)>,
  pub(crate) decoding: Decoding,
  floor_y: Vec<u32>,
  final_y: Vec<i32>,
  step2: Vec<PointUse>,
}

fn read_class(bs: &mut BitReaderRtl<'_>, codebooks: u16) -> Result<Class, Error> {
  let dimensions_raw = bs.bits_u8(3)?;
  let dimensions = dimensions_raw.saturating_add(1);
  let subclass_bits = bs.bits_u8(2)?;
  let mainbook = match subclass_bits {
    0 => None,
    _ => {
      let masterbook = bs.bits_u8(8)?;

      match u16::from(masterbook).cmp(&codebooks) {
        Ordering::Equal | Ordering::Greater => {
          return decode_error("vorbis: floor1, invalid codebook for class");
        }
        Ordering::Less => Some(masterbook),
      }
    }
  };
  let mut subbooks = Vec::new();

  for _ in 0..1u32.wrapping_shl(u32::from(subclass_bits)) {
    let number = bs.bits_u8(8)?;

    subbooks.push(match number.checked_sub(1) {
      None => None,
      Some(listed) => match u16::from(listed).cmp(&codebooks) {
        Ordering::Equal | Ordering::Greater => {
          return decode_error("vorbis: floor1, invalid codebook for subclass");
        }
        Ordering::Less => Some(listed),
      },
    });
  }

  Ok(Class {
    mainbook,
    dimensions,
    subclass_bits,
    subbooks,
  })
}

fn read_partitions(bs: &mut BitReaderRtl<'_>, codebooks: u16) -> Result<Vec<Class>, Error> {
  let count = bs.bits_u8(5)?;
  let mut class_list = Vec::new();

  for _ in 0..count {
    let class = bs.bits_u8(4)?;

    class_list.push(class);
  }

  let classes_wanted = match class_list.iter().max() {
    Some(highest) => u32::from(*highest).saturating_add(1),
    None => 0,
  };
  let mut classes = Vec::new();

  for _ in 0..classes_wanted {
    let class = read_class(bs, codebooks)?;

    classes.push(class);
  }

  class_list
    .iter()
    .map(|class| entry(&classes, u32::from(*class)).cloned())
    .collect()
}

fn read_x_list(bs: &mut BitReaderRtl<'_>, partitions: &[Class]) -> Result<Vec<u32>, Error> {
  let range_bits = bs.read_bits_leq32(4)?;
  let mut x_list = vec![0, 1u32.wrapping_shl(range_bits)];
  let mut unique = HashSet::new();

  for class in partitions {
    let Ok(held) = fitted::<_, u32>(x_list.len());

    match held
      .saturating_add(u32::from(class.dimensions))
      .cmp(&MOST_POINTS)
    {
      Ordering::Greater => return decode_error("vorbis: floor1, x_list too long"),
      Ordering::Less | Ordering::Equal => {}
    }

    for _ in 0..class.dimensions {
      let x = bs.read_bits_leq32(range_bits)?;

      match unique.replace(x) {
        None => x_list.push(x),
        Some(_) => return decode_error("vorbis: floor1, x_list is not unique"),
      }
    }
  }

  Ok(x_list)
}

fn neighborhood<'a>(
  prior: impl Iterator<Item = (u32, &'a u32)>,
  bound: u32,
) -> Result<(u32, u32), Never> {
  let (_, _, low_at, high_at) = prior.fold(
    (u32::MIN, u32::MAX, 0, 0),
    |(low, high, low_at, high_at), (at, x)| {
      let from_bound = x.cmp(&bound);
      let (low_2, low_at_2) = match (from_bound, x.cmp(&low)) {
        (Ordering::Less, Ordering::Greater) => (*x, at),
        _unchanged => (low, low_at),
      };
      let (high_2, high_at_2) = match (from_bound, x.cmp(&high)) {
        (Ordering::Greater, Ordering::Less) => (*x, at),
        _unchanged => (high, high_at),
      };

      (low_2, high_2, low_at_2, high_at_2)
    },
  );

  Ok((low_at, high_at))
}

fn render_point(low: Point, high: Point, x: u32) -> Result<i32, Error> {
  let dy = high.y.saturating_sub(low.y);
  let adx = high.x.saturating_sub(low.x);
  let err = dy.unsigned_abs().saturating_mul(x.saturating_sub(low.x));
  let off = match err.checked_div(adx) {
    Some(off) => off,
    None => return decode_error("vorbis: floor1 points share a position"),
  };
  let Ok(off_2) = fitted::<u32, i32>(off);

  Ok(match dy.cmp(&0) {
    Ordering::Less => low.y.saturating_sub(off_2),
    Ordering::Equal | Ordering::Greater => low.y.saturating_add(off_2),
  })
}

fn inverse_db(y: i32) -> Result<f32, Error> {
  match u32::try_from(y) {
    Ok(step) => entry(&FLOOR1_INVERSE_DB_TABLE, step).copied(),
    Err(_negative) => decode_error("vorbis: floor1 amplitude is below the table"),
  }
}

fn render_line(from: Point, to: Point, n: u32, v: &mut [f32]) -> Result<(), Error> {
  let Ok(adx) = fitted::<u32, i32>(to.x.saturating_sub(from.x));
  let dy = to.y.saturating_sub(from.y);
  let base = match dy.checked_div(adx) {
    Some(base) => base,
    None => return decode_error("vorbis: floor1 points share a position"),
  };
  let sy = match dy.cmp(&0) {
    Ordering::Less => base.saturating_sub(1),
    Ordering::Equal | Ordering::Greater => base.saturating_add(1),
  };
  let ady = dy
    .saturating_abs()
    .saturating_sub(base.saturating_abs().saturating_mul(adx));
  let first = entry_mut(v, from.x)?;

  let level = inverse_db(from.y)?;

  *first = level;

  let begin = from.x.saturating_add(1);
  let end = n.min(to.x);
  let Ok(skipped) = index(begin);
  let Ok(taken) = index(end.saturating_sub(begin));
  let mut y = from.y;
  let mut err = 0i32;

  for slot in v.iter_mut().skip(skipped).take(taken) {
    err = err.saturating_add(ady);

    y = match err.cmp(&adx) {
      Ordering::Equal | Ordering::Greater => {
        err = err.saturating_sub(adx);
        y.saturating_add(sy)
      }
      Ordering::Less => y.saturating_add(base),
    };

    let level_2 = inverse_db(y)?;

    *slot = level_2;
  }

  Ok(())
}

impl Class {
  fn read_value(
    &self,
    bs: &mut BitReaderRtl<'_>,
    codebooks: &[VorbisCodebook],
    subclass: u32,
  ) -> Result<u32, Error> {
    let used = match entry(&self.subbooks, subclass) {
      Ok(used) => *used,
      Err(_beyond) => None,
    };

    match used {
      Some(number) => {
        let codebook = book(codebooks, u16::from(number))?;

        codebook.read_scalar(bs)
      }
      None => Ok(0),
    }
  }

  fn read_main(
    &self,
    bs: &mut BitReaderRtl<'_>,
    codebooks: &[VorbisCodebook],
  ) -> Result<u32, Error> {
    match self.mainbook {
      Some(number) => {
        let codebook = book(codebooks, u16::from(number))?;

        codebook.read_scalar(bs)
      }
      None => Ok(0),
    }
  }
}

impl Floor1 {
  pub(crate) fn read(bs: &mut BitReaderRtl<'_>, codebooks: u16) -> Result<Floor1, Error> {
    let partitions = read_partitions(bs, codebooks)?;
    let multiplier_bits = bs.read_bits_leq32(2)?;
    let range = match multiplier_bits {
      0 => 256,
      1 => 128,
      2 => 86,
      _ => 64,
    };
    let Ok(multiplier) = fitted::<u32, i32>(multiplier_bits);
    let x_list = read_x_list(bs, &partitions)?;
    let neighbors = x_list
      .iter()
      .enumerate()
      .map(|(at, bound)| {
        let Ok(found) = neighborhood((0u32..).zip(x_list.iter().take(at)), *bound);

        found
      })
      .collect();
    let Ok(points) = fitted::<_, u32>(x_list.len());
    let mut sort_order: Vec<u32> = (0..points).collect();

    sort_order.sort_by_key(|at| {
      let Ok(position) = index(*at);

      x_list.get(position).copied()
    });

    Ok(Floor1 {
      partitions,
      multiplier: multiplier.saturating_add(1),
      range,
      floor_y: vec![0; x_list.len()],
      final_y: Vec::new(),
      step2: Vec::new(),
      x_list,
      sort_order,
      neighbors,
      decoding: Decoding::Needed,
    })
  }
}

#[derive(Debug, Clone, Copy)]
struct Prediction {
  predicted: i32,
  range: i32,
}

fn corrected(val: i32, prediction: Prediction) -> Result<i32, Never> {
  let Prediction { predicted, range } = prediction;
  let highroom = range.saturating_sub(predicted);
  let lowroom = predicted;
  let room = 2i32.saturating_mul(highroom.min(lowroom));

  Ok(match (val.cmp(&room), highroom.cmp(&lowroom), val & 1) {
    (Ordering::Equal | Ordering::Greater, Ordering::Greater, _) => {
      val.saturating_sub(lowroom).saturating_add(predicted)
    }
    (Ordering::Equal | Ordering::Greater, Ordering::Less | Ordering::Equal, _) => predicted
      .saturating_sub(val)
      .saturating_add(highroom)
      .saturating_sub(1),
    (Ordering::Less, _, 1) => predicted.saturating_sub(val.saturating_add(1).wrapping_shr(1)),
    (Ordering::Less, _, _) => predicted.saturating_add(val.wrapping_shr(1)),
  })
}

impl Floor1 {
  pub(crate) fn read_channel(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    codebooks: &[VorbisCodebook],
  ) -> Result<(), Error> {
    self.decoding = Decoding::Skipped;

    let read = self.read_values(bs, codebooks);

    tolerate_end(read)
  }

  fn read_values(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    codebooks: &[VorbisCodebook],
  ) -> Result<(), Error> {
    let used = bs.read_flag()?;

    match used {
      FlagBit::Zero => return Ok(()),
      FlagBit::One => {}
    }

    let Ok(range_bits) = ilog(self.range.saturating_sub(1));
    let mut slots = self.floor_y.iter_mut();

    for slot in slots.by_ref().take(2) {
      let y = bs.read_bits_leq32(range_bits)?;

      *slot = y;
    }

    for class in &self.partitions {
      let bits = u32::from(class.subclass_bits);
      let mask = 1u32.wrapping_shl(bits).saturating_sub(1);
      let Ok(dimensions) = index(u32::from(class.dimensions));
      let mut cval = class.read_main(bs, codebooks)?;

      for slot in slots.by_ref().take(dimensions) {
        let y = class.read_value(bs, codebooks, cval & mask)?;

        *slot = y;
        cval = cval.wrapping_shr(bits);
      }
    }

    self.decoding = Decoding::Needed;

    Ok(())
  }

  pub(crate) fn synthesis(&mut self, half: u32, floor: &mut [f32]) -> Result<(), Error> {
    self.synthesis_step1()?;
    self.synthesis_step2(half, floor)
  }

  fn synthesis_step1(&mut self) -> Result<(), Error> {
    let Floor1 {
      floor_y,
      x_list,
      neighbors,
      final_y,
      step2,
      range,
      ..
    } = self;
    let Ok(range_2) = fitted::<u32, i32>(*range);

    final_y.clear();
    step2.clear();

    for y in floor_y.iter().take(2) {
      let Ok(first) = fitted::<u32, i32>(*y);

      final_y.push(first);
      step2.push(PointUse::Render);
    }

    for ((y, (low, high)), x) in floor_y
      .iter()
      .zip(neighbors.iter())
      .zip(x_list.iter())
      .skip(2)
    {
      let below_x = copied(x_list, *low)?;
      let below_y = copied(final_y, *low)?;
      let above_x = copied(x_list, *high)?;
      let above_y = copied(final_y, *high)?;
      let below = Point {
        x: below_x,
        y: below_y,
      };
      let above = Point {
        x: above_x,
        y: above_y,
      };
      let predicted = render_point(below, above, *x)?;
      let Ok(encoded_delta) = fitted::<u32, i32>(*y);

      match encoded_delta {
        0 => {
          step2.push(PointUse::Skip);
          final_y.push(predicted);
        }
        _ => {
          let lower = entry_mut(step2, *low)?;

          *lower = PointUse::Render;

          let upper = entry_mut(step2, *high)?;

          *upper = PointUse::Render;
          step2.push(PointUse::Render);

          let Ok(value) = corrected(
            encoded_delta,
            Prediction {
              predicted,
              range: range_2,
            },
          );

          final_y.push(value);
        }
      }
    }

    Ok(())
  }

  fn synthesis_step2(&self, n: u32, floor: &mut [f32]) -> Result<(), Error> {
    let first = copied(&self.sort_order, 0)?;
    let first_y = copied(&self.final_y, first)?;
    let mut low = Point {
      x: 0,
      y: first_y.saturating_mul(self.multiplier),
    };
    let mut high = Point { x: 0, y: 0 };

    for at in self.sort_order.iter().skip(1) {
      let flagged = copied(&self.step2, *at)?;

      match flagged {
        PointUse::Render => {
          let y = copied(&self.final_y, *at)?;
          let x = copied(&self.x_list, *at)?;

          high = Point {
            x,
            y: y.saturating_mul(self.multiplier),
          };

          render_line(low, high, n, floor)?;

          low = high;
        }
        PointUse::Skip => {}
      }
    }

    match high.x.cmp(&n) {
      Ordering::Less => render_line(high, Point { x: n, y: high.y }, n, floor),
      Ordering::Equal | Ordering::Greater => Ok(()),
    }
  }
}
