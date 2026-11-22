//! Parsing a JP2 box, as specified in I.4.

use crate::reader::BitReader;

pub(crate) const JP2_SIGNATURE: u32 = 0x6A502020;

pub(crate) const FILE_TYPE: u32 = 0x66747970;

pub(crate) const JP2_HEADER: u32 = 0x6A703268;

pub(crate) const COLOUR_SPECIFICATION: u32 = 0x636F6C72;

pub(crate) const PALETTE: u32 = 0x70636C72;

pub(crate) const COMPONENT_MAPPING: u32 = 0x636D6170;

pub(crate) const CHANNEL_DEFINITION: u32 = 0x63646566;

pub(crate) const CONTIGUOUS_CODESTREAM: u32 = 0x6A703263;

pub(crate) struct Jp2Box<'a> {
  pub(crate) data: &'a [u8],
  pub(crate) box_type: u32,
}

pub(crate) fn read<'a>(
  reader: &mut BitReader<'a>,
) -> core::result::Result<Option<Jp2Box<'a>>, console_core_never::Never> {
  let l_box = match match reader.read_u32() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  let t_box = match match reader.read_u32() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let data = match l_box {
    0 => {
      let Ok(tail) = reader.tail();
      let data = match tail {
        Some(value) => value,
        None => return Ok(None),
      };
      let Ok(()) = reader.jump_to_end();
      data
    }

    1 => {
      let xl_box = match (match match reader.read_u64() {
        Ok(value) => value,
      } {
        Some(value) => value,
        None => return Ok(None),
      })
      .checked_sub(16)
      {
        Some(value) => value,
        None => return Ok(None),
      };

      let Ok(bytes) = reader.read_bytes(xl_box);

      match bytes {
        Some(value) => value,
        None => return Ok(None),
      }
    }

    _ => {
      let length = match l_box.checked_sub(8) {
        Some(value) => value,
        None => return Ok(None),
      };

      let Ok(bytes) = reader.read_bytes(u64::from(length));

      match bytes {
        Some(value) => value,
        None => return Ok(None),
      }
    }
  };

  Ok(Some(Jp2Box {
    data,
    box_type: t_box,
  }))
}
