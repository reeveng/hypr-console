//! The [cmap](https://docs.microsoft.com/en-us/typography/opentype/spec/cmap) table

include!("../../generated/generated_cmap.rs");

#[cfg(feature = "std")]
use crate::collections::IntSet;
use crate::{FontRef, TableProvider};
use std::ops::Range;

const WINDOWS_SYMBOL_ENCODING: u16 = 0;
const WINDOWS_UNICODE_BMP_ENCODING: u16 = 1;
const WINDOWS_UNICODE_FULL_ENCODING: u16 = 10;

const UNICODE_1_0_ENCODING: u16 = 0;
const UNICODE_1_1_ENCODING: u16 = 1;
const UNICODE_ISO_ENCODING: u16 = 2;
const UNICODE_2_0_BMP_ENCODING: u16 = 3;
const UNICODE_2_0_FULL_ENCODING: u16 = 4;
const UNICODE_FULL_ENCODING: u16 = 6;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MapVariant {
  UseDefault,
  Variant(GlyphId),
}

impl<'a> Cmap<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap::map_codepoint` method returns the optional value directly to callers that match its presence")
  )]
  pub fn map_codepoint(&self, codepoint: impl Into<u32>) -> Option<GlyphId> {
    let codepoint_2 = <_ as ::core::convert::Into<::core::primitive::u32>>::into(codepoint);

    let Ok(encoding_records) = self.encoding_records();

    for record in encoding_records {
      let Ok(value_1) = self.offset_data();

      match record.subtable(value_1) {
        Ok(subtable) => match subtable.map_codepoint(codepoint_2) {
          Some(gid) => {
            return Some(gid);
          }
          None => {}
        },
        Err(_unreadable_cmap_subtable) => continue,
      }
    }

    None
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap::best_subtable` method returns the optional value directly to callers that match its presence")
  )]
  pub fn best_subtable(&self) -> Option<(u16, EncodingRecord, CmapSubtable<'a>)> {
    let Ok(offset_data) = self.offset_data();
    let Ok(records) = self.encoding_records();
    let find = |platform_id, encoding_id| {
      for (index, record) in records.iter().enumerate() {
        let Ok(record_platform_id) = record.platform_id();
        let Ok(record_encoding_id) = record.encoding_id();

        match ::core::primitive::bool::then_some(record_platform_id == platform_id && record_encoding_id == encoding_id, ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => continue,
        }

        match record.subtable(offset_data) {
          Ok(subtable) => match subtable {
            CmapSubtable::Format0(_)
            | CmapSubtable::Format4(_)
            | CmapSubtable::Format6(_)
            | CmapSubtable::Format10(_)
            | CmapSubtable::Format12(_)
            | CmapSubtable::Format13(_) => {
              return Some((
                ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, u16>(index);
                  migrated_value
                }),
                *record,
                subtable,
              ));
            }
            CmapSubtable::Format2(_) | CmapSubtable::Format8(_) | CmapSubtable::Format14(_) => {}
          },
          Err(_unreadable_cmap_subtable) => continue,
        }
      }

      None
    };

    find(PlatformId::Windows, WINDOWS_SYMBOL_ENCODING)
      .or_else(|| find(PlatformId::Windows, WINDOWS_UNICODE_FULL_ENCODING))
      .or_else(|| find(PlatformId::Unicode, UNICODE_FULL_ENCODING))
      .or_else(|| find(PlatformId::Unicode, UNICODE_2_0_FULL_ENCODING))
      .or_else(|| find(PlatformId::Windows, WINDOWS_UNICODE_BMP_ENCODING))
      .or_else(|| find(PlatformId::Unicode, UNICODE_2_0_BMP_ENCODING))
      .or_else(|| find(PlatformId::Unicode, UNICODE_ISO_ENCODING))
      .or_else(|| find(PlatformId::Unicode, UNICODE_1_1_ENCODING))
      .or_else(|| find(PlatformId::Unicode, UNICODE_1_0_ENCODING))
      .or_else(|| find(PlatformId::Macintosh, 0))
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap::uvs_subtable` method returns the optional value directly to callers that match its presence")
  )]
  pub fn uvs_subtable(&self) -> Option<(u16, Cmap14<'a>)> {
    let Ok(offset_data) = self.offset_data();

    let Ok(encoding_records) = self.encoding_records();

    encoding_records.iter().enumerate().find_map(|(index, record)| {
      match record.subtable(offset_data) {
        Ok(CmapSubtable::Format14(table)) => {
          let Ok(index_2) = console_core_number_conversion::wrapped::<_, u16>(index);

          Some((index_2, table))
        }
        Ok(_) => None,
        Err(_unreadable_cmap_subtable) => None,
      }
    })
  }

  pub fn subtable(&self, index: u16) -> Result<CmapSubtable<'a>, ReadError> {
    let Ok(value_26) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(index);
      migrated_value
    });

    let Ok(encoding_records) = self.encoding_records();

    encoding_records
      .get(value_26)
      .ok_or(ReadError::OutOfBounds)
      .and_then(|encoding| {
        let Ok(value_2) = self.offset_data();

        encoding.subtable(value_2)
      })
  }

  #[cfg(feature = "std")]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap::closure_glyphs` method is called through its existing unit-returning signature")
  )]
  pub fn closure_glyphs(&self, unicodes: &IntSet<u32>, glyph_set: &mut IntSet<GlyphId>) {
    let Ok(encoding_records) = self.encoding_records();

    for record in encoding_records {
      let Ok(value_3) = self.offset_data();

      match record.subtable(value_3) {
        Ok(subtable) => match subtable {
          CmapSubtable::Format14(format14) => {
            format14.closure_glyphs(unicodes, glyph_set);
            return;
          }
          CmapSubtable::Format0(_)
          | CmapSubtable::Format2(_)
          | CmapSubtable::Format4(_)
          | CmapSubtable::Format6(_)
          | CmapSubtable::Format8(_)
          | CmapSubtable::Format10(_)
          | CmapSubtable::Format12(_)
          | CmapSubtable::Format13(_) => {
            continue;
          }
        },
        Err(_unreadable_cmap_subtable) => continue,
      }
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyEncoding {
  MacRoman,
  WindowsSymbol,
  Other,
}

impl LegacyEncoding {
  pub fn classify(platform: PlatformId, encoding: u16) -> Result<Self, Never> {
    Ok(match (platform, encoding) {
      (PlatformId::Macintosh, 0) => Self::MacRoman,
      (PlatformId::Windows, WINDOWS_SYMBOL_ENCODING) => Self::WindowsSymbol,
      (_, _) => Self::Other,
    })
  }
}

impl EncodingRecord {
  pub fn legacy_encoding(&self) -> Result<LegacyEncoding, Never> {
    let Ok(platform) = self.platform_id();
    let Ok(encoding) = self.encoding_id();

    LegacyEncoding::classify(platform, encoding)
  }
}

impl<'a> CmapSubtable<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `CmapSubtable::language` method returns `u32` directly in the existing parser interface")
  )]
  pub fn language(&self) -> u32 {
    match self {
      Self::Format0(item) => {
        let Ok(language) = item.language();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(language);
        migrated_value
      }
      Self::Format2(item) => {
        let Ok(language) = item.language();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(language);
        migrated_value
      }
      Self::Format4(item) => {
        let Ok(language) = item.language();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(language);
        migrated_value
      }
      Self::Format6(item) => {
        let Ok(language) = item.language();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(language);
        migrated_value
      }
      Self::Format10(item) => {
        let Ok(language) = item.language();

        language
      }
      Self::Format12(item) => {
        let Ok(language) = item.language();

        language
      }
      Self::Format13(item) => {
        let Ok(language) = item.language();

        language
      }
      Self::Format8(_) | Self::Format14(_) => 0,
    }
  }

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `CmapSubtable::map_codepoint` method returns the optional value directly to callers that match its presence")
  )]
  pub fn map_codepoint(&self, codepoint: impl Into<u32>) -> Option<GlyphId> {
    match self {
      Self::Format0(item) => item.map_codepoint(codepoint),
      Self::Format4(item) => item.map_codepoint(codepoint),
      Self::Format6(item) => item.map_codepoint(codepoint),
      Self::Format10(item) => item.map_codepoint(codepoint),
      Self::Format12(item) => item.map_codepoint(codepoint),
      Self::Format13(item) => item.map_codepoint(codepoint),
      Self::Format2(_) | Self::Format8(_) | Self::Format14(_) => None,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `CmapSubtable::iter` method returns `CmapSubtableIter` directly in the existing parser interface")
  )]
  pub fn iter(&self) -> CmapSubtableIter<'a> {
    let limits = CmapIterLimits {
      max_char: u32::MAX,
      glyph_count: u32::MAX,
    };
    self.iter_with_limits(limits)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `CmapSubtable::iter_with_limits` method returns `CmapSubtableIter` directly in the existing parser interface")
  )]
  pub fn iter_with_limits(&self, limits: CmapIterLimits) -> CmapSubtableIter<'a> {
    match self {
      Self::Format4(item) => CmapSubtableIter::Format4(item.iter()),
      Self::Format6(item) => CmapSubtableIter::Format6(item.iter()),
      Self::Format10(item) => CmapSubtableIter::Format10(item.iter()),
      Self::Format12(item) => CmapSubtableIter::Format12(item.iter_with_limits(limits)),
      Self::Format13(item) => CmapSubtableIter::Format13(item.iter_with_limits(limits)),
      Self::Format0(_) | Self::Format2(_) | Self::Format8(_) | Self::Format14(_) => {
        CmapSubtableIter::None
      }
    }
  }
}

