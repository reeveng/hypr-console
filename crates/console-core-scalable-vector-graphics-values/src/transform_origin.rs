use crate::directional_position::DirectionalPosition;
use crate::stream::Stream;
use crate::{Length, LengthUnit};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Position {
  Length(Length),
  DirectionalPosition(DirectionalPosition),
}

impl Position {
  fn is_vertical(
    &self,
  ) -> core::result::Result<crate::IsVerticalAnswer, console_core_never::Never> {
    let decided = {
      match self {
        Self::Length(_) => true,
        Self::DirectionalPosition(dp) => {
          let Ok(is_vertical) = dp.is_vertical();

          matches!(is_vertical, crate::IsVerticalAnswer::Yes)
        }
      }
    };

    Ok(match ::core::primitive::bool::then_some(decided, ()) {
      ::core::option::Option::Some(()) => crate::IsVerticalAnswer::Yes,
      ::core::option::Option::None => crate::IsVerticalAnswer::No,
    })
  }

  fn is_horizontal(
    &self,
  ) -> core::result::Result<crate::IsHorizontalAnswer, console_core_never::Never> {
    let decided = {
      match self {
        Self::Length(_) => true,
        Self::DirectionalPosition(dp) => {
          let Ok(is_horizontal) = dp.is_horizontal();

          matches!(is_horizontal, crate::IsHorizontalAnswer::Yes)
        }
      }
    };

    Ok(match ::core::primitive::bool::then_some(decided, ()) {
      ::core::option::Option::Some(()) => crate::IsHorizontalAnswer::Yes,
      ::core::option::Option::None => crate::IsHorizontalAnswer::No,
    })
  }
}

impl From<Position> for Length {
  fn from(value: Position) -> Self {
    match value {
      Position::Length(l) => l,
      Position::DirectionalPosition(dp) => <crate::length::Length as ::core::convert::From<
        crate::directional_position::DirectionalPosition,
      >>::from(dp),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "the public axes are typed lengths and parsing can refuse malformed origin syntax"
  )
)]
pub struct TransformOrigin {
  pub x_offset: Length,

  pub y_offset: Length,

  pub z_offset: Length,
}

impl TransformOrigin {
  #[inline]
  pub fn new(
    x_offset: Length,
    y_offset: Length,
    z_offset: Length,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      x_offset,
      y_offset,
      z_offset,
    })
  }
}

#[derive(Clone, Copy, Debug)]
pub enum TransformOriginError {
  MissingParameters,

  InvalidParameters,

  ZIndexIsPercentage,
}

impl core::fmt::Display for TransformOriginError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match *self {
      Self::MissingParameters => {
        write!(f, "transform origin doesn't have enough parameters")
      }
      Self::InvalidParameters => {
        write!(f, "transform origin has invalid parameters")
      }
      Self::ZIndexIsPercentage => {
        write!(f, "z-index cannot be a percentage")
      }
    }
  }
}

impl core::error::Error for TransformOriginError {
  fn description(&self) -> &str {
    "a transform origin parsing error"
  }
}

impl core::str::FromStr for TransformOrigin {
  type Err = TransformOriginError;

  fn from_str(text: &str) -> Result<Self, TransformOriginError> {
    let mut stream = Stream::from(text);
    let Ok(initial) = stream.at_end();

    match initial {
      crate::End::Reached => return Err(TransformOriginError::MissingParameters),
      crate::End::Pending => {}
    }

    let first = parse_part(&mut stream)?;
    let Ok(next) = stream.at_end();

    match next {
      crate::End::Reached => single(first),
      crate::End::Pending => {
        let Ok(()) = stream.skip_spaces();
        let Ok(()) = stream.parse_list_separator();
        let second = parse_part(&mut stream)?;
        let Ok(tail) = stream.at_end();
        let depth = match tail {
          crate::End::Reached => None,
          crate::End::Pending => {
            let Ok(()) = stream.skip_spaces();
            let Ok(()) = stream.parse_list_separator();
            let length = stream
              .parse_length()
              .map_err(|_invalid_depth| TransformOriginError::InvalidParameters)?;

            Some(length)
          }
        };
        let Ok(()) = stream.skip_spaces();
        stream
          .require_end()
          .map_err(|_trailing_data| TransformOriginError::InvalidParameters)?;

        pair(first, second, depth)
      }
    }
  }
}

