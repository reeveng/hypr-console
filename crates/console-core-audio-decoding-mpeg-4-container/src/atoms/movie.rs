//! The movie atom: its tracks, the defaults a fragmented movie's fragments fall back on, and the
//! tags kept under its user data.

use console_core_audio_decoding_interfaces::errors::{Error, Result};
use console_core_audio_decoding_interfaces::meta::MetadataRevision;
use console_core_byte_streams::ReadBytes;

use crate::atoms::metadata::UdtaAtom;
use crate::atoms::track::TrakAtom;
use crate::atoms::{Atom, AtomHeader, AtomIterator, Kind};

const MISSING_MVHD: Error = Error::DecodeError("isomp4: missing mvhd atom");

pub struct TrexAtom {
  pub track_id: u32,
  pub default_sample_duration: u32,
  pub default_sample_size: u32,
}

impl Atom for TrexAtom {
  fn read<B: ReadBytes>(reader: &mut B, _header: AtomHeader) -> Result<Self> {
    AtomHeader::read_full_box(reader)?;

    let track_id = reader.read_be_u32()?;

    reader.ignore_bytes(4)?;

    let default_sample_duration = reader.read_be_u32()?;
    let default_sample_size = reader.read_be_u32()?;

    reader.ignore_bytes(4)?;

    Ok(TrexAtom {
      track_id,
      default_sample_duration,
      default_sample_size,
    })
  }
}

pub struct MvexAtom {
  pub trexs: Vec<TrexAtom>,
}

impl Atom for MvexAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let Ok(mut iter) = AtomIterator::new(reader, header);
    let mut trexs = Vec::new();
    let Ok(limit) = iter.atoms_at_most();

    for _ in 0..limit {
      let next = iter.next()?;

      match next {
        Some(AtomHeader {
          kind: Kind::TRACK_EXTENDS,
          ..
        }) => {
          let parsed_value = iter.read_atom::<TrexAtom>()?;
          trexs.push(parsed_value);
        }
        Some(_) => {}
        None => break,
      }
    }

    Ok(MvexAtom { trexs })
  }
}

pub struct MoovAtom {
  pub traks: Vec<TrakAtom>,
  pub mvex: Option<MvexAtom>,
  pub metadata: Option<MetadataRevision>,
}

impl Atom for MoovAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let Ok(mut iter) = AtomIterator::new(reader, header);
    let mut has_mvhd = false;
    let mut traks = Vec::new();
    let mut mvex = None;
    let mut metadata = None;
    let Ok(limit) = iter.atoms_at_most();

    for _ in 0..limit {
      let next = iter.next()?;

      match next {
        Some(AtomHeader {
          kind: Kind::MOVIE_HEADER,
          ..
        }) => has_mvhd = true,
        Some(AtomHeader {
          kind: Kind::TRACK, ..
        }) => {
          let parsed_value = iter.read_atom::<TrakAtom>()?;
          traks.push(parsed_value);
        }
        Some(AtomHeader {
          kind: Kind::MOVIE_EXTENDS,
          ..
        }) => {
          let parsed_mvex = iter.read_atom::<MvexAtom>()?;
          mvex = Some(parsed_mvex);
        }
        Some(AtomHeader {
          kind: Kind::USER_DATA,
          ..
        }) => {
          let parsed_metadata = iter.read_atom::<UdtaAtom>()?;
          metadata = parsed_metadata.metadata;
        }
        Some(_) => {}
        None => break,
      }
    }

    match ::core::primitive::bool::then_some(has_mvhd, ()) {
      ::core::option::Option::Some(()) => Ok(MoovAtom {
        traks,
        mvex,
        metadata,
      }),
      ::core::option::Option::None => Err(MISSING_MVHD),
    }
  }
}
