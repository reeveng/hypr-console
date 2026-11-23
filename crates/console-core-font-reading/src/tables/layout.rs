//! OpenType Layout common table formats

#[cfg(feature = "std")]
mod closure;

mod feature;
mod lookup_flag;
mod script;

use core::cmp::Ordering;

pub use lookup_flag::LookupFlag;
pub use script::{ScriptTags, SelectedScript, UNICODE_TO_NEW_OPENTYPE_SCRIPT_TAGS};

use super::variations::DeltaSetIndex;

#[cfg(feature = "std")]
use crate::collections::IntSet;

#[cfg(feature = "std")]
pub(crate) use closure::{
  ContextFormat1, ContextFormat2, ContextFormat3, LayoutLookupList, LookupClosure,
  LookupClosureCtx, MAX_LOOKUP_VISIT_COUNT, MAX_NESTING_LEVEL, TraversalBudget, VisitDecision,
};

#[cfg(feature = "std")]
pub use closure::Intersect;

#[cfg(test)]
mod spec_tests;

include!("../../generated/generated_layout.rs");

impl<'a, T: FontRead<'a>> Lookup<'a, T> {
  pub fn get_subtable(&self, offset: Offset16) -> Result<T, ReadError> {
    self.resolve_offset(offset)
  }

  #[cfg(feature = "experimental_traverse")]
  #[must_use]
  fn traverse_lookup_flag(
    &self,
  ) -> core::result::Result<traversal::FieldType<'a>, console_core_never::Never> {
    let Ok(lookup_flag) = self.lookup_flag();

    Ok(<crate::traversal::FieldType<'_> as ::core::convert::From<
      ::core::result::Result<::core::primitive::u16, ::console_core_never::Never>,
    >>::from(lookup_flag.to_bits()))
  }
}

pub trait ExtensionLookup<'a, T: FontRead<'a>>: FontRead<'a> {
  fn extension(&self) -> Result<T, ReadError>;
}

