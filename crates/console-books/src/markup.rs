//! The tags and the words of a page of markup, and nothing else about it.
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

use console_core_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    StartTag { name: String, attributes: Vec<(String, String)> },
    EndTag { name: String },
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
    let number = match (text.strip_prefix("#x"), text.strip_prefix("#X"), text.strip_prefix('#')) {
        (Some(hexadecimal), _, _) | (_, Some(hexadecimal), _) => u32::from_str_radix(hexadecimal, 16),
        (None, None, Some(decimal)) => decimal.parse::<u32>(),
        (None, None, None) => return Ok(None),
    };

    Ok(match number {
        Ok(number) => char::from_u32(number),
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
        },
    })
}

const LONGEST_ENTITY: u32 = 10;

pub fn unescape(text: &str) -> Result<String, Never> {
    let mut unescaped = String::new();
    let mut rest = text;

    loop {
        let (before, after) = match rest.split_once('&') {
            Some(both) => both,
            None => {
                unescaped.push_str(rest);

                return Ok(unescaped);
            },
        };

        unescaped.push_str(before);

        let named = after
            .split_once(';')
            .filter(|(named, _)| u32::try_from(named.len()).is_ok_and(|long| long <= LONGEST_ENTITY));

        let letter = match named {
            Some((named, _)) => {
                let Ok(letter) = named_entity(named);

                letter
            },
            None => None,
        };

        rest = match (letter, named) {
            (Some(letter), Some((_, past))) => {
                unescaped.push(letter);

                past
            },
            (None, _) | (_, None) => {
                unescaped.push('&');

                after
            },
        };
    }
}

fn attributes(text: &str) -> Result<Vec<(String, String)>, Never> {
    let mut found = Vec::new();
    let mut rest = text.trim_start();

    loop {
        let (name, after) = match rest.split_once('=') {
            Some(both) => both,
            None => return Ok(found),
        };

        let after = after.trim_start();
        let quote = match after.chars().next() {
            Some(quote @ ('"' | '\'')) => quote,
            Some(_) | None => return Ok(found),
        };

        let (value, past) = match after.get(1..).and_then(|inside| inside.split_once(quote)) {
            Some(both) => both,
            None => return Ok(found),
        };

        let Ok(name) = local_name(name.trim());
        let Ok(value) = unescape(value);

        found.push((name, value));
        rest = past.trim_start();
    }
}

fn parse_tag(inside: &str) -> Result<Vec<Token>, Never> {
    let (closing, inside) = match inside.strip_prefix('/') {
        Some(inside) => (true, inside),
        None => (false, inside),
    };

    let (empty, inside) = match inside.trim_end().strip_suffix('/') {
        Some(inside) => (true, inside),
        None => (false, inside),
    };

    let (name, rest) = match inside.split_once(|letter: char| letter.is_whitespace()) {
        Some(both) => both,
        None => (inside, ""),
    };

    let Ok(name) = local_name(name);

    Ok(match (closing, empty) {
        (true, _) => vec![Token::EndTag { name }],
        (false, true) => {
            let Ok(attributes) = attributes(rest);

            vec![Token::StartTag { name: name.clone(), attributes }, Token::EndTag { name }]
        },
        (false, false) => {
            let Ok(attributes) = attributes(rest);

            vec![Token::StartTag { name, attributes }]
        },
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
    match text.is_empty() {
        true => {},
        false => {
            let Ok(text) = unescape(text);

            into.push(Token::Text(text));
        },
    }

    Ok(())
}

pub fn tokenize(page: &str) -> Result<Vec<Token>, Never> {
    let mut found = Vec::new();
    let mut rest = page;

    loop {
        let (before, after) = match rest.split_once('<') {
            Some(both) => both,
            None => {
                let Ok(()) = push_text(rest, &mut found);

                return Ok(found);
            },
        };

        let Ok(()) = push_text(before, &mut found);

        let (comment, data, declared) =
            (after.strip_prefix("!--"), after.strip_prefix("![CDATA["), after.starts_with(['!', '?']));

        rest = match (comment, data, declared) {
            (Some(comment), _, _) => {
                let Ok(past) = skip_past(comment, Delimiter("-->"));

                past
            },
            (None, Some(data), _) => {
                let (inside, past) = match data.split_once("]]>") {
                    Some(both) => both,
                    None => (data, ""),
                };

                found.push(Token::Text(inside.to_string()));

                past
            },
            (None, None, true) => {
                let Ok(past) = skip_past(after, Delimiter(">"));

                past
            },
            (None, None, false) => match after.split_once('>') {
                Some((inside, past)) => {
                    let Ok(tag) = parse_tag(inside);

                    found.extend(tag);

                    past
                },
                None => {
                    let Ok(()) = push_text(after, &mut found);

                    ""
                },
            },
        };
    }
}

pub fn attribute<'a>(attributes: &'a [(String, String)], name: &str) -> Result<Option<&'a str>, Never> {
    Ok(attributes.iter().find(|(named, _)| named == name).map(|(_, value)| value.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(name: &str, attributes: &[(&str, &str)]) -> Token {
        Token::StartTag {
            name: name.to_string(),
            attributes: attributes.iter().map(|(name, value)| (name.to_string(), value.to_string())).collect(),
        }
    }

    fn close(name: &str) -> Token {
        Token::EndTag { name: name.to_string() }
    }

    fn words(text: &str) -> Token {
        Token::Text(text.to_string())
    }

    #[test]
    fn a_paragraph_is_a_tag_its_words_and_the_tag_closing() {
        assert_eq!(
            tokenize("<p class=\"first\">Call me Ishmael.</p>"),
            Ok(vec![open("p", &[("class", "first")]), words("Call me Ishmael."), close("p")])
        );
    }

    #[test]
    fn an_empty_tag_opens_and_closes_at_once_and_a_namespace_is_dropped() {
        assert_eq!(
            tokenize("<svg:image xlink:href='cover.jpg'/>"),
            Ok(vec![open("image", &[("href", "cover.jpg")]), close("image")])
        );
    }

    #[test]
    fn entities_become_the_letters_they_stand_for() {
        assert_eq!(
            tokenize("Tom &amp; Jerry &#8212; &#x2019;&hellip; &unknown; & more"),
            Ok(vec![words("Tom & Jerry \u{2014} \u{2019}\u{2026} &unknown; & more")])
        );
    }

    #[test]
    fn comments_and_declarations_are_stepped_over() {
        assert_eq!(
            tokenize("<?xml version=\"1.0\"?><!DOCTYPE html><!-- a <b>note</b> --><b>bold</b>"),
            Ok(vec![open("b", &[]), words("bold"), close("b")])
        );
    }
}
