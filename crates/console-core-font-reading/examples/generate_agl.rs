//! Packs the Adobe Glyph List into the trie read by `ps::agl`.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::io::{self, Write as _};

use console_core_never::Never;
use console_core_number_conversion::index;

type Failure = Box<dyn std::error::Error>;

#[derive(Clone, Copy)]
struct NodeIndex(u32);

impl NodeIndex {
  const ROOT: Self = Self(0);
}

#[derive(Default)]
struct Node {
  letters: Vec<u8>,
  value: Option<u16>,
  children: BTreeMap<u8, NodeIndex>,
  offset: u16,
}

struct Trie {
  nodes: Vec<Node>,
}

impl Trie {
  fn new() -> Result<Self, Never> {
    Ok(Self {
      nodes: vec![Node::default()],
    })
  }

  fn node(&self, NodeIndex(position): NodeIndex) -> io::Result<&Node> {
    let Ok(slot) = index(position);
    self
      .nodes
      .get(slot)
      .ok_or_else(|| io::Error::other("invalid glyph trie node"))
  }

  fn node_mut(&mut self, NodeIndex(position): NodeIndex) -> io::Result<&mut Node> {
    let Ok(slot) = index(position);
    self
      .nodes
      .get_mut(slot)
      .ok_or_else(|| io::Error::other("invalid glyph trie node"))
  }

  fn insert(&mut self, name: &[u8], value: u16) -> Result<(), Failure> {
    let mut current = NodeIndex::ROOT;

    for &letter in name {
      let parent = self.node(current)?;
      current = match parent.children.get(&letter) {
        Some(&child) => child,
        None => {
          let position = u32::try_from(self.nodes.len())?;
          let child = NodeIndex(position);
          self.nodes.push(Node {
            letters: vec![letter],
            ..Node::default()
          });
          let parent_node = self.node_mut(current)?;
          let _ = parent_node.children.insert(letter, child);
          child
        }
      };
    }

    let terminal = self.node_mut(current)?;
    terminal.value = Some(value);

    Ok(())
  }

  fn compress(&mut self) -> Result<(), Failure> {
    let count = u32::try_from(self.nodes.len())?;

    for position in (1..count).rev() {
      let parent_key = NodeIndex(position);
      let parent = self.node(parent_key)?;
      let child_key = parent
        .children
        .first_key_value()
        .filter(|_| parent.value.is_none() && parent.children.len() == 1);

      match child_key {
        Some((_, &key)) => {
          let child_node = self.node_mut(key)?;
          let child = std::mem::take(child_node);
          let parent_node = self.node_mut(parent_key)?;
          parent_node.letters.extend(child.letters);
          parent_node.value = child.value;
          parent_node.children = child.children;
        }
        None => {}
      }
    }

    Ok(())
  }

  fn locate(&mut self) -> Result<Vec<NodeIndex>, Failure> {
    let mut seen = Vec::new();
    let mut pending = vec![NodeIndex::ROOT];
    let mut offset: u32 = 0;
    let count = u32::try_from(self.nodes.len())?;

    for _entry in 0..count {
      let next = match pending.pop() {
        Some(entry) => entry,
        None => break,
      };
      let node = self.node_mut(next)?;
      let encoded_offset = u16::try_from(offset)?;
      node.offset = encoded_offset;
      let letters = u32::try_from(node.letters.len())?;
      let children = u32::try_from(node.children.len())?;
      let value = match node.value {
        Some(_) => 2,
        None => 0,
      };
      offset = offset
        .saturating_add(letters.max(1))
        .saturating_add(1)
        .saturating_add(value)
        .saturating_add(children.saturating_mul(2));

      seen.push(next);
      pending.extend(node.children.values().copied().rev());
    }

    Ok(seen)
  }

  fn emit(&self, ordered: &[NodeIndex]) -> Result<Vec<u8>, Failure> {
    let mut out = Vec::new();

    for &key in ordered {
      let node = self.node(key)?;
      let position = u32::try_from(out.len())?;

      match position.cmp(&u32::from(node.offset)) {
        std::cmp::Ordering::Equal => {}
        std::cmp::Ordering::Less | std::cmp::Ordering::Greater => {
          return Err(Failure::from(io::Error::new(
            io::ErrorKind::InvalidData,
            "glyph trie offset mismatch",
          )));
        }
      }

      match node.letters.split_last() {
        Some((&last, prefix)) => {
          out.extend(prefix.iter().map(|letter| letter | 0x80));
          out.push(last);
        }
        None => out.push(0),
      }

      let count = u8::try_from(node.children.len())?;

      match node.value {
        Some(value) => {
          out.push(count | 0x80);
          out.extend(value.to_be_bytes());
        }
        None => out.push(count),
      }

      for &child_key in node.children.values() {
        let child = self.node(child_key)?;
        out.extend(child.offset.to_be_bytes());
      }
    }

    Ok(out)
  }
}

fn generate(glyphs: &str) -> Result<String, Failure> {
  let Ok(mut trie) = Trie::new();
  let mut multis = BTreeMap::new();
  let mut max_length = 0_u32;

  for line in glyphs
    .lines()
    .filter(|line| !line.is_empty() && !line.starts_with('#'))
  {
    let (name, points) = line.split_once(';').ok_or("missing glyph-list separator")?;

    match {
      ::core::primitive::bool::then_some(
        name.is_empty() || !name.bytes().all(|letter| letter.is_ascii_graphic()),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        return Err(Failure::from(format!("invalid glyph name {name:?}")));
      }
      ::core::option::Option::None => {}
    }

    let length = u32::try_from(name.len())?;
    max_length = max_length.max(length);
    let parsed = points
      .split_ascii_whitespace()
      .map(|point| u16::from_str_radix(point, 16))
      .collect::<Result<Vec<_>, _>>()?;

    match parsed.as_slice() {
      [] => return Err(Failure::from(format!("missing code points for {name}"))),
      [value] => trie.insert(name.as_bytes(), *value)?,
      _ => {
        multis.insert(name, points);
      }
    }
  }

  trie.compress()?;
  let ordered = trie.locate()?;
  let bytes = trie.emit(&ordered)?;
  let size = u32::try_from(bytes.len())?;

  let mut output = String::new();
  writeln!(
    output,
    "pub const MAX_NAME_LEN: ::console_core_api_compatibility::NativeSize = {};",
    max_length.saturating_add(1)
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

  match { ::core::primitive::bool::then_some(args.next().is_some(), ()) } {
    ::core::option::Option::Some(()) => {
      return Err(Failure::from("usage: generate_agl [glyphlist.txt]"));
    }
    ::core::option::Option::None => {}
  }

  let glyphs = match source {
    Some(path) => fs::read_to_string(path)?,
    None => include_str!("../data/glyphlist.txt").to_string(),
  };
  let packed = generate(&glyphs)?;
  io::stdout().lock().write_all(packed.as_bytes())?;
  Ok(())
}
