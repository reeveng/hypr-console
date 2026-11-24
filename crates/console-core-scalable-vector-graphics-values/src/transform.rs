use console_core_point::Point;
use console_core_size::Size;
use console_core_transform::AffineTransform;
use core::f64;

use crate::{Error, Stream};

#[cfg(not(feature = "std"))]
use kurbo::common::FloatFuncs;

#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "the public coefficient layout represents an arbitrary affine matrix and FromStr rejects malformed transform syntax"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the public affine matrix has unitless linear coefficients and user-space translations in e and f"
  )
)]
pub struct Transform {
  pub a: f64,
  pub b: f64,
  pub c: f64,
  pub d: f64,
  pub e: f64,
  pub f: f64,
}

impl Transform {
  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the public affine matrix has unitless linear coefficients and user-space translations in e and f"
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
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { a, b, c, d, e, f })
  }
}

impl Default for Transform {
  #[inline]
  fn default() -> Self {
    let Ok(transform) = Self::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);

    transform
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TransformListToken {
  Matrix {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
  },
  Translate {
    tx: f64,
    ty: f64,
  },
  Scale {
    sx: f64,
    sy: f64,
  },
  Rotate {
    angle: f64,
  },
  SkewX {
    angle: f64,
  },
  SkewY {
    angle: f64,
  },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TransformListParser<'a> {
  stream: Stream<'a>,
  rotate_ts: Option<(f64, f64)>,
  last_angle: Option<f64>,
}

impl<'a> From<&'a str> for TransformListParser<'a> {
  fn from(text: &'a str) -> Self {
    TransformListParser {
      stream: Stream::from(text),
      rotate_ts: None,
      last_angle: None,
    }
  }
}

impl Iterator for TransformListParser<'_> {
  type Item = Result<TransformListToken, Error>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.last_angle {
      Some(a) => {
        self.last_angle = None;
        return Some(Ok(TransformListToken::Rotate { angle: a }));
      }
      None => {}
    }

    match self.rotate_ts {
      Some((x, y)) => {
        self.rotate_ts = None;
        return Some(Ok(TransformListToken::Translate { tx: -x, ty: -y }));
      }
      None => {}
    }

    let Ok(()) = self.stream.skip_spaces();

    let Ok(at_end) = self.stream.at_end();

    match at_end {
      crate::End::Reached => {
        return None;
      }
      crate::End::Pending => {}
    }

    let res = self.parse_next();

    match res {
      ::core::result::Result::Err(ref _invalid_transform_ends_list) => {
        let Ok(()) = self.stream.jump_to_end();
      }
      ::core::result::Result::Ok(_) => {}
    }

    Some(res)
  }
}

impl TransformListParser<'_> {
  fn parse_next(&mut self) -> Result<TransformListToken, Error> {
    let s = &mut self.stream;

    let Ok(start) = s.pos();
    let Ok(name) = s.consume_ascii_ident();
    let Ok(()) = s.skip_spaces();
    s.consume_byte(b'(')?;

    let t = match name.as_bytes() {
      b"matrix" => {
        let list_number = s.parse_list_number()?;
        let b = s.parse_list_number()?;
        let c = s.parse_list_number()?;
        let d = s.parse_list_number()?;
        let e = s.parse_list_number()?;
        let f = s.parse_list_number()?;

        TransformListToken::Matrix {
          a: list_number,
          b,
          c,
          d,
          e,
          f,
        }
      }
      b"translate" => {
        let x = s.parse_list_number()?;
        let Ok(()) = s.skip_spaces();

        let Ok(is_curr_byte_eq) = s.is_curr_byte_eq(b')');
        let y = match is_curr_byte_eq {
          crate::IsCurrByteEqAnswer::Yes => 0.0,
          crate::IsCurrByteEqAnswer::No => s.parse_list_number()?,
        };

        TransformListToken::Translate { tx: x, ty: y }
      }
      b"scale" => {
        let x = s.parse_list_number()?;
        let Ok(()) = s.skip_spaces();

        let Ok(is_curr_byte_eq) = s.is_curr_byte_eq(b')');
        let y = match is_curr_byte_eq {
          crate::IsCurrByteEqAnswer::Yes => x,
          crate::IsCurrByteEqAnswer::No => s.parse_list_number()?,
        };

        TransformListToken::Scale { sx: x, sy: y }
      }
      b"rotate" => {
        let a = s.parse_list_number()?;
        let Ok(()) = s.skip_spaces();

        let Ok(is_curr_byte_eq) = s.is_curr_byte_eq(b')');

        match ::core::primitive::bool::then_some(
          !matches!(is_curr_byte_eq, crate::IsCurrByteEqAnswer::Yes),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            let cx = s.parse_list_number()?;
            let cy = s.parse_list_number()?;
            self.rotate_ts = Some((cx, cy));
            self.last_angle = Some(a);

            TransformListToken::Translate { tx: cx, ty: cy }
          }
          ::core::option::Option::None => TransformListToken::Rotate { angle: a },
        }
      }
      b"skewX" => {
        let angle = s.parse_list_number()?;

        TransformListToken::SkewX { angle }
      }
      b"skewY" => {
        let angle = s.parse_list_number()?;

        TransformListToken::SkewY { angle }
      }
      _ => {
        let Ok(calc_char_pos_at) = s.calc_char_pos_at(start);
        return Err(Error::UnexpectedData(calc_char_pos_at));
      }
    };

    let Ok(()) = s.skip_spaces();
    s.consume_byte(b')')?;
    let Ok(()) = s.skip_spaces();

    let Ok(is_curr_byte_eq) = s.is_curr_byte_eq(b',');

    match is_curr_byte_eq {
      crate::IsCurrByteEqAnswer::Yes => {
        let Ok(()) = s.advance(1);
      }
      crate::IsCurrByteEqAnswer::No => {}
    }

    Ok(t)
  }
}

