//! The scalefactor band edges every sampling rate cuts a spectrum at, and the
//! noise that fills the bands that carry none.
//!
//! ISO/IEC 14496-3 tables 4.110 to 4.126: a long window is 1024 lines cut at
//! one list of edges, and each of eight short windows is 128 lines cut at
//! another, both chosen by the sampling rate. The rate is looked up by the
//! lowest rate each pair of lists serves, which is how the decoder this was
//! carried from kept it, so a stream that says 44100 and one that says 44099
//! land on the same bands.

use crate::lane::Sure;
use console_core_number_conversion::fitted;

pub(crate) type Windows<T> = [T; 8];

pub(crate) type Bands<T> = [T; 64];

pub(crate) type Scales = Windows<Bands<f32>>;

pub(crate) type Spectrum = [f32; 1024];

const SWB_OFFSET_48K_LONG: [u32; 49 + 1] = [
  0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72, 80, 88, 96, 108, 120, 132, 144, 160,
  176, 196, 216, 240, 264, 292, 320, 352, 384, 416, 448, 480, 512, 544, 576, 608, 640, 672, 704,
  736, 768, 800, 832, 864, 896, 928, 1024,
];

const SWB_OFFSET_48K_SHORT: [u32; 14 + 1] =
  [0, 4, 8, 12, 16, 20, 28, 36, 44, 56, 68, 80, 96, 112, 128];

const SWB_OFFSET_32K_LONG: [u32; 51 + 1] = [
  0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72, 80, 88, 96, 108, 120, 132, 144, 160,
  176, 196, 216, 240, 264, 292, 320, 352, 384, 416, 448, 480, 512, 544, 576, 608, 640, 672, 704,
  736, 768, 800, 832, 864, 896, 928, 960, 992, 1024,
];

const SWB_OFFSET_8K_LONG: [u32; 40 + 1] = [
  0, 12, 24, 36, 48, 60, 72, 84, 96, 108, 120, 132, 144, 156, 172, 188, 204, 220, 236, 252, 268,
  288, 308, 328, 348, 372, 396, 420, 448, 476, 508, 544, 580, 620, 664, 712, 764, 820, 880, 944,
  1024,
];

const SWB_OFFSET_8K_SHORT: [u32; 15 + 1] = [
  0, 4, 8, 12, 16, 20, 24, 28, 36, 44, 52, 60, 72, 88, 108, 128,
];

const SWB_OFFSET_16K_LONG: [u32; 43 + 1] = [
  0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 100, 112, 124, 136, 148, 160, 172, 184, 196, 212,
  228, 244, 260, 280, 300, 320, 344, 368, 396, 424, 456, 492, 532, 572, 616, 664, 716, 772, 832,
  896, 960, 1024,
];

const SWB_OFFSET_16K_SHORT: [u32; 15 + 1] = [
  0, 4, 8, 12, 16, 20, 24, 28, 32, 40, 48, 60, 72, 88, 108, 128,
];

const SWB_OFFSET_24K_LONG: [u32; 47 + 1] = [
  0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 52, 60, 68, 76, 84, 92, 100, 108, 116, 124, 136,
  148, 160, 172, 188, 204, 220, 240, 260, 284, 308, 336, 364, 396, 432, 468, 508, 552, 600, 652,
  704, 768, 832, 896, 960, 1024,
];

const SWB_OFFSET_24K_SHORT: [u32; 15 + 1] = [
  0, 4, 8, 12, 16, 20, 24, 28, 36, 44, 52, 64, 76, 92, 108, 128,
];

const SWB_OFFSET_64K_LONG: [u32; 47 + 1] = [
  0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 64, 72, 80, 88, 100, 112, 124, 140,
  156, 172, 192, 216, 240, 268, 304, 344, 384, 424, 464, 504, 544, 584, 624, 664, 704, 744, 784,
  824, 864, 904, 944, 984, 1024,
];

const SWB_OFFSET_64K_SHORT: [u32; 12 + 1] = [0, 4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 92, 128];

const SWB_OFFSET_96K_LONG: [u32; 41 + 1] = [
  0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 64, 72, 80, 88, 96, 108, 120, 132,
  144, 156, 172, 188, 212, 240, 276, 320, 384, 448, 512, 576, 640, 704, 768, 832, 896, 960, 1024,
];

#[derive(Clone)]
pub(crate) struct Lcg {
  state: u32,
}

impl Lcg {
  pub(crate) fn new(state: u32) -> Sure<Self> {
    Ok(Lcg { state })
  }

  pub(crate) fn draw(&mut self) -> Sure<i32> {
    self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);

    Ok(i32::from_le_bytes(self.state.to_le_bytes()))
  }

  pub(crate) fn sample(&mut self) -> Sure<i16> {
    let Ok(drawn) = self.draw();

    fitted::<i32, i16>(drawn.wrapping_shr(16))
  }
}

#[derive(Clone, Copy)]
pub(crate) struct Subbands {
  pub(crate) rate: u32,
  min_rate: u32,
  pub(crate) long: &'static [u32],
  pub(crate) short: &'static [u32],
}

impl Subbands {
  pub(crate) fn find(rate: u32) -> Sure<Subbands> {
    Ok(
      match SUBBANDS.iter().find(|subbands| rate >= subbands.min_rate) {
        Some(found) => *found,
        None => LOWEST,
      },
    )
  }
}

const LOWEST: Subbands = Subbands {
  rate: 11,
  min_rate: 0,
  long: &SWB_OFFSET_8K_LONG,
  short: &SWB_OFFSET_8K_SHORT,
};

const SUBBANDS: [Subbands; 12] = [
  Subbands {
    rate: 0,
    min_rate: 92017,
    long: &SWB_OFFSET_96K_LONG,
    short: &SWB_OFFSET_64K_SHORT,
  },
  Subbands {
    rate: 1,
    min_rate: 75132,
    long: &SWB_OFFSET_96K_LONG,
    short: &SWB_OFFSET_64K_SHORT,
  },
  Subbands {
    rate: 2,
    min_rate: 55426,
    long: &SWB_OFFSET_64K_LONG,
    short: &SWB_OFFSET_64K_SHORT,
  },
  Subbands {
    rate: 3,
    min_rate: 46009,
    long: &SWB_OFFSET_48K_LONG,
    short: &SWB_OFFSET_48K_SHORT,
  },
  Subbands {
    rate: 4,
    min_rate: 37566,
    long: &SWB_OFFSET_48K_LONG,
    short: &SWB_OFFSET_48K_SHORT,
  },
  Subbands {
    rate: 5,
    min_rate: 27713,
    long: &SWB_OFFSET_32K_LONG,
    short: &SWB_OFFSET_48K_SHORT,
  },
  Subbands {
    rate: 6,
    min_rate: 23004,
    long: &SWB_OFFSET_24K_LONG,
    short: &SWB_OFFSET_24K_SHORT,
  },
  Subbands {
    rate: 7,
    min_rate: 18783,
    long: &SWB_OFFSET_24K_LONG,
    short: &SWB_OFFSET_24K_SHORT,
  },
  Subbands {
    rate: 8,
    min_rate: 13856,
    long: &SWB_OFFSET_16K_LONG,
    short: &SWB_OFFSET_16K_SHORT,
  },
  Subbands {
    rate: 9,
    min_rate: 11502,
    long: &SWB_OFFSET_16K_LONG,
    short: &SWB_OFFSET_16K_SHORT,
  },
  Subbands {
    rate: 10,
    min_rate: 9391,
    long: &SWB_OFFSET_16K_LONG,
    short: &SWB_OFFSET_16K_SHORT,
  },
  LOWEST,
];
