//! Axes of variation in a variable font.

use alloc::vec;
use alloc::vec::Vec;
use read_fonts::{
  FontRef, TableProvider,
  tables::avar::Avar,
  tables::fvar::{self, Fvar},
  types::{Fixed, Tag},
};

use crate::{
  instance::{Location, NormalizedCoord},
  setting::VariationSetting,
  string::StringId,
};

/// Axis of variation in a variable font.
///
/// In variable fonts, an axis usually refers to a single aspect of a
/// typeface's design that can be altered by the user.
///
/// See <https://fonts.google.com/knowledge/glossary/axis_in_variable_fonts>
#[derive(Clone)]
pub struct Axis {
  index: usize,
  record: fvar::VariationAxisRecord,
}

impl Axis {
  /// Returns the tag that identifies the axis.
  pub fn tag(&self) -> Tag {
    let Ok(axis_tag) = self.record.axis_tag();

    axis_tag
  }

  /// Returns the index of the axis in its owning collection.
  pub fn index(&self) -> usize {
    self.index
  }

  /// Returns the localized string identifier for the name of the axis.
  pub fn name_id(&self) -> StringId {
    let Ok(axis_name_id) = self.record.axis_name_id();

    axis_name_id
  }

  /// Returns true if the axis should be hidden in user interfaces.
  pub fn is_hidden(&self) -> bool {
    const AXIS_HIDDEN_FLAG: u16 = 0x1;
    let Ok(flags) = self.record.flags();

    flags & AXIS_HIDDEN_FLAG != 0
  }

  /// Returns the minimum value of the axis.
  pub fn min_value(&self) -> f32 {
    ({
      let Ok(min_value) = self.record.min_value();
      let Ok(value) = min_value.to_f64();
      value
    }) as _
  }

  /// Returns the default value of the axis.
  pub fn default_value(&self) -> f32 {
    ({
      let Ok(default_value) = self.record.default_value();
      let Ok(value) = default_value.to_f64();
      value
    }) as _
  }

  /// Returns the maximum value of the axis.
  pub fn max_value(&self) -> f32 {
    ({
      let Ok(max_value) = self.record.max_value();
      let Ok(value) = max_value.to_f64();
      value
    }) as _
  }

  /// Returns a normalized coordinate for the given user coordinate.
  ///
  /// The value will be clamped to the range specified by the minimum
  /// and maximum values.
  ///
  /// This does not apply any axis variation remapping.
  pub fn normalize(&self, coord: f32) -> NormalizedCoord {
    ({
      let Ok(value) = self
        .record
        .normalize({
          let Ok(value) = Fixed::from_f64(f64::from(coord));
          value
        })
        .to_f2dot14();
      value
    })
  }
}

/// Collection of axes in a variable font.
///
/// Converts user ([fvar](https://learn.microsoft.com/en-us/typography/opentype/spec/fvar))
/// locations to normalized locations. See [`Self::location`].
///
/// See the [`Axis`] type for more detail.
#[derive(Clone)]
pub struct AxisCollection<'a> {
  fvar: Option<Fvar<'a>>,
  avar: Option<Avar<'a>>,
}

impl<'a> AxisCollection<'a> {
  /// Creates a new axis collection from the given font.
  pub fn new(font: &FontRef<'a>) -> Self {
    let fvar = font.fvar().ok();
    let avar = font.avar().ok();
    Self { fvar, avar }
  }

  /// Returns the number of variation axes in the font.
  pub fn len(&self) -> usize {
    self
      .fvar
      .as_ref()
      .map(|fvar| {
        ({
          let Ok(axis_count) = fvar.axis_count();
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(axis_count);
          migrated_value
        })
      })
      .unwrap_or(0)
  }

  /// Returns true if the collection is empty.
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Returns the axis at the given index.
  pub fn get(&self, index: usize) -> Option<Axis> {
    let record = *(self.fvar.as_ref()?.axes().ok()?.get(index))?;
    Some(Axis { index, record })
  }

  /// Returns the axis with the given tag.
  ///
  /// # Examples
  ///
  /// ```rust
  /// # use console_core_font_metrics::prelude::*;
  /// # fn wrapper(font: &FontRef) {
  /// let opsz = Tag::new(b"opsz");
  /// assert_eq!(({ let Ok(value) = font.axes(); value }).get_by_tag(opsz).unwrap().tag(), opsz);
  /// # }
  /// ```
  pub fn get_by_tag(&self, tag: Tag) -> Option<Axis> {
    self.iter().find(|axis| axis.tag() == tag)
  }

