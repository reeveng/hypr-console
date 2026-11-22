//! Decoding one layer two frame into samples.
//!
//! A frame is thirty-six time slots of thirty-two subbands, though the table
//! the bit rate and rate select stops short of the top of the spectrum and
//! the subbands above its limit are silent. Each subband says which of a
//! few quantizers its samples went through, the quantizers differ in how many
//! levels they have, and three samples of one of the coarse ones are packed
//! into one codeword; a subband of twelve slots shares a scale factor, and
//! three of those make the frame, unless the selection information says that
//! two neighbours repeat one another and the repeat is not sent.
//!
//! From the subband the header names upward a joint stereo frame holds one
//! quantizer and one set of samples for both channels, scaled by each one's
//! own factors, as layer one does.

use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_byte_streams::BitReaderLtr;
use console_core_number_conversion::index;

use crate::bits::Fields;
use crate::common::{FrameHeader, Infallible, Version};
use crate::layer12::{self, Coding, Layer};

const GROUPS: u32 = 12;

#[derive(Copy, Clone)]
enum Packing {
  Grouped,
  Single,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "c and d are the standard's dimensionless requantization multiplier and offset"
  )
)]
struct QuantClass {
  c: f32,
  d: f32,
  packing: Packing,
  bits: u32,
  nlevels: u32,
}

struct SbQuantInfo {
  nbal: u32,
  classes: [u8; 16],
}

struct SbInfo {
  sblimit: u32,
  bands: [u8; 32],
}

#[allow(
  clippy::excessive_precision,
  reason = "the constants are the standard's, with the digits it prints"
)]
const QUANT_CLASS: [QuantClass; 17] = [
  QuantClass {
    c: 1.33333333333,
    d: 0.50000000000,
    packing: Packing::Grouped,
    bits: 5,
    nlevels: 3,
  },
  QuantClass {
    c: 1.60000000000,
    d: 0.50000000000,
    packing: Packing::Grouped,
    bits: 7,
    nlevels: 5,
  },
  QuantClass {
    c: 1.14285714286,
    d: 0.25000000000,
    packing: Packing::Single,
    bits: 3,
    nlevels: 7,
  },
  QuantClass {
    c: 1.77777777777,
    d: 0.50000000000,
    packing: Packing::Grouped,
    bits: 10,
    nlevels: 9,
  },
  QuantClass {
    c: 1.06666666666,
    d: 0.12500000000,
    packing: Packing::Single,
    bits: 4,
    nlevels: 15,
  },
  QuantClass {
    c: 1.03225806452,
    d: 0.06250000000,
    packing: Packing::Single,
    bits: 5,
    nlevels: 31,
  },
  QuantClass {
    c: 1.01587301587,
    d: 0.03125000000,
    packing: Packing::Single,
    bits: 6,
    nlevels: 63,
  },
  QuantClass {
    c: 1.00787401575,
    d: 0.01562500000,
    packing: Packing::Single,
    bits: 7,
    nlevels: 127,
  },
  QuantClass {
    c: 1.00392156863,
    d: 0.00781250000,
    packing: Packing::Single,
    bits: 8,
    nlevels: 255,
  },
  QuantClass {
    c: 1.00195694716,
    d: 0.00390625000,
    packing: Packing::Single,
    bits: 9,
    nlevels: 511,
  },
  QuantClass {
    c: 1.00097751711,
    d: 0.00195312500,
    packing: Packing::Single,
    bits: 10,
    nlevels: 1023,
  },
  QuantClass {
    c: 1.00048851979,
    d: 0.00097656250,
    packing: Packing::Single,
    bits: 11,
    nlevels: 2047,
  },
  QuantClass {
    c: 1.00024420024,
    d: 0.00048828125,
    packing: Packing::Single,
    bits: 12,
    nlevels: 4095,
  },
  QuantClass {
    c: 1.00012208522,
    d: 0.00024414063,
    packing: Packing::Single,
    bits: 13,
    nlevels: 8191,
  },
  QuantClass {
    c: 1.00006103888,
    d: 0.00012207031,
    packing: Packing::Single,
    bits: 14,
    nlevels: 16383,
  },
  QuantClass {
    c: 1.00003051851,
    d: 0.00006103516,
    packing: Packing::Single,
    bits: 15,
    nlevels: 32767,
  },
  QuantClass {
    c: 1.00001525902,
    d: 0.00003051758,
    packing: Packing::Single,
    bits: 16,
    nlevels: 65535,
  },
];