#[derive(Clone)]
#[non_exhaustive]
pub enum CmapSubtableIter<'a> {
  None,
  Format4(Cmap4Iter<'a>),
  Format6(Cmap6Iter<'a>),
  Format10(Cmap10Iter<'a>),
  Format12(Cmap12Iter<'a>),
  Format13(Cmap13Iter<'a>),
}

impl Iterator for CmapSubtableIter<'_> {
  type Item = (u32, GlyphId);

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    match self {
      Self::None => None,
      Self::Format4(iter) => iter.next(),
      Self::Format6(iter) => iter.next(),
      Self::Format10(iter) => iter.next(),
      Self::Format12(iter) => iter.next(),
      Self::Format13(iter) => iter.next(),
    }
  }
}

impl Cmap0<'_> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap0::map_codepoint` method returns the optional value directly to callers that match its presence")
  )]
  pub fn map_codepoint(&self, codepoint: impl Into<u32>) -> Option<GlyphId> {
    let codepoint_2 = <_ as ::core::convert::Into<::core::primitive::u32>>::into(codepoint);

    let Ok(value_31) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(codepoint_2);
      migrated_value
    });

    let Ok(glyph_id_array) = self.glyph_id_array();

    glyph_id_array.get(value_31).map(|g| {
      let Ok(value_30) = GlyphId::new({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(*g);
        migrated_value
      });

      value_30
    })
  }
}

struct SegmentCodepoint {
  codepoint: u16,
  index: u64,
  first_codepoint: u16,
}

