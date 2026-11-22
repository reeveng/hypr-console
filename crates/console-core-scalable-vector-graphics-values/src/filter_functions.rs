use crate::{Angle, AngleUnit, Color, Error, Length, LengthUnit, Stream};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FilterValue<'a> {
  Blur(Length),
  DropShadow {
    color: Option<Color>,

    dx: Length,

    dy: Length,

    std_dev: Length,
  },

  Brightness(f64),

  Contrast(f64),

  Grayscale(f64),
  HueRotate(Angle),

  Invert(f64),

  Opacity(f64),

  Sepia(f64),

  Saturate(f64),

  Url(&'a str),
}

#[derive(Debug)]
pub enum FilterValueListParserError {
  PercentageValue(u64),

  NegativeValue(u64),

  InvalidAngle(u64),

  MissingDropShadowOffset(u64),

  InvalidUrl(u64),

  StreamErrors(Error),
}

impl From<Error> for FilterValueListParserError {
  fn from(e: Error) -> Self {
    Self::StreamErrors(e)
  }
}

impl core::fmt::Display for FilterValueListParserError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match *self {
      Self::PercentageValue(pos) => {
        write!(f, "a percentage value detected at position {pos}")
      }
      Self::NegativeValue(pos) => {
        write!(f, "a negative value detected at position {pos}")
      }
      Self::InvalidAngle(pos) => {
        write!(f, "an invalid angle at position {pos}")
      }
      Self::MissingDropShadowOffset(pos) => {
        write!(
          f,
          "drop-shadow offset values are expected at position {pos}"
        )
      }
      Self::InvalidUrl(pos) => {
        write!(f, "an invalid url at position {pos}")
      }
      Self::StreamErrors(ref e) => {
        write!(f, "{e}")
      }
    }
  }
}

impl core::error::Error for FilterValueListParserError {
  fn description(&self) -> &str {
    "filter-value-list parsing error"
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FilterValueListParser<'a> {
  stream: Stream<'a>,
}

impl<'a> From<&'a str> for FilterValueListParser<'a> {
  fn from(text: &'a str) -> Self {
    FilterValueListParser {
      stream: Stream::from(text),
    }
  }
}

impl<'a> Iterator for FilterValueListParser<'a> {
  type Item = Result<FilterValue<'a>, FilterValueListParserError>;

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(()) = self.stream.skip_spaces();

    let Ok(at_end) = self.stream.at_end();

    match at_end {
      crate::End::Reached => {
        return None;
      }
      crate::End::Pending => {}
    }

    let Ok(starts_with) = self.stream.starts_with(b"none");

    match starts_with {
      crate::PrefixMatch::Matches => {
        let Ok(()) = self.stream.advance(4);
        let Ok(()) = self.stream.skip_spaces();

        let Ok(own_at_end) = self.stream.at_end();

        match own_at_end {
          crate::End::Reached => {
            return None;
          }
          crate::End::Pending => {
            let Ok(()) = self.stream.jump_to_end();
            return Some(Err(
              <crate::filter_functions::FilterValueListParserError as ::core::convert::From<
                crate::error::Error,
              >>::from(Error::InvalidValue),
            ));
          }
        }
      }
      crate::PrefixMatch::Different => {}
    }

    let res = self.parse_next();

    match res {
      ::core::result::Result::Err(ref _invalid_filter_ends_list) => {
        let Ok(()) = self.stream.jump_to_end();
      }
      ::core::result::Result::Ok(_) => {}
    }

    Some(res)
  }
}

impl<'a> FilterValueListParser<'a> {
  fn parse_next(&mut self) -> Result<FilterValue<'a>, FilterValueListParserError> {
    let s = &mut self.stream;

    let Ok(start) = s.pos();
    let Ok(name) = s.consume_ascii_ident();
    let Ok(()) = s.skip_spaces();
    s.consume_byte(b'(')?;
    let Ok(()) = s.skip_spaces();

