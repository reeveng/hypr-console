//! Hinting parameters.

use types::Fixed;

pub const MAX_BLUE_VALUES: ::console_core_api_compatibility::NativeSize = 7;

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct Blues {
  values: [(Fixed, Fixed); MAX_BLUE_VALUES],
  len: u32,
}

impl Blues {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Blues::new` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn new(mut values: impl Iterator<Item = Fixed>) -> Self {
    let mut blues = Self::default();

    for slot in &mut blues.values {
      let first = match values.next() {
        Some(value) => value,
        None => break,
      };
      let second = match values.next() {
        Some(value) => value,
        None => break,
      };
      *slot = (first, second);
      blues.len = blues.len.saturating_add(1);
    }

    blues
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Blues::values` method returns a borrow tied to the source lifetime directly")
  )]
  pub fn values(&self) -> &[(Fixed, Fixed)] {
    let Ok(indices) = console_core_number_conversion::indices_to(..u64::from(self.len));

    match self.values.get(indices) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => &[],
    }
  }
}

pub const MAX_STEM_SNAPS: ::console_core_api_compatibility::NativeSize = 12;

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct StemSnaps {
  values: [Fixed; MAX_STEM_SNAPS],
  len: u32,
}

impl StemSnaps {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `StemSnaps::new` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn new(values: impl Iterator<Item = Fixed>) -> Self {
    let mut snaps = Self::default();

    for (value, target_value) in values.take(MAX_STEM_SNAPS).zip(&mut snaps.values) {
      *target_value = value;
      match (1, &mut snaps.len) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    snaps
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `StemSnaps::values` method returns a borrow tied to the source lifetime directly")
  )]
  pub fn values(&self) -> &[Fixed] {
    let Ok(indices) = console_core_number_conversion::indices_to(..u64::from(self.len));

    match self.values.get(indices) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => &[],
    }
  }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HintingParams {
  pub blues: Blues,
  pub family_blues: Blues,
  pub other_blues: Blues,
  pub family_other_blues: Blues,
  pub blue_scale: Fixed,
  pub blue_shift: Fixed,
  pub blue_fuzz: Fixed,
  pub language_group: i32,
}

impl Default for HintingParams {
  fn default() -> Self {
    let Ok(value_1) = Fixed::from_f64(0.039625);
    let Ok(value_2) = Fixed::from_i32(7);

    Self {
      blues: Blues::default(),
      other_blues: Blues::default(),
      family_blues: Blues::default(),
      family_other_blues: Blues::default(),

      blue_scale: value_1,
      blue_shift: value_2,
      blue_fuzz: Fixed::ONE,
      language_group: 0,
    }
  }
}
