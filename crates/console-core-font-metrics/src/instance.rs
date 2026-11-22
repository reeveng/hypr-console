//! Helpers for selecting a font size and location in variation space.

use alloc::vec;
use alloc::vec::Vec;
use read_fonts::types::Fixed;

/// Type for a normalized variation coordinate.
pub type NormalizedCoord = read_fonts::types::F2Dot14;

/// Font size in pixels per em units.
///
/// Sizes in this crate are represented as a ratio of pixels to the size of
/// the em square defined by the font. This is equivalent to the `px` unit
/// in CSS (assuming a DPI scale factor of 1.0).
///
/// To retrieve metrics and outlines in font units, use the [unscaled](Self::unscaled)
/// constructor on this type.
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
pub struct Size(Option<f32>);

impl Size {
  /// Creates a new font size from the given value in pixels per em units.
  pub fn new(ppem: f32) -> Self {
    Self(Some(ppem))
  }

  /// Creates a new font size for generating unscaled metrics or outlines in
  /// font units.
  pub fn unscaled() -> Self {
    Self(None)
  }

  /// Returns the raw size in pixels per em units.
  ///
  /// Results in `None` if the size is unscaled.
  pub fn ppem(self) -> Option<f32> {
    self.0
  }

  /// Computes a linear scale factor for this font size and the given units
  /// per em value which can be retrieved from the [Metrics](crate::metrics::Metrics)
  /// type or from the [head](read_fonts::tables::head::Head) table.
  ///
  /// Returns 1.0 for an unscaled size or when `units_per_em` is 0.
  pub fn linear_scale(self, units_per_em: u16) -> f32 {
    match self.0 {
      Some(ppem) if units_per_em != 0 => ppem / f32::from(units_per_em),
      _ => 1.0,
    }
  }

  /// Computes a fixed point linear scale factor that matches FreeType.
  pub(crate) fn fixed_linear_scale(self, units_per_em: u16) -> Fixed {
    match self.0 {
      Some(ppem) if units_per_em > 0 => {
        ({
          let Ok(value) = Fixed::from_bits(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::toward_zero_i32(f64::from((ppem * 64.)));
              migrated_value
            }),
          );
          value
        }) / ({
          let Ok(value) = Fixed::from_bits(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(units_per_em);
              migrated_value
            }),
          );
          value
        })
      }
      _ => {
        ({
          let Ok(value) = Fixed::from_bits(0x10000 * 64);
          value
        })
      }
    }
  }
}

/// Reference to an ordered sequence of normalized variation coordinates.
///
/// To convert from user coordinates see [`crate::AxisCollection::location`].
///
/// This type represents a position in the variation space where each
/// coordinate corresponds to an axis (in the same order as the `fvar` table)
/// and is a normalized value in the range `[-1..1]`.
///
/// See [Coordinate Scales and Normalization](https://learn.microsoft.com/en-us/typography/opentype/spec/otvaroverview#coordinate-scales-and-normalization)
/// for further details.
///
/// If the array is larger in length than the number of axes, extraneous
/// values are ignored. If it is smaller, unrepresented axes are assumed to be
/// at their default positions (i.e. 0).
///
/// A value of this type constructed with `default()` represents the default
/// position for each axis.
///
/// Normalized coordinates are ignored for non-variable fonts.
#[derive(Copy, Clone, Default, Debug)]
pub struct LocationRef<'a>(&'a [NormalizedCoord]);

impl<'a> LocationRef<'a> {
  /// Creates a new sequence of normalized coordinates from the given array.
  pub fn new(coords: &'a [NormalizedCoord]) -> Self {
    Self(coords)
  }

