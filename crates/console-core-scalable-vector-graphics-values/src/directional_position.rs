use crate::{Error, Length, LengthUnit, Stream};
use alloc::string::ToString;
use alloc::vec;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DirectionalPosition {
  Top,

  Center,

  Bottom,

  Right,

  Left,
}

impl DirectionalPosition {
  #[inline]
  pub fn is_horizontal(
    &self,
  ) -> core::result::Result<crate::IsHorizontalAnswer, console_core_never::Never> {
    Ok(match self {
      Self::Center | Self::Left | Self::Right => crate::IsHorizontalAnswer::Yes,
      Self::Top | Self::Bottom => crate::IsHorizontalAnswer::No,
    })
  }

  #[inline]
  pub fn is_vertical(
    &self,
  ) -> core::result::Result<crate::IsVerticalAnswer, console_core_never::Never> {
    Ok(match self {
      Self::Center | Self::Top | Self::Bottom => crate::IsVerticalAnswer::Yes,
      Self::Left | Self::Right => crate::IsVerticalAnswer::No,
    })
  }
}

impl From<DirectionalPosition> for Length {
  fn from(value: DirectionalPosition) -> Self {
    match value {
      DirectionalPosition::Left | DirectionalPosition::Top => {
        let Ok(length) = Self::new(0.0, LengthUnit::Percent);

        length
      }
      DirectionalPosition::Right | DirectionalPosition::Bottom => {
        let Ok(length) = Self::new(100.0, LengthUnit::Percent);

        length
      }
      DirectionalPosition::Center => {
        let Ok(length) = Self::new(50.0, LengthUnit::Percent);

        length
      }
    }
  }
}

impl core::str::FromStr for DirectionalPosition {
  type Err = Error;

  #[inline]
  fn from_str(text: &str) -> Result<Self, Error> {
    let mut s = Stream::from(text);
    let dir_pos = s.parse_directional_position()?;

    let Ok(at_end) = s.at_end();

    match ::core::primitive::bool::then_some(!matches!(at_end, crate::End::Reached), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(calc_char_pos) = s.calc_char_pos();
        return Err(Error::UnexpectedData(calc_char_pos));
      }
      ::core::option::Option::None => {}
    }

    Ok(dir_pos)
  }
}

impl Stream<'_> {
  pub fn parse_directional_position(&mut self) -> Result<DirectionalPosition, Error> {
    use DirectionalPosition::{Bottom, Center, Left, Right, Top};

    let Ok(()) = self.skip_spaces();
    let Ok(direction) = self.take_prefix(&[
      (b"left", Left),
      (b"right", Right),
      (b"top", Top),
      (b"bottom", Bottom),
      (b"center", Center),
    ]);

    match direction {
      Some(direction_2) => Ok(direction_2),
      None => {
        let Ok(tail) = self.slice_tail();
        let Ok(position) = self.calc_char_pos();

        Err(Error::InvalidString(
          vec![
            tail.to_string(),
            "left".to_string(),
            "right".to_string(),
            "top".to_string(),
            "bottom".to_string(),
            "center".to_string(),
          ],
          position,
        ))
      }
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
    macro_rules! test_p {
        ($name:ident, $text:expr, $result:expr) => (
            #[test]
            fn $name() {
                assert_eq!(DirectionalPosition::from_str($text), Ok($result));
            }
        )
    }

    test_p!(parse_1,  "left", DirectionalPosition::Left);
    test_p!(parse_2,  "right", DirectionalPosition::Right);
    test_p!(parse_3,  "center", DirectionalPosition::Center);
    test_p!(parse_4,  "top", DirectionalPosition::Top);
    test_p!(parse_5,  "bottom", DirectionalPosition::Bottom);

    #[test]
    fn parse_6() {
        let mut s = Stream::from("left,");
        assert_eq!(s.parse_directional_position(), Ok(DirectionalPosition::Left));
    }

    #[test]
    fn parse_7() {
        let mut s = Stream::from("left ,");
        assert_eq!(s.parse_directional_position(), Ok(DirectionalPosition::Left));
    }

    #[test]
    fn parse_16() {
        let mut s = Stream::from("left center");
        assert_eq!(s.parse_directional_position(), Ok(DirectionalPosition::Left));
    }

    #[test]
    fn err_1() {
        let mut s = Stream::from("something");
        assert_eq!(s.parse_directional_position().map_err(|fault| fault.to_string()), Err("expected 'left', 'right', 'top', 'bottom', 'center' not 'something' at position 1".to_string()));
    }
}
