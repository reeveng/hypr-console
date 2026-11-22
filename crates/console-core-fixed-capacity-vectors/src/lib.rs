//! Inline storage for a variable number of elements with a fixed upper bound.

#![no_std]
#![forbid(unsafe_code)]

use core::cmp::Ordering;
use core::fmt;
use core::iter::{FusedIterator, Take};
use core::ops::{Deref, DerefMut};
use core::{array, slice};

#[doc = "An inline vector whose first insertion initializes its backing array with clones."]
#[doc = "Only its occupied prefix is visible to callers."]
pub struct FixedVec<T, const N: usize> {
  elements: Option<[T; N]>,
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(explicit051_no_machine_width, reason = "the occupied prefix is a native array index bounded by the array's const capacity")
  )]
  len: usize,
}

impl<T, const N: usize> FixedVec<T, N> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the constructor preserves the direct vector value used by fixed-capacity geometry callers")
  )]
  pub const fn new() -> Self {
    Self {
      elements: None,
      len: 0,
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the vector API exposes immutable slices directly, matching the Deref interface")
  )]
  #[must_use]
  pub fn as_slice(&self) -> &[T] {
    let occupied = self.elements.as_ref().and_then(|elements| elements.get(..self.len));

    match occupied {
      Some(elements) => elements,
      None => &[],
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the vector API exposes mutable slices directly, matching the DerefMut interface")
  )]
  pub fn as_mut_slice(&mut self) -> &mut [T] {
    let occupied = self.elements.as_mut().and_then(|elements| elements.get_mut(..self.len));

    match occupied {
      Some(elements) => elements,
      None => &mut [],
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "clear preserves the in-place vector operation used by geometry callers")
  )]
  pub fn clear(&mut self) {
    self.len = 0;
    self.elements = None;
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the vector API reports exhaustion through Option, matching callers that consume the optional last value")
  )]
  pub fn pop(&mut self) -> Option<T>
  where
    T: Copy,
  {
    match self.len.checked_sub(1) {
      Some(last) => {
        let value = self.as_slice().get(last).copied()?;
        self.len = last;
        Some(value)
      }
      None => None,
    }
  }
}

impl<T: Clone, const N: usize> FixedVec<T, N> {
  #[doc = "Append a value, returning it if the vector is full."]
  pub fn try_push(&mut self, value: T) -> Result<(), T> {
    match self.len.cmp(&N) {
      Ordering::Less => {}
      Ordering::Equal | Ordering::Greater => return Err(value),
    }

    let elements = self
      .elements
      .get_or_insert_with(|| array::from_fn(|_| value.clone()));

    match elements.get_mut(self.len) {
      Some(slot) => *slot = value,
      None => return Err(value),
    }

    self.len = self.len.saturating_add(1);
    Ok(())
  }

  #[doc = "Append a value. Panics if the vector is full."]
  #[track_caller]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "push preserves the capacity contract used by Extend and existing geometry callers")
  )]
  pub fn push(&mut self, value: T) {
    assert!(
      matches!(self.try_push(value), Ok(())),
      "fixed-capacity vector is full"
    );
  }
}

impl<T, const N: usize> Default for FixedVec<T, N> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T, const N: usize> From<[T; N]> for FixedVec<T, N> {
  fn from(elements: [T; N]) -> Self {
    Self {
      elements: Some(elements),
      len: N,
    }
  }
}

impl<T: Clone, const N: usize> Clone for FixedVec<T, N> {
  fn clone(&self) -> Self {
    self.iter().cloned().collect()
  }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for FixedVec<T, N> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.as_slice().fmt(formatter)
  }
}

impl<T: PartialEq, const N: usize> PartialEq for FixedVec<T, N> {
  fn eq(&self, other: &Self) -> bool {
    self.as_slice() == other.as_slice()
  }
}

impl<T, const N: usize> Deref for FixedVec<T, N> {
  type Target = [T];

  fn deref(&self) -> &[T] {
    self.as_slice()
  }
}

impl<T, const N: usize> DerefMut for FixedVec<T, N> {
  fn deref_mut(&mut self) -> &mut [T] {
    self.as_mut_slice()
  }
}

impl<T: Clone, const N: usize> Extend<T> for FixedVec<T, N> {
  fn extend<I: IntoIterator<Item = T>>(&mut self, values: I) {
    for value in values {
      self.push(value);
    }
  }
}

impl<T: Clone, const N: usize> FromIterator<T> for FixedVec<T, N> {
  fn from_iter<I: IntoIterator<Item = T>>(values: I) -> Self {
    let mut collected = Self::new();
    collected.extend(values);
    collected
  }
}

impl<'a, T, const N: usize> IntoIterator for &'a FixedVec<T, N> {
  type Item = &'a T;
  type IntoIter = slice::Iter<'a, T>;

  fn into_iter(self) -> Self::IntoIter {
    self.iter()
  }
}

impl<'a, T, const N: usize> IntoIterator for &'a mut FixedVec<T, N> {
  type Item = &'a mut T;
  type IntoIter = slice::IterMut<'a, T>;

  fn into_iter(self) -> Self::IntoIter {
    self.iter_mut()
  }
}

impl<T, const N: usize> IntoIterator for FixedVec<T, N> {
  type Item = T;
  type IntoIter = IntoIter<T, N>;

  fn into_iter(self) -> Self::IntoIter {
    let inner = self
      .elements
      .map(|elements| elements.into_iter().take(self.len));
    IntoIter { inner }
  }
}

pub struct IntoIter<T, const N: usize> {
  inner: Option<Take<array::IntoIter<T, N>>>,
}

impl<T, const N: usize> Iterator for IntoIter<T, N> {
  type Item = T;

  fn next(&mut self) -> Option<T> {
    let inner = self.inner.as_mut()?;
    inner.next()
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(explicit051_no_machine_width, reason = "Iterator::size_hint requires native element counts in both result fields")
  )]
  fn size_hint(&self) -> (usize, Option<usize>) {
    self
      .inner
      .as_ref()
      .map_or((0, Some(0)), Iterator::size_hint)
  }
}

impl<T, const N: usize> DoubleEndedIterator for IntoIter<T, N> {
  fn next_back(&mut self) -> Option<T> {
    let inner = self.inner.as_mut()?;
    inner.next_back()
  }
}

impl<T, const N: usize> ExactSizeIterator for IntoIter<T, N> {}

impl<T, const N: usize> FusedIterator for IntoIter<T, N> {}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn capacity_and_order_survive_a_failed_push() {
    let mut values = FixedVec::from([1, 2]);
    assert_eq!(values.try_push(3), Err(3));
    assert_eq!(values.as_slice(), &[1, 2]);
    assert_eq!(values.pop(), Some(2));
    assert_eq!(values.try_push(4), Ok(()));
    assert_eq!(values.as_slice(), &[1, 4]);
    assert_eq!(FixedVec::<_, 0>::new().try_push(5), Err(5));
  }

  #[test]
  fn owned_iteration_yields_only_the_live_values_from_either_end() {
    let values: FixedVec<_, 10> = [1, 2, 3].into_iter().collect();
    assert_eq!(values.as_slice(), &[1, 2, 3]);
    let mut occupied = values.into_iter();
    assert_eq!(occupied.next(), Some(1));
    assert_eq!(occupied.next_back(), Some(3));
    assert_eq!(occupied.len(), 1);
    assert_eq!(occupied.next(), Some(2));
    assert_eq!(occupied.next_back(), None);
  }
}
