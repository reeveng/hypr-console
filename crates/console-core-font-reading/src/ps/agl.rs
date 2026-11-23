//! The Adobe glyph list.
//!
//! Handles mapping between glyph names and characters.
//! Both directions traverse a static trie with two-byte child offsets.

use console_core_never::Never;
use console_core_number_conversion::{index, indices_to};
use core::ops::Range;

include!("../../data/generated/generated_agl.rs");

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the published `split_variant` function returns its tuple directly for existing callers to destructure"
  )
)]
pub fn split_variant(name: &str) -> (&str, Option<&str>) {
  match name.split_once('.') {
    Some(("", _)) | None => (name, None),
    Some((base, suffix)) => (base, Some(suffix)),
  }
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the published `name_to_char` function returns the optional value directly to callers that match its presence"
  )
)]
pub fn name_to_char(name: &str) -> Option<char> {
  let Ok(name_2) = trim_variant(name);
  ({
    let Ok(explicit_value) = lookup_char(name_2);
    explicit_value
  })
  .or_else(|| {
    let Ok(explicit_value) = u_to_unicode(name_2);
    explicit_value
  })
  .or_else(|| {
    ({
      let Ok(explicit_value) = uni_to_unicode(name_2, 1);
      explicit_value
    })
    .and_then(|mut iter| iter.next())
  })
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the published `name_to_chars` function returns its iterator or trait value directly for existing callers to compose"
  )
)]
pub fn name_to_chars(name: &str) -> impl Iterator<Item = char> + '_ {
  let Ok(name_2) = trim_variant(name);
  name_2.split('_').flat_map(|name_3| {
    match {
      let Ok(explicit_value) = lookup_multi_char(name_3);
      explicit_value
    } {
      Some(multi) => CharIter::Multi(multi, 0),
      None => match ({
        let Ok(explicit_value) = lookup_char(name_3);
        explicit_value
      })
      .or_else(|| {
        let Ok(explicit_value) = u_to_unicode(name_3);
        explicit_value
      }) {
        Some(single) => CharIter::Single(single),
        None => match {
          let Ok(explicit_value) = uni_to_unicode(name_3, { u64::MAX });
          explicit_value
        } {
          Some(iter) => iter,
          None => CharIter::None,
        },
      },
    }
  })
}

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the published `char_to_name` function returns the optional value directly to callers that match its presence"
  )
)]
pub fn char_to_name(ch: impl Into<u32>, dst: &mut [u8]) -> Option<&'_ str> {
  char_to_nth_name(ch, 0, dst)
}

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the glyph-list API uses absence for an unknown name, alias number, or insufficient caller buffer"
  )
)]
pub fn char_to_nth_name(ch: impl Into<u32>, n: u64, dst: &mut [u8]) -> Option<&'_ str> {
  let character = ::core::convert::Into::<u32>::into(ch);
  let Ok(root) = Node::new_root(&AGL_TRIE);
  let root_2 = root?;
  let mut frames: [Option<GlyphNameFrame>; MAX_NAME_LEN] = [const { None }; MAX_NAME_LEN];
  let first_frame = frames.first_mut()?;
  let Ok(root_frame) = GlyphNameFrame::new(root_2, 0);
  *first_frame = Some(root_frame);

  let mut depth = 0u16;
  let mut found = 0u64;

  for _step in AGL_TRIE.iter() {
    let Ok(frame_position) = index(depth);
    let frame = frames.get_mut(frame_position).and_then(Option::as_mut)?;
    let Ok(pending_child) = frame.next_child();
    let child = match pending_child {
      Some(node) => node,
      None => {
        match depth.checked_sub(1) {
          Some(parent_depth) => depth = parent_depth,
          None => break,
        }

        continue;
      }
    };
    let Ok(appended) = append_glyph_name(dst, frame.prefix_length, &child);
    let prefix_length = match appended {
      Some(length) => length,
      None => continue,
    };
    let Ok(value) = child.value();

    match ::core::primitive::bool::then_some(value.map(u32::from) == Some(character), ()) {
      ::core::option::Option::Some(()) => match ::core::cmp::Ord::cmp(
        &::core::convert::identity(found),
        &::core::convert::identity(n),
      ) {
        ::core::cmp::Ordering::Equal => {
          let Ok(name) = completed_glyph_name(dst, prefix_length);

          return name;
        }
        ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Greater => {
          found = found.wrapping_add(1)
        }
      },
      ::core::option::Option::None => {}
    }

    depth = depth.wrapping_add(1);
    let Ok(child_position) = index(depth);
    let child_frame = frames.get_mut(child_position)?;
    let Ok(pending) = GlyphNameFrame::new(child, prefix_length);
    *child_frame = Some(pending);
  }

  None
}