    let value = match name.as_bytes() {
      b"blur" => {
        let Ok(is_curr_byte_eq) = s.is_curr_byte_eq(b')');

        match is_curr_byte_eq {
          crate::IsCurrByteEqAnswer::Yes => {
            let Ok(zero) = Length::zero();

            FilterValue::Blur(zero)
          }
          crate::IsCurrByteEqAnswer::No => {
            let value = parse_filter_positive_length(s)?;
            FilterValue::Blur(value)
          }
        }
      }
      b"drop-shadow" => parse_drop_shadow_func(s)?,
      b"hue-rotate" => {
        let Ok(is_curr_byte_eq) = s.is_curr_byte_eq(b')');

        match is_curr_byte_eq {
          crate::IsCurrByteEqAnswer::Yes => {
            let Ok(angle) = Angle::new(0.0, AngleUnit::Degrees);

            FilterValue::HueRotate(angle)
          }
          crate::IsCurrByteEqAnswer::No => {
            let value = parse_filter_angle(s)?;
            FilterValue::HueRotate(value)
          }
        }
      }
      b"brightness" => {
        let generic_color_func = parse_generic_color_func(s)?;

        FilterValue::Brightness(generic_color_func)
      }
      b"contrast" => {
        let generic_color_func = parse_generic_color_func(s)?;

        FilterValue::Contrast(generic_color_func)
      }
      b"grayscale" => {
        let generic_color_func = parse_generic_color_func(s)?;

        FilterValue::Grayscale(generic_color_func)
      }
      b"invert" => {
        let generic_color_func = parse_generic_color_func(s)?;

        FilterValue::Invert(generic_color_func)
      }
      b"opacity" => {
        let generic_color_func = parse_generic_color_func(s)?;

        FilterValue::Opacity(generic_color_func)
      }
      b"saturate" => {
        let generic_color_func = parse_generic_color_func(s)?;

        FilterValue::Saturate(generic_color_func)
      }
      b"sepia" => {
        let generic_color_func = parse_generic_color_func(s)?;

        FilterValue::Sepia(generic_color_func)
      }
      b"url" => {
        s.consume_byte(b'#')?;
        let Ok(link) = s.consume_bytes(|_, c| c != b' ' && c != b')');

        match ::core::primitive::bool::then_some(!link.is_empty(), ()) {
          ::core::option::Option::Some(()) => FilterValue::Url(link),
          ::core::option::Option::None => {
            let Ok(calc_char_pos_at) = s.calc_char_pos_at(start);
            return Err(FilterValueListParserError::InvalidUrl(calc_char_pos_at));
          }
        }
      }
      _ => {
        let Ok(calc_char_pos_at) = s.calc_char_pos_at(start);
        return Err(
          <crate::filter_functions::FilterValueListParserError as ::core::convert::From<
            crate::error::Error,
          >>::from(Error::UnexpectedData(calc_char_pos_at)),
        );
      }
    };

    let Ok(()) = s.skip_spaces();
    s.consume_byte(b')')?;
    let Ok(()) = s.skip_spaces();

    Ok(value)
  }
}

#[inline(never)]
fn parse_drop_shadow_func<'a>(
  s: &mut Stream<'a>,
) -> Result<FilterValue<'a>, FilterValueListParserError> {
  let Ok(is_curr_byte_eq) = s.is_curr_byte_eq(b')');

  match is_curr_byte_eq {
    crate::IsCurrByteEqAnswer::Yes => {
      let Ok(pos) = s.calc_char_pos();
      return Err(FilterValueListParserError::MissingDropShadowOffset(pos));
    }
    crate::IsCurrByteEqAnswer::No => {}
  }

  let Ok(leading) = shadow_color(s);

  match leading {
    ShadowColor::Explicit(_) | ShadowColor::Current => {
      let Ok(()) = s.skip_spaces();
    }
    ShadowColor::Absent => {}
  }

  let dx = parse_filter_length(s)?;
  let Ok(()) = s.skip_spaces();
  let dy = parse_filter_length(s)?;
  let Ok(()) = s.skip_spaces();
  let std_dev = match parse_filter_positive_length(s) {
    Ok(blur) => {
      let Ok(()) = s.skip_spaces();

      blur
    }
    Err(_no_optional_blur) => {
      let Ok(zero) = Length::zero();

      zero
    }
  };
  let color = match leading {
    ShadowColor::Explicit(color) => Some(color),
    ShadowColor::Current => None,
    ShadowColor::Absent => {
      let Ok(trailing) = shadow_color(s);

      match trailing {
        ShadowColor::Explicit(color) => {
          let Ok(()) = s.skip_spaces();

          Some(color)
        }
        ShadowColor::Current | ShadowColor::Absent => None,
      }
    }
  };

  Ok(FilterValue::DropShadow {
    color,
    dx,
    dy,
    std_dev,
  })
}

