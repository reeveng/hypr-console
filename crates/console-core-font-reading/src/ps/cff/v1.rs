//! CFF version 1.

include!("../../../generated/generated_cff.rs");

use crate::ps::{
  cff::{charset::Charset, dict},
  error::Error,
  string::Sid,
};

#[derive(Clone)]
pub struct Cff<'a> {
  header: CffHeader<'a>,
  names: Index<'a>,
  top_dicts: Index<'a>,
  strings: Index<'a>,
  global_subrs: Index<'a>,
}

impl<'a> Cff<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Cff::offset_data` method returns `FontData` directly in the existing parser interface"
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
      reason = "the published `Cff::header` method returns `CffHeader` directly in the existing parser interface"
    )
  )]
  pub fn header(&self) -> CffHeader<'a> {
    self.header.clone()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Cff::names` method returns `Index` directly in the existing parser interface"
    )
  )]
  pub fn names(&self) -> Index<'a> {
    self.names.clone()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Cff::name` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn name(&self, index: u64) -> Option<&'a [u8]> {
    match self.names.get(index) {
      Ok(value) => Some(value),
      Err(_invalid_name_index) => None,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Cff::top_dicts` method returns `Index` directly in the existing parser interface"
    )
  )]
  pub fn top_dicts(&self) -> Index<'a> {
    self.top_dicts.clone()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Cff::strings` method returns `Index` directly in the existing parser interface"
    )
  )]
  pub fn strings(&self) -> Index<'a> {
    self.strings.clone()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Cff::string` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn string(&self, id: Sid) -> Option<&'a [u8]> {
    match id.resolve_standard() {
      Ok(name) => Some(name),
      Err(ix) => match self.strings.get(ix) {
        Ok(value) => Some(value),
        Err(_invalid_string_index) => None,
      },
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Cff::global_subrs` method returns `Index` directly in the existing parser interface"
    )
  )]
  pub fn global_subrs(&self) -> Index<'a> {
    self.global_subrs.clone()
  }

  pub fn charset(&self, top_dict_index: u64) -> Result<Option<Charset<'a>>, Error> {
    let top_dict = self.top_dicts().get(top_dict_index)?;
    let offset_data = self.offset_data();
    let mut charset_offset: Option<u64> = None;
    let mut num_glyphs: Option<u32> = None;

    for entry in dict::entries(top_dict, None) {
      match entry {
        Ok(dict::Entry::Charset(offset)) => {
          charset_offset = Some(offset);
        }
        Ok(dict::Entry::CharstringsOffset(offset)) => {
          let Ok(charstrings) = offset_data.split_off(offset);
          let charstrings_data = charstrings.ok_or(ReadError::OutOfBounds)?;
          let index = Index::read(charstrings_data)?;
          let Ok(count) = index.count();
          num_glyphs = Some(u32::from(count));
        }

        Ok(dict::Entry::Ros { .. }) => {
          return Ok(None);
        }
        Ok(_) => {}
        Err(_unreadable_dict_entry) => continue,
      }
    }

    match charset_offset.zip(num_glyphs) {
      Some((offset, count)) => {
        let charset = Charset::new(offset_data, offset, count)?;

        Ok(Some(charset))
      }
      None => Ok(None),
    }
  }
}

impl TopLevelTable for Cff<'_> {
  const TAG: Tag = ({
    let Ok(value) = Tag::new(b"CFF ");
    value
  });
}