pub enum Subtables<'a, T: FontRead<'a>, Ext: ExtensionLookup<'a, T>> {
  Subtable(ArrayOfOffsets<'a, T>),
  Extension(ArrayOfOffsets<'a, Ext>),
}

impl<'a, T: FontRead<'a> + 'a, Ext: ExtensionLookup<'a, T> + 'a> Subtables<'a, T, Ext> {
  #[must_use]
  pub(crate) fn new(
    offsets: &'a [BigEndian<Offset16>],
    data: FontData<'a>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(value_1) = ArrayOfOffsets::new(offsets, data, ());

    Ok(Subtables::Subtable(value_1))
  }

  #[must_use]
  pub(crate) fn new_ext(
    offsets: &'a [BigEndian<Offset16>],
    data: FontData<'a>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(value_2) = ArrayOfOffsets::new(offsets, data, ());

    Ok(Subtables::Extension(value_2))
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Subtables::len` method returns `u64` directly in the existing parser interface"
    )
  )]
  pub fn len(&self) -> u64 {
    match self {
      Subtables::Subtable(inner) => {
        let Ok(value_36) = inner.len();

        value_36
      }
      Subtables::Extension(inner) => {
        let Ok(value_37) = inner.len();

        value_37
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Subtables::is_empty` method returns `bool` directly in the existing parser interface"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(
      explicit007_no_bool_return,
      reason = "the published `Subtables::is_empty` method retains the Boolean result used by existing predicate callers and implementations"
    )
  )]
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  pub fn get(&self, idx: u64) -> Result<T, ReadError> {
    match self {
      Subtables::Subtable(inner) => inner.get(idx),
      Subtables::Extension(inner) => inner.get(idx).and_then(|ext| ext.extension()),
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Subtables::iter` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn iter(&self) -> impl Iterator<Item = Result<T, ReadError>> + 'a + use<'a, T, Ext> {
    let (left, right) = match self {
      Subtables::Subtable(inner) => {
        let Ok(items) = inner.iter();

        (Some(items), None)
      }
      Subtables::Extension(inner) => {
        let Ok(items) = inner.iter();

        (
          None,
          Some(items.map(|ext| ext.and_then(|ext_2| ext_2.extension()))),
        )
      }
    };
    left
      .into_iter()
      .flatten()
      .chain(right.into_iter().flatten())
  }
}

pub enum FeatureParams<'a> {
  StylisticSet(StylisticSetParams<'a>),
  Size(SizeParams<'a>),
  CharacterVariant(CharacterVariantParams<'a>),
}

impl ReadArgs for FeatureParams<'_> {
  type Arguments = Tag;
}

impl<'a> FontReadWithArgs<'a> for FeatureParams<'a> {
  fn read_with_args(bytes: FontData<'a>, tag: &Tag) -> Result<FeatureParams<'a>, ReadError> {
    let Ok(size) = Tag::new(b"size");

    match ::core::primitive::bool::then_some(*tag == size, ()) {
      ::core::option::Option::Some(()) => SizeParams::read(bytes).map(Self::Size),
      ::core::option::Option::None => {
        let Ok(raw) = tag.to_be_bytes();

        match raw.get(..2) {
          Some(b"ss") => StylisticSetParams::read(bytes).map(Self::StylisticSet),
          Some(b"cv") => CharacterVariantParams::read(bytes).map(Self::CharacterVariant),
          Some(_) | None => Err(ReadError::InvalidFormat(0xdead)),
        }
      }
    }
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for FeatureParams<'a> {
  fn type_name(&self) -> Result<&str, Never> {
    Ok(match self {
      FeatureParams::StylisticSet(table) => {
        let Ok(type_name) = table.type_name();

        type_name
      }
      FeatureParams::Size(table) => {
        let Ok(type_name) = table.type_name();

        type_name
      }
      FeatureParams::CharacterVariant(table) => {
        let Ok(type_name) = table.type_name();

        type_name
      }
    })
  }

  fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
    Ok(match self {
      FeatureParams::StylisticSet(table) => {
        let Ok(field) = table.get_field(idx);

        field
      }
      FeatureParams::Size(table) => {
        let Ok(field) = table.get_field(idx);

        field
      }
      FeatureParams::CharacterVariant(table) => {
        let Ok(field) = table.get_field(idx);

        field
      }
    })
  }
}

impl FeatureTableSubstitutionRecord {
  pub fn alternate_feature<'a>(&self, data: FontData<'a>) -> Result<Feature<'a>, ReadError> {
    let Ok(value_6) = Tag::new(b"NULL");

    let Ok(alternate_feature_offset) = self.alternate_feature_offset();

    alternate_feature_offset.resolve_with_args(data, &value_6)
  }
}

#[must_use]
fn bit_storage(v: u32) -> core::result::Result<u32, console_core_never::Never> {
  Ok(::core::primitive::u32::wrapping_sub(
    u32::BITS,
    v.leading_zeros(),
  ))
}

impl<'a> CoverageTable<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CoverageTable::iter` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn iter(&self) -> impl Iterator<Item = GlyphId16> + 'a + use<'a> {
    let (iter1, iter2) = match self {
      CoverageTable::Format1(t) => {
        let Ok(glyph_array) = t.glyph_array();

        (
          Some(glyph_array.iter().map(|g| {
            let Ok(value_67) = g.get();

            value_67
          })),
          None,
        )
      }
      CoverageTable::Format2(t) => {
        let Ok(range_records) = t.range_records();
        let iter = range_records.iter().flat_map(RangeRecord::iter);
        (None, Some(iter))
      }
    };

    iter1
      .into_iter()
      .flatten()
      .chain(iter2.into_iter().flatten())
  }

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CoverageTable::get` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn get(&self, gid: impl Into<GlyphId>) -> Option<u16> {
    match self {
      CoverageTable::Format1(sub) => sub.get(gid),
      CoverageTable::Format2(sub) => sub.get(gid),
    }
  }

  #[cfg(feature = "std")]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CoverageTable::intersects` method returns `bool` directly in the existing parser interface"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(
      explicit007_no_bool_return,
      reason = "the published `CoverageTable::intersects` method retains the Boolean result used by existing predicate callers and implementations"
    )
  )]
  pub fn intersects(&self, glyphs: &IntSet<GlyphId>) -> bool {
    match self {
      CoverageTable::Format1(sub) => {
        matches!(sub.overlap(glyphs), Ok(crate::Overlap::Intersecting))
      }
      CoverageTable::Format2(sub) => {
        matches!(sub.overlap(glyphs), Ok(crate::Overlap::Intersecting))
      }
    }
  }

  #[cfg(feature = "std")]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CoverageTable::intersect_set` method returns `IntSet` directly in the existing parser interface"
    )
  )]
  pub fn intersect_set(&self, glyphs: &IntSet<GlyphId>) -> IntSet<GlyphId> {
    match self {
      CoverageTable::Format1(sub) => {
        let Ok(intersect_set) = sub.intersect_set(glyphs);

        intersect_set
      }
      CoverageTable::Format2(sub) => {
        let Ok(intersect_set) = sub.intersect_set(glyphs);

        intersect_set
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CoverageTable::population` method returns `u64` directly in the existing parser interface"
    )
  )]
  pub fn population(&self) -> u64 {
    match self {
      CoverageTable::Format1(sub) => sub.population(),
      CoverageTable::Format2(sub) => sub.population(),
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CoverageTable::cost` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn cost(&self) -> u32 {
    match self {
      CoverageTable::Format1(sub) => sub.cost(),
      CoverageTable::Format2(sub) => sub.cost(),
    }
  }
}

impl CoverageFormat1<'_> {
  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CoverageFormat1::get` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn get(&self, gid: impl Into<GlyphId>) -> Option<u16> {
    let gid16: GlyphId16 =
      (match <_ as ::core::convert::Into<::font_types::GlyphId>>::into(gid).try_into() {
        Ok(value) => Some(value),
        Err(_glyph_outside_font_range) => None,
      })?;
    let be_glyph: BigEndian<GlyphId16> =
      <::font_types::BigEndian<::font_types::GlyphId16> as ::core::convert::From<
        ::font_types::GlyphId16,
      >>::from(gid16);
    let Ok(glyph_array) = self.glyph_array();

    match glyph_array.binary_search(&be_glyph) {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_insertion_position) => ::core::option::Option::None,
    }
    .map(|idx| {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(idx);
      migrated_value
    })
  }

  #[cfg(feature = "std")]
  #[must_use]
  fn overlap(&self, glyphs: &IntSet<GlyphId>) -> Result<crate::Overlap, Never> {
    let glyph_count = {
      let Ok(own_glyph_count) = self.glyph_count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(own_glyph_count);
      migrated_value
    };
    let Ok(value_43) = glyphs.len();

    let intersects = match ::core::primitive::bool::then_some(
      glyph_count
        > ::core::primitive::u32::wrapping_mul(
          {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(value_43);
            migrated_value
          },
          self.cost(),
        ),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(mut value_66) = glyphs.iter();

        value_66.any(|g| self.get(g).is_some())
      }
      ::core::option::Option::None => {
        let Ok(glyph_array) = self.glyph_array();

        glyph_array.iter().any(|g| {
          let Ok(value_44) = g.get();

          let Ok(value_78) = glyphs.contains(GlyphId::from(value_44));

          value_78 == console_core_integer_sets::Presence::Present
        })
      }
    };
    Ok(crate::Overlap::from(intersects))
  }

  #[cfg(feature = "std")]
  #[must_use]
  fn intersect_set(
    &self,
    glyphs: &IntSet<GlyphId>,
  ) -> core::result::Result<IntSet<GlyphId>, console_core_never::Never> {
    let glyph_count = {
      let Ok(own_glyph_count) = self.glyph_count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(own_glyph_count);
      migrated_value
    };
    let Ok(value_80) = glyphs.len();

    Ok(
      match ::core::primitive::bool::then_some(
        glyph_count
          > ::core::primitive::u32::wrapping_mul(
            {
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(value_80);
              migrated_value
            },
            self.cost(),
          ),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let Ok(value_5) = glyphs.iter();

          value_5.filter_map(|g| self.get(g).map(|_| g)).collect()
        }
        ::core::option::Option::None => {
          let Ok(glyph_array) = self.glyph_array();

          glyph_array
            .iter()
            .filter(|g| {
              let Ok(value_49) = g.get();

              let Ok(value_79) = glyphs.contains(GlyphId::from(value_49));

              value_79 == console_core_integer_sets::Presence::Present
            })
            .map(|g| {
              let Ok(value_72) = g.get();

              GlyphId::from(value_72)
            })
            .collect()
        }
      },
    )
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CoverageFormat1::population` method returns `u64` directly in the existing parser interface"
    )
  )]
  pub fn population(&self) -> u64 {
    {
      let Ok(glyph_count) = self.glyph_count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(glyph_count);
      migrated_value
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CoverageFormat1::cost` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn cost(&self) -> u32 {
    {
      let Ok(explicit_value) = bit_storage({
        let Ok(glyph_count) = self.glyph_count();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(glyph_count);
        migrated_value
      });
      explicit_value
    }
  }
}

impl CoverageFormat2<'_> {
  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CoverageFormat2::get` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn get(&self, gid: impl Into<GlyphId>) -> Option<u16> {
    let gid_2: GlyphId16 =
      (match <_ as ::core::convert::Into<::font_types::GlyphId>>::into(gid).try_into() {
        Ok(value) => Some(value),
        Err(_glyph_outside_font_range) => None,
      })?;
    let Ok(range_records) = self.range_records();

    match range_records.binary_search_by(|rec| {
      let Ok(end_glyph_id) = rec.end_glyph_id();

      match ::core::primitive::bool::then_some(end_glyph_id < gid_2, ()) {
        ::core::option::Option::Some(()) => Ordering::Less,
        ::core::option::Option::None => {
          let Ok(start_glyph_id) = rec.start_glyph_id();

          match ::core::primitive::bool::then_some(start_glyph_id > gid_2, ()) {
            ::core::option::Option::Some(()) => Ordering::Greater,
            ::core::option::Option::None => Ordering::Equal,
          }
        }
      }
    }) {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_insertion_position) => ::core::option::Option::None,
    }
    .and_then(|idx| {
      let rec = range_records.get(idx)?;

      let Ok(value_7) = gid_2.to_u16();
      let Ok(start_glyph_id) = rec.start_glyph_id();
      let Ok(value_8) = start_glyph_id.to_u16();

      let Ok(start_coverage_index) = rec.start_coverage_index();

      start_coverage_index.checked_add(::core::primitive::u16::wrapping_sub(value_7, value_8))
    })
  }

  #[cfg(feature = "std")]
  #[must_use]
  fn overlap(&self, glyphs: &IntSet<GlyphId>) -> Result<crate::Overlap, Never> {
    let range_count = {
      let Ok(own_range_count) = self.range_count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(own_range_count);
      migrated_value
    };
    let Ok(value_81) = glyphs.len();

    let intersects = match ::core::primitive::bool::then_some(
      range_count
        > ::core::primitive::u32::wrapping_mul(
          {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(value_81);
            migrated_value
          },
          self.cost(),
        ),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(mut value_82) = glyphs.iter();

        value_82.any(|g| self.get(g).is_some())
      }
      ::core::option::Option::None => {
        let Ok(range_records) = self.range_records();

        range_records.iter().any(|record| record.intersects(glyphs))
      }
    };
    Ok(crate::Overlap::from(intersects))
  }

  #[cfg(feature = "std")]
  #[must_use]
  fn intersect_set(
    &self,
    glyphs: &IntSet<GlyphId>,
  ) -> core::result::Result<IntSet<GlyphId>, console_core_never::Never> {
    let range_count = {
      let Ok(own_range_count) = self.range_count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(own_range_count);
      migrated_value
    };
    let Ok(value_83) = glyphs.len();

    Ok(
      match ::core::primitive::bool::then_some(
        range_count
          > ::core::primitive::u32::wrapping_mul(
            {
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(value_83);
              migrated_value
            },
            self.cost(),
          ),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let Ok(value_9) = glyphs.iter();

          value_9.filter_map(|g| self.get(g).map(|_| g)).collect()
        }
        ::core::option::Option::None => {
          let Ok(mut out) = IntSet::empty();
          let mut last = GlyphId16::from(0);

          let Ok(range_records) = self.range_records();

          for record in range_records {
            let Ok(start_glyph) = record.start_glyph_id();

            match ::core::primitive::bool::then_some(start_glyph < last, ()) {
              ::core::option::Option::Some(()) => {
                break;
              }
              ::core::option::Option::None => {}
            }

            let Ok(end) = record.end_glyph_id();
            last = end;

            let Ok(()) = collect_coverage_range(
              glyphs,
              &mut out,
              GlyphId::from(start_glyph)..=GlyphId::from(end),
            );
          }

          out
        }
      },
    )
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CoverageFormat2::population` method returns `u64` directly in the existing parser interface"
    )
  )]
  pub fn population(&self) -> u64 {
    let Ok(range_records) = self.range_records();

    range_records.iter().fold(0, |acc, record| {
      ::core::primitive::u64::wrapping_add(acc, record.population())
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CoverageFormat2::cost` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn cost(&self) -> u32 {
    {
      let Ok(explicit_value) = bit_storage({
        let Ok(range_count) = self.range_count();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(range_count);
        migrated_value
      });
      explicit_value
    }
  }
}

