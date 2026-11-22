use crate::Error;
use crate::stream::Stream;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Display;

pub fn parse_font_families(text: &str) -> Result<Vec<FontFamily>, Error> {
  let mut s = Stream::from(text);
  let font_families = s.parse_font_families()?;

  let Ok(()) = s.skip_spaces();
  let Ok(at_end) = s.at_end();

  match ::core::primitive::bool::then_some(!matches!(at_end, crate::End::Reached), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(calc_char_pos) = s.calc_char_pos();
      return Err(Error::UnexpectedData(calc_char_pos));
    }
    ::core::option::Option::None => {}
  }

  Ok(font_families)
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum FontFamily {
  Serif,

  SansSerif,

  Cursive,

  Fantasy,

  Monospace,

  Named(String),
}

impl Display for FontFamily {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    let str = match self {
      Self::Monospace => "monospace".to_string(),
      Self::Serif => "serif".to_string(),
      Self::SansSerif => "sans-serif".to_string(),
      Self::Cursive => "cursive".to_string(),
      Self::Fantasy => "fantasy".to_string(),
      Self::Named(s) => format!("\"{s}\""),
    };
    write!(f, "{str}")
  }
}

impl Stream<'_> {
  pub fn parse_font_families(&mut self) -> Result<Vec<FontFamily>, Error> {
    let mut families = Vec::new();
    let Ok(remaining) = self.remaining();

    for _ in 0..remaining {
      let Ok(end) = self.at_end();

      match end {
        crate::End::Reached => break,
        crate::End::Pending => {}
      }

      let Ok(()) = self.skip_spaces();
      let family = self.parse_font_family()?;

      let empty = match &family {
        FontFamily::Named(name) => name.is_empty(),
        FontFamily::Monospace
        | FontFamily::Serif
        | FontFamily::SansSerif
        | FontFamily::Cursive
        | FontFamily::Fantasy => false,
      };

      match empty.then_some(()) {
        Some(()) => {}
        None => families.push(family),
      }

      match self.curr_byte() {
        Ok(b',') => {
          let Ok(()) = self.advance(1);
        }
        Ok(_) => break,
        Err(_the_last_family_ended_at_eof) => break,
      }
    }

    Ok(families)
  }

  fn parse_font_family(&mut self) -> Result<FontFamily, Error> {
    let first = self.curr_byte()?;

    match first {
      b'\'' | b'"' => {
        let name = self.parse_quoted_string()?;

        Ok(FontFamily::Named(name.to_string()))
      }
      _ => self.parse_unquoted_family(),
    }
  }

  fn parse_unquoted_family(&mut self) -> Result<FontFamily, Error> {
    let mut parts = Vec::new();
    let Ok(remaining) = self.remaining();

    for _ in 0..remaining {
      match self.curr_byte() {
        Ok(b',') => break,
        Err(_the_name_ended_at_eof) => break,
        Ok(_) => {}
      }

      let part = self.parse_ident()?;
      parts.push(part);
      let Ok(()) = self.skip_spaces();
    }

    let name = parts.join(" ");

    Ok(match name.as_str() {
      "serif" => FontFamily::Serif,
      "sans-serif" => FontFamily::SansSerif,
      "cursive" => FontFamily::Cursive,
      "fantasy" => FontFamily::Fantasy,
      "monospace" => FontFamily::Monospace,
      _ => FontFamily::Named(name),
    })
  }
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct FontShorthand<'a> {
  pub font_style: Option<&'a str>,

  pub font_variant: Option<&'a str>,

  pub font_weight: Option<&'a str>,

  pub font_stretch: Option<&'a str>,

  pub font_size: &'a str,

  pub font_family: &'a str,
}

