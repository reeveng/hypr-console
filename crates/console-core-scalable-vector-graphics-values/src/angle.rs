use crate::{Error, Stream};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AngleUnit {
  Degrees,
  Gradians,
  Radians,
  Turns,
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "Angle stores a value and its unit, and parsing can refuse text while programmatic values use the same public representation"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the public number is measured in the adjacent AngleUnit"
  )
)]
pub struct Angle {
  pub number: f64,
  pub unit: AngleUnit,
}

impl Angle {
  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the public number is measured in the adjacent AngleUnit"
    )
  )]
  pub fn new(
    number: f64,
    unit: AngleUnit,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { number, unit })
  }

  #[inline]
  pub fn to_degrees(&self) -> core::result::Result<f64, console_core_never::Never> {
    Ok(match self.unit {
      AngleUnit::Degrees => self.number,
      AngleUnit::Gradians => self.number * 180.0 / 200.0,
      AngleUnit::Radians => self.number.to_degrees(),
      AngleUnit::Turns => self.number * 360.0,
    })
  }
}

impl core::str::FromStr for Angle {
  type Err = Error;

  #[inline]
  fn from_str(text: &str) -> Result<Self, Error> {
    let mut s = Stream::from(text);
    let l = s.parse_angle()?;

    let Ok(at_end) = s.at_end();

    match ::core::primitive::bool::then_some(!matches!(at_end, crate::End::Reached), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(calc_char_pos) = s.calc_char_pos();
        return Err(Error::UnexpectedData(calc_char_pos));
      }
      ::core::option::Option::None => {}
    }

    let Ok(angle) = Self::new(l.number, l.unit);

    Ok(angle)
  }
}

impl Stream<'_> {
  pub(crate) fn take_angle_unit(
    &mut self,
  ) -> Result<Option<AngleUnit>, console_core_never::Never> {
    self.take_prefix(&[
      (b"deg", AngleUnit::Degrees),
      (b"grad", AngleUnit::Gradians),
      (b"rad", AngleUnit::Radians),
      (b"turn", AngleUnit::Turns),
    ])
  }

  pub fn parse_angle(&mut self) -> Result<Angle, Error> {
    let Ok(()) = self.skip_spaces();
    let value = self.parse_number()?;
    let Ok(unit) = self.take_angle_unit();
    let measure = match unit {
      Some(unit_2) => unit_2,
      None => AngleUnit::Degrees,
    };
    let Ok(angle) = Angle::new(value, measure);

    Ok(angle)
  }
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use core::str::FromStr;

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "each compact upstream fixture generates a separately named test with its own input and expected SVG value"
      )
    )]
    macro_rules! test_p {
        ($name:ident, $text:expr, $result:expr) => (
            #[test]
            fn $name() {
                assert_eq!(Angle::from_str($text), Ok($result));
            }
        )
    }

    test_p!(parse_1,  "1",   Angle { number: 1.0, unit: AngleUnit::Degrees });
    test_p!(parse_2,  "1deg", Angle { number: 1.0, unit: AngleUnit::Degrees });
    test_p!(parse_3,  "1grad", Angle { number: 1.0, unit: AngleUnit::Gradians });
    test_p!(parse_4,  "1rad", Angle { number: 1.0, unit: AngleUnit::Radians });
    test_p!(parse_5,  "1turn", Angle { number: 1.0, unit: AngleUnit::Turns });

    #[test]
    fn err_1() {
        let mut s = Stream::from("1q");
        assert_eq!(s.parse_angle(), Ok(Angle { number: 1.0, unit: AngleUnit::Degrees }));
        assert_eq!(s.parse_angle().map_err(|fault| fault.to_string()), Err("invalid number at position 2".to_string()));
    }

    #[test]
    fn err_2() {
        assert_eq!(Angle::from_str("1degq").map_err(|fault| fault.to_string()), Err("unexpected data at position 5".to_string()));
    }
}