impl RangeRecord {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `RangeRecord::iter` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn iter(&self) -> impl Iterator<Item = GlyphId16> + '_ {
    let Ok(start_glyph_id) = self.start_glyph_id();
    let Ok(value_47) = start_glyph_id.to_u16();
    let Ok(end_glyph_id) = self.end_glyph_id();
    let Ok(value_48) = end_glyph_id.to_u16();

    let value_11 = value_47..=value_48;

    value_11.map(|argument| {
      let Ok(value_73) = GlyphId16::new(argument);

      value_73
    })
  }

  #[cfg(feature = "std")]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `RangeRecord::intersects` method returns `bool` directly in the existing parser interface"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(
      explicit007_no_bool_return,
      reason = "the published `RangeRecord::intersects` method retains the Boolean result used by existing predicate callers and implementations"
    )
  )]
  pub fn intersects(&self, glyphs: &IntSet<GlyphId>) -> bool {
    let Ok(start_glyph_id) = self.start_glyph_id();
    let Ok(value_12) = glyphs.intersects_range(
      GlyphId::from(start_glyph_id)..=GlyphId::from({
        let Ok(explicit_value) = self.end_glyph_id();
        explicit_value
      }),
    );

    value_12 == console_core_integer_sets::Overlap::Intersecting
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `RangeRecord::population` method returns `u64` directly in the existing parser interface"
    )
  )]
  pub fn population(&self) -> u64 {
    let Ok(start_glyph_id) = self.start_glyph_id();
    let Ok(value_13) = start_glyph_id.to_u32();

    let start = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_13);
      migrated_value
    };
    let Ok(end_glyph_id) = self.end_glyph_id();
    let Ok(value_14) = end_glyph_id.to_u32();

    let end = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_14);
      migrated_value
    };

    match ::core::primitive::bool::then_some(start > end, ()) {
      ::core::option::Option::Some(()) => 0,
      ::core::option::Option::None => {
        ::core::primitive::u64::wrapping_add(::core::primitive::u64::wrapping_sub(end, start), 1)
      }
    }
  }
}

