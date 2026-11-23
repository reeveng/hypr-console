//! The [name (Naming)](https://docs.microsoft.com/en-us/typography/opentype/spec/name) table

include!("../../generated/generated_name.rs");

pub use types::NameId;

impl<'a> Name<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Name::string_data` method returns `FontData` directly in the existing parser interface"
    )
  )]
  pub fn string_data(&self) -> FontData<'a> {
    let Ok(base) = self.offset_data();
    let Ok(off) = self.storage_offset();
    let Ok(value_9) = base.split_off({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(off);
      migrated_value
    });

    match value_9 {
      Some(strings) => strings,
      None => FontData::default(),
    }
  }
}

impl NameRecord {
  pub fn string<'a>(&self, data: FontData<'a>) -> Result<NameString<'a>, ReadError> {
    let Ok(string_offset) = self.string_offset();
    let Ok(value_10) = string_offset.non_null();

    let start = match value_10 {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0,
    };
    let end = ::core::primitive::u64::wrapping_add(start, {
      let Ok(length) = self.length();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(length);
      migrated_value
    });

    let Ok(value_1) = data.as_bytes();

    let Ok(value_3) = console_core_number_conversion::indices(start..end);

    let data_2 = value_1.get(value_3).ok_or(ReadError::OutOfBounds)?;

    let Ok(platform_id) = self.platform_id();
    let Ok(encoding_id) = self.encoding_id();
    let encoding = Encoding::new(platform_id, encoding_id);
    Ok(NameString {
      data: data_2,
      encoding,
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `NameRecord::is_unicode` method returns `bool` directly in the existing parser interface"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(
      explicit007_no_bool_return,
      reason = "the published `NameRecord::is_unicode` method retains the Boolean result used by existing predicate callers and implementations"
    )
  )]
  pub fn is_unicode(&self) -> bool {
    let Ok(platform_id) = self.platform_id();
    let Ok(encoding_id) = self.encoding_id();

    platform_id == 0 || (platform_id == 3 && [0, 1, 10].contains(&encoding_id))
  }
}

impl LangTagRecord {
  pub fn lang_tag<'a>(&self, data: FontData<'a>) -> Result<NameString<'a>, ReadError> {
    let Ok(lang_tag_offset) = self.lang_tag_offset();
    let Ok(value_11) = lang_tag_offset.non_null();

    let start = match value_11 {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0,
    };
    let end = ::core::primitive::u64::wrapping_add(start, {
      let Ok(length) = self.length();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(length);
      migrated_value
    });

    let Ok(value_2) = data.as_bytes();

    let Ok(value_4) = console_core_number_conversion::indices(start..end);

    let data_2 = value_2.get(value_4).ok_or(ReadError::OutOfBounds)?;

    let encoding = Encoding::Utf16Be;
    Ok(NameString {
      data: data_2,
      encoding,
    })
  }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct NameString<'a> {
  data: &'a [u8],
  encoding: Encoding,
}

impl<'a> NameString<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `NameString::chars` method returns `CharIter` directly in the existing parser interface"
    )
  )]
  pub fn chars(&self) -> CharIter<'a> {
    CharIter {
      data: self.data,
      encoding: self.encoding,
      pos: 0,
    }
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> traversal::SomeString<'a> for NameString<'a> {
  fn iter_chars(&self) -> Result<Box<dyn Iterator<Item = char> + 'a>, Never> {
    Ok(Box::new(self.into_iter()))
  }
}

#[cfg(feature = "experimental_traverse")]
impl NameRecord {
  #[must_use]
  fn traverse_string<'a>(
    &self,
    data: FontData<'a>,
  ) -> core::result::Result<traversal::FieldType<'a>, console_core_never::Never> {
    Ok(FieldType::StringOffset(traversal::StringOffset {
      offset: <crate::traversal::OffsetType as ::core::convert::From<
        ::core::result::Result<::font_types::Offset16, ::console_core_never::Never>,
      >>::from(self.string_offset()),
      target: self
        .string(data)
        .map(|text| -> Box<dyn traversal::SomeString<'a> + 'a> { Box::new(text) }),
    }))
  }
}

