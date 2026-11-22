use crate::Stream;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PointsParser<'a>(Stream<'a>);

impl<'a> From<&'a str> for PointsParser<'a> {
  #[inline]
  fn from(v: &'a str) -> Self {
    PointsParser(Stream::from(v))
  }
}

impl Iterator for PointsParser<'_> {
  type Item = (f64, f64);

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(at_end) = self.0.at_end();

    match at_end {
      crate::End::Reached => None,
      crate::End::Pending => {
        let x = match self.0.parse_list_number() {
          Ok(x) => x,
          Err(_coordinate_not_available) => return None,
        };

        let y = match self.0.parse_list_number() {
          Ok(y) => y,
          Err(_coordinate_not_available) => return None,
        };

        Some((x, y))
      }
    }
  }
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_1() {
        let mut parser = PointsParser::from("10 20 30 40");
        assert_eq!(parser.next(), Some((10.0, 20.0)));
        assert_eq!(parser.next(), Some((30.0, 40.0)));
        assert!(parser.next().is_none());
    }

    #[test]
    fn parse_2() {
        let mut parser = PointsParser::from("10 20 30 40 50");
        assert_eq!(parser.next(), Some((10.0, 20.0)));
        assert_eq!(parser.next(), Some((30.0, 40.0)));
        assert!(parser.next().is_none());
    }
}
