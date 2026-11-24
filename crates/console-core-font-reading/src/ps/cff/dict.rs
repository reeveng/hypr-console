//! Parsing for PostScript DICTs.

use super::{
  blend::BlendState,
  stack::{Number, Stack},
};
use crate::{
  Cursor, ReadError,
  ps::{
    error::Error,
    hinting::{Blues, StemSnaps},
    num::{self, BcdComponents},
    string::Sid,
    transform::ScaledFontMatrix,
  },
  types::Fixed,
};
use std::ops::Range;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Operator {
  Version,
  Notice,
  FullName,
  FamilyName,
  Weight,
  FontBbox,
  CharstringsOffset,
  PrivateDictRange,
  VariationStoreOffset,
  Copyright,
  IsFixedPitch,
  ItalicAngle,
  UnderlinePosition,
  UnderlineThickness,
  PaintType,
  CharstringType,
  FontMatrix,
  StrokeWidth,
  FdArrayOffset,
  FdSelectOffset,
  BlueValues,
  OtherBlues,
  FamilyBlues,
  FamilyOtherBlues,
  SubrsOffset,
  VariationStoreIndex,
  BlueScale,
  BlueShift,
  BlueFuzz,
  LanguageGroup,
  ExpansionFactor,
  Encoding,
  Charset,
  UniqueId,
  Xuid,
  SyntheticBase,
  PostScript,
  BaseFontName,
  BaseFontBlend,
  Ros,
  CidFontVersion,
  CidFontRevision,
  CidFontType,
  CidCount,
  UidBase,
  FontName,
  StdHw,
  StdVw,
  DefaultWidthX,
  NominalWidthX,
  Blend,
  StemSnapH,
  StemSnapV,
  ForceBold,
  InitialRandomSeed,
}

impl Operator {
  #[must_use]
  fn from_opcode(opcode: u8) -> core::result::Result<Option<Self>, console_core_never::Never> {
    use Operator::*;

    Ok(Some(match opcode {
      0 => Version,
      1 => Notice,
      2 => FullName,
      3 => FamilyName,
      4 => Weight,
      5 => FontBbox,
      13 => UniqueId,
      14 => Xuid,
      15 => Charset,
      16 => Encoding,
      17 => CharstringsOffset,
      18 => PrivateDictRange,
      24 => VariationStoreOffset,

      6 => BlueValues,
      7 => OtherBlues,
      8 => FamilyBlues,
      9 => FamilyOtherBlues,
      10 => StdHw,
      11 => StdVw,
      19 => SubrsOffset,
      20 => DefaultWidthX,
      21 => NominalWidthX,
      22 => VariationStoreIndex,
      23 => Blend,

      _ => return Ok(None),
    }))
  }

  #[must_use]
  fn from_extended_opcode(
    opcode: u8,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    use Operator::*;

    Ok(Some(match opcode {
      0 => Copyright,
      1 => IsFixedPitch,
      2 => ItalicAngle,
      3 => UnderlinePosition,
      4 => UnderlineThickness,
      5 => PaintType,
      6 => CharstringType,
      7 => FontMatrix,
      8 => StrokeWidth,
      20 => SyntheticBase,
      21 => PostScript,
      22 => BaseFontName,
      23 => BaseFontBlend,
      30 => Ros,
      31 => CidFontVersion,
      32 => CidFontRevision,
      33 => CidFontType,
      34 => CidCount,
      35 => UidBase,
      36 => FdArrayOffset,
      37 => FdSelectOffset,
      38 => FontName,

      9 => BlueScale,
      10 => BlueShift,
      11 => BlueFuzz,
      12 => StemSnapH,
      13 => StemSnapV,
      14 => ForceBold,
      17 => LanguageGroup,
      18 => ExpansionFactor,
      19 => InitialRandomSeed,
      _ => return Ok(None),
    }))
  }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Token {
  Operator(Operator),
  Operand(Number, Option<BcdComponents>),
}

impl From<Operator> for Token {
  fn from(value: Operator) -> Self {
    Self::Operator(value)
  }
}

impl<T> From<T> for Token
where
  T: Into<Number>,
{
  fn from(value: T) -> Self {
    Self::Operand(
      <_ as ::core::convert::Into<crate::ps::cff::stack::Number>>::into(value),
      None,
    )
  }
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the published `tokens` function returns its iterator or trait value directly for existing callers to compose"
  )
)]
pub fn tokens(dict_data: &[u8]) -> impl Iterator<Item = Result<Token, Error>> + '_ + Clone {
  let Ok(data) = crate::FontData::new(dict_data);
  let Ok(cursor) = data.cursor();
  core::iter::repeat_n((), dict_data.len()).scan(cursor, |cursor_2, ()| {
    match cursor_2.remaining_bytes() {
      Ok(0) => None,
      Ok(_remaining) => Some(parse_token(cursor_2)),
    }
  })
}

