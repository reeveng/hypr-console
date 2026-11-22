//! The two headers a stream opens with, and what they declare: how many
//! channels there are at what rate, the two block sizes, and the codebooks,
//! floors, residues, mappings and modes every audio packet is read with.
//!
//! In an Ogg stream the headers arrive as packets of their own; by the time
//! they reach here the demuxer has put the identification header and the setup
//! header together as the codec's extra data, and this reads them in that
//! order. The setup header after its first seven bytes is a stream of bits, so
//! everything in it is read a bit at a time and each count it states is
//! taken as one more than the number written, which is how the format says
//! "at least one".

use std::cmp::Ordering;

use console_core_audio_decoding_interfaces::errors::{Error, decode_error, unsupported_error};
use console_core_byte_streams::{
  BitReaderRtl, BufReader, FiniteBitStream, FlagBit, ReadBitsRtl, ReadBytes,
};
use console_core_logging::debug;
use console_core_number_conversion::fitted;

use crate::codebook::{VorbisCodebook, ilog};
use crate::floor::Floor;
use crate::reading::Fields;
use crate::residue::Residue;
use crate::window::{BLOCKSIZE_MAX, Block, Blocks};

const PACKET_TYPE_IDENTIFICATION: u8 = 1;

const PACKET_TYPE_SETUP: u8 = 5;

const HEADER_PACKET_SIGNATURE: [u8; 6] = *b"vorbis";

const VERSION: u32 = 0;

const BLOCKSIZE_MIN: u8 = 6;

const MOST_CHANNELS: u8 = 32;

