//! Font files resolve directory entries relative to the original file bytes.
//!
//! Collections retain their shared byte base. Sorted directories use binary
//! search; unsorted directories preserve their record order and use a scan.

use crate::{Emptiness, FontData, FontRead, ReadError, TTCHeader, TableDirectory, TableRecord};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use types::{CFF_SFNT_VERSION, TRUE_SFNT_VERSION, TT_SFNT_VERSION, TTC_HEADER_TAG, Tag};

#[derive(Clone)]
pub enum FileRef<'a> {
  Font(FontRef<'a>),
  Collection(CollectionRef<'a>),
}

impl<'a> FileRef<'a> {
  pub fn new(bytes: &'a [u8]) -> Result<Self, ReadError> {
    let Ok(data) = FontData::new(bytes);
    let tag = data.read_at::<Tag>(0)?;

    match ::core::primitive::bool::then_some(tag == TTC_HEADER_TAG, ()) {
      ::core::option::Option::Some(()) => CollectionRef::new(bytes).map(Self::Collection),
      ::core::option::Option::None => FontRef::new(bytes).map(Self::Font),
    }
  }

  pub fn fonts(
    &self,
  ) -> Result<impl Iterator<Item = Result<FontRef<'a>, ReadError>> + 'a + Clone + use<'a>, Never>
  {
    let (single, collection) = match self {
      Self::Font(font) => (Some(Ok(font.clone())), None),
      Self::Collection(collection) => {
        let Ok(fonts) = collection.iter();

        (None, Some(fonts))
      }
    };

    Ok(collection.into_iter().flatten().chain(single))
  }
}

#[derive(Clone)]
pub struct CollectionRef<'a> {
  data: FontData<'a>,
  header: TTCHeader<'a>,
}

impl<'a> CollectionRef<'a> {
  pub fn new(bytes: &'a [u8]) -> Result<Self, ReadError> {
    let Ok(data) = FontData::new(bytes);
    let header = TTCHeader::read(data)?;
    let Ok(tag) = header.ttc_tag();

    match ::core::primitive::bool::then_some(tag == TTC_HEADER_TAG, ()) {
      ::core::option::Option::Some(()) => Ok(Self { data, header }),
      ::core::option::Option::None => Err(ReadError::InvalidTtc(tag)),
    }
  }

  pub fn len(&self) -> Result<u32, Never> {
    self.header.num_fonts()
  }

  pub fn is_empty(&self) -> Result<Emptiness, Never> {
    let Ok(length) = self.len();

    Ok(match length {
      0 => Emptiness::Empty,
      _ => Emptiness::NonEmpty,
    })
  }

  pub fn get(&self, collection_index: u32) -> Result<FontRef<'a>, ReadError> {
    let Ok(offsets) = self.header.table_directory_offsets();
    let Ok(at) = index(collection_index);
    let offset = offsets
      .get(at)
      .ok_or(ReadError::InvalidCollectionIndex(collection_index))?;
    let Ok(offset_2) = offset.get();
    let Ok(directory_data) = self.data.split_off(u64::from(offset_2));
    let directory_data_2 = directory_data.ok_or(ReadError::OutOfBounds)?;
    let directory = TableDirectory::read(directory_data_2)?;

    FontRef::with_table_directory(self.data, directory, Some(collection_index))
  }

  pub fn iter(
    &self,
  ) -> Result<impl Iterator<Item = Result<FontRef<'a>, ReadError>> + 'a + Clone + use<'a>, Never>
  {
    let collection = self.clone();
    let Ok(count) = self.len();

    Ok((0..count).map(move |position| collection.get(position)))
  }
}

#[derive(Clone)]
pub(crate) enum TableOrder {
  Sorted,
  Unsorted,
}

#[derive(Clone)]
pub struct FontRef<'a> {
  data: FontData<'a>,
  #[cfg_attr(
    dylint_lib = "explicit065_checked_constructors",
    expect(explicit065_checked_constructors, reason = "FontRef publishes the directory field for existing callers, and table offsets from that directory are checked at each read")
  )]
  pub table_directory: TableDirectory<'a>,
  ttc_index: Option<u32>,
  order: TableOrder,
}