impl<'a> Cmap4<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap4::map_codepoint` method returns the optional value directly to callers that match its presence")
  )]
  pub fn map_codepoint(&self, codepoint: impl Into<u32>) -> Option<GlyphId> {
    let codepoint_2 = <_ as ::core::convert::Into<::core::primitive::u32>>::into(codepoint);

    match ::core::primitive::bool::then_some(codepoint_2 > 0xFFFF, ()) {
      ::core::option::Option::Some(()) => {
        return None;
      }
      ::core::option::Option::None => {}
    }

    let codepoint_3 = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(codepoint_2);
      migrated_value
    };
    let mut lo = 0;
    let mut hi = ::core::primitive::u64::wrapping_div(
      {
        let Ok(seg_count_x2) = self.seg_count_x2();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(seg_count_x2);
        migrated_value
      },
      2,
    );
    let Ok(start_codes) = self.start_code();
    let Ok(end_codes) = self.end_code();

    for _ in 0..u16::BITS {
      match lo.cmp(&hi) {
        core::cmp::Ordering::Less => {}
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => break,
      }

      let i =
        ::core::primitive::u64::wrapping_div(::core::primitive::u64::wrapping_add(lo, hi), 2);
      let Ok(value_33) = console_core_number_conversion::index(i);

      let checked_value = start_codes.get(value_33)?;
      let Ok(value_48) = checked_value.get();

      let start_code = value_48;
      let value_4 = start_code;

      match ::core::primitive::bool::then_some(codepoint_3 < value_4, ()) {
        ::core::option::Option::Some(()) => {
          hi = i;
        }
        ::core::option::Option::None => {
          let Ok(value_34) = console_core_number_conversion::index(i);

          let end_code = end_codes.get(value_34)?;
          let Ok(value_5) = end_code.get();

          match ::core::primitive::bool::then_some(codepoint_3 > value_5, ()) {
            ::core::option::Option::Some(()) => {
              lo = ::core::primitive::u64::wrapping_add(i, 1);
            }
            ::core::option::Option::None => {
              let value_6 = start_code;

              let Ok(lookup_glyph_id) = self.lookup_glyph_id(SegmentCodepoint {
                codepoint: codepoint_3,
                index: i,
                first_codepoint: value_6,
              });
              return lookup_glyph_id;
            }
          }
        }
      }
    }

    None
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap4::iter` method returns `Cmap4Iter` directly in the existing parser interface")
  )]
  pub fn iter(&self) -> Cmap4Iter<'a> {
    let Ok(cmap4iter) = Cmap4Iter::new(self.clone());

    cmap4iter
  }

  #[must_use]
  fn lookup_glyph_id(&self, location: SegmentCodepoint) -> Result<Option<GlyphId>, Never> {
      let SegmentCodepoint { codepoint, index, first_codepoint: start_code } = location;
      let Ok(deltas) = self.id_delta();
      let Ok(range_offsets) = self.id_range_offsets();
      let Ok(value_35) = console_core_number_conversion::index(index);

      let checked_value = match deltas.get(value_35) { Some(explicit_value) => explicit_value, None => return Ok(None) };
      let Ok(value_7) = checked_value.get();

      let delta = {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value_7);
        migrated_value
      };
      let Ok(value_36) = console_core_number_conversion::index(index);

      let checked_value_2 = match range_offsets.get(value_36) { Some(explicit_value) => explicit_value, None => return Ok(None) };
      let Ok(value_8) = checked_value_2.get();

      let range_offset = {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_8);
        migrated_value
      };

      match ::core::primitive::bool::then_some(range_offset == 0, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(Some(GlyphId::from({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
              ::core::primitive::i32::wrapping_add(
                {
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, i32>(codepoint);
                  migrated_value
                },
                delta,
              ),
            );
            migrated_value
          })));
        }
        ::core::option::Option::None => {}
      }

      let mut offset = ::core::primitive::u64::wrapping_add(
        ::core::primitive::u64::wrapping_div(range_offset, 2),
        {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
            ::core::primitive::u16::wrapping_sub(codepoint, start_code),
          );
          migrated_value
        },
      );
      let Ok(value_37) = console_core_number_conversion::index(index);

      let Ok(value_47) = console_core_number_conversion::fitted::<_, u64>(
        range_offsets.len().wrapping_sub(value_37),
      );

      offset = offset.saturating_sub(value_47);
      let Ok(value_38) = console_core_number_conversion::index(offset);

      let Ok(own_glyph_id_array) = self.glyph_id_array();
      let glyph_id_array = match own_glyph_id_array.get(value_38) { Some(explicit_value) => explicit_value, None => return Ok(None) };
      let Ok(gid) = glyph_id_array.get();

      Ok((gid != 0).then_some(GlyphId::from({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u16>(::core::primitive::i32::wrapping_add(
            {
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(gid);
              migrated_value
            },
            delta,
          ));
        migrated_value
      })))
  }

  #[must_use]
  fn code_range(&self, index: u64) -> core::result::Result<Option<Range<u32>>, console_core_never::Never> {
      let Ok(value_39) = console_core_number_conversion::index(index);

      let Ok(own_start_code) = self.start_code();
      let start_code = match own_start_code.get(value_39) { Some(explicit_value) => explicit_value, None => return Ok(None) };
      let Ok(value_9) = start_code.get();

      let start = {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(value_9);
        migrated_value
      };
      let Ok(value_40) = console_core_number_conversion::index(index);

      let Ok(own_end_code) = self.end_code();
      let end_code = match own_end_code.get(value_40) { Some(explicit_value) => explicit_value, None => return Ok(None) };
      let Ok(value_10) = end_code.get();

      let end = {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(value_10);
        migrated_value
      };

      Ok(Some(start..::core::primitive::u32::wrapping_add(end, 1)))
  }
}

#[derive(Clone)]
pub struct Cmap4Iter<'a> {
  subtable: Cmap4<'a>,
  cur_range: Range<u32>,
  cur_start_code: u16,
  cur_range_ix: u64,
}

impl<'a> Cmap4Iter<'a> {
  #[must_use]
  fn new(subtable: Cmap4<'a>) -> core::result::Result<Self, console_core_never::Never> {
      let Ok(code_range) = subtable.code_range(0);
      let cur_range = match code_range {
        Some(range) => range,
        None => Default::default(),
      };
      let cur_start_code = {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(cur_range.start);
        migrated_value
      };

      Ok(Self {
        subtable,
        cur_range,
        cur_start_code,
        cur_range_ix: 0,
      })
  }
}

impl Iterator for Cmap4Iter<'_> {
  type Item = (u32, GlyphId);

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(segment_count_x2) = self.subtable.seg_count_x2();
    let segment_count = u64::from(segment_count_x2).wrapping_div(2);

    for _ in self.cur_range_ix..=segment_count {
      for codepoint in self.cur_range.by_ref() {
        let Ok(code) = console_core_number_conversion::wrapped::<_, u16>(codepoint);
        let Ok(glyph) = self.subtable.lookup_glyph_id(SegmentCodepoint {
          codepoint: code,
          index: self.cur_range_ix,
          first_codepoint: self.cur_start_code,
        });

        match glyph {
          Some(glyph_2) => return Some((codepoint, glyph_2)),
          None => {}
        }
      }

      self.cur_range_ix = self.cur_range_ix.wrapping_add(1);
      let Ok(code_range) = self.subtable.code_range(self.cur_range_ix);
      let next = code_range?;
      let previous_end = self.cur_range.end;
      self.cur_range = next.start.max(previous_end)..next.end.max(previous_end);
      let Ok(start) = console_core_number_conversion::wrapped::<_, u16>(self.cur_range.start);
      self.cur_start_code = start;
    }

    None
  }
}

impl<'a> Cmap6<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap6::map_codepoint` method returns the optional value directly to callers that match its presence")
  )]
  pub fn map_codepoint(&self, codepoint: impl Into<u32>) -> Option<GlyphId> {
    let codepoint_2 = <_ as ::core::convert::Into<::core::primitive::u32>>::into(codepoint);

    let first = {
      let Ok(first_code) = self.first_code();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(first_code);
      migrated_value
    };
    let idx = codepoint_2.checked_sub(first)?;
    let Ok(value_41) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
      migrated_value
    });

    let Ok(glyph_id_array) = self.glyph_id_array();

    glyph_id_array.get(value_41).map(|g| {
      let Ok(value_11) = g.get();

      let Ok(value_27) = GlyphId::new({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(value_11);
        migrated_value
      });

      value_27
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap6::iter` method returns `Cmap6Iter` directly in the existing parser interface")
  )]
  pub fn iter(&self) -> Cmap6Iter<'a> {
    Cmap6Iter {
      first: ({
        let Ok(first_code) = self.first_code();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(first_code);
        migrated_value
      }),
      glyph_ids: ({
        let Ok(explicit_value) = self.glyph_id_array();
        explicit_value
      }),
      pos: 0,
    }
  }
}

#[derive(Clone)]
pub struct Cmap6Iter<'a> {
  first: u32,
  glyph_ids: &'a [BigEndian<u16>],
  pos: u32,
}

impl Iterator for Cmap6Iter<'_> {
  type Item = (u32, GlyphId);

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(value_42) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(self.pos);
      migrated_value
    });

    let glyph_id = self.glyph_ids.get(value_42)?;
    let Ok(value_49) = glyph_id.get();

    let gid =
      <::font_types::GlyphId as ::core::convert::From<::core::primitive::u16>>::from(value_49);
    let codepoint = ::core::primitive::u32::wrapping_add(self.first, self.pos);

    match (1, &mut self.pos) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    Some((codepoint, gid))
  }
}

impl<'a> Cmap10<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap10::map_codepoint` method returns the optional value directly to callers that match its presence")
  )]
  pub fn map_codepoint(&self, codepoint: impl Into<u32>) -> Option<GlyphId> {
    let codepoint_2 = <_ as ::core::convert::Into<::core::primitive::u32>>::into(codepoint);
    let Ok(start_char_code) = self.start_char_code();
    let idx = codepoint_2.checked_sub(start_char_code)?;
    let Ok(value_43) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
      migrated_value
    });

    let Ok(glyph_id_array) = self.glyph_id_array();

    glyph_id_array.get(value_43).map(|g| {
      let Ok(value_12) = g.get();

      let Ok(value_28) = GlyphId::new({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(value_12);
        migrated_value
      });

      value_28
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap10::iter` method returns `Cmap10Iter` directly in the existing parser interface")
  )]
  pub fn iter(&self) -> Cmap10Iter<'a> {
    let Ok(first) = self.start_char_code();
    let Ok(glyph_ids) = self.glyph_id_array();

    Cmap10Iter {
      first,
      glyph_ids,
      pos: 0,
    }
  }
}

#[derive(Clone)]
pub struct Cmap10Iter<'a> {
  first: u32,
  glyph_ids: &'a [BigEndian<u16>],
  pos: u32,
}

impl Iterator for Cmap10Iter<'_> {
  type Item = (u32, GlyphId);

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(value_44) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(self.pos);
      migrated_value
    });

    let glyph_id = self.glyph_ids.get(value_44)?;
    let Ok(value_50) = glyph_id.get();

    let gid =
      <::font_types::GlyphId as ::core::convert::From<::core::primitive::u16>>::from(value_50);
    let codepoint = ::core::primitive::u32::wrapping_add(self.first, self.pos);

    match (1, &mut self.pos) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    Some((codepoint, gid))
  }
}

struct GroupMapping {
  codepoint: u32,
  start_code: u32,
  reference_glyph: u32,
}

trait AnyMapGroup {
  const IS_CONSTANT: bool;

  fn start_char_code(&self) -> u32;

  fn end_char_code(&self) -> u32;

  fn ref_glyph_id(&self) -> u32;

  #[must_use]
  fn compute_glyph_id(mapping: GroupMapping) -> Result<GlyphId, console_core_never::Never> {
    let GroupMapping { codepoint, start_code, reference_glyph } = mapping;
    let code = match crate::Presence::from(Self::IS_CONSTANT) {
      crate::Presence::Present => reference_glyph,
      crate::Presence::Absent => reference_glyph.wrapping_add(codepoint.wrapping_sub(start_code)),
    };
    GlyphId::new(code)
  }
}

impl AnyMapGroup for ConstantMapGroup {
  const IS_CONSTANT: bool = true;

  fn start_char_code(&self) -> u32 {
    let Ok(start_char_code) = self.start_char_code();

    start_char_code
  }

  fn end_char_code(&self) -> u32 {
    let Ok(end_char_code) = self.end_char_code();

    end_char_code
  }

  fn ref_glyph_id(&self) -> u32 {
    let Ok(glyph_id) = self.glyph_id();

    glyph_id
  }
}

impl AnyMapGroup for SequentialMapGroup {
  const IS_CONSTANT: bool = false;

  fn start_char_code(&self) -> u32 {
    let Ok(start_char_code) = self.start_char_code();

    start_char_code
  }

  fn end_char_code(&self) -> u32 {
    let Ok(end_char_code) = self.end_char_code();

    end_char_code
  }

  fn ref_glyph_id(&self) -> u32 {
    let Ok(start_glyph_id) = self.start_glyph_id();

    start_glyph_id
  }
}

#[must_use]
fn cmap1213_map_codepoint<T: AnyMapGroup>(
  groups: &[T],
  codepoint: impl Into<u32>,
) -> core::result::Result<Option<GlyphId>, console_core_never::Never> {
    let codepoint_2 = <_ as ::core::convert::Into<::core::primitive::u32>>::into(codepoint);
    let mut lo = 0u64;
    let Ok(mut hi) = console_core_number_conversion::fitted::<_, u64>(groups.len());

    for _ in 0..u32::BITS {
      match lo.cmp(&hi) {
        core::cmp::Ordering::Less => {}
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => break,
      }

      let i = lo.midpoint(hi);
      let Ok(index) = console_core_number_conversion::index(i);
      let group = match groups.get(index) { Some(explicit_value) => explicit_value, None => return Ok(None) };

      match ::core::primitive::bool::then_some(codepoint_2 < group.start_char_code(), ()) {
        ::core::option::Option::Some(()) => {
          hi = i;
        }
        ::core::option::Option::None => match ::core::primitive::bool::then_some(codepoint_2 > group.end_char_code(), ()) {
          ::core::option::Option::Some(()) => {
            lo = i.wrapping_add(1);
          }
          ::core::option::Option::None => {
            let Ok(glyph) = T::compute_glyph_id(GroupMapping {
              codepoint: codepoint_2,
              start_code: group.start_char_code(),
              reference_glyph: group.ref_glyph_id(),
            });
            return Ok(Some(glyph));
          }
        },
      }
    }

    Ok(None)
}

#[derive(Copy, Clone, Debug)]
pub struct CmapIterLimits {
  pub max_char: u32,
  pub glyph_count: u32,
}

impl CmapIterLimits {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `CmapIterLimits::default_for_font` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn default_for_font(font: &FontRef) -> Self {
    let value_15 = match font
      .maxp()
      .map(|maxp| {
        let Ok(value_58) = maxp.num_glyphs();

        value_58
      }) { Ok(value) => value, Err(_unavailable_glyph_count) => u16::MAX };

    let glyph_count = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(value_15);
      migrated_value
    };
    Self {
      max_char: <u32 as ::core::convert::From<::core::primitive::char>>::from(char::MAX),
      glyph_count,
    }
  }
}

impl Default for CmapIterLimits {
  fn default() -> Self {
    Self {
      max_char: <u32 as ::core::convert::From<::core::primitive::char>>::from(char::MAX),

      glyph_count: ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(u16::MAX);
        migrated_value
      }),
    }
  }
}

#[derive(Clone, Debug)]
struct Cmap1213IterGroup {
  range: Range<u64>,
  start_code: u32,
  ref_glyph_id: u32,
}

#[must_use]
fn cmap1213_iter_group<T: AnyMapGroup>(
  groups: &[T],
  index: u64,
  limits: &Option<CmapIterLimits>,
) -> core::result::Result<Option<Cmap1213IterGroup>, console_core_never::Never> {
    let Ok(value_45) = console_core_number_conversion::index(index);

    let group = match groups.get(value_45) { Some(explicit_value) => explicit_value, None => return Ok(None) };
    let start_code = group.start_char_code();

    let end_code = ::core::primitive::u64::wrapping_add(
      {
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(group.end_char_code());
        migrated_value
      },
      1,
    );
    let start_glyph_id = group.ref_glyph_id();
    let end_code_2 = match limits {
      Some(limits_2) => match ::core::primitive::bool::then_some(T::IS_CONSTANT, ()) {
        ::core::option::Option::Some(()) => end_code.min({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(limits_2.max_char);
          migrated_value
        }),
        ::core::option::Option::None => ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(limits_2.glyph_count);
          migrated_value
        })
        .saturating_sub({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(start_glyph_id);
          migrated_value
        })
        .saturating_add({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(start_code);
          migrated_value
        })
        .min(end_code.min({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(limits_2.max_char);
          migrated_value
        })),
      },
      None => end_code,
    };

    Ok(Some(Cmap1213IterGroup {
      range: ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(start_code);
        migrated_value
      })..end_code_2,
      start_code,
      ref_glyph_id: start_glyph_id,
    }))
}

#[derive(Clone)]
struct Cmap1213Iter<'a, T> {
  groups: &'a [T],
  cur_group: Option<Cmap1213IterGroup>,
  cur_group_ix: u64,
  limits: Option<CmapIterLimits>,
}

impl<'a, T> Cmap1213Iter<'a, T>
where
  T: AnyMapGroup,
{
  #[must_use]
  fn new(groups: &'a [T], limits: Option<CmapIterLimits>) -> core::result::Result<Self, console_core_never::Never> {
      let Ok(cur_group) = cmap1213_iter_group(groups, 0, &limits);

      Ok(Self {
        groups,
        cur_group,
        cur_group_ix: 0,
        limits,
      })
  }
}

impl<T> Iterator for Cmap1213Iter<'_, T>
where
  T: AnyMapGroup,
{
  type Item = (u32, GlyphId);

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(group_count) = console_core_number_conversion::fitted::<_, u64>(self.groups.len());

    for _ in self.cur_group_ix..=group_count {
      let group = self.cur_group.as_mut()?;

      match group.range.next() {
        Some(codepoint) => {
          let codepoint_2 = {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(codepoint);
            migrated_value
          };
          let Ok(glyph_id) = T::compute_glyph_id(GroupMapping {
            codepoint: codepoint_2,
            start_code: group.start_code,
            reference_glyph: group.ref_glyph_id,
          });
          return Some((codepoint_2, glyph_id));
        }
        None => {
          match (1, &mut self.cur_group_ix) {
            (explicit_arithmetic_amount, explicit_arithmetic_place) => {
              *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
                *explicit_arithmetic_place,
                explicit_arithmetic_amount,
              )
            }
          };

          let mut next_group = ({ let Ok(explicit_value) = cmap1213_iter_group(self.groups, self.cur_group_ix, &self.limits); explicit_value })?;

          match ::core::primitive::bool::then_some(next_group.range.start < group.range.end, ()) {
            ::core::option::Option::Some(()) => {
              next_group.range = group.range.end..next_group.range.end;
            }
            ::core::option::Option::None => {}
          }

          self.cur_group = Some(next_group);
        }
      }
    }

    None
  }
}

impl<'a> Cmap12<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap12::map_codepoint` method returns the optional value directly to callers that match its presence")
  )]
  pub fn map_codepoint(&self, codepoint: impl Into<u32>) -> Option<GlyphId> {
    let Ok(groups) = self.groups();

    { let Ok(explicit_value) = cmap1213_map_codepoint(groups, codepoint); explicit_value }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap12::iter` method returns `Cmap12Iter` directly in the existing parser interface")
  )]
  pub fn iter(&self) -> Cmap12Iter<'a> {
    let Ok(cmap12iter) = Cmap12Iter::new(self.clone(), None);

    cmap12iter
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap12::iter_with_limits` method returns `Cmap12Iter` directly in the existing parser interface")
  )]
  pub fn iter_with_limits(&self, limits: CmapIterLimits) -> Cmap12Iter<'a> {
    let Ok(cmap12iter) = Cmap12Iter::new(self.clone(), Some(limits));

    cmap12iter
  }
}