enum ShadowColor {
  Explicit(Color),
  Current,
  Absent,
}

fn shadow_color(stream: &mut Stream<'_>) -> Result<ShadowColor, console_core_never::Never> {
  let Ok(color) = stream.try_parse_color();

  match color {
    Some(color_2) => Ok(ShadowColor::Explicit(color_2)),
    None => {
      let Ok(keyword) = stream.take_prefix(&[(b"currentColor", ())]);

      Ok(match keyword {
        Some(()) => ShadowColor::Current,
        None => ShadowColor::Absent,
      })
    }
  }
}

#[inline(never)]
fn parse_generic_color_func(s: &mut Stream<'_>) -> Result<f64, FilterValueListParserError> {
  let Ok(is_curr_byte_eq) = s.is_curr_byte_eq(b')');

  match is_curr_byte_eq {
    crate::IsCurrByteEqAnswer::Yes => Ok(1.0),
    crate::IsCurrByteEqAnswer::No => {
      let Ok(start) = s.pos();
      let value = s.parse_number_or_percent()?;

      match ::core::primitive::bool::then_some(value.is_sign_negative(), ()) {
        ::core::option::Option::Some(()) => {
          let Ok(pos) = s.calc_char_pos_at(start);
          return Err(FilterValueListParserError::NegativeValue(pos));
        }
        ::core::option::Option::None => {}
      }

      Ok(value)
    }
  }
}

fn parse_filter_length(s: &mut Stream<'_>) -> Result<Length, FilterValueListParserError> {
  let Ok(start) = s.pos();
  let value = s.parse_length()?;

  match ::core::primitive::bool::then_some(value.unit == LengthUnit::Percent, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(pos) = s.calc_char_pos_at(start);
      return Err(FilterValueListParserError::PercentageValue(pos));
    }
    ::core::option::Option::None => {}
  }

  Ok(value)
}

fn parse_filter_positive_length(
  s: &mut Stream<'_>,
) -> Result<Length, FilterValueListParserError> {
  let Ok(start) = s.pos();
  let value = s.parse_length()?;

  match ::core::primitive::bool::then_some(value.number.is_sign_negative(), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(pos) = s.calc_char_pos_at(start);
      return Err(FilterValueListParserError::NegativeValue(pos));
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(value.unit == LengthUnit::Percent, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(pos) = s.calc_char_pos_at(start);
      return Err(FilterValueListParserError::PercentageValue(pos));
    }
    ::core::option::Option::None => {}
  }

  Ok(value)
}

