//! The Vorbis setup header, walked only far enough to learn which block size each mode uses.
//!
//! A mapping times an audio packet from its mode number, and the modes are the last thing the
//! setup header lists, behind the codebooks, floors, residues and mappings. Nothing of those is
//! kept: every field is read for how many bits it takes, and the decoder beside this crate reads
//! the same header again for what they hold.

use console_core_audio_decoding_interfaces::errors::{Error, Result, decode_error};
use console_core_byte_streams::{BitReaderRtl, BufReader, FlagBit, ReadBitsRtl, ReadBytes};
use console_core_iteration::Step;
use console_core_number_conversion::fitted;

use super::vorbis::VORBIS_HEADER_PACKET_SIGNATURE;
use crate::stepping::{Certain, Stepped, walk};

const VORBIS_PACKET_TYPE_SETUP: u8 = 5;

const CODEBOOK_SYNC: u32 = 0x564342;

const FLOOR0_FIXED_BITS: u32 = 8 + 16 + 16 + 6 + 8;

const RESIDUE_FIXED_BITS: u32 = 24 + 24 + 24;

const SUBMAP_BITS: u32 = 8 + 8 + 8;

pub enum Block {
  Short,
  Long,
}

impl From<FlagBit> for Block {
  fn from(long: FlagBit) -> Self {
    match long {
      FlagBit::One => Self::Long,
      FlagBit::Zero => Self::Short,
    }
  }
}

enum OptionalField {
  Encoded,
  Omitted,
}

impl From<FlagBit> for OptionalField {
  fn from(encoded: FlagBit) -> Self {
    match encoded {
      FlagBit::One => Self::Encoded,
      FlagBit::Zero => Self::Omitted,
    }
  }
}

enum CodebookOrder {
  Ordered,
  Unordered,
}

impl From<FlagBit> for CodebookOrder {
  fn from(ordered: FlagBit) -> Self {
    match ordered {
      FlagBit::One => Self::Ordered,
      FlagBit::Zero => Self::Unordered,
    }
  }
}

enum CodebookLayout {
  Sparse,
  Dense,
}

impl From<FlagBit> for CodebookLayout {
  fn from(sparse: FlagBit) -> Self {
    match sparse {
      FlagBit::One => Self::Sparse,
      FlagBit::Zero => Self::Dense,
    }
  }
}

pub fn ilog(x: u32) -> Certain<u32> {
  Ok(32u32.saturating_sub(x.leading_zeros()))
}

fn count_of(bs: &mut BitReaderRtl<'_>, bits: u32) -> Result<u32> {
  let counted = bs.read_bits_leq32(bits)?;

  Ok(counted.saturating_add(1))
}

pub fn read_blocks(packet: &[u8], channels: u8) -> Result<Vec<Block>> {
  let Ok(mut reader) = BufReader::new(packet);
  let packet_type = reader.read_u8()?;

  match packet_type {
    VORBIS_PACKET_TYPE_SETUP => {}
    _other => return decode_error("ogg (vorbis): invalid packet type for setup header"),
  }

  let mut packet_sig_buf = [0; 6];

  reader.read_buf_exact(&mut packet_sig_buf)?;

  match packet_sig_buf {
    VORBIS_HEADER_PACKET_SIGNATURE => {}
    _other => return decode_error("ogg (vorbis): invalid setup header signature"),
  }

  let Ok(available) = reader.read_buf_bytes_available_ref();
  let Ok(mut bs) = BitReaderRtl::new(available);

  skip_codebooks(&mut bs)?;
  skip_time_domain_transforms(&mut bs)?;
  skip_floors(&mut bs)?;
  skip_residues(&mut bs)?;
  skip_mappings(&mut bs, channels)?;

  let blocks = read_modes(&mut bs)?;
  let framing = bs.read_flag()?;

  match framing {
    FlagBit::One => Ok(blocks),
    FlagBit::Zero => decode_error("ogg (vorbis): setup header framing flag unset"),
  }
}

