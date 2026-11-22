//! Font instance representation.

use super::Font;
use crate::{
  TableProvider,
  tables::{
    avar::Avar,
    fvar::Fvar,
    layout::{Condition, FeatureVariations},
  },
};
use alloc::vec::Vec;
use core::{
  str::FromStr,
  sync::atomic::{self, AtomicU32},
};
use types::{Fixed, Tag};

pub struct FontInstance {
  font: Font,
  size: Option<f32>,
  coords: CoordStorage,
  feature_vars: FeatureVarsStorage,
}

impl FontInstance {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontInstance::builder` method returns `FontInstanceBuilder` directly in the existing parser interface")
  )]
  pub fn builder(font: &Font) -> FontInstanceBuilder {
    FontInstanceBuilder {
      instance: Self {
        font: font.clone(),
        size: None,
        coords: CoordStorage::default(),
        feature_vars: ({ let Ok(explicit_value) = FeatureVarsStorage::new(); explicit_value }),
      },
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontInstance::font` method returns a borrow tied to the source lifetime directly")
  )]
  pub fn font(&self) -> &Font {
    &self.font
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontInstance::size` method returns the optional value directly to callers that match its presence")
  )]
  pub fn size(&self) -> Option<f32> {
    self.size
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontInstance::normalized_coords` method returns a borrow tied to the source lifetime directly")
  )]
  pub fn normalized_coords(&self) -> &[NormalizedCoord] {
    let Ok(slice) = self.coords.as_slice();

    slice
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontInstance::feature_variations` method returns `FontFeatureVariations` directly in the existing parser interface")
  )]
  pub fn feature_variations(&self) -> FontFeatureVariations {
    let Ok(load) = self.feature_vars.load(&self.font, { let Ok(explicit_value) = self.coords.as_slice(); explicit_value });

    load
  }
}

impl core::ops::Deref for FontInstance {
  type Target = Font;

  fn deref(&self) -> &Font {
    self.font()
  }
}

pub struct FontInstanceBuilder {
  instance: FontInstance,
}

impl FontInstanceBuilder {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontInstanceBuilder::size` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn size(mut self, size: Option<f32>) -> Self {
    self.instance.size = size;
    self
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontInstanceBuilder::variations` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn variations<V>(mut self, variations: V) -> Self
  where
    V: IntoIterator,
    V::Item: Into<FontVariation>,
  {
    let Ok(()) = self.set_variations(variations);
    self
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontInstanceBuilder::normalized_coords` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn normalized_coords(mut self, coords: impl IntoIterator<Item = NormalizedCoord>) -> Self {
    let Ok(()) = self.set_coords(coords);
    self
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontInstanceBuilder::named_instance` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn named_instance(mut self, index: u64) -> Self {
    let Ok(()) = self.set_named_instance(index);
    self
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontInstanceBuilder::named_instance_with_overrides` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn named_instance_with_overrides<V>(mut self, index: u64, overrides: V) -> Self
  where
    V: IntoIterator,
    V::Item: Into<FontVariation>,
  {
    let Ok(()) = self.set_named_instance_with_overrides(index, overrides);
    self
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontInstanceBuilder::build` method returns `FontInstance` directly in the existing parser interface")
  )]
  pub fn build(self) -> FontInstance {
    self.instance
  }
}

impl FontInstanceBuilder {
  fn set_variations<V>(&mut self, variations: V) -> core::result::Result<(), console_core_never::Never>
  where
    V: IntoIterator,
    V::Item: Into<FontVariation>,

  { {
    let tables = self.instance.font.tables();

    match tables.fvar() {
      Ok(fvar) => {
        let Ok(()) = set_variations(
          &fvar,
          match tables.avar() { Ok(value) => Some(value), Err(_unavailable_axis_variations) => None },
          &mut self.instance.coords,
          variations,
        );
      }
      Err(_unavailable_variation_axes) => {
        let Ok(()) = self.instance.coords.resize(0);
      }
    }
  }; Ok(()) }

  fn set_coords(&mut self, coords: impl IntoIterator<Item = NormalizedCoord>) -> core::result::Result<(), console_core_never::Never> { {
    match self.instance.font.tables().fvar() {
      Ok(fvar) => {
        let Ok(axis_count) = fvar.axis_count();
        let Ok(()) = self.instance.coords.resize(axis_count);

        let Ok(mut_slice) = self.instance.coords.as_mut_slice();
        let padding = core::iter::repeat_n(NormalizedCoord::ZERO, mut_slice.len());

        for (dst, src) in mut_slice.iter_mut().zip(coords.into_iter().chain(padding)) {
          *dst = src;
        }

        let Ok(()) = self.instance.coords.clear_if_all_zeroes();
      }
      Err(_unavailable_variation_axes) => {
        let Ok(()) = self.instance.coords.resize(0);
      }
    }
  }; Ok(()) }

  fn set_named_instance(&mut self, index: u64) -> core::result::Result<(), console_core_never::Never> { {
    let tables = self.instance.font.tables();

    match tables.fvar() {
      Ok(fvar) => {
        let Ok(()) = set_variations(
          &fvar,
          match tables.avar() { Ok(value) => Some(value), Err(_unavailable_axis_variations) => None },
          &mut self.instance.coords,
          { let Ok(explicit_value) = named_instance_variations(&fvar, index); explicit_value },
        );
      }
      Err(_unavailable_variation_axes) => {
        let Ok(()) = self.instance.coords.resize(0);
      }
    }
  }; Ok(()) }

  fn set_named_instance_with_overrides<V>(&mut self, index: u64, overrides: V) -> core::result::Result<(), console_core_never::Never>
  where
    V: IntoIterator,
    V::Item: Into<FontVariation>,

  { {
    let tables = self.instance.font.tables();

    match tables.fvar() {
      Ok(fvar) => {
        let Ok(()) = set_variations(
          &fvar,
          match tables.avar() { Ok(value) => Some(value), Err(_unavailable_axis_variations) => None },
          &mut self.instance.coords,
          ({ let Ok(explicit_value) = named_instance_variations(&fvar, index); explicit_value }).chain(overrides.into_iter().map(Into::into)),
        );
      }
      Err(_unavailable_variation_axes) => {
        let Ok(()) = self.instance.coords.resize(0);
      }
    }
  }; Ok(()) }
}

#[must_use]
fn named_instance_variations<'a>(
  fvar: &'a Fvar,
  index: u64,
) -> core::result::Result<impl Iterator<Item = FontVariation> + 'a, console_core_never::Never> {
    Ok((match fvar
      .axis_instance_arrays() { Ok(value) => Some(value), Err(_malformed_axis_instances) => None })
      .and_then(|arrays| {
        let Ok(axes) = arrays.axes();
        let Ok(instances) = arrays.instances();

        match instances.get(index) {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }
        .map(|instance| {
          axes.iter().zip(instance.coordinates).map(|(axis, coord)| {
            let Ok(value_3) = ({
              let Ok(explicit_value) = coord.get();
              explicit_value
            })
            .to_f32();
            let Ok(tag) = axis.axis_tag();

            FontVariation::new(tag, value_3)
          })
        })
      })
      .into_iter()
      .flatten())
}

fn set_variations<V>(fvar: &Fvar, avar: Option<Avar>, coords: &mut CoordStorage, variations: V) -> core::result::Result<(), console_core_never::Never>
where
  V: IntoIterator,
  V::Item: Into<FontVariation>,

{ {
  let Ok(axis_count) = fvar.axis_count();
  let Ok(()) = coords.resize(axis_count);
  fvar.user_to_normalized(
    avar.as_ref(),
    variations.into_iter().map(Into::into).map(|var| {
      let Ok(value_2) = Fixed::from_f64(f64::from(var.value));

      (var.tag, value_2)
    }),
    { let Ok(explicit_value) = coords.as_mut_slice(); explicit_value },
  );
  let Ok(()) = coords.clear_if_all_zeroes();
}; Ok(()) }

pub type NormalizedCoord = types::F2Dot14;

#[derive(Copy, Clone, PartialEq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(explicit067_no_unit_less_quantity, reason = "FontVariation exposes the design axis value as f32 for caller-provided struct literals")
)]
pub struct FontVariation {
  pub tag: Tag,
  pub value: f32,
}

impl FontVariation {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontVariation::new` method supports direct construction or chaining with its existing Self return")
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(explicit067_no_unit_less_quantity, reason = "the public variation constructor accepts an f32 design-space coordinate")
  )]
  pub fn new(tag: Tag, value: f32) -> Self {
    Self { tag, value }
  }
}

impl From<&'_ FontVariation> for FontVariation {
  fn from(value: &'_ FontVariation) -> Self {
    *value
  }
}