#[derive(Clone)]
pub struct Cmap12Iter<'a>(Cmap1213Iter<'a, SequentialMapGroup>);

impl<'a> Cmap12Iter<'a> {
  #[must_use]
  fn new(subtable: Cmap12<'a>, limits: Option<CmapIterLimits>) -> core::result::Result<Self, console_core_never::Never> {
      let Ok(groups) = subtable.groups();

      let Ok(cmap1213iter) = Cmap1213Iter::new(groups, limits);

      Ok(Self(cmap1213iter))
  }
}

impl Iterator for Cmap12Iter<'_> {
  type Item = (u32, GlyphId);

  fn next(&mut self) -> Option<Self::Item> {
    self.0.next()
  }
}

impl<'a> Cmap13<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap13::map_codepoint` method returns the optional value directly to callers that match its presence")
  )]
  pub fn map_codepoint(&self, codepoint: impl Into<u32>) -> Option<GlyphId> {
    let Ok(groups) = self.groups();

    { let Ok(explicit_value) = cmap1213_map_codepoint(groups, codepoint); explicit_value }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap13::iter` method returns `Cmap13Iter` directly in the existing parser interface")
  )]
  pub fn iter(&self) -> Cmap13Iter<'a> {
    let Ok(cmap13iter) = Cmap13Iter::new(self.clone(), None);

    cmap13iter
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap13::iter_with_limits` method returns `Cmap13Iter` directly in the existing parser interface")
  )]
  pub fn iter_with_limits(&self, limits: CmapIterLimits) -> Cmap13Iter<'a> {
    let Ok(cmap13iter) = Cmap13Iter::new(self.clone(), Some(limits));

    cmap13iter
  }
}

#[derive(Clone)]
pub struct Cmap13Iter<'a>(Cmap1213Iter<'a, ConstantMapGroup>);

impl<'a> Cmap13Iter<'a> {
  #[must_use]
  fn new(subtable: Cmap13<'a>, limits: Option<CmapIterLimits>) -> core::result::Result<Self, console_core_never::Never> {
      let Ok(groups) = subtable.groups();

      let Ok(cmap1213iter) = Cmap1213Iter::new(groups, limits);

      Ok(Self(cmap1213iter))
  }
}

impl Iterator for Cmap13Iter<'_> {
  type Item = (u32, GlyphId);

  fn next(&mut self) -> Option<Self::Item> {
    self.0.next()
  }
}

fn default_range_contains(table: &DefaultUvs<'_>, codepoint: u32) -> Result<crate::Presence, Never> {
  use core::cmp::Ordering;
  let Ok(ranges) = table.ranges();
  Ok(match ranges.binary_search_by(|range| {
    let Ok(unicode_value) = range.start_unicode_value();
    let Ok(additional) = range.additional_count();
    let start = u32::from(unicode_value);
    let end = start.wrapping_add(u32::from(additional));

    match (start..=end).contains(&codepoint).then_some(()) {
      Some(()) => Ordering::Equal,
      None => start.cmp(&codepoint),
    }
  }) {
    Ok(_containing_range) => crate::Presence::Present,
    Err(_insertion_position) => crate::Presence::Absent,
  })
}

