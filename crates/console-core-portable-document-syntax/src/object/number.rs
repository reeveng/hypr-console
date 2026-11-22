//! Numbers.

use crate::ObjectReadMode;
use crate::math::{powi_f64, trunc_f64};
use crate::object::macros::object;
use crate::object::{Object, ObjectLike};
use crate::reader::Reader;
use crate::reader::{Readable, ReaderContext, ReaderExt, Skippable};
use crate::trivia::{is_regular_character, is_white_space_character};
use console_core_never::Never;
use console_core_number_conversion::{Float, index, nearest_f32_from_f64};
use core::fmt::Debug;

#[rustfmt::skip]
static POWERS_OF_10: [f64; 20] = [
    1.0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9,
    1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18, 1e19,
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Number(pub(crate) InternalNumber);

impl Number {
  pub const ZERO: Self = ({
    let Ok(explicit_value) = Self::from_i32(0);
    explicit_value
  });

  pub const ONE: Self = ({
    let Ok(explicit_value) = Self::from_i32(1);
    explicit_value
  });

  pub fn as_f64(&self) -> core::result::Result<f64, console_core_never::Never> {
    Ok(match self.0 {
      InternalNumber::Real(r) => r,
      InternalNumber::Integer(i) => {
        let Ok(value) = i.float();
        value
      }
    })
  }

  pub fn as_f32(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(match self.0 {
      InternalNumber::Real(r) => {
        match ::console_core_number_conversion::nearest_f32_from_f64(r) {
          ::core::result::Result::Ok(_converted_number) => _converted_number,
        }
      }
      InternalNumber::Integer(i) => {
        match ::console_core_number_conversion::nearest_f32_from_i64(i) {
          ::core::result::Result::Ok(_converted_number) => _converted_number,
        }
      }
    })
  }

  pub fn as_i64(&self) -> core::result::Result<i64, console_core_never::Never> {
    Ok(match self.0 {
      InternalNumber::Real(r) => {
        let Ok(res) = console_core_number_conversion::toward_zero_i64(r);

        let Ok(truncated) = trunc_f64(r);

        match ::core::primitive::bool::then_some(
          truncated.partial_cmp(&r) != Some(core::cmp::Ordering::Equal),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            debug!("float {r} was truncated to {res}");
          }
          ::core::option::Option::None => {}
        }

        res
      }
      InternalNumber::Integer(i) => i,
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "General PDF numeric objects have no inherent unit; the containing dictionary or operator supplies it"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit011_no_as_cast",
    expect(
      explicit011_no_as_cast,
      reason = "The public const constructor widens the f32 exactly to the PDF number's f64 representation"
    )
  )]
  pub const fn from_f32(num: f32) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self(InternalNumber::Real(num as f64)))
  }

  pub const fn from_i32(num: i32) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self(InternalNumber::Integer(
      ::console_core_number_conversion::const_wrapped_integer!(signed ::core::primitive::i32 => ::core::primitive::i64, num),
    )))
  }
}

impl Skippable for Number {
  fn skip(r: &mut Reader<'_>, _: ObjectReadMode) -> Option<()> {
    let Ok(forward_if) = r.forward_if(|b| b == b'+' || b == b'-');

    let Ok(peek_byte) = r.peek_byte();

    let checked_value = peek_byte?;

    match checked_value {
      b'.' => {
        let Ok(byte) = r.read_byte();
        byte?;

        let Ok(()) = r.forward_while(|argument_0| {
          matches!(
            {
              let Ok(explicit_value) = is_digit_or_minus(argument_0);
              explicit_value
            },
            crate::IsDigitOrMinusAnswer::Yes
          )
        });
      }
      b'0'..=b'9' | b'-' => {
        let Ok(forward_while_1) = r.forward_while_1(|argument_0| {
          matches!(
            {
              let Ok(explicit_value) = is_digit_or_minus(argument_0);
              explicit_value
            },
            crate::IsDigitOrMinusAnswer::Yes
          )
        });
        forward_while_1?;
        let Ok(explicit_value) = r.forward_tag(b".");

        match explicit_value {
          Some(()) => {
            let Ok(()) = r.forward_while(|argument_0| {
              matches!(
                {
                  let Ok(explicit_value_2) = is_digit_or_minus(argument_0);
                  explicit_value_2
                },
                crate::IsDigitOrMinusAnswer::Yes
              )
            });
          }
          None => {}
        }
      }

      b => {
        let Ok(whitespace) = is_white_space_character(b);

        match (forward_if, whitespace, b) {
          (Some(_), crate::IsWhiteSpaceCharacterAnswer::Yes, _) | (Some(_), _, b'(' | b'<') => {}
          _ => return None,
        }
      }
    }

    let Ok(r_peek_byte) = r.peek_byte();

    match ::core::primitive::bool::then_some(
      r_peek_byte.is_some_and(|argument_0| {
        matches!(
          {
            let Ok(explicit_value) = is_regular_character(argument_0);
            explicit_value
          },
          crate::IsRegularCharacterAnswer::Yes
        )
      }),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return None;
      }
      ::core::option::Option::None => {}
    }

