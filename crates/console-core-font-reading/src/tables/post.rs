//! the [post (PostScript)](https://docs.microsoft.com/en-us/typography/opentype/spec/post#header) table

include!("../../generated/generated_post.rs");

impl<'a> Post<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Post::num_names` method returns `u64` directly in the existing parser interface")
  )]
  pub fn num_names(&self) -> u64 {
    let Ok(read_version) = self.version();

    match read_version {
      Version16Dot16::VERSION_1_0 => {
        let Ok(value_11) =
          console_core_number_conversion::fitted::<_, u64>(DEFAULT_GLYPH_NAMES.len());

        value_11
      }
      Version16Dot16::VERSION_2_0 => {
        let Ok(num_glyphs) = self.num_glyphs();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(match num_glyphs {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0,
          });
        migrated_value
      }
      _ => 0,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Post::glyph_name` method returns the optional value directly to callers that match its presence")
  )]
  pub fn glyph_name(&self, glyph_id: GlyphId16) -> Option<&str> {
    let Ok(value_1) = glyph_id.to_u16();

    let glyph_id_2 = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_1);
      migrated_value
    };
    let Ok(read_version) = self.version();

    match read_version {
      Version16Dot16::VERSION_1_0 => {
        let Ok(value_5) = console_core_number_conversion::index(glyph_id_2);

        DEFAULT_GLYPH_NAMES.get(value_5).copied()
      }
      Version16Dot16::VERSION_2_0 => {
        let Ok(value_6) = console_core_number_conversion::index(glyph_id_2);

        let Ok(glyph_name_index) = self.glyph_name_index();
        let glyph_name_index_2 = glyph_name_index?;
        let name_index = glyph_name_index_2.get(value_6)?;
        let Ok(value_2) = name_index.get();

        let idx = {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_2);
          migrated_value
        };
        let Ok(value_7) =
          console_core_number_conversion::fitted::<_, u64>(DEFAULT_GLYPH_NAMES.len());

        match ::core::primitive::bool::then_some(idx < value_7, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(value_8) = console_core_number_conversion::index(idx);

            return DEFAULT_GLYPH_NAMES.get(value_8).copied();
          }
          ::core::option::Option::None => {}
        }

        let Ok(value_9) =
          console_core_number_conversion::fitted::<_, u64>(DEFAULT_GLYPH_NAMES.len());

        let idx_2 = ::core::primitive::u64::wrapping_sub(idx, value_9);
        let Ok(string_data) = self.string_data();
        let string_data_2 = string_data?;
        let name = match string_data_2.get(idx_2) {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }?;

        name.map(|s| s.0)
      }
      _ => None,
    }
  }

  #[cfg(feature = "experimental_traverse")]
  #[must_use]
  fn traverse_string_data(
    &self,
  ) -> core::result::Result<FieldType<'a>, console_core_never::Never> {
    Ok(FieldType::I8(-42))
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PString<'a>(&'a str);

impl<'a> PString<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `PString::as_str` method returns a borrow tied to the source lifetime directly")
  )]
  pub fn as_str(&self) -> &'a str {
    self.0
  }
}

impl std::ops::Deref for PString<'_> {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    self.0
  }
}

impl PartialEq<&str> for PString<'_> {
  fn eq(&self, other: &&str) -> bool {
    self.0 == *other
  }
}

impl<'a> FontRead<'a> for PString<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let len: u8 = data.read_at(0)?;
    let Ok(bytes) = data.as_bytes();
    let Ok(count) = console_core_number_conversion::index(u32::from(len));
    let pstring = bytes
      .get(1..)
      .and_then(|tail| tail.get(..count))
      .ok_or(ReadError::OutOfBounds)?;
    let fault = ReadError::MalformedData("Must be valid ascii");

    match ::core::primitive::bool::then_some(!pstring.is_ascii(), ()) {
      ::core::option::Option::Some(()) => {
        return Err(fault);
      }
      ::core::option::Option::None => {}
    }

    std::str::from_utf8(pstring).map(PString).map_err(|_| fault)
  }
}

impl VarSize for PString<'_> {
  type Size = u8;
}

#[rustfmt::skip]
pub static DEFAULT_GLYPH_NAMES: [&str; 258] = [
    ".notdef", ".null", "nonmarkingreturn", "space", "exclam", "quotedbl", "numbersign", "dollar",
    "percent", "ampersand", "quotesingle", "parenleft", "parenright", "asterisk", "plus", "comma",
    "hyphen", "period", "slash", "zero", "one", "two", "three", "four", "five", "six", "seven",
    "eight", "nine", "colon", "semicolon", "less", "equal", "greater", "question", "at", "A", "B",
    "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U",
    "V", "W", "X", "Y", "Z", "bracketleft", "backslash", "bracketright", "asciicircum",
    "underscore", "grave", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n",
    "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z", "braceleft", "bar", "braceright",
    "asciitilde", "Adieresis", "Aring", "Ccedilla", "Eacute", "Ntilde", "Odieresis", "Udieresis",
    "aacute", "agrave", "acircumflex", "adieresis", "atilde", "aring", "ccedilla", "eacute",
    "egrave", "ecircumflex", "edieresis", "iacute", "igrave", "icircumflex", "idieresis", "ntilde",
    "oacute", "ograve", "ocircumflex", "odieresis", "otilde", "uacute", "ugrave", "ucircumflex",
    "udieresis", "dagger", "degree", "cent", "sterling", "section", "bullet", "paragraph",
    "germandbls", "registered", "copyright", "trademark", "acute", "dieresis", "notequal", "AE",
    "Oslash", "infinity", "plusminus", "lessequal", "greaterequal", "yen", "mu", "partialdiff",
    "summation", "product", "pi", "integral", "ordfeminine", "ordmasculine", "Omega", "ae",
    "oslash", "questiondown", "exclamdown", "logicalnot", "radical", "florin", "approxequal",
    "Delta", "guillemotleft", "guillemotright", "ellipsis", "nonbreakingspace", "Agrave", "Atilde",
    "Otilde", "OE", "oe", "endash", "emdash", "quotedblleft", "quotedblright", "quoteleft",
    "quoteright", "divide", "lozenge", "ydieresis", "Ydieresis", "fraction", "currency",
    "guilsinglleft", "guilsinglright", "fi", "fl", "daggerdbl", "periodcentered", "quotesinglbase",
    "quotedblbase", "perthousand", "Acircumflex", "Ecircumflex", "Aacute", "Edieresis", "Egrave",
    "Iacute", "Icircumflex", "Idieresis", "Igrave", "Oacute", "Ocircumflex", "apple", "Ograve",
    "Uacute", "Ucircumflex", "Ugrave", "dotlessi", "circumflex", "tilde", "macron", "breve",
    "dotaccent", "ring", "cedilla", "hungarumlaut", "ogonek", "caron", "Lslash", "lslash",
    "Scaron", "scaron", "Zcaron", "zcaron", "brokenbar", "Eth", "eth", "Yacute", "yacute", "Thorn",
    "thorn", "minus", "multiply", "onesuperior", "twosuperior", "threesuperior", "onehalf",
    "onequarter", "threequarters", "franc", "Gbreve", "gbreve", "Idotaccent", "Scedilla",
    "scedilla", "Cacute", "cacute", "Ccaron", "ccaron", "dcroat",
];

#[cfg(test)]
mod tests {
  use super::*;
  use font_test_data::{bebuffer::BeBuffer, post as test_data};

  #[test]
  fn a_glyph_name_requires_its_declared_ascii_bytes() -> Result<(), ReadError> {
    let Ok(empty) = FontData::new(&[0]);
    let Ok(short) = FontData::new(&[2, b'a']);
    let Ok(non_ascii) = FontData::new(&[1, 0xff]);
    let name = PString::read(empty)?;

    assert!(name.is_empty());
    assert!(matches!(PString::read(short), Err(ReadError::OutOfBounds)));
    assert!(matches!(
      PString::read(non_ascii),
      Err(ReadError::MalformedData(_))
    ));

    Ok(())
  }

  #[test]
  fn test_post() -> ::core::result::Result<(), crate::read::ReadError> {
    let table = Post::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(test_data::SIMPLE))?;
    assert_eq!(table.version(), Ok(Version16Dot16::VERSION_2_0));
    assert_eq!(
      table.underline_position(),
      Ok({
        let Ok(explicit_value) = FWord::new(-75);
        explicit_value
      })
    );
    assert_eq!(
      table.glyph_name({
        let Ok(explicit_value) = GlyphId16::new(1);
        explicit_value
      }),
      Some(".notdef")
    );
    assert_eq!(
      table.glyph_name({
        let Ok(explicit_value) = GlyphId16::new(2);
        explicit_value
      }),
      Some("space")
    );
    assert_eq!(
      table.glyph_name({
        let Ok(explicit_value) = GlyphId16::new(7);
        explicit_value
      }),
      Some("hello")
    );
    assert_eq!(
      table.glyph_name({
        let Ok(explicit_value) = GlyphId16::new(8);
        explicit_value
      }),
      Some("hi")
    );
    assert_eq!(
      table.glyph_name({
        let Ok(explicit_value) = GlyphId16::new(9);
        explicit_value
      }),
      Some("hola")
    );
    ::core::result::Result::Ok(())
  }

  fn make_basic_post(
    version: Version16Dot16,
    glyph_count: crate::Presence,
  ) -> ::core::result::Result<BeBuffer, ::console_core_never::Never> {
    let mut buf = font_test_data::be_buffer! { (version), ({ let Ok(value) = Fixed::from_i32(5); value }), (..([{ let Ok(value) = FWord::new(6); value }, { let Ok(value) = FWord::new(7); value }])), (0u32), (..([7u32, 8, 9, 10])) };

    match glyph_count {
      crate::Presence::Present => {
        buf.extend([0u16]);
      }
      crate::Presence::Absent => {}
    }

    ::core::result::Result::Ok(buf)
  }

  #[test]
  fn parse_versioned_fields_v1() -> ::core::result::Result<(), crate::read::ReadError> {
    let ::core::result::Result::Ok(buf) = make_basic_post(Version16Dot16::VERSION_1_0, crate::Presence::Present);
    let postv1 = Post::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;
    assert!(matches!(postv1.num_glyphs(), Ok(None)));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_versioned_fields_v2() -> ::core::result::Result<(), crate::read::ReadError> {
    let ::core::result::Result::Ok(buf) = make_basic_post(Version16Dot16::VERSION_2_0, crate::Presence::Absent);
    let postv2 = Post::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;

    assert!(matches!(postv2.num_glyphs(), Ok(None)));

    let ::core::result::Result::Ok(buf_2) = make_basic_post(Version16Dot16::VERSION_2_0, crate::Presence::Present);
    let postv2_2 = Post::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf_2))?;

    assert_eq!((postv2_2.num_glyphs()), Ok(Some(0)));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_versioned_fields_v3() -> ::core::result::Result<(), crate::read::ReadError> {
    let ::core::result::Result::Ok(buf) = make_basic_post(Version16Dot16::VERSION_3_0, crate::Presence::Present);
    let postv3 = Post::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;
    assert!(matches!(postv3.num_glyphs(), Ok(None)));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn num_names_defaults_to_zero_without_num_glyphs()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let ::core::result::Result::Ok(buf) = make_basic_post(Version16Dot16::VERSION_2_0, crate::Presence::Absent);
    let post = Post::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;

    assert_eq!(post.num_names(), 0);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn glyph_name_missing_string_data_returns_none()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let buf = font_test_data::be_buffer! { (Version16Dot16::VERSION_2_0), ({ let Ok(value) = Fixed::from_i32(5); value }), (..([{ let Ok(value) = FWord::new(6); value }, { let Ok(value) = FWord::new(7); value }])), (0u32), (..([7u32, 8, 9, 10])), (1u16), (258u16) };
    let post = Post::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;

    assert_eq!(
      post.glyph_name({
        let Ok(explicit_value) = GlyphId16::new(0);
        explicit_value
      }),
      None
    );
    ::core::result::Result::Ok(())
  }
}