impl DeltaFormat {
  #[must_use]
  pub(crate) fn value_count(self, sizes: core::ops::RangeInclusive<u16>) -> Result<u64, Never> {
    let (start_size, end_size) = sizes.into_inner();
    let range_len = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
        end_size.saturating_add(1).saturating_sub(start_size),
      );
      migrated_value
    };
    let val_per_word = match self {
      DeltaFormat::Local2BitDeltas => 8,
      DeltaFormat::Local4BitDeltas => 4,
      DeltaFormat::Local8BitDeltas => 2,
      DeltaFormat::VariationIndex | DeltaFormat::Unknown => return Ok(0),
    };

    Ok(range_len.div_ceil(val_per_word))
  }
}

impl From<DeltaFormat> for i64 {
  fn from(value: DeltaFormat) -> Self {
    let Ok(raw) = value.discriminant_value();
    i64::from(raw)
  }
}

#[cfg(feature = "std")]
fn class_glyph_span(glyphs: &IntSet<GlyphId>) -> Result<Option<(GlyphId, GlyphId)>, Never> {
  let Ok(first) = glyphs.first();
  let Ok(last) = glyphs.last();

  Ok(first.zip(last))
}

impl<'a> ClassDefFormat1<'a> {
  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDefFormat1::get` method returns `u16` directly in the existing parser interface"
    )
  )]
  pub fn get(&self, gid: impl Into<GlyphId>) -> u16 {
    let Ok(value_15) = <_ as ::core::convert::Into<::font_types::GlyphId>>::into(gid).to_u32();
    let Ok(start_glyph_id) = self.start_glyph_id();
    let Ok(value_16) = start_glyph_id.to_u32();

    let idx = match value_15.checked_sub(value_16) {
      Some(idx) => idx,
      None => {
        return 0;
      }
    };
    let Ok(value_68) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
      migrated_value
    });

    let Ok(class_value_array) = self.class_value_array();

    match class_value_array.get(value_68) {
      Some(class) => {
        let Ok(value_74) = class.get();

        value_74
      }
      None => 0,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDefFormat1::iter` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn iter(&self) -> impl Iterator<Item = (GlyphId16, u16)> + 'a + use<'a> {
    let Ok(start) = self.start_glyph_id();
    let Ok(class_value_array) = self.class_value_array();

    class_value_array.iter().enumerate().map(move |(i, val)| {
      let Ok(value_18) = start.to_u16();

      let gid = value_18.saturating_add({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(i);
        migrated_value
      });
      let Ok(value_41) = GlyphId16::new(gid);

      let Ok(value_75) = val.get();

      (value_41, value_75)
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDefFormat1::population` method returns `u64` directly in the existing parser interface"
    )
  )]
  pub fn population(&self) -> u64 {
    {
      let Ok(glyph_count) = self.glyph_count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(glyph_count);
      migrated_value
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDefFormat1::cost` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn cost(&self) -> u32 {
    1
  }

  #[cfg(feature = "std")]
  #[must_use]
  fn intersect_classes(
    &self,
    glyphs: &IntSet<GlyphId>,
  ) -> core::result::Result<IntSet<u16>, console_core_never::Never> {
    let Ok(mut out) = IntSet::empty();
    let Ok(span) = class_glyph_span(glyphs);
    let (first, last) = match span {
      Some(endpoints) => endpoints,
      None => return Ok(out),
    };
    let Ok(first_glyph) = first.to_u32();
    let Ok(last_glyph) = last.to_u32();

    let Ok(start_glyph_id) = self.start_glyph_id();
    let Ok(start_glyph) = start_glyph_id.to_u32();
    let Ok(glyph_count) = self.glyph_count();
    let end_glyph = ::core::primitive::u32::wrapping_sub(
      ::core::primitive::u32::wrapping_add(start_glyph, {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(glyph_count);
        migrated_value
      }),
      1,
    );

    match ::core::primitive::bool::then_some(
      first_glyph < start_glyph || last_glyph > end_glyph,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(_previous_presence) = out.insert(0);
      }
      ::core::option::Option::None => {}
    }

    let Ok(class_values) = self.class_value_array();
    let Ok(value_20) = glyphs.contains(GlyphId::from(start_glyph));

    match ::core::primitive::bool::then_some(
      value_20 == console_core_integer_sets::Presence::Present,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let start_glyph_class = match class_values.first() {
          Some(start_glyph_class) => start_glyph_class,
          None => {
            return Ok(out);
          }
        };
        let Ok(value_21) = start_glyph_class.get();

        let Ok(_previous_presence) = out.insert(value_21);
      }
      ::core::option::Option::None => {}
    }

    let Ok(value_93) = glyphs.iter_after(GlyphId::from(start_glyph));

    for g in value_93 {
      let Ok(value_52) = g.to_u32();

      let g_2 = value_52;

      match ::core::primitive::bool::then_some(g_2 > end_glyph, ()) {
        ::core::option::Option::Some(()) => {
          break;
        }
        ::core::option::Option::None => {}
      }

      let idx = ::core::primitive::u32::wrapping_sub(g_2, start_glyph);
      let Ok(value_69) = console_core_number_conversion::index({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
        migrated_value
      });

      let class = match class_values.get(value_69) {
        Some(class) => class,
        None => {
          break;
        }
      };
      let Ok(value_76) = class.get();

      let Ok(_previous_presence) = out.insert(value_76);
    }

    Ok(out)
  }

  #[cfg(feature = "std")]
  #[must_use]
  fn intersected_class_glyphs(
    &self,
    glyphs: &IntSet<GlyphId>,
    class: u16,
  ) -> core::result::Result<IntSet<GlyphId>, console_core_never::Never> {
    let Ok(mut out) = IntSet::empty();
    let Ok(value_22) = glyphs.is_empty();

    match ::core::primitive::bool::then_some(
      value_22 == console_core_integer_sets::Emptiness::Empty,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(out);
      }
      ::core::option::Option::None => {}
    }

    let Ok(start_glyph_id) = self.start_glyph_id();
    let Ok(start_glyph) = start_glyph_id.to_u32();
    let Ok(glyph_count) = self.glyph_count();
    let end_glyph = ::core::primitive::u32::wrapping_sub(
      ::core::primitive::u32::wrapping_add(start_glyph, {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(glyph_count);
        migrated_value
      }),
      1,
    );

    match ::core::primitive::bool::then_some(class == 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(span) = class_glyph_span(glyphs);
        let (first, last) = match span {
          Some(endpoints) => endpoints,
          None => return Ok(out),
        };
        let Ok(value_53) = first.to_u32();

        match ::core::primitive::bool::then_some(value_53 < start_glyph, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(value_96) = glyphs.range(first..GlyphId::from(start_glyph));

            out.extend(value_96);
          }
          ::core::option::Option::None => {}
        }

        let Ok(value_54) = last.to_u32();

        match ::core::primitive::bool::then_some(value_54 > end_glyph, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(value_98) = glyphs
              .range(GlyphId::from(::core::primitive::u32::wrapping_add(end_glyph, 1))..=last);

            out.extend(value_98);
          }
          ::core::option::Option::None => {}
        }