    Some(())
  }
}

impl Readable<'_> for Number {
  #[inline]
  fn read(r: &mut Reader<'_>, _: &ReaderContext<'_>) -> Option<Self> {
    let Ok(start) = r.fixed_offset();
    let Ok(number) = read_inner(r);

    match number {
      Some(value) => Some(value),
      None => {
        let Ok(native_offset) = index(start);
        let Ok(()) = r.jump(native_offset);
        None
      }
    }
  }
}

enum Sign {
  Negative,
  Nonnegative,
}

enum DecimalPoint {
  Absent,
  Present,
}

enum Digits {
  Absent,
  Present,
}

#[inline(always)]
fn read_inner(
  r: &mut Reader<'_>,
) -> core::result::Result<Option<Number>, console_core_never::Never> {
  let Ok(peek_byte) = r.peek_byte();
  let sign = match match peek_byte {
    Some(value) => value,
    None => return Ok(None),
  } {
    b'-' => {
      let Ok(()) = r.forward();
      Sign::Negative
    }
    b'+' => {
      let Ok(()) = r.forward();
      Sign::Nonnegative
    }
    _ => Sign::Nonnegative,
  };

  let mut mantissa: u64 = 0;
  let mut decimal_point = DecimalPoint::Absent;
  let mut decimal_shift: u32 = 0;
  let mut digits = Digits::Absent;

  let Ok(tail) = r.tail();
  let input = match tail {
    Some(bytes) => bytes,
    None => return Ok(None),
  };

  for _ in input {
    let Ok(next) = r.peek_byte();
    let byte = match next {
      Some(byte) => byte,
      None => break,
    };

    match (byte, &decimal_point, &digits) {
      (b'0'..=b'9', _, _) => {
        let Ok(()) = r.forward();
        mantissa = mantissa.wrapping_mul(10).wrapping_add({
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, u64>(byte.wrapping_sub(b'0'));
          numeric_value
        });
        digits = Digits::Present;

        let place = match decimal_point {
          DecimalPoint::Present => 1,
          DecimalPoint::Absent => 0,
        };
        decimal_shift = decimal_shift.wrapping_add(place);
      }
      (b'.', DecimalPoint::Absent, _) => {
        let Ok(()) = r.forward();
        decimal_point = DecimalPoint::Present;
      }

      (b'-', _, Digits::Present) => {
        let Ok(()) = r.forward();
        let Ok(()) = r.forward_while(|argument_0| {
          matches!(
            {
              let Ok(explicit_value_2) = is_digit_or_minus(argument_0);
              explicit_value_2
            },
            crate::IsDigitOrMinusAnswer::Yes
          )
        });
        break;
      }
      _ => break,
    }
  }

  match (&digits, &sign, &decimal_point) {
    (Digits::Absent, Sign::Negative, _) | (Digits::Absent, _, DecimalPoint::Present) => {
      return Ok(Some(Number(InternalNumber::Integer(0))));
    }
    (Digits::Absent, Sign::Nonnegative, DecimalPoint::Absent) => return Ok(None),
    (Digits::Present, _, _) => {}
  }

  let Ok(r_peek_byte) = r.peek_byte();

  match ::core::primitive::bool::then_some(
    r_peek_byte.is_some_and(|argument_0| {
      matches!(
        {
          let Ok(explicit_value) = is_regular_character(argument_0);
          explicit_value
        },
        crate::IsRegularCharacterAnswer::Yes
      )
    }),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  Ok(match decimal_point {
    DecimalPoint::Absent => {
      let value = match sign {
        Sign::Negative => ({
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i64>(mantissa);
          numeric_value
        })
        .wrapping_neg(),
        Sign::Nonnegative => {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i64>(mantissa);
          numeric_value
        }
      };
      Some(Number(InternalNumber::Integer(value)))
    }
    DecimalPoint::Present => {
      let Ok(value) = real_value(mantissa, decimal_shift);
      let mut value_2 = value;

      match sign {
        Sign::Negative => value_2 = -value_2,
        Sign::Nonnegative => {}
      }

      Some(Number(InternalNumber::Real(value_2)))
    }
  })
}

fn real_value(mantissa: u64, decimal_shift: u32) -> Result<f64, Never> {
  let Ok(value) = mantissa.float();

  match decimal_shift {
    0 => {
      return Ok(value);
    }
    _ => {}
  }

  let Ok(power_index) = index(decimal_shift);
  let power = match POWERS_OF_10.get(power_index) {
    Some(stored) => *stored,
    None => {
      let Ok(computed) = powi_f64(10.0, decimal_shift);
      computed
    }
  };
  Ok(value / power)
}

object!(Number, Number);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum InternalNumber {
  Real(f64),
  Integer(i64),
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "Each invocation names the integer used to parse, skip, and convert a PDF Number"
  )
)]
macro_rules! int_num {
  ($i:ident) => {
    impl Skippable for $i {
      fn skip(r: &mut Reader<'_>, _: ObjectReadMode) -> Option<()> {
        let Ok(_optional_sign) = r.forward_if(|b| b == b'+' || b == b'-');
        ({
          let Ok(value) = r.forward_while_1(|byte| {
            let Ok(answer) = is_digit(byte);
            matches!(answer, crate::IsDigitAnswer::Yes)
          });
          value
        })?;

        if ({
          let Ok(value) = r.peek_byte();
          value
        }) == Some(b'.')
        {
          return None;
        }

        if ({
          let Ok(value) = r.peek_byte();
          value
        })
        .is_some_and(|byte| {
          let Ok(answer) = is_regular_character(byte);
          matches!(answer, crate::IsRegularCharacterAnswer::Yes)
        }) {
          return None;
        }

        Some(())
      }
    }

    impl<'a> Readable<'a> for $i {
      fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<$i> {
        r.read::<Number>(ctx)
          .map(|n| {
            ({
              let Ok(value) = n.as_i64();
              value
            })
          })
          .and_then(|number| match number.try_into() {
            Ok(value) => Some(value),
            Err(_integer_out_of_range) => None,
          })
      }
    }

    impl TryFrom<Object<'_>> for $i {
      type Error = ();

      fn try_from(value: Object<'_>) -> core::result::Result<Self, Self::Error> {
        match value {
          Object::Number(number) => {
            let Ok(integer) = number.as_i64();
            match integer.try_into() {
              Ok(value) => Ok(value),
              Err(_integer_out_of_range) => Err(()),
            }
          }
          Object::Null(_)
          | Object::Boolean(_)
          | Object::String(_)
          | Object::Name(_)
          | Object::Array(_)
          | Object::Dict(_)
          | Object::Stream(_) => Err(()),
        }
      }
    }

    impl<'a> ObjectLike<'a> for $i {}
  };
}

