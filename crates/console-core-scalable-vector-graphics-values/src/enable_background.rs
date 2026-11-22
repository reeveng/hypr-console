use crate::{Error, Stream};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum EnableBackground {
  Accumulate,
  New,
  NewWithRegion {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
  },
}

impl core::str::FromStr for EnableBackground {
  type Err = Error;

  fn from_str(text: &str) -> Result<Self, Self::Err> {
    let mut stream = Stream::from(text);
    let Ok(()) = stream.skip_spaces();
    let Ok(accumulated) = stream.take_prefix(&[(b"accumulate", ())]);

    match accumulated {
      Some(()) => {
        let Ok(()) = stream.skip_spaces();
        stream.require_end()?;

        return Ok(Self::Accumulate);
      }
      None => {}
    }

    stream
      .consume_string(b"new")
      .map_err(|_no_background_keyword| Error::InvalidValue)?;
    let Ok(()) = stream.skip_spaces();
    let Ok(end) = stream.at_end();

    match end {
      crate::End::Reached => return Ok(Self::New),
      crate::End::Pending => {}
    }

    let x = stream.parse_list_number()?;
    let y = stream.parse_list_number()?;
    let width = stream.parse_list_number()?;
    let height = stream.parse_list_number()?;
    let Ok(()) = stream.skip_spaces();
    stream.require_end()?;

    match (width > 0.0 && height > 0.0).then_some(()) {
      Some(()) => Ok(Self::NewWithRegion {
        x,
        y,
        width,
        height,
      }),
      None => Err(Error::InvalidValue),
    }
  }
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use core::str::FromStr;

    #[test]
    fn parse_1() {
        assert_eq!(EnableBackground::from_str("accumulate"), Ok(EnableBackground::Accumulate));
    }

    #[test]
    fn parse_2() {
        assert_eq!(EnableBackground::from_str("  accumulate  "), Ok(EnableBackground::Accumulate));
    }

    #[test]
    fn parse_3() {
        assert_eq!(EnableBackground::from_str("new"), Ok(EnableBackground::New));
    }

    #[test]
    fn parse_4() {
        assert_eq!(EnableBackground::from_str("  new  "), Ok(EnableBackground::New));
    }

    #[test]
    fn parse_5() {
        assert_eq!(EnableBackground::from_str("new 1 2 3 4"), Ok(EnableBackground::NewWithRegion { x: 1.0, y: 2.0, width: 3.0, height: 4.0 }));
    }

    #[test]
    fn err_1() {
        assert_eq!(EnableBackground::from_str(" accumulate b ").map_err(|fault| fault.to_string()), Err("unexpected data at position 13".to_string()));
    }

    #[test]
    fn err_2() {
        assert_eq!(EnableBackground::from_str(" new b ").map_err(|fault| fault.to_string()), Err("invalid number at position 6".to_string()));
    }

    #[test]
    fn err_3() {
        assert_eq!(EnableBackground::from_str("new 1 2 3").map_err(|fault| fault.to_string()), Err("unexpected end of stream".to_string()));
    }

    #[test]
    fn err_4() {
        assert_eq!(EnableBackground::from_str("new 1 2 3 4 5").map_err(|fault| fault.to_string()), Err("unexpected data at position 13".to_string()));
    }

    #[test]
    fn err_5() {
        assert_eq!(EnableBackground::from_str("new 0 0 0 0").map_err(|fault| fault.to_string()), Err("invalid value".to_string()));
    }
}