impl<'a> FontRead<'a> for Cff<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let header = CffHeader::read(data)?;
    let Ok(trailing_data) = header.trailing_data();
    let Ok(mut data_2) = FontData::new(trailing_data);
    let names = Index::read(data_2)?;
    let size_in_bytes = names.size_in_bytes()?;
    let Ok(value_3) = data_2.split_off(size_in_bytes);

    let top_dict_index_data = value_3.ok_or(ReadError::OutOfBounds)?;
    data_2 = top_dict_index_data;
    let top_dicts = Index::read(data_2)?;
    let top_dicts_size_in_bytes = top_dicts.size_in_bytes()?;
    let Ok(value_4) = data_2.split_off(top_dicts_size_in_bytes);

    let string_index_data = value_4.ok_or(ReadError::OutOfBounds)?;
    data_2 = string_index_data;
    let strings = Index::read(data_2)?;
    let strings_size_in_bytes = strings.size_in_bytes()?;
    let Ok(value_5) = data_2.split_off(strings_size_in_bytes);

    let global_subroutine_data = value_5.ok_or(ReadError::OutOfBounds)?;
    data_2 = global_subroutine_data;
    let global_subrs = Index::read(data_2)?;
    Ok(Self {
      header,
      names,
      top_dicts,
      strings,
      global_subrs,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{FontRef, TableProvider};

  #[test]
  fn read_noto_serif_display_cff() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::NOTO_SERIF_DISPLAY_TRIMMED)?;
    let cff = font.cff()?;
    assert_eq!(cff.header().major(), Ok(1));
    assert_eq!(cff.header().minor(), Ok(0));
    assert_eq!(cff.top_dicts().count(), Ok(1));
    assert_eq!(cff.names().count(), Ok(1));
    assert_eq!(cff.global_subrs.count(), Ok(17));
    let name = crate::test_support::fixture_value(cff.names().get(0), "fixture value")?;
    assert_eq!(name, b"NotoSerifDisplay-Regular");
    assert_eq!(cff.strings().count(), Ok(5));

    let fixture_value =
      crate::test_support::fixture_value(cff.string(Sid::new(391)), "fixture value")?;
    assert_eq!(fixture_value, b"2.9");

    assert_eq!(
      {
        let notice =
          crate::test_support::fixture_value(cff.string(Sid::new(392)), "fixture value")?;
        notice
      },
      b"Noto is a trademark of Google LLC."
    );

    assert_eq!(
      {
        let copyright =
          crate::test_support::fixture_value(cff.string(Sid::new(393)), "fixture value")?;
        copyright
      },
      b"Copyright 2022 The Noto Project Authors https:github.comnotofontslatin-greek-cyrillic"
    );

    assert_eq!(
      {
        let full_name =
          crate::test_support::fixture_value(cff.string(Sid::new(394)), "fixture value")?;
        full_name
      },
      b"Noto Serif Display Regular"
    );

    assert_eq!(
      {
        let family_name =
          crate::test_support::fixture_value(cff.string(Sid::new(395)), "fixture value")?;
        family_name
      },
      b"Noto Serif Display"
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn glyph_names() -> Result<(), crate::test_support::Failure> {
    test_glyph_names(
      font_test_data::NOTO_SERIF_DISPLAY_TRIMMED,
      &[".notdef", "i", "j", "k", "l"],
    )
  }

  #[test]
  fn icons_glyph_names() -> Result<(), crate::test_support::Failure> {
    test_glyph_names(font_test_data::MATERIAL_ICONS_SUBSET, &[".notdef", "_10k"])
  }

  fn test_glyph_names(
    font_data: &[u8],
    expected_names: &[&str],
  ) -> Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_data)?;
    let cff = font.cff()?;
    let maybe_charset = cff.charset(0)?;
    let charset = crate::test_support::fixture_value(maybe_charset, "CFF charset")?;
    let name_for_sid = |sid| -> Result<&str, crate::test_support::Failure> {
      let bytes = crate::test_support::fixture_value(cff.string(sid), "CFF string ID")?;
      let name = std::str::from_utf8(bytes)?;
      Ok(name)
    };
    let names_by_lookup = (0..charset.num_glyphs())
      .map(|id| {
        let Ok(glyph) = GlyphId::new(id);
        let sid = charset.string_id(glyph)?;
        name_for_sid(sid)
      })
      .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(names_by_lookup, expected_names);
    let names_by_iter = charset
      .iter()
      .map(|(_glyph, sid)| name_for_sid(sid))
      .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(names_by_iter, expected_names);
    Ok(())
  }
}