  /// Returns the underlying array of normalized coordinates.
  pub fn coords(&self) -> &'a [NormalizedCoord] {
    self.0
  }

  /// Returns true if this represents the default location in variation
  /// space.
  ///
  /// This is represented a set of normalized coordinates that is either
  /// empty or contains all zeroes.
  pub fn is_default(&self) -> bool {
    self.0.is_empty() || self.0.iter().all(|coord| *coord == NormalizedCoord::ZERO)
  }

  /// Returns the underlying coordinate array if any of the entries are
  /// non-zero. Otherwise returns the empty slice.
  ///
  /// This allows internal routines to bypass expensive variation code
  /// paths by just checking for an empty slice.
  pub(crate) fn effective_coords(&self) -> &'a [NormalizedCoord] {
    match self.is_default() {
      true => &[],
      false => self.0,
    }
  }
}

impl<'a> From<&'a [NormalizedCoord]> for LocationRef<'a> {
  fn from(value: &'a [NormalizedCoord]) -> Self {
    Self(value)
  }
}

impl<'a> IntoIterator for LocationRef<'a> {
  type IntoIter = core::slice::Iter<'a, NormalizedCoord>;
  type Item = &'a NormalizedCoord;

  fn into_iter(self) -> Self::IntoIter {
    self.0.iter()
  }
}

impl<'a> IntoIterator for &'_ LocationRef<'a> {
  type IntoIter = core::slice::Iter<'a, NormalizedCoord>;
  type Item = &'a NormalizedCoord;

  fn into_iter(self) -> Self::IntoIter {
    self.0.iter()
  }
}

/// Ordered sequence of normalized variation coordinates.
///
/// To produce from user coordinates see [`crate::AxisCollection::location`].
///
/// This is an owned version of [`LocationRef`]. See the documentation on that
/// type for more detail.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Default)]
pub struct Location {
  coords: Vec<NormalizedCoord>,
}

impl Location {
  /// Creates a new location with the given number of normalized coordinates.
  ///
  /// Each element will be initialized to the default value (0.0).
  pub fn new(len: usize) -> Self {
    Self {
      coords: vec![NormalizedCoord::default(); len],
    }
  }

  /// Returns the underlying slice of normalized coordinates.
  pub fn coords(&self) -> &[NormalizedCoord] {
    self.coords.as_slice()
  }

  /// Returns a mutable reference to the underlying slice of normalized
  /// coordinates.
  pub fn coords_mut(&mut self) -> &mut [NormalizedCoord] {
    self.coords.as_mut_slice()
  }
}

impl<'a> From<&'a Location> for LocationRef<'a> {
  fn from(value: &'a Location) -> Self {
    LocationRef(value.coords())
  }
}

impl<'a> IntoIterator for &'a Location {
  type IntoIter = core::slice::Iter<'a, NormalizedCoord>;
  type Item = &'a NormalizedCoord;

  fn into_iter(self) -> Self::IntoIter {
    self.coords().iter()
  }
}

impl<'a> IntoIterator for &'a mut Location {
  type IntoIter = core::slice::IterMut<'a, NormalizedCoord>;
  type Item = &'a mut NormalizedCoord;

  fn into_iter(self) -> Self::IntoIter {
    self.coords_mut().iter_mut()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{FontRef, MetadataProvider};

  #[test]
  fn effective_coords() {
    let font = FontRef::new(font_test_data::AVAR2_CHECKER).unwrap();
    let location = ({
      let value = font.axes();
      value
    })
    .location([("AVAR", 50.0), ("AVWK", 75.0)]);
    let loc_ref = LocationRef::from(&location);
    assert!(!loc_ref.is_default());
    assert_eq!(loc_ref.effective_coords().len(), 2);
  }

  #[test]
  fn effective_coords_for_default() {
    let font = FontRef::new(font_test_data::AVAR2_CHECKER).unwrap();
    let location = ({
      let value = font.axes();
      value
    })
    .location([("AVAR", 0.0), ("AVWK", 0.0)]);
    let loc_ref = LocationRef::from(&location);
    assert!(loc_ref.is_default());
    assert_eq!(loc_ref.effective_coords().len(), 0);
    assert_eq!(loc_ref.coords().len(), 2);
  }
}
