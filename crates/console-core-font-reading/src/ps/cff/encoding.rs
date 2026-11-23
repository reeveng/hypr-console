//! PostScript encodings.
//!
//! This maps font specific character codes to string ids.
//!
//! See "Glyph Organization" at <https://adobe-type-tools.github.io/font-tech-notes/pdfs/5176.CFF.pdf#page=18>
//! for an explanation of how charsets, encodings and glyphs are related.

use super::charset::Charset;
use crate::{
  Cursor, FontData, GlyphId, Presence, ReadError,
  ps::{encoding::PredefinedEncoding, string::Sid},
};

#[doc(inline)]
pub use super::v1::{EncodingRange1 as Range1, EncodingSupplement as Supplement};

#[derive(Clone)]
pub enum Encoding<'a> {
  Predefined(PredefinedEncoding),
  Custom(CustomEncoding<'a>),
}

impl<'a> Encoding<'a> {
  pub fn new(data: &'a [u8], offset: u64) -> Result<Self, ReadError> {
    match offset {
      0 => Ok(Self::Predefined(PredefinedEncoding::Standard)),
      1 => Ok(Self::Predefined(PredefinedEncoding::Expert)),
      _ => {
        let Ok(value_4) = console_core_number_conversion::indices_from(offset..);

        let checked_value = data.get(value_4).ok_or(ReadError::OutOfBounds)?;
        CustomEncoding::new(checked_value).map(Self::Custom)
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Encoding::map` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn map(&self, charset: &Charset, code: u8) -> Option<GlyphId> {
    match self {
      Self::Predefined(predefined) => {
        let sid = predefined.sid(code)?;

        match charset.glyph_id(sid) {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }
      }
      Self::Custom(custom) => custom.map(charset, code),
    }
  }
}

#[derive(Clone)]
pub enum CustomEncoding<'a> {
  Format0(&'a [u8], &'a [Supplement]),
  Format1(&'a [Range1], &'a [Supplement]),
}

impl<'a> CustomEncoding<'a> {
  pub fn new(data: &'a [u8]) -> Result<Self, ReadError> {
    let Ok(value_1) = FontData::new(data);

    let Ok(mut cursor) = value_1.cursor();
    let header = cursor.read::<u8>()?;
    let supplement = match header & 0x80 {
      0 => Presence::Absent,
      _ => Presence::Present,
    };
    let format = header & 0x7F;

    match format {
      0 => {
        let n_codes = cursor.read::<u8>()?;
        let codes = cursor.read_array({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(n_codes);
          migrated_value
        })?;
        let supp = Self::read_supplement(&mut cursor, supplement)?;
        Ok(Self::Format0(codes, supp))
      }
      1 => {
        let n_ranges = cursor.read::<u8>()?;
        let ranges = cursor.read_array({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(n_ranges);
          migrated_value
        })?;
        let supp = Self::read_supplement(&mut cursor, supplement)?;
        Ok(Self::Format1(ranges, supp))
      }
      _ => Err(ReadError::InvalidFormat({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i64>(format);
        migrated_value
      })),
    }
  }

  fn read_supplement(
    cursor: &mut Cursor<'a>,
    supplement: Presence,
  ) -> Result<&'a [Supplement], ReadError> {
    match supplement {
      Presence::Absent => Ok(&[]),
      Presence::Present => {
        let count = cursor.read::<u8>()?;
        cursor.read_array(u64::from(count))
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CustomEncoding::map` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn map(&self, charset: &Charset, code: u8) -> Option<GlyphId> {
    let read_sup = |sup: &[Supplement]| {
      sup.iter().find(|s| s.code == code).and_then(|s| {
        let Ok(value_2) = s.glyph.get();

        match charset.glyph_id(Sid::new(value_2)) {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }
      })
    };

    match self {
      Self::Format0(codes, sup) => read_sup(sup).or_else(|| {
        codes.iter().position(|c| *c == code).map(|gid| {
          let Ok(value_5) = GlyphId::new(::core::primitive::u32::wrapping_add(
            {
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(gid);
              migrated_value
            },
            1,
          ));

          value_5
        })
      }),
      Self::Format1(ranges, sup) => read_sup(sup).or_else(|| {
        let mut gid = 1u32;

        for range in ranges.iter() {
          let end = range.first.saturating_add(range.n_left);

          match ::core::primitive::bool::then_some((range.first..=end).contains(&code), ()) {
            ::core::option::Option::Some(()) => {
              match (
                {
                  let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                    ::core::primitive::u8::wrapping_sub(code, range.first),
                  );
                  migrated_value
                },
                &mut gid,
              ) {
                (explicit_arithmetic_amount, explicit_arithmetic_place) => {
                  *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
                    *explicit_arithmetic_place,
                    explicit_arithmetic_amount,
                  )
                }
              };

              let Ok(value_3) = GlyphId::new(gid);

              return Some(value_3);
            }
            ::core::option::Option::None => {}
          }

          match (
            ::core::primitive::u32::wrapping_add(
              {
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, u32>(range.n_left);
                migrated_value
              },
              1,
            ),
            &mut gid,
          ) {
            (explicit_arithmetic_amount, explicit_arithmetic_place) => {
              *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
                *explicit_arithmetic_place,
                explicit_arithmetic_amount,
              )
            }
          };
        }

        None
      }),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn predefined_standard() -> ::core::result::Result<(), crate::test_support::Failure> {
    let encoding = Encoding::Predefined(PredefinedEncoding::Standard);
    let charset = iso_adobe_charset()?;

    for code in 0..=255 {
      let gid = encoding.map(&charset, code);
      let fixture_value = crate::test_support::fixture_value(gid, "fixture value")?;
      assert_eq!(fixture_value, {
        let standard_glyph = crate::test_support::fixture_value(
          charset.glyph_id({
            let standard_sid = crate::test_support::fixture_value(
              PredefinedEncoding::Standard.sid(code),
              "fixture value",
            )?;
            standard_sid
          }),
          "fixture value",
        )?;
        standard_glyph
      });
    }

    Ok(())
  }

  #[test]
  fn predefined_expert() -> ::core::result::Result<(), crate::test_support::Failure> {
    let encoding = Encoding::Predefined(PredefinedEncoding::Expert);
    let charset = iso_expert_charset()?;

    for code in 0..=255 {
      let gid = encoding.map(&charset, code);
      let fixture_value = crate::test_support::fixture_value(gid, "fixture value")?;
      assert_eq!(fixture_value, {
        let expert_glyph = crate::test_support::fixture_value(
          charset.glyph_id({
            let expert_sid = crate::test_support::fixture_value(
              PredefinedEncoding::Expert.sid(code),
              "fixture value",
            )?;
            expert_sid
          }),
          "fixture value",
        )?;
        expert_glyph
      });
    }

    Ok(())
  }

  #[test]
  fn custom_format_0() -> ::core::result::Result<(), crate::test_support::Failure> {
    let codes = [3, 8, 9, 10, 11];
    let encoding = Encoding::Custom(CustomEncoding::Format0(&codes, &[]));
    let charset = iso_adobe_charset()?;

    for (i, code) in codes.into_iter().enumerate() {
      let fixture_value =
        crate::test_support::fixture_value(encoding.map(&charset, code), "fixture value")?;
      assert_eq!(fixture_value, {
        let Ok(value) = GlyphId::new(::core::primitive::u32::wrapping_add(
          {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(i);
            migrated_value
          },
          1,
        ));
        value
      });
    }

    Ok(())
  }

  #[test]
  fn custom_format_1() -> Result<(), ReadError> {
    let ranges = [(51, 4), (250, 5)].map(|(first, n_left)| Range1 { first, n_left });
    let encoding = Encoding::Custom(CustomEncoding::Format1(&ranges, &[]));
    let charset = iso_adobe_charset()?;

    for code in 0..=255 {
      let gid = encoding.map(&charset, code);
      let expected = match code {
        51..=55 => Some(::core::primitive::u32::wrapping_sub(
          {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(code);
            migrated_value
          },
          50,
        )),
        250..=255 => Some(::core::primitive::u32::wrapping_add(
          ::core::primitive::u32::wrapping_sub(
            {
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(code);
              migrated_value
            },
            250,
          ),
          6,
        )),
        _ => None,
      };
      assert_eq!(
        gid,
        expected.map(|argument| {
          let Ok(value_7) = GlyphId::new(argument);

          value_7
        })
      );
    }

    Ok(())
  }

  #[test]
  fn supplemental() -> ::core::result::Result<(), crate::test_support::Failure> {
    let supplement = [(40, 91), (122, 9)].map(|(code, glyph)| Supplement {
      code,
      glyph: <::font_types::BigEndian<::core::primitive::u16> as ::core::convert::From<
        ::core::primitive::u16,
      >>::from(glyph),
    });
    let encoding = Encoding::Custom(CustomEncoding::Format0(&[], &supplement));
    let charset = iso_adobe_charset()?;
    assert_eq!(
      ({
        let fixture_value =
          crate::test_support::fixture_value(encoding.map(&charset, 40), "fixture value")?;
        let Ok(value) = fixture_value.to_u32();
        value
      }),
      91
    );
    assert_eq!(
      ({
        let fixture_value =
          crate::test_support::fixture_value(encoding.map(&charset, 122), "fixture value")?;
        let Ok(value) = fixture_value.to_u32();
        value
      }),
      9
    );
    let fixture_value = crate::test_support::fixture_value(
      match charset.string_id(<::font_types::GlyphId as ::core::convert::From<
        ::core::primitive::u32,
      >>::from(91u32))
      {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      }
      .resolve_standard(),
      "fixture value",
    )?;
    assert_eq!(fixture_value, b"z");
    assert_eq!(
      {
        let supplement_name = crate::test_support::fixture_value(
          match charset.string_id(<::font_types::GlyphId as ::core::convert::From<
            ::core::primitive::u32,
          >>::from(9u32))
          {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .resolve_standard(),
          "fixture value",
        )?;
        supplement_name
      },
      b"parenleft"
    );
    ::core::result::Result::Ok(())
  }

  fn iso_adobe_charset() -> Result<Charset<'static>, ReadError> {
    Charset::new(Default::default(), 0, 256)
  }

  fn iso_expert_charset() -> Result<Charset<'static>, ReadError> {
    Charset::new(Default::default(), 1, 256)
  }
}
