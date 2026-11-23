//! Streaming XML output with automatic element closing and explicit writer
//! faults. Element names remain ranges in the output buffer, so closing a
//! deeply nested tree does not allocate another copy of each name.

#![forbid(unsafe_code)]

use std::cmp::Ordering;
use std::fmt::{self, Display};
use std::ops::Range;
use std::string::FromUtf8Error;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, indices};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Indent {
  None,
  Spaces(u8),
  Tabs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeQuote {
  Single,
  Double,
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
  pub attribute_quote: AttributeQuote,
  pub indent: Indent,
  pub attributes_indent: Indent,
}

impl Default for Options {
  fn default() -> Self {
    Self {
      attribute_quote: AttributeQuote::Double,
      indent: Indent::Spaces(4),
      attributes_indent: Indent::None,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Whitespace {
  Indent,
  Preserve,
}

#[derive(Debug)]
pub enum Error {
  DeclarationAlreadyWritten,
  AttributeOutsideElement,
  TextOutsideElement,
  InvalidElementName,
  Formatting(fmt::Error),
  InvalidText(FromUtf8Error),
}

impl fmt::Display for Error {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::DeclarationAlreadyWritten => write!(to, "declaration was already written"),
      Self::AttributeOutsideElement => write!(to, "attribute must follow start_element"),
      Self::TextOutsideElement => write!(to, "text must follow start_element"),
      Self::InvalidElementName => {
        write!(to, "an element name was removed from the output buffer")
      }
      Self::Formatting(fault) => write!(to, "XML value could not be formatted: {fault}"),
      Self::InvalidText(fault) => write!(to, "XML output is not UTF-8: {fault}"),
    }
  }
}

impl std::error::Error for Error {}

impl From<Never> for Error {
  fn from(never: Never) -> Self {
    never.absurd()
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
  Empty,
  Document,
  Attributes,
}

#[derive(Clone, Copy)]
enum Escape {
  Quote(u8),
  Text,
}

enum StartTag {
  Pending,
  Finished,
}

struct DepthData {
  range: Range<u64>,
  start_tag: StartTag,
}

pub struct XmlWriter {
  buf: Vec<u8>,
  state: State,
  whitespace: Whitespace,
  depth_stack: Vec<DepthData>,
  opt: Options,
}

struct Buffer<'a>(&'a mut Vec<u8>);

impl fmt::Write for Buffer<'_> {
  fn write_str(&mut self, text: &str) -> fmt::Result {
    self.0.extend_from_slice(text.as_bytes());

    Ok(())
  }
}

impl XmlWriter {
  fn from_vec(buf: Vec<u8>, opt: Options) -> Result<Self, Never> {
    Ok(Self {
      buf,
      state: State::Empty,
      whitespace: Whitespace::Indent,
      depth_stack: Vec::new(),
      opt,
    })
  }

  pub fn new(opt: Options) -> Result<Self, Never> {
    Self::from_vec(Vec::new(), opt)
  }

  pub fn with_capacity(capacity: u64, opt: Options) -> Result<Self, Never> {
    let Ok(capacity_index) = index(capacity);

    Self::from_vec(Vec::with_capacity(capacity_index.min(65_536)), opt)
  }

  pub fn write_declaration(&mut self) -> Result<(), Error> {
    match self.state {
      State::Empty => self.state = State::Attributes,
      State::Document | State::Attributes => return Err(Error::DeclarationAlreadyWritten),
    }

    self.buf.extend_from_slice(b"<?xml");
    self.write_attribute("version", "1.0")?;
    self.write_attribute("encoding", "UTF-8")?;
    self.write_attribute("standalone", "no")?;
    self.buf.extend_from_slice(b"?>");
    self.state = State::Document;

    Ok(())
  }

  pub fn write_comment(&mut self, text: &str) -> Result<(), Error> {
    self.write_comment_fmt(format_args!("{text}"))
  }

  pub fn write_comment_fmt(&mut self, args: fmt::Arguments<'_>) -> Result<(), Error> {
    let Ok(()) = self.prepare_node();
    let Ok(()) = self.write_node_indent();

    self.buf.extend_from_slice(b"<!--");
    fmt::write(&mut Buffer(&mut self.buf), args).map_err(Error::Formatting)?;
    self.buf.extend_from_slice(b"-->");
    self.state = State::Document;

    Ok(())
  }

  pub fn start_element(&mut self, name: &str) -> Result<(), Never> {
    let Ok(()) = self.prepare_node();
    let Ok(()) = self.write_node_indent();

    self.buf.push(b'<');

    let Ok(start) = fitted::<_, u64>(self.buf.len());

    self.buf.extend_from_slice(name.as_bytes());

    let Ok(end) = fitted::<_, u64>(self.buf.len());

    self.depth_stack.push(DepthData {
      range: start..end,
      start_tag: StartTag::Pending,
    });
    self.state = State::Attributes;

    Ok(())
  }

  pub fn write_attribute<V: Display + ?Sized>(
    &mut self,
    name: &str,
    value: &V,
  ) -> Result<(), Error> {
    self.write_attribute_fmt(name, format_args!("{value}"))
  }

  pub fn write_attribute_fmt(
    &mut self,
    name: &str,
    args: fmt::Arguments<'_>,
  ) -> Result<(), Error> {
    self.write_attribute_prefix(name)?;

    let Ok(start) = fitted::<_, u64>(self.buf.len());

    fmt::write(&mut Buffer(&mut self.buf), args).map_err(Error::Formatting)?;

    let Ok(quote) = self.quote();

    self.escape(start, Escape::Quote(quote))?;
    self.buf.push(quote);

    Ok(())
  }

  pub fn write_attribute_raw<F>(&mut self, name: &str, write: F) -> Result<(), Error>
  where
    F: FnOnce(&mut Vec<u8>),
  {
    self.write_attribute_prefix(name)?;

    let Ok(start) = fitted::<_, u64>(self.buf.len());

    write(&mut self.buf);

    let Ok(quote) = self.quote();

    self.escape(start, Escape::Quote(quote))?;
    self.buf.push(quote);

    Ok(())
  }

  fn write_attribute_prefix(&mut self, name: &str) -> Result<(), Error> {
    match self.state {
      State::Attributes => {}
      State::Empty | State::Document => return Err(Error::AttributeOutsideElement),
    }

    match self.opt.attributes_indent {
      Indent::None => self.buf.push(b' '),
      Indent::Spaces(_) | Indent::Tabs => {
        self.buf.push(b'\n');

        let Ok(depth) = fitted::<_, u64>(self.depth_stack.len());
        let Ok(()) = self.write_indent(depth.saturating_sub(1), self.opt.indent);
        let Ok(()) = self.write_indent(1, self.opt.attributes_indent);
      }
    }

    self.buf.extend_from_slice(name.as_bytes());
    self.buf.push(b'=');

    let Ok(quote) = self.quote();

    self.buf.push(quote);

    Ok(())
  }

  fn escape(&mut self, start: u64, escape: Escape) -> Result<(), Error> {
    let Ok(end) = fitted::<_, u64>(self.buf.len());

    match start.cmp(&end) {
      Ordering::Less | Ordering::Equal => {}
      Ordering::Greater => return Err(Error::InvalidElementName),
    }

    let mut at = start;

    for _original_byte in start..end {
      let Ok(position) = index(at);
      let byte = self
        .buf
        .get(position)
        .copied()
        .ok_or(Error::InvalidElementName)?;
      let replacement: Option<&[u8]> = match (byte, escape) {
        (b'\'', Escape::Quote(b'\'')) => Some(b"&apos;"),
        (b'"', Escape::Quote(b'"')) => Some(b"&quot;"),
        (b'<', Escape::Text) => Some(b"&lt;"),
        (_ordinary, Escape::Quote(_) | Escape::Text) => None,
      };

      match replacement {
        Some(replacement_2) => {
          let Ok(range) = indices(at..at.saturating_add(1));

          drop(self.buf.splice(range, replacement_2.iter().copied()));

          let Ok(length) = fitted::<_, u64>(replacement_2.len());

          at = at.saturating_add(length);
        }
        None => at = at.saturating_add(1),
      }
    }

    Ok(())
  }

  pub fn set_preserve_whitespaces(&mut self, whitespace: Whitespace) -> Result<(), Never> {
    self.whitespace = whitespace;

    Ok(())
  }

  pub fn write_text(&mut self, text: &str) -> Result<(), Error> {
    self.write_text_fmt(format_args!("{text}"))
  }

  pub fn write_text_fmt(&mut self, args: fmt::Arguments<'_>) -> Result<(), Error> {
    match (self.state, self.depth_stack.as_slice()) {
      (State::Empty, _) | (_, []) => return Err(Error::TextOutsideElement),
      (State::Attributes | State::Document, [_, ..]) => {}
    }

    let Ok(()) = self.prepare_node();
    let Ok(()) = self.write_node_indent();
    let Ok(start) = fitted::<_, u64>(self.buf.len());

    fmt::write(&mut Buffer(&mut self.buf), args).map_err(Error::Formatting)?;
    self.escape(start, Escape::Text)?;
    self.state = State::Document;

    Ok(())
  }

  pub fn end_element(&mut self) -> Result<(), Error> {
    self.state = State::Document;

    let depth = match self.depth_stack.pop() {
      Some(depth) => depth,
      None => return Ok(()),
    };

    match depth.start_tag {
      StartTag::Finished => {
        let Ok(()) = self.write_new_line();
        let Ok(()) = self.write_node_indent();

        self.buf.extend_from_slice(b"</");

        let Ok(range) = indices(depth.range);

        match self.buf.get(range.clone()) {
          Some(_name) => self.buf.extend_from_within(range),
          None => return Err(Error::InvalidElementName),
        }

        self.buf.push(b'>');
      }
      StartTag::Pending => self.buf.extend_from_slice(b"/>"),
    }

    Ok(())
  }

  pub fn end_document(mut self) -> Result<String, Error> {
    let Ok(depth) = fitted::<_, u64>(self.depth_stack.len());

    for _element in 0..depth {
      self.end_element()?;
    }

    let Ok(()) = self.write_new_line();

    String::from_utf8(self.buf).map_err(Error::InvalidText)
  }

  fn quote(&self) -> Result<u8, Never> {
    Ok(match self.opt.attribute_quote {
      AttributeQuote::Single => b'\'',
      AttributeQuote::Double => b'"',
    })
  }

  fn prepare_node(&mut self) -> Result<(), Never> {
    match self.state {
      State::Attributes => {
        let Ok(()) = self.write_open_element();
      }
      State::Empty | State::Document => {}
    }

    match self.state {
      State::Empty => {}
      State::Attributes | State::Document => {
        let Ok(()) = self.write_new_line();
      }
    }

    Ok(())
  }

  fn write_open_element(&mut self) -> Result<(), Never> {
    match self.depth_stack.last_mut() {
      Some(depth) => {
        depth.start_tag = StartTag::Finished;
        self.buf.push(b'>');
        self.state = State::Document;
      }
      None => {}
    }

    Ok(())
  }

  fn write_node_indent(&mut self) -> Result<(), Never> {
    let Ok(depth) = fitted::<_, u64>(self.depth_stack.len());

    self.write_indent(depth, self.opt.indent)
  }

  fn write_indent(&mut self, depth: u64, indent: Indent) -> Result<(), Never> {
    match self.whitespace {
      Whitespace::Preserve => return Ok(()),
      Whitespace::Indent => {}
    }

    for _level in 0..depth {
      match indent {
        Indent::None => {}
        Indent::Spaces(spaces) => {
          for _space in 0..spaces {
            self.buf.push(b' ');
          }
        }
        Indent::Tabs => self.buf.push(b'\t'),
      }
    }

    Ok(())
  }

  fn write_new_line(&mut self) -> Result<(), Never> {
    match (self.opt.indent, self.whitespace) {
      (Indent::None, _) | (_, Whitespace::Preserve) => {}
      (Indent::Spaces(_) | Indent::Tabs, Whitespace::Indent) => self.buf.push(b'\n'),
    }

    Ok(())
  }
}
