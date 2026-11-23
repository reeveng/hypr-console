//! SFNT directories and TTC headers validate borrowed records before exposing them.

use crate::font::TableOrder;
use crate::table_ref::MinByteRange;
use crate::{FontData, FontRead, ReadError};
use console_core_never::Never;
use core::ops::Range;
use types::{BigEndian, Compatible, FixedSize, MajorMinor, Tag};

#[derive(Clone, Copy, Debug, Default, bytemuck::AnyBitPattern)]
#[repr(C)]
struct DirectoryHeader {
  sfnt_version: BigEndian<u32>,
  num_tables: BigEndian<u16>,
  search_range: BigEndian<u16>,
  entry_selector: BigEndian<u16>,
  range_shift: BigEndian<u16>,
}

impl FixedSize for DirectoryHeader {
  const RAW_BYTE_LEN: u64 = 12;
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck::AnyBitPattern)]
#[repr(C)]
pub struct TableRecord {
  pub tag: BigEndian<Tag>,
  pub checksum: BigEndian<u32>,
  pub offset: BigEndian<u32>,
  pub length: BigEndian<u32>,
}

const _: () = {
  assert!(core::mem::size_of::<TableRecord>() == 16);
  assert!(core::mem::align_of::<TableRecord>() == 1);
  assert!(core::mem::offset_of!(TableRecord, tag) == 0);
  assert!(core::mem::offset_of!(TableRecord, checksum) == 4);
  assert!(core::mem::offset_of!(TableRecord, offset) == 8);
  assert!(core::mem::offset_of!(TableRecord, length) == 12);
};

impl FixedSize for TableRecord {
  const RAW_BYTE_LEN: u64 = 16;
}

impl TableRecord {
  pub fn tag(&self) -> Result<Tag, Never> {
    self.tag.get()
  }

  pub fn checksum(&self) -> Result<u32, Never> {
    self.checksum.get()
  }

  pub fn offset(&self) -> Result<u32, Never> {
    self.offset.get()
  }

  pub fn length(&self) -> Result<u32, Never> {
    self.length.get()
  }
}

#[derive(Clone)]
pub struct TableDirectory<'a> {
  data: FontData<'a>,
  header: DirectoryHeader,
  records: &'a [TableRecord],
  minimum: &'a [u8],
  records_end: u64,
}

fn minimum(data: FontData<'_>, end: u64) -> Result<&[u8], ReadError> {
  let Ok(head) = data.slice(..end);
  let head_2 = head.ok_or(ReadError::OutOfBounds)?;
  let Ok(bytes) = head_2.as_bytes();

  Ok(bytes)
}

impl<'a> FontRead<'a> for TableDirectory<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let header = data.read_ref_at::<DirectoryHeader>(0)?;
    let header_2 = *header;
    let Ok(count) = header_2.num_tables.get();
    let records_end = u64::from(count).wrapping_mul(16).saturating_add(12);
    let records = data.read_array(12..records_end)?;
    let minimum = minimum(data, records_end)?;

    Ok(Self {
      data,
      header: header_2,
      records,
      minimum,
      records_end,
    })
  }
}

impl<'a> TableDirectory<'a> {
  pub const MIN_SIZE: u64 = 12;

  pub fn sfnt_version(&self) -> Result<u32, Never> {
    self.header.sfnt_version.get()
  }

  pub fn num_tables(&self) -> Result<u16, Never> {
    self.header.num_tables.get()
  }

  pub fn search_range(&self) -> Result<u16, Never> {
    self.header.search_range.get()
  }

  pub fn entry_selector(&self) -> Result<u16, Never> {
    self.header.entry_selector.get()
  }

  pub fn range_shift(&self) -> Result<u16, Never> {
    self.header.range_shift.get()
  }

  pub fn table_records(&self) -> Result<&'a [TableRecord], Never> {
    Ok(self.records)
  }

  pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
    Ok(self.data)
  }

  pub fn shape(&self) -> Result<&Self, Never> {
    Ok(self)
  }

  pub(crate) fn order(&self) -> Result<TableOrder, Never> {
    let out_of_order = self.records.windows(2).find(|pair| match pair {
      [left, right] => {
        let Ok(left_2) = left.tag();
        let Ok(right_2) = right.tag();

        left_2 >= right_2
      }
      _incomplete => false,
    });

    Ok(match out_of_order {
      Some(_) => TableOrder::Unsorted,
      None => TableOrder::Sorted,
    })
  }

  pub fn sfnt_version_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(0..4)
  }

  pub fn num_tables_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(4..6)
  }

  pub fn search_range_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(6..8)
  }

  pub fn entry_selector_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(8..10)
  }

  pub fn range_shift_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(10..12)
  }

  pub fn table_records_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(12..self.records_end)
  }
}

impl Default for TableDirectory<'_> {
  fn default() -> Self {
    let minimum = &[0; 12];
    let Ok(data) = FontData::default_table_data();

    Self {
      data,
      header: DirectoryHeader::default(),
      records: &[],
      minimum,
      records_end: 12,
    }
  }
}

impl<'a> MinByteRange<'a> for TableDirectory<'a> {
  fn min_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(0..self.records_end)
  }

  fn min_table_bytes(&self) -> Result<&'a [u8], ReadError> {
    Ok(self.minimum)
  }
}

