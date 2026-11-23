//! CFF version 2.

include!("../../../generated/generated_cff2.rs");

#[derive(Clone)]
pub struct Cff2<'a> {
  header: Cff2Header<'a>,
  global_subrs: Index<'a>,
}

impl<'a> Cff2<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Cff2::offset_data` method returns `FontData` directly in the existing parser interface"
    )
  )]
  pub fn offset_data(&self) -> FontData<'a> {
    let Ok(value_1) = self.header.offset_data();

    value_1
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Cff2::header` method returns a borrow tied to the source lifetime directly"
    )
  )]
  pub fn header(&self) -> &Cff2Header<'a> {
    &self.header
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Cff2::top_dict_data` method returns a borrow tied to the source lifetime directly"
    )
  )]
  pub fn top_dict_data(&self) -> &'a [u8] {
    let Ok(top_dict_data) = self.header.top_dict_data();

    top_dict_data
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Cff2::global_subrs` method returns `Index` directly in the existing parser interface"
    )
  )]
  pub fn global_subrs(&self) -> Index<'a> {
    self.global_subrs.clone()
  }
}

impl TopLevelTable for Cff2<'_> {
  const TAG: Tag = ({
    let Ok(value) = Tag::new(b"CFF2");
    value
  });
}

impl<'a> FontRead<'a> for Cff2<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let header = Cff2Header::read(data)?;
    let Ok(trailing_data) = header.trailing_data();
    let Ok(value_2) = FontData::new(trailing_data);

    let global_subrs = Index::read(value_2)?;
    Ok(Self {
      header,
      global_subrs,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{FontData, FontRead, FontRef, TableProvider};

  #[test]
  fn read_example_cff2_table() -> ::core::result::Result<(), crate::read::ReadError> {
    let cff2 = Cff2::read({
      let Ok(explicit_value) = FontData::new(font_test_data::cff2::EXAMPLE);
      explicit_value
    })?;
    assert_eq!(cff2.header().major_version(), Ok(2));
    assert_eq!(cff2.header().minor_version(), Ok(0));
    assert_eq!(cff2.header().header_size(), Ok(5));
    assert_eq!(cff2.top_dict_data().len(), 7);
    assert_eq!(cff2.global_subrs().count(), Ok(0));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn read_cantarell() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::CANTARELL_VF_TRIMMED)?;
    let cff2 = font.cff2()?;
    assert_eq!(cff2.header().major_version(), Ok(2));
    assert_eq!(cff2.header().minor_version(), Ok(0));
    assert_eq!(cff2.header().header_size(), Ok(5));
    assert_eq!(cff2.top_dict_data().len(), 7);
    assert_eq!(cff2.global_subrs().count(), Ok(0));
    ::core::result::Result::Ok(())
  }
}