impl From<(Tag, f32)> for FontVariation {
  fn from(value: (Tag, f32)) -> Self {
    Self::new(value.0, value.1)
  }
}

impl From<&(Tag, f32)> for FontVariation {
  fn from(value: &(Tag, f32)) -> Self {
    Self::new(value.0, value.1)
  }
}

impl From<(&str, f32)> for FontVariation {
  fn from(value: (&str, f32)) -> Self {
    Self::new(match Tag::from_str(value.0) { Ok(value_2) => value_2, Err(_invalid_variation_tag) => Default::default() }, value.1)
  }
}

impl From<&(&str, f32)> for FontVariation {
  fn from(value: &(&str, f32)) -> Self {
    Self::new(match Tag::from_str(value.0) { Ok(value_2) => value_2, Err(_invalid_variation_tag) => Default::default() }, value.1)
  }
}

const INLINE_COORDINATES: [NormalizedCoord; 15] = [NormalizedCoord::ZERO; _];

#[derive(Default)]
enum CoordStorage {
  #[default]
  None,
  Inline([NormalizedCoord; INLINE_COORDINATES.len()], u8),
  Heap(Vec<NormalizedCoord>),
}

impl CoordStorage {
  fn clear_if_all_zeroes(&mut self) -> Result<(), console_core_never::Never> {
    let Ok(coords) = self.as_slice();

    match coords.iter().find(|&&coord| coord != NormalizedCoord::ZERO) {
      Some(_nonzero_coordinate) => {}
      None => match self {
        Self::None => {}
        Self::Inline(_coords, len) => *len = 0,
        Self::Heap(heap) => heap.clear(),
      },
    }

    Ok(())
  }

