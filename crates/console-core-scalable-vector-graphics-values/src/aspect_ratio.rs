use crate::{Error, Stream};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
  None,
  XMinYMin,
  XMidYMin,
  XMaxYMin,
  XMinYMid,
  XMidYMid,
  XMaxYMid,
  XMinYMax,
  XMidYMax,
  XMaxYMax,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "default and named alignments are valid typed values, and FromStr refuses malformed SVG text"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "defer and slice encode separate SVG tokens and the grammar permits all combinations"
  )
)]
pub struct AspectRatio {
  pub defer: bool,

  pub align: Align,

  pub slice: bool,
}

impl core::str::FromStr for AspectRatio {
  type Err = Error;

  fn from_str(text: &str) -> Result<Self, Error> {
    let mut s = Stream::from(text);

    let Ok(()) = s.skip_spaces();

    let Ok(starts_with) = s.starts_with(b"defer");
    let defer = matches!(starts_with, crate::PrefixMatch::Matches);

    match ::core::primitive::bool::then_some(defer, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = s.advance(5);
        s.consume_byte(b' ')?;
        let Ok(()) = s.skip_spaces();
      }
      ::core::option::Option::None => {}
    }

    let Ok(start) = s.pos();
    let Ok(align) = s.consume_ascii_ident();
    let align_2 = match align {
      "none" => Align::None,
      "xMinYMin" => Align::XMinYMin,
      "xMidYMin" => Align::XMidYMin,
      "xMaxYMin" => Align::XMaxYMin,
      "xMinYMid" => Align::XMinYMid,
      "xMidYMid" => Align::XMidYMid,
      "xMaxYMid" => Align::XMaxYMid,
      "xMinYMax" => Align::XMinYMax,
      "xMidYMax" => Align::XMidYMax,
      "xMaxYMax" => Align::XMaxYMax,
      _ => {
        let Ok(calc_char_pos_at) = s.calc_char_pos_at(start);

        return Err(Error::UnexpectedData(calc_char_pos_at));
      }
    };

    let Ok(()) = s.skip_spaces();

    let mut slice = false;
    let Ok(at_end) = s.at_end();

    match ::core::primitive::bool::then_some(!matches!(at_end, crate::End::Reached), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(start_2) = s.pos();
        let Ok(v) = s.consume_ascii_ident();

        match v {
          "meet" => {}
          "slice" => slice = true,
          "" => {}
          _ => {
            let Ok(calc_char_pos_at) = s.calc_char_pos_at(start_2);

            return Err(Error::UnexpectedData(calc_char_pos_at));
          }
        };
      }
      ::core::option::Option::None => {}
    }

    Ok(Self {
      defer,
      align: align_2,
      slice,
    })
  }
}

impl Default for AspectRatio {
  #[inline]
  fn default() -> Self {
    Self {
      defer: false,
      align: Align::XMidYMid,
      slice: false,
    }
  }
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;
    use core::str::FromStr;

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "each compact upstream fixture generates a separately named test with its own input and expected SVG value"
      )
    )]
    macro_rules! test {
        ($name:ident, $text:expr, $result:expr) => (
            #[test]
            fn $name() {
                assert_eq!(AspectRatio::from_str($text), Ok($result));
            }
        )
    }

    test!(parse_1, "none", AspectRatio {
        defer: false,
        align: Align::None,
        slice: false,
    });

    test!(parse_2, "defer none", AspectRatio {
        defer: true,
        align: Align::None,
        slice: false,
    });

    test!(parse_3, "xMinYMid", AspectRatio {
        defer: false,
        align: Align::XMinYMid,
        slice: false,
    });

    test!(parse_4, "xMinYMid slice", AspectRatio {
        defer: false,
        align: Align::XMinYMid,
        slice: true,
    });

    test!(parse_5, "xMinYMid meet", AspectRatio {
        defer: false,
        align: Align::XMinYMid,
        slice: false,
    });
}
