//! Operand stack for CFF/CFF2 parsing.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use super::{super::error::Error, blend::BlendState};
use crate::{Emptiness, ps::cs::CharstringKind};
use types::Fixed;

type StackSlots<T> = [T; 513];

#[repr(u8)]
#[derive(Clone, Copy)]
enum NumberKind {
  Integer,
  Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperandParity {
  Even,
  Odd,
}

pub struct Stack {
  values: StackSlots<i32>,
  kinds: StackSlots<NumberKind>,
  top: u64,
}

impl Stack {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Stack::new` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub fn new() -> Self {
    Self {
      values: [0; _],
      kinds: [NumberKind::Integer; _],
      top: 0,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Stack::is_empty` method returns `Emptiness` directly in the existing parser interface"
    )
  )]
  pub fn is_empty(&self) -> Emptiness {
    match self.top {
      0 => Emptiness::Empty,
      _ => Emptiness::NonEmpty,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Stack::len` method returns `u64` directly in the existing parser interface"
    )
  )]
  pub fn len(&self) -> u64 {
    self.top
  }

  pub fn verify_exact_len(&self, len: u64) -> Result<(), Error> {
    match ::core::primitive::bool::then_some(self.top != len, ()) {
      ::core::option::Option::Some(()) => Err(Error::StackUnderflow),
      ::core::option::Option::None => Ok(()),
    }
  }

  pub fn verify_at_least_len(&self, len: u64) -> Result<(), Error> {
    match ::core::primitive::bool::then_some(self.top < len, ()) {
      ::core::option::Option::Some(()) => Err(Error::StackUnderflow),
      ::core::option::Option::None => Ok(()),
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Stack::operand_parity` method returns `OperandParity` directly in the existing parser interface"
    )
  )]
  pub fn operand_parity(&self) -> OperandParity {
    match self.top & 1 {
      0 => OperandParity::Even,
      _ => OperandParity::Odd,
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Stack::clear` method is called through its existing unit-returning signature"
    )
  )]
  pub fn clear(&mut self) {
    self.top = 0;
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Stack::reverse` method is called through its existing unit-returning signature"
    )
  )]
  pub fn reverse(&mut self) {
    let Ok(used) = index(self.top);
    let operands = self.values.get_mut(..used).zip(self.kinds.get_mut(..used));

    match operands {
      Some((values, kinds)) => {
        values.reverse();
        kinds.reverse();
      }
      None => {}
    }
  }

  pub(crate) fn exch(&mut self) -> Result<(), Error> {
    let Ok(used) = console_core_number_conversion::index(self.top);
    let values = self.values.get_mut(..used).ok_or(Error::StackUnderflow)?;
    let kinds = self.kinds.get_mut(..used).ok_or(Error::StackUnderflow)?;

    match (values, kinds) {
      ([.., previous, last], [.., previous_kind, last_kind]) => {
        core::mem::swap(previous, last);
        core::mem::swap(previous_kind, last_kind);

        Ok(())
      }
      _ => Err(Error::StackUnderflow),
    }
  }

  pub fn push(&mut self, number: impl Into<Number>) -> Result<(), Error> {
    let Ok((value, kind)) =
      <_ as ::core::convert::Into<crate::ps::cff::stack::Number>>::into(number).into_storage();
    self.write_slot(self.top, value, kind)?;

    self.top = self.top.wrapping_add(1);

    Ok(())
  }

  pub fn set(&mut self, index: u64, number: impl Into<Number>) -> Result<(), Error> {
    match ::core::primitive::bool::then_some(index >= self.top, ()) {
      ::core::option::Option::Some(()) => {
        return Err(Error::StackOverflow);
      }
      ::core::option::Option::None => {}
    }

    let Ok((value, kind)) =
      <_ as ::core::convert::Into<crate::ps::cff::stack::Number>>::into(number).into_storage();

    self.write_slot(index, value, kind)
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Stack::drop` method is called through its existing unit-returning signature"
    )
  )]
  pub fn drop(&mut self, n: u64) {
    self.top = self.top.saturating_sub(n);
  }

  pub fn get_i32(&self, index: u64) -> Result<i32, Error> {
    let Ok(operand) = self.operand_at(index);

    match operand {
      Some(Number::Fixed(_fraction)) => Err(Error::ExpectedI32StackEntry(index)),
      Some(Number::I32(value)) => Ok(value),
      None => Ok(0),
    }
  }

  pub fn get_fixed(&self, index: u64) -> Result<Fixed, Error> {
    let Ok(operand) = self.operand_at(index);

    Ok(match operand {
      Some(number) => {
        let Ok(fixed) = number.into_fixed();

        fixed
      }
      None => Fixed::ZERO,
    })
  }

  pub fn pop_i32(&mut self) -> Result<i32, Error> {
    let i = self.pop()?;
    self.get_i32(i)
  }

  pub fn pop_fixed(&mut self) -> Result<Fixed, Error> {
    let i = self.pop()?;
    self.get_fixed(i)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Stack::fixed_values` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn fixed_values(&self) -> impl Iterator<Item = Fixed> + '_ {
    let numbers = self.number_values();

    numbers.map(|number| {
      let Ok(fixed) = number.into_fixed();

      fixed
    })
  }

  pub fn fixed_array<const N: usize>(&self, first_index: u64) -> Result<[Fixed; N], Error> {
    let mut result = [Fixed::ZERO; N];
    let Ok(start) = index(first_index.min(self.top));

    for (value, dest) in self.fixed_values().skip(start).zip(&mut result) {
      *dest = value;
    }

    Ok(result)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Stack::number_values` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn number_values(&self) -> impl Iterator<Item = Number> + '_ {
    let Ok(used) = index(self.top);

    self
      .values
      .iter()
      .zip(&self.kinds)
      .take(used)
      .map(|(value, kind)| {
        let Ok(number) = Number::from_stack(*value, *kind);

        number
      })
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Stack::apply_delta_prefix_sum` method is called through its existing unit-returning signature"
    )
  )]
  pub fn apply_delta_prefix_sum(&mut self) {
    match ::core::primitive::bool::then_some(self.top > 1, ()) {
      ::core::option::Option::Some(()) => {
        let mut sum = Fixed::ZERO;
        let Ok(used) = index(self.top);

        for (value, kind) in self.values.iter_mut().zip(&mut self.kinds).take(used) {
          let fixed_value = match kind {
            NumberKind::Fixed => {
              let Ok(value_21) = Fixed::from_bits(*value);

              value_21
            }
            NumberKind::Integer => {
              let Ok(value_22) = Fixed::from_i32(*value);

              value_22
            }
          };

          let value_8 = fixed_value;

          let Ok(value_15) = sum.wrapping_add(value_8);

          sum = value_15;
          let Ok(value_9) = sum.to_bits();

          *value = value_9;
          *kind = NumberKind::Fixed;
        }
      }
      ::core::option::Option::None => {}
    }
  }

  #[inline(never)]
  pub fn apply_blend(&mut self, blend_state: &BlendState) -> Result<(), Error> {
    let target_value_count = {
      let pop_i32 = self.pop_i32()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(pop_i32);
      migrated_value
    };

    match ::core::primitive::bool::then_some(target_value_count > self.top, ()) {
      ::core::option::Option::Some(()) => {
        return Err(Error::StackUnderflow);
      }
      ::core::option::Option::None => {}
    }

    let region_count = blend_state.region_count()?;

    let operand_count = ::core::primitive::u64::wrapping_mul(
      target_value_count,
      ::core::primitive::u64::wrapping_add(region_count, 1),
    );

    match ::core::primitive::bool::then_some(self.len() < operand_count, ()) {
      ::core::option::Option::Some(()) => {
        return Err(Error::StackUnderflow);
      }
      ::core::option::Option::None => {}
    }

    let start = ::core::primitive::u64::wrapping_sub(self.len(), operand_count);
    let end = ::core::primitive::u64::wrapping_add(start, operand_count);

    let Ok(first_operand) = index(start);
    let Ok(end_operand) = index(end);
    let values = self
      .values
      .get_mut(first_operand..end_operand)
      .ok_or(Error::StackUnderflow)?;
    let kinds = self
      .kinds
      .get_mut(first_operand..end_operand)
      .ok_or(Error::StackUnderflow)?;

    for (value, kind) in values.iter_mut().zip(kinds) {
      match kind {
        NumberKind::Integer => {
          let Ok(value_10) = Fixed::from_i32(*value);

          let Ok(value_16) = value_10.to_bits();

          *value = value_16;
          *kind = NumberKind::Fixed;
        }
        NumberKind::Fixed => {}
      }
    }

    let Ok(target_count) = index(target_value_count);

    let (values_2, deltas) = values
      .split_at_mut_checked(target_count)
      .ok_or(Error::StackUnderflow)?;

    let scalars = blend_state.scalars()?;

    for (region_ix, maybe_scalar) in scalars.enumerate() {
      let scalar = maybe_scalar?;

      match ::core::primitive::bool::then_some(scalar == Fixed::ZERO, ()) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      for (value_ix, value) in values_2.iter_mut().enumerate() {
        let Ok(operand_index) = fitted::<_, u64>(value_ix);
        let Ok(region_index) = fitted::<_, u64>(region_ix);

        let delta_offset = region_count
          .wrapping_mul(operand_index)
          .wrapping_add(region_index);
        let Ok(delta_index) = index(delta_offset);
        let delta_bits = deltas
          .get(delta_index)
          .copied()
          .ok_or(Error::StackUnderflow)?;

        let Ok(delta) = Fixed::from_bits(delta_bits);
        let Ok(value_11) = Fixed::from_bits(*value);

        let Ok(value_17) = value_11.wrapping_add(delta * scalar);

        let Ok(value_6) = value_17.to_bits();

        *value = value_6;
      }
    }

    self.top = ::core::primitive::u64::wrapping_add(start, target_value_count);
    Ok(())
  }

  pub(crate) fn div(&mut self, charstring: CharstringKind) -> Result<(), Error> {
    let b_idx = self.pop()?;
    let a_idx = self.pop()?;
    let Ok(index) = index(a_idx);
    let kind = self
      .kinds
      .get(index)
      .copied()
      .ok_or(Error::StackUnderflow)?;

    let (a, b) = match kind {
      NumberKind::Fixed => {
        let fixed = self.get_fixed(a_idx)?;
        let own_fixed = self.get_fixed(b_idx)?;

        (fixed, own_fixed)
      }
      NumberKind::Integer => {
        let a = self.get_i32(a_idx)?;

        match (charstring, a) {
          (CharstringKind::Type1, ..-32000 | 32001..) => {
            let Ok(value_23) = Fixed::from_bits(a);
            let checked_value = self.get_i32(b_idx)?;
            let Ok(value_24) = Fixed::from_bits(checked_value);

            (value_23, value_24)
          }
          (_, _) => {
            let fixed = self.get_fixed(a_idx)?;
            let own_fixed = self.get_fixed(b_idx)?;

            (fixed, own_fixed)
          }
        }
      }
    };
    self.push(a / b)
  }

  fn write_slot(&mut self, position: u64, value: i32, kind: NumberKind) -> Result<(), Error> {
    let Ok(index) = index(position);
    let stored_value = self.values.get_mut(index).ok_or(Error::StackOverflow)?;
    let stored_kind = self.kinds.get_mut(index).ok_or(Error::StackOverflow)?;
    *stored_value = value;
    *stored_kind = kind;

    Ok(())
  }

  fn operand_at(&self, position: u64) -> Result<Option<Number>, Never> {
    let Ok(index) = index(position);
    let entry = self.values.get(index).zip(self.kinds.get(index));

    Ok(entry.map(|(&value, &kind)| {
      let Ok(operand) = Number::from_stack(value, kind);

      operand
    }))
  }

  fn pop(&mut self) -> Result<u64, Error> {
    match ::core::primitive::bool::then_some(self.top > 0, ()) {
      ::core::option::Option::Some(()) => {
        match (1, &mut self.top) {
          (explicit_arithmetic_amount, explicit_arithmetic_place) => {
            *explicit_arithmetic_place = ::core::primitive::u64::wrapping_sub(
              *explicit_arithmetic_place,
              explicit_arithmetic_amount,
            )
          }
        };

        Ok(self.top)
      }
      ::core::option::Option::None => Ok(0),
    }
  }
}