fn skip_codebooks(bs: &mut BitReaderRtl<'_>) -> Result<()> {
  let count = count_of(bs, 8)?;

  for _ in 0..count {
    skip_codebook(bs)?;
  }

  Ok(())
}

fn skip_sparse_entries(bs: &mut BitReaderRtl<'_>, entries: u32) -> Result<()> {
  for _ in 0..entries {
    let present = bs.read_flag().map(OptionalField::from)?;

    match present {
      OptionalField::Encoded => {
        let _length = bs.read_bits_leq32(5)?;
      }
      OptionalField::Omitted => {}
    }
  }

  Ok(())
}

fn skip_unordered_entries(bs: &mut BitReaderRtl<'_>, entries: u32) -> Result<()> {
  let layout = bs.read_flag().map(CodebookLayout::from)?;

  match layout {
    CodebookLayout::Sparse => skip_sparse_entries(bs, entries),
    CodebookLayout::Dense => bs
      .ignore_bits(entries.saturating_mul(5))
      .map_err(Error::from),
  }
}

type Ordered<'a, 'b> = (&'a mut BitReaderRtl<'b>, u32, u32);

fn ordered_step<'a, 'b>(
  (bs, entries, cur_entry): Ordered<'a, 'b>,
) -> Stepped<Ordered<'a, 'b>, ()> {
  let Ok(num_bits) = ilog(entries.saturating_sub(cur_entry));

  Ok(match bs.read_bits_leq32(num_bits) {
    Ok(num) => {
      let next = cur_entry.saturating_add(num);

      match next.cmp(&entries) {
        std::cmp::Ordering::Greater => Step::Halt(decode_error("ogg (vorbis): invalid codebook")),
        std::cmp::Ordering::Equal => Step::Halt(Ok(())),
        std::cmp::Ordering::Less => Step::Again((bs, entries, next)),
      }
    }
    Err(fault) => Step::Halt(Err(Error::from(fault))),
  })
}

fn skip_ordered_entries(bs: &mut BitReaderRtl<'_>, entries: u32) -> Result<()> {
  let _first_length = bs.read_bits_leq32(5)?;

  walk((bs, entries, 0), ordered_step)
}

fn lookup1_values(entries: u32, dimensions: u16) -> Result<u32> {
  match dimensions {
    0 => decode_error("ogg (vorbis): codebook of no dimensions"),
    1 => Ok(entries),
    dimensions_2 => {
      let roots = (1u32..).take_while(|root| match root.checked_pow(u32::from(dimensions_2)) {
        Some(power) => power <= entries,
        None => false,
      });
      let Ok(found) = fitted::<_, u32>(roots.count());

      Ok(found)
    }
  }
}

fn skip_lookup_table(bs: &mut BitReaderRtl<'_>, entries: u32, dimensions: u16) -> Result<()> {
  let lookup_type = bs.read_bits_leq32(4)?;

  match lookup_type & 0xf {
    0 => Ok(()),
    1 => {
      let values = lookup1_values(entries, dimensions)?;

      skip_multiplicands(bs, values)
    }
    2 => skip_multiplicands(bs, entries.saturating_mul(u32::from(dimensions))),
    _invalid => decode_error("ogg (vorbis): invalid codeword lookup type"),
  }
}

fn skip_multiplicands(bs: &mut BitReaderRtl<'_>, lookup_values: u32) -> Result<()> {
  let _min_value = bs.read_bits_leq32(32)?;
  let _delta_value = bs.read_bits_leq32(32)?;
  let value_bits = count_of(bs, 4)?;
  let _sequence = bs.read_flag()?;

  bs.ignore_bits(lookup_values.saturating_mul(value_bits))
    .map_err(Error::from)
}

