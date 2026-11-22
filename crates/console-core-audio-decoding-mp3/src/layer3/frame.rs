//! What a frame says about its granules, before any of it is decoded.
//!
//! A frame holds one granule (MPEG 2 and 2.5) or two (MPEG 1), and each granule
//! holds one set of fields per channel. The fields are read from the side
//! information, and the scale factors are added to them once the main data has
//! been found, which is why a [`GranuleChannel`] starts with none.

use console_core_audio_decoding_interfaces::errors::Result;
use console_core_byte_streams::{FlagBit, ReadBitsLtr};

use crate::common::Switch;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BlockType {
  Long,
  Start,
  Short,
  Mixed,
  End,
}

#[derive(Clone, Debug)]
pub struct GranuleChannel {
  pub part2_3_length: u16,
  pub big_values: u16,
  pub global_gain: u8,
  pub scalefac_compress: u16,
  pub block_type: BlockType,
  pub subblock_gain: [u8; 3],
  pub table_select: [u8; 3],
  pub region1_start: u32,
  pub region2_start: u32,
  pub preflag: Switch,
  pub scalefac_scale: Switch,
  pub count1table_select: u8,
  pub scalefacs: [u8; 39],
  pub rzero: u32,
}

impl Default for GranuleChannel {
  fn default() -> Self {
    GranuleChannel {
      part2_3_length: 0,
      big_values: 0,
      global_gain: 0,
      scalefac_compress: 0,
      block_type: BlockType::Long,
      subblock_gain: [0; 3],
      table_select: [0; 3],
      region1_start: 0,
      region2_start: 0,
      preflag: Switch::Off,
      scalefac_scale: Switch::Off,
      count1table_select: 0,
      scalefacs: [0; 39],
      rzero: 0,
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct Granule {
  pub channels: [GranuleChannel; 2],
}

#[derive(Clone, Copy, Debug, Default)]
pub enum ScaleFactorGroup {
  #[default]
  Read,
  Reuse,
}

impl From<FlagBit> for ScaleFactorGroup {
  fn from(flag: FlagBit) -> Self {
    match flag {
      FlagBit::Zero => Self::Read,
      FlagBit::One => Self::Reuse,
    }
  }
}

#[derive(Debug, Default)]
pub struct FrameData {
  pub main_data_begin: u16,
  pub scfsi: [[ScaleFactorGroup; 4]; 2],
  pub granules: [Granule; 2],
}

pub trait Switches: ReadBitsLtr {
  fn switch(&mut self) -> Result<Switch> {
    let bit = self.read_flag()?;

    Ok(match bit {
      FlagBit::One => Switch::On,
      FlagBit::Zero => Switch::Off,
    })
  }
}

impl<B: ReadBitsLtr> Switches for B {}