#[derive(Debug)]
pub(crate) struct IdentHeader {
  pub(crate) n_channels: u8,
  pub(crate) sample_rate: u32,
  pub(crate) blocks: Blocks,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Coupling {
  pub(crate) magnitude: u8,
  pub(crate) angle: u8,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Submap {
  pub(crate) floor: u8,
  pub(crate) residue: u8,
}

#[derive(Debug)]
pub(crate) struct Mapping {
  pub(crate) couplings: Vec<Coupling>,
  pub(crate) multiplex: Vec<u8>,
  pub(crate) submaps: Vec<Submap>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Mode {
  pub(crate) block: Block,
  pub(crate) mapping: u8,
}

pub(crate) struct Setup {
  pub(crate) codebooks: Vec<VorbisCodebook>,
  pub(crate) floors: Vec<Floor>,
  pub(crate) residues: Vec<Residue>,
  pub(crate) mappings: Vec<Mapping>,
  pub(crate) modes: Vec<Mode>,
}

#[derive(Debug, Clone, Copy)]
struct Limits {
  channels: u8,
  floors: u8,
  residues: u8,
}

pub(crate) fn read_ident_header<B: ReadBytes>(reader: &mut B) -> Result<IdentHeader, Error> {
  let packet_type = reader.read_u8()?;

  match packet_type {
    PACKET_TYPE_IDENTIFICATION => {}
    _other => return decode_error("vorbis: invalid packet type for identification header"),
  }

  let mut signature = [0; 6];

  reader.read_buf_exact(&mut signature)?;

  match signature {
    HEADER_PACKET_SIGNATURE => {}
    _other => return decode_error("vorbis: invalid header signature"),
  }

  let version = reader.read_u32()?;

  match version {
    VERSION => {}
    _unsupported => return unsupported_error("vorbis: only vorbis 1 is supported"),
  }

  let n_channels = reader.read_u8()?;

  match n_channels {
    0 => return decode_error("vorbis: number of channels cannot be 0"),
    1..=MOST_CHANNELS => {}
    _unsupported => {
      return unsupported_error("vorbis: only a maximum of 32 channels are supported");
    }
  }

  let sample_rate = reader.read_u32()?;

  match sample_rate {
    0 => return decode_error("vorbis: sample rate cannot be 0"),
    _ => {}
  }

  let _bitrate_max = reader.read_u32()?;
  let _bitrate_nom = reader.read_u32()?;
  let _bitrate_min = reader.read_u32()?;
  let block_sizes = reader.read_u8()?;
  let blocks = Blocks {
    short: block_sizes & 0x0f,
    long: block_sizes.wrapping_shr(4),
  };

  match blocks.short {
    BLOCKSIZE_MIN..=BLOCKSIZE_MAX => {}
    _outside => return decode_error("vorbis: blocksize_0 out-of-bounds"),
  }

  match blocks.long {
    BLOCKSIZE_MIN..=BLOCKSIZE_MAX => {}
    _outside => return decode_error("vorbis: blocksize_1 out-of-bounds"),
  }

  match blocks.short.cmp(&blocks.long) {
    Ordering::Greater => return decode_error("vorbis: blocksize_0 exceeds blocksize_1"),
    Ordering::Less | Ordering::Equal => {}
  }

  let framing = reader.read_u8()?;

  match framing {
    1 => Ok(IdentHeader {
      n_channels,
      sample_rate,
      blocks,
    }),
    _ => decode_error("vorbis: ident header framing flag unset"),
  }
}

fn read_codebooks(bs: &mut BitReaderRtl<'_>) -> Result<Vec<VorbisCodebook>, Error> {
  let count_raw = bs.read_bits_leq32(8)?;
  let count = count_raw.saturating_add(1);
  let mut codebooks = Vec::new();

  for _ in 0..count {
    let codebook = VorbisCodebook::read(bs)?;

    codebooks.push(codebook);
  }

  Ok(codebooks)
}

fn read_time_domain_transforms(bs: &mut BitReaderRtl<'_>) -> Result<(), Error> {
  let count_raw = bs.read_bits_leq32(6)?;
  let count = count_raw.saturating_add(1);

  for _ in 0..count {
    let placeholder = bs.read_bits_leq32(16)?;

    match placeholder {
      0 => {}
      _ => return decode_error("vorbis: invalid time domain tranform"),
    }
  }

  Ok(())
}

fn read_floors(
  bs: &mut BitReaderRtl<'_>,
  blocks: Blocks,
  codebooks: u16,
) -> Result<Vec<Floor>, Error> {
  let count_raw = bs.read_bits_leq32(6)?;
  let count = count_raw.saturating_add(1);
  let mut floors = Vec::new();

  for _ in 0..count {
    let floor = Floor::read(bs, blocks, codebooks)?;

    floors.push(floor);
  }

  Ok(floors)
}

fn read_residues(bs: &mut BitReaderRtl<'_>, codebooks: u16) -> Result<Vec<Residue>, Error> {
  let count_raw = bs.read_bits_leq32(6)?;
  let count = count_raw.saturating_add(1);
  let mut residues = Vec::new();

  for _ in 0..count {
    let residue = Residue::read(bs, codebooks)?;

    residues.push(residue);
  }

  Ok(residues)
}

fn read_mappings(bs: &mut BitReaderRtl<'_>, limits: Limits) -> Result<Vec<Mapping>, Error> {
  let count_raw = bs.read_bits_leq32(6)?;
  let count = count_raw.saturating_add(1);
  let mut mappings = Vec::new();

  for _ in 0..count {
    let mapping_type = bs.read_bits_leq32(16)?;

    match mapping_type {
      0 => {}
      _ => return decode_error("vorbis: invalid mapping type"),
    }

    let mapping = read_mapping(bs, limits)?;

    mappings.push(mapping);
  }

  Ok(mappings)
}

fn read_couplings(bs: &mut BitReaderRtl<'_>, channels: u8) -> Result<Vec<Coupling>, Error> {
  let coupled = bs.read_flag()?;
  let mut couplings = Vec::new();

  match coupled {
    FlagBit::Zero => return Ok(couplings),
    FlagBit::One => {}
  }

  let steps_raw = bs.read_bits_leq32(8)?;
  let steps = steps_raw.saturating_add(1);
  let most = channels.saturating_sub(1);
  let Ok(width) = ilog(u32::from(most));

  for _ in 0..steps {
    let magnitude = bs.bits_u8(width)?;
    let angle = bs.bits_u8(width)?;

    match (magnitude.cmp(&angle), magnitude.max(angle).cmp(&most)) {
      (Ordering::Equal, _) | (_, Ordering::Greater) => {
        return decode_error("vorbis: invalid channel coupling");
      }
      (Ordering::Less | Ordering::Greater, Ordering::Less | Ordering::Equal) => {
        couplings.push(Coupling { magnitude, angle });
      }
    }
  }

  Ok(couplings)
}

fn read_multiplex(
  bs: &mut BitReaderRtl<'_>,
  limits: Limits,
  submaps: u8,
) -> Result<Vec<u8>, Error> {
  let mut multiplex = Vec::new();

  for _ in 0..limits.channels {
    let mux = match submaps {
      0 | 1 => 0,
      _several => bs.bits_u8(4)?,
    };

    match mux.cmp(&submaps) {
      Ordering::Equal | Ordering::Greater => {
        return decode_error("vorbis: invalid channel multiplex");
      }
      Ordering::Less => multiplex.push(mux),
    }
  }

  Ok(multiplex)
}

fn read_mapping(bs: &mut BitReaderRtl<'_>, limits: Limits) -> Result<Mapping, Error> {
  let submap_count = bs.read_flag()?;
  let listed = match submap_count {
    FlagBit::One => bs.bits_u8(4)?,
    FlagBit::Zero => 0,
  };
  let num_submaps = listed.saturating_add(1);
  let couplings = read_couplings(bs, limits.channels)?;
  let reserved = bs.read_bits_leq32(2)?;

  match reserved {
    0 => {}
    _ => return decode_error("vorbis: reserved mapping bits non-zero"),
  }

  let multiplex = read_multiplex(bs, limits, num_submaps)?;
  let mut submaps = Vec::new();

  for _ in 0..num_submaps {
    let _unused = bs.read_bits_leq32(8)?;
    let floor = bs.bits_u8(8)?;

    match floor.cmp(&limits.floors) {
      Ordering::Equal | Ordering::Greater => {
        return decode_error("vorbis: invalid floor for mapping");
      }
      Ordering::Less => {}
    }

    let residue = bs.bits_u8(8)?;

    match residue.cmp(&limits.residues) {
      Ordering::Equal | Ordering::Greater => {
        return decode_error("vorbis: invalid residue for mapping");
      }
      Ordering::Less => {}
    }

    submaps.push(Submap { floor, residue });
  }

  Ok(Mapping {
    couplings,
    multiplex,
    submaps,
  })
}

fn read_modes(bs: &mut BitReaderRtl<'_>, mappings: u8) -> Result<Vec<Mode>, Error> {
  let count_raw = bs.read_bits_leq32(6)?;
  let count = count_raw.saturating_add(1);
  let mut modes = Vec::new();

  for _ in 0..count {
    let long_block_flag = bs.read_flag()?;
    let block = match long_block_flag {
      FlagBit::One => Block::Long,
      FlagBit::Zero => Block::Short,
    };
    let window_type = bs.bits_u16(16)?;
    let transform_type = bs.bits_u16(16)?;
    let mapping = bs.bits_u8(8)?;

    match window_type {
      0 => {}
      _ => return decode_error("vorbis: invalid window type for mode"),
    }

    match transform_type {
      0 => {}
      _ => return decode_error("vorbis: invalid transform type for mode"),
    }

    match mapping.cmp(&mappings) {
      Ordering::Equal | Ordering::Greater => return decode_error("vorbis: invalid mode mapping"),
      Ordering::Less => {}
    }

    modes.push(Mode { block, mapping });
  }

  Ok(modes)
}

pub(crate) fn read_setup(
  reader: &mut BufReader<'_>,
  ident: &IdentHeader,
) -> Result<Setup, Error> {
  let packet_type = reader.read_u8()?;

  match packet_type {
    PACKET_TYPE_SETUP => {}
    _other => return decode_error("vorbis: invalid packet type for setup header"),
  }

  let mut signature = [0; 6];

  reader.read_buf_exact(&mut signature)?;

  match signature {
    HEADER_PACKET_SIGNATURE => {}
    _other => return decode_error("vorbis: invalid setup header signature"),
  }

  let Ok(available) = reader.read_buf_bytes_available_ref();
  let Ok(mut bs) = BitReaderRtl::new(available);
  let codebooks = read_codebooks(&mut bs)?;

  read_time_domain_transforms(&mut bs)?;

  let Ok(books) = fitted::<_, u16>(codebooks.len());
  let floors = read_floors(&mut bs, ident.blocks, books)?;
  let residues = read_residues(&mut bs, books)?;
  let Ok(floor_count) = fitted::<_, u8>(floors.len());
  let Ok(residue_count) = fitted::<_, u8>(residues.len());
  let limits = Limits {
    channels: ident.n_channels,
    floors: floor_count,
    residues: residue_count,
  };
  let mappings = read_mappings(&mut bs, limits)?;
  let Ok(mapping_count) = fitted::<_, u8>(mappings.len());
  let modes = read_modes(&mut bs, mapping_count)?;
  let framing = bs.read_flag()?;

  match framing {
    FlagBit::One => {}
    FlagBit::Zero => return decode_error("vorbis: setup header framing flag unset"),
  }

  match bs.bits_left() {
    0 => {}
    _ => debug!("vorbis: leftover bits in setup head extra data"),
  }

  Ok(Setup {
    codebooks,
    floors,
    residues,
    mappings,
    modes,
  })
}
