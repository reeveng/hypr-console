//! Parsing for PostScript INDEX objects.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/cff2#5-index-data>

use super::CffVersion;
use crate::codegen_prelude::*;
use crate::ps::error::Error;

#[doc(inline)]
pub use super::{v1::Index as Index1, v2::Index as Index2};

#[derive(Clone, Default)]
pub enum Index<'a> {
  #[default]
  Empty,
  Format1(Index1<'a>),
  Format2(Index2<'a>),
}

impl<'a> Index<'a> {
  pub fn new(data: &'a [u8], version: CffVersion) -> Result<Self, Error> {
    let Ok(data_2) = FontData::new(data);
    Ok(match version {
      CffVersion::Cff2 => {
        let Ok(value_2) = data_2.len();

        match ::core::primitive::bool::then_some(value_2 == 4 && {
          let checked_value = data_2.read_at::<u32>(0)?;
          checked_value
        } == 0, ())
        {
          ::core::option::Option::Some(()) => {
            return Ok(Self::Empty);
          }
          ::core::option::Option::None => {}
        }

        Index2::read(data_2).map(|ix| {
          <crate::ps::cff::index::Index<'_> as ::core::convert::From<
            crate::ps::cff::v2::Index<'_>,
          >>::from(ix)
        })?
      }
      CffVersion::Cff1 => {
        let Ok(value_3) = data_2.len();

        match ::core::primitive::bool::then_some(value_3 == 2 && {
          let checked_value_2 = data_2.read_at::<u16>(0)?;
          checked_value_2
        } == 0, ())
        {
          ::core::option::Option::Some(()) => {
            return Ok(Self::Empty);
          }
          ::core::option::Option::None => {}
        }

        Index1::read(data_2).map(|ix| {
          <crate::ps::cff::index::Index<'_> as ::core::convert::From<
            crate::ps::cff::v1::Index<'_>,
          >>::from(ix)
        })?
      }
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Index::count` method returns `u32` directly in the existing parser interface")
  )]
  pub fn count(&self) -> u32 {
    match self {
      Self::Empty => 0,
      Self::Format1(ix) => {
        let Ok(count) = ix.count();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(count);
        migrated_value
      }
      Self::Format2(ix) => {
        let Ok(count) = ix.count();

        count
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Index::subr_bias` method returns `i32` directly in the existing parser interface")
  )]
  pub fn subr_bias(&self) -> i32 {
    let count = self.count();

    match ::core::primitive::bool::then_some(count < 1240, ()) {
      ::core::option::Option::Some(()) => 107,
      ::core::option::Option::None => match ::core::primitive::bool::then_some(count < 33900, ()) {
        ::core::option::Option::Some(()) => 1131,
        ::core::option::Option::None => 32768,
      },
    }
  }

  pub fn size_in_bytes(&self) -> Result<u64, ReadError> {
    match self {
      Self::Empty => Ok(0),
      Self::Format1(ix) => ix.size_in_bytes(),
      Self::Format2(ix) => ix.size_in_bytes(),
    }
  }

  pub fn get_offset(&self, index: u64) -> Result<u64, Error> {
    match self {
      Self::Empty => Err(<crate::ps::error::Error as ::core::convert::From<
        crate::read::ReadError,
      >>::from(ReadError::OutOfBounds)),
      Self::Format1(ix) => ix.get_offset(index),
      Self::Format2(ix) => ix.get_offset(index),
    }
  }

  pub fn get(&self, index: u64) -> Result<&'a [u8], Error> {
    match self {
      Self::Empty => Err(<crate::ps::error::Error as ::core::convert::From<
        crate::read::ReadError,
      >>::from(ReadError::OutOfBounds)),
      Self::Format1(ix) => ix.get(index),
      Self::Format2(ix) => ix.get(index),
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Index::off_size` method returns `u8` directly in the existing parser interface")
  )]
  pub fn off_size(&self) -> u8 {
    match self {
      Self::Empty => 0,
      Self::Format1(ix) => {
        let Ok(off_size) = ix.off_size();

        off_size
      }
      Self::Format2(ix) => {
        let Ok(off_size) = ix.off_size();

        off_size
      }
    }
  }
}

impl<'a> From<Index1<'a>> for Index<'a> {
  fn from(value: Index1<'a>) -> Self {
    Self::Format1(value)
  }
}

impl<'a> From<Index2<'a>> for Index<'a> {
  fn from(value: Index2<'a>) -> Self {
    Self::Format2(value)
  }
}

impl<'a> Index1<'a> {
  pub fn size_in_bytes(&self) -> Result<u64, ReadError> {
    const HEADER_SIZE: u64 = 3;

    const EMPTY_SIZE: u64 = 2;
    let count = {
      let Ok(own_count) = self.count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(own_count);
      migrated_value
    };
    Ok(match count {
      0 => EMPTY_SIZE,
      _ => {
        let Ok(offsets) = self.offsets();
        let Ok(value_4) = console_core_number_conversion::fitted::<_, u64>(offsets.len());

        let map_err = self.get_offset(count).map_err(|_| ReadError::OutOfBounds)?;

        ::core::primitive::u64::wrapping_add(
          ::core::primitive::u64::wrapping_add(HEADER_SIZE, value_4),
          map_err,
        )
      }
    })
  }

  pub fn get_offset(&self, index: u64) -> Result<u64, Error> {
    let Ok(count) = self.count();
    let Ok(off_size) = self.off_size();
    let Ok(offsets) = self.offsets();

    read_offset(OffsetRequest {
      index,
      count: u64::from(count),
      offset_size: off_size,
      data: offsets,
    })
  }

  pub fn get(&self, index: u64) -> Result<&'a [u8], Error> {
    let offset = self.get_offset(index)?;
    let own_offset = self.get_offset(::core::primitive::u64::wrapping_add(index, 1))?;
    let Ok(value_5) = console_core_number_conversion::indices(offset..own_offset);

    let Ok(data) = self.data();

    data
      .get(value_5)
      .ok_or(<crate::ps::error::Error as ::core::convert::From<
        crate::read::ReadError,
      >>::from(ReadError::OutOfBounds))
  }
}

impl<'a> Index2<'a> {
  pub fn size_in_bytes(&self) -> Result<u64, ReadError> {
    const HEADER_SIZE: u64 = 5;

    const EMPTY_SIZE: u64 = 4;
    let count = {
      let Ok(own_count) = self.count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(own_count);
      migrated_value
    };
    Ok(match count {
      0 => EMPTY_SIZE,
      _ => {
        let Ok(offsets) = self.offsets();
        let Ok(value_6) = console_core_number_conversion::fitted::<_, u64>(offsets.len());

        let map_err = self.get_offset(count).map_err(|_| ReadError::OutOfBounds)?;

        ::core::primitive::u64::wrapping_add(
          ::core::primitive::u64::wrapping_add(HEADER_SIZE, value_6),
          map_err,
        )
      }
    })
  }

  pub fn get_offset(&self, index: u64) -> Result<u64, Error> {
    let Ok(count) = self.count();
    let Ok(off_size) = self.off_size();
    let Ok(offsets) = self.offsets();

    read_offset(OffsetRequest {
      index,
      count: u64::from(count),
      offset_size: off_size,
      data: offsets,
    })
  }

  pub fn get(&self, index: u64) -> Result<&'a [u8], Error> {
    let offset = self.get_offset(index)?;
    let own_offset = self.get_offset(::core::primitive::u64::wrapping_add(index, 1))?;
    let Ok(value_7) = console_core_number_conversion::indices(offset..own_offset);

    let Ok(data) = self.data();

    data
      .get(value_7)
      .ok_or(<crate::ps::error::Error as ::core::convert::From<
        crate::read::ReadError,
      >>::from(ReadError::OutOfBounds))
  }
}

struct OffsetRequest<'a> {
  index: u64,
  count: u64,
  offset_size: u8,
  data: &'a [u8],
}

fn read_offset(request: OffsetRequest<'_>) -> Result<u64, Error> {
  let OffsetRequest { index, count, offset_size, data } = request;

  match ::core::primitive::bool::then_some(index > count, ()) {
    ::core::option::Option::Some(()) => {
      Err(ReadError::OutOfBounds)?;
    }
    ::core::option::Option::None => {}
  }

  let data_offset = ::core::primitive::u64::wrapping_mul(index, {
    let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(offset_size);
    migrated_value
  });
  let Ok(offset_data_2) = FontData::new(data);
  match offset_size {
    1 => {
      let at = offset_data_2.read_at::<u8>(data_offset)?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(at);
      migrated_value
    }
    2 => {
      let at = offset_data_2.read_at::<u16>(data_offset)?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(at);
      migrated_value
    }
    3 => {
      let at = offset_data_2.read_at::<Uint24>(data_offset)?;
      let Ok(value_1) = at.to_u32();

      {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_1);
        migrated_value
      }
    }
    4 => {
      let at = offset_data_2.read_at::<u32>(data_offset)?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(at);
      migrated_value
    }
    _ => return Err(Error::InvalidIndexOffsetSize(offset_size)),
  }
  .checked_sub(1)
  .ok_or(Error::ZeroOffsetInIndex)
}

#[cfg(test)]
mod tests {
  use font_test_data::bebuffer::BeBuffer;

  use super::*;

  enum IndexParams {
    Format1 { off_size: u8, count: u64 },
    Format2 { off_size: u8, count: u64 },
  }

  #[test]
  fn index_format1_offsize1_count4() -> Result<(), crate::test_support::Failure> {
    test_index(IndexParams::Format1 {
      off_size: 1,
      count: 4,
    })
  }

  #[test]
  fn index_format1_offsize2_count64() -> Result<(), crate::test_support::Failure> {
    test_index(IndexParams::Format1 {
      off_size: 2,
      count: 64,
    })
  }

  #[test]
  fn index_format1_offsize3_count128() -> Result<(), crate::test_support::Failure> {
    test_index(IndexParams::Format1 {
      off_size: 3,
      count: 128,
    })
  }

  #[test]
  fn index_format1_offsize4_count256() -> Result<(), crate::test_support::Failure> {
    test_index(IndexParams::Format1 {
      off_size: 4,
      count: 256,
    })
  }

  #[test]
  fn index_format2_offsize1_count4() -> Result<(), crate::test_support::Failure> {
    test_index(IndexParams::Format2 {
      off_size: 4,
      count: 256,
    })
  }

  #[test]
  fn index_format2_offsize2_count64() -> Result<(), crate::test_support::Failure> {
    test_index(IndexParams::Format2 {
      off_size: 2,
      count: 64,
    })
  }

  #[test]
  fn index_format2_offsize3_count128() -> Result<(), crate::test_support::Failure> {
    test_index(IndexParams::Format2 {
      off_size: 3,
      count: 128,
    })
  }

  #[test]
  fn index_format2_offsize4_count256() -> Result<(), crate::test_support::Failure> {
    test_index(IndexParams::Format2 {
      off_size: 4,
      count: 256,
    })
  }

  fn test_index(params: IndexParams) -> Result<(), crate::test_support::Failure> {
    let (version, offset_size, count) = match params {
      IndexParams::Format1 { off_size, count } => (CffVersion::Cff1, off_size, count),
      IndexParams::Format2 { off_size, count } => (CffVersion::Cff2, off_size, count),
    };
    let options = MakeIndexArguments { version, offset_size };
    let buf = make_index(options, count)?;
    let index = Index::new(&buf, version)?;
    let built_off_size = match &index {
      Index::Empty => 0,
      Index::Format1(v1) => {
        let Ok(size) = v1.off_size();
        size
      }
      Index::Format2(v2) => {
        let Ok(size) = v2.off_size();
        size
      }
    };
    assert_eq!(built_off_size, offset_size);
    let expected_count = u32::try_from(count)?;
    assert_eq!(index.count(), expected_count);

    for ordinal in 0..count {
      let object = index.get(ordinal)?;
      let Ok(byte) = console_core_number_conversion::wrapped::<_, u8>(ordinal);
      let Ok(size) = console_core_number_conversion::index(ordinal.wrapping_add(1).wrapping_mul(10));
      assert!(object.iter().copied().eq(core::iter::repeat_n(byte, size)));
    }

    Ok(())
  }

  fn make_index(
    MakeIndexArguments { version, offset_size }: MakeIndexArguments,
    count: u64,
  ) -> Result<BeBuffer, crate::test_support::Failure> {
    let mut buf = BeBuffer::default();

    match version {
      CffVersion::Cff1 => {
        let value = u16::try_from(count)?;
        buf.extend([value]);
      }
      CffVersion::Cff2 => {
        let value = u32::try_from(count)?;
        buf.extend([value]);
      }
    }

    match count {
      0 => return Ok(buf),
      _nonempty => {}
    }

    buf.extend([offset_size]);
    let mut offset = 1u64;

    for ordinal in 0..=count {
      match offset_size {
        1 => {
          let value = u8::try_from(offset)?;
          buf.extend([value]);
        }
        2 => {
          let value = u16::try_from(offset)?;
          buf.extend([value]);
        }
        3 => {
          let value = u32::try_from(offset)?;
          let Ok(candidate) = Uint24::checked_new(value);
          let encoded = crate::test_support::fixture_value(candidate, "three-byte CFF INDEX offset")?;
          buf.extend([encoded]);
        }
        4 => {
          let value = u32::try_from(offset)?;
          buf.extend([value]);
        }
        other => return Err(crate::test_support::Failure::from(Error::InvalidIndexOffsetSize(other))),
      }

      offset = offset.wrapping_add(ordinal.wrapping_add(1).wrapping_mul(10));
    }

    for ordinal in 0..count {
      let Ok(byte) = console_core_number_conversion::wrapped::<_, u8>(ordinal);
      let Ok(length) = console_core_number_conversion::index(ordinal.wrapping_add(1).wrapping_mul(10));
      buf.extend(core::iter::repeat_n(byte, length));
    }

    Ok(buf)
  }

  struct MakeIndexArguments {
    version: CffVersion,
    offset_size: u8,
  }
}
