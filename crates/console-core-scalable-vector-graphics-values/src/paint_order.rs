use crate::stream::Stream;
use alloc::vec::Vec;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaintOrderKind {
  Fill,
  Stroke,
  Markers,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "paint orders are represented by typed paint kinds and the default is the SVG normal paint order"
  )
)]
pub struct PaintOrder {
  pub order: [PaintOrderKind; 3],
}

impl Default for PaintOrder {
  #[inline]
  fn default() -> Self {
    Self {
      order: [
        PaintOrderKind::Fill,
        PaintOrderKind::Stroke,
        PaintOrderKind::Markers,
      ],
    }
  }
}

impl From<[PaintOrderKind; 3]> for PaintOrder {
  #[inline]
  fn from(order: [PaintOrderKind; 3]) -> Self {
    Self { order }
  }
}

impl core::str::FromStr for PaintOrder {
  type Err = ();

  fn from_str(text: &str) -> Result<Self, Self::Err> {
    let mut order = Vec::new();

    let mut fill = true;
    let mut stroke = true;
    let mut markers = true;

    let mut s = Stream::from(text);

    for _ in 0..3 {
      let Ok(end) = s.at_end();

      match end {
        crate::End::Reached => break,
        crate::End::Pending => {}
      }

      let Ok(()) = s.skip_spaces();
      let Ok(name) = s.consume_ascii_ident();
      let Ok(()) = s.skip_spaces();
      let name_2 = match name {
        "normal" => return Ok(Self::default()),
        "fill" => PaintOrderKind::Fill,
        "stroke" => PaintOrderKind::Stroke,
        "markers" => PaintOrderKind::Markers,
        _ => return Ok(Self::default()),
      };

      let left = match name_2 {
        PaintOrderKind::Fill => &mut fill,
        PaintOrderKind::Stroke => &mut stroke,
        PaintOrderKind::Markers => &mut markers,
      };

      match ::core::primitive::bool::then_some(*left, ()) {
        ::core::option::Option::Some(()) => *left = false,
        ::core::option::Option::None => return Ok(Self::default()),
      }

      order.push(name_2);
    }

    let Ok(()) = s.skip_spaces();
    let Ok(at_end) = s.at_end();

    match ::core::primitive::bool::then_some(!matches!(at_end, crate::End::Reached), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(Self::default());
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(order.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(Self::default());
      }
      ::core::option::Option::None => {}
    }

    for (left, kind) in [
      (fill, PaintOrderKind::Fill),
      (stroke, PaintOrderKind::Stroke),
      (markers, PaintOrderKind::Markers),
    ] {
      match ::core::primitive::bool::then_some(left, ()) {
        ::core::option::Option::Some(()) => order.push(kind),
        ::core::option::Option::None => {}
      }
    }

    Ok(match order.as_slice() {
      [first, second, third] => Self {
        order: [*first, *second, *third],
      },
      _ => Self::default(),
    })
  }
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;
    use core::str::FromStr;

    #[test]
    fn parse_1() {
        assert_eq!(PaintOrder::from_str("normal"), Ok(PaintOrder::default()));
    }

    #[test]
    fn parse_2() {
        assert_eq!(PaintOrder::from_str("qwe"), Ok(PaintOrder::default()));
    }

    #[test]
    fn parse_3() {
        assert_eq!(PaintOrder::from_str(""), Ok(PaintOrder::default()));
    }

    #[test]
    fn parse_4() {
        assert_eq!(PaintOrder::from_str("stroke qwe"), Ok(PaintOrder::default()));
    }

    #[test]
    fn parse_5() {
        assert_eq!(PaintOrder::from_str("stroke stroke"), Ok(PaintOrder::default()));
    }

    #[test]
    fn parse_6() {
        assert_eq!(PaintOrder::from_str("stroke"), Ok(PaintOrder::from([
            PaintOrderKind::Stroke, PaintOrderKind::Fill, PaintOrderKind::Markers
        ])));
    }

    #[test]
    fn parse_7() {
        assert_eq!(PaintOrder::from_str("stroke markers"), Ok(PaintOrder::from([
            PaintOrderKind::Stroke, PaintOrderKind::Markers, PaintOrderKind::Fill
        ])));
    }

    #[test]
    fn parse_8() {
        assert_eq!(PaintOrder::from_str("stroke markers fill"), Ok(PaintOrder::from([
            PaintOrderKind::Stroke, PaintOrderKind::Markers, PaintOrderKind::Fill
        ])));
    }

    #[test]
    fn parse_9() {
        assert_eq!(PaintOrder::from_str("markers"), Ok(PaintOrder::from([
            PaintOrderKind::Markers, PaintOrderKind::Fill, PaintOrderKind::Stroke
        ])));
    }

    #[test]
    fn parse_10() {
        assert_eq!(PaintOrder::from_str("  stroke\n"), Ok(PaintOrder::from([
            PaintOrderKind::Stroke, PaintOrderKind::Fill, PaintOrderKind::Markers
        ])));
    }

    #[test]
    fn parse_11() {
        assert_eq!(PaintOrder::from_str("stroke stroke stroke stroke"), Ok(PaintOrder::default()));
    }
}
