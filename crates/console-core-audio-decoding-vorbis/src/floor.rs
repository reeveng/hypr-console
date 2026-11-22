//! A floor: the coarse outline of a channel's spectrum, which the residue
//! then adds the detail to.
//!
//! A stream holds floors of two kinds. Type 0 describes the outline with a
//! line spectral pair and is rare; type 1 describes it with points joined by
//! lines and is what every common encoder writes. Upstream held them as trait
//! objects; there are two of them, so they are an enum, and what a packet that
//! ends early does to one is said once in `read_channel`.

use console_core_audio_decoding_interfaces::errors::{Error, decode_error};
use console_core_byte_streams::BitReaderRtl;
use console_core_never::Never;

use crate::codebook::VorbisCodebook;
use crate::dsp::Decoding;
use crate::floor0::Floor0;
use crate::floor1::Floor1;
use crate::reading::Fields;
use crate::window::{Block, Blocks};

pub(crate) enum Floor {
  Zero(Floor0),
  One(Floor1),
}

impl Floor {
  pub(crate) fn read(
    bs: &mut BitReaderRtl<'_>,
    blocks: Blocks,
    codebooks: u16,
  ) -> Result<Floor, Error> {
    let floor_type = bs.bits_u16(16)?;

    match floor_type {
      0 => {
        let floor = Floor0::read(bs, blocks, codebooks)?;

        Ok(Floor::Zero(floor))
      }
      1 => {
        let floor = Floor1::read(bs, codebooks)?;

        Ok(Floor::One(floor))
      }
      _ => decode_error("vorbis: invalid floor type"),
    }
  }

  pub(crate) fn read_channel(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    codebooks: &[VorbisCodebook],
  ) -> Result<(), Error> {
    match self {
      Floor::Zero(floor) => floor.read_channel(bs, codebooks),
      Floor::One(floor) => floor.read_channel(bs, codebooks),
    }
  }

  pub(crate) fn decoding(&self) -> Result<Decoding, Never> {
    Ok(match self {
      Floor::Zero(floor) => floor.decoding,
      Floor::One(floor) => floor.decoding,
    })
  }

  pub(crate) fn synthesis(
    &mut self,
    block: Block,
    half: u32,
    floor: &mut [f32],
  ) -> Result<(), Error> {
    match self {
      Floor::Zero(zero) => zero.synthesis(block, half, floor),
      Floor::One(one) => one.synthesis(half, floor),
    }
  }
}