impl<'a> FontShorthand<'a> {
  #[expect(
    clippy::should_implement_trait,
    reason = "the value parsed borrows from the text it was parsed from, which FromStr has no lifetime to say"
  )]
  pub fn from_str(text: &'a str) -> Result<Self, Error> {
    let mut stream = Stream::from(text);
    let Ok(()) = stream.skip_spaces();

    let Ok(mut prev_pos) = stream.pos();

    let mut font_style = None;
    let mut font_variant = None;
    let mut font_weight = None;
    let mut font_stretch = None;

    for _ in 0..4 {
      let Ok(ident) = stream.consume_ascii_ident();

      match ident {
        "normal" => {}
        "small-caps" => font_variant = Some(ident),
        "italic" | "oblique" => font_style = Some(ident),
        "bold" | "bolder" | "lighter" | "100" | "200" | "300" | "400" | "500" | "600" | "700"
        | "800" | "900" => font_weight = Some(ident),
        "ultra-condensed" | "extra-condensed" | "condensed" | "semi-condensed"
        | "semi-expanded" | "expanded" | "extra-expanded" | "ultra-expanded" => {
          font_stretch = Some(ident);
        }
        _ => {
          stream = Stream::from(text);
          let Ok(()) = stream.advance(prev_pos);
          break;
        }
      }

      let Ok(()) = stream.skip_spaces();
      let Ok(pos) = stream.pos();
      prev_pos = pos;
    }

    let Ok(pos) = stream.pos();
    prev_pos = pos;
    let curr_byte = stream.curr_byte()?;

    match curr_byte {
      b'0'..=b'9' => {
        let _ = stream.parse_length()?;
      }
      _ => {
        let Ok(size) = stream.consume_ascii_ident();

        match ::core::primitive::bool::then_some(
          !matches!(
            size,
            "xx-small"
              | "x-small"
              | "small"
              | "medium"
              | "large"
              | "x-large"
              | "xx-large"
              | "larger"
              | "smaller"
          ),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            return Err(Error::UnexpectedData(prev_pos));
          }
          ::core::option::Option::None => {}
        }
      }
    }

    let Ok(font_size) = stream.slice_back(prev_pos);
    let Ok(()) = stream.skip_spaces();

    let stream_curr_byte = stream.curr_byte()?;

    match ::core::primitive::bool::then_some(stream_curr_byte == b'/', ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = stream.advance(1);
        let Ok(()) = stream.skip_spaces();
        let _ = stream.parse_length()?;
        let Ok(()) = stream.skip_spaces();
      }
      ::core::option::Option::None => {}
    }

    let Ok(at_end) = stream.at_end();

    match at_end {
      crate::End::Reached => {
        return Err(Error::UnexpectedEndOfStream);
      }
      crate::End::Pending => {}
    }

    let Ok(font_family) = stream.slice_tail();

    Ok(Self {
      font_style,
      font_variant,
      font_weight,
      font_stretch,
      font_size,
      font_family,
    })
  }
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "each compact upstream fixture generates a separately named test with its own input and expected SVG value"
      )
    )]
    macro_rules! font_family {
        ($name:ident, $text:expr, $result:expr) => (
            #[test]
            fn $name() {
                assert_eq!(parse_font_families($text), Ok($result));
            }
        )
    }

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "the upstream font fixtures spell a named font family inline in expected values"
      )
    )]
    macro_rules! named {
        ($text:expr) => (
            FontFamily::Named($text.to_string())
        )
    }

    const SERIF: FontFamily = FontFamily::Serif;
    const SANS_SERIF: FontFamily = FontFamily::SansSerif;
    const FANTASY: FontFamily = FontFamily::Fantasy;
    const MONOSPACE: FontFamily = FontFamily::Monospace;
    const CURSIVE: FontFamily = FontFamily::Cursive;

    font_family!(font_family_1, "Times New Roman", vec![named!("Times New Roman")]);
    font_family!(font_family_2, "serif", vec![SERIF]);
    font_family!(font_family_3, "sans-serif", vec![SANS_SERIF]);
    font_family!(font_family_4, "cursive", vec![CURSIVE]);
    font_family!(font_family_5, "fantasy", vec![FANTASY]);
    font_family!(font_family_6, "monospace", vec![MONOSPACE]);
    font_family!(font_family_7, "'Times New Roman'", vec![named!("Times New Roman")]);
    font_family!(font_family_8, "'Times New Roman', sans-serif", vec![named!("Times New Roman"), SANS_SERIF]);
    font_family!(font_family_9, "'Times New Roman', sans-serif", vec![named!("Times New Roman"), SANS_SERIF]);
    font_family!(font_family_10, "Arial, sans-serif, 'fantasy'", vec![named!("Arial"), SANS_SERIF, named!("fantasy")]);
    font_family!(font_family_11, "    Arial  , monospace  , 'fantasy'", vec![named!("Arial"), MONOSPACE, named!("fantasy")]);
    font_family!(font_family_12, "Times    New Roman", vec![named!("Times New Roman")]);
    font_family!(font_family_13, "\"Times New Roman\", sans-serif, sans-serif, \"Arial\"",
        vec![named!("Times New Roman"), SANS_SERIF, SANS_SERIF, named!("Arial")]
    );
    font_family!(font_family_14, "Times New Roman,,,Arial", vec![named!("Times New Roman"), named!("Arial")]);
    font_family!(font_family_15, "简体中文,sans-serif  , ,\"日本語フォント\",Arial",
        vec![named!("简体中文"), SANS_SERIF, named!("日本語フォント"), named!("Arial")]);

    font_family!(font_family_16, "", vec![]);

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "each compact upstream fixture generates a separately named test with its own input and expected SVG value"
      )
    )]
    macro_rules! font_family_err {
        ($name:ident, $text:expr, $result:expr) => (
            #[test]
            fn $name() {
                assert_eq!(parse_font_families($text).map_err(|fault| fault.to_string()), Err($result.to_string()));
            }
        )
    }
    font_family_err!(font_family_err_1, "Red/Black, sans-serif", "invalid ident");
    font_family_err!(font_family_err_2, "\"Lucida\" Grande, sans-serif", "unexpected data at position 10");
    font_family_err!(font_family_err_3, "Ahem!, sans-serif", "invalid ident");
    font_family_err!(font_family_err_4, "test@foo, sans-serif", "invalid ident");
    font_family_err!(font_family_err_5, "#POUND, sans-serif", "invalid ident");
    font_family_err!(font_family_err_6, "Hawaii 5-0, sans-serif", "invalid ident");

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "each compact upstream fixture generates a separately named test with its own input and expected SVG value"
      )
    )]
    macro_rules! font_shorthand {
        ($name:ident, $text:expr, $result:expr) => (
            #[test]
            fn $name() {
                assert_eq!(FontShorthand::from_str($text), Ok($result));
            }
        )
    }

    font_shorthand!(font_shorthand_1, "12pt/14pt sans-serif",
        FontShorthand { font_style: None, font_variant: None, font_weight: None, font_stretch: None, font_size: "12pt", font_family: "sans-serif" });
    font_shorthand!(font_shorthand_2, "80% sans-serif",
        FontShorthand { font_style: None, font_variant: None, font_weight: None, font_stretch: None, font_size: "80%", font_family: "sans-serif" });
    font_shorthand!(font_shorthand_3, "bold italic large Palatino, serif",
        FontShorthand { font_style: Some("italic"), font_variant: None, font_weight: Some("bold"), font_stretch: None, font_size: "large", font_family: "Palatino, serif" });
    font_shorthand!(font_shorthand_4, "x-large/110% \"new century schoolbook\", serif",
        FontShorthand { font_style: None, font_variant: None, font_weight: None, font_stretch: None, font_size: "x-large", font_family: "\"new century schoolbook\", serif" });
    font_shorthand!(font_shorthand_5, "normal small-caps 120%/120% fantasy",
        FontShorthand { font_style: None, font_variant: Some("small-caps"), font_weight: None, font_stretch: None, font_size: "120%", font_family: "fantasy" });
    font_shorthand!(font_shorthand_6, "condensed oblique 12pt \"Helvetica Neue\", serif",
        FontShorthand { font_style: Some("oblique"), font_variant: None, font_weight: None, font_stretch: Some("condensed"), font_size: "12pt", font_family: "\"Helvetica Neue\", serif" });
    font_shorthand!(font_shorthand_7, "italic 500 2em sans-serif, 'Noto Sans'",
        FontShorthand { font_style: Some("italic"), font_variant: None, font_weight: Some("500"), font_stretch: None, font_size: "2em", font_family: "sans-serif, 'Noto Sans'" });
    font_shorthand!(font_shorthand_8, "xx-large 'Noto Sans'",
        FontShorthand { font_style: None, font_variant: None, font_weight: None, font_stretch: None, font_size: "xx-large", font_family: "'Noto Sans'" });
    font_shorthand!(font_shorthand_9, "small-caps normal normal italic xx-small Times",
        FontShorthand { font_style: Some("italic"), font_variant: Some("small-caps"), font_weight: None, font_stretch: None, font_size: "xx-small", font_family: "Times" });

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "each compact upstream fixture generates a separately named test with its own input and expected SVG value"
      )
    )]
    macro_rules! font_shorthand_err {
        ($name:ident, $text:expr, $result:expr) => (
            #[test]
            fn $name() {
                assert_eq!(FontShorthand::from_str($text), Err($result));
            }
        )
    }

    font_shorthand_err!(font_shorthand_err_1, "", Error::UnexpectedEndOfStream);
    font_shorthand_err!(font_shorthand_err_2, "Noto Sans", Error::UnexpectedData(0));
    font_shorthand_err!(font_shorthand_err_3, "12pt  ", Error::UnexpectedEndOfStream);
    font_shorthand_err!(font_shorthand_err_4, "something 12pt 'Noto Sans'", Error::UnexpectedData(0));
    font_shorthand_err!(font_shorthand_err_5, "'Noto Sans' 13pt", Error::UnexpectedData(0));
    font_shorthand_err!(font_shorthand_err_6,
        "small-caps normal normal normal italic xx-large Times", Error::UnexpectedData(32));
}