#[cfg(feature = "experimental_traverse")]
impl LangTagRecord {
  #[must_use]
  fn traverse_lang_tag<'a>(
    &self,
    data: FontData<'a>,
  ) -> core::result::Result<traversal::FieldType<'a>, console_core_never::Never> {
    Ok(FieldType::StringOffset(traversal::StringOffset {
      offset: <crate::traversal::OffsetType as ::core::convert::From<
        ::core::result::Result<::font_types::Offset16, ::console_core_never::Never>,
      >>::from(self.lang_tag_offset()),
      target: self
        .lang_tag(data)
        .map(|text| -> Box<dyn traversal::SomeString<'a> + 'a> { Box::new(text) }),
    }))
  }
}

impl<'a> IntoIterator for NameString<'a> {
  type Item = char;
  type IntoIter = CharIter<'a>;

  fn into_iter(self) -> Self::IntoIter {
    self.chars()
  }
}

impl std::fmt::Display for NameString<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    for c in self.chars() {
      c.fmt(f)?;
    }

    Ok(())
  }
}

impl std::fmt::Debug for NameString<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "\"{self}\"")
  }
}

#[derive(Clone)]
pub struct CharIter<'a> {
  data: &'a [u8],
  encoding: Encoding,
  pos: u64,
}

impl CharIter<'_> {
  fn bump_u16(&mut self) -> core::result::Result<Option<u16>, console_core_never::Never> {
    let next = match self.pos.checked_add(2) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(range) = console_core_number_conversion::indices(self.pos..next);
    let bytes = match self.data.get(range) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let pair = match bytes.first_chunk() {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    self.pos = next;

    Ok(Some(u16::from_be_bytes(*pair)))
  }

  fn bump_u8(&mut self) -> core::result::Result<Option<u8>, console_core_never::Never> {
    let Ok(at) = console_core_number_conversion::index(self.pos);
    let byte = match self.data.get(at) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    let checked_value = match self.pos.checked_add(1) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    self.pos = checked_value;

    Ok(Some(*byte))
  }
}

impl Iterator for CharIter<'_> {
  type Item = char;

  fn next(&mut self) -> Option<Self::Item> {
    match self.encoding {
      Encoding::Utf16Be => {
        let Ok(character) = self.next_utf16();
        character
      }
      Encoding::MacRoman => {
        let Ok(byte) = self.bump_u8();
        byte.map(|value| MacRomanMapping.decode(value))
      }
      Encoding::Unknown => None,
    }
  }
}

impl CharIter<'_> {
  fn next_utf16(&mut self) -> Result<Option<char>, console_core_never::Never> {
    let first = match self.bump_u16() {
      Ok(Some(unit)) => unit,
      Ok(None) => return Ok(None),
    };
    let replacement = core::char::REPLACEMENT_CHARACTER;
    let codepoint = match first {
      0xD800..=0xDBFF => {
        let low = match self.bump_u16() {
          Ok(Some(unit @ 0xDC00..=0xDFFF)) => unit,
          Ok(Some(_not_low_surrogate)) => {
            self.pos = self.pos.wrapping_sub(2);
            return Ok(Some(replacement));
          }
          Ok(None) => return Ok(Some(replacement)),
        };
        let high_bits = u32::from(first & 0x3FF).wrapping_shl(10);
        high_bits
          .wrapping_add(u32::from(low & 0x3FF))
          .wrapping_add(0x10000)
      }
      _single_unit => u32::from(first),
    };
    Ok(Some(match core::char::from_u32(codepoint) {
      Some(character) => character,
      None => replacement,
    }))
  }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Encoding {
  Utf16Be,
  MacRoman,
  Unknown,
}

