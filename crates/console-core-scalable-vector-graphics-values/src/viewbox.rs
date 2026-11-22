use crate::Stream;

#[derive(Clone, Copy, Debug)]
pub enum ViewBoxError {
  InvalidNumber,

  InvalidSize,
}

impl core::fmt::Display for ViewBoxError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match *self {
      Self::InvalidNumber => {
        write!(f, "viewBox contains an invalid number")
      }
      Self::InvalidSize => {
        write!(f, "viewBox has a negative or zero size")
      }
    }
  }
}

impl core::error::Error for ViewBoxError {
  fn description(&self) -> &str {
    "a viewBox parsing error"
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "the public rectangle representation is shared with SVG callers, while FromStr enforces the document viewBox size rule"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the public x, y, width and height are expressed in SVG user-space units"
  )
)]
pub struct ViewBox {
  pub x: f64,
  pub y: f64,
  pub w: f64,
  pub h: f64,
}

impl ViewBox {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the public x, y, width and height are expressed in SVG user-space units"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the established public constructor follows the argument order of the SVG grammar"
    )
  )]
  pub fn new(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { x, y, w, h })
  }
}

impl core::str::FromStr for ViewBox {
  type Err = ViewBoxError;

  fn from_str(text: &str) -> Result<Self, ViewBoxError> {
    let mut s = Stream::from(text);

    let x = s
      .parse_list_number()
      .map_err(|_| ViewBoxError::InvalidNumber)?;
    let y = s
      .parse_list_number()
      .map_err(|_| ViewBoxError::InvalidNumber)?;
    let w = s
      .parse_list_number()
      .map_err(|_| ViewBoxError::InvalidNumber)?;
    let h = s
      .parse_list_number()
      .map_err(|_| ViewBoxError::InvalidNumber)?;

    match ::core::primitive::bool::then_some(w <= 0.0 || h <= 0.0, ()) {
      ::core::option::Option::Some(()) => {
        return Err(ViewBoxError::InvalidSize);
      }
      ::core::option::Option::None => {}
    }

    let Ok(view_box) = Self::new(x, y, w, h);

    Ok(view_box)
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
    macro_rules! test {
        ($name:ident, $text:expr, $result:expr) => (
            #[test]
            fn $name() -> Result<(), ViewBoxError> {
                let parsed = ViewBox::from_str($text)?;
                assert_eq!(parsed, $result);

                Ok(())
            }
        )
    }

    test!(parse_1, "-20 30 100 500", ViewBox { x: -20.0, y: 30.0, w: 100.0, h: 500.0 });

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "each compact upstream fixture generates a separately named test with its own input and expected SVG value"
      )
    )]
    macro_rules! test_err {
        ($name:ident, $text:expr, $result:expr) => (
            #[test]
            fn $name() {
                assert_eq!(ViewBox::from_str($text).map_err(|fault| fault.to_string()), Err($result.to_string()));
            }
        )
    }

    test_err!(parse_err_1, "qwe", "viewBox contains an invalid number");
    test_err!(parse_err_2, "10 20 30 0", "viewBox has a negative or zero size");
    test_err!(parse_err_3, "10 20 0 40", "viewBox has a negative or zero size");
    test_err!(parse_err_4, "10 20 0 0", "viewBox has a negative or zero size");
    test_err!(parse_err_5, "10 20 -30 0", "viewBox has a negative or zero size");
    test_err!(parse_err_6, "10 20 30 -40", "viewBox has a negative or zero size");
    test_err!(parse_err_7, "10 20 -30 -40", "viewBox has a negative or zero size");
}