        return Ok(out);
      }
      ::core::option::Option::None => {}
    }

    let Ok(class_values) = self.class_value_array();
    let Ok(value_99) = glyphs.range(GlyphId::from(start_glyph)..=GlyphId::from(end_glyph));

    for g in value_99 {
      let Ok(value_55) = g.to_u32();

      let idx = ::core::primitive::u32::wrapping_sub(value_55, start_glyph);
      let Ok(value_70) = console_core_number_conversion::index({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
        migrated_value
      });

      let c = match class_values.get(value_70) {
        Some(c) => c,
        None => {
          break;
        }
      };
      let Ok(value_77) = c.get();

      match ::core::primitive::bool::then_some(value_77 == class, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(_previous_presence) = out.insert(g);
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(out)
  }
}

impl<'a> ClassDefFormat2<'a> {
  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDefFormat2::get` method returns `u16` directly in the existing parser interface"
    )
  )]
  pub fn get(&self, gid: impl Into<GlyphId>) -> u16 {
    let Ok(gid_2) = <_ as ::core::convert::Into<::font_types::GlyphId>>::into(gid).to_u32();
    let Ok(records) = self.class_range_records();
    let ::core::result::Result::Ok(ix) = ::console_core_number_conversion::fixed_index(
      match records.binary_search_by(|rec| {
        let Ok(start_glyph_id) = rec.start_glyph_id();
        let Ok(value_23) = start_glyph_id.to_u32();

        value_23.cmp(&gid_2)
      }) {
        Ok(ix) => ix,
        Err(ix) => ix.saturating_sub(1),
      },
    );

    match records.get(match ::console_core_number_conversion::index(ix) {
      ::core::result::Result::Ok(native_position) => native_position,
    }) {
      Some(record) => {
        let Ok(start_glyph_id) = record.start_glyph_id();
        let Ok(value_56) = start_glyph_id.to_u32();
        let Ok(end_glyph_id) = record.end_glyph_id();
        let Ok(value_57) = end_glyph_id.to_u32();

        let value_24 = value_56..=value_57;

        match ::core::primitive::bool::then_some(value_24.contains(&gid_2), ()) {
          ::core::option::Option::Some(()) => {
            let Ok(class) = record.class();
            return class;
          }
          ::core::option::Option::None => {}
        }
      }
      None => {}
    }

    0
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDefFormat2::iter` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn iter(&self) -> impl Iterator<Item = (GlyphId16, u16)> + 'a + use<'a> {
    let Ok(class_range_records) = self.class_range_records();

    class_range_records.iter().flat_map(|range| {
      let Ok(start_glyph_id) = range.start_glyph_id();
      let Ok(value_58) = start_glyph_id.to_u16();

      let start = value_58;
      let Ok(end_glyph_id) = range.end_glyph_id();
      let Ok(value_59) = end_glyph_id.to_u16();

      let end = value_59;
      let value_25 = start..=end;
      let Ok(class) = range.class();

      value_25.map(move |gid| {
        let Ok(value_42) = GlyphId16::new(gid);

        (value_42, class)
      })
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDefFormat2::population` method returns `u64` directly in the existing parser interface"
    )
  )]
  pub fn population(&self) -> u64 {
    let Ok(class_range_records) = self.class_range_records();

    class_range_records.iter().fold(0, |acc, record| {
      ::core::primitive::u64::wrapping_add(acc, record.population())
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDefFormat2::cost` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn cost(&self) -> u32 {
    {
      let Ok(explicit_value) = bit_storage({
        let Ok(class_range_count) = self.class_range_count();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(class_range_count);
        migrated_value
      });
      explicit_value
    }
  }

  #[cfg(feature = "std")]
  #[must_use]
  fn intersect_classes(
    &self,
    glyphs: &IntSet<GlyphId>,
  ) -> core::result::Result<IntSet<u16>, console_core_never::Never> {
    let Ok(mut out) = IntSet::empty();
    let Ok(first_glyph) = glyphs.first();
    let first_glyph_2 = match first_glyph {
      Some(glyph) => glyph,
      None => return Ok(out),
    };
    let Ok(range_records) = self.class_range_records();
    let (first_record, remaining_ranges) = match range_records.split_first() {
      Some(ranges) => ranges,
      None => {
        let Ok(_previous_presence) = out.insert(0);

        return Ok(out);
      }
    };
    let Ok(first_class_glyph) = first_record.start_glyph_id();

    match ::core::primitive::bool::then_some(first_glyph_2 < first_class_glyph, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(_previous_presence) = out.insert(0);
      }
      ::core::option::Option::None => {
        let Ok(end_glyph_id) = first_record.end_glyph_id();
        let mut glyph = GlyphId::from(end_glyph_id);

        for record in remaining_ranges {
          let Ok(mut value_27) = glyphs.iter_after(glyph);

          let g = match value_27.next() {
            Some(g) => g,
            None => {
              break;
            }
          };

          let Ok(start_glyph_id) = record.start_glyph_id();

          match ::core::primitive::bool::then_some(g < start_glyph_id, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(_previous_presence) = out.insert(0);
              break;
            }
            ::core::option::Option::None => {}
          }

          let Ok(record_end_glyph_id) = record.end_glyph_id();
          glyph = GlyphId::from(record_end_glyph_id);
        }

        let Ok(mut value_28) = glyphs.iter_after(glyph);

        match ::core::primitive::bool::then_some(value_28.next().is_some(), ()) {
          ::core::option::Option::Some(()) => {
            let Ok(_previous_presence) = out.insert(0);
          }
          ::core::option::Option::None => {}
        }
      }
    }

    let Ok(num_ranges) = self.class_range_count();
    let Ok(value_29) = glyphs.len();

    match ::core::primitive::bool::then_some(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(num_ranges);
        migrated_value
      }) > ::core::primitive::u64::wrapping_mul(value_29, {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(self.cost());
        migrated_value
      }),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(value_106) = glyphs.iter();

        for g in value_106 {
          let class = self.get(g);

          match ::core::primitive::bool::then_some(class != 0, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(_previous_presence) = out.insert(class);
            }
            ::core::option::Option::None => {}
          }
        }
      }
      ::core::option::Option::None => {
        for record in range_records {
          let Ok(start_glyph_id) = record.start_glyph_id();
          let Ok(value_30) = glyphs.intersects_range(
            GlyphId::from(start_glyph_id)..=GlyphId::from({
              let Ok(explicit_value) = record.end_glyph_id();
              explicit_value
            }),
          );

          match ::core::primitive::bool::then_some(
            value_30 == console_core_integer_sets::Overlap::Intersecting,
            (),
          ) {
            ::core::option::Option::Some(()) => {
              let Ok(record_class) = record.class();
              let Ok(_previous_presence) = out.insert(record_class);
            }
            ::core::option::Option::None => {}
          }
        }
      }
    }

    Ok(out)
  }

  #[cfg(feature = "std")]
  #[must_use]
  fn intersected_class_glyphs(
    &self,
    glyphs: &IntSet<GlyphId>,
    class: u16,
  ) -> core::result::Result<IntSet<GlyphId>, console_core_never::Never> {
    let Ok(mut out) = IntSet::empty();
    let Ok(span) = class_glyph_span(glyphs);
    let (first_glyph, last_glyph) = match span {
      Some(endpoints) => endpoints,
      None => return Ok(out),
    };
    let Ok(first) = first_glyph.to_u32();
    let Ok(last) = last_glyph.to_u32();

    match ::core::primitive::bool::then_some(class == 0, ()) {
      ::core::option::Option::Some(()) => {
        let mut start = first;

        let Ok(class_range_records) = self.class_range_records();

        for range in class_range_records {
          let Ok(start_glyph_id) = range.start_glyph_id();
          let Ok(value_62) = start_glyph_id.to_u32();

          let range_start = value_62;

          match ::core::primitive::bool::then_some(start < range_start, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(value_111) = glyphs.range(GlyphId::from(start)..GlyphId::from(range_start));

              out.extend(value_111);
            }
            ::core::option::Option::None => {}
          }

          let Ok(end_glyph_id) = range.end_glyph_id();
          let Ok(range_end) = end_glyph_id.to_u32();

          match ::core::primitive::bool::then_some(range_end >= last, ()) {
            ::core::option::Option::Some(()) => {
              break;
            }
            ::core::option::Option::None => {}
          }

          start = ::core::primitive::u32::wrapping_add(range_end, 1);
        }

        match ::core::primitive::bool::then_some(start <= last, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(value_112) = glyphs.range(GlyphId::from(start)..=GlyphId::from(last));

            out.extend(value_112);
          }
          ::core::option::Option::None => {}
        }

        return Ok(out);
      }
      ::core::option::Option::None => {}
    }

    let Ok(num_ranges) = self.class_range_count();
    let Ok(value_32) = glyphs.len();

    match ::core::primitive::bool::then_some(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(num_ranges);
        migrated_value
      }) > ::core::primitive::u64::wrapping_mul(value_32, {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(self.cost());
        migrated_value
      }),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(value_113) = glyphs.iter();

        for g in value_113 {
          let c = self.get(g);

          match ::core::primitive::bool::then_some(c == class, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(_previous_presence) = out.insert(g);
            }
            ::core::option::Option::None => {}
          }
        }
      }
      ::core::option::Option::None => {
        let Ok(class_range_records) = self.class_range_records();

        for range in class_range_records {
          let Ok(start_glyph_id) = range.start_glyph_id();
          let Ok(value_63) = start_glyph_id.to_u32();

          let range_start = value_63;
          let Ok(end_glyph_id) = range.end_glyph_id();
          let Ok(value_64) = end_glyph_id.to_u32();

          let range_end = value_64;

          match ::core::primitive::bool::then_some(range_start > last, ()) {
            ::core::option::Option::Some(()) => {
              break;
            }
            ::core::option::Option::None => {}
          }

          let Ok(range_class) = range.class();

          match (range_class != class || range_end < first).then_some(()) {
            ::core::option::Option::Some(()) => {
              continue;
            }
            ::core::option::Option::None => {}
          }

          let Ok(value_115) = glyphs.range(GlyphId::from(range_start)..=GlyphId::from(range_end));

          out.extend(value_115);
        }
      }
    }

    Ok(out)
  }
}