struct GlyphNameFrame {
  node: Node<'static>,
  children: Range<u8>,
  prefix_length: u64,
}

impl GlyphNameFrame {
  fn new(node: Node<'static>, prefix_length: u64) -> Result<Self, Never> {
    let children = 0..node.num_children;

    Ok(Self {
      node,
      children,
      prefix_length,
    })
  }

  fn next_child(&mut self) -> Result<Option<Node<'static>>, Never> {
    for child in self.children.by_ref() {
      let Ok(offset) = self.node.child_offset(u64::from(child));
      let offset_3 = match offset {
        Some(offset_2) => offset_2,
        None => continue,
      };
      let Ok(node) = Node::new_child(self.node.data, offset_3);

      match node {
        Some(node_2) => return Ok(Some(node_2)),
        None => continue,
      }
    }

    Ok(None)
  }
}

fn append_glyph_name(
  dst: &mut [u8],
  prefix_length: u64,
  child: &Node<'_>,
) -> Result<Option<u64>, Never> {
  let Ok(bytes) = child.name_chars();
  let mut length = prefix_length;

  for byte in bytes {
    let Ok(position) = index(length);
    let slot = match dst.get_mut(position) {
      Some(slot) => slot,
      None => return Ok(None),
    };
    *slot = byte;
    length = length.wrapping_add(1);
  }

  Ok(Some(length))
}

fn completed_glyph_name(dst: &mut [u8], length: u64) -> Result<Option<&str>, Never> {
  let Ok(position) = index(length);
  let terminator = match dst.get_mut(position) {
    Some(terminator) => terminator,
    None => return Ok(None),
  };
  *terminator = 0;
  let Ok(range) = indices_to(..length);
  let bytes = match dst.get(range) {
    Some(bytes) => bytes,
    None => return Ok(None),
  };

  Ok(match core::str::from_utf8(bytes) {
    Ok(name) => Some(name),
    Err(_invalid_glyph_name) => None,
  })
}

#[must_use]
fn trim_variant(name: &str) -> core::result::Result<&str, console_core_never::Never> {
  Ok(match name.split_once('.') {
    Some((base, _suffix)) => base,
    None => name,
  })
}

#[must_use]
fn lookup_char(name: &str) -> Result<Option<char>, Never> {
  let mut suffix = name.as_bytes();
  let mut current = match Node::new_root(&AGL_TRIE) {
    Ok(Some(node)) => node,
    Ok(None) => return Ok(None),
  };

  for _ in 0..MAX_NAME_LEN {
    let first = match suffix.first() {
      Some(first) => *first,
      None => return Ok(None),
    };
    let child = match current.child_for_char(first) {
      Ok(Some(child)) => child,
      Ok(None) => return Ok(None),
    };
    let Ok(name_length) = child.num_name_chars();
    let Ok(split) = index(name_length);
    let (prefix, rest) = match suffix.split_at_checked(split) {
      Some(parts) => parts,
      None => return Ok(None),
    };
    let Ok(child_name) = child.name_chars();

    match crate::Presence::from(child_name.eq(prefix.iter().copied())) {
      crate::Presence::Present => suffix = rest,
      crate::Presence::Absent => {}
    }

    match suffix {
      [] => return child.value(),
      [_, ..] => current = child,
    }
  }

  Ok(None)
}

