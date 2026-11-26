use std::sync::LazyLock;

use console_core_never::Never;
use console_core_number_conversion::index;

use super::huffman::{Huffman, tables};
use super::reader::{Reader, Remaining};
use crate::{CMapName, ParseError};

const FILE_HEADER: u32 = 10;
const MAX_BUNDLE_BYTES: u32 = 524_288;

const NAMES: [&[u8]; 65] = [
  b"83pv-RKSJ-H",
  b"90ms-RKSJ-H",
  b"90ms-RKSJ-V",
  b"90msp-RKSJ-H",
  b"90msp-RKSJ-V",
  b"90pv-RKSJ-H",
  b"Add-RKSJ-H",
  b"Add-RKSJ-V",
  b"Adobe-CNS1-UCS2",
  b"Adobe-GB1-UCS2",
  b"Adobe-Japan1-UCS2",
  b"Adobe-Korea1-UCS2",
  b"B5pc-H",
  b"B5pc-V",
  b"CNS-EUC-H",
  b"CNS-EUC-V",
  b"ETen-B5-H",
  b"ETen-B5-V",
  b"ETenms-B5-H",
  b"ETenms-B5-V",
  b"EUC-H",
  b"EUC-V",
  b"Ext-RKSJ-H",
  b"Ext-RKSJ-V",
  b"GB-EUC-H",
  b"GB-EUC-V",
  b"GBK-EUC-H",
  b"GBK-EUC-V",
  b"GBK2K-H",
  b"GBK2K-V",
  b"GBKp-EUC-H",
  b"GBKp-EUC-V",
  b"GBpc-EUC-H",
  b"GBpc-EUC-V",
  b"H",
  b"HKscs-B5-H",
  b"HKscs-B5-V",
  b"Identity-H",
  b"Identity-V",
  b"KSC-EUC-H",
  b"KSC-EUC-V",
  b"KSCms-UHC-H",
  b"KSCms-UHC-HW-H",
  b"KSCms-UHC-HW-V",
  b"KSCms-UHC-V",
  b"KSCpc-EUC-H",
  b"UniCNS-UCS2-H",
  b"UniCNS-UCS2-V",
  b"UniCNS-UTF16-H",
  b"UniCNS-UTF16-V",
  b"UniGB-UCS2-H",
  b"UniGB-UCS2-V",
  b"UniGB-UTF16-H",
  b"UniGB-UTF16-V",
  b"UniJIS-UCS2-H",
  b"UniJIS-UCS2-HW-H",
  b"UniJIS-UCS2-HW-V",
  b"UniJIS-UCS2-V",
  b"UniJIS-UTF16-H",
  b"UniJIS-UTF16-V",
  b"UniKS-UCS2-H",
  b"UniKS-UCS2-V",
  b"UniKS-UTF16-H",
  b"UniKS-UTF16-V",
  b"V",
];

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "load_embedded hands out slices of the one decompressed bundle for the life of the process, so the bundle can live nowhere but in the process and nobody is in a position to hand it in"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "the bundle is filled once on first use and never changed after, and the static lifetime of what load_embedded returns is the contract of its callers"
  )
)]
static BUNDLE: LazyLock<Result<Bundle, ParseError>> = LazyLock::new(load);

#[derive(Debug)]
pub(crate) struct Bundle {
  data: Vec<u8>,
  pub(crate) delta: Huffman,
  pub(crate) count: Huffman,
  files: Vec<(u32, u32)>,
}

fn load() -> Result<Bundle, ParseError> {
  let compressed = include_bytes!("../../assets/cmaps.zlib");
  let decompressed = console_core_deflate::zlib::decode(compressed, MAX_BUNDLE_BYTES)
    .map_err(|_broken| ParseError::BadBundle)?;

  let Ok(mut head) = Reader::new(&decompressed);
  let tables_length = head.u32()?;
  let table_data = head.take(tables_length)?;
  let (delta, count) = tables(table_data)?;
  let mut offset = tables_length.checked_add(4).ok_or(ParseError::BadBundle)?;
  let mut files = Vec::new();

  for _file in &decompressed {
    let Ok(remaining) = head.remaining();

    match remaining {
      Remaining::Absent => break,
      Remaining::Present => {}
    }

    head.take(6)?;

    let length = head.u32()?;
    let body = length
      .checked_sub(FILE_HEADER)
      .ok_or(ParseError::BadBundle)?;

    head.take(body)?;

    let end = offset.checked_add(length).ok_or(ParseError::BadBundle)?;

    files.push((offset, end));
    offset = end;
  }

  Ok(Bundle {
    data: decompressed,
    delta,
    count,
    files,
  })
}

pub(crate) fn bundle() -> Result<&'static Bundle, ParseError> {
  match &*BUNDLE {
    Ok(bundle) => Ok(bundle),
    Err(fault) => Err(*fault),
  }
}

pub fn load_embedded(name: CMapName<'_>) -> Result<Option<&'static [u8]>, Never> {
  let Ok(wanted) = name.to_bytes();
  let bundle = match bundle() {
    Ok(bundle) => bundle,
    Err(_broken) => return Ok(None),
  };
  let span = NAMES
    .iter()
    .position(|known| *known == wanted)
    .and_then(|at| bundle.files.get(at));

  Ok(match span {
    Some((from, to)) => {
      let Ok(start) = index(*from);
      let Ok(end) = index(*to);

      bundle.data.get(start..end)
    }
    None => None,
  })
}
