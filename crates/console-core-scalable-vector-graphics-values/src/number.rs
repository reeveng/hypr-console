use core::str::FromStr;

use crate::{Error, Stream};

#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "the public wrapper stores an SVG scalar while FromStr validates the lexical token"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped value is the unitless SVG number grammar production"
  )
)]
pub struct Number(pub f64);

impl core::str::FromStr for Number {
  type Err = Error;

  fn from_str(text: &str) -> Result<Self, Self::Err> {
    let mut s = Stream::from(text);
    let n = s.parse_number()?;
    let Ok(()) = s.skip_spaces();
    let Ok(at_end) = s.at_end();

    match ::core::primitive::bool::then_some(!matches!(at_end, crate::End::Reached), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(calc_char_pos) = s.calc_char_pos();
        return Err(Error::UnexpectedData(calc_char_pos));
      }
      ::core::option::Option::None => {}
    }

    Ok(Self(n))
  }
}

impl Stream<'_> {
  pub fn parse_number(&mut self) -> Result<f64, Error> {
    let Ok(()) = self.skip_spaces();

    let Ok(start) = self.pos();

    let Ok(at_end) = self.at_end();

    match at_end {
      crate::End::Reached => {
        let Ok(calc_char_pos_at) = self.calc_char_pos_at(start);
        return Err(Error::InvalidNumber(calc_char_pos_at));
      }
      crate::End::Pending => {}
    }

    self.parse_number_impl().map_err(|_| {
      let Ok(calc_char_pos_at) = self.calc_char_pos_at(start);

      Error::InvalidNumber(calc_char_pos_at)
    })
  }

  fn parse_number_impl(&mut self) -> Result<f64, Error> {
    let Ok(start) = self.pos();
    let opening = self.curr_byte()?;
    let first = match opening {
      b'+' | b'-' => {
        let Ok(()) = self.advance(1);

        self.curr_byte()?
      }
      byte => byte,
    };

    match first {
      b'0'..=b'9' => {
        let Ok(()) = self.skip_digits();
      }
      b'.' => {}
      _ => return Err(Error::InvalidNumber(0)),
    }

    let Ok(dot) = self.is_curr_byte_eq(b'.');

    match dot {
      crate::IsCurrByteEqAnswer::Yes => {
        let Ok(()) = self.advance(1);
        let Ok(()) = self.skip_digits();
      }
      crate::IsCurrByteEqAnswer::No => {}
    }

    self.parse_exponent()?;
    let Ok(token) = self.slice_back(start);

    match f64::from_str(token) {
      Ok(value) => match value.is_finite().then_some(()) {
        Some(()) => Ok(value),
        None => Err(Error::InvalidNumber(0)),
      },
      Err(_invalid_float_token) => Err(Error::InvalidNumber(0)),
    }
  }

  fn parse_exponent(&mut self) -> Result<(), Error> {
    match self.curr_byte() {
      Ok(b'e' | b'E') => {}
      Ok(_) => return Ok(()),
      Err(_token_ended) => return Ok(()),
    }

    let after = self.next_byte()?;

    match after {
      b'm' | b'x' => return Ok(()),
      _ => {}
    }

    let Ok(()) = self.advance(1);
    let exponent = self.curr_byte()?;

    match exponent {
      b'+' | b'-' => {
        let Ok(()) = self.advance(1);
      }
      b'0'..=b'9' => {}
      _ => return Err(Error::InvalidNumber(0)),
    }

    let Ok(()) = self.skip_digits();

    Ok(())
  }

  pub fn parse_list_number(&mut self) -> Result<f64, Error> {
    let Ok(at_end) = self.at_end();

    match at_end {
      crate::End::Reached => {
        return Err(Error::UnexpectedEndOfStream);
      }
      crate::End::Pending => {}
    }

    let n = self.parse_number()?;
    let Ok(()) = self.skip_spaces();
    let Ok(()) = self.parse_list_separator();
    Ok(n)
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NumberListParser<'a>(Stream<'a>);

impl<'a> From<&'a str> for NumberListParser<'a> {
  #[inline]
  fn from(v: &'a str) -> Self {
    NumberListParser(Stream::from(v))
  }
}

impl Iterator for NumberListParser<'_> {
  type Item = Result<f64, Error>;

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(at_end) = self.0.at_end();

    match at_end {
      crate::End::Reached => None,
      crate::End::Pending => {
        let v = self.0.parse_list_number();

        match v {
          Err(ref _invalid_number_ends_list) => {
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
    use alloc::string::ToString;
    use crate::Stream;

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
                let mut s = Stream::from($text);
                assert_eq!(s.parse_number(), Ok($result));
            }
        )
    }

    test_p!(parse_1,  "0", 0.0);
    test_p!(parse_2,  "1", 1.0);
    test_p!(parse_3,  "-1", -1.0);
    test_p!(parse_4,  " -1 ", -1.0);
    test_p!(parse_5,  "  1  ", 1.0);
    test_p!(parse_6,  ".4", 0.4);
    test_p!(parse_7,  "-.4", -0.4);
    test_p!(parse_8,  "-.4text", -0.4);
    test_p!(parse_9,  "-.01 text", -0.01);
    test_p!(parse_10, "-.01 4", -0.01);
    test_p!(parse_11, ".0000000000008", 0.0000000000008);
    test_p!(parse_12, "1000000000000", 1000000000000.0);
    test_p!(parse_13, "123456.123456", 123456.123456);
    test_p!(parse_14, "+10", 10.0);
    test_p!(parse_15, "1e2", 100.0);
    test_p!(parse_16, "1e+2", 100.0);
    test_p!(parse_17, "1E2", 100.0);
    test_p!(parse_18, "1e-2", 0.01);
    test_p!(parse_19, "1ex", 1.0);
    test_p!(parse_20, "1em", 1.0);
    test_p!(parse_21, "12345678901234567890", 12345678901234567000.0);
    test_p!(parse_22, "0.", 0.0);
    test_p!(parse_23, "1.3e-2", 0.013);

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "each compact upstream fixture generates a separately named test with its own input and expected SVG value"
      )
    )]
    macro_rules! test_p_err {
        ($name:ident, $text:expr) => (
            #[test]
            fn $name() {
                let mut s = Stream::from($text);
                assert_eq!(s.parse_number().map_err(|fault| fault.to_string()), Err("invalid number at position 1".to_string()));
            }
        )
    }

    test_p_err!(parse_err_1, "q");
    test_p_err!(parse_err_2, "");
    test_p_err!(parse_err_3, "-");
    test_p_err!(parse_err_4, "+");
    test_p_err!(parse_err_5, "-q");
    test_p_err!(parse_err_6, ".");
    test_p_err!(parse_err_7, "99999999e99999999");
    test_p_err!(parse_err_8, "-99999999e99999999");
}