fn parse_token(cursor: &mut Cursor) -> Result<Token, Error> {
  const ESCAPE: u8 = 12;
  let b0 = cursor.read::<u8>()?;
  Ok(match ::core::primitive::bool::then_some(b0 == ESCAPE, ()) {
    ::core::option::Option::Some(()) => {
      let b1 = cursor.read::<u8>()?;
      let operator = ({
        let Ok(explicit_value) = Operator::from_extended_opcode(b1);
        explicit_value
      })
      .ok_or(Error::InvalidDictOperator(b1))?;

      Token::Operator(operator)
    }
    ::core::option::Option::None => {
      match b0 {
        28 | 29 | 32..=254 => {
          let int = num::parse_int(cursor, b0)?;

          Token::Operand(<crate::ps::cff::stack::Number as ::core::convert::From<::core::primitive::i32>>::from(int), None)
        }
        30 => {
          let components = BcdComponents::parse(cursor)?;
          Token::Operand(
            <crate::ps::cff::stack::Number as ::core::convert::From<::font_types::Fixed>>::from(
              {
                let Ok(explicit_value) = components.value(false);
                explicit_value
              },
            ),
            Some(components),
          )
        }
        _ => {
          let operator = ({
            let Ok(explicit_value) = Operator::from_opcode(b0);
            explicit_value
          })
          .ok_or(Error::InvalidDictOperator(b0))?;

          Token::Operator(operator)
        }
      }
    }
  })
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Entry {
  Version(Sid),
  Notice(Sid),
  FullName(Sid),
  FamilyName(Sid),
  Weight(Sid),
  FontBbox([Fixed; 4]),
  CharstringsOffset(u64),
  PrivateDictRange(Range<u64>),
  VariationStoreOffset(u64),
  Copyright(Sid),
  IsFixedPitch(crate::ps::Pitch),
  ItalicAngle(Fixed),
  UnderlinePosition(Fixed),
  UnderlineThickness(Fixed),
  PaintType(i32),
  CharstringType(i32),
  FontMatrix(ScaledFontMatrix),
  StrokeWidth(Fixed),
  FdArrayOffset(u64),
  FdSelectOffset(u64),
  BlueValues(Blues),
  OtherBlues(Blues),
  FamilyBlues(Blues),
  FamilyOtherBlues(Blues),
  SubrsOffset(u64),
  VariationStoreIndex(u16),
  BlueScale(Fixed),
  BlueShift(Fixed),
  BlueFuzz(Fixed),
  LanguageGroup(i32),
  ExpansionFactor(Fixed),
  Encoding(u64),
  Charset(u64),
  UniqueId(i32),
  Xuid,
  SyntheticBase(i32),
  PostScript(Sid),
  BaseFontName(Sid),
  BaseFontBlend,
  Ros {
    registry: Sid,
    ordering: Sid,
    supplement: Fixed,
  },
  CidFontVersion(Fixed),
  CidFontRevision(Fixed),
  CidFontType(i32),
  CidCount(u32),
  UidBase(i32),
  FontName(Sid),
  StdHw(Fixed),
  StdVw(Fixed),
  DefaultWidthX(Fixed),
  NominalWidthX(Fixed),
  StemSnapH(StemSnaps),
  StemSnapV(StemSnaps),
  ForceBold(bool),
  InitialRandomSeed(i32),
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the published `entries` function returns its iterator or trait value directly for existing callers to compose"
  )
)]
pub fn entries<'a>(
  dict_data: &'a [u8],
  blend_state: Option<BlendState<'a>>,
) -> impl Iterator<Item = Result<Entry, Error>> + 'a {
  let Ok(data) = crate::FontData::new(dict_data);
  let Ok(cursor) = data.cursor();
  let initial = (cursor, Stack::new(), None, blend_state, 0);
  core::iter::repeat_n((), dict_data.len()).scan(initial, move |state, ()| {
    let (cursor_3, stack, last_bcd_components, blend_state_2, cursor_pos) = state;
    let Ok(remaining) = cursor_3.remaining_bytes();

    for _ in 0..remaining {
      let Ok(value_5) = cursor_3.remaining_bytes();

      match ::core::primitive::bool::then_some(value_5 == 0, ()) {
        ::core::option::Option::Some(()) => {
          return None;
        }
        ::core::option::Option::None => {}
      }

      let token = match parse_token(cursor_3) {
        Ok(token) => token,
        Err(Error::InvalidDictOperator(_)) => {
          stack.clear();
          continue;
        }
        Err(e) => return Some(Err(e)),
      };

      match token {
        Token::Operand(number, bcd_components) => {
          *last_bcd_components = bcd_components;

          match stack.push(number) {
            Ok(_) => continue,
            Err(e) => return Some(Err(e)),
          }
        }
        Token::Operator(op) => {
          match ::core::primitive::bool::then_some(
            op == Operator::Blend || op == Operator::VariationStoreIndex,
            (),
          ) {
            ::core::option::Option::Some(()) => {
              let state_3 = match blend_state_2.as_mut() {
                Some(state_2) => state_2,
                None => return Some(Err(Error::MissingBlendState)),
              };

              match ::core::primitive::bool::then_some(op == Operator::VariationStoreIndex, ()) {
                ::core::option::Option::Some(()) => {
                  let operand = match stack.get_i32(0) {
                    Ok(index) => index,
                    Err(error) => return Some(Err(error)),
                  };
                  let Ok(index) = console_core_number_conversion::wrapped::<_, u16>(operand);

                  match state_3.set_store_index(index) {
                    Ok(()) => {}
                    Err(error) => return Some(Err(error)),
                  }
                }
                ::core::option::Option::None => {}
              }

              match ::core::primitive::bool::then_some(op == Operator::Blend, ()) {
                ::core::option::Option::Some(()) => match stack.apply_blend(state_3) {
                  Ok(_) => continue,
                  Err(e) => return Some(Err(e)),
                },
                ::core::option::Option::None => {}
              }
            }
            ::core::option::Option::None => {}
          }

          match ::core::primitive::bool::then_some(op == Operator::BlueScale, ()) {
            ::core::option::Option::Some(()) => match last_bcd_components.take() {
              Some(bcd_components) => {
                (match stack.pop_fixed() {
                  Ok(value) => Some(value),
                  Err(_invalid_bcd_operand) => None,
                })?;
                (match stack.push({
                  let Ok(explicit_value) = bcd_components.value(true);
                  explicit_value
                }) {
                  Ok(value) => Some(value),
                  Err(_operand_stack_full) => None,
                })?;
              }
              None => {}
            },
            ::core::option::Option::None => {}
          }

          match ::core::primitive::bool::then_some(op == Operator::FontMatrix, ()) {
            ::core::option::Option::Some(()) => {
              stack.clear();
              *last_bcd_components = None;

              let Ok(value_3) = crate::FontData::new(dict_data);

              let Ok(mut cursor_2) = value_3.cursor();
              let ::core::result::Result::Ok(()) = cursor_2.advance_by(*cursor_pos);

              let Ok(parse) = ScaledFontMatrix::parse(&mut cursor_2);

              match parse {
                Some(matrix) => {
                  return Some(Ok(Entry::FontMatrix(matrix)));
                }
                None => {}
              }

              continue;
            }
            ::core::option::Option::None => {}
          }

          *last_bcd_components = None;
          let entry = parse_entry(op, stack);
          stack.clear();
          *cursor_pos = match cursor_3.position() {
            Ok(value) => value,
            Err(_invalid_dict_position) => Default::default(),
          };
          return Some(entry);
        }
      }
    }

    None
  })
}