#[must_use]
fn lookup_multi_char(
  name: &str,
) -> core::result::Result<Option<&'static [u16]>, console_core_never::Never> {
  Ok(
    match AGL_MULTIS.binary_search_by_key(&name, |e| e.0) {
      ::core::result::Result::Ok(value___) => ::core::option::Option::Some(value___),
      ::core::result::Result::Err(_insertion_position) => ::core::option::Option::None,
    }
    .and_then(|idx| AGL_MULTIS.get(idx))
    .map(|e| e.1),
  )
}

#[must_use]
fn uni_to_unicode(
  component: &str,
  max_values: u64,
) -> core::result::Result<Option<CharIter<'_>>, console_core_never::Never> {
  let digits = match component.strip_prefix("uni") {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };

  match ::core::primitive::bool::then_some(
    !digits
      .bytes()
      .all(|byte| matches!(uppercase_hex(byte), Ok(crate::Presence::Present)))
      || digits.len().wrapping_rem(4) != 0,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let n_values = {
    let Ok(explicit_index) =
      console_core_number_conversion::fitted::<_, u64>(digits.len().wrapping_div(4));
    explicit_index
  };

  Ok(
    (n_values
      <= ({
        let Ok(explicit_index) = console_core_number_conversion::index(max_values);

        {
          let Ok(explicit_index_2) =
            console_core_number_conversion::fitted::<_, u64>(explicit_index);
          explicit_index_2
        }
      }))
    .then_some(CharIter::Uni(digits, 0, {
      let Ok(count) = console_core_number_conversion::fitted::<_, u64>(n_values);
      count
    })),
  )
}

#[must_use]
fn uppercase_hex(byte: u8) -> Result<crate::Presence, Never> {
  Ok(match byte {
    b'0'..=b'9' | b'A'..=b'F' => crate::Presence::Present,
    _other_byte => crate::Presence::Absent,
  })
}

#[must_use]
fn u_to_unicode(
  component: &str,
) -> core::result::Result<Option<char>, console_core_never::Never> {
  let value = match component.strip_prefix('u') {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };

  match ::core::primitive::bool::then_some(
    !value
      .bytes()
      .all(|byte| matches!(uppercase_hex(byte), Ok(crate::Presence::Present)))
      || !(4..=6).contains(&value.len()),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let uv = match match u32::from_str_radix(value, 16) {
    ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
    ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None,
  } {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };

  Ok(
    match ::core::primitive::bool::then_some(
      (0..=0xD7FF).contains(&uv) || (0xE000..=0x10FFFF).contains(&uv),
      (),
    ) {
      ::core::option::Option::Some(()) => char::from_u32(uv),
      ::core::option::Option::None => None,
    },
  )
}

enum CharIter<'a> {
  Multi(&'a [u16], u64),
  Single(char),

  Uni(&'a str, u64, u64),
  None,
}

impl Iterator for CharIter<'_> {
  type Item = char;

  fn next(&mut self) -> Option<Self::Item> {
    match self {
      Self::Multi(multi, idx) => {
        let cp = {
          let checked_value_2 = multi.get({
            let Ok(explicit_index) = console_core_number_conversion::index(*idx);
            explicit_index
          })?;
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u32>(*checked_value_2);
          migrated_value
        };

        match (1, &mut *idx) {
          (explicit_arithmetic_amount, explicit_arithmetic_place) => {
            *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
              *explicit_arithmetic_place,
              explicit_arithmetic_amount,
            )
          }
        };

        char::from_u32(cp)
      }
      Self::Single(cp) => {
        let cp_2 = *cp;
        *self = Self::None;
        Some(cp_2)
      }
      Self::Uni(src, cur, count) => {
        let i = *cur;

        match ::core::primitive::bool::then_some(i >= *count, ()) {
          ::core::option::Option::Some(()) => {
            return None;
          }
          ::core::option::Option::None => {}
        }

        match (1, &mut *cur) {
          (explicit_arithmetic_amount, explicit_arithmetic_place) => {
            *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
              *explicit_arithmetic_place,
              explicit_arithmetic_amount,
            )
          }
        };

        let checked_value_3 = src.get({
          let Ok(range) = console_core_number_conversion::indices(
            i.saturating_mul(4)..i.saturating_mul(4).saturating_add(4),
          );
          range
        })?;
        let checked_value = match u32::from_str_radix(checked_value_3, 16) {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None,
        }?;
        char::from_u32(checked_value)
      }
      Self::None => None,
    }
  }
}