fn parse_part(stream: &mut Stream<'_>) -> Result<Position, TransformOriginError> {
  match stream.parse_directional_position() {
    Ok(direction) => Ok(Position::DirectionalPosition(direction)),
    Err(_no_direction_at_this_position) => stream
      .parse_length()
      .map(Position::Length)
      .map_err(|_not_a_position| TransformOriginError::InvalidParameters),
  }
}

fn single(only: Position) -> Result<TransformOrigin, TransformOriginError> {
  let Ok(horizontal) = only.is_horizontal();
  let middle = Length::from(DirectionalPosition::Center);
  let length = Length::from(only);
  let (x, y) = match horizontal {
    crate::IsHorizontalAnswer::Yes => (length, middle),
    crate::IsHorizontalAnswer::No => (middle, length),
  };
  let Ok(depth) = Length::new(0.0, LengthUnit::Px);
  let Ok(origin) = TransformOrigin::new(x, y, depth);

  Ok(origin)
}

fn pair(
  first: Position,
  second: Position,
  depth: Option<Length>,
) -> Result<TransformOrigin, TransformOriginError> {
  let z = match depth {
    Some(depth_2) => match (depth_2.unit == LengthUnit::Percent).then_some(()) {
      Some(()) => return Err(TransformOriginError::ZIndexIsPercentage),
      None => depth_2,
    },
    None => {
      let Ok(default) = Length::new(0.0, LengthUnit::Px);

      default
    }
  };
  let center_or_number = |position| {
    matches!(
      position,
      Position::Length(_) | Position::DirectionalPosition(DirectionalPosition::Center)
    )
  };
  let Ok(horizontal) = first.is_horizontal();
  let Ok(vertical) = second.is_vertical();
  let given_order = (center_or_number(first) && center_or_number(second))
    || (horizontal == crate::IsHorizontalAnswer::Yes && vertical == crate::IsVerticalAnswer::Yes);

  let (x, y) = match given_order.then_some(()) {
    Some(()) => (first, second),
    None => {
      let Ok(first_vertical) = first.is_vertical();
      let Ok(second_horizontal) = second.is_horizontal();

      match (first_vertical, second_horizontal) {
        (crate::IsVerticalAnswer::Yes, crate::IsHorizontalAnswer::Yes) => (second, first),
        (crate::IsVerticalAnswer::No, _) | (_, crate::IsHorizontalAnswer::No) => {
          return Err(TransformOriginError::InvalidParameters);
        }
      }
    }
  };
  let Ok(origin) = TransformOrigin::new(Length::from(x), Length::from(y), z);

  Ok(origin)
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use core::str::FromStr;

    #[test]
    fn malformed_first_components_return_a_typed_error() {
        for text in ["not-an-origin", " ", "#"] {
            assert!(matches!(TransformOrigin::from_str(text), Err(TransformOriginError::InvalidParameters)));
        }
    }

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
            fn $name() -> Result<(), TransformOriginError> {
                let parsed = TransformOrigin::from_str($text)?;
                assert_eq!(parsed, $result);

                Ok(())
            }
        )
    }

    test!(parse_1, "center", TransformOrigin { x_offset: Length { number: 50.0, unit: LengthUnit::Percent }, y_offset: Length { number: 50.0, unit: LengthUnit::Percent }, z_offset: Length { number: 0.0, unit: LengthUnit::Px } });
    test!(parse_2, "left", TransformOrigin { x_offset: Length { number: 0.0, unit: LengthUnit::Percent }, y_offset: Length { number: 50.0, unit: LengthUnit::Percent }, z_offset: Length { number: 0.0, unit: LengthUnit::Px } });
    test!(parse_3, "right", TransformOrigin { x_offset: Length { number: 100.0, unit: LengthUnit::Percent }, y_offset: Length { number: 50.0, unit: LengthUnit::Percent }, z_offset: Length { number: 0.0, unit: LengthUnit::Px } });
    test!(parse_4, "top", TransformOrigin { x_offset: Length { number: 50.0, unit: LengthUnit::Percent }, y_offset: Length { number: 0.0, unit: LengthUnit::Percent }, z_offset: Length { number: 0.0, unit: LengthUnit::Px } });
    test!(parse_5, "bottom", TransformOrigin { x_offset: Length { number: 50.0, unit: LengthUnit::Percent }, y_offset: Length { number: 100.0, unit: LengthUnit::Percent }, z_offset: Length { number: 0.0, unit: LengthUnit::Px } });
    test!(parse_6, "30px", TransformOrigin { x_offset: Length { number: 30.0, unit: LengthUnit::Px }, y_offset: Length { number: 50.0, unit: LengthUnit::Percent }, z_offset: Length { number: 0.0, unit: LengthUnit::Px } });

    test!(parse_7, "center left", TransformOrigin { x_offset: Length { number: 0.0, unit: LengthUnit::Percent }, y_offset: Length { number: 50.0, unit: LengthUnit::Percent }, z_offset: Length { number: 0.0, unit: LengthUnit::Px } });
    test!(parse_8, "left center", TransformOrigin { x_offset: Length { number: 0.0, unit: LengthUnit::Percent }, y_offset: Length { number: 50.0, unit: LengthUnit::Percent }, z_offset: Length { number: 0.0, unit: LengthUnit::Px } });
    test!(parse_9, "center bottom", TransformOrigin { x_offset: Length { number: 50.0, unit: LengthUnit::Percent }, y_offset: Length { number: 100.0, unit: LengthUnit::Percent }, z_offset: Length { number: 0.0, unit: LengthUnit::Px } });
    test!(parse_10, "bottom center", TransformOrigin { x_offset: Length { number: 50.0, unit: LengthUnit::Percent }, y_offset: Length { number: 100.0, unit: LengthUnit::Percent }, z_offset: Length { number: 0.0, unit: LengthUnit::Px } });
    test!(parse_11, "30%, center", TransformOrigin { x_offset: Length { number: 30.0, unit: LengthUnit::Percent }, y_offset: Length { number: 50.0, unit: LengthUnit::Percent }, z_offset: Length { number: 0.0, unit: LengthUnit::Px } });
    test!(parse_12, " center, 30%", TransformOrigin { x_offset: Length { number: 50.0, unit: LengthUnit::Percent }, y_offset: Length { number: 30.0, unit: LengthUnit::Percent }, z_offset: Length { number: 0.0, unit: LengthUnit::Px } });
    test!(parse_13, "left top", TransformOrigin { x_offset: Length { number: 0.0, unit: LengthUnit::Percent }, y_offset: Length { number: 0.0, unit: LengthUnit::Percent }, z_offset: Length { number: 0.0, unit: LengthUnit::Px } });

    test!(parse_14, "center right 3px", TransformOrigin { x_offset: Length { number: 100.0, unit: LengthUnit::Percent }, y_offset: Length { number: 50.0, unit: LengthUnit::Percent }, z_offset: Length { number: 3.0, unit: LengthUnit::Px } });

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
                assert_eq!(TransformOrigin::from_str($text).map_err(|fault| fault.to_string()), Err($result.to_string()));
            }
        )
    }

    test_err!(parse_err_1, "", "transform origin doesn't have enough parameters");
    test_err!(parse_err_2, "some", "transform origin has invalid parameters");
    test_err!(parse_err_3, "center some", "transform origin has invalid parameters");
    test_err!(parse_err_4, "left right", "transform origin has invalid parameters");
    test_err!(parse_err_5, "left top 3%", "z-index cannot be a percentage");
}