int_num!(i32);
int_num!(i64);
int_num!(u32);
int_num!(u16);
int_num!(usize);
int_num!(u64);
int_num!(u8);

impl Skippable for f32 {
  fn skip(r: &mut Reader<'_>, mode: ObjectReadMode) -> Option<()> {
    r.skip::<Number>(mode).map(|_| {})
  }
}

impl Readable<'_> for f32 {
  fn read(r: &mut Reader<'_>, _: &ReaderContext<'_>) -> Option<Self> {
    r.read_without_context::<Number>().map(|number| {
      let Ok(value) = number.as_f64();
      let Ok(real) = nearest_f32_from_f64(value);
      real
    })
  }
}

impl TryFrom<Object<'_>> for f32 {
  type Error = ();

  fn try_from(value: Object<'_>) -> Result<Self, Self::Error> {
    match value {
      Object::Number(number) => {
        let Ok(value_2) = number.as_f64();
        let Ok(real) = nearest_f32_from_f64(value_2);
        Ok(real)
      }
      Object::Null(_)
      | Object::Boolean(_)
      | Object::String(_)
      | Object::Name(_)
      | Object::Dict(_)
      | Object::Array(_)
      | Object::Stream(_) => Err(()),
    }
  }
}

impl ObjectLike<'_> for f32 {}

impl Skippable for f64 {
  fn skip(r: &mut Reader<'_>, mode: ObjectReadMode) -> Option<()> {
    r.skip::<Number>(mode).map(|_| {})
  }
}

impl Readable<'_> for f64 {
  fn read(r: &mut Reader<'_>, _: &ReaderContext<'_>) -> Option<Self> {
    r.read_without_context::<Number>().map(|n| {
      let Ok(explicit_value) = n.as_f64();
      explicit_value
    })
  }
}

impl TryFrom<Object<'_>> for f64 {
  type Error = ();

  fn try_from(value: Object<'_>) -> Result<Self, Self::Error> {
    match value {
      Object::Number(n) => Ok({
        let Ok(explicit_value) = n.as_f64();
        explicit_value
      }),
      Object::Null(_)
      | Object::Boolean(_)
      | Object::String(_)
      | Object::Name(_)
      | Object::Dict(_)
      | Object::Array(_)
      | Object::Stream(_) => Err(()),
    }
  }
}

impl ObjectLike<'_> for f64 {}

