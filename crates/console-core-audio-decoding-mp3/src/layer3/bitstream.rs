//! Reading a frame's side information and a granule's scale factors.
//!
//! The side information is a fixed layout at the front of every frame, one set
//! of fields per granule and channel, and says how the main data after it is
//! to be read. The scale factors are the first thing in that main data, and
//! how many bits each takes is decided by the side information, which is why
//! they are read here too.

use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_byte_streams::{FlagBit, ReadBitsLtr};
use console_core_number_conversion::index;

use super::frame::{BlockType, FrameData, Granule, GranuleChannel, ScaleFactorGroup, Switches};
use crate::bits::Fields;
use crate::common::{ChannelMode, FrameHeader, Infallible, Switch, Version};

const SCALE_FACTOR_SLEN: [(u32, u32); 16] = [
  (0, 0),
  (0, 1),
  (0, 2),
  (0, 3),
  (3, 0),
  (1, 1),
  (1, 2),
  (1, 3),
  (2, 1),
  (2, 2),
  (2, 3),
  (3, 1),
  (3, 2),
  (3, 3),
  (4, 2),
  (4, 3),
];

const SCALE_FACTOR_MPEG2_NSFB: [[[u32; 4]; 3]; 6] = [
  [[7, 7, 7, 0], [12, 12, 12, 0], [6, 15, 12, 0]],
  [[6, 6, 6, 3], [12, 9, 9, 6], [6, 12, 9, 6]],
  [[8, 8, 5, 0], [15, 12, 9, 0], [6, 18, 9, 0]],
  [[6, 5, 5, 5], [9, 9, 9, 9], [6, 9, 9, 9]],
  [[6, 5, 7, 3], [9, 9, 12, 6], [6, 9, 12, 6]],
  [[11, 10, 0, 0], [18, 18, 0, 0], [15, 18, 0, 0]],
];

const SCALE_FACTOR_BANDS: [(u32, u32); 4] = [(0, 6), (6, 11), (11, 16), (16, 21)];

const LONG_BAND_COUNT: u32 = 22;

const WHOLE_SPECTRUM: u32 = 576;

fn band_start(bands: &[u32; 23], count: u32) -> Infallible<u32> {
  let Ok(position) = index(count);

  Ok(match bands.get(position) {
    Some(&start) => start,
    None => WHOLE_SPECTRUM,
  })
}

struct Regions {
  block_type: BlockType,
  table_select: [u8; 3],
  subblock_gain: [u8; 3],
  region1_start: u32,
  region2_start: u32,
}

fn switched_regions<B: ReadBitsLtr>(
  bs: &mut B,
  version: Version,
  bands: &[u32; 23],
) -> Result<Regions> {
  let block_type_code = bs.field::<u8>(2)?;
  let mixed = bs.read_flag()?;

  let block_type = match (block_type_code, mixed) {
    (0b00, _) => return decode_error("mpa: invalid block_type"),
    (0b01, _) => BlockType::Start,
    (0b10, FlagBit::One) => BlockType::Mixed,
    (0b10, FlagBit::Zero) => BlockType::Short,
    (0b11, _) => BlockType::End,
    (_, _) => return decode_error("mpa: invalid block_type"),
  };

  let first = bs.field::<u8>(5)?;
  let second = bs.field::<u8>(5)?;
  let gain0 = bs.field::<u8>(3)?;
  let gain1 = bs.field::<u8>(3)?;
  let gain2 = bs.field::<u8>(3)?;

  let region1_start = match (version, block_type_code) {
    (Version::Mpeg2p5, _) => {
      let region0_count = match block_type {
        BlockType::Short => 6,
        BlockType::Long | BlockType::Start | BlockType::Mixed | BlockType::End => 8,
      };

      let Ok(start) = band_start(bands, region0_count);

      start
    }
    (Version::Mpeg1, _) | (Version::Mpeg2, 0b10) => 36,
    (Version::Mpeg2, _) => 54,
  };

  Ok(Regions {
    block_type,
    table_select: [first, second, 0],
    subblock_gain: [gain0, gain1, gain2],
    region1_start,
    region2_start: WHOLE_SPECTRUM,
  })
}