fn skip_codebook(bs: &mut BitReaderRtl<'_>) -> Result<()> {
  let sync = bs.read_bits_leq32(24)?;

  match sync {
    CODEBOOK_SYNC => {}
    _other => return decode_error("ogg (vorbis): invalid codebook sync"),
  }

  let dimensions = bs.read_bits_leq32(16)?;
  let Ok(codebook_dimensions) = fitted::<u32, u16>(dimensions);
  let codebook_entries = bs.read_bits_leq32(24)?;
  let order = bs.read_flag().map(CodebookOrder::from)?;

  match order {
    CodebookOrder::Unordered => {
      skip_unordered_entries(bs, codebook_entries)?;
    }
    CodebookOrder::Ordered => {
      skip_ordered_entries(bs, codebook_entries)?;
    }
  }

  skip_lookup_table(bs, codebook_entries, codebook_dimensions)
}

fn skip_time_domain_transforms(bs: &mut BitReaderRtl<'_>) -> Result<()> {
  let count = count_of(bs, 6)?;

  for _ in 0..count {
    let placeholder = bs.read_bits_leq32(16)?;

    match placeholder {
      0 => {}
      _set => return decode_error("ogg (vorbis): invalid time domain tranform"),
    }
  }

  Ok(())
}

fn skip_floors(bs: &mut BitReaderRtl<'_>) -> Result<()> {
  let count = count_of(bs, 6)?;

  for _ in 0..count {
    skip_floor(bs)?;
  }

  Ok(())
}

fn skip_floor(bs: &mut BitReaderRtl<'_>) -> Result<()> {
  let floor_type = bs.read_bits_leq32(16)?;

  match floor_type {
    0 => skip_floor0_setup(bs),
    1 => skip_floor1_setup(bs),
    _invalid => decode_error("ogg (vorbis): invalid floor type"),
  }
}

fn skip_floor0_setup(bs: &mut BitReaderRtl<'_>) -> Result<()> {
  bs.ignore_bits(FLOOR0_FIXED_BITS)?;

  let floor0_number_of_books = count_of(bs, 4)?;

  bs.ignore_bits(floor0_number_of_books.saturating_mul(8))
    .map_err(Error::from)
}

fn skip_floor1_classes(bs: &mut BitReaderRtl<'_>, num_classes: u32) -> Result<Vec<u32>> {
  let mut dimensions = Vec::new();

  for _ in 0..num_classes {
    let class_dimensions = count_of(bs, 3)?;
    let subclass_bits = bs.read_bits_leq32(2)?;

    dimensions.push(class_dimensions);

    match subclass_bits {
      0 => {}
      _subclassed => {
        let _main_book = bs.read_bits_leq32(8)?;
      }
    }

    bs.ignore_bits(1u32.wrapping_shl(subclass_bits).saturating_mul(8))?;
  }

  Ok(dimensions)
}

fn skip_floor1_setup(bs: &mut BitReaderRtl<'_>) -> Result<()> {
  let floor1_partitions = bs.read_bits_leq32(5)?;
  let mut partition_classes = Vec::new();

  for _ in 0..floor1_partitions {
    let class = bs.read_bits_leq32(4)?;

    partition_classes.push(class);
  }

  let num_classes = match partition_classes.iter().max() {
    Some(max_class) => max_class.saturating_add(1),
    None => 0,
  };

  let classes_dimensions = skip_floor1_classes(bs, num_classes)?;
  let _floor1_multiplier = bs.read_bits_leq32(2)?;
  let rangebits = bs.read_bits_leq32(4)?;

  for class in partition_classes {
    let Ok(position) = console_core_number_conversion::index(class);

    match classes_dimensions.get(position) {
      Some(class_dimensions) => {
        bs.ignore_bits(class_dimensions.saturating_mul(rangebits))?;
      }
      None => return decode_error("ogg (vorbis): floor partition class out of range"),
    }
  }

  Ok(())
}

fn skip_residues(bs: &mut BitReaderRtl<'_>) -> Result<()> {
  let count = count_of(bs, 6)?;

  for _ in 0..count {
    let _residue_type = bs.read_bits_leq32(16)?;

    skip_residue_setup(bs)?;
  }

  Ok(())
}

