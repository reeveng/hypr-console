//! MIME essences and parameters, parsed using the MIME Sniffing Standard's
//! token and quoted-value rules. Invalid parameters are skipped after their
//! complete quoted value has been consumed.

use alloc::{borrow::ToOwned, collections::BTreeSet, string::String, vec::Vec};
use console_core_never::Never;
use core::fmt::{self, Write};
use core::str::{FromStr, Split};

#[derive(Clone, Copy)]
struct Budget<'a>(&'a str);

const WHITESPACE: [char; 4] = [' ', '\t', '\n', '\r'];

enum ParameterContent {
  Valid,
  Invalid,
}

impl From<bool> for ParameterContent {
  fn from(matches_value_grammar: bool) -> Self {
    match bool::then_some(matches_value_grammar, ()) {
      Some(()) => Self::Valid,
      None => Self::Invalid,
    }
  }
}

fn parameter_content(text: &str) -> Result<ParameterContent, Never> {
  Ok(ParameterContent::from(text.chars().all(
    |ch| matches!(ch, '\t' | ' '..='~' | '\u{80}'..='\u{FF}'),
  )))
}

fn http_token(text: &str) -> Result<&str, MimeParsingError> {
  match text {
    "" => Err(MimeParsingError(())),
    token => match token.bytes().find(|byte| {
      !matches!(
        byte,
        b'!' | b'#'..=b'\'' | b'*' | b'+' | b'-' | b'.' | b'0'..=b'9' |
        b'A'..=b'Z' | b'^'..=b'z' | b'|' | b'~'
      )
    }) {
      Some(_invalid_byte) => Err(MimeParsingError(())),
      None => Ok(token),
    },
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mime {
  type_: String,
  subtype: String,
  parameters: Vec<(String, String)>,
}

impl Mime {
  pub(super) fn default_for_data_url() -> Result<Self, Never> {
    Ok(Self {
      type_: String::from("text"),
      subtype: String::from("plain"),
      parameters: alloc::vec![(String::from("charset"), String::from("US-ASCII"))],
    })
  }

  pub fn essence(&self) -> Result<(&str, &str), Never> {
    Ok((&self.type_, &self.subtype))
  }

  pub fn parameters(&self) -> Result<&[(String, String)], Never> {
    Ok(&self.parameters)
  }

  pub fn get_parameter<P>(&self, name: &P) -> Result<Option<&str>, Never>
  where
    P: ?Sized + PartialEq<str>,
  {
    Ok(
      self
        .parameters
        .iter()
        .find(|(parameter, _value)| name == parameter.as_str())
        .map(|(_name, value)| value.as_str()),
    )
  }
}

#[derive(Debug)]
pub struct MimeParsingError(());

impl fmt::Display for MimeParsingError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(to, "invalid mime type")
  }
}

#[cfg(feature = "std")]
impl std::error::Error for MimeParsingError {}

impl FromStr for Mime {
  type Err = MimeParsingError;

  fn from_str(text: &str) -> Result<Self, Self::Err> {
    let trimmed = text.trim_matches(WHITESPACE);
    let (type_, remainder) = trimmed.split_once('/').ok_or(MimeParsingError(()))?;
    let (raw_subtype, parameters_text) = match remainder.split_once(';') {
      Some((subtype, parameters)) => (subtype, Some(parameters)),
      None => (remainder, None),
    };
    let subtype = raw_subtype.trim_end_matches(WHITESPACE);

    let type_token = http_token(type_)?;
    let subtype_token = http_token(subtype)?;

    let mut parameters = Vec::new();

    match parameters_text {
      Some(source) => {
        let Ok(()) = parse_parameters(source, &mut parameters);
      }
      None => {}
    }

    Ok(Mime {
      type_: type_token.to_ascii_lowercase(),
      subtype: subtype_token.to_ascii_lowercase(),
      parameters,
    })
  }
}

fn parse_parameters(text: &str, parameters: &mut Vec<(String, String)>) -> Result<(), Never> {
  let mut pieces = text.split(';');
  let mut names = BTreeSet::new();

  for _budget in text.bytes().chain(core::iter::once(0)) {
    let piece = match pieces.next() {
      Some(piece) => piece.trim_start_matches(WHITESPACE),
      None => return Ok(()),
    };
    let (name, value) = match piece.split_once('=') {
      Some(split) => split,
      None => continue,
    };
    let Ok(value_option) = parse_value(value, &mut pieces, Budget(text));
    let decoded = match value_option {
      Some(parsed) => parsed,
      None => continue,
    };

    let token = match http_token(name) {
      Ok(token) => token,
      Err(_invalid_token) => continue,
    };

    match names.get(token) {
      Some(_first) => continue,
      None => {
        let lower = token.to_ascii_lowercase();
        let _new = names.insert(lower.clone());

        parameters.push((lower, decoded));
      }
    }
  }

  Ok(())
}

fn parse_value<'a>(
  value: &'a str,
  pieces: &mut Split<'a, char>,
  budget: Budget<'_>,
) -> Result<Option<String>, Never> {
  match value.strip_prefix('"') {
    Some(quoted) => {
      let Ok(unescaped) = quoted_value(quoted, pieces, budget);
      let Ok(content) = parameter_content(value);

      Ok(match content {
        ParameterContent::Valid => Some(unescaped),
        ParameterContent::Invalid => None,
      })
    }
    None => {
      let trimmed = value.trim_end_matches(WHITESPACE);
      let Ok(content) = parameter_content(trimmed);

      Ok(match (trimmed, content) {
        ("", _) | (_, ParameterContent::Invalid) => None,
        (nonempty, ParameterContent::Valid) => Some(nonempty.to_owned()),
      })
    }
  }
}

fn quoted_value<'a>(
  quoted: &'a str,
  pieces: &mut Split<'a, char>,
  budget: Budget<'_>,
) -> Result<String, Never> {
  let mut chars = quoted.chars();
  let mut decoded = String::new();

  for _budget in budget.0.bytes().chain(core::iter::once(0)) {
    match chars.next() {
      Some('"') => return Ok(decoded),
      Some('\\') => match chars.next() {
        Some(escaped) => decoded.push(escaped),
        None => match pieces.next() {
          Some(piece) => {
            chars = piece.chars();
            decoded.push(';');
          }
          None => decoded.push('\\'),
        },
      },
      Some(other) => decoded.push(other),
      None => match pieces.next() {
        Some(piece) => {
          chars = piece.chars();
          decoded.push(';');
        }
        None => return Ok(decoded),
      },
    }
  }

  Ok(decoded)
}

impl fmt::Display for Mime {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(to, "{}/{}", self.type_, self.subtype)?;

    for (name, value) in &self.parameters {
      write!(to, ";{name}=")?;

      write_value(value, to)?;
    }

    Ok(())
  }
}

fn write_value(value: &str, to: &mut fmt::Formatter<'_>) -> fmt::Result {
  match http_token(value) {
    Ok(token) => to.write_str(token)?,
    Err(_invalid_token) => {
      to.write_str("\"")?;

      for ch in value.chars() {
        match ch {
          '"' | '\\' => to.write_str("\\")?,
          _ordinary => {}
        }

        to.write_char(ch)?;
      }

      to.write_str("\"")?;
    }
  }

  Ok(())
}
