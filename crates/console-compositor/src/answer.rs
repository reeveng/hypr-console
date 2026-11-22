//! An answer's bytes, read as text one string at a time.
//!
//! Hyprland writes a window's title into its JSON the way the window set it:
//! what JSON will not take as it is gets escaped, and every other byte goes
//! through as it came. So an answer is JSON whose strings are each in whatever
//! encoding their own window wrote, and reading the whole answer one way is the
//! mistake `console-core-text-encodings` was written against -- one Latin-1
//! title, and every other window's em dash reads as windows-1252. Each string
//! is one window's words, so each string is the unit that is decoded, and what
//! lies between the strings -- the braces, the commas, the names of the fields
//! -- is Hyprland's own and is decoded as a unit of its own.
//!
//! The walk needs nothing but a quote and the escape that can stand in front of
//! one. Both are ASCII, and every encoding a title arrives in writes ASCII as
//! itself, so a quote in the bytes is a quote whatever wrote the title around
//! it.

use std::borrow::Cow;

use console_core_never::Never;
use console_core_text_encodings::decode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
  Between,
  Inside,
  Escaped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Read {
  Quote,
  Within,
}

pub fn text(printed: &[u8]) -> Result<Cow<'_, str>, Never> {
  Ok(match std::str::from_utf8(printed) {
    Ok(text) => Cow::Borrowed(text),
    Err(_a_window_wrote_something_else) => {
      let Ok(each) = each_string_decoded(printed);

      Cow::Owned(each)
    }
  })
}

fn each_string_decoded(printed: &[u8]) -> Result<String, Never> {
  let mut text = String::with_capacity(printed.len());
  let mut unit = Vec::new();
  let mut standing = Standing::Between;

  for byte in printed {
    let (next, read) = match (standing, *byte) {
      (Standing::Between, b'"') => (Standing::Inside, Read::Quote),
      (Standing::Between, _structure) => (Standing::Between, Read::Within),
      (Standing::Inside, b'"') => (Standing::Between, Read::Quote),
      (Standing::Inside, b'\\') => (Standing::Escaped, Read::Within),
      (Standing::Inside, _written) => (Standing::Inside, Read::Within),
      (Standing::Escaped, _escaped) => (Standing::Inside, Read::Within),
    };

    match read {
      Read::Within => unit.push(*byte),
      Read::Quote => {
        let Ok(()) = decoded_onto(&mut text, &unit);

        text.push('"');
        unit.clear();
      }
    }

    standing = next;
  }

  let Ok(()) = decoded_onto(&mut text, &unit);

  Ok(text)
}

fn decoded_onto(text: &mut String, unit: &[u8]) -> Result<(), Never> {
  let Ok(decoded) = decode(unit);

  text.push_str(&decoded.text);

  Ok(())
}