fn long_regions<B: ReadBitsLtr>(bs: &mut B, bands: &[u32; 23]) -> Result<Regions> {
  let first = bs.field::<u8>(5)?;
  let second = bs.field::<u8>(5)?;
  let third = bs.field::<u8>(5)?;

  let region0 = bs.field::<u32>(4)?;
  let region1 = bs.field::<u32>(3)?;

  let region0_count = region0.saturating_add(1);
  let region0_1_count = region1.saturating_add(region0_count).saturating_add(1);

  let Ok(region2_start) = match region0_1_count {
    0..=LONG_BAND_COUNT => band_start(bands, region0_1_count),
    _ => Ok(WHOLE_SPECTRUM),
  };
  let Ok(region1_start) = band_start(bands, region0_count);

  Ok(Regions {
    block_type: BlockType::Long,
    table_select: [first, second, third],
    subblock_gain: [0; 3],
    region1_start,
    region2_start,
  })
}

fn granule_channel<B: ReadBitsLtr>(
  bs: &mut B,
  header: &FrameHeader,
  bands: &[u32; 23],
) -> Result<GranuleChannel> {
  let part2_3_length = bs.field::<u16>(12)?;
  let big_values = bs.field::<u16>(9)?;

  match ::core::primitive::bool::then_some(big_values > 288, ()) {
    ::core::option::Option::Some(()) => return decode_error("mpa: granule big_values > 288"),
    ::core::option::Option::None => {}
  }

  let global_gain = bs.field::<u8>(8)?;

  let compress_width = match header.version {
    Version::Mpeg1 => 4,
    Version::Mpeg2 | Version::Mpeg2p5 => 9,
  };

  let scalefac_compress = bs.field::<u16>(compress_width)?;

  let window_switching = bs.read_flag()?;

  let regions = match window_switching {
    FlagBit::One => switched_regions(bs, header.version, bands),
    FlagBit::Zero => long_regions(bs, bands),
  }?;

  let preflag = match header.version {
    Version::Mpeg1 => bs.switch()?,
    Version::Mpeg2 | Version::Mpeg2p5 => Switch::Off,
  };

  let scalefac_scale = bs.switch()?;
  let count1table_select = bs.field::<u8>(1)?;

  Ok(GranuleChannel {
    part2_3_length,
    big_values,
    global_gain,
    scalefac_compress,
    block_type: regions.block_type,
    subblock_gain: regions.subblock_gain,
    table_select: regions.table_select,
    region1_start: regions.region1_start,
    region2_start: regions.region2_start,
    preflag,
    scalefac_scale,
    count1table_select,
    scalefacs: [0; 39],
    rzero: 0,
  })
}

fn second_channel<B: ReadBitsLtr>(
  bs: &mut B,
  header: &FrameHeader,
  bands: &[u32; 23],
) -> Result<GranuleChannel> {
  match header.channel_mode {
    ChannelMode::Mono => Ok(GranuleChannel::default()),
    ChannelMode::DualMono | ChannelMode::Stereo | ChannelMode::JointStereo { .. } => {
      granule_channel(bs, header, bands)
    }
  }
}

fn granule<B: ReadBitsLtr>(
  bs: &mut B,
  header: &FrameHeader,
  bands: &[u32; 23],
) -> Result<Granule> {
  let first = granule_channel(bs, header, bands)?;
  let second = second_channel(bs, header, bands)?;

  Ok(Granule {
    channels: [first, second],
  })
}

fn selectors<B: ReadBitsLtr>(bs: &mut B) -> Result<[ScaleFactorGroup; 4]> {
  let first = bs.read_flag().map(ScaleFactorGroup::from)?;
  let second = bs.read_flag().map(ScaleFactorGroup::from)?;
  let third = bs.read_flag().map(ScaleFactorGroup::from)?;
  let fourth = bs.read_flag().map(ScaleFactorGroup::from)?;

  Ok([first, second, third, fourth])
}

fn second_selectors<B: ReadBitsLtr>(
  bs: &mut B,
  header: &FrameHeader,
) -> Result<[ScaleFactorGroup; 4]> {
  match header.channel_mode {
    ChannelMode::Mono => Ok([ScaleFactorGroup::Read; 4]),
    ChannelMode::DualMono | ChannelMode::Stereo | ChannelMode::JointStereo { .. } => {
      selectors(bs)
    }
  }
}