impl<'a> Cmap14<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap14::map_variant` method returns the optional value directly to callers that match its presence")
  )]
  pub fn map_variant(
    &self,
    codepoint: impl Into<u32>,
    selector: impl Into<u32>,
  ) -> Option<MapVariant> {
    let codepoint_2 = <_ as ::core::convert::Into<::core::primitive::u32>>::into(codepoint);
    let selector_2 = <_ as ::core::convert::Into<::core::primitive::u32>>::into(selector);
    let Ok(selector_records) = self.var_selector();

    let selector_record = match selector_records
      .binary_search_by(|rec| {
        let Ok(var_selector) = rec.var_selector();
        let rec_selector: u32 = <::core::primitive::u32 as ::core::convert::From<
          ::font_types::Uint24,
        >>::from(var_selector);
        rec_selector.cmp(&selector_2)
      }) { ::core::result::Result::Ok(value____) => ::core::option::Option::Some(value____), ::core::result::Result::Err(_insertion_position) => ::core::option::Option::None }
      .and_then(|idx| selector_records.get(idx))?;

    let Ok(value_16) = self.offset_data();

    let default = match selector_record.default_uvs(value_16) {
      Ok(Some(default_table)) => {
        let Ok(contains) = default_range_contains(&default_table, codepoint_2);
        contains
      }
      Ok(None) => crate::Presence::Absent,
      Err(_unreadable_default_variants) => crate::Presence::Absent,
    };

    match default {
      crate::Presence::Present => return Some(MapVariant::UseDefault),
      crate::Presence::Absent => {}
    }

    let non_default_uvs = match selector_record.non_default_uvs(value_16) {
      Ok(Some(non_default_table)) => non_default_table,
      Ok(None) => return None,
      Err(_unreadable_non_default_variants) => return None,
    };
    let Ok(mapping) = non_default_uvs.uvs_mapping();
    let record = match mapping.binary_search_by(|map| {
      let Ok(unicode_value) = map.unicode_value();
      u32::from(unicode_value).cmp(&codepoint_2)
    }) {
      Ok(position) => mapping.get(position),
      Err(_insertion_position) => None,
    }?;
    let Ok(glyph_id) = record.glyph_id();

    Some(MapVariant::Variant(GlyphId::from(glyph_id)))
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap14::iter` method returns `Cmap14Iter` directly in the existing parser interface")
  )]
  pub fn iter(&self) -> Cmap14Iter<'a> {
    let Ok(cmap14iter) = Cmap14Iter::new(self.clone());

    cmap14iter
  }

  #[must_use]
  fn selector(
    &self,
    index: u64,
  ) -> core::result::Result<(
    Option<VariationSelector>,
    Option<DefaultUvs<'a>>,
    Option<NonDefaultUvs<'a>>,
  ), console_core_never::Never> {
      let Ok(value_46) = console_core_number_conversion::index(index);

      let Ok(var_selector) = self.var_selector();
      let selector = var_selector.get(value_46).cloned();
      let default_uvs = selector.as_ref().and_then(|selector_2| {
        let Ok(value_18) = self.offset_data();

        match selector_2.default_uvs(value_18) {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }
        .flatten()
      });
      let non_default_uvs = selector.as_ref().and_then(|selector_3| {
        let Ok(value_19) = self.offset_data();

        match selector_3.non_default_uvs(value_19) {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }
        .flatten()
      });

      Ok((selector, default_uvs, non_default_uvs))
  }

  #[cfg(feature = "std")]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Cmap14::closure_glyphs` method is called through its existing unit-returning signature")
  )]
  pub fn closure_glyphs(&self, unicodes: &IntSet<u32>, glyph_set: &mut IntSet<GlyphId>) {
    let Ok(var_selector) = self.var_selector();

    for selector in var_selector {
      let Ok(selector_var_selector) = selector.var_selector();
      let Ok(value_20) = selector_var_selector.to_u32();

      let Ok(value_24) = unicodes.contains(value_20);

      match ::core::primitive::bool::then_some(value_24 == console_core_integer_sets::Presence::Absent, ()) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let Ok(value_21) = self.offset_data();

      match (match selector.non_default_uvs(value_21) { Ok(value) => Some(value), Err(_unreadable_variant_mapping) => None }).flatten() {
        Some(non_default_uvs) => {
          let Ok(uvs_mapping) = non_default_uvs.uvs_mapping();
          glyph_set.extend(
            uvs_mapping
              .iter()
              .filter(|m| {
                let Ok(unicode_value) = m.unicode_value();
                let Ok(value_22) = unicode_value.to_u32();

                let Ok(value_25) = unicodes.contains(value_22);

                value_25 == console_core_integer_sets::Presence::Present
              })
              .map(|m| {
                let Ok(glyph_id) = m.glyph_id();

                <::font_types::GlyphId as ::core::convert::From<::core::primitive::u16>>::from(
                  glyph_id,
                )
              }),
          );
        }
        None => {}
      }
    }
  }
}

#[derive(Clone)]
pub struct Cmap14Iter<'a> {
  subtable: Cmap14<'a>,
  selector_record: Option<VariationSelector>,
  default_uvs: Option<DefaultUvsIter<'a>>,
  non_default_uvs: Option<NonDefaultUvsIter<'a>>,
  cur_selector_ix: u64,
}

impl<'a> Cmap14Iter<'a> {
  #[must_use]
  fn new(subtable: Cmap14<'a>) -> core::result::Result<Self, console_core_never::Never> {
      let Ok((selector_record, default_uvs, non_default_uvs)) = subtable.selector(0);

      Ok(Self {
        subtable,
        selector_record,
        default_uvs: default_uvs.map(|records| {
          let Ok(iterator) = DefaultUvsIter::new(records);
          iterator
        }),
        non_default_uvs: non_default_uvs.map(|records| {
          let Ok(iterator) = NonDefaultUvsIter::new(records);
          iterator
        }),
        cur_selector_ix: 0,
      })
  }
}

impl Iterator for Cmap14Iter<'_> {
  type Item = (u32, u32, MapVariant);

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(records) = self.subtable.var_selector();
    let Ok(record_count) = console_core_number_conversion::fitted::<_, u64>(records.len());

    for _ in self.cur_selector_ix..=record_count {
      let selector_record = self.selector_record.as_ref()?;
      let Ok(var_selector) = selector_record.var_selector();
      let selector: u32 = <::core::primitive::u32 as ::core::convert::From<
        ::font_types::Uint24,
      >>::from(var_selector);

      match self.default_uvs.as_mut() {
        Some(default_uvs) => match default_uvs.next() {
          Some(codepoint) => {
            return Some((codepoint, selector, MapVariant::UseDefault));
          }
          None => {}
        },
        None => {}
      }

      match self.non_default_uvs.as_mut() {
        Some(non_default_uvs) => match non_default_uvs.next() {
          Some((codepoint, variant)) => {
            return Some((
              codepoint,
              selector,
              MapVariant::Variant(<::font_types::GlyphId as ::core::convert::From<
                ::font_types::GlyphId16,
              >>::from(variant)),
            ));
          }
          ::core::option::Option::None => {}
        },
        None => {}
      }

      match (1, &mut self.cur_selector_ix) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };

      let Ok((selector_record_2, default_uvs, non_default_uvs)) =
        self.subtable.selector(self.cur_selector_ix);
      self.selector_record = selector_record_2;
      self.default_uvs = default_uvs.map(|records_2| {
          let Ok(iterator) = DefaultUvsIter::new(records_2);
          iterator
        });
      self.non_default_uvs = non_default_uvs.map(|records_3| {
          let Ok(iterator) = NonDefaultUvsIter::new(records_3);
          iterator
        });
    }

    None
  }
}

#[derive(Clone)]
struct DefaultUvsIter<'a> {
  ranges: std::slice::Iter<'a, UnicodeRange>,
  cur_range: Range<u32>,
}

impl<'a> DefaultUvsIter<'a> {
  #[must_use]
  fn new(ranges: DefaultUvs<'a>) -> Result<Self, console_core_never::Never> {
    let Ok(records) = ranges.ranges();
    Ok(Self { ranges: records.iter(), cur_range: 0..0 })
  }
}

impl Iterator for DefaultUvsIter<'_> {
  type Item = u32;

  fn next(&mut self) -> Option<Self::Item> {
    match self.cur_range.next() {
      Some(codepoint) => return Some(codepoint),
      None => {}
    }

    let range = self.ranges.next()?;
    let Ok(start_value) = range.start_unicode_value();
    let start = u32::from(start_value);
    let Ok(additional) = range.additional_count();
    let end = start.wrapping_add(u32::from(additional)).wrapping_add(1);
    self.cur_range = start..end;
    self.cur_range.next()
  }
}

#[derive(Clone)]
struct NonDefaultUvsIter<'a> {
  iter: std::slice::Iter<'a, UvsMapping>,
}

impl<'a> NonDefaultUvsIter<'a> {
  #[must_use]
  fn new(uvs: NonDefaultUvs<'a>) -> Result<Self, console_core_never::Never> {
    let Ok(uvs_mapping) = uvs.uvs_mapping();
    Ok(Self { iter: uvs_mapping.iter() })
  }
}

impl Iterator for NonDefaultUvsIter<'_> {
  type Item = (u32, GlyphId16);

  fn next(&mut self) -> Option<Self::Item> {
    let mapping = self.iter.next()?;
    let Ok(unicode_value) = mapping.unicode_value();
    let codepoint: u32 = <::core::primitive::u32 as ::core::convert::From<
      ::font_types::Uint24,
    >>::from(unicode_value);
    let Ok(mapping_glyph_id) = mapping.glyph_id();
    let Ok(value_29) = GlyphId16::new(mapping_glyph_id);

    let glyph_id = value_29;
    let value_23 = glyph_id;

    Some((codepoint, value_23))
  }
}

#[cfg(test)]
mod tests {
  use font_test_data::{be_buffer, bebuffer::BeBuffer};

  use super::*;
  use crate::{FontRef, GlyphId, TableProvider};

