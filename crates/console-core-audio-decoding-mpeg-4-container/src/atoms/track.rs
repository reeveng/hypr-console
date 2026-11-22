//! A track: the media header that says how its timestamps count, and the sample table.

use console_core_audio_decoding_interfaces::errors::{Error, Result, decode_error};
use console_core_byte_streams::ReadBytes;

use crate::atoms::sample_table::StblAtom;
use crate::atoms::{Atom, AtomHeader, AtomIterator, Kind};

const MISSING_TKHD: Error = Error::DecodeError("isomp4: missing tkhd atom");

const MISSING_MDHD: Error = Error::DecodeError("isomp4: missing mdhd atom");

const MISSING_HDLR: Error = Error::DecodeError("isomp4: missing hdlr atom");

const MISSING_MINF: Error = Error::DecodeError("isomp4: missing minf atom");

const MISSING_STBL: Error = Error::DecodeError("isomp4: missing stbl atom");

const MISSING_MDIA: Error = Error::DecodeError("isomp4: missing mdia atom");

pub struct MdhdAtom {
  pub timescale: u32,
  pub duration: u64,
}

impl Atom for MdhdAtom {
  fn read<B: ReadBytes>(reader: &mut B, _header: AtomHeader) -> Result<Self> {
    let full_box = AtomHeader::read_full_box(reader)?;
    let (timescale, duration) = match full_box.version {
      0 => {
        reader.ignore_bytes(8)?;
        let timescale = reader.read_be_u32()?;
        let parsed_duration = reader.read_be_u32()?;
        let duration = match parsed_duration {
          u32::MAX => u64::MAX,
          duration => u64::from(duration),
        };
        (timescale, duration)
      }
      1 => {
        reader.ignore_bytes(16)?;
        let timescale = reader.read_be_u32()?;
        let duration = reader.read_be_u64()?;
        (timescale, duration)
      }
      _ => return decode_error("isomp4: invalid mdhd version"),
    };
    reader.ignore_bytes(4)?;
    Ok(MdhdAtom {
      timescale,
      duration,
    })
  }
}

pub struct MinfAtom {
  pub stbl: StblAtom,
}

impl Atom for MinfAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let Ok(mut iter) = AtomIterator::new(reader, header);
    let mut stbl = None;
    let Ok(limit) = iter.atoms_at_most();

    for _ in 0..limit {
      let next = iter.next()?;

      match next {
        Some(AtomHeader {
          kind: Kind::SAMPLE_TABLE,
          ..
        }) => {
          let parsed_stbl = iter.read_atom::<StblAtom>()?;
          stbl = Some(parsed_stbl);
        }
        Some(_) => {}
        None => break,
      }
    }

    let parsed_stbl_2 = stbl.ok_or(MISSING_STBL)?;
    Ok(MinfAtom {
      stbl: parsed_stbl_2,
    })
  }
}

pub struct MdiaAtom {
  pub mdhd: MdhdAtom,
  pub minf: MinfAtom,
}

impl Atom for MdiaAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let Ok(mut iter) = AtomIterator::new(reader, header);
    let mut mdhd = None;
    let mut has_hdlr = false;
    let mut minf = None;
    let Ok(limit) = iter.atoms_at_most();

    for _ in 0..limit {
      let next = iter.next()?;

      match next {
        Some(AtomHeader {
          kind: Kind::MEDIA_HEADER,
          ..
        }) => {
          let parsed_mdhd = iter.read_atom::<MdhdAtom>()?;
          mdhd = Some(parsed_mdhd);
        }
        Some(AtomHeader {
          kind: Kind::HANDLER,
          ..
        }) => has_hdlr = true,
        Some(AtomHeader {
          kind: Kind::MEDIA_INFO,
          ..
        }) => {
          let parsed_minf = iter.read_atom::<MinfAtom>()?;
          minf = Some(parsed_minf);
        }
        Some(_) => {}
        None => break,
      }
    }

    let media_header = mdhd.ok_or(MISSING_MDHD)?;

    match ::core::primitive::bool::then_some(has_hdlr, ()) {
      ::core::option::Option::Some(()) => {
        let parsed_minf_2 = minf.ok_or(MISSING_MINF)?;
        Ok(MdiaAtom {
          mdhd: media_header,
          minf: parsed_minf_2,
        })
      }
      ::core::option::Option::None => Err(MISSING_HDLR),
    }
  }
}

pub struct TrakAtom {
  pub mdia: MdiaAtom,
}

impl Atom for TrakAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let Ok(mut iter) = AtomIterator::new(reader, header);
    let mut has_tkhd = false;
    let mut mdia = None;
    let Ok(limit) = iter.atoms_at_most();

    for _ in 0..limit {
      let next = iter.next()?;

      match next {
        Some(AtomHeader {
          kind: Kind::TRACK_HEADER,
          ..
        }) => has_tkhd = true,
        Some(AtomHeader {
          kind: Kind::MEDIA, ..
        }) => {
          let parsed_mdia = iter.read_atom::<MdiaAtom>()?;
          mdia = Some(parsed_mdia);
        }
        Some(_) => {}
        None => break,
      }
    }

    match ::core::primitive::bool::then_some(has_tkhd, ()) {
      ::core::option::Option::Some(()) => {
        let parsed_mdia_2 = mdia.ok_or(MISSING_MDIA)?;
        Ok(TrakAtom {
          mdia: parsed_mdia_2,
        })
      }
      ::core::option::Option::None => Err(MISSING_TKHD),
    }
  }
}
