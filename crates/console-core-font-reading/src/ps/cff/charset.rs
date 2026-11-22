//! CFF charset support.

use crate::ps::string::Sid;
use crate::{FontData, FontRead, GlyphId, ReadError};

#[doc(inline)]
pub use super::v1::{
  CharsetFormat0 as Format0, CharsetFormat1 as Format1, CharsetFormat2 as Format2,
  CharsetRange1 as Range1, CharsetRange2 as Range2, CustomCharset,
};

#[derive(Clone)]
pub struct Charset<'a> {
  kind: CharsetKind<'a>,
  num_glyphs: u32,
}

impl<'a> Charset<'a> {
  pub fn new(
    cff_data: FontData<'a>,
    charset_offset: u64,
    num_glyphs: u32,
  ) -> Result<Self, ReadError> {
    let kind = match charset_offset {
      0 => CharsetKind::IsoAdobe,
      1 => CharsetKind::Expert,
      2 => CharsetKind::ExpertSubset,
      _ => {
        let Ok(value_1) = cff_data.split_off(charset_offset);

        let data = value_1.ok_or(ReadError::OutOfBounds)?;
        let read = CustomCharset::read(data)?;

        CharsetKind::Custom(read)
      }
    };
    Ok(Self { kind, num_glyphs })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Charset::kind` method returns a borrow tied to the source lifetime directly")
  )]
  pub fn kind(&self) -> &CharsetKind<'a> {
    &self.kind
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Charset::num_glyphs` method returns `u32` directly in the existing parser interface")
  )]
  pub fn num_glyphs(&self) -> u32 {
    self.num_glyphs
  }

  pub fn string_id(&self, glyph_id: GlyphId) -> Result<Sid, ReadError> {
    let Ok(gid) = glyph_id.to_u32();

    match ::core::primitive::bool::then_some(gid >= self.num_glyphs, ()) {
      ::core::option::Option::Some(()) => {
        return Err(ReadError::OutOfBounds);
      }
      ::core::option::Option::None => {}
    }

    match &self.kind {
      CharsetKind::IsoAdobe => match ::core::primitive::bool::then_some(gid <= 228, ()) {
        ::core::option::Option::Some(()) => Ok(Sid::new({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(gid);
          migrated_value
        })),
        ::core::option::Option::None => Err(ReadError::OutOfBounds),
      },
      CharsetKind::Expert => {
        let Ok(value_14) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(gid);
          migrated_value
        });

        EXPERT_CHARSET
          .get(value_14)
          .copied()
          .ok_or(ReadError::OutOfBounds)
          .map(Sid::new)
      }
      CharsetKind::ExpertSubset => {
        let Ok(value_19) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(gid);
          migrated_value
        });

        EXPERT_SUBSET_CHARSET
          .get(value_19)
          .copied()
          .ok_or(ReadError::OutOfBounds)
          .map(Sid::new)
      }
      CharsetKind::Custom(custom) => match custom {
        CustomCharset::Format0(fmt) => fmt.string_id(glyph_id),
        CustomCharset::Format1(fmt) => fmt.string_id(glyph_id),
        CustomCharset::Format2(fmt) => fmt.string_id(glyph_id),
      },
    }
  }

  pub fn glyph_id(&self, string_id: Sid) -> Result<GlyphId, ReadError> {
    let Ok(value_13) = string_id.to_u16();

    let sid = value_13;

    match &self.kind {
      CharsetKind::IsoAdobe => match ::core::primitive::bool::then_some(sid <= 228, ()) {
        ::core::option::Option::Some(()) => Ok(GlyphId::from(sid)),
        ::core::option::Option::None => Err(ReadError::OutOfBounds),
      },
      CharsetKind::Expert => EXPERT_CHARSET
        .iter()
        .position(|n| *n == sid)
        .map(|pos| {
          let Ok(value_17) = GlyphId::new({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(pos);
            migrated_value
          });

          value_17
        })
        .ok_or(ReadError::OutOfBounds),
      CharsetKind::ExpertSubset => EXPERT_SUBSET_CHARSET
        .iter()
        .position(|n| *n == sid)
        .map(|pos| {
          let Ok(value_18) = GlyphId::new({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(pos);
            migrated_value
          });

          value_18
        })
        .ok_or(ReadError::OutOfBounds),
      CharsetKind::Custom(custom) => match custom {
        CustomCharset::Format0(fmt) => fmt.glyph_id(string_id),
        CustomCharset::Format1(fmt) => fmt.glyph_id(string_id),
        CustomCharset::Format2(fmt) => fmt.glyph_id(string_id),
      },
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Charset::iter` method returns `Iter` directly in the existing parser interface")
  )]
  pub fn iter(&self) -> Iter<'a> {
    match &self.kind {
      CharsetKind::IsoAdobe
      | CharsetKind::Expert
      | CharsetKind::ExpertSubset
      | CharsetKind::Custom(CustomCharset::Format0(_)) => Iter(IterKind::Simple(self.clone(), 0)),
      CharsetKind::Custom(CustomCharset::Format1(custom)) => {
        let Ok(ranges) = custom.ranges();

        let Ok(range_iter) = RangeIter::new(ranges, self.num_glyphs);

        Iter(IterKind::Custom1(range_iter))
      }
      CharsetKind::Custom(CustomCharset::Format2(custom)) => {
        let Ok(ranges) = custom.ranges();

        let Ok(range_iter) = RangeIter::new(ranges, self.num_glyphs);

        Iter(IterKind::Custom2(range_iter))
      }
    }
  }
}

#[derive(Clone)]
pub enum CharsetKind<'a> {
  IsoAdobe,
  Expert,
  ExpertSubset,
  Custom(CustomCharset<'a>),
}

impl Format0<'_> {
  fn string_id(&self, glyph_id: GlyphId) -> Result<Sid, ReadError> {
    let Ok(value_3) = glyph_id.to_u32();

    let gid = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_3);
      migrated_value
    };

    match ::core::primitive::bool::then_some(gid == 0, ()) {
      ::core::option::Option::Some(()) => Ok(Sid::new(0)),
      ::core::option::Option::None => {
        let Ok(value_25) =
          console_core_number_conversion::index(::core::primitive::u64::wrapping_sub(gid, 1));

        let Ok(glyph) = self.glyph();

        glyph
          .get(value_25)
          .map(|id| {
            let Ok(value_4) = id.get();

            Sid::new(value_4)
          })
          .ok_or(ReadError::OutOfBounds)
      }
    }
  }

  fn glyph_id(&self, string_id: Sid) -> Result<GlyphId, ReadError> {
    let Ok(value_20) = string_id.to_u16();

    match ::core::primitive::bool::then_some(value_20 == 0, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(GlyphId::NOTDEF);
      }
      ::core::option::Option::None => {}
    }

    let Ok(glyph) = self.glyph();

    glyph
      .iter()
      .position(|n| {
        let Ok(value_15) = n.get();

        let Ok(value_21) = string_id.to_u16();

        value_15 == value_21
      })
      .map(|n| {
        GlyphId::from(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(n);
            migrated_value
          })
          .saturating_add(1),
        )
      })
      .ok_or(ReadError::OutOfBounds)
  }
}

impl Format1<'_> {
  fn string_id(&self, glyph_id: GlyphId) -> Result<Sid, ReadError> {
    let Ok(ranges) = self.ranges();

    string_id_from_ranges(ranges, glyph_id)
  }

  fn glyph_id(&self, string_id: Sid) -> Result<GlyphId, ReadError> {
    let Ok(ranges) = self.ranges();

    glyph_id_from_ranges(ranges, string_id)
  }
}

impl Format2<'_> {
  fn string_id(&self, glyph_id: GlyphId) -> Result<Sid, ReadError> {
    let Ok(ranges) = self.ranges();

    string_id_from_ranges(ranges, glyph_id)
  }

  fn glyph_id(&self, string_id: Sid) -> Result<GlyphId, ReadError> {
    let Ok(ranges) = self.ranges();

    glyph_id_from_ranges(ranges, string_id)
  }
}

fn string_id_from_ranges<T: CharsetRange>(
  ranges: &[T],
  glyph_id: GlyphId,
) -> Result<Sid, ReadError> {
  let Ok(mut gid) = glyph_id.to_u32();

  match ::core::primitive::bool::then_some(gid == 0, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(Sid::new(0));
    }
    ::core::option::Option::None => {}
  }

  gid = ::core::primitive::u32::wrapping_sub(gid, 1);
  let mut end = 0u32;

  for range in ranges {
    let next_end = end
      .checked_add(::core::primitive::u32::wrapping_add(range.n_left(), 1))
      .ok_or(ReadError::OutOfBounds)?;

    match ::core::primitive::bool::then_some(gid < next_end, ()) {
      ::core::option::Option::Some(()) => {
        return ::core::primitive::u32::wrapping_sub(gid, end)
          .checked_add(range.first())
          .and_then(|sid| match sid.try_into() {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
          })
          .ok_or(ReadError::OutOfBounds)
          .map(Sid::new);
      }
      ::core::option::Option::None => {}
    }

    end = next_end;
  }

  Err(ReadError::OutOfBounds)
}

fn glyph_id_from_ranges<T: CharsetRange>(
  ranges: &[T],
  string_id: Sid,
) -> Result<GlyphId, ReadError> {
  let Ok(value_22) = string_id.to_u16();

  let sid = {
    let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(value_22);
    migrated_value
  };

  match ::core::primitive::bool::then_some(sid == 0, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(GlyphId::NOTDEF);
    }
    ::core::option::Option::None => {}
  }

  let mut gid = 1u32;

  for range in ranges {
    let first = range.first();
    let n_left = range.n_left();

    match ::core::primitive::bool::then_some(first <= sid && sid <= ::core::primitive::u32::wrapping_add(first, n_left), ()) {
      ::core::option::Option::Some(()) => {
        match (::core::primitive::u32::wrapping_sub(sid, first), &mut gid) {
          (explicit_arithmetic_amount, explicit_arithmetic_place) => {
            *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
              *explicit_arithmetic_place,
              explicit_arithmetic_amount,
            )
          }
        };

        let Ok(value_5) = GlyphId::new(gid);

        return Ok(value_5);
      }
      ::core::option::Option::None => {}
    }

    match (::core::primitive::u32::wrapping_add(n_left, 1), &mut gid) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  Err(ReadError::OutOfBounds)
}

trait CharsetRange {
  fn first(&self) -> u32;

  fn n_left(&self) -> u32;
}

impl CharsetRange for Range1 {
  fn first(&self) -> u32 {
    let Ok(value_6) = self.first.get();

    {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(value_6);
      migrated_value
    }
  }

  fn n_left(&self) -> u32 {
    {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(self.n_left);
      migrated_value
    }
  }
}

impl CharsetRange for Range2 {
  fn first(&self) -> u32 {
    let Ok(value_7) = self.first.get();

    {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(value_7);
      migrated_value
    }
  }

  fn n_left(&self) -> u32 {
    let Ok(value_8) = self.n_left.get();

    {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(value_8);
      migrated_value
    }
  }
}

#[derive(Clone)]
pub struct Iter<'a>(IterKind<'a>);

impl Iterator for Iter<'_> {
  type Item = (GlyphId, Sid);

  fn next(&mut self) -> Option<Self::Item> {
    match &mut self.0 {
      IterKind::Simple(charset, cur) => {
        let Ok(value_16) = GlyphId::new(*cur);

        let gid = value_16;
        let value_9 = gid;

        let sid = match charset.string_id(value_9) {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }?;
        let checked_value = cur.checked_add(1)?;
        *cur = checked_value;
        let value_10 = gid;

        Some((value_10, sid))
      }
      IterKind::Custom1(custom) => {
          let Ok(next) = custom.next();

          next
      }
      IterKind::Custom2(custom) => {
          let Ok(next) = custom.next();

          next
      }
    }
  }
}

#[derive(Clone)]
enum IterKind<'a> {
  Simple(Charset<'a>, u32),
  Custom1(RangeIter<'a, Range1>),
  Custom2(RangeIter<'a, Range2>),
}

#[derive(Clone)]
struct RangeIter<'a, T> {
  ranges: std::slice::Iter<'a, T>,
  num_glyphs: u32,
  gid: u32,
  first: u32,
  end: u32,
  prev_end: u32,
}

impl<'a, T> RangeIter<'a, T>
where
  T: CharsetRange,
{
  #[must_use]
  fn new(ranges: &'a [T], num_glyphs: u32) -> core::result::Result<Self, console_core_never::Never> {
      let mut ranges_2 = ranges.iter();
      let Ok(range) = next_range(&mut ranges_2);
      let (first, end) = match range {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => (0, 0),
      };

      Ok(Self {
        ranges: ranges_2,
        num_glyphs,
        gid: 0,
        first,
        end,
        prev_end: 0,
      })
  }

  fn next(&mut self) -> core::result::Result<Option<(GlyphId, Sid)>, console_core_never::Never> {
      match ::core::primitive::bool::then_some(self.gid >= self.num_glyphs, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(None);
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(self.gid == 0, ()) {
        ::core::option::Option::Some(()) => {
          match (1, &mut self.gid) {
            (explicit_arithmetic_amount, explicit_arithmetic_place) => {
              *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
                *explicit_arithmetic_place,
                explicit_arithmetic_amount,
              )
            }
          };

          let Ok(value_11) = GlyphId::new(0);

          return Ok(Some((value_11, Sid::new(0))));
        }
        ::core::option::Option::None => {}
      }

      let gid = ::core::primitive::u32::wrapping_sub(self.gid, 1);
      let checked_value_2 = match self.gid.checked_add(1) { Some(explicit_value) => explicit_value, None => return Ok(None) };
      self.gid = checked_value_2;

      match gid.cmp(&self.end) {
        core::cmp::Ordering::Less => {}
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
          for range in self.ranges.by_ref() {
            self.prev_end = self.end;
            self.first = range.first();
            let length = range.n_left().wrapping_add(1);
            self.end = match self.prev_end.checked_add(length) {
              Some(end) => end,
              None => return Ok(None),
            };

            match gid.cmp(&self.end) {
              core::cmp::Ordering::Less => break,
              core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {}
            }
          }
        }
      }

      match gid.cmp(&self.end) {
        core::cmp::Ordering::Less => {}
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => return Ok(None),
      }

      let checked_value = match self.first.checked_add({
        let checked_value_4 = match gid.checked_sub(self.prev_end) { Some(explicit_value) => explicit_value, None => return Ok(None) };
        checked_value_4
      }) { Some(explicit_value) => explicit_value, None => return Ok(None) };
      let sid = match match checked_value.try_into() {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
      } { Some(explicit_value) => explicit_value, None => return Ok(None) };
      let Ok(value_12) = GlyphId::new(::core::primitive::u32::wrapping_add(gid, 1));

      Ok(Some((value_12, Sid::new(sid))))
  }
}

fn next_range<T: CharsetRange>(ranges: &mut std::slice::Iter<T>) -> core::result::Result<Option<(u32, u32)>, console_core_never::Never> {
    Ok(ranges.next().map(|range| {
      (
        range.first(),
        ::core::primitive::u32::wrapping_add(range.n_left(), 1),
      )
    }))
}

#[rustfmt::skip]
const EXPERT_CHARSET: &[u16] = &[
      0,    1,  229,  230,  231,  232,  233,  234,  235,  236,  237,  238,   13,   14,   15,   99,
    239,  240,  241,  242,  243,  244,  245,  246,  247,  248,   27,   28,  249,  250,  251,  252,
    253,  254,  255,  256,  257,  258,  259,  260,  261,  262,  263,  264,  265,  266,  109,  110,
    267,  268,  269,  270,  271,  272,  273,  274,  275,  276,  277,  278,  279,  280,  281,  282,
    283,  284,  285,  286,  287,  288,  289,  290,  291,  292,  293,  294,  295,  296,  297,  298,
    299,  300,  301,  302,  303,  304,  305,  306,  307,  308,  309,  310,  311,  312,  313,  314,
    315,  316,  317,  318,  158,  155,  163,  319,  320,  321,  322,  323,  324,  325,  326,  150,
    164,  169,  327,  328,  329,  330,  331,  332,  333,  334,  335,  336,  337,  338,  339,  340,
    341,  342,  343,  344,  345,  346,  347,  348,  349,  350,  351,  352,  353,  354,  355,  356,
    357,  358,  359,  360,  361,  362,  363,  364,  365,  366,  367,  368,  369,  370,  371,  372,
    373,  374,  375,  376,  377,  378,
];

#[rustfmt::skip]
const EXPERT_SUBSET_CHARSET: &[u16] = &[
      0,    1,  231,  232,  235,  236,  237,  238,   13,   14,   15,   99,  239,  240,  241,  242,
    243,  244,  245,  246,  247,  248,   27,   28,  249,  250,  251,  253,  254,  255,  256,  257,
    258,  259,  260,  261,  262,  263,  264,  265,  266,  109,  110,  267,  268,  269,  270,  272,
    300,  301,  302,  305,  314,  315,  158,  155,  163,  320,  321,  322,  323,  324,  325,  326,
    150,  164,  169,  327,  328,  329,  330,  331,  332,  333,  334,  335,  336,  337,  338,  339,
    340,  341,  342,  343,  344,  345,  346
];

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn iso_adobe_charset() -> Result<(), crate::test_support::Failure> {
    let charset_offset = 0;
    let num_glyphs = 64;

    let expected = |gid: GlyphId| {
      Some({
        let Ok(value) = gid.to_u32();
        value
      })
    };
    test_simple_mapping(charset_offset, num_glyphs, expected)
  }

  #[test]
  fn expert_charset() -> Result<(), crate::test_support::Failure> {
    let charset_offset = 1;
    let num_glyphs = 64;

    let expected = |gid: GlyphId| {
      EXPERT_CHARSET
        .get({
          let Ok(explicit_index) = console_core_number_conversion::index({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>({
              let Ok(explicit_value) = gid.to_u32();
              explicit_value
            });
            migrated_value
          });
          explicit_index
        })
        .map(|id| {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(*id);
          migrated_value
        })
    };
    test_simple_mapping(charset_offset, num_glyphs, expected)
  }

  #[test]
  fn expert_subset_charset() -> Result<(), crate::test_support::Failure> {
    let charset_offset = 2;
    let num_glyphs = 64;

    let expected = |gid: GlyphId| {
      EXPERT_SUBSET_CHARSET
        .get({
          let Ok(explicit_index) = console_core_number_conversion::index({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>({
              let Ok(explicit_value) = gid.to_u32();
              explicit_value
            });
            migrated_value
          });
          explicit_index
        })
        .map(|id| {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(*id);
          migrated_value
        })
    };
    test_simple_mapping(charset_offset, num_glyphs, expected)
  }

  fn test_simple_mapping(
    charset_offset: u64,
    num_glyphs: u32,
    expected: impl Fn(GlyphId) -> Option<u32>,
  ) -> Result<(), crate::test_support::Failure> {
    let Ok(data) = FontData::new(&[]);
    let charset = Charset::new(data, charset_offset, num_glyphs)?;

    for id in 0..num_glyphs {
      let Ok(glyph) = GlyphId::new(id);
      let raw_sid = crate::test_support::fixture_value(expected(glyph), "expected CFF string ID")?;
      let sid = charset.string_id(glyph)?;
      let Ok(actual) = sid.to_u16();
      assert_eq!(u32::from(actual), raw_sid);
      let expected_sid = u16::try_from(raw_sid)?;
      let round_trip = charset.glyph_id(Sid::new(expected_sid))?;
      assert_eq!(round_trip, glyph);
    }

    for id in num_glyphs..u32::from(u16::MAX) {
      let Ok(glyph) = GlyphId::new(id);
      assert!(matches!(charset.string_id(glyph), Err(_missing_glyph)));
    }

    Ok(())
  }

  #[test]
  fn custom_mapping_format0() -> Result<(), crate::test_support::Failure> {
    let mut buf = font_test_data::bebuffer::BeBuffer::default();
    let glyph_count = 6;
    buf.extend([0u8; 4]);
    buf.extend([0u8]);
    buf.extend([2u16, 4, 6, 8, 10]);
    let Ok(data) = FontData::new(&buf);
    let charset = Charset::new(data, 4, glyph_count)?;

    for number in 0..glyph_count {
      let Ok(glyph) = GlyphId::new(number);
      let sid = charset.string_id(glyph)?;
      let Ok(id) = sid.to_u16();
      assert_eq!(u32::from(id), number.wrapping_mul(2));
    }

    for (glyph, sid) in charset.iter() {
      let Ok(number) = glyph.to_u32();
      let Ok(id) = sid.to_u16();
      assert_eq!(u32::from(id), number.wrapping_mul(2));
    }

    let Ok(iterated) = console_core_number_conversion::fitted::<_, u32>(charset.iter().count());
    assert_eq!(iterated, glyph_count);

    for number in glyph_count..u32::from(u16::MAX) {
      let Ok(glyph) = GlyphId::new(number);
      assert!(matches!(charset.string_id(glyph), Err(_missing_glyph)));
    }

    Ok(())
  }

  #[test]
  fn custom_mapping_format1() -> Result<(), crate::test_support::Failure> {
    let mut buf = font_test_data::bebuffer::BeBuffer::default();
    let num_glyphs = 7;

    buf.extend([0u8; 4]);

    buf.extend([1u8]);

    {
      buf.extend([8u16]);
      buf.extend([2u8]);
    };

    {
      buf.extend([1200u16]);
      buf.extend([0u8]);
    };

    {
      buf.extend([20u16]);
      buf.extend([1u8]);
    };

    let expected_sids = [0, 8, 9, 10, 1200, 20, 21];
    test_range_mapping(&buf, num_glyphs, &expected_sids)
  }

  #[test]
  fn custom_mapping_format2() -> Result<(), crate::test_support::Failure> {
    let mut buf = font_test_data::bebuffer::BeBuffer::default();

    buf.extend([0u8; 4]);

    buf.extend([2u8]);

    {
      buf.extend([8u16]);
      buf.extend([2u16]);
    };

    {
      buf.extend([1200u16]);
      buf.extend([0u16]);
    };

    {
      buf.extend([20u16]);
      buf.extend([800u16]);
    };

    let mut expected_sids = vec![0, 8, 9, 10, 1200];

    for i in 0..=800 {
      expected_sids.push(::core::primitive::u32::wrapping_add(i, 20));
    }

    let num_glyphs = {
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u32>(expected_sids.len());
      migrated_value
    };
    test_range_mapping(&buf, num_glyphs, &expected_sids)
  }

  fn test_range_mapping(
    data: &[u8],
    num_glyphs: u32,
    expected_sids: &[u32],
  ) -> Result<(), crate::test_support::Failure> {
    let Ok(table_data) = FontData::new(data);
    let charset = Charset::new(table_data, 4, num_glyphs)?;
    let mut expected_pairs = Vec::with_capacity(expected_sids.len());

    for (index, &expected_sid) in expected_sids.iter().enumerate() {
      let id = u32::try_from(index)?;
      let Ok(glyph) = GlyphId::new(id);
      let actual = charset.string_id(glyph)?;
      let Ok(raw_sid) = actual.to_u16();
      assert_eq!(u32::from(raw_sid), expected_sid);
      let encoded_sid = u16::try_from(expected_sid)?;
      expected_pairs.push((glyph, Sid::new(encoded_sid)));
    }

    assert!(charset.iter().eq(expected_pairs.iter().copied()));
    let expected_count = ::console_core_number_conversion::checked_index(num_glyphs)?;
    assert_eq!(charset.iter().count(), expected_count);

    for id in num_glyphs..u32::from(u16::MAX) {
      let Ok(glyph) = GlyphId::new(id);
      assert!(matches!(charset.string_id(glyph), Err(_missing_glyph)));
    }

    for (glyph, sid) in expected_pairs {
      assert_eq!(charset.glyph_id(sid), Ok(glyph));
    }

    Ok(())
  }
}
