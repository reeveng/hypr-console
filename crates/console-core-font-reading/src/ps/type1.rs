//! Type1 fonts.

use super::{
  Pitch,
  charmap::Charmap,
  cs::{self, CharstringContext, CharstringKind, CommandSink, NopFilterSink, TransformSink},
  encoding::PredefinedEncoding,
  error::Error,
  transform::{self, FontMatrix, ScaledFontMatrix, Transform},
};
use crate::{
  ReadError,
  model::pen::OutlinePen,
  types::{BoundingBox, Fixed, GlyphId},
};
use alloc::{string::String, vec::Vec};
use console_core_never::Never;
use console_core_number_conversion::{fitted, indices, wrapped};
use core::ops::Range;

pub struct Type1Font {
  name: Option<String>,
  full_name: Option<String>,
  family_name: Option<String>,
  weight: Option<String>,
  bbox: BoundingBox<Fixed>,
  italic_angle: i32,
  pitch: Pitch,
  underline_position: i32,
  underline_thickness: i32,
  matrix: ScaledFontMatrix,
  charstrings: Charstrings,
  subrs: Subrs,
  encoding: Option<RawEncoding>,
  weight_vector: Vec<Fixed>,
  unicode_charmap: Charmap,
}

impl Type1Font {
  pub fn new(data: &[u8]) -> Result<Self, Error> {
    let Ok(type1font) = Self::new_impl(data);

    type1font.ok_or(Error::InvalidFontFormat)
  }

  #[must_use]
  fn new_impl(data: &[u8]) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let raw_dicts = match {
      let Ok(explicit_value) = RawDicts::new(data);
      explicit_value
    } {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    let Ok(type1font) = Self::from_dicts(raw_dicts.base, &raw_dicts.private);

    Ok(type1font)
  }

  #[must_use]
  fn empty() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      name: None,
      full_name: None,
      family_name: None,
      weight: None,
      italic_angle: 0,
      pitch: Pitch::Proportional,
      underline_position: 0,
      underline_thickness: 0,
      matrix: ScaledFontMatrix {
        matrix: FontMatrix::IDENTITY,
        scale: 1000,
      },
      bbox: BoundingBox::default(),
      charstrings: Charstrings::default(),
      subrs: Subrs::default(),
      encoding: None,
      weight_vector: Vec::new(),
      unicode_charmap: Charmap::default(),
    })
  }

  #[must_use]
  fn from_dicts(
    base: &[u8],
    private: &[u8],
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(mut font) = Self::empty();

    let mut encoding_offset = None;
    let Ok(mut parser) = Parser::new(base);

    let Ok(base_input) = parser.unparsed_bytes();

    for _ in base_input {
      let token = match parser.next() {
        Ok(Some(token)) => token,
        Ok(None) => break,
      };

      match font.read_base_entry(token, &mut parser, &mut encoding_offset) {
        Ok(()) => {}
        Err(_invalid_font_matrix) => return Ok(None),
      }
    }

    match font.read_private_dict(private) {
      Ok(()) => {}
      Err(_invalid_private_dictionary) => return Ok(None),
    }

    match ::core::primitive::bool::then_some(font.charstrings.index.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    match encoding_offset {
      Some(encoding_start) => {
        let Ok(value_17) = console_core_number_conversion::indices_from(encoding_start..);

        let checked_value = match base.get(value_17) {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        };
        let Ok(mut encoding_parser) = Parser::new(checked_value);
        let Ok(parser_encoding) = encoding_parser.read_encoding(&font.charstrings);
        let encoding = match parser_encoding {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        };
        font.encoding = Some(encoding);
      }
      None => {}
    }

    #[cfg(feature = "agl")]
    {
      font.unicode_charmap = Charmap::from_glyph_names(font.glyph_names());
    }

    Ok(Some(font))
  }

  fn read_private_dict(&mut self, private: &[u8]) -> Result<(), ReadError> {
    let Ok(mut parser) = Parser::new(private);
    let Ok(original_input) = parser.unparsed_bytes();
    let mut len_iv = 4;

    for _ in original_input {
      let token = match parser.next() {
        Ok(Some(token)) => token,
        Ok(None) => break,
      };

      match token {
        Token::Name(b"lenIV") => {
          let Ok(length) = parser.read_int();
          let value = length.ok_or(ReadError::MalformedData("invalid Type 1 lenIV"))?;
          len_iv = value;
        }
        Token::Name(b"Subrs") => match self.subrs.index.is_empty().then_some(()) {
          Some(()) => {
            let Ok(subroutines) = parser.read_subrs(len_iv);
            let parsed = subroutines.ok_or(ReadError::MalformedData("invalid Type 1 Subrs"))?;
            self.subrs = parsed;
          }
          None => {}
        },
        Token::Name(b"CharStrings") => match self.charstrings.index.as_slice() {
          [] => {
            let Ok(charstrings) = parser.read_charstrings(len_iv);
            let parsed =
              charstrings.ok_or(ReadError::MalformedData("invalid Type 1 CharStrings"))?;
            self.charstrings = parsed;
          }
          [_, ..] => {}
        },
        Token::Raw(_)
        | Token::Int(_)
        | Token::LitString(_)
        | Token::HexString(_)
        | Token::Proc(_)
        | Token::Binary(_)
        | Token::Name(_) => {}
      }
    }

    Ok(())
  }

  fn read_base_entry(
    &mut self,
    token: Token<'_>,
    parser: &mut Parser<'_>,
    encoding_offset: &mut Option<u64>,
  ) -> Result<(), crate::ReadError> {
    match token {
      Token::Name(b"FontName") => {
        let Ok(string) = parser.read_string();

        self.name = string
      }
      Token::Name(b"FullName") => {
        let Ok(string) = parser.read_string();

        self.full_name = string
      }
      Token::Name(b"FamilyName") => {
        let Ok(string) = parser.read_string();

        self.family_name = string
      }
      Token::Name(b"Weight") => {
        match ::core::primitive::bool::then_some(self.weight.is_none(), ()) {
          ::core::option::Option::Some(()) => {
            let Ok(string) = parser.read_string();
            self.weight = string;
          }
          ::core::option::Option::None => {}
        }
      }
      Token::Name(b"ItalicAngle") => {
        let Ok(num_as_int) = parser.read_num_as_int();

        self.italic_angle = match num_as_int {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => 0,
        }
      }
      Token::Name(b"IsFixedPitch") => {
        let Ok(next) = parser.next();

        self.pitch = match next {
          Some(Token::Raw(b"true")) => Pitch::Fixed,
          Some(_) | None => Pitch::Proportional,
        }
      }
      Token::Name(b"UnderlinePosition") => {
        let Ok(num_as_int) = parser.read_num_as_int();
        self.underline_position = match num_as_int {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => 0,
        };
      }
      Token::Name(b"UnderlineThickness") => {
        let Ok(num_as_int) = parser.read_num_as_int();
        self.underline_thickness = match num_as_int {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => 0,
        };
      }
      Token::Name(b"FontBBox") => {
        let Ok(font_bbox) = parser.read_font_bbox();

        match font_bbox {
          Some([x_min, y_min, x_max, y_max]) => {
            self.bbox = BoundingBox {
              x_min,
              y_min,
              x_max,
              y_max,
            };
          }
          ::core::option::Option::None => {}
        }
      }
      Token::Name(b"FontMatrix") => {
        let Ok(parser_font_matrix) = parser.read_font_matrix();
        let font_matrix = parser_font_matrix
          .ok_or(crate::ReadError::MalformedData("invalid Type 1 FontMatrix"))?;

        self.matrix = font_matrix
      }

      Token::Name(b"Encoding") => *encoding_offset = Some(parser.pos),
      Token::Name(b"WeightVector") => {
        let Ok(weight_vector) = parser.read_weight_vector();

        match weight_vector {
          Some(weights) => {
            self.weight_vector = weights;
          }
          None => {}
        }
      }
      Token::Raw(_)
      | Token::Int(_)
      | Token::LitString(_)
      | Token::HexString(_)
      | Token::Proc(_)
      | Token::Binary(_)
      | Token::Name(_) => {}
    }

    Ok(())
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::name` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::full_name` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn full_name(&self) -> Option<&str> {
    self.full_name.as_deref()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::family_name` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn family_name(&self) -> Option<&str> {
    self.family_name.as_deref()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::weight` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn weight(&self) -> Option<&str> {
    self.weight.as_deref()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::italic_angle` method returns `i32` directly in the existing parser interface"
    )
  )]
  pub fn italic_angle(&self) -> i32 {
    self.italic_angle
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::pitch` method returns `Pitch` directly in the existing parser interface"
    )
  )]
  pub fn pitch(&self) -> Pitch {
    self.pitch
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::underline_position` method returns `i32` directly in the existing parser interface"
    )
  )]
  pub fn underline_position(&self) -> i32 {
    self.underline_position
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::underline_thickness` method returns `i32` directly in the existing parser interface"
    )
  )]
  pub fn underline_thickness(&self) -> i32 {
    self.underline_thickness
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::bbox` method returns `BoundingBox` directly in the existing parser interface"
    )
  )]
  pub fn bbox(&self) -> BoundingBox<Fixed> {
    self.bbox
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::num_glyphs` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn num_glyphs(&self) -> u32 {
    let Ok(num_glyphs) = self.charstrings.num_glyphs();

    num_glyphs
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::upem` method returns `i32` directly in the existing parser interface"
    )
  )]
  pub fn upem(&self) -> i32 {
    self.matrix.scale
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::matrix` method returns `FontMatrix` directly in the existing parser interface"
    )
  )]
  pub fn matrix(&self) -> FontMatrix {
    self.matrix.matrix
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::transform` method returns `Transform` directly in the existing parser interface"
    )
  )]
  pub fn transform(&self, ppem: Option<f32>) -> Transform {
    let scale = ppem.map(|ppem_2| Transform::compute_scale(ppem_2, self.upem()));
    Transform {
      matrix: self.matrix(),
      scale,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::encoding` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn encoding(&self) -> Option<Encoding<'_>> {
    self.encoding.as_ref().map(|enc| Encoding {
      encoding: enc,
      charstrings: &self.charstrings,
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::unicode_charmap` method returns a borrow tied to the source lifetime directly"
    )
  )]
  pub fn unicode_charmap(&self) -> &Charmap {
    &self.unicode_charmap
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::glyph_name` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn glyph_name(&self, gid: GlyphId) -> Option<&str> {
    let Ok(value_1) = gid.to_u32();

    let Ok(name) = self.charstrings.name(value_1);

    name
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::glyph_names` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn glyph_names(&self) -> impl Iterator<Item = (GlyphId, &str)> {
    (0..self.num_glyphs()).filter_map(|idx| {
      let Ok(value_7) = GlyphId::new(idx);

      let Ok(own_name) = self.charstrings.name(idx);
      let name = own_name?;

      Some((value_7, name))
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Type1Font::remapped_gid` method returns `GlyphId` directly in the existing parser interface"
    )
  )]
  pub fn remapped_gid(&self, original_gid: GlyphId) -> GlyphId {
    let stored_notdef = match self.charstrings.orig_notdef_index {
      Some(position) => position,
      None => return original_gid,
    };

    match (original_gid == GlyphId::NOTDEF).then_some(()) {
      Some(()) => {
        let Ok(numeric_id) = wrapped::<_, u32>(stored_notdef);
        let Ok(remapped) = GlyphId::new(numeric_id);

        remapped
      }
      None => {
        let Ok(numeric_id) = original_gid.to_u32();

        match (u64::from(numeric_id) == stored_notdef).then_some(()) {
          Some(()) => GlyphId::NOTDEF,
          None => original_gid,
        }
      }
    }
  }

  pub fn evaluate_charstring(
    &self,
    gid: GlyphId,
    sink: &mut impl CommandSink,
  ) -> Result<Option<Fixed>, Error> {
    let Ok(value_4) = gid.to_u32();

    let Ok(charstring) = self.charstrings.get(value_4);
    let charstring_data = charstring.ok_or(ReadError::OutOfBounds)?;
    cs::evaluate(self, None, charstring_data, sink)
  }

  pub fn draw(
    &self,
    gid: GlyphId,
    ppem: Option<f32>,
    pen: &mut impl OutlinePen,
  ) -> Result<Option<f32>, Error> {
    let mut nop_filter = NopFilterSink::new(pen);
    let transform = self.transform(ppem);
    let mut transformer = TransformSink::new(&mut nop_filter, transform);
    let width = self.evaluate_charstring(gid, &mut transformer)?;
    Ok(width.map(|w| {
      let Ok(value_5) = transform.transform_h_metric(w).to_f32();

      value_5.max(0.0)
    }))
  }
}

impl CharstringContext for Type1Font {
  fn kind(&self) -> CharstringKind {
    CharstringKind::Type1
  }

  fn seac_components(&self, base_code: i32, accent_code: i32) -> Result<[&[u8]; 2], Error> {
    let decode = |code: i32| {
      let map_err = code.try_into().map_err(|_| Error::InvalidSeacCode(code))?;
      let name = PredefinedEncoding::Standard.name(map_err);
      let Ok(index_for_name) = self.charstrings.index_for_name(name);

      index_for_name
        .and_then(|idx| {
          let Ok(charstring) = self.charstrings.get(idx);

          charstring
        })
        .ok_or(Error::InvalidSeacCode(code))
    };
    let base = decode(base_code)?;
    let accent = decode(accent_code)?;
    Ok([base, accent])
  }

  fn subr(&self, index: i32) -> Result<&[u8], Error> {
    let Ok(own_subr) = self.subrs.get({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(index);
      migrated_value
    });
    let subr = own_subr.ok_or(ReadError::OutOfBounds)?;

    Ok(subr)
  }