#[cfg(feature = "std")]
fn collect_coverage_range(
  glyphs: &IntSet<GlyphId>,
  output: &mut IntSet<GlyphId>,
  bounds: core::ops::RangeInclusive<GlyphId>,
) -> Result<(), Never> {
  let (start, end) = bounds.into_inner();

  match glyphs.contains(start) {
    Ok(Presence::Present) => {
      let Ok(_previous_presence) = output.insert(start);
    }
    Ok(Presence::Absent) => {}
  }

  let Ok(after_start) = glyphs.iter_after(start);

  for glyph in after_start {
    match glyph.cmp(&end) {
      core::cmp::Ordering::Greater => break,
      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {
        let Ok(_previous_presence) = output.insert(glyph);
      }
    }
  }

  Ok(())
}

impl ClassRangeRecord {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassRangeRecord::population` method returns `u64` directly in the existing parser interface"
    )
  )]
  pub fn population(&self) -> u64 {
    let Ok(start_glyph_id) = self.start_glyph_id();
    let Ok(value_33) = start_glyph_id.to_u32();

    let start = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_33);
      migrated_value
    };
    let Ok(end_glyph_id) = self.end_glyph_id();
    let Ok(value_34) = end_glyph_id.to_u32();

    let end = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_34);
      migrated_value
    };

    match ::core::primitive::bool::then_some(start > end, ()) {
      ::core::option::Option::Some(()) => 0,
      ::core::option::Option::None => {
        ::core::primitive::u64::wrapping_add(::core::primitive::u64::wrapping_sub(end, start), 1)
      }
    }
  }
}

impl ClassDef<'_> {
  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDef::get` method returns `u16` directly in the existing parser interface"
    )
  )]
  pub fn get(&self, gid: impl Into<GlyphId>) -> u16 {
    match self {
      ClassDef::Format1(table) => table.get(gid),
      ClassDef::Format2(table) => table.get(gid),
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDef::iter` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn iter(&self) -> impl Iterator<Item = (GlyphId16, u16)> + '_ {
    let (one, two) = match self {
      ClassDef::Format1(inner) => (Some(inner.iter()), None),
      ClassDef::Format2(inner) => (None, Some(inner.iter())),
    };
    one.into_iter().flatten().chain(two.into_iter().flatten())
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDef::population` method returns `u64` directly in the existing parser interface"
    )
  )]
  pub fn population(&self) -> u64 {
    match self {
      ClassDef::Format1(table) => table.population(),
      ClassDef::Format2(table) => table.population(),
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDef::cost` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn cost(&self) -> u32 {
    match self {
      ClassDef::Format1(sub) => sub.cost(),
      ClassDef::Format2(sub) => sub.cost(),
    }
  }

  #[cfg(feature = "std")]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDef::intersect_classes` method returns `IntSet` directly in the existing parser interface"
    )
  )]
  pub fn intersect_classes(&self, glyphs: &IntSet<GlyphId>) -> IntSet<u16> {
    match self {
      ClassDef::Format1(table) => {
        let Ok(intersect_classes) = table.intersect_classes(glyphs);

        intersect_classes
      }
      ClassDef::Format2(table) => {
        let Ok(intersect_classes) = table.intersect_classes(glyphs);

        intersect_classes
      }
    }
  }

  #[cfg(feature = "std")]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ClassDef::intersected_class_glyphs` method returns `IntSet` directly in the existing parser interface"
    )
  )]
  pub fn intersected_class_glyphs(
    &self,
    glyphs: &IntSet<GlyphId>,
    class: u16,
  ) -> IntSet<GlyphId> {
    match self {
      ClassDef::Format1(table) => {
        let Ok(intersected_class_glyphs) = table.intersected_class_glyphs(glyphs, class);

        intersected_class_glyphs
      }
      ClassDef::Format2(table) => {
        let Ok(intersected_class_glyphs) = table.intersected_class_glyphs(glyphs, class);

        intersected_class_glyphs
      }
    }
  }
}