#[derive(Clone)]
struct Node<'a> {
  data: &'a [u8],
  name_chars_range: Range<u64>,
  value: Option<char>,
  num_children: u8,
  children_start: u64,
}

impl<'a> Node<'a> {
  #[must_use]
  fn new_root(data: &'a [u8]) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let num_children = match ::core::option::Option::<&_>::copied(data.get(1)) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok(Some(Self {
      data,
      name_chars_range: 0..0,
      value: None,
      num_children,
      children_start: 2,
    }))
  }

  #[must_use]
  fn new_child(
    data: &'a [u8],
    offset: u64,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let chars_start = offset;
    let mut chars_end = offset;

    for &ch in match data.get({
      let Ok(explicit_index) = console_core_number_conversion::indices_from(offset..);
      explicit_index
    }) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    } {
      chars_end = ::core::primitive::u64::wrapping_add(chars_end, 1);

      match ::core::primitive::bool::then_some(ch & 0x80 == 0, ()) {
        ::core::option::Option::Some(()) => {
          break;
        }
        ::core::option::Option::None => {}
      }
    }

    let mut offset_2 = chars_end;

    let control = match ::core::option::Option::<&_>::copied(data.get({
      let Ok(explicit_index) = console_core_number_conversion::index(offset_2);
      explicit_index
    })) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let num_children = control & 0x7F;
    let value = match ::core::primitive::bool::then_some(control & 0x80 != 0, ()) {
      ::core::option::Option::Some(()) => {
        let ch_start = ::core::primitive::u64::wrapping_add(offset_2, 1);
        offset_2 = ::core::primitive::u64::wrapping_add(offset_2, 3);
        char::from_u32(::core::primitive::u32::wrapping_add(
          ::core::primitive::u32::wrapping_mul(
            {
              let checked_value = match data.get({
                let Ok(explicit_index) = console_core_number_conversion::index(ch_start);
                explicit_index
              }) {
                Some(explicit_value) => explicit_value,
                None => return Ok(None),
              };
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(*checked_value);
              migrated_value
            },
            256,
          ),
          {
            let checked_value_2 = match data.get({
              let Ok(explicit_index) = console_core_number_conversion::index(
                ::core::primitive::u64::wrapping_add(ch_start, 1),
              );
              explicit_index
            }) {
              Some(explicit_value) => explicit_value,
              None => return Ok(None),
            };
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(*checked_value_2);
            migrated_value
          },
        ))
      }
      ::core::option::Option::None => {
        offset_2 = ::core::primitive::u64::wrapping_add(offset_2, 1);
        None
      }
    };

    Ok(Some(Self {
      data,
      name_chars_range: chars_start..chars_end,
      value,
      num_children,
      children_start: offset_2,
    }))
  }

  #[must_use]
  fn num_name_chars(&self) -> core::result::Result<u64, console_core_never::Never> {
    Ok(
      self
        .name_chars_range
        .end
        .saturating_sub(self.name_chars_range.start),
    )
  }

  #[must_use]
  fn name_chars(
    &self,
  ) -> core::result::Result<impl Iterator<Item = u8> + 'a + use<'a>, console_core_never::Never>
  {
    Ok(
      match self.data.get({
        let Ok(explicit_index) =
          console_core_number_conversion::indices(self.name_chars_range.clone());
        explicit_index
      }) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => &[],
      }
      .iter()
      .map(|ch| *ch & 0x7F),
    )
  }

  #[must_use]
  fn value(&self) -> core::result::Result<Option<char>, console_core_never::Never> {
    Ok(self.value)
  }

  #[must_use]
  fn child_offset(
    &self,
    index: u64,
  ) -> core::result::Result<Option<u64>, console_core_never::Never> {
    let child_start = ::core::primitive::u64::wrapping_add(
      self.children_start,
      ::core::primitive::u64::wrapping_mul(index, 2),
    );

    Ok(Some(::core::primitive::u64::wrapping_add(
      ::core::primitive::u64::wrapping_mul(
        {
          let data = match self.data.get({
            let Ok(explicit_index) = console_core_number_conversion::index(child_start);
            explicit_index
          }) {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          };
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(*data);
          migrated_value
        },
        256,
      ),
      {
        let data = match self.data.get({
          let Ok(explicit_index) = console_core_number_conversion::index(
            ::core::primitive::u64::wrapping_add(child_start, 1),
          );
          explicit_index
        }) {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        };
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(*data);
        migrated_value
      },
    )))
  }

  #[must_use]
  fn child_for_char(
    &self,
    ch: u8,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let mut min = 0u64;
    let mut max = {
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(self.num_children);
      migrated_value
    };

    for _ in 0..u8::BITS {
      match min.cmp(&max) {
        core::cmp::Ordering::Less => {}
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => break,
      }

      let idx =
        ::core::primitive::u64::wrapping_div(::core::primitive::u64::wrapping_add(min, max), 2);
      let Ok(own_child_offset) = self.child_offset(idx);
      let child_offset = match own_child_offset {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      };
      let data = match self.data.get({
        let Ok(explicit_index) = console_core_number_conversion::index(child_offset);
        explicit_index
      }) {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      };
      let child_ch = *data & 0x7F;
      use core::cmp::Ordering;

      match child_ch.cmp(&ch) {
        Ordering::Equal => {
          let Ok(node) = Self::new_child(self.data, child_offset);

          return Ok(node);
        }
        Ordering::Less => min = ::core::primitive::u64::wrapping_add(idx, 1),
        Ordering::Greater => max = idx,
      }
    }

    Ok(None)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn variants() {
    let pairs = [
      ("A.swash", Some("swash")),
      (".notdef", None),
      ("A", None),
      ("uni20AC0308.alt", Some("alt")),
    ];
    for (name, var) in pairs {
      assert_eq!(split_variant(name).1, var);
    }
  }

  #[test]
  fn test_spec_examples() {
    for (input, expected) in [
      ("Lcommaaccent", "Ļ"),
      ("uni20AC0308", "\u{20AC}\u{0308}"),
      ("u1040C", "\u{1040C}"),
      ("uniD801DC0C", ""),
      ("uni20ac", ""),
      (
        "Lcommaaccent_uni20AC0308_u1040C.alternate",
        "\u{013B}\u{20AC}\u{0308}\u{1040C}",
      ),
      ("Lcommaaccent_uni013B_u013B", "ĻĻĻ"),
      ("foo", ""),
      (".notdef", ""),
    ] {
      let result = name_to_chars(input).collect::<String>();

      assert_eq!(result, expected, "{result:?} != {expected:?} for '{input}'")
    }
  }

  #[test]
  fn test_aglfn() {
    for (input, expected) in [("longs_t", "ſt"), ("f_f_i.alt123", "ffi")] {
      let result = name_to_chars(input).collect::<String>();

      assert_eq!(result, expected, "{result:?} != {expected:?} for '{input}'")
    }
  }

  #[test]
  fn test_uni_abcd() {
    for (input, expected) in [
      ("uni0041", "A"),
      ("uni0041_uni0042_uni0043", "ABC"),
      ("uni004100420043", "ABC"),
      ("uni", ""),
      ("uni41", ""),
      ("uni004101", ""),
      ("uniDC00", ""),
    ] {
      let result = name_to_chars(input).collect::<String>();

      assert_eq!(result, expected, "{result:?} != {expected:?} for '{input}'")
    }
  }

  #[test]
  fn test_u_abcd() {
    for (input, expected) in [
      ("u0041", "A"),
      ("u00041", "A"),
      ("u000041", "A"),
      ("u0000041", ""),
      ("u0041_uni0041_A.alt", "AAA"),
    ] {
      let result = name_to_chars(input).collect::<String>();

      assert_eq!(result, expected, "{result:?} != {expected:?} for '{input}'")
    }
  }

  #[test]
  fn union() {
    for (input, expected) in [("union", "∪"), ("union_uniFE00", "\u{222A}\u{FE00}")] {
      let result = name_to_chars(input).collect::<String>();

      assert_eq!(result, expected, "{result:?} != {expected:?} for '{input}'")
    }
  }

  #[test]
  fn one_name() -> ::core::result::Result<(), crate::test_support::Failure> {
    let expected = [
      ("Cdot", 0x10A),
      ("Cdotaccent", 0x10A),
      ("A", 0x41),
      ("union", 0x222A),
    ];
    for (name, expected_codepoint) in expected {
      let fixture_value =
        crate::test_support::fixture_value(name_to_char(name), "fixture value")?;
      assert_eq!(
        <u32 as ::core::convert::From<::core::primitive::char>>::from(fixture_value),
        expected_codepoint
      );
      assert_eq!(name_to_chars(name).count(), 1);
      assert_eq!(
        <u32 as ::core::convert::From<::core::primitive::char>>::from({
          let glyph_character =
            crate::test_support::fixture_value(name_to_chars(name).next(), "fixture value")?;
          glyph_character
        }),
        expected_codepoint
      );
    }

    Ok(())
  }

  #[test]
  fn multi_names() -> Result<(), crate::test_support::Failure> {
    for (name, expected_cps) in AGL_MULTIS.iter() {
      let cps = name_to_chars(name)
        .map(u16::try_from)
        .collect::<Result<Vec<_>, _>>()?;
      assert_eq!(cps, *expected_cps);
    }

    Ok(())
  }

  #[test]
  fn reverse_map() {
    let pairs = [
      ('∪', "union"),
      ('A', "A"),
      ('á', "aacute"),
      ('Á', "Aacute"),
      (';', "semicolon"),
      ('Ļ', "Lcedilla"),
      ('Ċ', "Cdot"),
    ];
    let mut name_buf = [0u8; MAX_NAME_LEN];

    for (ch, name) in pairs {
      assert_eq!(
        char_to_name(ch, &mut name_buf),
        ::core::option::Option::Some(name)
      );
    }
  }

  #[test]
  fn reverse_map_duplicate_names() -> ::core::result::Result<(), crate::test_support::Failure> {
    let pairs = [
      ('Ċ', &["Cdot", "Cdotaccent"]),
      ('Ļ', &["Lcedilla", "Lcommaaccent"]),
    ];
    let mut name_buf = [0u8; MAX_NAME_LEN];

    for (ch, names) in pairs {
      for (n, name) in names.iter().enumerate() {
        let fixture_value = crate::test_support::fixture_value(
          char_to_nth_name(
            ch,
            {
              let Ok(explicit_index) = console_core_number_conversion::fitted::<_, u64>(n);
              explicit_index
            },
            &mut name_buf,
          ),
          "fixture value",
        )?;
        assert_eq!(fixture_value, *name);
      }

      assert!(
        char_to_nth_name(
          ch,
          {
            let Ok(explicit_index) =
              console_core_number_conversion::fitted::<_, u64>(names.len());
            explicit_index
          },
          &mut name_buf
        )
        .is_none()
      );
    }

    Ok(())
  }
}
