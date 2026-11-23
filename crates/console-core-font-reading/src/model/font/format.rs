//! Font format detection.

use crate::{FileRef, FontRead};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum FontFormat {
  Sfnt(u32),
  Type1,
  Cff(u32),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum FontContainer {
  Sfnt,
  Standalone,
}

impl FontFormat {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `FontFormat::new` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn new(data: &[u8]) -> Option<Self> {
    let sfnt = match FileRef::new(data) {
      Ok(FileRef::Collection(collection)) => {
        let Ok(count) = collection.len();
        Some(Self::Sfnt(count))
      }
      Ok(FileRef::Font(_font)) => Some(Self::Sfnt(1)),
      Err(_unreadable_sfnt_header) => None,
    };

    match sfnt {
      Some(format) => return Some(format),
      None => {}
    }

    let Ok(direct_header) = type1_header(data);
    let type1 = direct_header.or_else(|| {
      let payload = data.get(6..)?;
      let Ok(pfb_header) = type1_header(payload);
      pfb_header
    });

    match type1 {
      Some(_header) => return Some(Self::Type1),
      None => {}
    }

    let Ok(font_data) = crate::FontData::new(data);

    match crate::ps::cff::v1::Cff::read(font_data) {
      Ok(cff) => {
        let Ok(count) = cff.top_dicts().count();
        Some(Self::Cff(u32::from(count)))
      }
      Err(_unreadable_cff_header) => None,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `FontFormat::container` method returns `FontContainer` directly in the existing parser interface"
    )
  )]
  pub fn container(&self) -> FontContainer {
    match self {
      Self::Sfnt(_) => FontContainer::Sfnt,
      Self::Type1 | Self::Cff(_) => FontContainer::Standalone,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `FontFormat::num_fonts` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn num_fonts(&self) -> u32 {
    match self {
      Self::Sfnt(n) | Self::Cff(n) => *n,
      Self::Type1 => 1,
    }
  }
}

#[must_use]
fn type1_header(data: &[u8]) -> Result<Option<&[u8]>, console_core_never::Never> {
  Ok(
    data
      .strip_prefix(b"%!PS-AdobeFont")
      .or_else(|| data.strip_prefix(b"%!FontType")),
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{FontRef, TableProvider};

  #[test]
  fn check_formats() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(pure_cff) = match match FontRef::new(font_test_data::MATERIAL_ICONS_SUBSET) {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => {
        return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
      }
    }
    .cff()
    {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => {
        return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
      }
    }
    .offset_data()
    .as_bytes();
    use FontFormat::*;

    #[rustfmt::skip]
        let pairs = [
            (font_test_data::CANTARELL_VF_TRIMMED, Sfnt(1)),
            (font_test_data::TINOS_SUBSET, Sfnt(1)),
            (pure_cff, Cff(1)),
            (font_test_data::ttc::TTC, Sfnt(2)),
            (font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFA, Type1),
        ];
    for (data, expected_format) in pairs {
      let fixture_value =
        crate::test_support::fixture_value(FontFormat::new(data), "fixture value")?;
      assert_eq!(fixture_value, expected_format);
    }

    assert!(FontFormat::new(b"I'm not a font").is_none());
    ::core::result::Result::Ok(())
  }
}