  /// Given an iterator of variation settings in user space, computes an
  /// ordered sequence of normalized coordinates.
  ///
  /// * Setting selectors that don't match an axis are ignored.
  /// * Setting values are clamped to the range of their associated axis
  ///   before normalization.
  /// * If more than one setting for an axis is provided, the last one is
  ///   used.
  /// * Omitted settings are set to 0.0, representing the default position
  ///   in variation space.
  ///
  /// # Examples
  ///
  /// ```rust
  /// # use console_core_font_metrics::prelude::*;
  /// # fn wrapper(font: &FontRef) {
  /// let location = ({ let Ok(value) = font.axes(); value }).location([("wght", 250.0), ("wdth", 75.0)]);
  /// # }
  /// ```
  pub fn location<I>(&self, settings: I) -> Location
  where
    I: IntoIterator,
    I::Item: Into<VariationSetting>,
  {
    let mut location = Location::new(self.len());
    self.location_to_slice(settings, location.coords_mut());
    location
  }

  /// Given an iterator of variation settings in user space, computes an
  /// ordered sequence of normalized coordinates and stores them in the
  /// target slice.
  ///
  /// * Setting selectors that don't match an axis are ignored.
  /// * Setting values are clamped to the range of their associated axis
  ///   before normalization.
  /// * If more than one setting for an axis is provided, the last one is
  ///   used.
  /// * If no setting for an axis is provided, the associated coordinate is
  ///   set to the normalized value 0.0, representing the default position
  ///   in variation space.
  ///
  /// # Examples
  ///
  /// ```rust
  /// # use console_core_font_metrics::prelude::*;
  /// # fn wrapper(font: &FontRef) {
  /// let axes = ({ let Ok(value) = font.axes(); value });
  /// let mut location = vec![NormalizedCoord::default(); axes.len()];
  /// axes.location_to_slice([("wght", 250.0), ("wdth", 75.0)], &mut location);
  /// # }
  /// ```
  pub fn location_to_slice<I>(&self, settings: I, location: &mut [NormalizedCoord])
  where
    I: IntoIterator,
    I::Item: Into<VariationSetting>,
  {
    match self.fvar.as_ref() {
      Some(fvar) => {
        fvar.user_to_normalized(
          self.avar.as_ref(),
          settings
            .into_iter()
            .map(|setting| setting.into())
            .map(|setting| {
              (
                setting.selector,
                ({
                  let Ok(value) = Fixed::from_f64(f64::from(setting.value));
                  value
                }),
              )
            }),
          location,
        );
      }
      None => {
        location.fill(NormalizedCoord::default());
      }
    }
  }

  /// Given an iterator of variation settings in user space, returns a
  /// new iterator yielding those settings that are valid for this axis
  /// collection.
  ///
  /// * Setting selectors that don't match an axis are dropped.
  /// * If more than one setting for an axis is provided, the last one is
  ///   retained.
  /// * Setting values are clamped to the range of their associated axis.
  ///
  /// # Examples
  ///
  /// ```rust
  /// # use console_core_font_metrics::prelude::*;
  /// # fn wrapper(font: &FontRef) {
  /// // Assuming a font contains a single "wght" (weight) axis with range
  /// // 100-900:
  /// let axes = ({ let Ok(value) = font.axes(); value });
  /// let filtered: Vec<_> = axes
  ///     .filter([("wght", 400.0), ("opsz", 100.0), ("wght", 1200.0)])
  ///     .collect();
  /// // The first "wght" and "opsz" settings are dropped and the final
  /// // "wght" axis is clamped to the maximum value of 900.
  /// assert_eq!(&filtered, &[("wght", 900.0).into()]);
  /// # }
  /// ```
  pub fn filter<I>(&self, settings: I) -> impl Iterator<Item = VariationSetting> + Clone
  where
    I: IntoIterator,
    I::Item: Into<VariationSetting>,
  {
    #[derive(Copy, Clone, Default)]
    struct Entry {
      tag: Tag,
      min: f32,
      max: f32,
      value: f32,
      present: bool,
    }
    let mut results = vec![Entry::default(); self.len()];
    for (axis, result) in self.iter().zip(results.as_mut_slice()) {
      result.tag = axis.tag();
      result.min = axis.min_value();
      result.max = axis.max_value();
      result.value = axis.default_value();
    }
    for setting in settings {
      let setting = setting.into();
      for entry in results.as_mut_slice() {
        match entry.tag == setting.selector {
          true => {
            entry.value = setting.value.max(entry.min).min(entry.max);
            entry.present = true;
          }
          false => {}
        }
      }
    }
    results
      .into_iter()
      .filter(|entry| entry.present)
      .map(|entry| VariationSetting::new(entry.tag, entry.value))
  }