  fn resize(&mut self, axis_count: u16) -> Result<(), console_core_never::Never> {
    let Ok(capacity) = console_core_number_conversion::index(axis_count);

    match self {
      Self::Heap(heap) => {
        heap.resize(capacity.min(65_535), NormalizedCoord::ZERO);
        return Ok(());
      }
      Self::None | Self::Inline(_, _) => {}
    }

    let Ok(inline_capacity) = console_core_number_conversion::fixed_index(INLINE_COORDINATES.len());

    match (u64::from(axis_count) <= inline_capacity).then_some(()) {
      Some(()) => {
        let Ok(length) = console_core_number_conversion::fitted::<_, u8>(axis_count);

        match self {
          Self::None => *self = Self::Inline(INLINE_COORDINATES, length),
          Self::Inline(_, used) => *used = length,
          Self::Heap(_) => {}
        }
      }
      None => {
        *self = Self::Heap(alloc::vec![NormalizedCoord::ZERO; capacity.min(65_535)]);
      }
    }

    Ok(())
  }

  #[must_use]
  fn as_slice(&self) -> Result<&[NormalizedCoord], console_core_never::Never> {
    Ok(match self {
      Self::None => &[],
      Self::Inline(coords, len) => match coords.get(..match ::console_core_number_conversion::index::<::core::primitive::u8>(*len) { ::core::result::Result::Ok(converted_position) => converted_position }) {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => &[],
      },
      Self::Heap(heap) => heap.as_slice(),
    })
  }

  fn as_mut_slice(&mut self) -> Result<&mut [NormalizedCoord], console_core_never::Never> {
    Ok(match self {
      Self::None => &mut [],
      Self::Inline(coords, len) => match coords.get_mut(..match ::console_core_number_conversion::index::<::core::primitive::u8>(*len) { ::core::result::Result::Ok(converted_position) => converted_position }) {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => &mut [],
      },
      Self::Heap(heap) => heap.as_mut_slice(),
    })
  }
}

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct FontFeatureVariations {
  gsub: Option<u32>,
  gpos: Option<u32>,
}

impl FontFeatureVariations {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontFeatureVariations::gsub` method returns the optional value directly to callers that match its presence")
  )]
  pub fn gsub(&self) -> Option<u32> {
    self.gsub
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FontFeatureVariations::gpos` method returns the optional value directly to callers that match its presence")
  )]
  pub fn gpos(&self) -> Option<u32> {
    self.gpos
  }
}

struct FeatureVarsStorage {
  status: AtomicU32,
  gsub: AtomicU32,
  gpos: AtomicU32,
}

impl Default for FeatureVarsStorage {
  fn default() -> Self {
    let Ok(feature_vars_storage) = Self::new();

    feature_vars_storage
  }
}