pub fn read_side_info<B: ReadBitsLtr>(
  bs: &mut B,
  header: &FrameHeader,
  bands: &[u32; 23],
) -> Result<FrameData> {
  match header.version {
    Version::Mpeg1 => {
      let main_data_begin = bs.field::<u16>(9)?;

      let private_bits = match header.channel_mode {
        ChannelMode::Mono => 5,
        ChannelMode::DualMono | ChannelMode::Stereo | ChannelMode::JointStereo { .. } => 3,
      };

      bs.ignore_bits(private_bits)?;

      let first = selectors(bs)?;
      let second = second_selectors(bs, header)?;

      let granule0 = granule(bs, header, bands)?;
      let granule1 = granule(bs, header, bands)?;

      Ok(FrameData {
        main_data_begin,
        scfsi: [first, second],
        granules: [granule0, granule1],
      })
    }
    Version::Mpeg2 | Version::Mpeg2p5 => {
      let main_data_begin = bs.field::<u16>(8)?;

      let private_bits = match header.channel_mode {
        ChannelMode::Mono => 1,
        ChannelMode::DualMono | ChannelMode::Stereo | ChannelMode::JointStereo { .. } => 2,
      };

      bs.ignore_bits(private_bits)?;

      let granule0 = granule(bs, header, bands)?;

      Ok(FrameData {
        main_data_begin,
        scfsi: [[ScaleFactorGroup::Read; 4]; 2],
        granules: [granule0, Granule::default()],
      })
    }
  }
}

fn fill<B: ReadBitsLtr>(
  bs: &mut B,
  channel: &mut GranuleChannel,
  [start, count]: [u32; 2],
  width: u32,
) -> Result<u32> {
  let Ok(skip) = index(start);
  let Ok(take) = index(count);

  match width {
    0 => Ok(0),
    _ => {
      for slot in channel.scalefacs.iter_mut().skip(skip).take(take) {
        let value = bs.field::<u8>(width)?;

        *slot = value;
      }

      Ok(count.saturating_mul(width))
    }
  }
}

fn read_short_mpeg1<B: ReadBitsLtr>(
  bs: &mut B,
  channel: &mut GranuleChannel,
  slen: (u32, u32),
) -> Result<u32> {
  let mixed_short_bands = match channel.block_type {
    BlockType::Mixed => 17,
    BlockType::Long | BlockType::Start | BlockType::Short | BlockType::End => 18,
  };

  let first = fill(bs, channel, [0, mixed_short_bands], slen.0)?;
  let second = fill(bs, channel, [mixed_short_bands, 18], slen.1)?;

  Ok(first.saturating_add(second))
}

fn read_long_mpeg1<B: ReadBitsLtr>(
  bs: &mut B,
  channel: &mut GranuleChannel,
  slen: (u32, u32),
  shared: [ScaleFactorGroup; 4],
  previous: Option<&[u8; 39]>,
) -> Result<u32> {
  let mut bits_read = 0u32;

  for ((&(start, end), &reuse), width) in SCALE_FACTOR_BANDS
    .iter()
    .zip(shared.iter())
    .zip([slen.0, slen.0, slen.1, slen.1])
  {
    let Ok(skip) = index(start);
    let count = end.saturating_sub(start);
    let Ok(take) = index(count);

    let read = match (reuse, previous) {
      (ScaleFactorGroup::Reuse, Some(previous_2)) => {
        let slots = channel
          .scalefacs
          .iter_mut()
          .zip(previous_2.iter())
          .skip(skip)
          .take(take);

        for (slot, &value) in slots {
          *slot = value;
        }

        Ok(0)
      }
      (ScaleFactorGroup::Read, Some(_) | None) | (ScaleFactorGroup::Reuse, None) => {
        fill(bs, channel, [start, count], width)
      }
    }?;

    bits_read = bits_read.saturating_add(read);
  }

  Ok(bits_read)
}

