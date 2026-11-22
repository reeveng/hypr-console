//! Unicode character map generated from glyph names.
//!
//! Building a character map depends on the `agl` feature.

#[cfg(feature = "agl")]
use super::agl;
use alloc::vec::Vec;
use types::GlyphId;

const VARIANT_BIT: u32 = 0x80000000;

#[derive(Clone, Default, Debug)]
pub struct Charmap {
  mapping: Vec<(u32, GlyphId)>,
}

impl Charmap {
  #[cfg(feature = "agl")]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Charmap::from_glyph_names` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn from_glyph_names<'a>(pairs: impl Iterator<Item = (GlyphId, &'a str)>) -> Self {
    #[derive(Copy, Clone, PartialEq)]
    enum State {
      Unchecked,
      Include,
      Exclude,
    }

    let mut extra_glyphs = [(State::Unchecked, GlyphId::NOTDEF); 10];
    let mut mapping = Vec::new();

    for (gid, name) in pairs {
      let extra = EXTRA_GLYPH_LIST.iter().zip(&mut extra_glyphs)
        .find(|((_, extra_name), _)| name == *extra_name);

      match extra.map(|(_, entry)| entry) {
        Some((state @ State::Unchecked, glyph)) => {
          *state = State::Include;
          *glyph = gid;
        }
        Some((State::Include | State::Exclude, _)) | None => {}
      }

      let mut ch = match agl::name_to_char(name)
        .map(|ch| <u32 as ::core::convert::From<::core::primitive::char>>::from(ch))
      {
        Some(ch) => ch,
        None => {
          continue;
        }
      };

      match ::core::primitive::bool::then_some(agl::split_variant(name).1.is_some(), ()) {
        ::core::option::Option::Some(()) => {
          ch |= VARIANT_BIT;
        }
        ::core::option::Option::None => {}
      }

      let matching_base = EXTRA_GLYPH_LIST.iter().zip(&mut extra_glyphs)
        .find(|((extra_ch, _), _)| ch == *extra_ch);

      match matching_base {
        Some((_, (state, _))) => *state = State::Exclude,
        None => {}
      }

      mapping.push((ch, gid));
    }

    for ((extra_ch, _), (state, gid)) in EXTRA_GLYPH_LIST.iter().zip(extra_glyphs) {
      match ::core::primitive::bool::then_some(state == State::Include, ()) {
        ::core::option::Option::Some(()) => {
          mapping.push((*extra_ch, gid));
        }
        ::core::option::Option::None => {}
      }
    }

    mapping.shrink_to_fit();
    mapping.sort_unstable_by(|a, b| {
      let a_base = a.0 & !VARIANT_BIT;
      let b_base = b.0 & !VARIANT_BIT;

      match ::core::primitive::bool::then_some(a_base == b_base, ()) {
        ::core::option::Option::Some(()) => a.0.cmp(&b.0),
        ::core::option::Option::None => a_base.cmp(&b_base),
      }
    });
    Self { mapping }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Charmap::map` method returns the optional value directly to callers that match its presence")
  )]
  pub fn map(&self, ch: impl Into<u32>) -> Option<GlyphId> {
    let ch_2 = <_ as ::core::convert::Into<::core::primitive::u32>>::into(ch);
    let mut min = 0u64;
    let Ok(mut max) = console_core_number_conversion::fitted::<_, u64>(self.mapping.len());
    let mut result = None;

    for _ in 0..u64::BITS {
      match min.cmp(&max) {
        core::cmp::Ordering::Less => {}
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => break,
      }

      let mid = min.midpoint(max);
      let Ok(index) = console_core_number_conversion::index(mid);
      let entry = self.mapping.get(index)?;

      match ::core::primitive::bool::then_some(entry.0 == ch_2, ()) {
        ::core::option::Option::Some(()) => {
          result = Some(entry.1);
          break;
        }
        ::core::option::Option::None => {}
      }

      let base_gid = entry.0 & !VARIANT_BIT;

      match ::core::primitive::bool::then_some(base_gid == ch_2, ()) {
        ::core::option::Option::Some(()) => {
          result = Some(entry.1);
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(base_gid < ch_2, ()) {
        ::core::option::Option::Some(()) => {
          min = mid.wrapping_add(1);
        }
        ::core::option::Option::None => {
          max = mid;
        }
      }
    }

    result
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Charmap::iter` method returns `Iter` directly in the existing parser interface")
  )]
  pub fn iter(&self) -> Iter<'_> {
    Iter(self.mapping.iter().copied())
  }
}