pub(crate) fn is_digit(
  byte: u8,
) -> core::result::Result<crate::IsDigitAnswer, console_core_never::Never> {
  Ok(
    match ::core::primitive::bool::then_some(byte.is_ascii_digit(), ()) {
      ::core::option::Option::Some(()) => crate::IsDigitAnswer::Yes,
      ::core::option::Option::None => crate::IsDigitAnswer::No,
    },
  )
}

pub(crate) fn is_digit_or_minus(
  byte: u8,
) -> core::result::Result<crate::IsDigitOrMinusAnswer, console_core_never::Never> {
  Ok(
    match ::core::primitive::bool::then_some(
      matches!(
        {
          let Ok(explicit_value) = is_digit(byte);
          explicit_value
        },
        crate::IsDigitAnswer::Yes
      ) || byte == b'-',
      (),
    ) {
      ::core::option::Option::Some(()) => crate::IsDigitOrMinusAnswer::Yes,
      ::core::option::Option::None => crate::IsDigitOrMinusAnswer::No,
    },
  )
}

#[cfg(test)]
mod tests {
  use crate::object::Number;
  use crate::reader::{Readable, Reader, ReaderExt};

  fn read<'a, T: Readable<'a>>(data: &'a [u8]) -> Result<Option<T>, console_core_never::Never> {
    let Ok(mut reader) = Reader::new(data);

    Ok(reader.read_without_context::<T>())
  }

  fn number_as_f32(data: &[u8]) -> Result<Option<f32>, console_core_never::Never> {
    let Ok(read) = read::<Number>(data);

    Ok(read.map(|number| {
      let Ok(wide) = number.as_f64();
      let Ok(narrow) = console_core_number_conversion::narrow_f32(wide);
      narrow
    }))
  }

  fn number_as_i64(data: &[u8]) -> Result<Option<i64>, console_core_never::Never> {
    let Ok(read) = read::<Number>(data);

    Ok(read.map(|number| {
      let Ok(whole) = number.as_i64();
      whole
    }))
  }

  #[test]
  fn int_1() {
    assert_eq!(read::<i32>(b"0"), Ok(Some(0)));
  }

  #[test]
  fn int_3() {
    assert_eq!(read::<i32>(b"+32"), Ok(Some(32)));
  }

  #[test]
  fn int_4() {
    assert_eq!(read::<i32>(b"-32"), Ok(Some(-32)));
  }

  #[test]
  fn int_6() {
    assert_eq!(read::<i32>(b"98349"), Ok(Some(98349)));
  }

  #[test]
  fn int_7() {
    assert_eq!(read::<i32>(b"003245"), Ok(Some(3245)));
  }

  #[test]
  fn int_min_does_not_panic() {
    assert_eq!(read::<i64>(b"-9223372036854775808"), Ok(Some(i64::MIN)));
  }

  #[test]
  fn real_1() {
    assert_eq!(read::<f32>(b"3"), Ok(Some(3.0)));
  }

  #[test]
  fn real_3() {
    assert_eq!(read::<f32>(b"+32"), Ok(Some(32.0)));
  }

  #[test]
  fn real_4() {
    assert_eq!(read::<f32>(b"-32"), Ok(Some(-32.0)));
  }

  #[test]
  fn real_5() {
    assert_eq!(read::<f32>(b"-32.01"), Ok(Some(-32.01)));
  }

  #[test]
  fn real_6() {
    assert_eq!(read::<f32>(b"-.345"), Ok(Some(-0.345)));
  }

  #[test]
  fn real_7() {
    assert_eq!(read::<f32>(b"-.00143"), Ok(Some(-0.00143)));
  }

  #[test]
  fn real_8() {
    assert_eq!(read::<f32>(b"-12.0013"), Ok(Some(-12.0013)));
  }

  #[test]
  fn real_9() {
    assert_eq!(read::<f32>(b"98349.432534"), Ok(Some(98_349.43)));
  }

  #[test]
  fn real_10() {
    assert_eq!(read::<f32>(b"-34534656.34"), Ok(Some(-34534656.34)));
  }

  #[test]
  fn real_failing() {
    assert_eq!(read::<f32>(b"+abc"), Ok(None));
  }

  #[test]
  fn number_1() {
    assert_eq!(number_as_f32(b"+32"), Ok(Some(32.0)));
  }

  #[test]
  fn number_2() {
    assert_eq!(number_as_f32(b"-32.01"), Ok(Some(-32.01)));
  }

  #[test]
  fn number_3() {
    assert_eq!(number_as_f32(b"-.345"), Ok(Some(-0.345)));
  }

  #[test]
  fn large_number() {
    assert_eq!(number_as_i64(b"38359922"), Ok(Some(38359922)));
  }

  #[test]
  fn large_number_2() {
    assert_eq!(read::<u32>(b"4294966260"), Ok(Some(4294966260)));
  }
}