  fn global_subr(&self, _index: i32) -> Result<&[u8], Error> {
    Err(Error::MissingSubroutines)
  }

  fn weight_vector(&self) -> &[Fixed] {
    &self.weight_vector
  }
}

#[derive(Clone)]
pub struct Encoding<'a> {
  encoding: &'a RawEncoding,
  charstrings: &'a Charstrings,
}

impl<'a> Encoding<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Encoding::predefined` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn predefined(&self) -> Option<PredefinedEncoding> {
    match self.encoding {
      RawEncoding::Predefined(encoding) => Some(*encoding),
      RawEncoding::Custom(_) => None,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Encoding::glyph_name` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn glyph_name(&self, code: u8) -> Option<&'a str> {
    match self.encoding {
      RawEncoding::Predefined(pre) => Some(pre.name(code)),
      RawEncoding::Custom(custom) => {
        let Ok(value_26) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(code);
          migrated_value
        });

        let checked_value = custom.get(value_26)?;
        let Ok(value_6) = checked_value.to_u32();

        let Ok(name) = self.charstrings.name(value_6);

        name
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
  pub fn map(&self, code: u8) -> Option<GlyphId> {
    match self.encoding {
      RawEncoding::Predefined(pre) => {
        let Ok(index_for_name) = self.charstrings.index_for_name(pre.name(code));

        index_for_name.map(|argument| {
          let Ok(value_21) = GlyphId::new(argument);

          value_21
        })
      }
      RawEncoding::Custom(custom) => {
        let Ok(value_31) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(code);
          migrated_value
        });

        custom.get(value_31).copied()
      }
    }
  }
}

struct RawDicts<'a> {
  base: &'a [u8],
  private: Vec<u8>,
}

impl<'a> RawDicts<'a> {
  #[must_use]
  fn new(data: &'a [u8]) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(pfb_tag) = decode_pfb_tag(data, 0);

    Ok(match pfb_tag {
      Some((PfbSegment::Text, base_size)) => {
        let data_2 = match data.get(6..) {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        };
        (match {
          let Ok(explicit_value) = verify_header(data_2);
          explicit_value
        } {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        });
        let Ok(value_32) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(base_size);
          migrated_value
        });

        let (base_dict, raw_private_dict) = match data_2.split_at_checked(value_32) {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        };

        let Ok(pfb_binary_segments) = decode_pfb_binary_segments(raw_private_dict);
        let private_dict = ({
          let Ok(explicit_value) = decrypt(
            pfb_binary_segments.flat_map(|segment| segment.iter().copied()),
            EEXEC_SEED,
          );
          explicit_value
        })
        .skip(4)
        .collect::<Vec<_>>();
        Some(Self {
          base: base_dict,
          private: private_dict,
        })
      }
      Some((PfbSegment::Binary, _)) | None => {
        (match {
          let Ok(explicit_value) = verify_header(data);
          explicit_value
        } {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        });

        let start = match {
          let Ok(explicit_value) = find_eexec_data(data);
          explicit_value
        } {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        };
        let Ok(value_33) = console_core_number_conversion::index(start);

        let (base_dict, raw_private_dict) = match data.split_at_checked(value_33) {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        };
        let carries_hex = raw_private_dict
          .first_chunk::<4>()
          .is_some_and(|prefix| prefix.iter().all(u8::is_ascii_hexdigit));
        let private_dict = match ::core::primitive::bool::then_some(carries_hex, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(hex) = decode_hex(raw_private_dict);

            ({
              let Ok(explicit_value) = decrypt(hex, EEXEC_SEED);
              explicit_value
            })
            .skip(4)
            .collect::<Vec<_>>()
          }
          ::core::option::Option::None => ({
            let Ok(explicit_value) = decrypt(raw_private_dict.iter().copied(), EEXEC_SEED);
            explicit_value
          })
          .skip(4)
          .collect::<Vec<_>>(),
        };
        Some(Self {
          base: base_dict,
          private: private_dict,
        })
      }
    })
  }
}

#[must_use]
fn verify_header(data: &[u8]) -> core::result::Result<Option<()>, console_core_never::Never> {
  Ok((data.starts_with(b"%!PS-AdobeFont") || data.starts_with(b"%!FontType")).then_some(()))
}

const PFB_TEXT_SEGMENT_TAG: u16 = 0x8001;
const PFB_BINARY_SEGMENT_TAG: u16 = 0x8002;

#[derive(Debug, PartialEq, Eq)]
enum PfbSegment {
  Text,
  Binary,
}

#[must_use]
fn decode_pfb_tag(
  data: &[u8],
  start: u64,
) -> core::result::Result<Option<(PfbSegment, u32)>, console_core_never::Never> {
  let end = match start.checked_add(6) {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let Ok(range) = console_core_number_conversion::indices(start..end);
  let bytes = match data.get(range) {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let header = match bytes.first_chunk::<6>() {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let [kind, marker, length @ ..] = *header;
  let tag = u16::from_be_bytes([kind, marker]);

  let segment = match tag {
    PFB_TEXT_SEGMENT_TAG => PfbSegment::Text,
    PFB_BINARY_SEGMENT_TAG => PfbSegment::Binary,
    _ => return Ok(None),
  };

  Ok(Some((segment, u32::from_le_bytes(length))))
}

#[must_use]
fn decode_pfb_binary_segments(data: &[u8]) -> Result<impl Iterator<Item = &[u8]> + '_, Never> {
  let ::core::result::Result::Ok(max_segments) =
    ::console_core_number_conversion::fixed_index(data.len().wrapping_div(6));
  Ok(
    core::iter::repeat_n(
      (),
      match ::console_core_number_conversion::index(max_segments) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    )
    .scan(0u64, move |position, ()| {
      let Ok(pfb_tag) = decode_pfb_tag(data, *position);
      let (segment, length) = pfb_tag?;

      match segment {
        PfbSegment::Text => return None,
        PfbSegment::Binary => {}
      }

      let start = position.checked_add(6)?;
      let end = start.checked_add(u64::from(length))?;
      let Ok(range) = indices(start..end);
      let payload = data.get(range)?;
      *position = end;
      Some(payload)
    }),
  )
}

#[must_use]
fn find_eexec_data(data: &[u8]) -> core::result::Result<Option<u64>, console_core_never::Never> {
  let Ok(mut parser) = Parser::new(data);

  let Ok(remaining_input) = parser.unparsed_bytes();

  for _ in remaining_input {
    let token = match parser.next() {
      Ok(Some(token)) => token,
      Ok(None) => break,
    };

    match ::core::primitive::bool::then_some(token != Token::Raw(b"eexec"), ()) {
      ::core::option::Option::Some(()) => {
        continue;
      }
      ::core::option::Option::None => {}
    }

    return eexec_payload_start(data, parser.pos);
  }

  Ok(None)
}

fn eexec_payload_start(data: &[u8], start: u64) -> Result<Option<u64>, Never> {
  let Ok(range) = console_core_number_conversion::indices_from(start..);
  let tail = match data.get(range) {
    Some(bytes) => bytes,
    None => return Ok(None),
  };
  let mut linefeed = None;

  for (offset, &byte) in tail.iter().enumerate() {
    let Ok(distance) = fitted::<_, u64>(offset);
    let position = start.wrapping_add(distance);

    match byte {
      b' ' | b'\t' => {}
      b'\n' => linefeed = Some(position),
      b'\r' => {
        let next_linefeed = match linefeed {
          Some(known) => known,
          None => {
            let Ok(found) = find_linefeed(tail.get(offset..), position);
            linefeed = Some(found);
            found
          }
        };

        match next_linefeed.cmp(&position) {
          core::cmp::Ordering::Less => return Ok(Some(position)),
          core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {}
        }
      }
      _payload => return Ok(Some(position)),
    }
  }

  Ok(None)
}

fn find_linefeed(bytes: Option<&[u8]>, from: u64) -> Result<u64, Never> {
  Ok(
    match bytes.and_then(|tail| tail.iter().position(|&byte| byte == b'\n')) {
      Some(offset) => {
        let Ok(fixed_offset) = fitted::<_, u64>(offset);
        from.wrapping_add(fixed_offset)
      }
      None => 0,
    },
  )
}

#[must_use]
fn decode_hex(data: &[u8]) -> Result<impl Iterator<Item = u8> + '_, Never> {
  let input = data.iter().copied();
  Ok(
    core::iter::repeat_n((), data.len()).scan(input, |bytes, ()| {
      let mut pending_nibble = None;

      for byte in bytes.by_ref() {
        match byte_spacing(byte) {
          Ok(ByteSpacing::Whitespace) => continue,
          Ok(ByteSpacing::NonWhitespace) => {}
        }

        let nibble = match byte {
          b'0'..=b'9' => byte.wrapping_sub(b'0'),
          b'a'..=b'f' => byte.wrapping_sub(b'a').wrapping_add(10),
          b'A'..=b'F' => byte.wrapping_sub(b'A').wrapping_add(10),
          _invalid_hex_digit => break,
        };

        match pending_nibble.take() {
          Some(first) => return Some(u8::wrapping_shl(first, 4) | nibble),
          None => pending_nibble = Some(nibble),
        }
      }

      pending_nibble.take().map(|nibble| nibble.wrapping_shl(4))
    }),
  )
}

const EEXEC_SEED: u32 = 55665;

const CHARSTRING_SEED: u32 = 4330;

#[must_use]
fn decrypt(
  bytes: impl Iterator<Item = u8>,
  mut seed: u32,
) -> core::result::Result<impl Iterator<Item = u8>, console_core_never::Never> {
  Ok(bytes.map(move |b| {
    let b_2 = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(b);
      migrated_value
    };
    let plain = b_2 ^ ::core::primitive::u32::wrapping_shr(seed, 8_u32);
    seed = b_2
      .wrapping_add(seed)
      .wrapping_mul(52845)
      .wrapping_add(22719)
      & 0xFFFF;

    {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(plain);
      migrated_value
    }
  }))
}

enum ByteSpacing {
  Whitespace,
  NonWhitespace,
}

#[must_use]
fn byte_spacing(byte: u8) -> core::result::Result<ByteSpacing, console_core_never::Never> {
  Ok(match byte {
    b' ' | b'\n' | b'\r' | b'\t' | b'\0' | 0x0C => ByteSpacing::Whitespace,
    _ => ByteSpacing::NonWhitespace,
  })
}

enum TokenDelimiter {
  Delimiter,
  Ordinary,
}

#[must_use]
fn token_delimiter(byte: u8) -> core::result::Result<TokenDelimiter, console_core_never::Never> {
  Ok(match byte {
    b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%' => {
      TokenDelimiter::Delimiter
    }
    _ => TokenDelimiter::Ordinary,
  })
}

enum TokenBoundary {
  Separator,
  Content,
}

#[must_use]
fn token_boundary(byte: u8) -> Result<TokenBoundary, Never> {
  let Ok(delimiter) = token_delimiter(byte);

  Ok(match delimiter {
    TokenDelimiter::Delimiter => TokenBoundary::Separator,
    TokenDelimiter::Ordinary => {
      let Ok(spacing) = byte_spacing(byte);

      match spacing {
        ByteSpacing::Whitespace => TokenBoundary::Separator,
        ByteSpacing::NonWhitespace => TokenBoundary::Content,
      }
    }
  })
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Token<'a> {
  Int(i64),
  LitString(&'a [u8]),
  HexString(&'a [u8]),
  Proc(&'a [u8]),
  Binary(&'a [u8]),
  Name(&'a [u8]),
  Raw(&'a [u8]),
}

#[derive(Default)]
struct Subrs {
  data: Vec<u8>,
  index: Vec<(u32, Range<u64>)>,
  is_dense: bool,
}

impl Subrs {
  #[must_use]
  fn get(&self, index: u32) -> core::result::Result<Option<&[u8]>, console_core_never::Never> {
    let entry_idx = match ::core::primitive::bool::then_some(self.is_dense, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(index);
        migrated_value
      }
      ::core::option::Option::None => {
        let ::core::result::Result::Ok(binary_search_by_key) =
          ::console_core_number_conversion::fixed_index(
            match match self.index.binary_search_by_key(&index, |e| e.0) {
              ::core::result::Result::Ok(value___) => ::core::option::Option::Some(value___),
              ::core::result::Result::Err(_insertion_position) => ::core::option::Option::None,
            } {
              Some(explicit_value) => explicit_value,
              None => return Ok(None),
            },
          );
        let Ok(value_41) = console_core_number_conversion::fitted::<_, u64>(
          match ::console_core_number_conversion::index(binary_search_by_key) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        );

        value_41
      }
    };
    let Ok(value_68) = console_core_number_conversion::index(entry_idx);

    let own_index = match self.index.get(value_68) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(value_77) = console_core_number_conversion::indices(own_index.1.clone());

    Ok(self.data.get(value_77))
  }
}

struct CharstringEntry {
  name: Range<u64>,
  data: Range<u64>,
}

#[derive(Default)]
struct Charstrings {
  data: Vec<u8>,
  names: Vec<u8>,
  index: Vec<CharstringEntry>,
  orig_notdef_index: Option<u64>,
}

impl Charstrings {
  #[must_use]
  fn num_glyphs(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u32>(self.index.len());
      migrated_value
    })
  }

  #[must_use]
  fn get(&self, index: u32) -> core::result::Result<Option<&[u8]>, console_core_never::Never> {
    let Ok(value_42) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(index);
      migrated_value
    });

    let own_index = match self.index.get(value_42) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(value_75) = console_core_number_conversion::indices(own_index.data.clone());

    Ok(self.data.get(value_75))
  }

  #[must_use]
  fn name(&self, index: u32) -> core::result::Result<Option<&str>, console_core_never::Never> {
    let Ok(value_43) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(index);
      migrated_value
    });

    let own_index = match self.index.get(value_43) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(value_76) = console_core_number_conversion::indices(own_index.name.clone());

    let name = match self.names.get(value_76) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok(match core::str::from_utf8(name) {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_invalid_utf8) => ::core::option::Option::None,
    })
  }

  #[must_use]
  fn index_for_name(
    &self,
    name: &str,
  ) -> core::result::Result<Option<u32>, console_core_never::Never> {
    let name_2 = name.as_bytes();

    for (idx, entry) in self.index.iter().enumerate() {
      let Ok(value_44) = console_core_number_conversion::indices(entry.name.clone());

      match ::core::primitive::bool::then_some(self.names.get(value_44) == Some(name_2), ()) {
        ::core::option::Option::Some(()) => {
          return Ok(Some({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(idx);
            migrated_value
          }));
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(None)
  }

  fn push(
    &mut self,
    name: &[u8],
    data: &[u8],
    len_iv: i64,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(value_45) = console_core_number_conversion::fitted::<_, u64>(self.data.len());

      let start = value_45;

      match ::core::primitive::bool::then_some(len_iv >= 0, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(value_46) = console_core_number_conversion::index({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(len_iv);
            migrated_value
          });

          self.data.extend(
            ({
              let Ok(explicit_value) = decrypt(data.iter().copied(), CHARSTRING_SEED);
              explicit_value
            })
            .skip(value_46),
          );
        }
        ::core::option::Option::None => {
          self.data.extend_from_slice(data);
        }
      }

      let Ok(value_47) = console_core_number_conversion::fitted::<_, u64>(self.data.len());

      let end = value_47;
      let Ok(value_48) = console_core_number_conversion::fitted::<_, u64>(self.names.len());

      let name_start = value_48;
      self.names.extend_from_slice(name);
      let Ok(value_49) = console_core_number_conversion::fitted::<_, u64>(self.names.len());

      let name_end = value_49;
      self.index.push(CharstringEntry {
        name: name_start..name_end,
        data: start..end,
      });
    };

    Ok(())
  }
}

#[derive(PartialEq, Debug)]
enum RawEncoding {
  Predefined(PredefinedEncoding),
  Custom(Vec<GlyphId>),
}

const NOTDEF_GLYPH: &[u8] = &[0x8B, 0xF7, 0xE1, 0x0D, 0x0E];

enum ProcedureItem {
  Nested,
  Content,
  Truncated,
}

#[derive(Clone)]
struct Parser<'a> {
  data: &'a [u8],
  pos: u64,
}

enum Lexeme<'a> {
  Token(Token<'a>),
  ProcedureStart(u64),
}

impl<'a> Parser<'a> {
  #[must_use]
  fn new(data: &'a [u8]) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { data, pos: 0 })
  }

  fn unparsed_bytes(&self) -> Result<core::ops::Range<u64>, Never> {
    let Ok(length) = fitted::<_, u64>(self.data.len());
    Ok(self.pos..length)
  }

  fn next(&mut self) -> Result<Option<Token<'a>>, Never> {
    let Ok(lexeme) = self.scan();

    match lexeme {
      Some(Lexeme::Token(Token::Int(value))) => self.read_integer_or_data(value),
      Some(Lexeme::Token(token)) => Ok(Some(token)),
      Some(Lexeme::ProcedureStart(start)) => self.read_proc(start),
      None => Ok(None),
    }
  }

  fn scan(&mut self) -> Result<Option<Lexeme<'a>>, Never> {
    let Ok(remaining) = self.unparsed_bytes();

    for _byte in remaining {
      let Ok(whitespace) = self.skip_whitespace();

      match whitespace {
        Some(()) => {}
        None => return Ok(None),
      }

      let start = self.pos;
      let Ok(first_byte) = self.next_byte();
      let byte = match first_byte {
        Some(byte) => byte,
        None => return Ok(None),
      };

      let token = match byte {
        b'%' => {
          let Ok(()) = self.skip_line();

          continue;
        }
        b'{' => return Ok(Some(Lexeme::ProcedureStart(start))),
        b'(' => self.read_lit_string(start),
        b'<' => {
          let Ok(following) = self.peek_byte();

          match following {
            Some(b'<') => {
              self.pos = self.pos.wrapping_add(1);

              continue;
            }
            Some(_) | None => self.read_hex_string(start),
          }
        }
        b'>' => {
          let Ok(following) = self.next_byte();

          match following {
            Some(b'>') => continue,
            Some(_) | None => return Ok(None),
          }
        }
        b'/' => self.read_name(start),
        b'[' | b']' => {
          let Ok(range) = indices(start..start.wrapping_add(1));

          Ok(self.data.get(range).map(Token::Raw))
        }
        word_start => self.read_word(start, word_start),
      };
      let Ok(token_2) = token;

      return Ok(token_2.map(Lexeme::Token));
    }

    Ok(None)
  }

  fn read_name(&mut self, start: u64) -> Result<Option<Token<'a>>, Never> {
    let Ok(following) = self.peek_byte();
    let byte = match following {
      Some(byte) => byte,
      None => return Ok(None),
    };
    let Ok(boundary) = token_boundary(byte);

    match boundary {
      TokenBoundary::Separator => {
        let Ok(delimiter) = token_delimiter(byte);

        match delimiter {
          TokenDelimiter::Ordinary => self.pos = self.pos.wrapping_add(1),
          TokenDelimiter::Delimiter => {}
        }

        Ok(Some(Token::Name(&[])))
      }
      TokenBoundary::Content => {
        let Ok(count) = self.skip_token_content();
        let Ok(range) = indices(start.wrapping_add(1)..start.wrapping_add(count));

        Ok(self.data.get(range).map(Token::Name))
      }
    }
  }

  fn read_word(&mut self, start: u64, first: u8) -> Result<Option<Token<'a>>, Never> {
    let Ok(count) = self.skip_token_content();
    let Ok(range) = indices(start..start.wrapping_add(count));
    let bytes = match self.data.get(range) {
      Some(bytes) => bytes,
      None => return Ok(None),
    };

    match ::core::primitive::bool::then_some(
      (first.is_ascii_digit() || first == b'-') && !bytes.contains(&b'.'),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(number) = decode_int(bytes);

        Ok(match number {
          Some(number_2) => Some(Token::Int(number_2)),
          None => Some(Token::Raw(bytes)),
        })
      }
      ::core::option::Option::None => Ok(Some(Token::Raw(bytes))),
    }
  }

  fn read_integer_or_data(&mut self, integer: i64) -> Result<Option<Token<'a>>, Never> {
    let mut lookahead = self.clone();
    let Ok(following) = lookahead.scan();

    match following {
      Some(Lexeme::Token(Token::Raw(b"RD" | b"-|"))) => {
        self.pos = lookahead.pos.wrapping_add(1);
        let Ok(length) = wrapped::<_, u64>(integer);
        let Ok(bytes) = self.read_bytes(length);

        Ok(bytes.map(Token::Binary))
      }
      Some(_) | None => Ok(Some(Token::Int(integer))),
    }
  }

  #[must_use]
  fn peek(&self) -> core::result::Result<Option<Token<'a>>, console_core_never::Never> {
    let Ok(next) = self.clone().next();

    Ok(next)
  }

  fn accept(&mut self, token: Token) -> Result<crate::Presence, Never> {
    let Ok(peek) = self.peek();

    match crate::Presence::from(peek == Some(token)) {
      crate::Presence::Present => {
        let Ok(_consumed_token) = self.next();
        Ok(crate::Presence::Present)
      }
      crate::Presence::Absent => Ok(crate::Presence::Absent),
    }
  }

  fn require_token(
    &mut self,
    token: Token,
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    let Ok(own_next) = self.next();
    let next = match own_next {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok((next == token).then_some(()))
  }

  fn next_byte(&mut self) -> core::result::Result<Option<u8>, console_core_never::Never> {
    let Ok(peek_byte) = self.peek_byte();
    let byte = match peek_byte {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    match (1, &mut self.pos) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    Ok(Some(byte))
  }

  #[must_use]
  fn peek_byte(&self) -> core::result::Result<Option<u8>, console_core_never::Never> {
    let Ok(value_53) = console_core_number_conversion::index(self.pos);

    Ok(self.data.get(value_53).copied())
  }

  fn read_bytes(
    &mut self,
    len: u64,
  ) -> core::result::Result<Option<&'a [u8]>, console_core_never::Never> {
    let end = match self.pos.checked_add(len) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(value_54) = console_core_number_conversion::indices(self.pos..end);

    let content = match self.data.get(value_54) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    self.pos = end;

    Ok(Some(content))
  }

  fn skip_whitespace(&mut self) -> Result<Option<()>, Never> {
    let Ok(index) = console_core_number_conversion::index(self.pos);
    let remaining = match self.data.get(index..) {
      Some(bytes) => bytes,
      None => &[],
    };

    for &byte in remaining {
      match byte_spacing(byte) {
        Ok(ByteSpacing::Whitespace) => self.pos = self.pos.wrapping_add(1),
        Ok(ByteSpacing::NonWhitespace) => return Ok(Some(())),
      }
    }

    Ok(None)
  }

  fn skip_line(&mut self) -> Result<(), Never> {
    let Ok(remaining) = self.unparsed_bytes();

    for _ in remaining {
      match self.next_byte() {
        Ok(Some(b'\n' | b'\r')) | Ok(None) => break,
        Ok(Some(_line_byte)) => {}
      }
    }

    Ok(())
  }

  fn skip_token_content(&mut self) -> Result<u64, Never> {
    let Ok(index) = console_core_number_conversion::index(self.pos);
    let remaining = match self.data.get(index..) {
      Some(bytes) => bytes,
      None => &[],
    };
    let ::core::result::Result::Ok(count) = ::console_core_number_conversion::fixed_index(
      remaining
        .iter()
        .take_while(|&&byte| matches!(token_boundary(byte), Ok(TokenBoundary::Content)))
        .count(),
    );
    let Ok(length) = fitted::<_, u64>(match ::console_core_number_conversion::index(count) {
      ::core::result::Result::Ok(native_position) => native_position,
    });
    self.pos = self.pos.wrapping_add(length);
    Ok(length.wrapping_add(1))
  }

  fn read_proc(&mut self, start: u64) -> Result<Option<Token<'a>>, Never> {
    let Ok(remaining) = self.unparsed_bytes();
    let mut depth = 1u64;

    for _byte in remaining {
      let Ok(byte) = self.next_byte();

      match byte {
        Some(b'}') => {
          depth = depth.wrapping_sub(1);

          match depth {
            0 => {
              let Ok(content) = indices(start.wrapping_add(1)..self.pos.wrapping_sub(1));

              return Ok(self.data.get(content).map(Token::Proc));
            }
            _nested_procedure => {}
          }
        }
        Some(_procedure_content) => match self.read_procedure_item() {
          Ok(ProcedureItem::Nested) => {
            depth = depth.wrapping_add(1);
            continue;
          }
          Ok(ProcedureItem::Content) => {}
          Ok(ProcedureItem::Truncated) => return Ok(None),
        },
        None => return Ok(None),
      }

      let Ok(_next_token) = self.skip_whitespace();
    }

    Ok(None)
  }

  fn read_procedure_item(&mut self) -> Result<ProcedureItem, Never> {
    let Ok(lexeme) = self.scan();
    Ok(match lexeme {
      Some(Lexeme::ProcedureStart(_nested_start)) => ProcedureItem::Nested,
      Some(Lexeme::Token(Token::Int(integer))) => {
        let Ok(token) = self.read_integer_or_data(integer);

        match token {
          Some(_integer_or_data) => ProcedureItem::Content,
          None => ProcedureItem::Truncated,
        }
      }
      Some(Lexeme::Token(_content)) => ProcedureItem::Content,
      None => ProcedureItem::Truncated,
    })
  }

  fn read_lit_string(
    &mut self,
    start: u64,
  ) -> core::result::Result<Option<Token<'a>>, console_core_never::Never> {
    let mut nest_depth = 1u64;
    let Ok(remaining_input) = self.unparsed_bytes();

    for _ in remaining_input {
      let c = match self.next_byte() {
        Ok(Some(byte)) => byte,
        Ok(None) => break,
      };

      match c {
        b'(' => nest_depth = nest_depth.wrapping_add(1),
        b')' => {
          nest_depth = nest_depth.wrapping_sub(1);

          match ::core::primitive::bool::then_some(nest_depth == 0, ()) {
            ::core::option::Option::Some(()) => {
              break;
            }
            ::core::option::Option::None => {}
          }
        }

        b'\\' => {
          let Ok(byte) = self.next_byte();
          (match byte {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          });
        }
        _ => {}
      }
    }

    match ::core::primitive::bool::then_some(nest_depth != 0, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let end = self.pos;

    match (1, &mut self.pos) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    let Ok(value_58) = console_core_number_conversion::indices(
      ::core::primitive::u64::wrapping_add(start, 1)
        ..::core::primitive::u64::wrapping_sub(end, 1),
    );

    let data = match self.data.get(value_58) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok(Some(Token::LitString(data)))
  }

  fn read_hex_string(
    &mut self,
    start: u64,
  ) -> core::result::Result<Option<Token<'a>>, console_core_never::Never> {
    let Ok(remaining_input) = self.unparsed_bytes();

    for _ in remaining_input {
      let c = match self.next_byte() {
        Ok(Some(byte)) => byte,
        Ok(None) => break,
      };

      match {
        let Ok(explicit_value) = byte_spacing(c);
        explicit_value
      } {
        ByteSpacing::Whitespace => {}
        ByteSpacing::NonWhitespace => match c {
          b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F' => {}
          _ => break,
        },
      }
    }

    let end = self.pos;
    let Ok(value_59) =
      console_core_number_conversion::index(::core::primitive::u64::wrapping_sub(end, 1));

    match ::core::primitive::bool::then_some(self.data.get(value_59) != Some(&b'>'), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let Ok(value_60) = console_core_number_conversion::indices(
      ::core::primitive::u64::wrapping_add(start, 1)
        ..::core::primitive::u64::wrapping_sub(end, 1),
    );

    let data = match self.data.get(value_60) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok(Some(Token::HexString(data)))
  }

  fn read_int(&mut self) -> core::result::Result<Option<i64>, console_core_never::Never> {
    let Ok(next) = self.next();

    Ok(next.and_then(|t| match t {
      Token::Int(n) => Some(n),
      Token::LitString(_)
      | Token::HexString(_)
      | Token::Proc(_)
      | Token::Binary(_)
      | Token::Name(_)
      | Token::Raw(_) => None,
    }))
  }

  fn read_num_as_int(&mut self) -> core::result::Result<Option<i32>, console_core_never::Never> {
    let Ok(own_next) = self.next();
    let next = match own_next {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok(match next {
      Token::Int(n) => Some({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(n);
        migrated_value
      }),

      Token::Raw(bytes) => {
        let Ok(int_prefix) = decode_int_prefix(bytes, 0);

        int_prefix.map(|n| {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(n.0);
          migrated_value
        })
      }
      Token::LitString(_)
      | Token::HexString(_)
      | Token::Proc(_)
      | Token::Binary(_)
      | Token::Name(_) => None,
    })
  }

  fn read_string(&mut self) -> core::result::Result<Option<String>, console_core_never::Never> {
    use alloc::borrow::ToOwned;
    let Ok(own_next) = self.next();
    let next = match own_next {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let bytes = match next {
      Token::Name(bytes) | Token::LitString(bytes) => bytes,
      Token::Int(_) | Token::HexString(_) | Token::Proc(_) | Token::Binary(_) | Token::Raw(_) => {
        return Ok(None);
      }
    };

    Ok(
      match core::str::from_utf8(bytes) {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_invalid_utf8) => ::core::option::Option::None,
      }
      .map(|s| s.to_owned()),
    )
  }
}

impl Parser<'_> {
  fn read_font_matrix(
    &mut self,
  ) -> core::result::Result<Option<ScaledFontMatrix>, console_core_never::Never> {
    let mut components = [Fixed::ZERO; 6];

    match self.accept(Token::Raw(b"[")) {
      Ok(crate::Presence::Absent) => {
        let Ok(require_token) = self.require_token(Token::Raw(b"{"));
        (match require_token {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        });
      }
      Ok(crate::Presence::Present) => {}
    }

    for component in &mut components {
      let Ok(own_next) = self.next();
      let next = match own_next {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      };

      *component = match next {
        Token::Int(int) => {
          let checked_value = match ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(int);
            migrated_value
          })
          .checked_mul(1000)
          {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          };
          let Ok(value_30) = Fixed::from_i32(checked_value);

          value_30
        }
        Token::Raw(bytes) => {
          let Ok(fixed) = decode_fixed(bytes, 3);

          match fixed {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          }
        }
        Token::LitString(_)
        | Token::HexString(_)
        | Token::Proc(_)
        | Token::Binary(_)
        | Token::Name(_) => return Ok(None),
      }
    }

    let Ok(own_next) = self.next();
    (match own_next {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    });
    let Ok(temp_scale) = (*match &components {
      [_, _, _, element_2, ..] => element_2,
    })
    .abs();

    match ::core::primitive::bool::then_some(temp_scale == Fixed::ZERO, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let mut upem = 1000;

    match ::core::primitive::bool::then_some(temp_scale != Fixed::ONE, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_8) = Fixed::from_bits(1000);

        let Ok(value_22) = (value_8 / temp_scale).to_bits();

        upem = value_22;
        let [xx, xy, yx, yy, dx, dy] = &mut components;

        for component in [xx, xy, yx, dx, dy] {
          *component /= temp_scale;
        }

        *yy = match (*yy).cmp(&Fixed::ZERO) {
          core::cmp::Ordering::Less => -Fixed::ONE,
          core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => Fixed::ONE,
        };
      }
      ::core::option::Option::None => {}
    }

    for offset in components.iter_mut().skip(4) {
      let Ok(value_14) = offset.to_bits();

      let Ok(value_23) = Fixed::from_bits(::core::primitive::i32::wrapping_shr(value_14, 16_u32));

      *offset = value_23;
    }

    let Ok(matrix) = FontMatrix::from_elements(components);
    let value_15 = &matrix;

    match ::core::primitive::bool::then_some(
      matches!(
        transform::matrix_condition(value_15),
        Ok(transform::MatrixCondition::Degenerate)
      ),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let value_16 = matrix;

    Ok(Some(ScaledFontMatrix {
      matrix: value_16,
      scale: upem,
    }))
  }

  fn read_subrs(
    &mut self,
    len_iv: i64,
  ) -> core::result::Result<Option<Subrs>, console_core_never::Never> {
    let mut subrs = Subrs::default();
    let next = match {
      let Ok(explicit_value) = self.next();
      explicit_value
    } {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let _: u64 = match next {
      Token::Raw(b"[") => {
        let Ok(require_token) = self.require_token(Token::Raw(b"]"));
        (match require_token {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        });
        return Ok(Some(subrs));
      }
      Token::Int(n) => match match n.try_into() {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
      } {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      },
      Token::Raw(_)
      | Token::LitString(_)
      | Token::HexString(_)
      | Token::Proc(_)
      | Token::Binary(_)
      | Token::Name(_) => return Ok(None),
    };
    let Ok(require_token) = self.require_token(Token::Raw(b"array"));
    (match require_token {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    });
    let mut is_dense = true;

    let Ok(remaining_input) = self.unparsed_bytes();

    for _ in remaining_input {
      match self.accept(Token::Raw(b"dup")) {
        Ok(crate::Presence::Present) => {}
        Ok(crate::Presence::Absent) => break,
      }

      let own_next = match {
        let Ok(explicit_value) = self.next();
        explicit_value
      } {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      };

      let (n, data) = {
        let outcome_n_2 = (
          own_next,
          (match {
            let Ok(explicit_value) = self.next();
            explicit_value
          } {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          }),
        );

        match outcome_n_2 {
          (Token::Int(matched_n_2), Token::Binary(matched_data_2)) => {
            (matched_n_2, matched_data_2)
          }
          (crate::ps::type1::Token::LitString(..), _)
          | (crate::ps::type1::Token::HexString(..), _)
          | (crate::ps::type1::Token::Proc(..), _)
          | (crate::ps::type1::Token::Binary(..), _)
          | (crate::ps::type1::Token::Name(..), _)
          | (crate::ps::type1::Token::Raw(..), _)
          | (_, crate::ps::type1::Token::Int(..))
          | (_, crate::ps::type1::Token::LitString(..))
          | (_, crate::ps::type1::Token::HexString(..))
          | (_, crate::ps::type1::Token::Proc(..))
          | (_, crate::ps::type1::Token::Name(..))
          | (_, crate::ps::type1::Token::Raw(..)) => return Ok(None),
        }
      };

      let Ok(_consumed_token) = self.next();

      let Ok(_optional_put) = self.accept(Token::Raw(b"put"));
      let subr_num: u32 = match match n.try_into() {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
      } {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      };
      let Ok(value_62) = console_core_number_conversion::fitted::<_, u64>(subrs.index.len());

      match ::core::primitive::bool::then_some(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(subr_num);
          migrated_value
        }) != value_62,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          is_dense = false;
        }
        ::core::option::Option::None => {}
      }

      let Ok(value_63) = console_core_number_conversion::fitted::<_, u64>(subrs.data.len());

      let start = value_63;

      match ::core::primitive::bool::then_some(len_iv >= 0, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(value_64) = console_core_number_conversion::index({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(len_iv);
            migrated_value
          });

          subrs.data.extend(
            ({
              let Ok(explicit_value) = decrypt(data.iter().copied(), CHARSTRING_SEED);
              explicit_value
            })
            .skip(value_64),
          );
        }
        ::core::option::Option::None => {
          subrs.data.extend_from_slice(data);
        }
      }

      let Ok(value_65) = console_core_number_conversion::fitted::<_, u64>(subrs.data.len());

      let end = value_65;
      subrs.index.push((subr_num, start..end));
    }

    match ::core::primitive::bool::then_some(!is_dense, ()) {
      ::core::option::Option::Some(()) => {
        subrs.index.sort_unstable_by_key(|(n, ..)| *n);
      }
      ::core::option::Option::None => {}
    }

    subrs.is_dense = is_dense;
    subrs.data.shrink_to_fit();
    subrs.index.shrink_to_fit();

    Ok(Some(subrs))
  }

  fn read_charstrings(
    &mut self,
    len_iv: i64,
  ) -> core::result::Result<Option<Charstrings>, console_core_never::Never> {
    let mut charstrings = Charstrings::default();
    let Ok(own_next) = self.next();
    let next = match own_next {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let _: u64 = match next {
      Token::Int(n) => match match n.try_into() {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
      } {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      },
      Token::LitString(_)
      | Token::HexString(_)
      | Token::Proc(_)
      | Token::Binary(_)
      | Token::Name(_)
      | Token::Raw(_) => return Ok(None),
    };
    let mut notdef_idx = None;

    let Ok(remaining_input) = self.unparsed_bytes();

    for _ in remaining_input {
      let token = match self.next() {
        Ok(Some(token)) => token,
        Ok(None) => break,
      };
      let name = match token {
        Token::Raw(b"end") => {
          let Ok(peek_byte) = self.peek_byte();

          match peek_byte.map(token_boundary) {
            Some(Ok(TokenBoundary::Separator)) => break,
            Some(Ok(TokenBoundary::Content)) | None => continue,
          }
        }
        Token::Raw(b"def") => {
          let Ok(peek_byte) = self.peek_byte();

          match peek_byte.map(token_boundary) {
            Some(Ok(TokenBoundary::Separator)) => match charstrings.index.first() {
              Some(_) => break,
              None => continue,
            },
            Some(Ok(TokenBoundary::Content)) | None => continue,
          }
        }
        Token::Name(name) => name,
        Token::Raw(_)
        | Token::Int(_)
        | Token::LitString(_)
        | Token::HexString(_)
        | Token::Proc(_)
        | Token::Binary(_) => continue,
      };

      match ::core::primitive::bool::then_some(name == b".notdef", ()) {
        ::core::option::Option::Some(()) => {
          let Ok(index) =
            console_core_number_conversion::fitted::<_, u64>(charstrings.index.len());
          notdef_idx = Some(index);
        }
        ::core::option::Option::None => {}
      }

      let data = {
        let outcome_data = match {
          let Ok(explicit_value) = self.next();
          explicit_value
        } {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        };

        match outcome_data {
          Token::Binary(matched_data) => matched_data,
          crate::ps::type1::Token::Int(..)
          | crate::ps::type1::Token::LitString(..)
          | crate::ps::type1::Token::HexString(..)
          | crate::ps::type1::Token::Proc(..)
          | crate::ps::type1::Token::Name(..)
          | crate::ps::type1::Token::Raw(..) => return Ok(None),
        }
      };

      let Ok(()) = charstrings.push(name, data, len_iv);
    }

    let moved = match notdef_idx {
      Some(0) => None,
      Some(index) => Some(index),
      None => {
        let Ok(index) = console_core_number_conversion::fitted::<_, u64>(charstrings.index.len());
        let Ok(()) = charstrings.push(b".notdef", NOTDEF_GLYPH, -1);

        Some(index)
      }
    };

    match moved {
      Some(0) => charstrings.orig_notdef_index = Some(0),
      Some(index) => {
        let Ok(at) = console_core_number_conversion::index(index);
        let [first, notdef] = match charstrings.index.get_disjoint_mut([0, at]) {
          Ok(pair) => pair,
          Err(_invalid_charstring_pair) => return Ok(None),
        };
        core::mem::swap(first, notdef);
        charstrings.orig_notdef_index = Some(index);
      }
      None => {}
    }

    charstrings.data.shrink_to_fit();
    charstrings.names.shrink_to_fit();
    charstrings.index.shrink_to_fit();

    Ok(Some(charstrings))
  }

  fn read_font_bbox(
    &mut self,
  ) -> core::result::Result<Option<[Fixed; 4]>, console_core_never::Never> {
    let mut bbox = [Fixed::ZERO; 4];

    let mut parser;
    let parser_2 = match self.accept(Token::Raw(b"[")) {
      Ok(crate::Presence::Present) => self,
      Ok(crate::Presence::Absent) => {
        let Ok(own_next) = self.next();
        let next = match own_next {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        };

        match next {
          Token::Proc(procedure) => {
            parser = {
              let Ok(explicit_value) = Parser::new(procedure);
              explicit_value
            };

            &mut parser
          }
          Token::Int(_)
          | Token::LitString(_)
          | Token::HexString(_)
          | Token::Binary(_)
          | Token::Name(_)
          | Token::Raw(_) => return Ok(None),
        }
      }
    };

    for component in &mut bbox {
      let Ok(parser_2_next) = parser_2.next();
      let next = match parser_2_next {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      };

      *component = match next {
        Token::Int(int) => {
          let Ok(value_87) = Fixed::from_i32({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(int);
            migrated_value
          });

          value_87
        }
        Token::Raw(bytes) => {
          let Ok(fixed) = decode_fixed(bytes, 0);

          match fixed {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          }
        }
        Token::LitString(_)
        | Token::HexString(_)
        | Token::Proc(_)
        | Token::Binary(_)
        | Token::Name(_) => return Ok(None),
      }
    }

    Ok(Some(bbox))
  }

  fn read_weight_vector(
    &mut self,
  ) -> core::result::Result<Option<Vec<Fixed>>, console_core_never::Never> {
    let Ok(_optional_open_bracket) = self.accept(Token::Raw(b"["));
    let mut weights = Vec::new();

    let Ok(remaining_input) = self.unparsed_bytes();

    for _ in remaining_input {
      let token = match self.next() {
        Ok(Some(token)) => token,
        Ok(None) => break,
      };

      match token {
        Token::Raw(b"]") => break,
        Token::Int(val) => {
          let Ok(value_27) = Fixed::from_i32({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(val);
            migrated_value
          });

          weights.push(value_27)
        }
        Token::Raw(raw) => {
          let fixed = match {
            let Ok(explicit_value) = decode_fixed(raw, 0);
            explicit_value
          } {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          };

          weights.push(fixed)
        }
        Token::LitString(_)
        | Token::HexString(_)
        | Token::Proc(_)
        | Token::Binary(_)
        | Token::Name(_) => return Ok(None),
      }
    }

    Ok(Some(weights))
  }

  fn read_encoding(&mut self, charstrings: &Charstrings) -> Result<Option<RawEncoding>, Never> {
    let Ok(token) = self.next();
    let (code_count, entries) = match token {
      Some(Token::Raw(b"[")) => {
        let Ok(entries) = self.dense_encoding();
        (256, entries)
      }
      Some(Token::Int(count)) => {
        let Ok(entries) = self.sparse_encoding();
        (count.clamp(0, 256), entries)
      }
      Some(Token::Raw(b"StandardEncoding")) => {
        return Ok(Some(RawEncoding::Predefined(PredefinedEncoding::Standard)));
      }
      Some(Token::Raw(b"ExpertEncoding")) => {
        return Ok(Some(RawEncoding::Predefined(PredefinedEncoding::Expert)));
      }
      Some(Token::Raw(b"ISOLatin1Encoding")) => {
        return Ok(Some(RawEncoding::Predefined(PredefinedEncoding::IsoLatin1)));
      }
      Some(
        Token::Raw(_)
        | Token::Name(_)
        | Token::LitString(_)
        | Token::HexString(_)
        | Token::Proc(_)
        | Token::Binary(_),
      )
      | None => return Ok(None),
    };

    let Ok(capacity) =
      console_core_number_conversion::index(code_count.clamp(0, 256).unsigned_abs());
    let mut map = alloc::vec![GlyphId::NOTDEF; capacity.min(256)];

    for (code, glyph_name) in entries {
      let code_index = match u64::try_from(code) {
        Ok(index) => index,
        Err(_negative_code) => continue,
      };
      let Ok(position) = console_core_number_conversion::index(code_index);
      let slot = match map.get_mut(position) {
        Some(slot) => slot,
        None => continue,
      };

      match charstrings.index_for_name(glyph_name) {
        Ok(Some(glyph_id)) => *slot = GlyphId::from(glyph_id),
        Ok(None) => {}
      }
    }

    Ok(Some(RawEncoding::Custom(map)))
  }
}

impl<'data> Parser<'data> {
  fn dense_encoding(&mut self) -> Result<EncodingEntries<'_, 'data>, Never> {
    let Ok(_optional_open_bracket) = self.accept(Token::Raw(b"["));
    let Ok(remaining) = self.unparsed_bytes();
    Ok(EncodingEntries {
      parser: self,
      remaining,
      mode: EncodingMode::Dense { next_code: 0 },
    })
  }

  fn sparse_encoding(&mut self) -> Result<EncodingEntries<'_, 'data>, Never> {
    let Ok(remaining) = self.unparsed_bytes();
    Ok(EncodingEntries {
      parser: self,
      remaining,
      mode: EncodingMode::Sparse,
    })
  }
}

enum EncodingMode {
  Dense { next_code: i64 },
  Sparse,
}

enum EncodingRecord<'a> {
  Named { code: i64, bytes: &'a [u8] },
  Ignored,
  End,
}

impl EncodingMode {
  fn read_record<'data>(
    &mut self,
    parser: &mut Parser<'data>,
  ) -> Result<EncodingRecord<'data>, Never> {
    let Ok(token) = parser.next();
    Ok(match (self, token) {
      (Self::Dense { next_code }, Some(Token::Name(bytes))) => {
        let code = *next_code;
        *next_code = next_code.wrapping_add(1);
        EncodingRecord::Named { code, bytes }
      }
      (Self::Sparse, Some(Token::Int(code))) => match parser.next() {
        Ok(Some(Token::Name(bytes))) => EncodingRecord::Named { code, bytes },
        Ok(
          Some(
            Token::Raw(_)
            | Token::Int(_)
            | Token::LitString(_)
            | Token::HexString(_)
            | Token::Proc(_)
            | Token::Binary(_),
          )
          | None,
        ) => EncodingRecord::Ignored,
      },
      (Self::Sparse, Some(Token::Raw(b"def"))) | (_, None) => EncodingRecord::End,
      (
        Self::Sparse,
        Some(
          Token::Raw(_)
          | Token::Name(_)
          | Token::LitString(_)
          | Token::HexString(_)
          | Token::Proc(_)
          | Token::Binary(_),
        ),
      ) => EncodingRecord::Ignored,
      (
        Self::Dense { .. },
        Some(
          Token::Raw(_)
          | Token::Int(_)
          | Token::LitString(_)
          | Token::HexString(_)
          | Token::Proc(_)
          | Token::Binary(_),
        ),
      ) => EncodingRecord::End,
    })
  }
}

struct EncodingEntries<'parser, 'data> {
  parser: &'parser mut Parser<'data>,
  remaining: core::ops::Range<u64>,
  mode: EncodingMode,
}

impl<'data> Iterator for EncodingEntries<'_, 'data> {
  type Item = (i64, &'data str);

  fn next(&mut self) -> Option<Self::Item> {
    for _byte in self.remaining.by_ref() {
      match self.mode.read_record(self.parser) {
        Ok(EncodingRecord::Named { code, bytes }) => match core::str::from_utf8(bytes) {
          Ok(name) => return Some((code, name)),
          Err(_invalid_glyph_name) => continue,
        },
        Ok(EncodingRecord::Ignored) => {}
        Ok(EncodingRecord::End) => {
          self.remaining = 0..0;
          return None;
        }
      }
    }

    None
  }
}

#[must_use]
fn decode_int(bytes: &[u8]) -> Result<Option<i64>, Never> {
  let source = match core::str::from_utf8(bytes) {
    Ok(source) => source,
    Err(_invalid_utf8) => return Ok(None),
  };
  let parsed = match source.split_once('#') {
    Some((prefix, digits)) => {
      let base = match prefix.len() {
        1 | 2 => match prefix.parse::<u32>() {
          Ok(base @ 2..=36) => base,
          Ok(_unsupported_base) => return Ok(None),
          Err(_malformed_base) => return Ok(None),
        },
        _unsupported_prefix_length => return Ok(None),
      };
      i64::from_str_radix(digits, base)
    }
    None => source.parse::<i64>(),
  };
  Ok(match parsed {
    Ok(number) => Some(number),
    Err(_invalid_integer) => None,
  })
}

#[must_use]
fn decode_int_prefix(
  bytes: &[u8],
  start: u64,
) -> core::result::Result<Option<(i64, u64)>, console_core_never::Never> {
  let Ok(value_72) = console_core_number_conversion::indices_from(start..);

  let tail = match bytes.get(value_72) {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let ::core::result::Result::Ok(end_index) = ::console_core_number_conversion::fixed_index(
    match tail.iter().position(|c| *c != b'-' && !c.is_ascii_digit()) {
      Some(delimiter) => delimiter,
      None => tail.len(),
    },
  );
  let Ok(end) = console_core_number_conversion::fitted::<_, u64>(
    match ::console_core_number_conversion::index(end_index) {
      ::core::result::Result::Ok(native_position) => native_position,
    },
  );
  let Ok(value_89) = console_core_number_conversion::indices_to(..end);

  let checked_value = match tail.get(value_89) {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let int = match {
    let Ok(explicit_value) = decode_int(checked_value);
    explicit_value
  } {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };

  Ok(Some((
    int,
    ::core::primitive::u64::wrapping_add(start, end),
  )))
}

#[must_use]
fn decode_fixed(
  bytes: &[u8],
  mut power_ten: i32,
) -> core::result::Result<Option<Fixed>, console_core_never::Never> {
  const LIMIT: i32 = 0xCCCCCCC;
  let Ok(value_73) = console_core_number_conversion::fitted::<_, u64>(0);

  let mut idx = value_73;
  let Ok(value_81) = console_core_number_conversion::index(idx);

  let &first = match bytes.get(value_81) {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let sign = match ::core::primitive::bool::then_some(first == b'-' || first == b'+', ()) {
    ::core::option::Option::Some(()) => {
      idx = ::core::primitive::u64::wrapping_add(idx, 1);

      match ::core::primitive::bool::then_some(first == b'-', ()) {
        ::core::option::Option::Some(()) => -1,
        ::core::option::Option::None => 1,
      }
    }
    ::core::option::Option::None => 1,
  };
  let overflow = || {
    let Ok(value_28) = Fixed::from_bits(::core::primitive::i32::wrapping_mul(0x7FFFFFFF, sign));

    Some(value_28)
  };
  let mut integral = 0;
  let Ok(value_82) = console_core_number_conversion::index(idx);

  let checked_value_2 = match bytes.get(value_82) {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };

  match ::core::primitive::bool::then_some(*checked_value_2 != b'.', ()) {
    ::core::option::Option::Some(()) => {
      let Ok(int_prefix) = decode_int_prefix(bytes, idx);
      let (int, end_idx) = match int_prefix {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      };

      match ::core::primitive::bool::then_some(int > 0x7FFF, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(overflow());
        }
        ::core::option::Option::None => {}
      }

      integral = {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
          ::core::primitive::i64::wrapping_shl(int, 16_u32),
        );
        migrated_value
      };
      let Ok(value_74) = console_core_number_conversion::index(end_idx);

      let Ok(value_83) = console_core_number_conversion::fitted::<_, u64>(value_74);

      idx = value_83;
    }
    ::core::option::Option::None => {}
  }

  let mut decimal = 0i32;
  let mut divider = 1i32;
  let Ok(decimal_marker) = console_core_number_conversion::index(idx);
  let digits = match bytes.get(decimal_marker..) {
    Some([b'.', rest @ ..]) => {
      idx = idx.wrapping_add(1);
      rest
    }
    Some(_) | None => &[],
  };

  for &byte in digits.iter().take_while(|byte| byte.is_ascii_digit()) {
    idx = idx.wrapping_add(1);

    match (divider < LIMIT && decimal < LIMIT).then_some(()) {
      Some(()) => {}
      None => continue,
    }

    let digit = i32::from(byte.wrapping_sub(b'0'));
    decimal = decimal.wrapping_mul(10).wrapping_add(digit);

    match (integral == 0 && power_ten > 0).then_some(()) {
      Some(()) => power_ten = power_ten.wrapping_sub(1),
      None => divider = divider.wrapping_mul(10),
    }
  }

  let Ok(exponent_marker) = console_core_number_conversion::index(idx);

  match bytes.get(exponent_marker) {
    Some(b'e' | b'E') => {
      let Ok(parsed_exponent) = decode_int_prefix(bytes, idx.wrapping_add(1));
      let (exponent, _end) = match parsed_exponent {
        Some(value) => value,
        None => return Ok(None),
      };
      let bounded = match exponent {
        ..=-1001 => return Ok(Some(Fixed::ZERO)),
        1001.. => return Ok(overflow()),
        _valid => exponent,
      };
      let Ok(scale) = console_core_number_conversion::wrapped::<_, i32>(bounded);
      power_ten = match power_ten.checked_add(scale) {
        Some(sum) => sum,
        None => return Ok(None),
      };
    }
    Some(_) | None => {}
  }

  match ::core::primitive::bool::then_some(integral == 0 && decimal == 0, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(Some(Fixed::ZERO));
    }
    ::core::option::Option::None => {}
  }

  for _ in (::core::ops::Range {
    start: 0,
    end: power_ten,
  })
  .rev()
  {
    match ::core::primitive::bool::then_some(integral >= LIMIT, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(overflow());
      }
      ::core::option::Option::None => {}
    }

    integral = ::core::primitive::i32::wrapping_mul(integral, 10);

    match ::core::primitive::bool::then_some(decimal >= LIMIT, ()) {
      ::core::option::Option::Some(()) => {
        match ::core::primitive::bool::then_some(divider == 1, ()) {
          ::core::option::Option::Some(()) => {
            return Ok(overflow());
          }
          ::core::option::Option::None => {}
        }

        divider = ::core::primitive::i32::wrapping_div(divider, 10);
      }
      ::core::option::Option::None => {
        decimal = ::core::primitive::i32::wrapping_mul(decimal, 10);
      }
    }

    power_ten = (::core::num::Wrapping(power_ten) - ::core::num::Wrapping(1)).0;
  }

  for _ in (::core::ops::Range {
    start: power_ten,
    end: 0,
  }) {
    integral = ::core::primitive::i32::wrapping_div(integral, 10);

    match ::core::primitive::bool::then_some(divider < LIMIT, ()) {
      ::core::option::Option::Some(()) => {
        divider = ::core::primitive::i32::wrapping_mul(divider, 10);
      }
      ::core::option::Option::None => {
        decimal = ::core::primitive::i32::wrapping_div(decimal, 10);
      }
    }

    match ::core::primitive::bool::then_some(integral == 0 && decimal == 0, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(Some(Fixed::ZERO));
      }
      ::core::option::Option::None => {}
    }

    power_ten = (::core::num::Wrapping(power_ten) + ::core::num::Wrapping(1)).0;
  }

  match ::core::primitive::bool::then_some(decimal != 0, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(value_18) = Fixed::from_bits(decimal);
      let Ok(value_19) = Fixed::from_bits(divider);

      let Ok(value_25) = (value_18 / value_19).to_bits();

      decimal = value_25;

      match (decimal, &mut integral) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }
    ::core::option::Option::None => {}
  }

  let Ok(value_20) = Fixed::from_bits(::core::primitive::i32::wrapping_mul(integral, sign));

  Ok(Some(value_20))
}

#[cfg(test)]
mod tests {
  use super::*;
  use cs::test_helpers::*;

  #[test]
  fn pfb_tags() -> ::core::result::Result<(), &'static str> {
    let data = [0x80, 0x01, 0x01, 0x02, 0x00, 0x00];
    let Ok(pfb_tag) = decode_pfb_tag(&data, 0);
    let (tag, len) = match pfb_tag {
      ::core::option::Option::Some(read_value) => read_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(::core::concat!(
          ::core::file!(),
          ":",
          ::core::line!(),
          ": expected Some, found None"
        ));
      }
    };
    assert_eq!(tag, PfbSegment::Text);
    assert_eq!(len, 513);

    let binary_segment = [0x80, 0x02, 0x01, 0x03, 0x00, 0x00];
    let (tag_2, len_2) = match {
      let Ok(explicit_value) = decode_pfb_tag(&binary_segment, 0);
      explicit_value
    } {
      ::core::option::Option::Some(read_value) => read_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(::core::concat!(
          ::core::file!(),
          ":",
          ::core::line!(),
          ": expected Some, found None"
        ));
      }
    };
    assert_eq!(tag_2, PfbSegment::Binary);
    assert_eq!(len_2, 769);

    let bad_marker = [0x00; 6];
    assert!(
      ({
        let Ok(explicit_value) = decode_pfb_tag(&bad_marker, 0);
        explicit_value
      })
      .is_none()
    );

    let short_header = [0x00; 5];
    assert!(
      ({
        let Ok(explicit_value) = decode_pfb_tag(&short_header, 0);
        explicit_value
      })
      .is_none()
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn pfb_segments() {
    let segments = [
      vec![0x01; 8],
      vec![0x02; 10],
      vec![0x03; 4],
      vec![0x04; 255],
    ];

    let mut buf = vec![];

    for segment in &segments {
      buf.push(0x80);
      buf.push(0x02);
      buf.push({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(segment.len());
        migrated_value
      });
      buf.extend_from_slice(&[0; 3]);

      for byte in segment {
        buf.push(*byte);
      }
    }

    let mut parsed_count = 0u64;

    let Ok(pfb_binary_segments) = decode_pfb_binary_segments(&buf);

    for (parsed, expected) in pfb_binary_segments.zip(&segments) {
      assert_eq!(parsed, expected);
      parsed_count = parsed_count.wrapping_add(1);
    }

    let Ok(segment_count) = console_core_number_conversion::fixed_index(segments.len());
    assert_eq!(parsed_count, segment_count);
  }

  #[test]
  fn hex_decode() {
    let Ok(()) = check_hex_decode(
      b"743F8413F3636CA85A9FFEFB50B4BB27",
      &[
        116, 63, 132, 19, 243, 99, 108, 168, 90, 159, 254, 251, 80, 180, 187, 39,
      ],
    );
  }

  #[test]
  fn hex_decode_ignores_whitespace() {
    let Ok(()) = check_hex_decode(
      b"743F 8413F3636C\nA85A9FFEF\tB50B     4BB27",
      &[
        116, 63, 132, 19, 243, 99, 108, 168, 90, 159, 254, 251, 80, 180, 187, 39,
      ],
    );
  }

  #[test]
  fn hex_decode_truncate() {
    let Ok(()) = check_hex_decode(b"743F.8413F3636CA85A9FFEFB50B4BB27", &[116, 63]);
  }

  #[test]
  fn hex_decode_odd_chars() {
    let Ok(()) = check_hex_decode(b"743", &[116, 48]);
  }

  #[track_caller]
  fn check_hex_decode(
    hex: &[u8],
    expected: &[u8],
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let decoded = ({
        let Ok(explicit_value) = decode_hex(hex);
        explicit_value
      })
      .collect::<Vec<_>>();
      assert_eq!(decoded, expected);
    };

    Ok(())
  }

  #[test]
  fn decrypt_bytes() {
    let cipher = [
      0x74, 0x3f, 0x84, 0x13, 0xf3, 0x63, 0x6c, 0xa8, 0x5a, 0x9f, 0xfe, 0xfb, 0x50, 0xb4, 0xbb,
      0x27,
    ];
    let plain = ({
      let Ok(explicit_value) = decrypt(cipher.iter().copied(), EEXEC_SEED);
      explicit_value
    })
    .collect::<Vec<_>>();

    assert_eq!(plain.get(4..), Some(b"dup\n/Private".as_slice()));
  }

  #[test]
  fn find_eexec() {
    assert_eq!(
      ({
        let Ok(explicit_value) = find_eexec_data(b"dup\n/Private\ncurrentfile eexec *&&FW");
        explicit_value
      }),
      Some(31)
    );

    assert_eq!(
      ({
        let Ok(explicit_value) = find_eexec_data(b"dup\n/Private\ncurrentfile eexec     *&&FW");
        explicit_value
      }),
      Some(35)
    );

    assert_eq!(
      ({
        let Ok(explicit_value) = find_eexec_data(b"dup\n/Private\ncurrentfile eexec\n\n*&&FW");
        explicit_value
      }),
      Some(32)
    );

    assert_eq!(
      ({
        let Ok(explicit_value) = find_eexec_data(b"dup\n/Private\ncurrentfile eexec\r\n\r*&&FW");
        explicit_value
      }),
      Some(32)
    );

    assert_eq!(
      ({
        let Ok(explicit_value) =
          find_eexec_data(b"% eexec in comment\n(eexec in string) currentfile eexec $$$$");
        explicit_value
      }),
      Some(55)
    );

    assert!(
      ({
        let Ok(explicit_value) =
          find_eexec_data(b"% eexec in comment\n(eexec in string) currentfile");
        explicit_value
      })
      .is_none()
    );
  }

  #[test]
  fn read_pfb_raw_dicts() -> ::core::result::Result<(), &'static str> {
    let Ok(raw_dicts) = RawDicts::new(font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFB);
    let dicts = match raw_dicts {
      ::core::option::Option::Some(read_value) => read_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(::core::concat!(
          ::core::file!(),
          ":",
          ::core::line!(),
          ": expected Some, found None"
        ));
      }
    };
    let Ok(()) = check_noto_serif_base(dicts.base);
    let Ok(()) = check_noto_serif_private(&dicts.private);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn read_pfa_raw_dicts() -> ::core::result::Result<(), &'static str> {
    let Ok(raw_dicts) = RawDicts::new(font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFA);
    let dicts = match raw_dicts {
      ::core::option::Option::Some(read_value) => read_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(::core::concat!(
          ::core::file!(),
          ":",
          ::core::line!(),
          ": expected Some, found None"
        ));
      }
    };
    let Ok(()) = check_noto_serif_base(dicts.base);
    let Ok(()) = check_noto_serif_private(&dicts.private);
    ::core::result::Result::Ok(())
  }

  fn check_noto_serif_base(base: &[u8]) -> core::result::Result<(), console_core_never::Never> {
    {
      const EXPECTED_PREFIX: &str = r#"%!PS-AdobeFont-1.0: NotoSerif-Regular 2.007; ttfautohint (v1.8) -l 8 -r 50 -G 200 -x 14 -D latn -f none -a qsq -X ""
%%Title: NotoSerif-Regular
%Version: 2.007; ttfautohint (v1.8) -l 8 -r 50 -G 200 -x 14 -D latn -f none -a qsq -X ""
%%CreationDate: Tue Feb 10 16:07:25 2026
%%Creator: www-data
%Copyright: Copyright 2015-2021 Google LLC. All Rights Reserved.
% Generated by FontForge 20190801 (http://fontforge.sf.net/)
%%EndComments

10 dict begin
/FontType 1 def
/FontMatrix [0.001 0 0 0.001 0 0 ]readonly def
/FontName /NotoSerif-Regular def
/FontBBox {5 0 989 775 }readonly def
"#;
      assert!(base.starts_with(EXPECTED_PREFIX.as_bytes()));
    };

    Ok(())
  }

  fn check_noto_serif_private(
    private: &[u8],
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      const EXPECTED_PREFIX: &str = r#"dup
/Private 8 dict dup begin
/RD{string currentfile exch readstring pop}executeonly def
/ND{noaccess def}executeonly def
/NP{noaccess put}executeonly def
/MinFeature{16 16}ND
/password 5839 def
/BlueValues [0 0 536 536 714 714 770 770 ]ND
/OtherSubrs"#;

      assert!(private.starts_with(EXPECTED_PREFIX.as_bytes()))
    };

    Ok(())
  }

  #[test]
  fn parse_ints() {
    let Ok(()) = check_tokens(
      "% a comment\n20 -30 2#1011 10#-5 %another!\r 16#fC",
      &[
        Token::Int(20),
        Token::Int(-30),
        Token::Int(11),
        Token::Int(-5),
        Token::Int(252),
      ],
    );
  }

  #[test]
  fn parse_num_to_int() -> ::core::result::Result<(), &'static str> {
    let Ok(mut parser) =
      Parser::new(b"102 102.1 102.4 102.5 102.9 -102.1 -102.5 -102.9 8#146 16#66");

    for _ in 0..10 {
      let Ok(num_as_int) = parser.read_num_as_int();
      assert_eq!(
        match num_as_int {
          ::core::option::Option::Some(read_value) => read_value,
          ::core::option::Option::None =>
            return ::core::result::Result::Err(::core::concat!(
              ::core::file!(),
              ":",
              ::core::line!(),
              ": expected Some, found None"
            )),
        }
        .abs(),
        102
      );
    }

    let Ok(next) = parser.next();
    assert!(next.is_none());
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_strings() {
    let Ok(()) = check_tokens(
      "(string (nested) 1) % and a hex string:\n <DEAD BEEF>",
      &[
        Token::LitString(b"string (nested) 1"),
        Token::HexString(b"DEAD BEEF"),
      ],
    );
  }

  #[test]
  fn parse_unterminated_strings() {
    let Ok(()) = check_tokens("(string (nested) 1", &[]);
    let Ok(()) = check_tokens("<DEAD BEEF", &[]);
  }

  #[test]
  fn parse_procs() {
    let Ok(()) = check_tokens(
      "{a {nested 20} proc } % and a\n {simple proc}",
      &[
        Token::Proc(b"a {nested 20} proc "),
        Token::Proc(b"simple proc"),
      ],
    );
  }

  #[test]
  fn parse_unterminated_procs() {
    let Ok(()) = check_tokens("{a {nested 20} proc", &[]);
  }

  #[test]
  fn parse_long_integer_runs_and_deep_procedures() -> Result<(), crate::test_support::Failure> {
    let integers = b"1 ".repeat(100_000);
    let Ok(mut parser) = Parser::new(&integers);

    for _integer in 0..100_000 {
      assert_eq!(parser.next(), Ok(Some(Token::Int(1))));
    }

    assert_eq!(parser.next(), Ok(None));

    let mut procedure = b"{ ".repeat(49_999);
    procedure.push(b'{');
    procedure.extend(core::iter::repeat_n(b'}', 50_000));

    let Ok(mut nested) = Parser::new(&procedure);
    let Ok(token) = nested.next();

    match token {
      Some(Token::Proc(content)) => assert_eq!(content.len(), procedure.len().wrapping_sub(2)),
      Some(_) | None => {
        return Err(crate::test_support::Failure::from(
          "expected the complete nested procedure",
        ));
      }
    }

    assert_eq!(nested.next(), Ok(None));

    Ok(())
  }

  #[test]
  fn parse_names() {
    let Ok(()) = check_tokens(
      "/FontMatrix\r %comment\n /CharStrings",
      &[Token::Name(b"FontMatrix"), Token::Name(b"CharStrings")],
    );
  }

  #[test]
  fn parse_binary_blobs() {
    {
      let Ok(explicit_value) = check_tokens(
        "/.notdef 4 RD abcd \n5 11\n \t-| a83jnshf7 3 ",
        &[
          Token::Name(b".notdef"),
          Token::Binary(b"abcd"),
          Token::Int(5),
          Token::Binary(b"a83jnshf7 3"),
        ],
      );
      explicit_value
    }
  }

  #[test]
  fn parse_base_dict_prefix() -> ::core::result::Result<(), &'static str> {
    let Ok(raw_dicts) = RawDicts::new(font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFA);
    let dicts = match raw_dicts {
      ::core::option::Option::Some(read_value) => read_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(::core::concat!(
          ::core::file!(),
          ":",
          ::core::line!(),
          ": expected Some, found None"
        ));
      }
    };
    let ::core::result::Result::Ok(ts) = parse_to_tokens(dicts.base);
    assert_eq!(
      ts.iter().take(19).copied().collect::<Vec<_>>(),
      &[
        Token::Int(10),
        Token::Raw(b"dict"),
        Token::Raw(b"begin"),
        Token::Name(b"FontType"),
        Token::Int(1),
        Token::Raw(b"def"),
        Token::Name(b"FontMatrix"),
        Token::Raw(b"["),
        Token::Raw(b"0.001"),
        Token::Int(0),
        Token::Int(0),
        Token::Raw(b"0.001"),
        Token::Int(0),
        Token::Int(0),
        Token::Raw(b"]"),
        Token::Raw(b"readonly"),
        Token::Raw(b"def"),
        Token::Name(b"FontName"),
        Token::Name(b"NotoSerif-Regular"),
      ]
    );
    ::core::result::Result::Ok(())
  }

  #[track_caller]
  fn check_tokens(
    source: &str,
    expected: &[Token],
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let ::core::result::Result::Ok(ts) = parse_to_tokens(source.as_bytes());
      assert_eq!(ts, expected);
    };

    Ok(())
  }

  fn parse_to_tokens(
    data: &'_ [u8],
  ) -> ::core::result::Result<Vec<Token<'_>>, ::console_core_never::Never> {
    let mut tokens = vec![];
    let Ok(mut parser) = Parser::new(data);

    let Ok(remaining_input) = parser.unparsed_bytes();

    for _ in remaining_input {
      let token = match parser.next() {
        Ok(Some(token)) => token,
        Ok(None) => break,
      };
      tokens.push(token);
    }

    ::core::result::Result::Ok(tokens)
  }

  #[test]
  fn parse_fixed() -> Result<(), crate::test_support::Failure> {
    let cases: &[(&[u8], i32, f64)] = &[
      (b"42.5", 0, 42.5),
      (b"0.0015", 0, 0.001495361328125),
      (b"425.000e-1", 0, 42.5),
      (b"1.5e-3", 0, 0.001495361328125),
      (b"1.5", 3, 1500.0),
      (b"0.001", 3, 1.0),
      (b"15000e-4", 3, 1500.0),
      (b"1.000e-3", 3, 1.0),
    ];

    for &(input, power_ten, reference) in cases {
      let Ok(parsed) = decode_fixed(input, power_ten);
      let actual = crate::test_support::fixture_value(parsed, "Type 1 fixed-point number")?;
      let Ok(expected) = Fixed::from_f64(reference);

      assert_eq!(actual, expected, "input {input:?} with power {power_ten}");
    }

    Ok(())
  }

  #[test]
  fn parse_font_matrix() -> Result<(), crate::test_support::Failure> {
    use crate::test_support::fixture_value;

    let Ok(mut identity) = Parser::new(b"[0.001 0 0 0.001 0 0]");
    let Ok(identity_matrix) = identity.read_font_matrix();
    let actual_identity = fixture_value(identity_matrix, "identity font matrix")?;
    assert_eq!(actual_identity.matrix, FontMatrix::IDENTITY);

    let Ok(double) = Fixed::from_i32(2);
    let Ok(tx) = Fixed::from_bits(1000);
    let Ok(ty) = Fixed::from_bits(20000);
    let Ok(translated) =
      FontMatrix::from_elements([double, Fixed::ZERO, Fixed::ZERO, Fixed::ONE, tx, ty]);
    let Ok(mut with_translation) = Parser::new(b"[0.002 0 0 0.001 1 2e1]");
    let Ok(translated_matrix) = with_translation.read_font_matrix();
    let actual_translation = fixture_value(translated_matrix, "translated font matrix")?;
    assert_eq!(actual_translation.matrix, translated);

    let Ok(scaled) = FontMatrix::from_elements([
      double,
      Fixed::ZERO,
      Fixed::ZERO,
      Fixed::ONE,
      Fixed::ZERO,
      Fixed::ZERO,
    ]);
    let Ok(mut with_scaling) = Parser::new(b"[0.001 0 0 0.0005 0.0 0.0]");
    let Ok(scaled_matrix) = with_scaling.read_font_matrix();
    let actual_scale = fixture_value(scaled_matrix, "scaled font matrix")?;
    assert_eq!(
      actual_scale,
      ScaledFontMatrix {
        matrix: scaled,
        scale: 2000
      }
    );

    Ok(())
  }

  #[test]
  fn parse_subrs() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(raw_dicts) = RawDicts::new(font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFA);
    let dicts = crate::test_support::fixture_value(raw_dicts, "fixture value")?;
    let Ok(mut parser) = Parser::new(&dicts.private);
    let mut subrs = None;

    let Ok(remaining_input) = parser.unparsed_bytes();

    for _ in remaining_input {
      let token = match parser.next() {
        Ok(Some(token)) => token,
        Ok(None) => break,
      };

      match token {
        Token::Name(b"Subrs") => {
          let Ok(parser_subrs) = parser.read_subrs(4);
          subrs = parser_subrs;
          break;
        }
        Token::Int(_)
        | Token::LitString(_)
        | Token::HexString(_)
        | Token::Proc(_)
        | Token::Binary(_)
        | Token::Name(_)
        | Token::Raw(_) => {}
      }
    }

    let mut parsed_subrs = crate::test_support::fixture_value(subrs, "fixture value")?;

    let expected_subrs: [&[u8]; 5] = [
      &[142, 139, 12, 16, 12, 17, 12, 17, 12, 33, 11],
      &[139, 140, 12, 16, 11],
      &[139, 141, 12, 16, 11],
      &[11],
      &[140, 142, 12, 16, 12, 17, 10, 11],
    ];
    assert_eq!(parsed_subrs.index.len(), expected_subrs.len());
    assert!(parsed_subrs.is_dense);

    for is_dense in [true, false] {
      parsed_subrs.is_dense = is_dense;

      for (idx, &expected) in expected_subrs.iter().enumerate() {
        let subr = crate::test_support::fixture_value(
          {
            let Ok(explicit_value) = parsed_subrs.get({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(idx);
              migrated_value
            });
            explicit_value
          },
          "fixture value",
        )?;
        assert_eq!(subr, expected);
      }
    }

    Ok(())
  }

  #[test]
  fn parse_empty_array_subrs() -> ::core::result::Result<(), &'static str> {
    let Ok(mut parser) = Parser::new(b"[ ]");
    let Ok(parser_subrs) = parser.read_subrs(4);
    let subrs = match parser_subrs {
      ::core::option::Option::Some(read_value) => read_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(::core::concat!(
          ::core::file!(),
          ":",
          ::core::line!(),
          ": expected Some, found None"
        ));
      }
    };
    assert!(subrs.data.is_empty());
    assert!(subrs.index.is_empty());
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_empty_subrs() -> ::core::result::Result<(), &'static str> {
    let Ok(mut parser) = Parser::new(b" 0 array\nND\n");
    let Ok(parser_subrs) = parser.read_subrs(4);
    let subrs = match parser_subrs {
      ::core::option::Option::Some(read_value) => read_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(::core::concat!(
          ::core::file!(),
          ":",
          ::core::line!(),
          ": expected Some, found None"
        ));
      }
    };
    assert!(subrs.data.is_empty());
    assert!(subrs.index.is_empty());
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_malformed_subrs() {
    let Ok(mut parser) = Parser::new(b" 20 \nND\n");
    let Ok(subrs) = parser.read_subrs(4);
    assert!(subrs.is_none());
  }

  #[test]
  fn parse_subrs_duplicate_def() -> ::core::result::Result<(), &'static str> {
    let private = b"/Subrs 2 array dup 5 2 RD nd NP dup 42 2 RD ab NP ND\n/Subrs 1 array dup 0 2 RD xy NP ND /CharStrings 0";
    let Ok(type1font) = Type1Font::from_dicts(b"", private);
    let font = match type1font {
      ::core::option::Option::Some(read_value) => read_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(::core::concat!(
          ::core::file!(),
          ":",
          ::core::line!(),
          ": expected Some, found None"
        ));
      }
    };
    assert_eq!(font.subrs.index.len(), 2);
    assert_eq!(font.subrs.index.first().map(|entry| entry.0), Some(5));
    assert_eq!(font.subrs.index.get(1).map(|entry| entry.0), Some(42));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_charstrings() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(raw_dicts) = RawDicts::new(font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFA);
    let dicts = crate::test_support::fixture_value(raw_dicts, "fixture value")?;
    let Ok(mut parser) = Parser::new(&dicts.private);
    let mut charstrings = None;

    let Ok(remaining_input) = parser.unparsed_bytes();

    for _ in remaining_input {
      let token = match parser.next() {
        Ok(Some(token)) => token,
        Ok(None) => break,
      };

      match token {
        Token::Name(b"CharStrings") => {
          let Ok(parser_charstrings) = parser.read_charstrings(4);
          charstrings = parser_charstrings;
          break;
        }
        Token::Int(_)
        | Token::LitString(_)
        | Token::HexString(_)
        | Token::Proc(_)
        | Token::Binary(_)
        | Token::Name(_)
        | Token::Raw(_) => {}
      }
    }

    let parsed_charstrings = crate::test_support::fixture_value(charstrings, "fixture value")?;
    let Ok(num_glyphs) = parsed_charstrings.num_glyphs();
    assert_eq!(num_glyphs, 9);
    assert!(parsed_charstrings.orig_notdef_index.is_none());
    let expected_names = [
      ".notdef",
      "H",
      "f",
      "i",
      "x",
      "f_f.liga",
      "f_f_i.liga",
      "f_i.liga",
      "H.c2sc",
    ];
    let Ok(charstrings_num_glyphs) = parsed_charstrings.num_glyphs();
    let names = (0..charstrings_num_glyphs)
      .map(|idx| {
        let Ok(name) = parsed_charstrings.name(idx);

        crate::test_support::fixture_value(name, "Type 1 charstring name")
      })
      .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(names, expected_names);

    let expected_charstrings_prefix: [&[u8]; 9] = [
      &[139, 248, 236, 13, 14],
      &[177, 249, 173, 13, 139, 4, 247, 183],
      &[166, 248, 5, 13, 139, 4, 247, 201],
      &[162, 247, 212, 13, 247, 30, 249, 16],
      &[144, 248, 214, 13, 139, 4, 247, 130],
      &[166, 249, 88, 13, 139, 4, 247, 181],
      &[166, 250, 126, 13, 139, 4, 247, 181],
      &[166, 249, 43, 13, 139, 4, 247, 181],
      &[180, 249, 60, 13, 139, 4, 247, 141],
    ];
    for (idx, &expected) in expected_charstrings_prefix.iter().enumerate() {
      let charstring = crate::test_support::fixture_value(
        {
          let Ok(explicit_value) = parsed_charstrings.get({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(idx);
            migrated_value
          });
          explicit_value
        },
        "fixture value",
      )?;
      assert!(charstring.starts_with(expected));
    }

    Ok(())
  }

  #[test]
  fn parse_charstrings_duplicate_def() -> ::core::result::Result<(), &'static str> {
    let private = b"/CharStrings 2 /.notdef 2 RD nd ND /H 2 RD ab ND /I 2 RD cd ND def\n/CharStrings 1 /B 2 RD xy ND def";
    let Ok(type1font) = Type1Font::from_dicts(b"", private);
    let font = match type1font {
      ::core::option::Option::Some(read_value) => read_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(::core::concat!(
          ::core::file!(),
          ":",
          ::core::line!(),
          ": expected Some, found None"
        ));
      }
    };
    assert_eq!((font.num_glyphs()), 3);
    let Ok(name) = font.charstrings.name(0);
    assert_eq!(name, ::core::option::Option::Some(".notdef"));
    let Ok(font_name) = font.charstrings.name(1);
    assert_eq!(font_name, ::core::option::Option::Some("H"));
    assert_eq!(
      ({
        let Ok(explicit_value) = font.charstrings.name(2);
        explicit_value
      }),
      ::core::option::Option::Some("I")
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_charstrings_missing_notdef() -> Result<(), crate::test_support::Failure> {
    let Ok(mut parser) = Parser::new(b"1 /H 2 RD ab ND /B 2 RD xy ND");
    let Ok(parser_charstrings) = parser.read_charstrings(-1);
    let charstrings =
      crate::test_support::fixture_value(parser_charstrings, "Type 1 CharStrings fixture")?;
    let Ok(num_glyphs) = charstrings.num_glyphs();
    assert_eq!(num_glyphs, 3);
    assert_eq!(charstrings.orig_notdef_index, Some(2));
    let expected_glyphs: &[(&str, &[u8])] =
      &[(".notdef", NOTDEF_GLYPH), ("B", b"xy"), ("H", b"ab")];
    check_charstrings(&charstrings, expected_glyphs)?;
    let Ok(mut font) = Type1Font::empty();
    font.charstrings = charstrings;
    assert_eq!(
      font.remapped_gid({
        let Ok(explicit_value) = GlyphId::new(0);
        explicit_value
      }),
      {
        let Ok(value) = GlyphId::new(2);
        value
      }
    );
    assert_eq!(
      font.remapped_gid({
        let Ok(explicit_value) = GlyphId::new(1);
        explicit_value
      }),
      {
        let Ok(value) = GlyphId::new(1);
        value
      }
    );
    assert_eq!(
      font.remapped_gid({
        let Ok(explicit_value) = GlyphId::new(2);
        explicit_value
      }),
      {
        let Ok(value) = GlyphId::new(0);
        value
      }
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_charstrings_notdef_moved() -> Result<(), crate::test_support::Failure> {
    let Ok(mut parser) = Parser::new(b"1 /H 2 RD ab ND /.notdef 2 RD nd ND /B 2 RD xy ND");
    let Ok(parser_charstrings) = parser.read_charstrings(-1);
    let charstrings =
      crate::test_support::fixture_value(parser_charstrings, "Type 1 CharStrings fixture")?;
    let Ok(num_glyphs) = charstrings.num_glyphs();
    assert_eq!(num_glyphs, 3);
    assert_eq!(charstrings.orig_notdef_index, Some(1));
    let expected_glyphs: &[(&str, &[u8])] = &[(".notdef", b"nd"), ("H", b"ab"), ("B", b"xy")];
    check_charstrings(&charstrings, expected_glyphs)?;
    let Ok(mut font) = Type1Font::empty();
    font.charstrings = charstrings;
    assert_eq!(
      font.remapped_gid({
        let Ok(explicit_value) = GlyphId::new(0);
        explicit_value
      }),
      {
        let Ok(value) = GlyphId::new(1);
        value
      }
    );
    assert_eq!(
      font.remapped_gid({
        let Ok(explicit_value) = GlyphId::new(1);
        explicit_value
      }),
      {
        let Ok(value) = GlyphId::new(0);
        value
      }
    );
    assert_eq!(
      font.remapped_gid({
        let Ok(explicit_value) = GlyphId::new(2);
        explicit_value
      }),
      {
        let Ok(value) = GlyphId::new(2);
        value
      }
    );
    ::core::result::Result::Ok(())
  }

  #[track_caller]
  fn check_charstrings(
    charstrings: &Charstrings,
    expected_glyphs: &[(&str, &[u8])],
  ) -> Result<(), ReadError> {
    for (idx, expected) in expected_glyphs.iter().enumerate() {
      let idx_2 = {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(idx);
        migrated_value
      };
      let Ok(charstrings_name) = charstrings.name(idx_2);
      let name =
        charstrings_name.ok_or(ReadError::MalformedData("expected Type 1 glyph name"))?;
      let Ok(charstring) = charstrings.get(idx_2);
      let data = charstring.ok_or(ReadError::MalformedData("expected Type 1 charstring"))?;
      assert_eq!((name, data), *expected);
    }

    Ok(())
  }

  #[test]
  fn parse_weight_vector() -> ::core::result::Result<(), &'static str> {
    let Ok(mut parser) = Parser::new(b"[0 0.125, 1.25 -0.87]");
    let Ok(weight_vector) = parser.read_weight_vector();
    let weights = match weight_vector {
      ::core::option::Option::Some(read_value) => read_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(::core::concat!(
          ::core::file!(),
          ":",
          ::core::line!(),
          ": expected Some, found None"
        ));
      }
    }
    .drain(..)
    .map(|w| {
      let Ok(value) = w.to_f32();
      value
    })
    .collect::<Vec<_>>();
    assert_eq!(weights, &[0.0, 0.125, 1.25, -0.8699951]);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_type1_font_pfb() -> ::core::result::Result<(), crate::ps::error::Error> {
    let Ok(()) = check_type1_font(&match Type1Font::new(
      font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFB,
    ) {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => {
        return ::core::result::Result::Err(read_error);
      }
    });
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_type1_font_pfa() -> ::core::result::Result<(), crate::ps::error::Error> {
    let Ok(()) = check_type1_font(&match Type1Font::new(
      font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFA,
    ) {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => {
        return ::core::result::Result::Err(read_error);
      }
    });
    ::core::result::Result::Ok(())
  }

  #[track_caller]
  fn check_type1_font(font: &Type1Font) -> core::result::Result<(), console_core_never::Never> {
    {
      assert_eq!(font.name(), Some("NotoSerif-Regular"));
      assert_eq!(font.full_name(), Some("Noto Serif Regular"));
      assert_eq!(font.family_name(), Some("Noto Serif"));
      assert_eq!(font.weight(), Some("Book"));
      assert_eq!(font.italic_angle(), 0);
      assert_eq!(font.pitch(), Pitch::Proportional);
      assert_eq!(font.underline_position(), -125);
      assert_eq!(font.underline_thickness(), 50);
      assert_eq!(
        font.bbox(),
        BoundingBox {
          x_min: {
            let Ok(value) = Fixed::from_i32(5);
            value
          },
          y_min: Fixed::ZERO,
          x_max: {
            let Ok(value) = Fixed::from_i32(989);
            value
          },
          y_max: {
            let Ok(value) = Fixed::from_i32(775);
            value
          }
        }
      );
      assert_eq!((font.num_glyphs()), 9);
      assert_eq!(font.subrs.index.len(), 5);
      assert_eq!(
        font.matrix,
        ScaledFontMatrix {
          matrix: FontMatrix::IDENTITY,
          scale: 1000
        }
      );

      assert!(
        font
          .glyph_names()
          .map(|(_, name)| name)
          .take(4)
          .eq([".notdef", "H", "f", "i"].into_iter())
      )
    };

    Ok(())
  }

  #[test]
  fn parse_encoding() -> ::core::result::Result<(), crate::test_support::Failure> {
    let fixture_value = crate::test_support::fixture_value(
      Type1Font::new(font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFA),
      "fixture value",
    )?;
    assert!(matches!(
      fixture_value.encoding,
      Some(RawEncoding::Predefined(PredefinedEncoding::Standard)),
    ));

    Ok(())
  }

  #[test]
  fn parse_known_encodings() -> ::core::result::Result<(), crate::test_support::Failure> {
    for (blob, encoding) in [
      (
        "StandardEncoding",
        RawEncoding::Predefined(PredefinedEncoding::Standard),
      ),
      (
        "ExpertEncoding",
        RawEncoding::Predefined(PredefinedEncoding::Expert),
      ),
      (
        "ISOLatin1Encoding",
        RawEncoding::Predefined(PredefinedEncoding::IsoLatin1),
      ),
    ] {
      let Ok(mut parser) = Parser::new(blob.as_bytes());
      let Ok(parser_encoding) = parser.read_encoding(&Charstrings::default());
      let fixture_value = crate::test_support::fixture_value(parser_encoding, "fixture value")?;
      assert_eq!(fixture_value, encoding);
    }

    Ok(())
  }

  #[test]
  fn parse_custom_dense_encoding() -> Result<(), crate::test_support::Failure> {
    let mut map = [".notdef"; 256];
    let Ok(mut parser) = Parser::new(b"[/.notdef /A /b /.notdef /comma /at]");
    let Ok(entries) = parser.dense_encoding();

    for (code, name) in entries {
      let index = u64::try_from(code).map_err(|_| crate::ReadError::OutOfBounds)?;
      let Ok(position) = console_core_number_conversion::index(index);
      let slot = crate::test_support::fixture_value(map.get_mut(position), "encoding code")?;
      *slot = name;
    }

    for (ch, entry) in map.iter().enumerate() {
      let expected = match ch {
        1 => "A",
        2 => "b",
        4 => "comma",
        5 => "at",
        _ => ".notdef",
      };
      assert_eq!(*entry, expected);
    }

    Ok(())
  }

  #[test]
  fn parse_custom_sparse_encoding() -> Result<(), crate::test_support::Failure> {
    let mut map = [".notdef"; 256];
    let Ok(mut parser) = Parser::new(CUSTOM_SPARSE_ENCODING.as_bytes());
    let Ok(entries) = parser.sparse_encoding();

    for (code, name) in entries {
      let index = u64::try_from(code).map_err(|_| crate::ReadError::OutOfBounds)?;
      let Ok(position) = console_core_number_conversion::index(index);
      let slot = crate::test_support::fixture_value(map.get_mut(position), "encoding code")?;
      *slot = name;
    }

    for (ch, entry) in map.iter().enumerate() {
      let expected = match ch {
        66 => "B",
        97 => "a",
        64 => "at",
        44 => "comma",
        56 => "eight",
        _ => ".notdef",
      };
      assert_eq!(*entry, expected);
    }

    Ok(())
  }

  #[test]
  fn sparse_encoding_ignores_negative_codes() {
    let mut charstrings = Charstrings::default();
    let Ok(()) = charstrings.push(b".notdef", &[14], -1);
    let Ok(()) = charstrings.push(b"H", &[14], -1);
    let Ok(mut parser) = Parser::new(b"2 array dup -1 /H put dup 1 /H put def");
    let Ok(encoding) = parser.read_encoding(&charstrings);
    let Ok(h) = GlyphId::new(1);

    assert_eq!(
      encoding,
      Some(RawEncoding::Custom(vec![GlyphId::NOTDEF, h]))
    );
  }

  const CUSTOM_SPARSE_ENCODING: &str = r#"
        array
        0 1 255 {1 index exch /.notdef put} for
        dup 66 /B put
        dup 97 /a put
        dup 64 /at put
        dup 44 /comma put
        dup 56 /eight put
        readonly def
    "#;

  #[test]
  fn eval_charstrings() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      Type1Font::new(font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFA),
      "fixture value",
    )?;
    let expected_eval_prefix = [
      "M38,0 L329,0 L329,42 L316,42 C293,42 274,46 258,54 C242,63 234,83 234,114",
      "M27,0 L336,0 L336,42 L298,42 C275,42 256,46 240,54 C224,63 216,83 216,114",
      "M161,636 C176,636 190,641 201,650 C212,659 218,675 218,698 C218,721 212,738 201,746",
      "M5,0 L243,0 L243,42 L240,42 C218,42 202,44 192,50 C183,54 178,62 178,73",
      "M27,0 L316,0 L316,42 L298,42 C275,42 256,46 240,54 C224,63 216,83 216,114",
      "M27,0 L316,0 L316,42 L298,42 C275,42 256,46 240,54 C224,63 216,83 216,114",
      "M27,0 L316,0 L316,42 L298,42 C275,42 256,46 240,54 C224,63 216,83 216,114",
      "M41,0 L290,0 L290,42 L269,42 C254,42 240,45 229,52 C218,58 212,73 212,98",
    ];

    assert_eq!(
      ::core::primitive::u64::wrapping_sub(
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(font.num_glyphs());
          migrated_value
        },
        1
      ),
      ({
        let Ok(explicit_index) =
          console_core_number_conversion::fitted::<_, u64>(expected_eval_prefix.len());
        explicit_index
      })
    );
    let mut commands = CaptureCommandSink::default();

    for (gid, expected_prefix) in (1..(font.num_glyphs())).zip(&expected_eval_prefix) {
      commands.0.clear();
      crate::test_support::fixture_value(
        font.evaluate_charstring(
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(gid),
          &mut commands,
        ),
        "fixture value",
      )?;
      let svg = commands.to_svg()?;
      assert!(svg.starts_with(expected_prefix));
    }

    Ok(())
  }

  #[test]
  fn eval_charstring_widths() -> ::core::result::Result<(), crate::ps::error::Error> {
    let font = Type1Font::new(font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFA)?;
    let expected_widths = [
      600.0, 793.0, 369.0, 320.0, 578.0, 708.0, 1002.0, 663.0, 680.0,
    ];
    let mut commands = CaptureCommandSink::default();
    let mut widths = Vec::new();

    for id in 0..font.num_glyphs() {
      commands.0.clear();
      let Ok(glyph) = GlyphId::new(id);
      let width = font.evaluate_charstring(glyph, &mut commands)?;
      let present = width.ok_or(Error::Read(ReadError::MalformedData(
        "missing Type 1 glyph width",
      )))?;
      let Ok(width_value) = present.to_f32();
      widths.push(width_value);
    }

    assert_eq!(widths, expected_widths);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn csctx_seac_components() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = Type1Font::new(font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFA)?;

    let x_code = 120;
    let i_code = 105;
    let [x_data, i_data] = font.seac_components(x_code, i_code)?;
    let name_to_gid = |name| {
      font.glyph_names().find_map(|(glyph, glyph_name)| {
        let Ok(id) = glyph.to_u32();
        (name == glyph_name).then_some(id)
      })
    };
    let x_id = crate::test_support::fixture_value(name_to_gid("x"), "Type 1 x glyph")?;
    let Ok(x_charstring) = font.charstrings.get(x_id);
    let expected_x = crate::test_support::fixture_value(x_charstring, "Type 1 x charstring")?;
    assert_eq!(x_data, expected_x);
    let i_id = crate::test_support::fixture_value(name_to_gid("i"), "Type 1 i glyph")?;
    let Ok(i_charstring) = font.charstrings.get(i_id);
    let expected_i = crate::test_support::fixture_value(i_charstring, "Type 1 i charstring")?;
    assert_eq!(i_data, expected_i);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn csctx_subrs() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      Type1Font::new(font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFA),
      "fixture value",
    )?;
    assert!(!font.subrs.index.is_empty());

    for (subr_idx, _) in font.subrs.index.iter().enumerate() {
      let Ok(subr) = font.subrs.get({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(subr_idx);
        migrated_value
      });

      let fixture_value = crate::test_support::fixture_value(subr, "fixture value")?;

      assert_eq!(fixture_value, {
        let fixture_subroutine = crate::test_support::fixture_value(
          font.subr({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(subr_idx);
            migrated_value
          }),
          "fixture value",
        )?;
        fixture_subroutine
      })
    }

    Ok(())
  }

  #[test]
  fn encoding_mapping() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      Type1Font::new(font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFA),
      "fixture value",
    )?;
    let encoding = crate::test_support::fixture_value(font.encoding(), "fixture value")?;

    let expected = [
      (0, 0, ".notdef"),
      (72, 1, "H"),
      (102, 2, "f"),
      (105, 3, "i"),
      (120, 4, "x"),
    ];
    for (code, gid, name) in expected {
      assert_eq!(
        encoding.glyph_name(code),
        ::core::option::Option::Some(name)
      );
      assert_eq!(
        ({
          let fixture_value =
            crate::test_support::fixture_value(encoding.map(code), "fixture value")?;
          let Ok(value) = fixture_value.to_u32();
          value
        }),
        gid
      );
    }

    Ok(())
  }
}