#[derive(Clone)]
pub struct Iter<'a>(core::iter::Copied<core::slice::Iter<'a, (u32, GlyphId)>>);

impl Iterator for Iter<'_> {
  type Item = (u32, GlyphId);

  fn next(&mut self) -> Option<Self::Item> {
    self.0.next()
  }
}

#[cfg(feature = "agl")]
#[rustfmt::skip]
const EXTRA_GLYPH_LIST: [(u32, &str); 10] = [

    (0x0394, "Delta"),
    (0x03A9, "Omega"),
    (0x2215, "fraction"),
    (0x00AD, "hyphen"),
    (0x02C9, "macron"),
    (0x03BC, "mu"),
    (0x2219, "periodcentered"),
    (0x00A0, "space"),

    (0x021A, "Tcommaaccent"),
    (0x021B, "tcommaaccent"),
];

#[cfg(test)]
#[cfg(feature = "agl")]
mod tests {
  use super::super::type1::Type1Font;
  use super::*;

  #[test]
  fn cmap() {
    let cases = [
      (1, "A", 'A'),
      (2, "uni0042", 'B'),
      (333, "C.alt", 'C'),
      (4, "D", 'D'),
      (51, "Lcedilla", 'Ļ'),
      (22, "Cdot", 'Ċ'),
      (8, "aacute", 'á'),
      (7, "union", '∪'),
    ];
    let cmap = Charmap::from_glyph_names(cases.iter().map(|&(number, name, _character)| {
      let Ok(glyph) = GlyphId::new(number);
      (glyph, name)
    }));

    for (number, _glyph_name, character) in cases {
      let Ok(glyph) = GlyphId::new(number);
      assert_eq!(cmap.map(character), Some(glyph));
    }
  }

  #[test]
  fn cmap_from_type1() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      Type1Font::new(font_test_data::type1::NOTO_SERIF_REGULAR_SUBSET_PFB),
      "fixture value",
    )?;
    let cmap = Charmap::from_glyph_names(font.glyph_names());

    let expected = [
      (
        <u32 as ::core::convert::From<::core::primitive::char>>::from('H'),
        1,
      ),
      (
        <u32 as ::core::convert::From<::core::primitive::char>>::from('H') | VARIANT_BIT,
        8,
      ),
      (
        <u32 as ::core::convert::From<::core::primitive::char>>::from('f'),
        2,
      ),
      (
        <u32 as ::core::convert::From<::core::primitive::char>>::from('i'),
        3,
      ),
      (
        <u32 as ::core::convert::From<::core::primitive::char>>::from('x'),
        4,
      ),
    ];
    let result = cmap
      .iter()
      .map(|(ch, gid)| {
        (
          ch,
          ({
            let Ok(value) = gid.to_u32();
            value
          }),
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(result, expected);
    assert_eq!(cmap.map('a'), None);

    for (ch, gid) in expected {
      match ::core::primitive::bool::then_some(ch & VARIANT_BIT != 0, ()) {
        ::core::option::Option::Some(()) => {
          assert_eq!(cmap.map(ch), None);
        }
        ::core::option::Option::None => {
          assert_eq!(
            cmap.map(ch),
            Some({
              let Ok(value) = GlyphId::new(gid);
              value
            }),
            "cmap failed for {ch} -> {gid}"
          );
        }
      }
    }

    Ok(())
  }

  #[test]
  fn extra_glyphs_override() {
    let cmap = Charmap::from_glyph_names(
      [(
        {
          let Ok(value) = GlyphId::new(1);
          value
        },
        "hyphen",
      )]
      .into_iter(),
    );
    assert_eq!(cmap.mapping.len(), 2);
    assert_eq!(
      cmap.map('\u{00AD}'),
      Some({
        let Ok(value) = GlyphId::new(1);
        value
      })
    );
    assert_eq!(
      cmap.map('\u{002D}'),
      Some({
        let Ok(value) = GlyphId::new(1);
        value
      })
    );
  }

  #[test]
  fn extra_glyphs_no_override() {
    let cmap = Charmap::from_glyph_names(
      [(
        {
          let Ok(value) = GlyphId::new(1);
          value
        },
        "softhyphen",
      )]
      .into_iter(),
    );
    assert_eq!(cmap.mapping.len(), 1);
    assert_eq!(
      cmap.map('\u{00AD}'),
      Some({
        let Ok(value) = GlyphId::new(1);
        value
      })
    );
  }
}
