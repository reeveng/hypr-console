//! Primitive and font identifier domains preserve their fixed-width bounds.
//!
//! Each is continuous: every value from zero to its width's maximum is a member.

use super::{Continuity, Domain, InDomain, Presence};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use core::ops::RangeInclusive;
#[cfg(feature = "font-types")]
use font_types::{GlyphId, GlyphId16, NameId, Tag};

fn within(values: RangeInclusive<u32>, value: u32) -> Result<Presence, Never> {
  Ok(Presence::from(values.contains(&value)))
}

impl Domain for u32 {
  fn to_u32(&self) -> Result<u32, Never> {
    Ok(*self)
  }

  fn from_u32(member: InDomain) -> Result<Self, Never> {
    Ok(member.0)
  }

  fn contains(_value: u32) -> Result<Presence, Never> {
    Ok(Presence::Present)
  }

  fn continuity() -> Result<Continuity, Never> {
    Ok(Continuity::Continuous)
  }

  fn ordered_values() -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    Ok(0..=u32::MAX)
  }

  fn ordered_values_range(
    range: RangeInclusive<Self>,
  ) -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    let Ok(first) = range.start().to_u32();
    let Ok(last) = range.end().to_u32();

    Ok(first..=last)
  }

  fn count() -> Result<u64, Never> {
    Ok(u64::from(u32::MAX).saturating_add(1))
  }
}

impl Domain for u16 {
  fn to_u32(&self) -> Result<u32, Never> {
    Ok(u32::from(*self))
  }

  fn from_u32(member: InDomain) -> Result<Self, Never> {
    fitted(member.0)
  }

  fn contains(value: u32) -> Result<Presence, Never> {
    within(0..=u32::from(u16::MAX), value)
  }

  fn continuity() -> Result<Continuity, Never> {
    Ok(Continuity::Continuous)
  }

  fn ordered_values() -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    Ok(0..=u32::from(u16::MAX))
  }

  fn ordered_values_range(
    range: RangeInclusive<Self>,
  ) -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    let Ok(first) = range.start().to_u32();
    let Ok(last) = range.end().to_u32();

    Ok(first..=last)
  }

  fn count() -> Result<u64, Never> {
    Ok(u64::from(u16::MAX).saturating_add(1))
  }
}

impl Domain for u8 {
  fn to_u32(&self) -> Result<u32, Never> {
    Ok(u32::from(*self))
  }

  fn from_u32(member: InDomain) -> Result<Self, Never> {
    fitted(member.0)
  }

  fn contains(value: u32) -> Result<Presence, Never> {
    within(0..=u32::from(u8::MAX), value)
  }

  fn continuity() -> Result<Continuity, Never> {
    Ok(Continuity::Continuous)
  }

  fn ordered_values() -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    Ok(0..=u32::from(u8::MAX))
  }

  fn ordered_values_range(
    range: RangeInclusive<Self>,
  ) -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    let Ok(first) = range.start().to_u32();
    let Ok(last) = range.end().to_u32();

    Ok(first..=last)
  }

  fn count() -> Result<u64, Never> {
    Ok(u64::from(u8::MAX).saturating_add(1))
  }
}

#[cfg(feature = "font-types")]
impl Domain for GlyphId16 {
  fn to_u32(&self) -> Result<u32, Never> {
    Ok(u32::from(*self))
  }

  fn from_u32(member: InDomain) -> Result<Self, Never> {
    let Ok(value) = fitted(member.0);

    Self::new(value)
  }

  fn contains(value: u32) -> Result<Presence, Never> {
    within(0..=u32::from(u16::MAX), value)
  }

  fn continuity() -> Result<Continuity, Never> {
    Ok(Continuity::Continuous)
  }

  fn ordered_values() -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    Ok(0..=u32::from(u16::MAX))
  }

  fn ordered_values_range(
    range: RangeInclusive<Self>,
  ) -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    let Ok(first) = range.start().to_u32();
    let Ok(last) = range.end().to_u32();

    Ok(first..=last)
  }

  fn count() -> Result<u64, Never> {
    Ok(u64::from(u16::MAX).saturating_add(1))
  }
}

#[cfg(feature = "font-types")]
impl Domain for GlyphId {
  fn to_u32(&self) -> Result<u32, Never> {
    Ok(u32::from(*self))
  }

  fn from_u32(member: InDomain) -> Result<Self, Never> {
    Ok(Self::from(member.0))
  }

  fn contains(_value: u32) -> Result<Presence, Never> {
    Ok(Presence::Present)
  }

  fn continuity() -> Result<Continuity, Never> {
    Ok(Continuity::Continuous)
  }

  fn ordered_values() -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    Ok(0..=u32::MAX)
  }

  fn ordered_values_range(
    range: RangeInclusive<Self>,
  ) -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    let Ok(first) = range.start().to_u32();
    let Ok(last) = range.end().to_u32();

    Ok(first..=last)
  }

  fn count() -> Result<u64, Never> {
    Ok(u64::from(u32::MAX).saturating_add(1))
  }
}

#[cfg(feature = "font-types")]
impl Domain for Tag {
  fn to_u32(&self) -> Result<u32, Never> {
    let Ok(bytes) = self.to_be_bytes();

    Ok(u32::from_be_bytes(bytes))
  }

  fn from_u32(member: InDomain) -> Result<Self, Never> {
    Self::from_u32(member.0)
  }

  fn contains(_value: u32) -> Result<Presence, Never> {
    Ok(Presence::Present)
  }

  fn continuity() -> Result<Continuity, Never> {
    Ok(Continuity::Continuous)
  }

  fn ordered_values() -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    Ok(0..=u32::MAX)
  }

  fn ordered_values_range(
    range: RangeInclusive<Self>,
  ) -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    let Ok(first) = range.start().to_u32();
    let Ok(last) = range.end().to_u32();

    Ok(first..=last)
  }

  fn count() -> Result<u64, Never> {
    Ok(u64::from(u32::MAX).saturating_add(1))
  }
}

#[cfg(feature = "font-types")]
impl Domain for NameId {
  fn to_u32(&self) -> Result<u32, Never> {
    let Ok(value) = self.to_u16();

    Ok(u32::from(value))
  }

  fn from_u32(member: InDomain) -> Result<Self, Never> {
    let Ok(value) = fitted(member.0);

    Self::new(value)
  }

  fn contains(value: u32) -> Result<Presence, Never> {
    within(0..=u32::from(u16::MAX), value)
  }

  fn continuity() -> Result<Continuity, Never> {
    Ok(Continuity::Continuous)
  }

  fn ordered_values() -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    Ok(0..=u32::from(u16::MAX))
  }

  fn ordered_values_range(
    range: RangeInclusive<Self>,
  ) -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    let Ok(first) = range.start().to_u32();
    let Ok(last) = range.end().to_u32();

    Ok(first..=last)
  }

  fn count() -> Result<u64, Never> {
    Ok(u64::from(u16::MAX).saturating_add(1))
  }
}
