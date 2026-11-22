//! A tolerant tokenizer for the tags and text of XML and XHTML.
//!
//! A book is XML: the file that says where the rest is, the list of its
//! chapters, and every chapter itself. What a reader wants out of any of them
//! is small -- which element a run of words is inside, and the one or two
//! attributes that name another file -- and a parser that builds a tree, checks
//! a namespace and refuses a page for an unclosed `<br>` is refusing books that
//! every other reader opens. A chapter written by a person with a text editor
//! is the ordinary case, not the damaged one.
//!
//! So this reads the markup as a stream of tokens: a tag opening, a tag
//! closing, and the words between them with the five entities and the numbered
//! ones turned back into letters. A namespace prefix is dropped from a name,
//! because `dc:title` and `title` are the same question to a reader. Comments,
//! declarations and processing instructions are stepped over. What it cannot
//! read -- a `>` inside a quoted attribute, which nobody writes -- comes out as
//! words rather than as a failure, and a page with a stray tag in it is a page
//! with a stray tag in it.

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::index;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
  StartTag {
    name: String,
    attributes: Vec<(String, String)>,
  },
  EndTag {
    name: String,
  },
  Text(String),
}

fn local_name(name: &str) -> Result<String, Never> {
  let local = match name.rsplit_once(':') {
    Some((_prefix, local)) => local,
    None => name,
  };

  Ok(local.to_ascii_lowercase())
}

fn numeric_entity(text: &str) -> Result<Option<char>, Never> {
  let number = match (
    text.strip_prefix("#x"),
    text.strip_prefix("#X"),
    text.strip_prefix('#'),
  ) {
    (Some(hexadecimal), _, _) | (_, Some(hexadecimal), _) => u32::from_str_radix(hexadecimal, 16),
    (None, None, Some(decimal)) => decimal.parse::<u32>(),
    (None, None, None) => return Ok(None),
  };

  Ok(match number {
    Ok(number_2) => char::from_u32(number_2),
    Err(_not_a_number) => None,
  })
}

fn named_entity(text: &str) -> Result<Option<char>, Never> {
  Ok(match text {
    "amp" => Some('&'),
    "lt" => Some('<'),
    "gt" => Some('>'),
    "quot" => Some('"'),
    "apos" => Some('\''),
    "nbsp" => Some('\u{a0}'),
    "mdash" => Some('\u{2014}'),
    "ndash" => Some('\u{2013}'),
    "hellip" => Some('\u{2026}'),
    "lsquo" => Some('\u{2018}'),
    "rsquo" => Some('\u{2019}'),
    "ldquo" => Some('\u{201c}'),
    "rdquo" => Some('\u{201d}'),
    "copy" => Some('\u{a9}'),
    other => {
      let Ok(letter) = numeric_entity(other);

      letter
    }
  })
}

const LONGEST_ENTITY: u32 = 10;

pub fn unescape(text: &str) -> Result<String, Never> {
  let unescaped = iterate((text, String::new()), |(rest, mut unescaped)| {
    let (before, after) = match rest.split_once('&') {
      Some(both) => both,
      None => {
        unescaped.push_str(rest);

        return Ok(Step::Halt(unescaped));
      }
    };

    unescaped.push_str(before);

    let named = after.split_once(';').filter(|(named, _)| {
      let Ok(limit) = index(LONGEST_ENTITY);
      named.len() <= limit
    });

    let letter = match named {
      Some((named_2, _)) => {
        let Ok(letter) = named_entity(named_2);

        letter
      }
      None => None,
    };

    let rest_2 = match (letter, named) {
      (Some(letter_2), Some((_, past))) => {
        unescaped.push(letter_2);

        past
      }
      (None, _) | (_, None) => {
        unescaped.push('&');

        after
      }
    };

    Ok(Step::Again((rest_2, unescaped)))
  });

  Ok(match unescaped {
    Ok(unescaped_2) => unescaped_2,
    Err(Endless) => text.to_string(),
  })
}

fn attributes(text: &str) -> Result<Vec<(String, String)>, Never> {
  let found = iterate((text.trim_start(), Vec::new()), |(rest, mut found)| {
    let (name, after) = match rest.split_once('=') {
      Some(both) => both,
      None => return Ok(Step::Halt(found)),
    };

    let after_2 = after.trim_start();
    let quote = match after_2.chars().next() {
      Some(quote @ ('"' | '\'')) => quote,
      Some(_) | None => return Ok(Step::Halt(found)),
    };

    let (value, past) = match after_2.get(1..).and_then(|inside| inside.split_once(quote)) {
      Some(both) => both,
      None => return Ok(Step::Halt(found)),
    };

    let Ok(name_2) = local_name(name.trim());
    let Ok(value_2) = unescape(value);

    found.push((name_2, value_2));

    Ok(Step::Again((past.trim_start(), found)))
  });

  Ok(match found {
    Ok(found_2) => found_2,
    Err(Endless) => Vec::new(),
  })
}

enum TagForm {
  Opening,
  Closing,
  Empty,
}

