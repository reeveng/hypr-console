use crate::{Error, Stream};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IRI<'a>(pub &'a str);

impl<'a> IRI<'a> {
  #[expect(
    clippy::should_implement_trait,
    reason = "the value parsed borrows from the text it was parsed from, which FromStr has no lifetime to say"
  )]
  pub fn from_str(text: &'a str) -> Result<Self, Error> {
    let mut s = Stream::from(text);
    let link = s.parse_iri()?;
    let Ok(()) = s.skip_spaces();
    let Ok(at_end) = s.at_end();

    match ::core::primitive::bool::then_some(!matches!(at_end, crate::End::Reached), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(calc_char_pos) = s.calc_char_pos();
        return Err(Error::UnexpectedData(calc_char_pos));
      }
      ::core::option::Option::None => {}
    }

    Ok(Self(link))
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FuncIRI<'a>(pub &'a str);

impl<'a> FuncIRI<'a> {
  #[expect(
    clippy::should_implement_trait,
    reason = "the value parsed borrows from the text it was parsed from, which FromStr has no lifetime to say"
  )]
  pub fn from_str(text: &'a str) -> Result<Self, Error> {
    let mut s = Stream::from(text);
    let link = s.parse_func_iri()?;
    let Ok(()) = s.skip_spaces();
    let Ok(at_end) = s.at_end();

    match ::core::primitive::bool::then_some(!matches!(at_end, crate::End::Reached), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(calc_char_pos) = s.calc_char_pos();
        return Err(Error::UnexpectedData(calc_char_pos));
      }
      ::core::option::Option::None => {}
    }

    Ok(Self(link))
  }
}

impl<'a> Stream<'a> {
  pub fn parse_iri(&mut self) -> Result<&'a str, Error> {
    let Ok(()) = self.skip_spaces();
    self.consume_byte(b'#')?;
    let Ok(link) = self.consume_bytes(|_, c| c != b' ');

    match ::core::primitive::bool::then_some(link.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        return Err(Error::InvalidValue);
      }
      ::core::option::Option::None => {}
    }

    Ok(link)
  }

  pub fn parse_func_iri(&mut self) -> Result<&'a str, Error> {
    let Ok(()) = self.skip_spaces();
    self.consume_string(b"url(")?;
    let Ok(()) = self.skip_spaces();

    let opening = self.curr_byte()?;
    let quote = match opening {
      quoted @ (b'\'' | b'"') => Some(quoted),
      _ => None,
    };

    match quote {
      ::core::option::Option::Some(_) => {
        let Ok(()) = self.advance(1);
        let Ok(()) = self.skip_spaces();
      }
      ::core::option::Option::None => {}
    }

    self.consume_byte(b'#')?;
    let link = match quote {
      Some(quote_2) => {
        let Ok(consume_bytes) = self.consume_bytes(|_, c| c != quote_2);

        consume_bytes.trim_end()
      }
      None => {
        let Ok(consume_bytes) = self.consume_bytes(|_, c| c != b' ' && c != b')');

        consume_bytes
      }
    };

    match ::core::primitive::bool::then_some(link.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        return Err(Error::InvalidValue);
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(link.contains('\'') || link.contains('"'), ()) {
      ::core::option::Option::Some(()) => {
        return Err(Error::InvalidValue);
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = self.skip_spaces();

    match quote {
      Some(quote_3) => {
        self.consume_byte(quote_3)?;
        let Ok(()) = self.skip_spaces();
      }
      None => {}
    }

    self.consume_byte(b')')?;
    Ok(link)
  }
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn parse_iri_1() {
        assert_eq!(IRI::from_str("#id"), Ok(IRI("id")));
    }

    #[test]
    fn parse_iri_2() {
        assert_eq!(IRI::from_str("   #id   "), Ok(IRI("id")));
    }

    #[test]
    fn parse_iri_3() {

        assert_eq!(Stream::from("   #id   text").parse_iri(), Ok("id"));
        assert_eq!(IRI::from_str("   #id   text").map_err(|fault| fault.to_string()), Err("unexpected data at position 10".to_string()));
    }

    #[test]
    fn parse_iri_4() {
        assert_eq!(IRI::from_str("#1"), Ok(IRI("1")));
    }

    #[test]
    fn parse_err_iri_1() {
        assert_eq!(IRI::from_str("# id").map_err(|fault| fault.to_string()), Err("invalid value".to_string()));
    }

    #[test]
    fn parse_func_iri_1() {
        assert_eq!(FuncIRI::from_str("url(#id)"), Ok(FuncIRI("id")));
    }

    #[test]
    fn parse_func_iri_2() {
        assert_eq!(FuncIRI::from_str("url(#1)"), Ok(FuncIRI("1")));
    }

    #[test]
    fn parse_func_iri_3() {
        assert_eq!(FuncIRI::from_str("    url(    #id    )   "), Ok(FuncIRI("id")));
    }

    #[test]
    fn parse_func_iri_4() {

        assert_eq!(Stream::from("url(#id) qwe").parse_func_iri(), Ok("id"));
        assert_eq!(FuncIRI::from_str("url(#id) qwe").map_err(|fault| fault.to_string()), Err("unexpected data at position 10".to_string()));
    }

    #[test]
    fn parse_func_iri_5() {
        assert_eq!(FuncIRI::from_str("url('#id')"), Ok(FuncIRI("id")));
        assert_eq!(FuncIRI::from_str("url(' #id ')"), Ok(FuncIRI("id")));
    }

    #[test]
    fn parse_func_iri_6() {
        assert_eq!(FuncIRI::from_str("url(\"#id\")"), Ok(FuncIRI("id")));
        assert_eq!(FuncIRI::from_str("url(\" #id \")"), Ok(FuncIRI("id")));
    }

    #[test]
    fn parse_err_func_iri_1() {
        assert_eq!(FuncIRI::from_str("url ( #1 )").map_err(|fault| fault.to_string()), Err("expected 'url(' not 'url ' at position 1".to_string()));
    }

    #[test]
    fn parse_err_func_iri_2() {
        assert_eq!(FuncIRI::from_str("url(#)").map_err(|fault| fault.to_string()), Err("invalid value".to_string()));
    }

    #[test]
    fn parse_err_func_iri_3() {
        assert_eq!(FuncIRI::from_str("url(# id)").map_err(|fault| fault.to_string()), Err("invalid value".to_string()));
    }

    #[test]
    fn parse_err_func_iri_4() {

        assert_eq!(FuncIRI::from_str("url('#id)").map_err(|fault| fault.to_string()), Err("unexpected end of stream".to_string()));
        assert_eq!(FuncIRI::from_str("url(#id')").map_err(|fault| fault.to_string()), Err("invalid value".to_string()));
    }
}