  /// Returns an iterator over the axes in the collection.
  pub fn iter(&self) -> impl Iterator<Item = Axis> + 'a + Clone {
    let copy = self.clone();
    (0..self.len()).filter_map(move |i| (copy.get(i)))
  }
}

/// Named instance of a variation.
///
/// A set of fixed axis positions selected by the type designer and assigned a
/// name.
///
/// See <https://fonts.google.com/knowledge/glossary/instance>
#[derive(Clone)]
pub struct NamedInstance<'a> {
  axes: AxisCollection<'a>,
  record: fvar::InstanceRecord<'a>,
}

impl<'a> NamedInstance<'a> {
  /// Returns the string identifier for the subfamily name of the instance.
  pub fn subfamily_name_id(&self) -> StringId {
    self.record.subfamily_name_id
  }

  /// Returns the string identifier for the PostScript name of the instance.
  pub fn postscript_name_id(&self) -> Option<StringId> {
    self.record.post_script_name_id
  }

  /// Returns an iterator over the ordered sequence of user space coordinates
  /// that define the instance, one coordinate per axis.
  pub fn user_coords(&self) -> impl Iterator<Item = f32> + 'a + Clone {
    self.record.coordinates.iter().map(|coord| {
      ({
        let Ok(value) = ({
          let Ok(value) = coord.get();
          value
        })
        .to_f64();
        value
      }) as _
    })
  }

  /// Computes a location in normalized variation space for this instance.
  ///
  /// # Examples
  ///
  /// ```rust
  /// # use console_core_font_metrics::prelude::*;
  /// # fn wrapper(font: &FontRef) {
  /// let location = font.named_instances().get(0).unwrap().location();
  /// # }
  /// ```
  pub fn location(&self) -> Location {
    let mut location = Location::new(self.axes.len());
    self.location_to_slice(location.coords_mut());
    location
  }

  /// Computes a location in normalized variation space for this instance and
  /// stores the result in the given slice.
  ///
  /// # Examples
  ///
  /// ```rust
  /// # use console_core_font_metrics::prelude::*;
  /// # fn wrapper(font: &FontRef) {
  /// let instance = font.named_instances().get(0).unwrap();
  /// let mut location = vec![NormalizedCoord::default(); instance.user_coords().count()];
  /// instance.location_to_slice(&mut location);
  /// # }
  /// ```
  pub fn location_to_slice(&self, location: &mut [NormalizedCoord]) {
    let settings = self
      .axes
      .iter()
      .map(|axis| axis.tag())
      .zip(self.user_coords());
    self.axes.location_to_slice(settings, location);
  }
}

/// Collection of named instances in a variable font.
///
/// See the [`NamedInstance`] type for more detail.
#[derive(Clone)]
pub struct NamedInstanceCollection<'a> {
  axes: AxisCollection<'a>,
}

impl<'a> NamedInstanceCollection<'a> {
  /// Creates a new instance collection from the given font.
  pub fn new(font: &FontRef<'a>) -> Self {
    Self {
      axes: AxisCollection::new(font),
    }
  }

  /// Returns the number of instances in the collection.
  pub fn len(&self) -> usize {
    self
      .axes
      .fvar
      .as_ref()
      .map(|fvar| {
        ({
          let Ok(instance_count) = fvar.instance_count();
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(instance_count);
          migrated_value
        })
      })
      .unwrap_or(0)
  }