impl<'a> FontRef<'a> {
  pub fn new(bytes: &'a [u8]) -> Result<Self, ReadError> {
    let Ok(data) = FontData::new(bytes);
    let directory = TableDirectory::read(data)?;

    Self::with_table_directory(data, directory, None)
  }

  pub fn from_index(bytes: &'a [u8], collection_index: u32) -> Result<Self, ReadError> {
    let file = FileRef::new(bytes)?;

    match file {
      FileRef::Font(font) => match collection_index {
        0 => Ok(font),
        _outside => Err(ReadError::InvalidCollectionIndex(collection_index)),
      },
      FileRef::Collection(collection) => collection.get(collection_index),
    }
  }

  pub fn data(&self) -> Result<FontData<'a>, Never> {
    Ok(self.data)
  }

  pub fn ttc_index(&self) -> Result<Option<u32>, Never> {
    Ok(self.ttc_index)
  }

  pub fn table_directory(&self) -> Result<&TableDirectory<'a>, Never> {
    Ok(&self.table_directory)
  }

  pub fn table_data(&self, tag: Tag) -> Result<Option<FontData<'a>>, ReadError> {
    let Ok(record) = self.record(tag);
    let record_3 = match record {
      Some(record_2) => record_2,
      None => return Ok(None),
    };
    let Ok(offset) = record_3.offset();
    let Ok(length) = record_3.length();

    match offset {
      0 => return Err(ReadError::NullOffset),
      _present => {}
    }

    let first = u64::from(offset);
    let last = first
      .checked_add(u64::from(length))
      .ok_or(ReadError::OutOfBounds)?;
    let Ok(data) = self.data.slice(first..last);
    let data_2 = data.ok_or(ReadError::OutOfBounds)?;

    Ok(Some(data_2))
  }

  pub fn fonts(
    bytes: &'a [u8],
  ) -> Result<impl Iterator<Item = Result<FontRef<'a>, ReadError>> + 'a + Clone, ReadError> {
    let file = FileRef::new(bytes)?;
    let Ok(fonts) = file.fonts();

    Ok(fonts)
  }

  fn record(&self, tag: Tag) -> Result<Option<&'a TableRecord>, Never> {
    let Ok(records) = self.table_directory.table_records();

    Ok(match self.order {
      TableOrder::Sorted => match records.binary_search_by(|record| {
        let Ok(found) = record.tag();

        found.cmp(&tag)
      }) {
        Ok(position) => {
          let Ok(position_2) = fitted::<_, u64>(position);
          let Ok(at) = index(position_2);

          records.get(at)
        }
        Err(_insertion_position) => None,
      },
      TableOrder::Unsorted => records.iter().find(|record| {
        let Ok(found) = record.tag();

        found == tag
      }),
    })
  }

  fn with_table_directory(
    data: FontData<'a>,
    table_directory: TableDirectory<'a>,
    ttc_index: Option<u32>,
  ) -> Result<Self, ReadError> {
    let Ok(version) = table_directory.sfnt_version();

    match ::core::primitive::bool::then_some([TT_SFNT_VERSION, CFF_SFNT_VERSION, TRUE_SFNT_VERSION].contains(&version), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(ReadError::InvalidSfnt(version)),
    }

    let Ok(order) = table_directory.order();

    Ok(Self {
      data,
      table_directory,
      ttc_index,
      order,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use font_test_data::{AHEM, ttc::TTC};

  #[test]
  fn a_single_font_exposes_its_original_table_bytes() -> Result<(), ReadError> {
    let font = FontRef::new(AHEM)?;
    let Ok(tag) = Tag::new(b"head");
    let table = font.table_data(tag)?;
    let table_2 = table.ok_or(ReadError::TableIsMissing(tag))?;
    let Ok(bytes) = table_2.as_bytes();
    let Ok(collection_index) = font.ttc_index();
    let fonts = FontRef::fonts(AHEM)?;

    assert_eq!(bytes.first_chunk::<4>(), Some(&[0, 1, 0, 0]));
    assert_eq!(collection_index, None);
    assert_eq!(fonts.count(), 1);
    assert!(matches!(
      FontRef::from_index(AHEM, 1),
      Err(ReadError::InvalidCollectionIndex(1))
    ));

    Ok(())
  }

  #[test]
  fn collection_offsets_use_the_whole_file_as_their_base() -> Result<(), ReadError> {
    let collection = CollectionRef::new(TTC)?;
    let Ok(length) = collection.len();
    let Ok(empty) = collection.is_empty();
    let Ok(fonts) = collection.iter();
    let Ok(head) = Tag::new(b"head");

    assert_eq!(length, 2);
    assert_eq!(empty, Emptiness::NonEmpty);

    for (position, font) in (0u32..).zip(fonts) {
      let font_2 = font?;
      let Ok(collection_index) = font_2.ttc_index();
      let table = font_2.table_data(head)?;
      let table_2 = table.ok_or(ReadError::TableIsMissing(head))?;
      let Ok(bytes) = table_2.as_bytes();

      assert_eq!(collection_index, Some(position));
      assert_eq!(bytes.first_chunk::<4>(), Some(&[0, 1, 0, 0]));
    }

    assert!(matches!(
      collection.get(2),
      Err(ReadError::InvalidCollectionIndex(2))
    ));

    Ok(())
  }

  #[test]
  fn an_unsorted_directory_keeps_each_table_addressable() -> Result<(), ReadError> {
    let mut bytes = alloc::vec![0, 1, 0, 0, 0, 2, 0, 32, 0, 1, 0, 0];

    for (tag, offset) in [(b"zzzz", 44u32), (b"aaaa", 48)] {
      bytes.extend_from_slice(tag);
      bytes.extend_from_slice(&0u32.to_be_bytes());
      bytes.extend_from_slice(&offset.to_be_bytes());
      bytes.extend_from_slice(&4u32.to_be_bytes());
    }

    bytes.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    let font = FontRef::new(&bytes)?;

    for (tag, expected) in [(b"zzzz", [1, 2, 3, 4]), (b"aaaa", [5, 6, 7, 8])] {
      let Ok(tag_2) = Tag::new(tag);
      let table = font.table_data(tag_2)?;
      let table_2 = table.ok_or(ReadError::TableIsMissing(tag_2))?;
      let Ok(actual) = table_2.as_bytes();

      assert_eq!(actual, expected);
    }

    let Ok(missing) = Tag::new(b"none");

    let absent = font.table_data(missing)?;

    assert!(absent.is_none());

    Ok(())
  }

  #[test]
  fn invalid_directory_extents_and_targets_are_read_errors() -> Result<(), ReadError> {
    let missing_record = [0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];

    assert!(matches!(
      FontRef::new(&missing_record),
      Err(ReadError::OutOfBounds)
    ));
    assert!(matches!(
      FontRef::fonts(b"NOT_A_FONT"),
      Err(ReadError::OutOfBounds)
    ));

    let mut bytes = missing_record.to_vec();

    bytes.extend_from_slice(b"head");
    bytes.extend_from_slice(&0u32.to_be_bytes());
    bytes.extend_from_slice(&u32::MAX.to_be_bytes());
    bytes.extend_from_slice(&16u32.to_be_bytes());
    let font = FontRef::new(&bytes)?;
    let Ok(tag) = Tag::new(b"head");

    assert!(matches!(font.table_data(tag), Err(ReadError::OutOfBounds)));

    let missing_signature = [b't', b't', b'c', b'f', 0, 2, 0, 0, 0, 0, 0, 0];

    assert!(matches!(
      CollectionRef::new(&missing_signature),
      Err(ReadError::OutOfBounds)
    ));

    Ok(())
  }
}