#[derive(Clone, Copy, Debug, Default, bytemuck::AnyBitPattern)]
#[repr(C)]
struct CollectionHeader {
  tag: BigEndian<Tag>,
  version: BigEndian<MajorMinor>,
  num_fonts: BigEndian<u32>,
}

impl FixedSize for CollectionHeader {
  const RAW_BYTE_LEN: u64 = 12;
}

#[derive(Clone)]
struct Signature {
  tag: u32,
  length: u32,
  offset: u32,
}

#[derive(Clone)]
pub struct TTCHeader<'a> {
  data: FontData<'a>,
  header: CollectionHeader,
  offsets: &'a [BigEndian<u32>],
  signature: Option<Signature>,
  offsets_end: u64,
  minimum: &'a [u8],
}

impl<'a> FontRead<'a> for TTCHeader<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let header = data.read_ref_at::<CollectionHeader>(0)?;
    let header_2 = *header;
    let Ok(count) = header_2.num_fonts.get();
    let offsets_end = u64::from(count).wrapping_mul(4).saturating_add(12);
    let offsets = data.read_array(12..offsets_end)?;
    let Ok(version) = header_2.version.get();
    let Ok(has_signature) = version.compatibility((2u16, 0u16));
    let has_signature_2 = matches!(has_signature, font_types::Compatibility::Compatible);
    let (signature, end) = match ::core::primitive::bool::then_some(has_signature_2, ()) {
      ::core::option::Option::Some(()) => {
        let tag = data.read_at(offsets_end)?;
        let length = data.read_at(offsets_end.saturating_add(4))?;
        let offset = data.read_at(offsets_end.saturating_add(8))?;

        (
          Some(Signature {
            tag,
            length,
            offset,
          }),
          offsets_end.saturating_add(12),
        )
      }
      ::core::option::Option::None => (None, offsets_end),
    };
    let minimum = minimum(data, end)?;

    Ok(Self {
      data,
      header: header_2,
      offsets,
      signature,
      offsets_end,
      minimum,
    })
  }
}

impl<'a> TTCHeader<'a> {
  pub const MIN_SIZE: u64 = 12;

  pub fn ttc_tag(&self) -> Result<Tag, Never> {
    self.header.tag.get()
  }

  pub fn version(&self) -> Result<MajorMinor, Never> {
    self.header.version.get()
  }

  pub fn num_fonts(&self) -> Result<u32, Never> {
    self.header.num_fonts.get()
  }

  pub fn table_directory_offsets(&self) -> Result<&'a [BigEndian<u32>], Never> {
    Ok(self.offsets)
  }

  pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
    Ok(self.data)
  }

  pub fn shape(&self) -> Result<&Self, Never> {
    Ok(self)
  }

  pub fn dsig_tag(&self) -> Result<Option<u32>, Never> {
    Ok(self.signature.as_ref().map(|signature| signature.tag))
  }

  pub fn dsig_length(&self) -> Result<Option<u32>, Never> {
    Ok(self.signature.as_ref().map(|signature| signature.length))
  }

  pub fn dsig_offset(&self) -> Result<Option<u32>, Never> {
    Ok(self.signature.as_ref().map(|signature| signature.offset))
  }

  pub fn ttc_tag_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(0..4)
  }

  pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(4..8)
  }

  pub fn num_fonts_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(8..12)
  }

  pub fn table_directory_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(12..self.offsets_end)
  }

  pub fn dsig_tag_byte_range(&self) -> Result<Range<u64>, Never> {
    let start = match self.signature {
      Some(_) => self.offsets_end.saturating_add(0),
      None => self.offsets_end,
    };
    let end = match self.signature {
      Some(_) => start.saturating_add(4),
      None => start,
    };

    Ok(start..end)
  }

  pub fn dsig_length_byte_range(&self) -> Result<Range<u64>, Never> {
    let start = match self.signature {
      Some(_) => self.offsets_end.saturating_add(4),
      None => self.offsets_end,
    };
    let end = match self.signature {
      Some(_) => start.saturating_add(4),
      None => start,
    };

    Ok(start..end)
  }

  pub fn dsig_offset_byte_range(&self) -> Result<Range<u64>, Never> {
    let start = match self.signature {
      Some(_) => self.offsets_end.saturating_add(8),
      None => self.offsets_end,
    };
    let end = match self.signature {
      Some(_) => start.saturating_add(4),
      None => start,
    };

    Ok(start..end)
  }
}

impl Default for TTCHeader<'_> {
  fn default() -> Self {
    let minimum = &[0; 12];
    let Ok(data) = FontData::default_table_data();

    Self {
      data,
      header: CollectionHeader::default(),
      offsets: &[],
      signature: None,
      offsets_end: 12,
      minimum,
    }
  }
}

impl<'a> MinByteRange<'a> for TTCHeader<'a> {
  fn min_byte_range(&self) -> Result<Range<u64>, Never> {
    let Ok(length) = console_core_number_conversion::fitted(self.minimum.len());

    Ok(0..length)
  }

  fn min_table_bytes(&self) -> Result<&'a [u8], ReadError> {
    Ok(self.minimum)
  }
}