impl Default for Stack {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Number {
  I32(i32),
  Fixed(Fixed),
}

impl Number {
  fn into_storage(self) -> Result<(i32, NumberKind), Never> {
    Ok(match self {
      Self::I32(value) => (value, NumberKind::Integer),
      Self::Fixed(value) => {
        let Ok(bits) = value.to_bits();

        (bits, NumberKind::Fixed)
      }
    })
  }

  fn into_fixed(self) -> Result<Fixed, Never> {
    match self {
      Self::I32(value) => Fixed::from_i32(value),
      Self::Fixed(value) => Ok(value),
    }
  }

  #[must_use]
  fn from_stack(
    raw: i32,
    kind: NumberKind,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(match kind {
      NumberKind::Fixed => {
        let Ok(value_12) = Fixed::from_bits(raw);

        Self::Fixed(value_12)
      }
      NumberKind::Integer => Self::I32(raw),
    })
  }
}

impl From<i32> for Number {
  fn from(value: i32) -> Self {
    Self::I32(value)
  }
}

impl From<Fixed> for Number {
  fn from(value: Fixed) -> Self {
    Self::Fixed(value)
  }
}

impl std::fmt::Display for Number {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::I32(value) => value.fmt(f),
      Self::Fixed(value) => value.fmt(f),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{FontData, FontRead, tables::variations::ItemVariationStore};
  use types::{F2Dot14, Fixed};