impl core::str::FromStr for Transform {
  type Err = Error;

  fn from_str(text: &str) -> Result<Self, Error> {
    let mut combined = AffineTransform::IDENTITY;

    for token in TransformListParser::from(text) {
      let checked_value = token?;
      let Ok(transform) = match checked_value {
        TransformListToken::Matrix { a, b, c, d, e, f } => {
          AffineTransform::from_coefficients([a, b, c, d, e, f])
        }
        TransformListToken::Translate { tx, ty } => {
          AffineTransform::translate(Point { x: tx, y: ty })
        }
        TransformListToken::Scale { sx, sy } => AffineTransform::scale(Size {
          width: sx,
          height: sy,
        }),
        TransformListToken::Rotate { angle } => {
          let radians = angle.to_radians();
          let cosine = radians.cos();
          let sine = radians.sin();

          AffineTransform::from_coefficients([cosine, sine, -sine, cosine, 0.0, 0.0])
        }
        TransformListToken::SkewX { angle } => {
          let skew = angle.to_radians().tan();

          AffineTransform::from_coefficients([1.0, 0.0, skew, 1.0, 0.0, 0.0])
        }
        TransformListToken::SkewY { angle } => {
          let skew = angle.to_radians().tan();

          AffineTransform::from_coefficients([1.0, skew, 0.0, 1.0, 0.0, 0.0])
        }
      };

      combined = combined * transform;
    }

    let Ok([a, b, c, d, e, f]) = combined.coefficients();

    Ok(Self { a, b, c, d, e, f })
  }
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
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
            fn $name() -> Result<(), Error> {
                let ts = Transform::from_str($text)?;
                let s = format!("matrix({} {} {} {} {} {})", ts.a, ts.b, ts.c, ts.d, ts.e, ts.f);
                assert_eq!(s, $result);

                Ok(())
            }
        )
    }

    test!(parse_1,
        "matrix(1 0 0 1 10 20)",
        "matrix(1 0 0 1 10 20)"
    );

    test!(parse_2,
        "translate(10 20)",
        "matrix(1 0 0 1 10 20)"
    );

    test!(parse_3,
        "scale(2 3)",
        "matrix(2 0 0 3 0 0)"
    );

    test!(parse_4,
        "rotate(30)",
        "matrix(0.8660254037844387 0.49999999999999994 -0.49999999999999994 0.8660254037844387 0 0)"
    );

    test!(parse_5,
        "rotate(30 10 20)",
        "matrix(0.8660254037844387 0.49999999999999994 -0.49999999999999994 0.8660254037844387 11.339745962155611 -2.3205080756887746)"
    );

    test!(parse_6,
        "translate(10 15) translate(0 5)",
        "matrix(1 0 0 1 10 20)"
    );

    test!(parse_7,
        "translate(10) scale(2)",
        "matrix(2 0 0 2 10 0)"
    );

    test!(parse_8,
        "translate(25 215) scale(2) skewX(45)",
        "matrix(2 0 1.9999999999999998 2 25 215)"
    );

    test!(parse_9,
        "skewX(45)",
        "matrix(1 0 0.9999999999999999 1 0 0)"
    );

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
                let ts = Transform::from_str($text);
                assert_eq!(ts.map_err(|fault| fault.to_string()), Err($result.to_string()));
            }
        )
    }

    test_err!(parse_err_1, "text", "unexpected end of stream");

    #[test]
    fn parse_err_2() {
        let mut ts = TransformListParser::from("scale(2) text");
        assert!(ts.next().is_some());
        assert_eq!(ts.next().map(|item| item.map_err(|fault| fault.to_string())), Some(Err("unexpected end of stream".to_string())));
    }

    test_err!(parse_err_3, "???G", "expected '(' not '?' at position 1");

    #[test]
    fn parse_err_4() {
        let mut ts = TransformListParser::from(" ");
        assert!(ts.next().is_none());
    }

    #[test]
    fn parse_err_5() {
        let mut ts = TransformListParser::from("\x01");
        assert_eq!(ts.next().map(|item| item.map_err(|fault| fault.to_string())), Some(Err("expected '(' not '\u{1}' at position 1".to_string())));
    }

    test_err!(parse_err_6, "rect()", "unexpected data at position 1");

    test_err!(parse_err_7, "scale(2) rect()", "unexpected data at position 10");
}
