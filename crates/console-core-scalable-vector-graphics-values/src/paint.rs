use core::str::FromStr;

use crate::{Color, Error, Stream};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaintFallback {
  None,

  CurrentColor,

  Color(Color),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paint<'a> {
  None,

  Inherit,

  CurrentColor,

  Color(Color),

  FuncIRI(&'a str, Option<PaintFallback>),

  ContextFill,

  ContextStroke,
}

impl<'a> Paint<'a> {
  #[expect(
    clippy::should_implement_trait,
    reason = "the value parsed borrows from the text it was parsed from, which FromStr has no lifetime to say"
  )]
  pub fn from_str(text: &'a str) -> Result<Self, Error> {
    let value = text.trim();

    match value {
      "none" => Ok(Paint::None),
      "inherit" => Ok(Paint::Inherit),
      "currentColor" => Ok(Paint::CurrentColor),
      "context-fill" => Ok(Paint::ContextFill),
      "context-stroke" => Ok(Paint::ContextStroke),
      _ => {
        let mut stream = Stream::from(value);
        let Ok(prefix) = stream.starts_with(b"url(");

        match prefix {
          crate::PrefixMatch::Matches => reference(&mut stream),
          crate::PrefixMatch::Different => Color::from_str(value)
            .map(Paint::Color)
            .map_err(|_invalid_color| Error::InvalidValue),
        }
      }
    }
  }
}

fn reference<'a>(stream: &mut Stream<'a>) -> Result<Paint<'a>, Error> {
  let link = stream
    .parse_func_iri()
    .map_err(|_invalid_reference| Error::InvalidValue)?;
  let Ok(()) = stream.skip_spaces();
  let Ok(fallback) = stream.slice_tail();
  let alternative = match fallback {
    "" => None,
    "none" => Some(PaintFallback::None),
    "currentColor" => Some(PaintFallback::CurrentColor),
    _ => {
      let color = Color::from_str(fallback)?;

      Some(PaintFallback::Color(color))
    }
  };

  Ok(Paint::FuncIRI(link, alternative))
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

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
                assert_eq!(Paint::from_str($text), Ok($result));
            }
        )
    }

    test!(parse_1, "none", Paint::None);
    test!(parse_2, "  none   ", Paint::None);
    test!(parse_3, " inherit ", Paint::Inherit);
    test!(parse_4, " currentColor ", Paint::CurrentColor);
    test!(parse_5, " red ", Paint::Color(Color { red: 255, green: 0, blue: 0, alpha: 255 }));
    test!(parse_6, " url(#qwe) ", Paint::FuncIRI("qwe", None));
    test!(parse_7, " url(#qwe) none ", Paint::FuncIRI("qwe", Some(PaintFallback::None)));
    test!(parse_8, " url(#qwe) currentColor ", Paint::FuncIRI("qwe", Some(PaintFallback::CurrentColor)));
    test!(parse_9, " url(#qwe) red ", Paint::FuncIRI("qwe", Some(PaintFallback::Color(Color { red: 255, green: 0, blue: 0, alpha: 255 }))));

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
                assert_eq!(Paint::from_str($text).map_err(|fault| fault.to_string()), Err($result.to_string()));
            }
        )
    }

    test_err!(parse_err_1, "qwe", "invalid value");
    test_err!(parse_err_2, "red icc-color(acmecmyk, 0.11, 0.48, 0.83, 0.00)", "invalid value");

}