impl FeatureVarsStorage {
  const UNCHECKED: u32 = 0;
  const PRESENT: u32 = 1;
  const ABSENT: u32 = 2;
  const BOTH_ABSENT: u32 = Self::ABSENT | (Self::ABSENT << Self::GPOS_SHIFT);
  const GPOS_SHIFT: u32 = 16;

  #[must_use]
  fn new() -> core::result::Result<Self, console_core_never::Never> {
      Ok(Self {
        status: AtomicU32::new(Self::UNCHECKED),
        gsub: AtomicU32::new(0),
        gpos: AtomicU32::new(0),
      })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit069_lock_discipline",
    expect(
      explicit069_lock_discipline,
      reason = "the Release store to status publishes the selected indices before readers pass the Acquire load; concurrent initializers compute identical results for one immutable instance"
    )
  )]
  fn load(&self, font: &Font, coords: &[NormalizedCoord]) -> core::result::Result<FontFeatureVariations, console_core_never::Never> {
      let mut status = self.status.load(atomic::Ordering::Acquire);

      match ::core::primitive::bool::then_some(status == Self::UNCHECKED, ()) {
        ::core::option::Option::Some(()) => {
          let tables = font.tables();

          let feature_var_tables = [
            (match tables.gsub() { Ok(value) => Some(value), Err(_unavailable_substitution_table) => None }).and_then(|gsub| {
              match gsub.feature_variations() {
                ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
                ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
              }
              .flatten()
            }),
            (match tables.gpos() { Ok(value) => Some(value), Err(_unavailable_positioning_table) => None }).and_then(|gpos| {
              match gpos.feature_variations() {
                ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
                ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
              }
              .flatten()
            }),
          ];
          let [substitution, positioning] = feature_var_tables;

          for (table, state, shift) in [
            (substitution, &self.gsub, 0),
            (positioning, &self.gpos, Self::GPOS_SHIFT),
          ] {
            let index = match table {
              Some(variations) => {
                let Ok(index) = feature_variation_index(&variations, coords);
                index
              }
              None => None,
            };
            let table_status = match index {
              Some(index_2) => {
                state.store(index_2, atomic::Ordering::Release);
                Self::PRESENT
              }
              None => Self::ABSENT,
            };
            status |= table_status.wrapping_shl(shift);
          }

          self.status.store(status, atomic::Ordering::Release);
        }
        ::core::option::Option::None => {}
      }

      Ok(match ::core::primitive::bool::then_some(status != Self::BOTH_ABSENT, ()) {
        ::core::option::Option::Some(()) => {
          let gsub_status = status & 0xFFFF;
          let gpos_status = ::core::primitive::u32::wrapping_shr(status, Self::GPOS_SHIFT) & 0xFFFF;
          FontFeatureVariations {
            gsub: (gsub_status == Self::PRESENT).then(|| self.gsub.load(atomic::Ordering::Acquire)),
            gpos: (gpos_status == Self::PRESENT).then(|| self.gpos.load(atomic::Ordering::Acquire)),
          }
        }
        ::core::option::Option::None => FontFeatureVariations::default(),
      })
  }
}

