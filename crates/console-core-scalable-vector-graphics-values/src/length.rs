use crate::{Error, Stream};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LengthUnit {
  None,
  Em,
  Ex,
  Px,
  In,
  Cm,
  Mm,
  Pt,
  Pc,
  Percent,
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "parsing validates the unit spelling while typed Length stores the numeric value and a LengthUnit"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the public number is measured in the adjacent LengthUnit"
  )
)]
pub struct Length {
  pub number: f64,
  pub unit: LengthUnit,
}

impl Length {
  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the public number is measured in the adjacent LengthUnit"
    )
  )]
  pub fn new(
    number: f64,
    unit: LengthUnit,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { number, unit })
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "this constructor creates an SVG length in implicit user-space units"
    )
  )]
  pub fn new_number(number: f64) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      number,
      unit: LengthUnit::None,
    })
  }

  #[inline]
  pub fn zero() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      number: 0.0,
      unit: LengthUnit::None,
    })
  }
}

impl Default for Length {
  #[inline]
  fn default() -> Self {
    let Ok(zero) = Self::zero();

    zero
  }
}

impl core::str::FromStr for Length {
  type Err = Error;

  #[inline]
  fn from_str(text: &str) -> Result<Self, Error> {
    let mut s = Stream::from(text);
    let l = s.parse_length()?;

    let Ok(at_end) = s.at_end();

    match ::core::primitive::bool::then_some(!matches!(at_end, crate::End::Reached), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(calc_char_pos) = s.calc_char_pos();
        return Err(Error::UnexpectedData(calc_char_pos));
      }
      ::core::option::Option::None => {}
    }

    let Ok(length) = Self::new(l.number, l.unit);

    Ok(length)
  }
}

impl Stream<'_> {
  pub fn parse_length(&mut self) -> Result<Length, Error> {
    let Ok(()) = self.skip_spaces();
    let value = self.parse_number()?;
    let Ok(unit) = self.take_prefix(&[
      (b"%", LengthUnit::Percent),
      (b"em", LengthUnit::Em),
      (b"ex", LengthUnit::Ex),
      (b"px", LengthUnit::Px),
      (b"in", LengthUnit::In),
      (b"cm", LengthUnit::Cm),
      (b"mm", LengthUnit::Mm),
      (b"pt", LengthUnit::Pt),
      (b"pc", LengthUnit::Pc),
    ]);
    let measure = match unit {
      Some(unit_2) => unit_2,
      None => LengthUnit::None,
    };
    let Ok(length) = Length::new(value, measure);

    Ok(length)
  }

  pub fn parse_list_length(&mut self) -> Result<Length, Error> {
    let Ok(at_end) = self.at_end();

    match at_end {
      crate::End::Reached => {
        return Err(Error::UnexpectedEndOfStream);
      }
      crate::End::Pending => {}
    }

    let l = self.parse_length()?;
    let Ok(()) = self.skip_spaces();
    let Ok(()) = self.parse_list_separator();
    Ok(l)
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LengthListParser<'a>(Stream<'a>);

impl<'a> From<&'a str> for LengthListParser<'a> {
  #[inline]
  fn from(v: &'a str) -> Self {
    LengthListParser(Stream::from(v))
  }
}

impl Iterator for LengthListParser<'_> {
  type Item = Result<Length, Error>;

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(at_end) = self.0.at_end();

    match at_end {
      crate::End::Reached => None,
      crate::End::Pending => {
        let v = self.0.parse_list_length();

        match v {
          ::core::result::Result::Err(ref _invalid_length_ends_list) => {
            let Ok(()) = self.0.jump_to_end();
          }
          ::core::result::Result::Ok(_) => {}
        }

        Some(v)
      }
    }
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
                assert_eq!(Length::from_str($text), Ok($result));
            }
        )
    }

    test_p!(parse_1,  "1",   Length { number: 1.0, unit: LengthUnit::None });
    test_p!(parse_2,  "1em", Length { number: 1.0, unit: LengthUnit::Em });
    test_p!(parse_3,  "1ex", Length { number: 1.0, unit: LengthUnit::Ex });
    test_p!(parse_4,  "1px", Length { number: 1.0, unit: LengthUnit::Px });
    test_p!(parse_5,  "1in", Length { number: 1.0, unit: LengthUnit::In });
    test_p!(parse_6,  "1cm", Length { number: 1.0, unit: LengthUnit::Cm });
    test_p!(parse_7,  "1mm", Length { number: 1.0, unit: LengthUnit::Mm });
    test_p!(parse_8,  "1pt", Length { number: 1.0, unit: LengthUnit::Pt });
    test_p!(parse_9,  "1pc", Length { number: 1.0, unit: LengthUnit::Pc });
    test_p!(parse_10, "1%",  Length { number: 1.0, unit: LengthUnit::Percent });
    test_p!(parse_11, "1e0", Length { number: 1.0, unit: LengthUnit::None });
    test_p!(parse_12, "1.0e0", Length { number: 1.0, unit: LengthUnit::None });
    test_p!(parse_13, "1.0e0em", Length { number: 1.0, unit: LengthUnit::Em });

    #[test]
    fn parse_14() {
        let mut s = Stream::from("1,");
        assert_eq!(s.parse_length(), Ok(Length { number: 1.0, unit: LengthUnit::None }));
    }

    #[test]
    fn parse_15() {
        let mut s = Stream::from("1 ,");
        assert_eq!(s.parse_length(), Ok(Length { number: 1.0, unit: LengthUnit::None }));
    }

    #[test]
    fn parse_16() {
        let mut s = Stream::from("1 1");
        assert_eq!(s.parse_length(), Ok(Length { number: 1.0, unit: LengthUnit::None }));
    }

    #[test]
    fn err_1() {
        let mut s = Stream::from("1q");
        assert_eq!(s.parse_length(), Ok(Length { number: 1.0, unit: LengthUnit::None }));
        assert_eq!(s.parse_length().map_err(|fault| fault.to_string()), Err("invalid number at position 2".to_string()));
    }

    #[test]
    fn err_2() {
        assert_eq!(Length::from_str("1mmx").map_err(|fault| fault.to_string()), Err("unexpected data at position 4".to_string()));
    }
}