pub fn read_scale_factors_mpeg1<B: ReadBitsLtr>(
  bs: &mut B,
  channel: &mut GranuleChannel,
  shared: [ScaleFactorGroup; 4],
  previous: Option<&[u8; 39]>,
) -> Result<u32> {
  let Ok(position) = index(channel.scalefac_compress);

  let slen = match SCALE_FACTOR_SLEN.get(position) {
    Some(&slen) => slen,
    None => return decode_error("mpa: invalid scalefac_compress"),
  };

  match channel.block_type {
    BlockType::Short | BlockType::Mixed => read_short_mpeg1(bs, channel, slen),
    BlockType::Long | BlockType::Start | BlockType::End => {
      read_long_mpeg1(bs, channel, slen, shared, previous)
    }
  }
}

fn mpeg2_layout(channel: &GranuleChannel, intensity: Switch) -> Result<([u32; 4], [u32; 4])> {
  let block_index: u32 = match channel.block_type {
    BlockType::Mixed => 2,
    BlockType::Short => 1,
    BlockType::Long | BlockType::Start | BlockType::End => 0,
  };

  let sfc = u32::from(channel.scalefac_compress);

  let (slen, table): ([u32; 4], u32) = match (intensity, sfc.wrapping_shr(1), sfc) {
    (Switch::On, half @ 0..=179, _) => {
      let rest = half.rem_euclid(36);

      (
        [
          half.div_euclid(36),
          rest.div_euclid(6),
          rest.rem_euclid(6),
          0,
        ],
        0,
      )
    }
    (Switch::On, half @ 180..=243, _) => {
      let above = half.saturating_sub(180);

      (
        [
          above.rem_euclid(64).wrapping_shr(4),
          above.rem_euclid(16).wrapping_shr(2),
          above.rem_euclid(4),
          0,
        ],
        1,
      )
    }
    (Switch::On, half @ 244..=255, _) => {
      let above = half.saturating_sub(244);

      ([above.div_euclid(3), above.rem_euclid(3), 0, 0], 2)
    }
    (Switch::Off, _, whole @ 0..=399) => {
      let high = whole.wrapping_shr(4);

      (
        [
          high.div_euclid(5),
          high.rem_euclid(5),
          whole.rem_euclid(16).wrapping_shr(2),
          whole.rem_euclid(4),
        ],
        3,
      )
    }
    (Switch::Off, _, whole @ 400..=499) => {
      let above = whole.saturating_sub(400);
      let high = above.wrapping_shr(2);

      (
        [
          high.div_euclid(5),
          high.rem_euclid(5),
          above.rem_euclid(4),
          0,
        ],
        4,
      )
    }
    (Switch::Off, _, whole @ 500..=512) => {
      let above = whole.saturating_sub(500);

      ([above.div_euclid(3), above.rem_euclid(3), 0, 0], 5)
    }
    (Switch::On | Switch::Off, _, _) => return decode_error("mpa: invalid scalefac_compress"),
  };

  let Ok(table_2) = index(table);
  let Ok(block_index_2) = index(block_index);

  let nsfb = SCALE_FACTOR_MPEG2_NSFB
    .get(table_2)
    .and_then(|row| row.get(block_index_2));

  match nsfb {
    Some(&nsfb_2) => Ok((slen, nsfb_2)),
    None => decode_error("mpa: invalid scalefac_compress"),
  }
}

pub fn read_scale_factors_mpeg2<B: ReadBitsLtr>(
  bs: &mut B,
  channel: &mut GranuleChannel,
  intensity: Switch,
) -> Result<u32> {
  let (slen, nsfb) = mpeg2_layout(channel, intensity)?;

  match intensity {
    Switch::On => {}
    Switch::Off => {
      channel.preflag = match ::core::primitive::bool::then_some(channel.scalefac_compress >= 500, ()) {
        ::core::option::Option::Some(()) => Switch::On,
        ::core::option::Option::None => Switch::Off,
      }
    }
  }

  let mut bits_read = 0u32;
  let mut start = 0u32;

  for (&width, &count) in slen.iter().zip(nsfb.iter()) {
    let read = fill(bs, channel, [start, count], width)?;

    bits_read = bits_read.saturating_add(read);
    start = start.saturating_add(count);
  }

  Ok(bits_read)
}