  #[test]
  fn map_codepoints() -> Result<(), ReadError> {
    let font = FontRef::new(font_test_data::VAZIRMATN_VAR)?;
    let cmap = font.cmap()?;
    let Ok(()) = verify_mappings(&cmap, &[('A', Some(1)), ('À', Some(2)), ('`', Some(3)), ('B', None)]);

    let simple = FontRef::new(font_test_data::SIMPLE_GLYF)?;
    let simple_cmap = simple.cmap()?;
    let Ok(()) = verify_mappings(&simple_cmap, &[(' ', Some(1)), ('B', None)]);
    let Ok(()) = verify_mappings(&simple_cmap, &[(0xE_u32, Some(2))]);

    let Ok(format0_bytes) = cmap0_data();
    let Ok(format0_data) = FontData::new(&format0_bytes);
    let format0 = Cmap::read(format0_data)?;
    let Ok(()) = verify_mappings(
      &format0, &[(0, Some(0)), (b' ', Some(178)), (b'r', Some(193)), (b'X', Some(13)), (255u8, Some(3))]
    );

    let cmap6_data = be_buffer! {
        0u16,
        1u16,
        1u16,
        0u16,
        12u32,
        6u16,
        32u16,
        0u16,
        32u16,
        5u16,
        [10u16, 15, 7, 20, 4]
    };
    let Ok(format6_data) = FontData::new(&cmap6_data);
    let format6 = Cmap::read(format6_data)?;
    let Ok(()) = verify_mappings(
      &format6, &[(0u8, None), (31, None), (33, Some(15)), (35, Some(20)), (36, Some(4)), (50, None)]
    );
    Ok(())
  }

  fn verify_mappings<C: Into<u32> + Copy>(
    cmap: &Cmap<'_>,
    cases: &[(C, Option<u32>)],
  ) -> Result<(), Never> {
    for &(codepoint, expected) in cases {
      let expected_glyph = expected.map(|number| {
        let Ok(glyph) = GlyphId::new(number);
        glyph
      });
      assert_eq!(cmap.map_codepoint(codepoint), expected_glyph);
    }

    Ok(())
  }

  #[test]
  fn map_variants() -> ::core::result::Result<(), crate::test_support::Failure> {
    use super::MapVariant::*;
    let font = FontRef::new(font_test_data::CMAP14_FONT1)?;
    let cmap = font.cmap()?;
    let cmap14 = crate::test_support::fixture_value({ let Ok(explicit_value) = find_cmap14(&cmap); explicit_value }, "fixture value")?;
    let selector = '\u{e0100}';
    assert_eq!(cmap14.map_variant('a', selector), None);
    assert_eq!(cmap14.map_variant('\u{4e00}', selector), Some(UseDefault));
    assert_eq!(cmap14.map_variant('\u{4e06}', selector), Some(UseDefault));
    assert_eq!(
      cmap14.map_variant('\u{4e08}', selector),
      Some(Variant({
        let Ok(explicit_value) = GlyphId::new(25);
        explicit_value
      }))
    );
    assert_eq!(
      cmap14.map_variant('\u{4e09}', selector),
      Some(Variant({
        let Ok(explicit_value) = GlyphId::new(26);
        explicit_value
      }))
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  #[cfg(feature = "std")]
  fn cmap14_closure_glyphs() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::CMAP14_FONT1)?;
    let cmap = font.cmap()?;
    let Ok(mut unicodes) = IntSet::empty();
    let Ok(_previous_presence) = (unicodes).insert(0x4e08_u32);
    let Ok(_previous_presence) = (unicodes).insert(0xe0100_u32);

    let Ok(mut glyph_set) = IntSet::empty();
    let Ok(_previous_presence) = glyph_set.insert({
      let Ok(value) = GlyphId::new(18);
      value
    });
    cmap.closure_glyphs(&unicodes, &mut glyph_set);

    let Ok(value_54) = (glyph_set).len();

    assert_eq!(value_54, 2);
    let Ok(value_55) = (glyph_set).contains({
      let Ok(value) = GlyphId::new(18);
      value
    });

    assert_eq!(value_55, console_core_integer_sets::Presence::Present);
    let Ok(value_56) = (glyph_set).contains({
      let Ok(value) = GlyphId::new(25);
      value
    });

    assert_eq!(value_56, console_core_integer_sets::Presence::Present);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn cmap4_iter() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::VAZIRMATN_VAR)?;
    let cmap4 = crate::test_support::fixture_value({ let Ok(explicit_value) = find_cmap4(&match font.cmap() {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
    }); explicit_value }, "fixture value")?;
    let mut count = 0;

    for (codepoint, glyph_id) in cmap4.iter() {
      assert_eq!(cmap4.map_codepoint(codepoint), Some(glyph_id));
      count = ::core::primitive::i32::wrapping_add(count, 1);
    }

    assert_eq!(count, 4);
    let font_2 = FontRef::new(font_test_data::SIMPLE_GLYF)?;
    let cmap4_2 = crate::test_support::fixture_value({ let Ok(explicit_value) = find_cmap4(&match font_2.cmap() {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
    }); explicit_value }, "fixture value")?;
    let mut count_2 = 0;

    for (codepoint, glyph_id) in cmap4_2.iter() {
      assert_eq!(cmap4_2.map_codepoint(codepoint), Some(glyph_id));
      count_2 = ::core::primitive::i32::wrapping_add(count_2, 1);
    }