#[must_use]
pub(crate) fn feature_variation_index(
  feature_vars: &FeatureVariations,
  coords: &[NormalizedCoord],
) -> core::result::Result<Option<u32>, console_core_never::Never> {
    let Ok(feature_variation_records) = feature_vars.feature_variation_records();

    for (index, rec) in feature_variation_records.iter().enumerate() {
      let Ok(condition_set_offset) = rec.condition_set_offset();
      let Ok(value_1) = condition_set_offset.nullness();
      let value_1_2 = matches!(value_1, font_types::Nullness::Null);

      match ::core::primitive::bool::then_some(value_1_2, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(Some({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(index);
            migrated_value
          }));
        }
        ::core::option::Option::None => {}
      }

      let Ok(condition_data) = feature_vars.offset_data();
      let condition_set = match rec.condition_set(condition_data) {
        Ok(Some(condition_set)) => condition_set,
        Ok(None) => continue,
        Err(_invalid_condition_set) => continue,
      };

      let Ok(condition_set_conditions) = condition_set.conditions();
      let Ok(conditions) = condition_set_conditions.iter();

      match ::core::primitive::bool::then_some({
        conditions.filter_map(Result::ok).all(|cond| match cond {
          Condition::Format1AxisRange(format1) => {
            let Ok(axis_index) = format1.axis_index();
            let Ok(axis) = console_core_number_conversion::index(u64::from(axis_index));
            let coord = match coords.get(axis) {
              Some(coordinate) => *coordinate,
              None => NormalizedCoord::ZERO,
            };
            let Ok(filter_range_min_value) = format1.filter_range_min_value();

            coord >= filter_range_min_value
              && coord
                <= ({
                  let Ok(explicit_value) = format1.filter_range_max_value();
                  explicit_value
                })
          }
          Condition::Format2VariableValue(_)
          | Condition::Format3And(_)
          | Condition::Format4Or(_)
          | Condition::Format5Negate(_) => false,
        })
      }, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(Some({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(index);
            migrated_value
          }));
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
  use super::*;
  use core::sync::atomic::Ordering;

  #[test]
  fn named_instances() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = Font::new(font_test_data::CANTARELL_VF_TRIMMED, 0)?;
    let cases = [(0, 100.0), (1, 300.0), (2, 400.0), (3, 700.0), (4, 800.0)];

    for (index, weight) in cases {
      let named_instance = FontInstance::builder(&font).named_instance(index).build();
      let var_instance = FontInstance::builder(&font)
        .variations([("wght", weight)])
        .build();
      assert_eq!(
        named_instance.normalized_coords(),
        var_instance.normalized_coords(),
        "index={index}"
      );
    }

    let invalid_instance = FontInstance::builder(&font).named_instance(5).build();
    assert!(
      invalid_instance.normalized_coords().is_empty(),
      "out of bounds index should give default instance"
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn named_instance_with_overrides_override_named_value()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = Font::new(font_test_data::MATERIAL_SYMBOLS_SUBSET, 0)?;
    let actual = FontInstance::builder(&font)
      .named_instance_with_overrides(3, [("FILL", 1.0)])
      .build();
    let expected = FontInstance::builder(&font)
      .variations([
        ("FILL", 1.0),
        ("GRAD", 0.0),
        ("opsz", 24.0),
        ("wght", 400.0),
      ])
      .build();
    assert_eq!(actual.normalized_coords(), expected.normalized_coords());
    ::core::result::Result::Ok(())
  }

  #[test]
  fn named_instance_with_overrides_invalid_index_uses_overrides_only()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = Font::new(font_test_data::MATERIAL_SYMBOLS_SUBSET, 0)?;
    let actual = FontInstance::builder(&font)
      .named_instance_with_overrides(999, [("FILL", 1.0), ("ZZZZ", 123.0)])
      .build();
    let expected = FontInstance::builder(&font)
      .variations([("FILL", 1.0), ("ZZZZ", 123.0)])
      .build();
    assert_eq!(actual.normalized_coords(), expected.normalized_coords());
    assert_eq!(actual.normalized_coords().len(), 4);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn named_instance_with_overrides_overwrites_previous_settings()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = Font::new(font_test_data::MATERIAL_SYMBOLS_SUBSET, 0)?;
    let actual = FontInstance::builder(&font)
      .variations([("FILL", 0.0), ("wght", 100.0)])
      .named_instance_with_overrides(5, [("GRAD", -25.0)])
      .build();
    let expected = FontInstance::builder(&font)
      .variations([
        ("FILL", 0.0),
        ("GRAD", -25.0),
        ("opsz", 24.0),
        ("wght", 600.0),
      ])
      .build();
    assert_eq!(actual.normalized_coords(), expected.normalized_coords());
    ::core::result::Result::Ok(())
  }

  #[test]
  fn feature_variations() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(Font::new(font_test_data::MATERIAL_SYMBOLS_SUBSET, 0), "fixture value")?;

    let cases = [
      (0.0, [None, None]),
      (0.5, [None, None]),
      (0.98, [None, None]),
      (0.99, [Some(0), None]),
      (1.0, [Some(0), None]),
    ];
    for (fill, [gsub, gpos]) in cases {
      let instance = FontInstance::builder(&font)
        .variations([("FILL", fill)])
        .build();
      let feature_vars = instance.feature_variations();
      let actual = [feature_vars.gsub(), feature_vars.gpos()];
      assert_eq!(actual, [gsub, gpos], "fill={fill}");
    }
  ;

    Ok(())
  }

  #[test]
  fn feature_variation_cache_marks_both_absent()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = Font::new(font_test_data::MATERIAL_SYMBOLS_SUBSET, 0)?;
    let instance = FontInstance::builder(&font)
      .variations([("FILL", 0.5)])
      .build();
    assert_eq!(instance.feature_vars.status.load(Ordering::SeqCst), 0);
    assert_eq!(
      instance.feature_variations(),
      FontFeatureVariations::default()
    );
    assert_eq!(
      instance.feature_vars.status.load(Ordering::SeqCst),
      FeatureVarsStorage::BOTH_ABSENT
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn feature_variation_cache_is_thread_safe_and_stable()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = Font::new(font_test_data::MATERIAL_SYMBOLS_SUBSET, 0)?;
    let instance = FontInstance::builder(&font)
      .variations([("FILL", 1.0)])
      .build();
    std::thread::scope(|scope| {
      for _ in 0..8 {
        #[cfg_attr(
          dylint_lib = "explicit103_no_thread_per_item",
          expect(
            explicit103_no_thread_per_item,
            reason = "this race test starts a fixed set of contenders and each one repeatedly reads the same immutable font instance"
          )
        )]
        scope.spawn(|| {
          for _ in 0..64 {
            let vars = instance.feature_variations();
            assert_eq!(
              vars,
              FontFeatureVariations {
                gsub: Some(0),
                gpos: None
              }
            );
          }
        });
      }
    });
    let status = instance.feature_vars.status.load(Ordering::SeqCst);
    assert_eq!(status & 0xFFFF, FeatureVarsStorage::PRESENT);
    assert_eq!(
      ::core::primitive::u32::wrapping_shr(status, FeatureVarsStorage::GPOS_SHIFT) & 0xFFFF,
      FeatureVarsStorage::ABSENT
    );
    assert_eq!(instance.feature_vars.gsub.load(Ordering::SeqCst), 0);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn variations_last_value_wins_and_unknown_axis_ignored()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = Font::new(font_test_data::CANTARELL_VF_TRIMMED, 0)?;
    let expected = FontInstance::builder(&font)
      .variations([("wght", 700.0)])
      .build();
    let repeated_axis = FontInstance::builder(&font)
      .variations([("wght", 100.0), ("wght", 700.0)])
      .build();
    assert_eq!(
      repeated_axis.normalized_coords(),
      expected.normalized_coords()
    );
    let unknown_axis = FontInstance::builder(&font)
      .variations([("wght", 700.0), ("ZZZZ", 123.0)])
      .build();
    assert_eq!(
      unknown_axis.normalized_coords(),
      expected.normalized_coords()
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn later_variation_call_overwrites_previous()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = Font::new(font_test_data::CANTARELL_VF_TRIMMED, 0)?;
    let overwritten = FontInstance::builder(&font)
      .variations([("wght", 700.0)])
      .variations([("wght", 100.0)])
      .build();
    let expected = FontInstance::builder(&font)
      .variations([("wght", 100.0)])
      .build();
    assert_eq!(
      overwritten.normalized_coords(),
      expected.normalized_coords()
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn normalized_coords_empty_resets_to_default_instance()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = Font::new(font_test_data::MATERIAL_SYMBOLS_SUBSET, 0)?;
    let instance = FontInstance::builder(&font).normalized_coords([]).build();
    assert!(instance.normalized_coords().is_empty());
    ::core::result::Result::Ok(())
  }

  #[test]
  fn normalized_coords_truncates_and_pads() -> ::core::result::Result<(), crate::read::ReadError>
  {
    let font = Font::new(font_test_data::MATERIAL_SYMBOLS_SUBSET, 0)?;
    let axis_count = {
      let Ok(font_axis_count) = match font.tables().fvar() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(read_error);
        }
      }
      .axis_count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(font_axis_count);
      migrated_value
    };
    let values = [0.25, -0.5, 1.0, 0.75, -0.25].map(|coordinate| {
      let Ok(value) = NormalizedCoord::from_f32(coordinate);
      value
    });
    let instance = FontInstance::builder(&font)
      .normalized_coords(values)
      .build();
    let coords = instance.normalized_coords();
    assert_eq!(
      coords.len(),
      ({
        let Ok(explicit_index) = console_core_number_conversion::index(axis_count);
        explicit_index
      })
    );
    let Ok(input_count) = console_core_number_conversion::fixed_index(values.len());
    let Ok(copied) = console_core_number_conversion::index(input_count.min(axis_count));
    assert_eq!(
      &*match <[_]>::get(&*coords, ..copied) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None =>
          return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds),
      },
      &*match <[_]>::get(&values, ..copied) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None =>
          return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds),
      }
    );
    assert!(
      (*match <[_]>::get(&*coords, copied..) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None =>
          return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds),
      })
      .iter()
      .all(|&coord| coord == NormalizedCoord::ZERO)
    );
    ::core::result::Result::Ok(())
  }
}