impl Encoding {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Encoding::new` method returns `Encoding` directly in the existing parser interface"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the published `Encoding::new` method retains the positional argument order for `platform_id`, `encoding_id`"
    )
  )]
  pub fn new(platform_id: u16, encoding_id: u16) -> Encoding {
    match (platform_id, encoding_id) {
      (0, _) => Encoding::Utf16Be,
      (1, 0) => Encoding::MacRoman,
      (3, 0) => Encoding::Utf16Be,
      (3, 1) => Encoding::Utf16Be,
      (3, 10) => Encoding::Utf16Be,
      _ => Encoding::Unknown,
    }
  }
}

pub struct MacRomanMapping;

impl MacRomanMapping {
  const START_REMAP: u8 = 128;

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public Mac Roman codec maps each byte directly to a Unicode scalar"
    )
  )]
  #[must_use]
  pub fn decode(self, raw: u8) -> char {
    let upper_half = raw.checked_sub(Self::START_REMAP);

    match upper_half {
      Some(index) => match MAC_ROMAN_DECODE.get(
        match ::console_core_number_conversion::index::<::core::primitive::u8>(index) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        },
      ) {
        Some(&character) => character,
        None => char::REPLACEMENT_CHARACTER,
      },
      None => char::from(raw),
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `MacRomanMapping::encode` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn encode(self, c: char) -> Option<u8> {
    let raw_c = <u32 as ::core::convert::From<::core::primitive::char>>::from(c);
    let raw_c_2: u16 = match raw_c.try_into() {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
    }?;

    match ::core::primitive::bool::then_some(
      raw_c_2
        < ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u16>(Self::START_REMAP);
          migrated_value
        }),
      (),
    ) {
      ::core::option::Option::Some(()) => Some({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(raw_c_2);
        migrated_value
      }),
      ::core::option::Option::None => {
        match MAC_ROMAN_ENCODE.binary_search_by_key(&raw_c_2, |(unic, _)| *unic) {
          Ok(idx) => <[_]>::get(&MAC_ROMAN_ENCODE, idx).map(|searched_entry| searched_entry.1),
          Err(_insertion_position) => None,
        }
      }
    }
  }
}

#[rustfmt::skip]
static MAC_ROMAN_DECODE: [char; 128] = [
  '\u{00c4}', '\u{00c5}', '\u{00c7}', '\u{00c9}', '\u{00d1}', '\u{00d6}', '\u{00dc}', '\u{00e1}',
  '\u{00e0}', '\u{00e2}', '\u{00e4}', '\u{00e3}', '\u{00e5}', '\u{00e7}', '\u{00e9}', '\u{00e8}',
  '\u{00ea}', '\u{00eb}', '\u{00ed}', '\u{00ec}', '\u{00ee}', '\u{00ef}', '\u{00f1}', '\u{00f3}',
  '\u{00f2}', '\u{00f4}', '\u{00f6}', '\u{00f5}', '\u{00fa}', '\u{00f9}', '\u{00fb}', '\u{00fc}',
  '\u{2020}', '\u{00b0}', '\u{00a2}', '\u{00a3}', '\u{00a7}', '\u{2022}', '\u{00b6}', '\u{00df}',
  '\u{00ae}', '\u{00a9}', '\u{2122}', '\u{00b4}', '\u{00a8}', '\u{2260}', '\u{00c6}', '\u{00d8}',
  '\u{221e}', '\u{00b1}', '\u{2264}', '\u{2265}', '\u{00a5}', '\u{00b5}', '\u{2202}', '\u{2211}',
  '\u{220f}', '\u{03c0}', '\u{222b}', '\u{00aa}', '\u{00ba}', '\u{03a9}', '\u{00e6}', '\u{00f8}',
  '\u{00bf}', '\u{00a1}', '\u{00ac}', '\u{221a}', '\u{0192}', '\u{2248}', '\u{2206}', '\u{00ab}',
  '\u{00bb}', '\u{2026}', '\u{00a0}', '\u{00c0}', '\u{00c3}', '\u{00d5}', '\u{0152}', '\u{0153}',
  '\u{2013}', '\u{2014}', '\u{201c}', '\u{201d}', '\u{2018}', '\u{2019}', '\u{00f7}', '\u{25ca}',
  '\u{00ff}', '\u{0178}', '\u{2044}', '\u{20ac}', '\u{2039}', '\u{203a}', '\u{fb01}', '\u{fb02}',
  '\u{2021}', '\u{00b7}', '\u{201a}', '\u{201e}', '\u{2030}', '\u{00c2}', '\u{00ca}', '\u{00c1}',
  '\u{00cb}', '\u{00c8}', '\u{00cd}', '\u{00ce}', '\u{00cf}', '\u{00cc}', '\u{00d3}', '\u{00d4}',
  '\u{f8ff}', '\u{00d2}', '\u{00da}', '\u{00db}', '\u{00d9}', '\u{0131}', '\u{02c6}', '\u{02dc}',
  '\u{00af}', '\u{02d8}', '\u{02d9}', '\u{02da}', '\u{00b8}', '\u{02dd}', '\u{02db}', '\u{02c7}',
];

#[rustfmt::skip]
static MAC_ROMAN_ENCODE: [(u16, u8); 128] = [
    (160, 202), (161, 193), (162, 162), (163, 163),
    (165, 180), (167, 164), (168, 172), (169, 169),
    (170, 187), (171, 199), (172, 194), (174, 168),
    (175, 248), (176, 161), (177, 177), (180, 171),
    (181, 181), (182, 166), (183, 225), (184, 252),
    (186, 188), (187, 200), (191, 192), (192, 203),
    (193, 231), (194, 229), (195, 204), (196, 128),
    (197, 129), (198, 174), (199, 130), (200, 233),
    (201, 131), (202, 230), (203, 232), (204, 237),
    (205, 234), (206, 235), (207, 236), (209, 132),
    (210, 241), (211, 238), (212, 239), (213, 205),
    (214, 133), (216, 175), (217, 244), (218, 242),
    (219, 243), (220, 134), (223, 167), (224, 136),
    (225, 135), (226, 137), (227, 139), (228, 138),
    (229, 140), (230, 190), (231, 141), (232, 143),
    (233, 142), (234, 144), (235, 145), (236, 147),
    (237, 146), (238, 148), (239, 149), (241, 150),
    (242, 152), (243, 151), (244, 153), (245, 155),
    (246, 154), (247, 214), (248, 191), (249, 157),
    (250, 156), (251, 158), (252, 159), (255, 216),
    (305, 245), (338, 206), (339, 207), (376, 217),
    (402, 196), (710, 246), (711, 255), (728, 249),
    (729, 250), (730, 251), (731, 254), (732, 247),
    (733, 253), (937, 189), (960, 185), (8211, 208),
    (8212, 209), (8216, 212), (8217, 213), (8218, 226),
    (8220, 210), (8221, 211), (8222, 227), (8224, 160),
    (8225, 224), (8226, 165), (8230, 201), (8240, 228),
    (8249, 220), (8250, 221), (8260, 218), (8364, 219),
    (8482, 170), (8706, 182), (8710, 198), (8719, 184),
    (8721, 183), (8730, 195), (8734, 176), (8747, 186),
    (8776, 197), (8800, 173), (8804, 178), (8805, 179),
    (9674, 215), (63743, 240), (64257, 222), (64258, 223),
];

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn mac_roman() -> ::core::result::Result<(), crate::test_support::Failure> {
    static INPUT: &str = "Joachim Müller-Lancé";

    for c in INPUT.chars() {
      let enc = crate::test_support::fixture_value(MacRomanMapping.encode(c), "fixture value")?;
      assert_eq!(MacRomanMapping.decode(enc), c);
    }

    Ok(())
  }

  #[test]
  fn lone_surrogate_at_end() {
    let chars = CharIter {
      data: &[0x09, 0x04, 0xD8, 0x00],
      encoding: Encoding::Utf16Be,
      pos: 0,
    };

    assert!(chars.eq(['ऄ', std::char::REPLACEMENT_CHARACTER].into_iter()))
  }

  #[test]
  fn high_surrogate_followed_by_non_low() {
    let rep = std::char::REPLACEMENT_CHARACTER;

    let chars = CharIter {
      data: &[0xD8, 0x00, 0x00, 0x41],
      encoding: Encoding::Utf16Be,
      pos: 0,
    };
    assert!(chars.eq([rep, 'A'].into_iter()));

    let surrogate_pair = CharIter {
      data: &[0xD8, 0x00, 0xD8, 0x3D, 0xDE, 0x00],
      encoding: Encoding::Utf16Be,
      pos: 0,
    };
    assert!(surrogate_pair.eq([rep, '😀'].into_iter()));
  }
}