    assert_eq!(count_2, 3);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn cmap4_iter_explicit_notdef() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::VAZIRMATN_VAR)?;
    let cmap4 = crate::test_support::fixture_value({ let Ok(explicit_value) = find_cmap4(&match font.cmap() {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
    }); explicit_value }, "fixture value")?;
    let mut notdef_count = 0;

    for (_, glyph_id) in cmap4.iter() {
      match (i32::from(glyph_id == GlyphId::NOTDEF), &mut notdef_count) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    assert!(notdef_count > 0);
    assert_eq!(cmap4.map_codepoint(0xFFFF_u32), Some(GlyphId::NOTDEF));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn cmap4_iter_sparse_range() -> ::core::result::Result<(), crate::read::ReadError> {
    #[rustfmt::skip]
        let cmap4_data: &[u16] = &[

            4, 0, 0,

            4,

            0, 0, 0,

            262, 0xFFFF,

            0,

            259, 0xFFFF,

            0, 1,

            4, 0,

            236, 0, 0, 326,
        ];
    let mut buf = font_test_data::bebuffer::BeBuffer::default();

    for &word in cmap4_data {
      buf.extend([word]);
    }

    let cmap4 = Cmap4::read({
      let Ok(explicit_value) = FontData::new(&buf);
      explicit_value
    })?;
    let mappings = cmap4
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
    assert_eq!(mappings, &[(259, 236), (262, 326), (65535, 0)]);
    ::core::result::Result::Ok(())
  }

  const CMAP6_PAIRS: &[(u32, u32)] = &[
    (0x1723, 1),
    (0x1724, 2),
    (0x1725, 3),
    (0x1726, 4),
    (0x1727, 5),
  ];

  #[test]
  fn cmap6_map() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CMAP6)?;
    let cmap = font.cmap()?;

    let subtable = cmap.subtable(0)?;
    let cmap6 = match subtable {
      CmapSubtable::Format6(mapping) => mapping,
      CmapSubtable::Format0(_) | CmapSubtable::Format2(_) | CmapSubtable::Format4(_)
      | CmapSubtable::Format8(_) | CmapSubtable::Format10(_) | CmapSubtable::Format12(_)
      | CmapSubtable::Format13(_) | CmapSubtable::Format14(_) => {
        return Err(crate::test_support::Failure::from("expected a cmap format 6 subtable"));
      }
    };

    for (ch, gid) in CMAP6_PAIRS {
      assert_eq!(
        ({
          let fixture_value = crate::test_support::fixture_value(cmap6.map_codepoint(*ch), "fixture value")?;
          let Ok(value) = fixture_value.to_u32();
          value
        }),
        *gid
      );
    }

    let &(first_codepoint, _first_glyph) = crate::test_support::fixture_value(
      CMAP6_PAIRS.first(), "first cmap 6 mapping",
    )?;
    assert!(
      cmap6
        .map_codepoint(first_codepoint.wrapping_sub(1))
        .is_none()
    );
    let fixture_value = crate::test_support::fixture_value(CMAP6_PAIRS.last().copied(), "fixture value")?;
    assert!(
      cmap6
        .map_codepoint(::core::primitive::u32::wrapping_add(
          fixture_value.0,
          1
        ))
        .is_none()
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn cmap6_iter() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CMAP6)?;
    let cmap = font.cmap()?;

    let subtable = cmap.subtable(0)?;
    let cmap6 = match subtable {
      CmapSubtable::Format6(mapping) => mapping,
      CmapSubtable::Format0(_) | CmapSubtable::Format2(_) | CmapSubtable::Format4(_)
      | CmapSubtable::Format8(_) | CmapSubtable::Format10(_) | CmapSubtable::Format12(_)
      | CmapSubtable::Format13(_) | CmapSubtable::Format14(_) => {
        return Err(crate::test_support::Failure::from("expected a cmap format 6 subtable"));
      }
    };

    let pairs = cmap6
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
    assert_eq!(pairs, CMAP6_PAIRS);
    ::core::result::Result::Ok(())
  }

  const CMAP10_PAIRS: &[(u32, u32)] = &[(0x109423, 26), (0x109424, 27), (0x109425, 32)];

  #[test]
  fn cmap10_map() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CMAP10)?;
    let cmap = font.cmap()?;

    let subtable = cmap.subtable(0)?;
    let cmap10 = match subtable {
      CmapSubtable::Format10(mapping) => mapping,
      CmapSubtable::Format0(_) | CmapSubtable::Format2(_) | CmapSubtable::Format4(_)
      | CmapSubtable::Format6(_) | CmapSubtable::Format8(_) | CmapSubtable::Format12(_)
      | CmapSubtable::Format13(_) | CmapSubtable::Format14(_) => {
        return Err(crate::test_support::Failure::from("expected a cmap format 10 subtable"));
      }
    };

    for (ch, gid) in CMAP10_PAIRS {
      assert_eq!(
        ({
          let fixture_value = crate::test_support::fixture_value(cmap10.map_codepoint(*ch), "fixture value")?;
          let Ok(value) = fixture_value.to_u32();
          value
        }),
        *gid
      );
    }

    let &(first_codepoint, _first_glyph) = crate::test_support::fixture_value(
      CMAP10_PAIRS.first(), "first cmap 10 mapping",
    )?;
    assert!(
      cmap10
        .map_codepoint(first_codepoint.wrapping_sub(1))
        .is_none()
    );
    let fixture_value = crate::test_support::fixture_value(CMAP10_PAIRS.last().copied(), "fixture value")?;
    assert!(
      cmap10
        .map_codepoint(::core::primitive::u32::wrapping_add(
          fixture_value.0,
          1
        ))
        .is_none()
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn cmap10_iter() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CMAP10)?;
    let cmap = font.cmap()?;

    let subtable = cmap.subtable(0)?;
    let cmap10 = match subtable {
      CmapSubtable::Format10(mapping) => mapping,
      CmapSubtable::Format0(_) | CmapSubtable::Format2(_) | CmapSubtable::Format4(_)
      | CmapSubtable::Format6(_) | CmapSubtable::Format8(_) | CmapSubtable::Format12(_)
      | CmapSubtable::Format13(_) | CmapSubtable::Format14(_) => {
        return Err(crate::test_support::Failure::from("expected a cmap format 10 subtable"));
      }
    };

    let pairs = cmap10
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
    assert_eq!(pairs, CMAP10_PAIRS);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn cmap12_iter() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CMAP12_FONT1)?;
    let cmap12 = crate::test_support::fixture_value({ let Ok(explicit_value) = find_cmap12(&match font.cmap() {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
    }); explicit_value }, "fixture value")?;
    let mut count = 0;

    for (codepoint, glyph_id) in cmap12.iter() {
      assert_eq!(cmap12.map_codepoint(codepoint), Some(glyph_id));
      count = ::core::primitive::i32::wrapping_add(count, 1);
    }

    assert_eq!(count, 10);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn cmap12_iter_avoid_overflow() -> ::core::result::Result<(), crate::read::ReadError> {
    let data = be_buffer! {
        12u16,
        0u16,
        0u32,
        0u32,
        2u32,

        [0xFFFFFFFA_u32, 0xFFFFFFFC, 0],
        [0xFFFFFFFB_u32, 0xFFFFFFFF, 0]
    };
    let cmap12 = Cmap12::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*data))?;
    let _ = cmap12.iter().count();
    ::core::result::Result::Ok(())
  }

  #[test]
  fn cmap12_iter_avoid_timeout() -> ::core::result::Result<(), crate::read::ReadError> {
    let cmap12_data = be_buffer! {
        12u16,
        0u16,
        0u32,
        0u32,
        1u32,

        [170u32, 1330926671, 328960]
    };
    let cmap12 = Cmap12::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*cmap12_data))?;
    assert!(
      cmap12.iter_with_limits(CmapIterLimits::default()).count()
        <= ({
          let Ok(explicit_index) =
            console_core_number_conversion::index(::core::primitive::u64::wrapping_add(
              <u64 as ::core::convert::From<::core::primitive::char>>::from(char::MAX),
              1,
            ));
          explicit_index
        })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn cmap12_iter_avoid_timeout2() -> ::core::result::Result<(), crate::read::ReadError> {
    let cmap12_data = be_buffer! {
        12u16,
        0u16,
        0u32,
        0u32,
        3u32,

        [199u32, 16777271, 2],
        [262u32, 262, 3],
        [268u32, 268, 4]
    };
    let cmap12 = Cmap12::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*cmap12_data))?;

    const MAX_GLYPHS: u32 = 8;
    let limits = CmapIterLimits {
      glyph_count: MAX_GLYPHS,
      ..Default::default()
    };
    assert_eq!(
      cmap12.iter_with_limits(limits).count(),
      ({
        let Ok(explicit_index) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(MAX_GLYPHS);
          migrated_value
        });
        explicit_index
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn cmap12_iter_glyph_limit() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(FontRef::new(font_test_data::CMAP12_FONT1), "fixture value")?;
    let cmap12 = crate::test_support::fixture_value({ let fixture_value = crate::test_support::fixture_value(font.cmap(), "fixture value")?; let Ok(explicit_value) = find_cmap12(&fixture_value); explicit_value }, "fixture value")?;
    let mut limits = CmapIterLimits::default_for_font(&font);

    for glyph_count in 0..=11 {
      limits.glyph_count = glyph_count;
      assert_eq!(
        cmap12.iter_with_limits(limits).count(),
        ({
          let Ok(explicit_index) = console_core_number_conversion::index(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u64>(glyph_count);
              migrated_value
            })
            .saturating_sub(1),
          );
          explicit_index
        })
      );
    }
  ;

    Ok(())
  }

  #[test]
  fn cmap12_iter_range_clamping() -> ::core::result::Result<(), crate::read::ReadError> {
    let data = be_buffer! {
        12u16,
        0u16,
        0u32,
        0u32,
        2u32,

        [0u32, 16777215, 0],
        [255u32, 0xFFFFFFFF, 0]
    };
    let cmap12 = Cmap12::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*data))?;
    let Ok(groups) = cmap12.groups();
    let ranges = groups
      .iter()
      .map(|group| {
        let Ok(start) = group.start_char_code();
        let Ok(end) = group.end_char_code();

        (start, end)
      })
      .collect::<Vec<_>>();

    assert_eq!(ranges, &[(0, 16777215), (255, u32::MAX)]);

    let limits = CmapIterLimits {
      glyph_count: u32::MAX,
      ..Default::default()
    };
    assert!(
      cmap12.iter_with_limits(limits).count()
        <= ({
          let Ok(explicit_index) =
            console_core_number_conversion::index(::core::primitive::u64::wrapping_add(
              <u64 as ::core::convert::From<::core::primitive::char>>::from(char::MAX),
              1,
            ));
          explicit_index
        })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn cmap12_iter_explicit_notdef() -> ::core::result::Result<(), crate::test_support::Failure> {
    let data = be_buffer! {
        12u16,
        0u16,
        0u32,
        0u32,
        1u32,

        [0_u32, 1_u32, 0]
    };
    let cmap12 = Cmap12::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*data))?;

    for (i, (codepoint, glyph_id)) in cmap12.iter().enumerate() {
      assert_eq!(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(codepoint);
          migrated_value
        }),
        ({
          let Ok(explicit_index) = console_core_number_conversion::fitted::<_, u64>(i);
          explicit_index
        })
      );
      assert_eq!(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>({
            let Ok(explicit_value) = glyph_id.to_u32();
            explicit_value
          });
          migrated_value
        }),
        ({
          let Ok(explicit_index) = console_core_number_conversion::fitted::<_, u64>(i);
          explicit_index
        })
      );
    }

    let fixture_value = crate::test_support::fixture_value(cmap12.iter().next(), "fixture value")?;
    assert_eq!(fixture_value.1, GlyphId::NOTDEF);
    ::core::result::Result::Ok(())
  }

  #[must_use]
  fn cmap13_data() -> core::result::Result<Vec<u8>, console_core_never::Never> {
      let data = be_buffer! {
          13u16,
          0u16,
          0u32,
          0u32,
          2u32,

          [0u32, 8, 20],
          [42u32, 46u32, 30]
      };

      Ok(data.to_vec())
  }

  #[test]
  fn cmap13_map() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(data) = cmap13_data();
    let cmap13 = crate::test_support::fixture_value(Cmap13::read({
      let Ok(explicit_value) = FontData::new(&data);
      explicit_value
    }), "fixture value")?;

    for ch in 0u32..=8 {
      assert_eq!(
        cmap13.map_codepoint(ch),
        Some({
          let Ok(value) = GlyphId::new(20);
          value
        })
      );
    }

    for ch in 9u32..42 {
      assert_eq!(cmap13.map_codepoint(ch), None);
    }

    for ch in 42u32..=46 {
      assert_eq!(
        cmap13.map_codepoint(ch),
        Some({
          let Ok(value) = GlyphId::new(30);
          value
        })
      );
    }

    for ch in 47u32..1024 {
      assert_eq!(cmap13.map_codepoint(ch), None);
    }
  ;

    Ok(())
  }

  #[test]
  fn cmap13_iter() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(data) = cmap13_data();
    let cmap13 = crate::test_support::fixture_value(Cmap13::read({
      let Ok(explicit_value) = FontData::new(&data);
      explicit_value
    }), "fixture value")?;

    for (ch, gid) in cmap13.iter() {
      assert_eq!(cmap13.map_codepoint(ch), Some(gid));
    }
  ;

    Ok(())
  }

  #[test]
  fn cmap14_iter() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CMAP14_FONT1)?;
    let cmap14 = crate::test_support::fixture_value({ let Ok(explicit_value) = find_cmap14(&match font.cmap() {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
    }); explicit_value }, "fixture value")?;
    let mut count = 0;

    for (codepoint, selector, mapping) in cmap14.iter() {
      assert_eq!(cmap14.map_variant(codepoint, selector), Some(mapping));
      count = ::core::primitive::i32::wrapping_add(count, 1);
    }

    assert_eq!(count, 7);
    ::core::result::Result::Ok(())
  }

  #[must_use]
  fn find_cmap4<'a>(cmap: &Cmap<'a>) -> core::result::Result<Option<Cmap4<'a>>, console_core_never::Never> {
      let Ok(encoding_records) = cmap.encoding_records();

      Ok(encoding_records
        .iter()
        .filter_map(|record| {
          match record.subtable({
            let Ok(explicit_value) = cmap.offset_data();
            explicit_value
          }) {
            ::core::result::Result::Ok(value___) => ::core::option::Option::Some(value___),
            ::core::result::Result::Err(_unreadable_font_data_) => ::core::option::Option::None,
          }
        })
        .find_map(|subtable| match subtable {
          CmapSubtable::Format4(cmap4) => Some(cmap4),
          CmapSubtable::Format0(_)
          | CmapSubtable::Format2(_)
          | CmapSubtable::Format6(_)
          | CmapSubtable::Format8(_)
          | CmapSubtable::Format10(_)
          | CmapSubtable::Format12(_)
          | CmapSubtable::Format13(_)
          | CmapSubtable::Format14(_) => None,
        }))
  }

  #[must_use]
  fn find_cmap12<'a>(cmap: &Cmap<'a>) -> core::result::Result<Option<Cmap12<'a>>, console_core_never::Never> {
      let Ok(encoding_records) = cmap.encoding_records();

      Ok(encoding_records
        .iter()
        .filter_map(|record| {
          match record.subtable({
            let Ok(explicit_value) = cmap.offset_data();
            explicit_value
          }) {
            ::core::result::Result::Ok(value___) => ::core::option::Option::Some(value___),
            ::core::result::Result::Err(_unreadable_font_data_) => ::core::option::Option::None,
          }
        })
        .find_map(|subtable| match subtable {
          CmapSubtable::Format12(cmap12) => Some(cmap12),
          CmapSubtable::Format0(_)
          | CmapSubtable::Format2(_)
          | CmapSubtable::Format4(_)
          | CmapSubtable::Format6(_)
          | CmapSubtable::Format8(_)
          | CmapSubtable::Format10(_)
          | CmapSubtable::Format13(_)
          | CmapSubtable::Format14(_) => None,
        }))
  }

  #[must_use]
  fn find_cmap14<'a>(cmap: &Cmap<'a>) -> core::result::Result<Option<Cmap14<'a>>, console_core_never::Never> {
      let Ok(encoding_records) = cmap.encoding_records();

      Ok(encoding_records
        .iter()
        .filter_map(|record| {
          match record.subtable({
            let Ok(explicit_value) = cmap.offset_data();
            explicit_value
          }) {
            ::core::result::Result::Ok(value___) => ::core::option::Option::Some(value___),
            ::core::result::Result::Err(_unreadable_font_data_) => ::core::option::Option::None,
          }
        })
        .find_map(|subtable| match subtable {
          CmapSubtable::Format14(cmap14) => Some(cmap14),
          CmapSubtable::Format0(_)
          | CmapSubtable::Format2(_)
          | CmapSubtable::Format4(_)
          | CmapSubtable::Format6(_)
          | CmapSubtable::Format8(_)
          | CmapSubtable::Format10(_)
          | CmapSubtable::Format12(_)
          | CmapSubtable::Format13(_) => None,
        }))
  }

  #[test]
  fn cmap4_bad_data() -> ::core::result::Result<(), crate::test_support::Failure> {
    let buf = font_test_data::cmap::repetitive_cmap4()?;
    let cmap4 = crate::test_support::fixture_value(Cmap4::read({
      let Ok(explicit_value) = FontData::new(&buf);
      explicit_value
    }), "fixture value")?;

    assert_eq!(
      (6..=64).collect::<Vec<_>>(),
      cmap4.iter().map(|(cp, _)| cp).collect::<Vec<_>>()
    );

    Ok(())
  }

  #[must_use]
  fn cmap0_data() -> core::result::Result<BeBuffer, console_core_never::Never> {
      Ok(be_buffer! {

          0u16,

          1u16,

          1u16,

          0u16,

          12u32,

          0u16,

          274u16,

          0u16,

          [0u8, 249, 32, 2, 198, 23, 1, 4, 26, 36,
          171, 168, 69, 151, 208, 238, 226, 153, 161, 138,
          160, 130, 169, 223, 162, 207, 146, 227, 111, 248,
          163, 79, 178, 27, 50, 234, 213, 57, 45, 63,
          103, 186, 30, 105, 131, 118, 35, 140, 51, 211,
          75, 172, 56, 71, 137, 99, 22, 76, 61, 125,
          39, 8, 177, 117, 108, 97, 202, 92, 49, 134,
          93, 43, 80, 66, 84, 54, 180, 113, 11, 176,
          229, 48, 47, 17, 124, 40, 119, 21, 13, 133,
          181, 224, 33, 128, 44, 46, 38, 24, 65, 152,
          197, 225, 102, 251, 157, 126, 182, 242, 28, 184,
          90, 170, 201, 144, 193, 189, 250, 142, 77, 221,
          81, 164, 154, 60, 37, 200, 12, 53, 219, 89,
          31, 209, 188, 179, 253, 220, 127, 18, 19, 64,
          20, 141, 98, 173, 55, 194, 70, 107, 228, 104,
          10, 9, 15, 217, 255, 222, 196, 236, 67, 165,
          5, 143, 149, 100, 91, 95, 135, 235, 145, 204,
          72, 114, 246, 82, 245, 233, 106, 158, 185, 212,
          86, 243, 16, 195, 123, 190, 120, 187, 132, 139,
          192, 239, 110, 183, 240, 214, 166, 41, 59, 231,
          42, 94, 244, 83, 121, 25, 215, 96, 73, 87,
          174, 136, 62, 206, 156, 175, 230, 150, 116, 147,
          68, 122, 78, 112, 6, 167, 232, 254, 52, 34,
          191, 85, 241, 14, 216, 155, 29, 101, 115, 210,
          252, 218, 129, 247, 203, 159, 109, 74, 7, 58,
          237, 199, 88, 205, 148, 3]
      })
  }

  #[test]
  fn best_subtable_full() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::VORG)?;
    let cmap = font.cmap()?;
    let (index, record, _) = crate::test_support::fixture_value(cmap.best_subtable(), "fixture value")?;
    assert_eq!(
      (index, record.platform_id(), record.encoding_id()),
      (
        3,
        Ok(PlatformId::Windows),
        Ok(WINDOWS_UNICODE_FULL_ENCODING)
      )
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn best_subtable_bmp() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CMAP12_FONT1)?;
    let cmap = font.cmap()?;
    let (index, record, _) = crate::test_support::fixture_value(cmap.best_subtable(), "fixture value")?;
    assert_eq!(
      (index, record.platform_id(), record.encoding_id()),
      (0, Ok(PlatformId::Windows), Ok(WINDOWS_UNICODE_BMP_ENCODING))
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn best_subtable_symbol() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CMAP4_SYMBOL_PUA)?;
    let cmap = font.cmap()?;
    let (index, record, _) = crate::test_support::fixture_value(cmap.best_subtable(), "fixture value")?;
    assert_eq!(record.legacy_encoding(), Ok(LegacyEncoding::WindowsSymbol));
    assert_eq!(
      (index, record.platform_id(), record.encoding_id()),
      (0, Ok(PlatformId::Windows), Ok(WINDOWS_SYMBOL_ENCODING))
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn uvs_subtable() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CMAP14_FONT1)?;
    let cmap = font.cmap()?;
    let (index, _) = crate::test_support::fixture_value(cmap.uvs_subtable(), "fixture value")?;
    assert_eq!(index, 0);
    ::core::result::Result::Ok(())
  }
}