fn parse_tag(inside: &str) -> Result<Vec<Token>, Never> {
  let closing = inside.strip_prefix('/');
  let unmarked = match closing {
    Some(rest) => rest,
    None => inside,
  };
  let empty = unmarked.trim_end().strip_suffix('/');
  let (form, body) = match (closing, empty) {
    (Some(_), Some(content)) | (Some(content), None) => (TagForm::Closing, content),
    (None, Some(content)) => (TagForm::Empty, content),
    (None, None) => (TagForm::Opening, unmarked),
  };

  let (qualified_name, rest) = match body.split_once(|letter: char| letter.is_whitespace()) {
    Some(both) => both,
    None => (body, ""),
  };

  let Ok(name) = local_name(qualified_name);

  Ok(match form {
    TagForm::Closing => vec![Token::EndTag { name }],
    TagForm::Empty => {
      let Ok(attributes) = attributes(rest);

      vec![
        Token::StartTag {
          name: name.clone(),
          attributes,
        },
        Token::EndTag { name },
      ]
    }
    TagForm::Opening => {
      let Ok(attributes) = attributes(rest);

      vec![Token::StartTag { name, attributes }]
    }
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Delimiter<'a>(&'a str);

fn skip_past<'a>(after: &'a str, until: Delimiter<'_>) -> Result<&'a str, Never> {
  Ok(match after.split_once(until.0) {
    Some((_, past)) => past,
    None => "",
  })
}

fn push_text(text: &str, into: &mut Vec<Token>) -> Result<(), Never> {
  match text {
    "" => {}
    _nonempty => {
      let Ok(text_2) = unescape(text);

      into.push(Token::Text(text_2));
    }
  }

  Ok(())
}

pub fn tokenize(page: &str) -> Result<Vec<Token>, Never> {
  let found = iterate((page, Vec::new()), |(rest, mut found)| {
    let (before, after) = match rest.split_once('<') {
      Some(both) => both,
      None => {
        let Ok(()) = push_text(rest, &mut found);

        return Ok(Step::Halt(found));
      }
    };

    let Ok(()) = push_text(before, &mut found);

    let (comment, data, first) = (
      after.strip_prefix("!--"),
      after.strip_prefix("![CDATA["),
      after.bytes().next(),
    );

    let rest_2 = match (comment, data, first) {
      (Some(comment_2), _, _) => {
        let Ok(past) = skip_past(comment_2, Delimiter("-->"));

        past
      }
      (None, Some(data_2), _) => {
        let (inside, past) = match data_2.split_once("]]>") {
          Some(both) => both,
          None => (data_2, ""),
        };

        found.push(Token::Text(inside.to_string()));

        past
      }
      (None, None, Some(b'!' | b'?')) => {
        let Ok(past) = skip_past(after, Delimiter(">"));

        past
      }
      (None, None, _first) => match after.split_once('>') {
        Some((inside, past)) => {
          let Ok(tag) = parse_tag(inside);

          found.extend(tag);

          past
        }
        None => {
          let Ok(()) = push_text(after, &mut found);

          ""
        }
      },
    };

    Ok(Step::Again((rest_2, found)))
  });

  Ok(match found {
    Ok(found_2) => found_2,
    Err(Endless) => Vec::new(),
  })
}

pub fn attribute<'a>(
  attributes: &'a [(String, String)],
  name: &str,
) -> Result<Option<&'a str>, Never> {
  Ok(
    attributes
      .iter()
      .find(|(named, _)| named == name)
      .map(|(_, value)| value.as_str()),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn open(name: &str, attributes: &[(&str, &str)]) -> Result<Token, Never> {
    Ok(Token::StartTag {
      name: name.to_string(),
      attributes: attributes
        .iter()
        .map(|(name_2, value)| (name_2.to_string(), value.to_string()))
        .collect(),
    })
  }

  fn close(name: &str) -> Result<Token, Never> {
    Ok(Token::EndTag {
      name: name.to_string(),
    })
  }

  fn words(text: &str) -> Result<Token, Never> {
    Ok(Token::Text(text.to_string()))
  }

  #[test]
  fn a_paragraph_is_a_tag_its_words_and_the_tag_closing() {
    let Ok(opened) = open("p", &[("class", "first")]);
    let Ok(said) = words("Call me Ishmael.");
    let Ok(closed) = close("p");

    assert_eq!(
      tokenize("<p class=\"first\">Call me Ishmael.</p>"),
      Ok(vec![opened, said, closed])
    );
  }

  #[test]
  fn an_empty_tag_opens_and_closes_at_once_and_a_namespace_is_dropped() {
    let Ok(opened) = open("image", &[("href", "cover.jpg")]);
    let Ok(closed) = close("image");

    assert_eq!(
      tokenize("<svg:image xlink:href='cover.jpg'/>"),
      Ok(vec![opened, closed])
    );
  }

  #[test]
  fn entities_become_the_letters_they_stand_for() {
    let Ok(said) = words("Tom & Jerry \u{2014} \u{2019}\u{2026} &unknown; & more");

    assert_eq!(
      tokenize("Tom &amp; Jerry &#8212; &#x2019;&hellip; &unknown; & more"),
      Ok(vec![said])
    );
  }

  #[test]
  fn comments_and_declarations_are_stepped_over() {
    let Ok(opened) = open("b", &[]);
    let Ok(said) = words("bold");
    let Ok(closed) = close("b");

    assert_eq!(
      tokenize("<?xml version=\"1.0\"?><!DOCTYPE html><!-- a <b>note</b> --><b>bold</b>"),
      Ok(vec![opened, said, closed])
    );
  }
}