fn high_bits(bs: &mut BitReaderRtl<'_>) -> Result<u32> {
  let present = bs.read_flag().map(OptionalField::from)?;

  match present {
    OptionalField::Encoded => bs.read_bits_leq32(5).map_err(Error::from),
    OptionalField::Omitted => Ok(0),
  }
}

fn skip_residue_setup(bs: &mut BitReaderRtl<'_>) -> Result<()> {
  bs.ignore_bits(RESIDUE_FIXED_BITS)?;

  let residue_classifications = count_of(bs, 6)?;

  bs.ignore_bits(8)?;

  let mut num_codebooks = 0u32;

  for _ in 0..residue_classifications {
    let low_bits = bs.read_bits_leq32(3)?;
    let high = high_bits(bs)?;
    let is_used = high.wrapping_shl(3) | low_bits;

    num_codebooks = num_codebooks.saturating_add(is_used.count_ones());
  }

  bs.ignore_bits(num_codebooks.saturating_mul(8))
    .map_err(Error::from)
}

fn skip_mappings(bs: &mut BitReaderRtl<'_>, audio_channels: u8) -> Result<()> {
  let count = count_of(bs, 6)?;

  for _ in 0..count {
    skip_mapping(bs, audio_channels)?;
  }

  Ok(())
}

fn skip_mapping(bs: &mut BitReaderRtl<'_>, audio_channels: u8) -> Result<()> {
  let mapping_type = bs.read_bits_leq32(16)?;

  match mapping_type {
    0 => skip_mapping_type0_setup(bs, audio_channels),
    _invalid => decode_error("ogg (vorbis): invalid mapping type"),
  }
}

fn skip_couplings(bs: &mut BitReaderRtl<'_>, audio_channels: u8) -> Result<()> {
  let coupling_steps = count_of(bs, 8)?;
  let Ok(coupling_bits) = ilog(u32::from(audio_channels.saturating_sub(1)));

  for _ in 0..coupling_steps {
    let _magnitude_ch = bs.read_bits_leq32(coupling_bits)?;
    let _angle_ch = bs.read_bits_leq32(coupling_bits)?;
  }

  Ok(())
}

fn skip_mapping_type0_setup(bs: &mut BitReaderRtl<'_>, audio_channels: u8) -> Result<()> {
  let submap_count = bs.read_flag().map(OptionalField::from)?;

  let num_submaps = match submap_count {
    OptionalField::Encoded => count_of(bs, 4)?,
    OptionalField::Omitted => 1,
  };

  let couplings = bs.read_flag().map(OptionalField::from)?;

  match couplings {
    OptionalField::Encoded => {
      skip_couplings(bs, audio_channels)?;
    }
    OptionalField::Omitted => {}
  }

  let reserved = bs.read_bits_leq32(2)?;

  match reserved {
    0 => {}
    _set => return decode_error("ogg (vorbis): reserved mapping bits non-zero"),
  }

  match num_submaps {
    0 | 1 => {}
    _several => {
      bs.ignore_bits(u32::from(audio_channels).saturating_mul(4))?;
    }
  }

  bs.ignore_bits(num_submaps.saturating_mul(SUBMAP_BITS))
    .map_err(Error::from)
}

fn read_modes(bs: &mut BitReaderRtl<'_>) -> Result<Vec<Block>> {
  let count = count_of(bs, 6)?;
  let mut blocks = Vec::new();

  for _ in 0..count {
    let block = read_mode(bs)?;

    blocks.push(block);
  }

  Ok(blocks)
}

fn read_mode(bs: &mut BitReaderRtl<'_>) -> Result<Block> {
  let block = bs.read_flag().map(Block::from)?;
  let window_type = bs.read_bits_leq32(16)?;
  let transform_type = bs.read_bits_leq32(16)?;
  let _mapping = bs.read_bits_leq32(8)?;

  match window_type {
    0 => {}
    _other => return decode_error("ogg (vorbis): invalid window type for mode"),
  }

  match transform_type {
    0 => {}
    _other => return decode_error("ogg (vorbis): invalid transform type for mode"),
  }

  Ok(block)
}