impl<'a> Device<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Device::iter` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn iter(&self) -> impl Iterator<Item = i8> + 'a + use<'a> {
    let Ok(format) = self.delta_format();
    let mut n = ::core::primitive::u64::wrapping_add(
      {
        let Ok(end_size) = self.end_size();
        let Ok(start_size) = self.start_size();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
          ::core::primitive::u16::wrapping_sub(end_size, start_size),
        );
        migrated_value
      },
      1,
    );
    let deltas_per_word = match format {
      DeltaFormat::Local2BitDeltas => 8,
      DeltaFormat::Local4BitDeltas => 4,
      DeltaFormat::Local8BitDeltas => 2,
      DeltaFormat::VariationIndex | DeltaFormat::Unknown => 0,
    };

    let Ok(delta_value) = self.delta_value();

    delta_value.iter().flat_map(move |val| {
      let Ok(value_35) = val.get();

      let Ok(iter) = iter_packed_values(value_35, format, n);
      n = n.saturating_sub(deltas_per_word);
      iter
    })
  }
}

#[must_use]
fn iter_packed_values(
  raw: u16,
  format: DeltaFormat,
  n: u64,
) -> core::result::Result<impl Iterator<Item = i8>, console_core_never::Never> {
  let mut decoded = [None; 8];
  let (mask, sign_mask, bits, max_per_word) = match format {
    DeltaFormat::Local2BitDeltas => (0b11, 0b10, 2u64, 8),
    DeltaFormat::Local4BitDeltas => (0b1111, 0b1000, 4, 4),
    DeltaFormat::Local8BitDeltas => (0b1111_1111, 0b1000_0000, 8, 2),
    DeltaFormat::VariationIndex | DeltaFormat::Unknown => {
      return Ok(decoded.into_iter().flatten());
    }
  };

  for (i, delta) in (0..n.min(max_per_word)).zip(&mut decoded) {
    let shift = 16u64.wrapping_sub(bits).wrapping_sub(i.wrapping_mul(bits));
    let Ok(shift_count) = console_core_number_conversion::fitted::<_, u32>(shift);
    let mask_2 = u16::wrapping_shl(mask, shift_count);
    let val = (raw & mask_2).wrapping_shr(shift_count);
    let sign = val & sign_mask != 0;

    let val_2 = match ::core::primitive::bool::then_some(sign, ()) {
      ::core::option::Option::Some(()) => ::core::primitive::i8::wrapping_neg({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i8>(
          ::core::primitive::u16::wrapping_add((!val) & mask_2, 1),
        );
        migrated_value
      }),
      ::core::option::Option::None => {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i8>(val);
        migrated_value
      }
    };
    *delta = Some(val_2);
  }

  Ok(decoded.into_iter().flatten())
}