const SB_QUANT_INFO: [SbQuantInfo; 8] = [
  SbQuantInfo {
    nbal: 2,
    classes: [0, 0, 1, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  },
  SbQuantInfo {
    nbal: 2,
    classes: [0, 0, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  },
  SbQuantInfo {
    nbal: 3,
    classes: [0, 0, 1, 3, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0],
  },
  SbQuantInfo {
    nbal: 3,
    classes: [0, 0, 1, 2, 3, 4, 5, 16, 0, 0, 0, 0, 0, 0, 0, 0],
  },
  SbQuantInfo {
    nbal: 4,
    classes: [0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
  },
  SbQuantInfo {
    nbal: 4,
    classes: [0, 0, 1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
  },
  SbQuantInfo {
    nbal: 4,
    classes: [0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 16],
  },
  SbQuantInfo {
    nbal: 4,
    classes: [0, 0, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
  },
];

const SB_INFO: [SbInfo; 5] = [
  SbInfo {
    sblimit: 27,
    bands: [
      7, 7, 7, 6, 6, 6, 6, 6, 6, 6, 6, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0, 0,
      0, 0,
    ],
  },
  SbInfo {
    sblimit: 30,
    bands: [
      7, 7, 7, 6, 6, 6, 6, 6, 6, 6, 6, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0, 0,
      0, 0,
    ],
  },
  SbInfo {
    sblimit: 8,
    bands: [
      5, 5, 2, 2, 2, 2, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0,
    ],
  },
  SbInfo {
    sblimit: 12,
    bands: [
      5, 5, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0,
    ],
  },
  SbInfo {
    sblimit: 30,
    bands: [
      4, 4, 4, 4, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
      0, 0,
    ],
  },
];

#[derive(Clone, Copy, Default)]
struct Band {
  class: [Option<&'static QuantClass>; 2],
  selection: [u32; 2],
  scale: [[f32; 3]; 2],
}

struct Plan {
  info: &'static SbInfo,
  bound: u32,
  channels: u32,
}

fn find_sb_info(header: &FrameHeader) -> Infallible<&'static SbInfo> {
  let [a, b, c, d, e] = &SB_INFO;
  let per_channel = header.bitrate.div_euclid(header.channels.max(1));

  Ok(match header.version {
    Version::Mpeg2 | Version::Mpeg2p5 => e,
    Version::Mpeg1 => match (per_channel, header.sample_rate) {
      (..=48_000, 32_000) => d,
      (..=48_000, _) => c,
      (..=80_000, _) | (_, 48_000) => a,
      (_, _) => b,
    },
  })
}

fn entry<T>(table: &'static [T], at: u8) -> Infallible<Option<&'static T>> {
  let Ok(at_2) = index(at);

  Ok(table.get(at_2))
}

fn quantization(info: &SbInfo, at: u32) -> Result<&'static SbQuantInfo> {
  let Ok(at_2) = index(at);

  let found = match info.bands.get(at_2) {
    Some(&table) => {
      let Ok(found) = entry(&SB_QUANT_INFO, table);

      found
    }
    None => None,
  };

  match found {
    Some(quantization) => Ok(quantization),
    None => decode_error("mp2: subband beyond the table"),
  }
}

fn find_class(quantization: &SbQuantInfo, code: u32) -> Result<Option<&'static QuantClass>> {
  let Ok(at) = index(code);

  match (code, quantization.classes.get(at)) {
    (0, _) => Ok(None),
    (_, Some(&class)) => {
      let Ok(found) = entry(&QUANT_CLASS, class);

      match found {
        Some(class_2) => Ok(Some(class_2)),
        None => decode_error("mp2: invalid quantization class"),
      }
    }
    (_, None) => decode_error("mp2: invalid bit allocation"),
  }
}

fn allocation(
  bs: &mut BitReaderLtr<'_>,
  quantization: &SbQuantInfo,
) -> Result<Option<&'static QuantClass>> {
  let code: u32 = bs.field(quantization.nbal)?;

  find_class(quantization, code)
}

fn read_allocations(
  bs: &mut BitReaderLtr<'_>,
  plan: &Plan,
  bands: &mut [Band; 32],
) -> Result<()> {
  let Ok(channels) = index(plan.channels);
  let Ok(limit) = index(plan.info.sblimit);

  for (at, band) in (0u32..).zip(bands.iter_mut()).take(limit) {
    let quantization = quantization(plan.info, at)?;

    match ::core::primitive::bool::then_some(at < plan.bound, ()) {
      ::core::option::Option::Some(()) => {
        for lane in band.class.iter_mut().take(channels) {
          let class = allocation(bs, quantization)?;

          *lane = class;
        }
      }
      ::core::option::Option::None => {
        let class = allocation(bs, quantization)?;

        band.class = [class; 2];
      }
    }
  }

  Ok(())
}

fn read_selections(bs: &mut BitReaderLtr<'_>, plan: &Plan, bands: &mut [Band; 32]) -> Result<()> {
  let Ok(channels) = index(plan.channels);
  let Ok(limit) = index(plan.info.sblimit);

  for band in bands.iter_mut().take(limit) {
    for (class, selection) in band
      .class
      .iter()
      .zip(band.selection.iter_mut())
      .take(channels)
    {
      match class {
        None => {}
        Some(_) => {
          let code: u32 = bs.field(2)?;

          *selection = code;
        }
      }
    }
  }

  Ok(())
}

fn read_codes(bs: &mut BitReaderLtr<'_>, selection: u32) -> Result<[u32; 3]> {
  let first: u32 = bs.field(6)?;

  match selection {
    0 => {
      let second: u32 = bs.field(6)?;
      let third: u32 = bs.field(6)?;

      Ok([first, second, third])
    }
    1 => {
      let third: u32 = bs.field(6)?;

      Ok([first, first, third])
    }
    2 => Ok([first; 3]),
    3.. => {
      let second: u32 = bs.field(6)?;

      Ok([first, second, second])
    }
  }
}

fn read_scale_factors(
  bs: &mut BitReaderLtr<'_>,
  plan: &Plan,
  bands: &mut [Band; 32],
) -> Result<()> {
  let Ok(channels) = index(plan.channels);
  let Ok(limit) = index(plan.info.sblimit);

  for band in bands.iter_mut().take(limit) {
    let lanes = band
      .class
      .iter()
      .zip(band.selection)
      .zip(band.scale.iter_mut());

    for ((class, selection), scale) in lanes.take(channels) {
      match class {
        None => {}
        Some(_) => {
          let [first, second, third] = read_codes(bs, selection)?;
          let first_2 = layer12::scale_factor(first)?;
          let second_2 = layer12::scale_factor(second)?;
          let third_2 = layer12::scale_factor(third)?;

          *scale = [first_2, second_2, third_2];
        }
      }
    }
  }

  Ok(())
}

fn triplet(bs: &mut BitReaderLtr<'_>, class: &QuantClass) -> Result<[f32; 3]> {
  let (raw, bits) = match class.packing {
    Packing::Grouped => {
      let mut code: u32 = bs.field(class.bits)?;
      let mut raw = [0u32; 3];

      for item in &mut raw {
        *item = code.rem_euclid(class.nlevels);
        code = code.div_euclid(class.nlevels);
      }

      (raw, class.nlevels.next_power_of_two().trailing_zeros())
    }
    Packing::Single => {
      let first: u32 = bs.field(class.bits)?;
      let second: u32 = bs.field(class.bits)?;
      let third: u32 = bs.field(class.bits)?;

      ([first, second, third], class.bits)
    }
  };

  let divisor = f32::from(1u16.wrapping_shl(bits.saturating_sub(1)));

  Ok(raw.map(|raw_2| {
    let Ok(centered) = layer12::centered([raw_2, bits]);

    class.c * (f32::from(centered) / divisor + class.d)
  }))
}

fn part_scale(scale: [f32; 3], group: u32) -> Infallible<f32> {
  let Ok(part) = index(group.div_euclid(4));

  Ok(match scale.get(part) {
    Some(&factor) => factor,
    None => 0.0,
  })
}

fn put_triplet(row: &mut [f32], group: u32, values: [f32; 3]) -> Infallible<()> {
  for (offset, value) in (0u32..).zip(values) {
    let Ok(()) = layer12::put(row, group.saturating_mul(3).saturating_add(offset), value);
  }

  Ok(())
}

fn read_group(
  bs: &mut BitReaderLtr<'_>,
  band: &Band,
  rows: [&mut [f32]; 2],
  at: [u32; 2],
) -> Result<()> {
  let [group, channels] = at;
  let Ok(channels_2) = index(channels);

  for (row, (class, scale)) in rows
    .into_iter()
    .zip(band.class.into_iter().zip(band.scale))
    .take(channels_2)
  {
    match class {
      None => {}
      Some(class_2) => {
        let values = triplet(bs, class_2)?;
        let Ok(factor) = part_scale(scale, group);
        let Ok(()) = put_triplet(row, group, values.map(|value| factor * value));
      }
    }
  }

  Ok(())
}

fn read_shared_group(
  bs: &mut BitReaderLtr<'_>,
  band: &Band,
  rows: [&mut [f32]; 2],
  at: [u32; 2],
) -> Result<()> {
  let [group, channels] = at;
  let Ok(channels_2) = index(channels);
  let [first, _] = band.class;

  match first {
    None => Ok(()),
    Some(class) => {
      let values = triplet(bs, class)?;

      for (row, scale) in rows.into_iter().zip(band.scale).take(channels_2) {
        let Ok(factor) = part_scale(scale, group);
        let Ok(()) = put_triplet(row, group, values.map(|value| factor * value));
      }

      Ok(())
    }
  }
}

fn read_samples(
  bs: &mut BitReaderLtr<'_>,
  plan: &Plan,
  bands: &[Band; 32],
  sheets: [&mut [f32]; 2],
) -> Result<()> {
  let [left, right] = sheets;
  let Ok(limit) = index(plan.info.sblimit);

  for group in 0..GROUPS {
    let rows = left.chunks_mut(36).zip(right.chunks_mut(36));
    let subbands = (0u32..).zip(bands.iter().zip(rows)).take(limit);

    for (at, (band, (left_2, right_2))) in subbands {
      let place = [group, plan.channels];

      let read = match ::core::primitive::bool::then_some(at < plan.bound, ()) {
        ::core::option::Option::Some(()) => read_group(bs, band, [left_2, right_2], place),
        ::core::option::Option::None => read_shared_group(bs, band, [left_2, right_2], place),
      };

      read?;
    }
  }

  Ok(())
}

pub struct LayerTwo;

pub type Layer2 = Layer<LayerTwo>;

impl Coding for LayerTwo {
  const SLOTS: u32 = 36;

  fn read(
    bs: &mut BitReaderLtr<'_>,
    header: &FrameHeader,
    sheets: [&mut [f32]; 2],
  ) -> Result<()> {
    let Ok(info) = find_sb_info(header);
    let Ok(bound) = layer12::bound(header);
    let plan = Plan {
      info,
      bound: bound.min(info.sblimit),
      channels: header.channels,
    };
    let mut bands = [Band::default(); 32];

    read_allocations(bs, &plan, &mut bands)?;
    read_selections(bs, &plan, &mut bands)?;
    read_scale_factors(bs, &plan, &mut bands)?;

    read_samples(bs, &plan, &bands, sheets)
  }
}