  #[test]
  fn push_pop() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut stack = Stack::new();
    stack.push(20)?;
    stack.push({
      let Ok(value) = Fixed::from_f64(42.42);
      value
    })?;
    assert_eq!(stack.operand_parity(), OperandParity::Even);
    stack.verify_exact_len(2)?;
    stack.verify_at_least_len(2)?;
    assert_eq!(
      match stack.pop_fixed() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      ({
        let Ok(explicit_value) = Fixed::from_f64(42.42);
        explicit_value
      })
    );
    let asserted_value = stack.pop_i32()?;
    assert_eq!(asserted_value, 20);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn push_fixed_pop_i32() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut stack = Stack::new();
    stack.push({
      let Ok(value) = Fixed::from_f64(42.42);
      value
    })?;
    assert!(matches!(
      stack.pop_i32(),
      Err(Error::ExpectedI32StackEntry(_))
    ));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn push_i32_pop_fixed() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut stack = Stack::new();
    stack.push(123)?;
    assert_eq!(
      match stack.pop_fixed() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      ({
        let Ok(explicit_value) = Fixed::from_f64(123.0);
        explicit_value
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn reverse() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut stack = Stack::new();
    stack.push({
      let Ok(value) = Fixed::from_f64(1.5);
      value
    })?;
    stack.push(42)?;
    stack.push({
      let Ok(value) = Fixed::from_f64(4.2);
      value
    })?;
    stack.reverse();
    assert_eq!(
      match stack.pop_fixed() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      ({
        let Ok(explicit_value) = Fixed::from_f64(1.5);
        explicit_value
      })
    );
    let asserted_value = stack.pop_i32()?;
    assert_eq!(asserted_value, 42);
    assert_eq!(
      match stack.pop_fixed() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      ({
        let Ok(explicit_value) = Fixed::from_f64(4.2);
        explicit_value
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn delta_prefix_sum() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut stack = Stack::new();
    stack.push({
      let Ok(value) = Fixed::from_f64(1.5);
      value
    })?;
    stack.push(42)?;
    stack.push({
      let Ok(value) = Fixed::from_f64(4.2);
      value
    })?;
    stack.apply_delta_prefix_sum();
    assert_eq!(stack.operand_parity(), OperandParity::Odd);
    let values: Vec<_> = stack.fixed_values().collect();
    let expected = &[
      {
        let Ok(value) = Fixed::from_f64(1.5);
        value
      },
      {
        let Ok(value) = Fixed::from_f64(43.5);
        value
      },
      {
        let Ok(value) = Fixed::from_f64(47.69999694824219);
        value
      },
    ];
    assert_eq!(&values, expected);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn blend() -> ::core::result::Result<(), crate::test_support::Failure> {
    let ivs_data = &*match <[_]>::get(&*font_test_data::cff2::EXAMPLE, 18..) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::test_support::Failure::from(
          crate::read::ReadError::OutOfBounds,
        ));
      }
    };
    let ivs = ItemVariationStore::read({
      let Ok(explicit_value) = FontData::new(ivs_data);
      explicit_value
    })?;

    let coords = &[{
      let Ok(value) = F2Dot14::from_f32(-0.75);
      value
    }];
    let blend_state =
      crate::test_support::fixture_value(BlendState::new(ivs, coords, 0), "fixture value")?;
    let mut stack = Stack::new();

    crate::test_support::fixture_value(stack.push(10), "fixture value")?;
    crate::test_support::fixture_value(stack.push(20), "fixture value")?;

    crate::test_support::fixture_value(stack.push(4), "fixture value")?;
    crate::test_support::fixture_value(stack.push(-8), "fixture value")?;

    crate::test_support::fixture_value(stack.push(-60), "fixture value")?;
    crate::test_support::fixture_value(stack.push(2), "fixture value")?;

    crate::test_support::fixture_value(stack.push(2), "fixture value")?;
    crate::test_support::fixture_value(stack.apply_blend(&blend_state), "fixture value")?;
    let result: Vec<_> = stack.fixed_values().collect();

    let expected = &[
      {
        let Ok(value) = Fixed::from_f64(8.0);
        value
      },
      {
        let Ok(value) = Fixed::from_f64(-9.0);
        value
      },
    ];
    assert_eq!(&result, expected);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn invalid_access_yields_zero() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut stack = Stack::new();
    let asserted_value = stack.pop_i32()?;
    assert_eq!(asserted_value, 0);
    assert_eq!(
      match stack.pop_fixed() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      Fixed::ZERO
    );
    let asserted_value_2 = stack.get_i32(10)?;
    assert_eq!(asserted_value_2, 0);
    assert_eq!(
      match stack.get_fixed(10) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      Fixed::ZERO
    );

    stack.push(5)?;
    assert_eq!(
      match stack.fixed_array::<3>(0) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      [
        {
          let Ok(value) = Fixed::from_i32(5);
          value
        },
        Fixed::ZERO,
        Fixed::ZERO
      ]
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn exch() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut stack = Stack::new();
    stack.push(4)?;
    stack.push({
      let Ok(value) = Fixed::from_f64(2.5);
      value
    })?;
    stack.exch()?;
    let asserted_value = stack.pop_i32()?;
    assert_eq!(asserted_value, 4);
    assert_eq!(
      match stack.pop_fixed() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      ({
        let Ok(explicit_value) = Fixed::from_f64(2.5);
        explicit_value
      })
    );

    stack.clear();
    stack.push(1)?;
    assert!(matches!(stack.exch(), Err(Error::StackUnderflow)));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn div() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut stack = Stack::new();

    stack.push(200)?;
    stack.push(50)?;
    stack.div(CharstringKind::Type2)?;
    assert_eq!(
      match stack.pop_fixed() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      ({
        let Ok(explicit_value) = Fixed::from_i32(4);
        explicit_value
      })
    );
    stack.push({
      let Ok(value) = Fixed::from_f64(151.0);
      value
    })?;
    stack.push(2)?;
    stack.div(CharstringKind::Type2)?;
    assert_eq!(
      match stack.pop_fixed() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      ({
        let Ok(explicit_value) = Fixed::from_f64(75.5);
        explicit_value
      })
    );

    stack.push(40000)?;
    stack.push(20)?;
    stack.div(CharstringKind::Type1)?;
    assert_eq!(
      match stack.pop_fixed() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      ({
        let Ok(explicit_value) = Fixed::from_i32(2000);
        explicit_value
      })
    );
    stack.push(-40000)?;
    stack.push(20)?;
    stack.div(CharstringKind::Type1)?;
    assert_eq!(
      match stack.pop_fixed() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      ({
        let Ok(explicit_value) = Fixed::from_i32(-2000);
        explicit_value
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn set() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut stack = Stack::new();
    stack.push(0)?;
    stack.push(0)?;
    stack.push(0)?;
    stack.set(0, {
      let Ok(value) = Fixed::from_f64(-4.2);
      value
    })?;
    stack.set(2, 25)?;
    assert_eq!(
      match stack.fixed_array(0) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      [
        {
          let Ok(value) = Fixed::from_f64(-4.2);
          value
        },
        Fixed::ZERO,
        {
          let Ok(value) = Fixed::from_f64(25.0);
          value
        }
      ]
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn drop() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut stack = Stack::new();

    for _ in 0..20 {
      match stack.push(0) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(read_error);
        }
      };
    }

    assert_eq!(stack.len(), 20);
    stack.drop(4);
    assert_eq!(stack.len(), 16);
    stack.drop(10);
    assert_eq!(stack.len(), 6);

    stack.drop(7);
    assert_eq!(stack.len(), 0);
    ::core::result::Result::Ok(())
  }
}