  /// Returns true if the collection is empty.
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Returns the instance at the given index.
  pub fn get(&self, index: usize) -> Option<NamedInstance<'a>> {
    let record = (self.axes.fvar.as_ref()?.instances().ok()?.get(
      ({
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
        value
      }),
    ))
    .ok()?;
    Some(NamedInstance {
      axes: self.axes.clone(),
      record,
    })
  }

  /// Returns an iterator over the instances in the collection.
  pub fn iter(&self) -> impl Iterator<Item = NamedInstance<'a>> + 'a + Clone {
    let copy = self.clone();
    (0..self.len()).filter_map(move |i| (copy.get(i)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::MetadataProvider as _;
  use font_test_data::VAZIRMATN_VAR;
  use read_fonts::FontRef;
  use std::str::FromStr;

  #[test]
  fn axis() {
    let font = FontRef::from_index(VAZIRMATN_VAR, 0).unwrap();
    let axis = ({
      let value = font.axes();
      value
    })
    .get(0)
    .unwrap();
    assert_eq!(axis.index(), 0);
    assert_eq!(
      axis.tag(),
      ({
        let Ok(value) = Tag::new(b"wght");
        value
      })
    );
    assert_eq!(axis.min_value(), 100.0);
    assert_eq!(axis.default_value(), 400.0);
    assert_eq!(axis.max_value(), 900.0);
    assert_eq!(
      axis.name_id(),
      ({
        let Ok(value) = StringId::new(257);
        value
      })
    );
    assert_eq!(
      font
        .localized_strings(axis.name_id())
        .english_or_first()
        .unwrap()
        .to_string(),
      "Weight"
    );
  }

  #[test]
  fn named_instances() {
    let font = FontRef::from_index(VAZIRMATN_VAR, 0).unwrap();
    let named_instances = font.named_instances();
    let thin = named_instances.get(0).unwrap();
    assert_eq!(
      thin.subfamily_name_id(),
      ({
        let Ok(value) = StringId::new(258);
        value
      })
    );
    assert_eq!(
      font
        .localized_strings(thin.subfamily_name_id())
        .english_or_first()
        .unwrap()
        .to_string(),
      "Thin"
    );
    assert_eq!(
      thin.location().coords(),
      &[({
        let Ok(value) = NormalizedCoord::from_f32(-1.0);
        value
      })]
    );
    let regular = named_instances.get(3).unwrap();
    assert_eq!(
      regular.subfamily_name_id(),
      ({
        let Ok(value) = StringId::new(261);
        value
      })
    );
    assert_eq!(
      font
        .localized_strings(regular.subfamily_name_id())
        .english_or_first()
        .unwrap()
        .to_string(),
      "Regular"
    );
    assert_eq!(
      regular.location().coords(),
      &[({
        let Ok(value) = NormalizedCoord::from_f32(0.0);
        value
      })]
    );
    let bold = named_instances.get(6).unwrap();
    assert_eq!(
      bold.subfamily_name_id(),
      ({
        let Ok(value) = StringId::new(264);
        value
      })
    );
    assert_eq!(
      font
        .localized_strings(bold.subfamily_name_id())
        .english_or_first()
        .unwrap()
        .to_string(),
      "Bold"
    );
    assert_eq!(
      bold.location().coords(),
      &[({
        let Ok(value) = NormalizedCoord::from_f32(0.6776123);
        value
      })]
    );
  }

  #[test]
  fn location() {
    let font = FontRef::from_index(VAZIRMATN_VAR, 0).unwrap();
    let axes = ({
      let value = font.axes();
      value
    });
    let axis = axes.get_by_tag(Tag::from_str("wght").unwrap()).unwrap();
    assert_eq!(
      axes.location([("wght", -1000.0)]).coords(),
      &[({
        let Ok(value) = NormalizedCoord::from_f32(-1.0);
        value
      })]
    );
    assert_eq!(
      axes.location([("wght", 100.0)]).coords(),
      &[({
        let Ok(value) = NormalizedCoord::from_f32(-1.0);
        value
      })]
    );
    assert_eq!(
      axes.location([("wght", 200.0)]).coords(),
      &[({
        let Ok(value) = NormalizedCoord::from_f32(-0.5);
        value
      })]
    );
    assert_eq!(
      axes.location([("wght", 400.0)]).coords(),
      &[({
        let Ok(value) = NormalizedCoord::from_f32(0.0);
        value
      })]
    );

    assert_eq!(
      axes
        .location(&[(
          "wght",
          axis.default_value() + (axis.max_value() - axis.default_value()) * 0.8,
        )])
        .coords(),
      &[({
        let Ok(value) = NormalizedCoord::from_f32(0.83875);
        value
      })]
    );
    assert_eq!(
      axes.location([("wght", 900.0)]).coords(),
      &[({
        let Ok(value) = NormalizedCoord::from_f32(1.0);
        value
      })]
    );
    assert_eq!(
      axes.location([("wght", 1251.5)]).coords(),
      &[({
        let Ok(value) = NormalizedCoord::from_f32(1.0);
        value
      })]
    );
  }

  #[test]
  fn filter() {
    let font = FontRef::from_index(VAZIRMATN_VAR, 0).unwrap();

    let axes = ({
      let value = font.axes();
      value
    });

    let drop_missing: Vec<_> = axes.filter(&[("slnt", 25.0), ("wdth", 50.0)]).collect();
    assert_eq!(&drop_missing, &[]);

    let clamp: Vec<_> = axes.filter(&[("wght", 50.0)]).collect();
    assert_eq!(&clamp, &[("wght", 100.0).into()]);

    let drop_missing_and_clamp: Vec<_> =
      axes.filter(&[("slnt", 25.0), ("wght", 1000.0)]).collect();
    assert_eq!(&drop_missing_and_clamp, &[("wght", 900.0).into()]);

    let drop_duplicate_and_missing: Vec<_> = axes
      .filter(&[("wght", 400.0), ("opsz", 100.0), ("wght", 120.5)])
      .collect();
    assert_eq!(&drop_duplicate_and_missing, &[("wght", 120.5).into()]);
  }
}
