//! Content stream operators.

use crate::content::{Instruction, Operator, OperatorTrait, Stack};
use crate::object;
use crate::object::Array;
use crate::object::Name;
use crate::object::Number;
use crate::object::Object;
use crate::object::Stream;
use alloc::vec;
use alloc::vec::Vec;

use crate::content::macros::{op_all, op_fixed, op_impl, op0};

include!("operators.rs");

type NamedColor<'b, 'a> = (Vec<Number>, Option<&'b Name<'a>>);

fn parse_named_color<'b, 'a>(
  objects: &'b [Object<'a>],
) -> core::result::Result<Option<NamedColor<'b, 'a>>, console_core_never::Never> {
  let mut nums = vec![];
  let mut name = None;

  for o in objects {
    match o {
      Object::Number(n) => nums.push(*n),
      Object::Name(n) => name = Some(n),
      Object::Null(_)
      | Object::Boolean(_)
      | Object::String(_)
      | Object::Dict(_)
      | Object::Array(_)
      | Object::Stream(_) => {
        warn!("encountered unknown object {o:?} when parsing scn/SCN");

        return Ok(None);
      }
    }
  }

  Ok(Some((nums, name)))
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "Each field is an independent PDF operand and TryFrom only checks the instruction variant"
  )
)]
pub struct StrokeColorNamed<'b, 'a>(pub Vec<Number>, pub Option<&'b Name<'a>>);

op_impl!(StrokeColorNamed<'b, 'a>, "SCN", |stack: &'b Stack<'a>| {
  let Ok(operands) = stack.as_slice();
  let Ok(decoded) = parse_named_color(operands);
  let (nums, name) = decoded?;
  Some(StrokeColorNamed(nums, name))
});

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "Each field is an independent PDF operand and TryFrom only checks the instruction variant"
  )
)]
pub struct NonStrokeColorNamed<'b, 'a>(pub Vec<Number>, pub Option<&'b Name<'a>>);

op_impl!(NonStrokeColorNamed<'b, 'a>, "scn", |stack: &'b Stack<
  'a,
>| {
  let Ok(operands) = stack.as_slice();
  let Ok(decoded) = parse_named_color(operands);
  let (nums, name) = decoded?;
  Some(NonStrokeColorNamed(nums, name))
});

#[cfg(test)]
mod tests {
  use crate::content::TypedIter;
  use crate::content::ops::{
    BeginMarkedContentWithProperties, ClosePath, EndMarkedContent, FillPathNonZero, LineTo,
    MarkedContentPointWithProperties, MoveTo, NonStrokeColorDeviceRgb, NonStrokeColorNamed,
    SetGraphicsState, StrokeColorNamed, Transform, TypedInstruction,
  };
  use crate::object::Name;
  use crate::object::Number;
  use crate::object::Object;
  use crate::object::{Dict, FromBytes};
  use crate::test_support::{Required, TestFailure};

  fn n(num: i32) -> Result<Number, console_core_never::Never> {
    Number::from_i32(num)
  }

  fn name(bytes: &[u8]) -> Result<Name<'_>, console_core_never::Never> {
    Name::new_unescaped(bytes)
  }

  #[test]
  fn basic_ops_1() {
    let input = b"
1 0 0 -1 0 200 cm
/g0 gs
1 0 0 rg
";

    let Ok(mut iter) = TypedIter::new(input);
    let (Ok(zero), Ok(one), Ok(minus_one), Ok(two_hundred)) = (n(0), n(1), n(-1), n(200));
    let Ok(g0) = name(b"g0");

    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::Transform(Transform(
        one,
        zero,
        zero,
        minus_one,
        zero,
        two_hundred
      ))))
    );
    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::SetGraphicsState(SetGraphicsState(
        &g0
      ))))
    );
    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::NonStrokeColorDeviceRgb(
        NonStrokeColorDeviceRgb(one, zero, zero)
      )))
    );
    assert_eq!(iter.next(), Ok(None));
  }

  #[test]
  fn basic_ops_2() {
    let input = b"
20 20 m
180 20 l
180 180 l
20 180 l
h
f
";

    let Ok(mut iter) = TypedIter::new(input);
    let (Ok(twenty), Ok(one_eighty)) = (n(20), n(180));

    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::MoveTo(MoveTo(twenty, twenty))))
    );
    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::LineTo(LineTo(one_eighty, twenty))))
    );
    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::LineTo(LineTo(
        one_eighty, one_eighty
      ))))
    );
    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::LineTo(LineTo(twenty, one_eighty))))
    );
    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::ClosePath(ClosePath)))
    );
    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::FillPathNonZero(FillPathNonZero)))
    );
    assert_eq!(iter.next(), Ok(None));
  }

  #[test]
  fn scn() {
    let input = b"
0.0 scn
1.0 1.0 1.0 /DeviceRgb SCN
";

    let Ok(mut iter) = TypedIter::new(input);
    let (Ok(zero), Ok(one)) = (Number::from_f32(0.0), Number::from_f32(1.0));
    let Ok(device_rgb) = name(b"DeviceRgb");

    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::NonStrokeColorNamed(
        NonStrokeColorNamed(alloc::vec![zero], None)
      )))
    );
    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::StrokeColorNamed(StrokeColorNamed(
        alloc::vec![one, one, one],
        Some(&device_rgb)
      ))))
    );
    assert_eq!(iter.next(), Ok(None));
  }

  #[test]
  fn dp() -> Result<(), TestFailure> {
    let input = b"/Attribute<</ShowCenterPoint false >> DP";

    let Ok(mut iter) = TypedIter::new(input);
    let Ok(attribute) = name(b"Attribute");
    let dict = Dict::from_bytes(b"<</ShowCenterPoint false >>").required()?;
    let properties = Object::Dict(dict);

    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::MarkedContentPointWithProperties(
        MarkedContentPointWithProperties(&attribute, &properties)
      )))
    );
    assert_eq!(iter.next(), Ok(None));

    Ok(())
  }

  #[test]
  fn bdc_with_dict() -> Result<(), TestFailure> {
    let input = b"/Span << /MCID 0 /Alt (Alt)>> BDC EMC";

    let Ok(mut iter) = TypedIter::new(input);
    let Ok(span) = name(b"Span");
    let dict = Dict::from_bytes(b"<< /MCID 0 /Alt (Alt)>>").required()?;
    let properties = Object::Dict(dict);

    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::BeginMarkedContentWithProperties(
        BeginMarkedContentWithProperties(&span, &properties)
      )))
    );
    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::EndMarkedContent(EndMarkedContent)))
    );
    assert_eq!(iter.next(), Ok(None));

    Ok(())
  }

  #[test]
  fn bdc_with_name() {
    let input = b"/Span /Name BDC EMC";

    let Ok(mut iter) = TypedIter::new(input);
    let (Ok(span), Ok(named)) = (name(b"Span"), name(b"Name"));
    let properties = Object::Name(named);

    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::BeginMarkedContentWithProperties(
        BeginMarkedContentWithProperties(&span, &properties)
      )))
    );
    assert_eq!(
      iter.next(),
      Ok(Some(TypedInstruction::EndMarkedContent(EndMarkedContent)))
    );
    assert_eq!(iter.next(), Ok(None));
  }
}