impl From<VariationIndex<'_>> for DeltaSetIndex {
  fn from(src: VariationIndex) -> DeltaSetIndex {
    let Ok(outer) = src.delta_set_outer_index();
    let Ok(inner) = src.delta_set_inner_index();

    DeltaSetIndex { outer, inner }
  }
}

#[derive(Clone)]
pub struct TaggedElement<T> {
  pub tag: Tag,
  pub element: T,
}

impl<T> TaggedElement<T> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `TaggedElement::new` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub fn new(tag: Tag, element: T) -> Self {
    Self { tag, element }
  }
}

impl<T> std::ops::Deref for TaggedElement<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.element
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn coverage_get_format1() -> ::core::result::Result<(), crate::read::ReadError> {
    const COV1_DATA: FontData = {
      let Ok(explicit_value) = FontData::new(&[0, 1, 0, 5, 0, 1, 0, 7, 0, 13, 0, 27, 0, 44]);
      explicit_value
    };

    let coverage = CoverageFormat1::read(COV1_DATA)?;
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(1);
        value
      }),
      Some(0)
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(2);
        value
      }),
      None
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(7);
        value
      }),
      Some(1)
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(27);
        value
      }),
      Some(3)
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(45);
        value
      }),
      None
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn coverage_get_format2() -> ::core::result::Result<(), crate::read::ReadError> {
    const COV2_DATA: FontData = {
      let Ok(explicit_value) = FontData::new(&[0, 2, 0, 2, 0, 5, 0, 9, 0, 0, 0, 30, 0, 39, 0, 5]);
      explicit_value
    };
    let coverage = CoverageFormat2::read(COV2_DATA)?;
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(2);
        value
      }),
      None
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(7);
        value
      }),
      Some(2)
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(9);
        value
      }),
      Some(4)
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(10);
        value
      }),
      None
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(32);
        value
      }),
      Some(7)
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(39);
        value
      }),
      Some(14)
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(40);
        value
      }),
      None
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn coverage_get_format2_no_u16_overflow() -> ::core::result::Result<(), crate::read::ReadError>
  {
    const COV2_DATA: FontData = {
      let Ok(explicit_value) = FontData::new(&[0, 2, 0, 1, 0x9c, 0x40, 0x9c, 0x4a, 0x9c, 0x40]);
      explicit_value
    };
    let coverage = CoverageFormat2::read(COV2_DATA)?;
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(40000);
        value
      }),
      Some(40000)
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(40005);
        value
      }),
      Some(40005)
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(40010);
        value
      }),
      Some(40010)
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(40011);
        value
      }),
      None
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn coverage_get_format2_rejects_overflowing_coverage_index()
  -> ::core::result::Result<(), crate::read::ReadError> {
    const COV2_DATA: FontData = {
      let Ok(explicit_value) = FontData::new(&[0, 2, 0, 1, 0, 1, 0, 2, 0xff, 0xff]);
      explicit_value
    };
    let coverage = CoverageFormat2::read(COV2_DATA)?;
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(1);
        value
      }),
      Some(u16::MAX)
    );
    assert_eq!(
      coverage.get({
        let Ok(value) = GlyphId::new(2);
        value
      }),
      None
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn classdef_get_format2() -> ::core::result::Result<(), crate::test_support::Failure> {
    let classdef = crate::test_support::fixture_value(
      ClassDef::read({
        let Ok(explicit_value) = FontData::new(font_test_data::gdef::MARKATTACHCLASSDEF_TABLE);
        explicit_value
      }),
      "fixture value",
    )?;
    assert!(matches!(classdef, ClassDef::Format2(..)));

    let gid_class_pairs = [
      (616, 1),
      (617, 1),
      (618, 1),
      (624, 1),
      (625, 1),
      (626, 1),
      (652, 2),
      (653, 2),
      (654, 2),
      (655, 2),
      (661, 2),
    ];
    for (gid, class) in gid_class_pairs {
      assert_eq!(
        classdef.get({
          let Ok(value) = GlyphId16::new(gid);
          value
        }),
        class
      );
    }

    for (gid, class) in classdef.iter() {
      assert_eq!(classdef.get(gid), class);
    }

    Ok(())
  }

  #[test]
  fn delta_decode() {
    assert_eq!(
      ({
        let Ok(explicit_value) = iter_packed_values(0x123f, DeltaFormat::Local4BitDeltas, 4);
        explicit_value
      })
      .collect::<Vec<_>>(),
      &[1, 2, 3, -1]
    );

    assert_eq!(
      ({
        let Ok(explicit_value) = iter_packed_values(0x5540, DeltaFormat::Local2BitDeltas, 5);
        explicit_value
      })
      .collect::<Vec<_>>(),
      &[1, 1, 1, 1, 1]
    );
  }

  #[test]
  fn delta_decode_all() -> ::core::result::Result<(), crate::read::ReadError> {
    let bytes: &[u8] = &[0, 7, 0, 13, 0, 3, 1, 244, 30, 245, 101, 8, 42, 0];
    let device = Device::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(bytes))?;
    assert_eq!(
      device.iter().collect::<Vec<_>>(),
      &[1i8, -12, 30, -11, 101, 8, 42]
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn bit_storage_tests() {
    assert_eq!(
      ({
        let Ok(explicit_value) = bit_storage(0);
        explicit_value
      }),
      0
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = bit_storage(1);
        explicit_value
      }),
      1
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = bit_storage(2);
        explicit_value
      }),
      2
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = bit_storage(4);
        explicit_value
      }),
      3
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = bit_storage(9);
        explicit_value
      }),
      4
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = bit_storage(0x123);
        explicit_value
      }),
      9
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = bit_storage(0x1234);
        explicit_value
      }),
      13
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = bit_storage(0xffff);
        explicit_value
      }),
      16
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = bit_storage(0xffff_ffff);
        explicit_value
      }),
      32
    );
  }

  #[test]
  fn default_coverage() {
    let coverage = CoverageTable::default();

    assert_eq!(coverage.iter().count(), 0)
  }

  #[test]
  fn default_classdef() {
    let classdef = ClassDef::default();
    assert_eq!(classdef.population(), 0);
    assert_eq!(classdef.iter().count(), 0);
  }
}