fn parse_entry(op: Operator, stack: &mut Stack) -> Result<Entry, Error> {
  use Operator::*;
  Ok(match op {
    Version => {
      let pop_i32 = stack.pop_i32()?;

      Entry::Version(<crate::ps::string::Sid as ::core::convert::From<
        ::core::primitive::i32,
      >>::from(pop_i32))
    }
    Notice => {
      let pop_i32 = stack.pop_i32()?;

      Entry::Notice(<crate::ps::string::Sid as ::core::convert::From<
        ::core::primitive::i32,
      >>::from(pop_i32))
    }
    FullName => {
      let pop_i32 = stack.pop_i32()?;

      Entry::FullName(<crate::ps::string::Sid as ::core::convert::From<
        ::core::primitive::i32,
      >>::from(pop_i32))
    }
    FamilyName => {
      let pop_i32 = stack.pop_i32()?;

      Entry::FamilyName(<crate::ps::string::Sid as ::core::convert::From<
        ::core::primitive::i32,
      >>::from(pop_i32))
    }
    Weight => {
      let pop_i32 = stack.pop_i32()?;

      Entry::Weight(<crate::ps::string::Sid as ::core::convert::From<
        ::core::primitive::i32,
      >>::from(pop_i32))
    }
    FontBbox => {
      let left = stack.get_fixed(0)?;
      let bottom = stack.get_fixed(1)?;
      let right = stack.get_fixed(2)?;
      let top = stack.get_fixed(3)?;

      Entry::FontBbox([left, bottom, right, top])
    }
    CharstringsOffset => Entry::CharstringsOffset({
      let pop_i32 = stack.pop_i32()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(pop_i32);
      migrated_value
    }),
    PrivateDictRange => {
      let len = {
        let checked_value = stack.get_i32(0)?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(checked_value);
        migrated_value
      };
      let start = {
        let checked_value_2 = stack.get_i32(1)?;
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(checked_value_2);
        migrated_value
      };
      let end = start.checked_add(len).ok_or(ReadError::OutOfBounds)?;
      Entry::PrivateDictRange(start..end)
    }
    VariationStoreOffset => Entry::VariationStoreOffset({
      let pop_i32 = stack.pop_i32()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(pop_i32);
      migrated_value
    }),
    Copyright => {
      let pop_i32 = stack.pop_i32()?;

      Entry::Copyright(<crate::ps::string::Sid as ::core::convert::From<
        ::core::primitive::i32,
      >>::from(pop_i32))
    }
    IsFixedPitch => {
      let pop_i32 = stack.pop_i32()?;

      Entry::IsFixedPitch(match pop_i32 {
        0 => crate::ps::Pitch::Proportional,
        _ => crate::ps::Pitch::Fixed,
      })
    }
    ItalicAngle => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::ItalicAngle(pop_fixed)
    }
    UnderlinePosition => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::UnderlinePosition(pop_fixed)
    }
    UnderlineThickness => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::UnderlineThickness(pop_fixed)
    }
    PaintType => {
      let pop_i32 = stack.pop_i32()?;

      Entry::PaintType(pop_i32)
    }
    CharstringType => {
      let pop_i32 = stack.pop_i32()?;

      Entry::CharstringType(pop_i32)
    }
    FontMatrix => {
      return Err(Error::Read(ReadError::MalformedData(
        "unexpected font matrix",
      )));
    }
    StrokeWidth => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::StrokeWidth(pop_fixed)
    }
    FdArrayOffset => Entry::FdArrayOffset({
      let pop_i32 = stack.pop_i32()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(pop_i32);
      migrated_value
    }),
    FdSelectOffset => Entry::FdSelectOffset({
      let pop_i32 = stack.pop_i32()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(pop_i32);
      migrated_value
    }),
    BlueValues => {
      stack.apply_delta_prefix_sum();
      Entry::BlueValues(Blues::new(stack.fixed_values()))
    }
    OtherBlues => {
      stack.apply_delta_prefix_sum();
      Entry::OtherBlues(Blues::new(stack.fixed_values()))
    }
    FamilyBlues => {
      stack.apply_delta_prefix_sum();
      Entry::FamilyBlues(Blues::new(stack.fixed_values()))
    }
    FamilyOtherBlues => {
      stack.apply_delta_prefix_sum();
      Entry::FamilyOtherBlues(Blues::new(stack.fixed_values()))
    }
    SubrsOffset => Entry::SubrsOffset({
      let pop_i32 = stack.pop_i32()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(pop_i32);
      migrated_value
    }),
    VariationStoreIndex => Entry::VariationStoreIndex({
      let pop_i32 = stack.pop_i32()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(pop_i32);
      migrated_value
    }),
    BlueScale => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::BlueScale(pop_fixed)
    }
    BlueShift => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::BlueShift(pop_fixed)
    }
    BlueFuzz => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::BlueFuzz(pop_fixed)
    }
    LanguageGroup => {
      let pop_i32 = stack.pop_i32()?;

      Entry::LanguageGroup(pop_i32)
    }
    ExpansionFactor => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::ExpansionFactor(pop_fixed)
    }
    Encoding => Entry::Encoding({
      let pop_i32 = stack.pop_i32()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(pop_i32);
      migrated_value
    }),
    Charset => Entry::Charset({
      let pop_i32 = stack.pop_i32()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(pop_i32);
      migrated_value
    }),
    UniqueId => {
      let pop_i32 = stack.pop_i32()?;

      Entry::UniqueId(pop_i32)
    }
    Xuid => Entry::Xuid,
    SyntheticBase => {
      let pop_i32 = stack.pop_i32()?;

      Entry::SyntheticBase(pop_i32)
    }
    PostScript => {
      let pop_i32 = stack.pop_i32()?;

      Entry::PostScript(<crate::ps::string::Sid as ::core::convert::From<
        ::core::primitive::i32,
      >>::from(pop_i32))
    }
    BaseFontName => {
      let pop_i32 = stack.pop_i32()?;

      Entry::BaseFontName(<crate::ps::string::Sid as ::core::convert::From<
        ::core::primitive::i32,
      >>::from(pop_i32))
    }
    BaseFontBlend => Entry::BaseFontBlend,
    Ros => {
      let registry = stack.get_i32(0)?;
      let ordering = stack.get_i32(1)?;
      let supplement = stack.get_fixed(2)?;

      Entry::Ros {
        registry: Sid::from(registry),
        ordering: Sid::from(ordering),
        supplement,
      }
    }
    CidFontVersion => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::CidFontVersion(pop_fixed)
    }
    CidFontRevision => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::CidFontRevision(pop_fixed)
    }
    CidFontType => {
      let pop_i32 = stack.pop_i32()?;

      Entry::CidFontType(pop_i32)
    }
    CidCount => Entry::CidCount({
      let pop_i32 = stack.pop_i32()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(pop_i32);
      migrated_value
    }),
    UidBase => {
      let pop_i32 = stack.pop_i32()?;

      Entry::UidBase(pop_i32)
    }
    FontName => {
      let pop_i32 = stack.pop_i32()?;

      Entry::FontName(<crate::ps::string::Sid as ::core::convert::From<
        ::core::primitive::i32,
      >>::from(pop_i32))
    }
    StdHw => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::StdHw(pop_fixed)
    }
    StdVw => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::StdVw(pop_fixed)
    }
    DefaultWidthX => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::DefaultWidthX(pop_fixed)
    }
    NominalWidthX => {
      let pop_fixed = stack.pop_fixed()?;

      Entry::NominalWidthX(pop_fixed)
    }
    StemSnapH => {
      stack.apply_delta_prefix_sum();
      Entry::StemSnapH(StemSnaps::new(stack.fixed_values()))
    }
    StemSnapV => {
      stack.apply_delta_prefix_sum();
      Entry::StemSnapV(StemSnaps::new(stack.fixed_values()))
    }
    ForceBold => {
      let pop_i32 = stack.pop_i32()?;

      Entry::ForceBold(pop_i32 != 0)
    }
    InitialRandomSeed => {
      let pop_i32 = stack.pop_i32()?;

      Entry::InitialRandomSeed(pop_i32)
    }

    Blend => {
      return Err(Error::Read(ReadError::MalformedData(
        "unexpected blend operator",
      )));
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    FontData, FontRead, FontRef, TableProvider, tables::variations::ItemVariationStore,
    types::F2Dot14,
  };

  #[test]
  fn example_top_dict_tokens() -> Result<(), Error> {
    use Operator::*;
    let top_dict_data = font_test_data::cff2::EXAMPLE
      .get(5..12)
      .ok_or(crate::ReadError::OutOfBounds)?;
    let tokens: Vec<_> = tokens(top_dict_data).collect::<Result<_, _>>()?;
    let expected: &[Token] = &[
            <crate::ps::cff::dict::Token as ::core::convert::From<::core::primitive::i32>>::from(68),
            <crate::ps::cff::dict::Token as ::core::convert::From<crate::ps::cff::dict::Operator>>::from(FdArrayOffset),
            <crate::ps::cff::dict::Token as ::core::convert::From<::core::primitive::i32>>::from(56),
            <crate::ps::cff::dict::Token as ::core::convert::From<crate::ps::cff::dict::Operator>>::from(CharstringsOffset),
            <crate::ps::cff::dict::Token as ::core::convert::From<::core::primitive::i32>>::from(16),
            <crate::ps::cff::dict::Token as ::core::convert::From<crate::ps::cff::dict::Operator>>::from(VariationStoreOffset),
        ];
    assert_eq!(&tokens, expected);
    Ok(())
  }

  #[test]
  fn example_top_dict_entries() -> Result<(), Error> {
    use Entry::*;
    let top_dict_data = font_test_data::cff2::EXAMPLE
      .get(0x5..=0xB)
      .ok_or(crate::ReadError::OutOfBounds)?;
    let entries: Vec<_> = entries(top_dict_data, None).collect::<Result<_, _>>()?;
    let expected: &[Entry] = &[
      FdArrayOffset(68),
      CharstringsOffset(56),
      VariationStoreOffset(16),
    ];
    assert_eq!(&entries, expected);
    Ok(())
  }

  #[test]
  fn example_private_dict_entries() -> ::core::result::Result<(), crate::test_support::Failure> {
    use Entry::*;
    let private_dict_data = crate::test_support::fixture_value(
      font_test_data::cff2::EXAMPLE.get(0x4f..=0xc0),
      "CFF2 fixture private DICT",
    )?;
    let variation_data = crate::test_support::fixture_value(
      font_test_data::cff2::EXAMPLE.get(18..),
      "CFF2 fixture variation store bytes",
    )?;
    let Ok(store_data) = FontData::new(variation_data);
    let store = ItemVariationStore::read(store_data)?;
    let coords = &[F2Dot14::ZERO];
    let blend_state =
      crate::test_support::fixture_value(BlendState::new(store, coords, 0), "CFF2 blend state")?;
    let entries: Vec<_> =
      entries(private_dict_data, Some(blend_state)).collect::<Result<_, _>>()?;

    let fixture_fixed = |&value: &f64| {
      let Ok(fixed) = Fixed::from_f64(value);

      fixed
    };

    let Ok(blue_scale) = Fixed::from_f64(0.037506103515625);
    let Ok(std_hw) = Fixed::from_i32(55);
    let Ok(std_vw) = Fixed::from_i32(80);
    let expected: &[Entry] = &[
      BlueValues(Blues::new(
        [
          -20.0, 0.0, 472.0, 490.0, 525.0, 540.0, 645.0, 660.0, 670.0, 690.0, 730.0, 750.0,
        ]
        .iter()
        .map(fixture_fixed),
      )),
      OtherBlues(Blues::new([-250.0, -240.0].iter().map(fixture_fixed))),
      FamilyBlues(Blues::new(
        [
          -20.0, 0.0, 473.0, 491.0, 525.0, 540.0, 644.0, 659.0, 669.0, 689.0, 729.0, 749.0,
        ]
        .iter()
        .map(fixture_fixed),
      )),
      FamilyOtherBlues(Blues::new([-249.0, -239.0].iter().map(fixture_fixed))),
      BlueScale(blue_scale),
      BlueFuzz(Fixed::ZERO),
      StdHw(std_hw),
      StdVw(std_vw),
      StemSnapH(StemSnaps::new([40.0, 55.0].iter().map(fixture_fixed))),
      StemSnapV(StemSnaps::new([80.0, 90.0].iter().map(fixture_fixed))),
      SubrsOffset(114),
    ];
    assert_eq!(&entries, expected);

    Ok(())
  }

  #[test]
  fn noto_serif_display_top_dict_entries()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    use Entry::*;
    let fixture_value = crate::test_support::fixture_value(
      FontRef::new(font_test_data::NOTO_SERIF_DISPLAY_TRIMMED),
      "fixture value",
    )?;
    let top_dict_data = {
      let cff_table = crate::test_support::fixture_value(fixture_value.cff(), "fixture value")?;
      cff_table
    }
    .top_dicts()
    .get(0)?;
    let entries: Vec<_> = entries(top_dict_data, None).collect::<Result<_, _>>()?;
    let expected = &[
      Version(Sid::new(391)),
      Notice(Sid::new(392)),
      Copyright(Sid::new(393)),
      FullName(Sid::new(394)),
      FamilyName(Sid::new(395)),
      FontBbox([-693.0, -470.0, 2797.0, 1048.0].map(|argument| {
        let Ok(value_11) = Fixed::from_f64(argument);

        value_11
      })),
      Charset(517),
      PrivateDictRange(549..587),
      CharstringsOffset(521),
    ];
    assert_eq!(&entries, expected);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn private_dict_range_avoid_overflow() {
    let private_dict =
      font_test_data::be_buffer! { (29u8), (-1i32), (29u8), (-1i32), (18u8) }.to_vec();

    let _ = entries(&private_dict, None).count();
  }

  #[test]
  fn read_font_matrix() -> ::core::result::Result<(), crate::test_support::Failure> {
    let dict_data = [
      30u8, 10, 0, 31, 139, 30, 10, 0, 1, 103, 255, 30, 10, 0, 31, 139, 139, 12, 7,
    ];
    let matrix = {
      let fixture_value =
        crate::test_support::fixture_value(entries(&dict_data, None).next(), "fixture value")?;
      let outcome_matrix = fixture_value?;

      match outcome_matrix {
        Entry::FontMatrix(matched_matrix) => matched_matrix,
        crate::ps::cff::dict::Entry::Version(..)
        | crate::ps::cff::dict::Entry::Notice(..)
        | crate::ps::cff::dict::Entry::FullName(..)
        | crate::ps::cff::dict::Entry::FamilyName(..)
        | crate::ps::cff::dict::Entry::Weight(..)
        | crate::ps::cff::dict::Entry::FontBbox(..)
        | crate::ps::cff::dict::Entry::CharstringsOffset(..)
        | crate::ps::cff::dict::Entry::PrivateDictRange(..)
        | crate::ps::cff::dict::Entry::VariationStoreOffset(..)
        | crate::ps::cff::dict::Entry::Copyright(..)
        | crate::ps::cff::dict::Entry::IsFixedPitch(..)
        | crate::ps::cff::dict::Entry::ItalicAngle(..)
        | crate::ps::cff::dict::Entry::UnderlinePosition(..)
        | crate::ps::cff::dict::Entry::UnderlineThickness(..)
        | crate::ps::cff::dict::Entry::PaintType(..)
        | crate::ps::cff::dict::Entry::CharstringType(..)
        | crate::ps::cff::dict::Entry::StrokeWidth(..)
        | crate::ps::cff::dict::Entry::FdArrayOffset(..)
        | crate::ps::cff::dict::Entry::FdSelectOffset(..)
        | crate::ps::cff::dict::Entry::BlueValues(..)
        | crate::ps::cff::dict::Entry::OtherBlues(..)
        | crate::ps::cff::dict::Entry::FamilyBlues(..)
        | crate::ps::cff::dict::Entry::FamilyOtherBlues(..)
        | crate::ps::cff::dict::Entry::SubrsOffset(..)
        | crate::ps::cff::dict::Entry::VariationStoreIndex(..)
        | crate::ps::cff::dict::Entry::BlueScale(..)
        | crate::ps::cff::dict::Entry::BlueShift(..)
        | crate::ps::cff::dict::Entry::BlueFuzz(..)
        | crate::ps::cff::dict::Entry::LanguageGroup(..)
        | crate::ps::cff::dict::Entry::ExpansionFactor(..)
        | crate::ps::cff::dict::Entry::Encoding(..)
        | crate::ps::cff::dict::Entry::Charset(..)
        | crate::ps::cff::dict::Entry::UniqueId(..)
        | crate::ps::cff::dict::Entry::Xuid
        | crate::ps::cff::dict::Entry::SyntheticBase(..)
        | crate::ps::cff::dict::Entry::PostScript(..)
        | crate::ps::cff::dict::Entry::BaseFontName(..)
        | crate::ps::cff::dict::Entry::BaseFontBlend
        | crate::ps::cff::dict::Entry::Ros { .. }
        | crate::ps::cff::dict::Entry::CidFontVersion(..)
        | crate::ps::cff::dict::Entry::CidFontRevision(..)
        | crate::ps::cff::dict::Entry::CidFontType(..)
        | crate::ps::cff::dict::Entry::CidCount(..)
        | crate::ps::cff::dict::Entry::UidBase(..)
        | crate::ps::cff::dict::Entry::FontName(..)
        | crate::ps::cff::dict::Entry::StdHw(..)
        | crate::ps::cff::dict::Entry::StdVw(..)
        | crate::ps::cff::dict::Entry::DefaultWidthX(..)
        | crate::ps::cff::dict::Entry::NominalWidthX(..)
        | crate::ps::cff::dict::Entry::StemSnapH(..)
        | crate::ps::cff::dict::Entry::StemSnapV(..)
        | crate::ps::cff::dict::Entry::ForceBold(..)
        | crate::ps::cff::dict::Entry::InitialRandomSeed(..) => {
          return Err(crate::test_support::Failure::from(alloc::format!(
            "This was totally a font matrix"
          )));
        }
      }
    };

    assert_eq!(
      ({
        let Ok(value) = matrix.matrix.elements();
        value
      }),
      [
        Fixed::ONE,
        Fixed::ZERO,
        ({
          let Ok(explicit_value) = Fixed::from_f64(0.167007446289062);
          explicit_value
        }),
        Fixed::ONE,
        Fixed::ZERO,
        Fixed::ZERO,
      ]
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_degenerate_font_matrix() {
    let dict_data = [
      30u8, 0x0F, 30, 0x0F, 30, 0x0F, 30, 0x0F, 30, 0x0F, 30, 0x0F, 12, 7,
    ];

    assert!(entries(&dict_data, None).next().is_none());
  }
}