fn parse_filter_angle(s: &mut Stream<'_>) -> Result<Angle, FilterValueListParserError> {
  let Ok(()) = s.skip_spaces();
  let Ok(start) = s.pos();
  let value = s.parse_number()?;
  let Ok(unit) = s.take_angle_unit();
  let measure = match (unit, value.classify()) {
    (Some(unit_2), _) => unit_2,
    (None, core::num::FpCategory::Zero) => AngleUnit::Degrees,
    (
      None,
      core::num::FpCategory::Nan
      | core::num::FpCategory::Infinite
      | core::num::FpCategory::Normal
      | core::num::FpCategory::Subnormal,
    ) => {
      let Ok(position) = s.calc_char_pos_at(start);

      return Err(FilterValueListParserError::InvalidAngle(position));
    }
  };
  let Ok(angle) = Angle::new(value, measure);

  Ok(angle)
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use crate::Color;

    #[test]
    fn empty() {
        let mut parser = FilterValueListParser::from("");
        assert!(parser.next().is_none());
    }

    #[test]
    fn none() {
        let mut parser = FilterValueListParser::from("none");
        assert!(parser.next().is_none());
    }

    #[test]
    fn blur_default() {
        let mut parser = FilterValueListParser::from("blur()");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::Blur(Length { number: 0.0, unit: LengthUnit::None })));
        assert!(parser.next().is_none());
    }

    #[test]
    fn blur_2() {
        let mut parser = FilterValueListParser::from("blur(2)");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::Blur(Length { number: 2.0, unit: LengthUnit::None })));
        assert!(parser.next().is_none());
    }

    #[test]
    fn blur_2mm() {
        let mut parser = FilterValueListParser::from("blur(2mm)");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::Blur(Length { number: 2.0, unit: LengthUnit::Mm })));
        assert!(parser.next().is_none());
    }

    #[test]
    fn blur_2percent() {
        let mut parser = FilterValueListParser::from("blur(2%)");
        assert_eq!(parser.next().map(|item| item.map_err(|fault| fault.to_string())), Some(Err("a percentage value detected at position 6".to_string())));
        assert!(parser.next().is_none());
    }

    #[test]
    fn blur_negative() {
        let mut parser = FilterValueListParser::from("blur(-1)");
        assert_eq!(parser.next().map(|item| item.map_err(|fault| fault.to_string())), Some(Err("a negative value detected at position 6".to_string())));
        assert!(parser.next().is_none());
    }

    #[test]
    fn blur_two_values() {
        let mut parser = FilterValueListParser::from("blur(1 2)");
        assert_eq!(parser.next().map(|item| item.map_err(|fault| fault.to_string())), Some(Err("expected ')' not '2' at position 8".to_string())));
        assert!(parser.next().is_none());
    }

    #[test]
    fn brightness_default() {
        let mut parser = FilterValueListParser::from("brightness()");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::Brightness(1.0)));
        assert!(parser.next().is_none());
    }

    #[test]
    fn brightness_2() {
        let mut parser = FilterValueListParser::from("brightness(2)");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::Brightness(2.0)));
        assert!(parser.next().is_none());
    }

    #[test]
    fn brightness_50percent() {
        let mut parser = FilterValueListParser::from("brightness(50%)");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::Brightness(0.5)));
        assert!(parser.next().is_none());
    }

    #[test]
    fn brightness_negative() {
        let mut parser = FilterValueListParser::from("brightness(-1)");
        assert_eq!(parser.next().map(|item| item.map_err(|fault| fault.to_string())), Some(Err("a negative value detected at position 12".to_string())));
        assert!(parser.next().is_none());
    }

    #[test]
    fn brightness_2mm() {
        let mut parser = FilterValueListParser::from("brightness(2mm)");
        assert_eq!(parser.next().map(|item| item.map_err(|fault| fault.to_string())), Some(Err("expected ')' not 'm' at position 13".to_string())));
        assert!(parser.next().is_none());
    }

    #[test]
    fn drop_shadow_no_values() {
        let mut parser = FilterValueListParser::from("drop-shadow()");
        assert_eq!(parser.next().map(|item| item.map_err(|fault| fault.to_string())), Some(Err("drop-shadow offset values are expected at position 13".to_string())));
        assert!(parser.next().is_none());
    }

    #[test]
    fn drop_shadow_default() {
        let mut parser = FilterValueListParser::from("drop-shadow(2 3)");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::DropShadow {
            color: None,
            dx: ({ let Ok(explicit_value) = Length::new_number(2.0); explicit_value }),
            dy: ({ let Ok(explicit_value) = Length::new_number(3.0); explicit_value }),
            std_dev: Length { number: 0.0, unit: LengthUnit::None },
         }));
        assert!(parser.next().is_none());
    }

    #[test]
    fn drop_shadow_color_first() {
        let mut parser = FilterValueListParser::from("drop-shadow(red 2 3)");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::DropShadow {
            color: Some(Color { red: 255, green: 0, blue: 0, alpha: 255 }),
            dx: ({ let Ok(explicit_value) = Length::new_number(2.0); explicit_value }),
            dy: ({ let Ok(explicit_value) = Length::new_number(3.0); explicit_value }),
            std_dev: Length { number: 0.0, unit: LengthUnit::None },
         }));
        assert!(parser.next().is_none());
    }

    #[test]
    fn drop_shadow_color_after() {
        let mut parser = FilterValueListParser::from("drop-shadow(2 3 red)");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::DropShadow {
            color: Some(Color { red: 255, green: 0, blue: 0, alpha: 255 }),
            dx: ({ let Ok(explicit_value) = Length::new_number(2.0); explicit_value }),
            dy: ({ let Ok(explicit_value) = Length::new_number(3.0); explicit_value }),
            std_dev: Length { number: 0.0, unit: LengthUnit::None },
         }));
        assert!(parser.next().is_none());
    }

    #[test]
    fn drop_shadow_curr_color_first() {
        let mut parser = FilterValueListParser::from("drop-shadow(currentColor 2 3)");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::DropShadow {
            color: None,
            dx: ({ let Ok(explicit_value) = Length::new_number(2.0); explicit_value }),
            dy: ({ let Ok(explicit_value) = Length::new_number(3.0); explicit_value }),
            std_dev: Length { number: 0.0, unit: LengthUnit::None },
         }));
        assert!(parser.next().is_none());
    }

    #[test]
    fn drop_shadow_curr_color_after() {
        let mut parser = FilterValueListParser::from("drop-shadow(2 3 currentColor)");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::DropShadow {
            color: None,
            dx: ({ let Ok(explicit_value) = Length::new_number(2.0); explicit_value }),
            dy: ({ let Ok(explicit_value) = Length::new_number(3.0); explicit_value }),
            std_dev: Length { number: 0.0, unit: LengthUnit::None },
         }));
        assert!(parser.next().is_none());
    }

    #[test]
    fn drop_shadow_with_dev() {
        let mut parser = FilterValueListParser::from("drop-shadow(red 2 3 4)");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::DropShadow {
            color: Some(Color { red: 255, green: 0, blue: 0, alpha: 255 }),
            dx: ({ let Ok(explicit_value) = Length::new_number(2.0); explicit_value }),
            dy: ({ let Ok(explicit_value) = Length::new_number(3.0); explicit_value }),
            std_dev: ({ let Ok(explicit_value) = Length::new_number(4.0); explicit_value }),
         }));
        assert!(parser.next().is_none());
    }

    #[test]
    fn drop_shadow_color_twice() {
        let mut parser = FilterValueListParser::from("drop-shadow(red 2 3 4 red)");
        assert_eq!(parser.next().map(|item| item.map_err(|fault| fault.to_string())), Some(Err("expected ')' not 'r' at position 23".to_string())));
        assert!(parser.next().is_none());
    }

    #[test]
    fn drop_shadow_curr_color_twice() {
        let mut parser = FilterValueListParser::from("drop-shadow(currentColor 2 3 4 currentColor)");
        assert_eq!(parser.next().map(|item| item.map_err(|fault| fault.to_string())), Some(Err("expected ')' not 'c' at position 32".to_string())));
        assert!(parser.next().is_none());
    }

    #[test]
    fn drop_shadow_percent() {
        let mut parser = FilterValueListParser::from("drop-shadow(2% 3% 4%)");
        assert_eq!(parser.next().map(|item| item.map_err(|fault| fault.to_string())), Some(Err("a percentage value detected at position 13".to_string())));
        assert!(parser.next().is_none());
    }

    #[test]
    fn drop_shadow_negative_offset() {
        let mut parser = FilterValueListParser::from("drop-shadow(-1 -2 3)");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::DropShadow {
            color: None,
            dx: ({ let Ok(explicit_value) = Length::new_number(-1.0); explicit_value }),
            dy: ({ let Ok(explicit_value) = Length::new_number(-2.0); explicit_value }),
            std_dev: ({ let Ok(explicit_value) = Length::new_number(3.0); explicit_value }),
         }));
        assert!(parser.next().is_none());
    }

    #[test]
    fn hue_rotate_no_units() {
        let mut parser = FilterValueListParser::from("hue-rotate(45)");
        assert_eq!(parser.next().map(|item| item.map_err(|fault| fault.to_string())), Some(Err("an invalid angle at position 12".to_string())));
        assert!(parser.next().is_none());
    }

    #[test]
    fn url() {
        let mut parser = FilterValueListParser::from("url(#qwe)");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::Url("qwe")));
        assert!(parser.next().is_none());
    }

    #[test]
    fn multiple_1() {
        let mut parser = FilterValueListParser::from("blur() blur()");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::Blur(Length { number: 0.0, unit: LengthUnit::None })));
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::Blur(Length { number: 0.0, unit: LengthUnit::None })));
        assert!(parser.next().is_none());
    }

    #[test]
    fn multiple_2() {
        let mut parser = FilterValueListParser::from("blur() contrast(1)");
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::Blur(Length { number: 0.0, unit: LengthUnit::None })));
        assert_eq!(parser.next().and_then(Result::ok), Some(FilterValue::Contrast(1.0)));
        assert!(parser.next().is_none());
    }
}
