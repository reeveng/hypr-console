//! Packs the Adobe Glyph List into the trie read by `ps::agl`.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::io::{self, Write as _};

type Failure = Box<dyn std::error::Error>;

#[derive(Default)]
struct Node {
  letters: Vec<u8>,
  value: Option<u16>,
  children: BTreeMap<u8, Node>,
  offset: u16,
}

impl Node {
  fn insert(&mut self, name: &[u8], value: u16) {
    let mut node = self;
    for &letter in name {
      node = node.children.entry(letter).or_insert_with(|| Self {
        letters: vec![letter],
        ..Self::default()
      });
    }
    node.value = Some(value);
  }

  // A branching node or a node with its own value ends a compressed edge.
  fn compress(&mut self) {
    for child in self.children.values_mut() {
      child.compress();
    }
    if self.value.is_none()
      && self.children.len() == 1
      && let Some((_, child)) = self.children.pop_first()
    {
      self.letters.extend(child.letters);
      self.value = child.value;
      self.children = child.children;
    }
  }

  fn locate(&mut self, offset: usize) -> Result<usize, Failure> {
    self.offset = u16::try_from(offset)?;
    let value_size = if self.value.is_some() { 2 } else { 0 };
    let mut next = offset + self.letters.len().max(1) + 1 + value_size + self.children.len() * 2;
    for child in self.children.values_mut() {
      next = child.locate(next)?;
    }
    Ok(next)
  }

  fn emit(&self, out: &mut Vec<u8>) -> Result<(), Failure> {
    debug_assert_eq!(out.len(), usize::from(self.offset));
    if let Some((&last, prefix)) = self.letters.split_last() {
      out.extend(prefix.iter().map(|letter| letter | 0x80));
      out.push(last);
    } else {
      out.push(0);
    }

    let count = u8::try_from(self.children.len())?;
    if let Some(value) = self.value {
      out.push(count | 0x80);
      out.extend(value.to_be_bytes());
    } else {
      out.push(count);
    }

    for child in self.children.values() {
      out.extend(child.offset.to_be_bytes());
    }
    for child in self.children.values() {
      child.emit(out)?;
    }
    Ok(())
  }
}

fn generate(glyphs: &str) -> Result<String, Failure> {
  let mut trie = Node::default();
  let mut multis = BTreeMap::new();
  let mut max_length = 0;
  for line in glyphs
    .lines()
    .filter(|line| !line.is_empty() && !line.starts_with('#'))
  {
    let (name, points) = line.split_once(';').ok_or("missing glyph-list separator")?;
    if name.is_empty() || !name.bytes().all(|letter| letter.is_ascii_graphic()) {
      return Err(format!("invalid glyph name {name:?}").into());
    }
    max_length = max_length.max(name.len());
    let parsed = points
      .split_ascii_whitespace()
      .map(|point| u16::from_str_radix(point, 16))
      .collect::<Result<Vec<_>, _>>()?;
    match parsed.as_slice() {
      [] => return Err(format!("missing code points for {name}").into()),
      [value] => trie.insert(name.as_bytes(), *value),
      _ => {
        multis.insert(name, points);
      }
    }
  }

  // Keep the root's NUL letter and compress the edges below it.
  for child in trie.children.values_mut() {
    child.compress();
  }
  let size = trie.locate(0)?;
  let mut bytes = Vec::with_capacity(size);
  trie.emit(&mut bytes)?;

  let mut output = String::new();
  writeln!(
    output,
    "pub const MAX_NAME_LEN: ::console_core_api_compatibility::NativeSize = {};",
    max_length + 1
  )?;
  writeln!(
    output,
    "\n#[rustfmt::skip]\nstatic AGL_TRIE: [u8; {size}] = ["
  )?;
  for line in bytes.chunks(32) {
    output.push_str("   ");
    for byte in line {
      write!(output, " {byte:3},")?;
    }
    output.push('\n');
  }
  writeln!(
    output,
    "];\n\n#[rustfmt::skip]\nstatic AGL_MULTIS: [(&str, &[u16]); {}] = [",
    multis.len()
  )?;
  for (name, points) in multis {
    let literals = points
      .split_ascii_whitespace()
      .map(|point| format!("0x{point}"))
      .collect::<Vec<_>>();
    writeln!(output, "    ({name:?}, &[{}]),", literals.join(", "))?;
  }
  writeln!(output, "];")?;
  Ok(output)
}

fn main() -> Result<(), Failure> {
  let mut args = std::env::args_os().skip(1);
  let source = args.next();
  if args.next().is_some() {
    return Err("usage: generate_agl [glyphlist.txt]".into());
  }
  let glyphs = match source {
    Some(path) => fs::read_to_string(path)?,
    None => include_str!("../data/glyphlist.txt").to_string(),
  };
  io::stdout()
    .lock()
    .write_all(generate(&glyphs)?.as_bytes())?;
  Ok(())
}
