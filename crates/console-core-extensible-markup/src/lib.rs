/*!
Represent an [XML](https://www.w3.org/TR/xml/) document as a read-only tree.

The root point of the documentations is [`Document::parse`].

You can find more details in the [README] and the [parsing doc].

The tree structure itself is a heavily modified <https://github.com/causal-agent/ego-tree>
License: ISC.

[`Document::parse`]: struct.Document.html#method.parse
[README]: https://github.com/RazrFalcon/console_core_extensible_markup/blob/master/README.md
[parsing doc]: https://github.com/RazrFalcon/console_core_extensible_markup/blob/master/docs/parsing.md
*/

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(any(feature = "std", test))]
extern crate std;

use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::num::NonZeroU32;
use core::ops::Range;

use alloc::{boxed::Box, vec::Vec};

mod parse;
mod tokenizer;

#[cfg(test)]
mod tokenizer_tests;

pub use crate::parse::*;

pub const NS_XML_URI: &str = "http://www.w3.org/XML/1998/namespace";

const NS_XML_PREFIX: &str = "xml";

pub const NS_XMLNS_URI: &str = "http://www.w3.org/2000/xmlns/";

const XMLNS: &str = "xmlns";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TextPos {
  pub row: u32,
  pub col: u32,
}

impl TextPos {
  pub fn new(row: Row, col: Column) -> core::result::Result<TextPos, console_core_never::Never> {
    Ok(TextPos {
      row: row.0,
      col: col.0,
    })
  }
}

pub struct Row(pub u32);

pub struct Column(pub u32);

impl fmt::Display for TextPos {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}:{}", self.row, self.col)
  }
}

pub struct Document<'input> {
  text: &'input str,
  nodes: NodeList<'input>,
  attributes: Vec<AttributeData<'input>>,
  namespaces: Namespaces<'input>,
}

impl<'input> Document<'input> {
  #[inline]
  pub fn root<'a>(&'a self) -> core::result::Result<Node<'a, 'input>, console_core_never::Never> {
    Ok(Node {
      id: NodeId::ROOT,
      d: &self.nodes.root,
      doc: self,
    })
  }

  #[inline]
  pub fn get_node<'a>(
    &'a self,
    id: NodeId,
  ) -> core::result::Result<Option<Node<'a, 'input>>, console_core_never::Never> {
    let Ok(found) = self.nodes.get(id);

    Ok(found.map(|data| Node {
      id,
      d: data,
      doc: self,
    }))
  }

  fn node_at<'a>(
    &'a self,
    position: u64,
  ) -> core::result::Result<Option<Node<'a, 'input>>, console_core_never::Never> {
    let id = match NodeId::from_count(position) {
      Ok(id) => id,
      Err(_outside_node_ids) => return Ok(None),
    };

    self.get_node(id)
  }

  #[inline]
  pub fn root_element<'a>(&'a self) -> core::result::Result<Node<'a, 'input>, Error> {
    let Ok(root) = self.root();
    let Ok(element) = root.first_element_child();

    element.ok_or(Error::NoRootNode)
  }

  #[inline]
  pub fn descendants(
    &self,
  ) -> core::result::Result<Descendants<'_, 'input>, console_core_never::Never> {
    let Ok(root) = self.root();
    let Ok(descendants) = root.descendants();

    Ok(descendants)
  }

  #[inline]
  pub fn text_pos_at(
    &self,
    pos: u64,
  ) -> core::result::Result<TextPos, console_core_never::Never> {
    let Ok(stream) = tokenizer::Stream::new(self.text);
    let Ok(gen_text_pos_from) = stream.gen_text_pos_from(pos);

    Ok(gen_text_pos_from)
  }

  #[inline]
  pub fn input_text(&self) -> core::result::Result<&'input str, console_core_never::Never> {
    Ok(self.text)
  }
}

fn writeln_indented(
  depth: u64,
  f: &mut fmt::Formatter<'_>,
  text: fmt::Arguments<'_>,
) -> fmt::Result {
  for _ in 0..depth {
    f.write_str("    ")?;
  }

  writeln!(f, "{text}")
}

fn print_into_iter<
  T: fmt::Debug,
  E: ExactSizeIterator<Item = T>,
  I: IntoIterator<Item = T, IntoIter = E>,
>(
  prefix: &str,
  data: I,
  depth: u64,
  f: &mut fmt::Formatter,
) -> Result<(), fmt::Error> {
  let data_2 = data.into_iter();

  match data_2.len() {
    0 => {
      return Ok(());
    }
    1.. => {}
  }

  writeln_indented(depth, f, format_args!("{prefix}: ["))?;

  for v in data_2 {
    writeln_indented(depth.wrapping_add(1), f, format_args!("{v:?}"))?;
  }

  writeln_indented(depth, f, format_args!("]"))
}

enum PrintFrame<'a, 'input> {
  Node(Node<'a, 'input>, u64),
  ElementEnd(u64),
  ChildrenEnd(u64),
}

fn print_children(parent: Node, depth: u64, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
  let Ok(children) = parent.children();
  let mut pending: Vec<_> = children
    .rev()
    .map(|child| PrintFrame::Node(child, depth))
    .collect();
  let Ok(nodes) = parent.doc.nodes.len();

  for _event in 0..nodes.saturating_mul(3) {
    let frame = match pending.pop() {
      Some(frame) => frame,
      None => break,
    };

    match frame {
      PrintFrame::Node(child, level) => {
        print_node(child, level, f, &mut pending)?;
      }
      PrintFrame::ElementEnd(element_level) => {
        writeln_indented(element_level, f, format_args!("}}"))?;
      }
      PrintFrame::ChildrenEnd(children_level) => {
        writeln_indented(children_level, f, format_args!("    ]"))?;
      }
    }
  }

  Ok(())
}

fn print_node<'a, 'input>(
  node: Node<'a, 'input>,
  depth: u64,
  f: &mut fmt::Formatter<'_>,
  pending: &mut Vec<PrintFrame<'a, 'input>>,
) -> fmt::Result {
  match node.d.kind {
    NodeKind::Element { .. } => {}
    NodeKind::Root | NodeKind::Text(_) | NodeKind::Comment(_) | NodeKind::PI(_) => {
      return writeln_indented(depth, f, format_args!("{node:?}"));
    }
  }

  writeln_indented(depth, f, format_args!("Element {{"))?;
  let Ok(tag_name) = node.tag_name();
  writeln_indented(depth, f, format_args!("    tag_name: {tag_name:?}"))?;
  let Ok(attributes) = node.attributes();
  let Ok(namespaces) = node.namespaces();
  let field_depth = depth.saturating_add(1);
  print_into_iter("attributes", attributes, field_depth, f)?;
  print_into_iter("namespaces", namespaces, field_depth, f)?;
  pending.push(PrintFrame::ElementEnd(depth));

  let Ok(has_children) = node.has_children();

  match has_children {
    Presence::Present => {
      writeln_indented(depth, f, format_args!("    children: ["))?;
      pending.push(PrintFrame::ChildrenEnd(depth));
      let Ok(children) = node.children();
      let child_depth = depth.saturating_add(2);
      pending.extend(
        children
          .rev()
          .map(|child| PrintFrame::Node(child, child_depth)),
      );
    }
    Presence::Absent => {}
  }

  Ok(())
}

impl<'input> fmt::Debug for Document<'input> {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    let Ok(root) = self.root();
    let Ok(has_children) = root.has_children();

    match has_children {
      crate::Presence::Absent => {
        return write!(f, "Document []");
      }
      crate::Presence::Present => {}
    }

    writeln!(f, "Document [")?;
    let Ok(own_root) = self.root();
    print_children(own_root, 1, f)?;
    writeln!(f, "]")?;

    Ok(())
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeType {
  Root,

  Element,

  PI,

  Comment,

  Text,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PI<'input> {
  pub target: &'input str,
  pub value: Option<&'input str>,
}

#[derive(Clone, Copy, Debug)]
struct ShortRange {
  start: u32,
  end: u32,
}

impl TryFrom<Range<u64>> for ShortRange {
  type Error = Error;

  #[inline]
  fn try_from(range: Range<u64>) -> Result<Self, Self::Error> {
    let start = u32::try_from(range.start).map_err(|_wide_start| Error::RangeTooLarge)?;
    let end = u32::try_from(range.end).map_err(|_wide_end| Error::RangeTooLarge)?;

    let Ok(range_2) = ShortRange::new(start..end);

    Ok(range_2)
  }
}

impl ShortRange {
  #[inline]
  fn new(range: Range<u32>) -> core::result::Result<Self, console_core_never::Never> {
    Ok(ShortRange {
      start: range.start,
      end: range.end,
    })
  }

  #[inline]
  fn to_urange(self) -> core::result::Result<Range<u64>, console_core_never::Never> {
    let Ok(start) = console_core_number_conversion::wrapped::<_, u64>(self.start);
    let Ok(end) = console_core_number_conversion::wrapped::<_, u64>(self.end);

    Ok(start..end)
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(NonZeroU32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdOverflow;

impl fmt::Display for IdOverflow {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(to, "node identifier is outside its range")
  }
}

impl core::error::Error for IdOverflow {}

impl NodeId {
  pub const ROOT: Self = Self(NonZeroU32::MIN);

  pub fn new(id: u32) -> core::result::Result<Self, IdOverflow> {
    NonZeroU32::MIN.checked_add(id).map(Self).ok_or(IdOverflow)
  }

  fn from_count(count: u64) -> core::result::Result<Self, IdOverflow> {
    let id = u32::try_from(count).map_err(|_too_wide| IdOverflow)?;

    Self::new(id)
  }

  pub fn get(self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.0.get().saturating_sub(1))
  }

  pub fn position(self) -> core::result::Result<u64, console_core_never::Never> {
    let Ok(id) = self.get();

    Ok(u64::from(id))
  }

  fn following(self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(self.0.checked_add(1).map(Self))
  }
}

#[derive(Debug)]
enum NodeKind<'input> {
  Root,
  Element {
    tag_name: ExpandedNameIndexed<'input>,
    attributes: ShortRange,
    namespaces: ShortRange,
  },
  PI(PI<'input>),
  Comment(StringStorage<'input>),
  Text(StringStorage<'input>),
}

#[derive(Debug)]
struct NodeData<'input> {
  parent: Option<NodeId>,
  prev_sibling: Option<NodeId>,
  next_subtree: Option<NodeId>,
  last_child: Option<NodeId>,
  kind: NodeKind<'input>,
  #[cfg(feature = "positions")]
  range: Range<u64>,
}

#[derive(Clone, Eq, Debug)]
pub enum StringStorage<'input> {
  Borrowed(&'input str),

  Owned(Box<str>),
}

impl StringStorage<'_> {
  pub fn new_owned(s: &str) -> core::result::Result<Self, console_core_never::Never> {
    Ok(StringStorage::Owned(Box::from(s)))
  }

  pub fn as_str(&self) -> core::result::Result<&str, console_core_never::Never> {
    Ok(match self {
      StringStorage::Borrowed(s) => s,
      StringStorage::Owned(s) => s,
    })
  }
}

impl PartialEq for StringStorage<'_> {
  fn eq(&self, other: &Self) -> bool {
    ({
      let Ok(explicit_value) = self.as_str();
      explicit_value
    }) == ({
      let Ok(explicit_value) = other.as_str();
      explicit_value
    })
  }
}

impl core::fmt::Display for StringStorage<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{}",
      ({
        let Ok(explicit_value) = self.as_str();
        explicit_value
      })
    )
  }
}

impl core::ops::Deref for StringStorage<'_> {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    {
      let Ok(explicit_value) = self.as_str();
      explicit_value
    }
  }
}

#[derive(Clone, Debug)]
struct AttributeData<'input> {
  name: ExpandedNameIndexed<'input>,
  value: StringStorage<'input>,
  #[cfg(feature = "positions")]
  range: Range<u64>,
  #[cfg(feature = "positions")]
  qname_len: u16,
  #[cfg(feature = "positions")]
  eq_len: u8,
}

#[derive(Copy, Clone)]
pub struct Attribute<'a, 'input: 'a> {
  doc: &'a Document<'input>,
  data: &'a AttributeData<'input>,
}

impl<'a, 'input> Attribute<'a, 'input> {
  #[inline]
  pub fn namespace(&self) -> core::result::Result<Option<&'a str>, console_core_never::Never> {
    let Ok(namespace) = self.data.name.namespace(self.doc);

    Ok(namespace.map(|argument_0| {
      let Ok(uri) = Namespace::uri(argument_0);

      uri
    }))
  }

  #[inline]
  pub fn name(&self) -> core::result::Result<&'input str, console_core_never::Never> {
    Ok(self.data.name.local_name)
  }

  #[inline]
  pub fn value(&self) -> core::result::Result<&'a str, console_core_never::Never> {
    Ok(&self.data.value)
  }

  #[inline]
  pub fn value_storage(
    &self,
  ) -> core::result::Result<&StringStorage<'input>, console_core_never::Never> {
    Ok(&self.data.value)
  }

  #[deprecated(note = "replaced by `range`")]
  #[cfg(feature = "positions")]
  #[inline]
  pub fn position(&self) -> core::result::Result<u64, console_core_never::Never> {
    Ok(self.data.range.start)
  }

  #[cfg(feature = "positions")]
  #[inline]
  pub fn range(&self) -> core::result::Result<Range<u64>, console_core_never::Never> {
    Ok(self.data.range.clone())
  }

  #[cfg(feature = "positions")]
  #[inline]
  pub fn range_qname(&self) -> core::result::Result<Range<u64>, console_core_never::Never> {
    let Ok(start) = console_core_number_conversion::fitted::<_, u64>(self.data.range.start);
    let Ok(qname_len) =
      console_core_number_conversion::fitted::<_, u64>(u64::from(self.data.qname_len));
    let end = u64::wrapping_add(start, qname_len);

    Ok(self.data.range.start..end)
  }

  #[cfg(feature = "positions")]
  #[inline]
  pub fn range_value(&self) -> core::result::Result<Range<u64>, console_core_never::Never> {
    let Ok(own_start) = console_core_number_conversion::fitted::<_, u64>(self.data.range.start);
    let Ok(qname_len) =
      console_core_number_conversion::fitted::<_, u64>(u64::from(self.data.qname_len));
    let start = u64::wrapping_add(
      {
        let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(u64::wrapping_add(
          {
            let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(
              u64::wrapping_add(own_start, qname_len),
            );
            converted
          },
          {
            let Ok(converted) =
              console_core_number_conversion::fitted::<_, u64>(u64::from(self.data.eq_len));
            converted
          },
        ));
        converted
      },
      {
        let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(1);
        converted
      },
    );
    let Ok(own_end) = console_core_number_conversion::fitted::<_, u64>(self.data.range.end);
    let end = u64::wrapping_sub(own_end, {
      let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(1);
      converted
    });

    Ok(start..end)
  }
}

impl PartialEq for Attribute<'_, '_> {
  #[inline]
  fn eq(&self, other: &Attribute<'_, '_>) -> bool {
    let Ok(expanded_name) = self.data.name.as_expanded_name(self.doc);
    let Ok(other_expanded_name) = other.data.name.as_expanded_name(other.doc);

    expanded_name == other_expanded_name && self.data.value == other.data.value
  }
}

impl fmt::Debug for Attribute<'_, '_> {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    let Ok(expanded_name) = self.data.name.as_expanded_name(self.doc);

    write!(
      f,
      "Attribute {{ name: {:?}, value: {:?} }}",
      expanded_name, self.data.value
    )
  }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Namespace<'input> {
  name: Option<&'input str>,
  uri: StringStorage<'input>,
}

impl<'input> Namespace<'input> {
  #[inline]
  pub fn name(&self) -> core::result::Result<Option<&'input str>, console_core_never::Never> {
    Ok(self.name)
  }

  #[inline]
  pub fn uri(&self) -> core::result::Result<&str, console_core_never::Never> {
    Ok(self.uri.as_ref())
  }
}

#[derive(Default)]
struct Namespaces<'input> {
  values: Vec<Namespace<'input>>,

  tree_order: Vec<NamespaceIdx>,

  sorted_order: Vec<NamespaceIdx>,
}

impl<'input> Namespaces<'input> {
  fn push_ns(
    &mut self,
    name: Option<&'input str>,
    uri: StringStorage<'input>,
  ) -> Result<(), Error> {
    match name {
      Some("") => return Err(Error::InvalidNamespaceIndex),
      Some(_) | None => {}
    }

    let idx = match self.sorted_order.binary_search_by(|idx| {
      let Ok(position) = console_core_number_conversion::index(idx.0);
      let key = self
        .values
        .get(position)
        .map(|value| (value.name, value.uri.as_ref()));
      let Ok(uri_2) = uri.as_str();

      key.cmp(&Some((name, uri_2)))
    }) {
      Ok(sorted_idx) => self
        .sorted_order
        .get(sorted_idx)
        .copied()
        .ok_or(Error::InvalidNamespaceIndex)?,
      Err(sorted_idx) => {
        let Ok(limit) = console_core_number_conversion::index(u16::MAX);

        match self.values.len().cmp(&limit) {
          Ordering::Greater => {
            return Err(Error::NamespacesLimitReached);
          }
          Ordering::Less | Ordering::Equal => {}
        }

        let Ok(length) = console_core_number_conversion::wrapped::<_, u16>(self.values.len());
        let idx = NamespaceIdx(length);
        self.values.push(Namespace { name, uri });
        self.sorted_order.push(idx);
        let insertion_tail = self
          .sorted_order
          .get_mut(sorted_idx..)
          .ok_or(Error::InvalidNamespaceIndex)?;
        insertion_tail.rotate_right(1);
        idx
      }
    };

    self.tree_order.push(idx);

    Ok(())
  }

  #[inline]
  fn push_ref(&mut self, tree_idx: u64) -> Result<(), Error> {
    let Ok(position) = console_core_number_conversion::index(tree_idx);
    let idx = self
      .tree_order
      .get(position)
      .copied()
      .ok_or(Error::InvalidNamespaceIndex)?;
    self.tree_order.push(idx);

    Ok(())
  }

  #[inline]
  fn exists(
    &self,
    start: u64,
    prefix: Option<&str>,
  ) -> core::result::Result<crate::Presence, console_core_never::Never> {
    let Ok(range) = console_core_number_conversion::indices_from(start..);

    let matching = self
      .tree_order
      .get(range)
      .into_iter()
      .flatten()
      .find_map(|idx| {
        let Ok(namespace) = self.get(*idx);

        namespace.filter(|value| value.name == prefix)
      });

    Ok(match matching {
      Some(_namespace) => crate::Presence::Present,
      None => crate::Presence::Absent,
    })
  }

  fn shrink_to_fit(&mut self) -> core::result::Result<(), console_core_never::Never> {
    self.values.shrink_to_fit();
    self.tree_order.shrink_to_fit();

    self.sorted_order = Vec::new();

    Ok(())
  }

  #[inline]
  fn get(
    &self,
    idx: NamespaceIdx,
  ) -> core::result::Result<Option<&Namespace<'input>>, console_core_never::Never> {
    let Ok(position) = console_core_number_conversion::index(idx.0);

    Ok(self.values.get(position))
  }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
struct NamespaceIdx(u16);

#[derive(Clone, Copy, Debug)]
struct ExpandedNameIndexed<'input> {
  namespace_idx: Option<NamespaceIdx>,
  local_name: &'input str,
}

impl<'input> ExpandedNameIndexed<'input> {
  #[inline]
  fn namespace<'a>(
    &self,
    doc: &'a Document<'input>,
  ) -> core::result::Result<Option<&'a Namespace<'input>>, console_core_never::Never> {
    Ok(self.namespace_idx.and_then(|idx| {
      let Ok(namespace) = doc.namespaces.get(idx);

      namespace
    }))
  }

  #[inline]
  fn as_expanded_name<'a>(
    &self,
    doc: &'a Document<'input>,
  ) -> core::result::Result<ExpandedName<'a, 'input>, console_core_never::Never> {
    let Ok(namespace) = self.namespace(doc);

    Ok(ExpandedName {
      uri: namespace.map(|argument_0| {
        let Ok(uri) = Namespace::uri(argument_0);

        uri
      }),
      name: self.local_name,
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ExpandedName<'a, 'b> {
  uri: Option<&'a str>,
  name: &'b str,
}

impl<'a, 'b> ExpandedName<'a, 'b> {
  #[inline]
  pub fn namespace(&self) -> core::result::Result<Option<&'a str>, console_core_never::Never> {
    Ok(self.uri)
  }

  #[inline]
  pub fn name(&self) -> core::result::Result<&'b str, console_core_never::Never> {
    Ok(self.name)
  }
}

pub struct StaticName {
  pub uri: &'static str,
  pub name: &'static str,
}

impl ExpandedName<'static, 'static> {
  pub const fn from_static(
    name: StaticName,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      uri: Some(name.uri),
      name: name.name,
    })
  }
}

impl fmt::Debug for ExpandedName<'_, '_> {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    let Ok(namespace) = self.namespace();

    match namespace {
      Some(ns) => write!(f, "{{{}}}{}", ns, self.name),
      None => write!(f, "{}", self.name),
    }
  }
}

impl<'a, 'b> From<&'b str> for ExpandedName<'a, 'b> {
  #[inline]
  fn from(v: &'b str) -> Self {
    ExpandedName { uri: None, name: v }
  }
}

impl<'a, 'b> From<(&'a str, &'b str)> for ExpandedName<'a, 'b> {
  #[inline]
  fn from(v: (&'a str, &'b str)) -> Self {
    ExpandedName {
      uri: Some(v.0),
      name: v.1,
    }
  }
}

#[derive(Clone, Copy)]
pub struct Node<'a, 'input: 'a> {
  id: NodeId,

  doc: &'a Document<'input>,

  d: &'a NodeData<'input>,
}

impl Eq for Node<'_, '_> {}

impl PartialEq for Node<'_, '_> {
  #[inline]
  fn eq(&self, other: &Self) -> bool {
    (self.id, core::ptr::from_ref(self.doc)) == (other.id, core::ptr::from_ref(other.doc))
  }
}

impl PartialOrd for Node<'_, '_> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for Node<'_, '_> {
  fn cmp(&self, other: &Self) -> Ordering {
    (self.id.0, core::ptr::from_ref(self.doc)).cmp(&(other.id.0, core::ptr::from_ref(other.doc)))
  }
}

impl Hash for Node<'_, '_> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.id.0.hash(state);
    core::ptr::from_ref(self.doc).hash(state);
    core::ptr::from_ref(self.d).hash(state);
  }
}

impl<'a, 'input: 'a> Node<'a, 'input> {
  #[inline]
  pub fn node_type(&self) -> core::result::Result<NodeType, console_core_never::Never> {
    Ok(match self.d.kind {
      NodeKind::Root => NodeType::Root,
      NodeKind::Element { .. } => NodeType::Element,
      NodeKind::PI { .. } => NodeType::PI,
      NodeKind::Comment(_) => NodeType::Comment,
      NodeKind::Text(_) => NodeType::Text,
    })
  }

  #[inline]
  pub fn is_root(&self) -> core::result::Result<crate::NodeMatch, console_core_never::Never> {
    let Ok(node_type) = self.node_type();

    Ok(crate::NodeMatch::from(node_type == NodeType::Root))
  }

  #[inline]
  pub fn is_element(&self) -> core::result::Result<crate::NodeMatch, console_core_never::Never> {
    let Ok(node_type) = self.node_type();

    Ok(crate::NodeMatch::from(node_type == NodeType::Element))
  }

  #[inline]
  pub fn is_pi(&self) -> core::result::Result<crate::NodeMatch, console_core_never::Never> {
    let Ok(node_type) = self.node_type();

    Ok(crate::NodeMatch::from(node_type == NodeType::PI))
  }

  #[inline]
  pub fn is_comment(&self) -> core::result::Result<crate::NodeMatch, console_core_never::Never> {
    let Ok(node_type) = self.node_type();

    Ok(crate::NodeMatch::from(node_type == NodeType::Comment))
  }

  #[inline]
  pub fn is_text(&self) -> core::result::Result<crate::NodeMatch, console_core_never::Never> {
    let Ok(node_type) = self.node_type();

    Ok(crate::NodeMatch::from(node_type == NodeType::Text))
  }

  #[inline]
  pub fn document(
    &self,
  ) -> core::result::Result<&'a Document<'input>, console_core_never::Never> {
    Ok(self.doc)
  }

  #[inline]
  pub fn tag_name(
    &self,
  ) -> core::result::Result<ExpandedName<'a, 'input>, console_core_never::Never> {
    Ok(match self.d.kind {
      NodeKind::Element { ref tag_name, .. } => {
        let Ok(expanded_name) = tag_name.as_expanded_name(self.doc);

        expanded_name
      }
      NodeKind::Root | NodeKind::PI(_) | NodeKind::Text(_) | NodeKind::Comment(_) => {
        ExpandedName::from("")
      }
    })
  }

  pub fn has_tag_name<'n, 'm, N>(
    &self,
    name: N,
  ) -> core::result::Result<crate::NodeMatch, console_core_never::Never>
  where
    ExpandedName<'n, 'm>: From<N>,
  {
    let name_2 = ExpandedName::from(name);

    Ok(match self.d.kind {
      NodeKind::Element { ref tag_name, .. } => {
        let Ok(namespace) = name_2.namespace();

        match namespace {
          Some(_) => {
            let Ok(expanded_name) = tag_name.as_expanded_name(self.doc);

            crate::NodeMatch::from(expanded_name == name_2)
          }
          None => crate::NodeMatch::from(tag_name.local_name == name_2.name),
        }
      }
      NodeKind::Root | NodeKind::PI(_) | NodeKind::Text(_) | NodeKind::Comment(_) => {
        crate::NodeMatch::Different
      }
    })
  }

  pub fn default_namespace(
    &self,
  ) -> core::result::Result<Option<&'a str>, console_core_never::Never> {
    let Ok(mut namespaces) = self.namespaces();

    Ok(
      namespaces
        .find(|ns| ns.name.is_none())
        .map(|v| v.uri.as_ref()),
    )
  }

  pub fn lookup_prefix(
    &self,
    uri: &str,
  ) -> core::result::Result<Option<&'input str>, console_core_never::Never> {
    match uri {
      NS_XML_URI => {
        return Ok(Some(NS_XML_PREFIX));
      }
      _ => {}
    }

    let Ok(mut namespaces) = self.namespaces();

    Ok(
      namespaces
        .find(|ns| &*ns.uri == uri)
        .and_then(|value| value.name),
    )
  }

  pub fn lookup_namespace_uri(
    &self,
    prefix: Option<&str>,
  ) -> core::result::Result<Option<&'a str>, console_core_never::Never> {
    let Ok(mut namespaces) = self.namespaces();

    Ok(
      namespaces
        .find(|ns| ns.name == prefix)
        .map(|v| v.uri.as_ref()),
    )
  }

  pub fn attribute<'n, 'm, N>(
    &self,
    name: N,
  ) -> core::result::Result<Option<&'a str>, console_core_never::Never>
  where
    ExpandedName<'n, 'm>: From<N>,
  {
    let Ok(attribute_node) = self.attribute_node(name);

    Ok(attribute_node.map(|a| {
      let Ok(explicit_value) = a.value();
      explicit_value
    }))
  }

  pub fn attribute_node<'n, 'm, N>(
    &self,
    name: N,
  ) -> core::result::Result<Option<Attribute<'a, 'input>>, console_core_never::Never>
  where
    ExpandedName<'n, 'm>: From<N>,
  {
    let name_2 = ExpandedName::from(name);

    let Ok(namespace) = name_2.namespace();

    Ok(match namespace {
      Some(_) => {
        let Ok(mut attributes) = self.attributes();

        attributes.find(|a| {
          let Ok(expanded_name) = a.data.name.as_expanded_name(self.doc);

          expanded_name == name_2
        })
      }
      None => {
        let Ok(mut attributes) = self.attributes();

        attributes.find(|a| a.data.name.local_name == name_2.name)
      }
    })
  }

  pub fn has_attribute<'n, 'm, N>(
    &self,
    name: N,
  ) -> core::result::Result<crate::Presence, console_core_never::Never>
  where
    ExpandedName<'n, 'm>: From<N>,
  {
    let Ok(attribute_node) = self.attribute_node(name);

    Ok(match attribute_node {
      ::core::option::Option::Some(_) => crate::Presence::Present,
      ::core::option::Option::None => crate::Presence::Absent,
    })
  }

  #[inline]
  pub fn attributes(
    &self,
  ) -> core::result::Result<Attributes<'a, 'input>, console_core_never::Never> {
    let Ok(attributes) = Attributes::new(self);

    Ok(attributes)
  }

  #[inline]
  pub fn namespaces(
    &self,
  ) -> core::result::Result<NamespaceIter<'a, 'input>, console_core_never::Never> {
    let namespaces = match self.d.kind {
      NodeKind::Element { ref namespaces, .. } => {
        let Ok(range) = namespaces.to_urange();
        let Ok(range_2) = console_core_number_conversion::indices(range);

        match self.doc.namespaces.tree_order.get(range_2) {
          Some(namespaces_2) => namespaces_2,
          None => &[],
        }
      }
      NodeKind::Root | NodeKind::PI(_) | NodeKind::Text(_) | NodeKind::Comment(_) => &[],
    };

    Ok(NamespaceIter {
      doc: self.doc,
      namespaces: namespaces.iter(),
    })
  }

  #[inline]
  pub fn text(&self) -> core::result::Result<Option<&'a str>, console_core_never::Never> {
    let Ok(text_storage) = self.text_storage();

    Ok(text_storage.map(|s| {
      let Ok(explicit_value) = s.as_str();
      explicit_value
    }))
  }

  pub fn text_storage(
    &self,
  ) -> core::result::Result<Option<&'a StringStorage<'input>>, console_core_never::Never> {
    Ok(match &self.d.kind {
      NodeKind::Element { .. } => {
        let Ok(first) = self.first_child();

        match first {
          Some(child) => match &child.d.kind {
            NodeKind::Text(text) => Some(text),
            NodeKind::Root
            | NodeKind::Element { .. }
            | NodeKind::PI(_)
            | NodeKind::Comment(_) => None,
          },
          None => None,
        }
      }
      NodeKind::Comment(text) | NodeKind::Text(text) => Some(text),
      NodeKind::Root | NodeKind::PI(_) => None,
    })
  }

  #[inline]
  pub fn tail(&self) -> core::result::Result<Option<&'a str>, console_core_never::Never> {
    let Ok(tail_storage) = self.tail_storage();

    Ok(tail_storage.map(|s| {
      let Ok(explicit_value) = s.as_str();
      explicit_value
    }))
  }

  pub fn tail_storage(
    &self,
  ) -> core::result::Result<Option<&'a StringStorage<'input>>, console_core_never::Never> {
    match self.d.kind {
      NodeKind::Element { .. } => {}
      NodeKind::Root | NodeKind::PI(_) | NodeKind::Comment(_) | NodeKind::Text(_) => {
        return Ok(None);
      }
    }

    let Ok(next) = self.next_sibling();

    Ok(match next {
      Some(node) => match &node.d.kind {
        NodeKind::Text(text) => Some(text),
        NodeKind::Root | NodeKind::Element { .. } | NodeKind::PI(_) | NodeKind::Comment(_) => {
          None
        }
      },
      None => None,
    })
  }

  #[inline]
  pub fn pi(&self) -> core::result::Result<Option<PI<'input>>, console_core_never::Never> {
    Ok(match self.d.kind {
      NodeKind::PI(pi) => Some(pi),
      NodeKind::Root | NodeKind::Element { .. } | NodeKind::Text(_) | NodeKind::Comment(_) => {
        None
      }
    })
  }

  #[inline]
  pub fn parent(&self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(self.d.parent.and_then(|id| {
      let Ok(node) = self.doc.get_node(id);

      node
    }))
  }

  pub fn parent_element(&self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(ancestors) = self.ancestors();

    Ok(ancestors.skip(1).find(|n| {
      let Ok(is_element) = n.is_element();

      is_element == crate::NodeMatch::Matches
    }))
  }

  #[inline]
  pub fn prev_sibling(&self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(self.d.prev_sibling.and_then(|id| {
      let Ok(node) = self.doc.get_node(id);

      node
    }))
  }

  pub fn prev_sibling_element(
    &self,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(prev_siblings) = self.prev_siblings();

    Ok(prev_siblings.skip(1).find(|n| {
      let Ok(is_element) = n.is_element();

      is_element == crate::NodeMatch::Matches
    }))
  }

  #[inline]
  pub fn next_sibling(&self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let next = self.d.next_subtree.and_then(|id| {
      let Ok(node) = self.doc.get_node(id);

      node
    });

    Ok(next.filter(|node| node.d.prev_sibling == Some(self.id)))
  }

  pub fn next_sibling_element(
    &self,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(siblings) = self.next_siblings();

    Ok(siblings.skip(1).find(|n| {
      let Ok(is_element) = n.is_element();

      is_element == crate::NodeMatch::Matches
    }))
  }

  #[inline]
  pub fn first_child(&self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    match self.d.last_child {
      Some(_last) => {}
      None => return Ok(None),
    }

    let Ok(following) = self.id.following();

    match following {
      Some(id) => self.doc.get_node(id),
      None => Ok(None),
    }
  }

  pub fn first_element_child(
    &self,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(mut children) = self.children();

    Ok(children.find(|n| {
      let Ok(is_element) = n.is_element();

      is_element == crate::NodeMatch::Matches
    }))
  }

  #[inline]
  pub fn last_child(&self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(self.d.last_child.and_then(|id| {
      let Ok(node) = self.doc.get_node(id);

      node
    }))
  }

  pub fn last_element_child(
    &self,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(mut children) = self.children();

    Ok(children.rfind(|n| {
      let Ok(is_element) = n.is_element();

      is_element == crate::NodeMatch::Matches
    }))
  }

  #[inline]
  pub fn has_siblings(&self) -> core::result::Result<crate::Presence, console_core_never::Never> {
    match self.d.prev_sibling {
      Some(_previous) => return Ok(crate::Presence::Present),
      None => {}
    }

    let Ok(next) = self.next_sibling();

    Ok(match next {
      Some(_next_sibling) => crate::Presence::Present,
      None => crate::Presence::Absent,
    })
  }

  #[inline]
  pub fn has_children(&self) -> core::result::Result<crate::Presence, console_core_never::Never> {
    Ok(match self.d.last_child {
      Some(_last_child) => crate::Presence::Present,
      None => crate::Presence::Absent,
    })
  }

  #[inline]
  pub fn ancestors(
    &self,
  ) -> core::result::Result<AxisIter<'a, 'input>, console_core_never::Never> {
    Ok(AxisIter {
      node: Some(*self),
      next: |argument_0| {
        let Ok(parent) = Node::parent(argument_0);

        parent
      },
    })
  }

  #[inline]
  pub fn prev_siblings(
    &self,
  ) -> core::result::Result<AxisIter<'a, 'input>, console_core_never::Never> {
    Ok(AxisIter {
      node: Some(*self),
      next: |argument_0| {
        let Ok(prev_sibling) = Node::prev_sibling(argument_0);

        prev_sibling
      },
    })
  }

  #[inline]
  pub fn next_siblings(
    &self,
  ) -> core::result::Result<AxisIter<'a, 'input>, console_core_never::Never> {
    Ok(AxisIter {
      node: Some(*self),
      next: |argument_0| {
        let Ok(sibling) = Node::next_sibling(argument_0);

        sibling
      },
    })
  }

  #[inline]
  pub fn first_children(
    &self,
  ) -> core::result::Result<AxisIter<'a, 'input>, console_core_never::Never> {
    Ok(AxisIter {
      node: Some(*self),
      next: |argument_0| {
        let Ok(first_child) = Node::first_child(argument_0);

        first_child
      },
    })
  }

  #[inline]
  pub fn last_children(
    &self,
  ) -> core::result::Result<AxisIter<'a, 'input>, console_core_never::Never> {
    Ok(AxisIter {
      node: Some(*self),
      next: |argument_0| {
        let Ok(last_child) = Node::last_child(argument_0);

        last_child
      },
    })
  }

  #[inline]
  pub fn children(
    &self,
  ) -> core::result::Result<Children<'a, 'input>, console_core_never::Never> {
    let Ok(front) = self.first_child();
    let Ok(back) = self.last_child();

    Ok(Children { front, back })
  }

  #[inline]
  pub fn descendants(
    &self,
  ) -> core::result::Result<Descendants<'a, 'input>, console_core_never::Never> {
    let Ok(descendants) = Descendants::new(*self);

    Ok(descendants)
  }

  #[cfg(feature = "positions")]
  #[inline]
  pub fn range(&self) -> core::result::Result<Range<u64>, console_core_never::Never> {
    Ok(self.d.range.clone())
  }

  #[inline]
  pub fn id(&self) -> core::result::Result<NodeId, console_core_never::Never> {
    Ok(self.id)
  }
}

impl<'a, 'input: 'a> fmt::Debug for Node<'a, 'input> {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    match self.d.kind {
      NodeKind::Root => write!(f, "Root"),
      NodeKind::Element { .. } => {
        let Ok(tag_name) = self.tag_name();
        let Ok(attributes) = self.attributes();
        let Ok(namespaces) = self.namespaces();

        write!(
          f,
          "Element {{ tag_name: {:?}, attributes: {:?}, namespaces: {:?} }}",
          tag_name, attributes, namespaces
        )
      }
      NodeKind::PI(pi) => {
        write!(f, "PI {{ target: {:?}, value: {:?} }}", pi.target, pi.value)
      }
      NodeKind::Comment(ref text) => write!(
        f,
        "Comment({:?})",
        ({
          let Ok(explicit_value) = text.as_str();
          explicit_value
        })
      ),
      NodeKind::Text(ref text) => write!(
        f,
        "Text({:?})",
        ({
          let Ok(explicit_value) = text.as_str();
          explicit_value
        })
      ),
    }
  }
}

#[derive(Clone)]
pub struct Attributes<'a, 'input> {
  doc: &'a Document<'input>,
  attrs: core::slice::Iter<'a, AttributeData<'input>>,
}

impl<'a, 'input> Attributes<'a, 'input> {
  #[inline]
  fn new(
    node: &Node<'a, 'input>,
  ) -> core::result::Result<Attributes<'a, 'input>, console_core_never::Never> {
    let attrs = match node.d.kind {
      NodeKind::Element { ref attributes, .. } => {
        let Ok(range) = attributes.to_urange();
        let Ok(range_2) = console_core_number_conversion::indices(range);

        match node.doc.attributes.get(range_2) {
          Some(attributes_2) => attributes_2,
          None => &[],
        }
      }
      NodeKind::Root | NodeKind::PI(_) | NodeKind::Text(_) | NodeKind::Comment(_) => &[],
    };

    Ok(Attributes {
      doc: node.doc,
      attrs: attrs.iter(),
    })
  }
}

impl<'a, 'input> Iterator for Attributes<'a, 'input> {
  type Item = Attribute<'a, 'input>;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    self.attrs.next().map(|attr| Attribute {
      doc: self.doc,
      data: attr,
    })
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "Iterator fixes the index and size types in this method signature"
    )
  )]
  fn nth(&mut self, n: usize) -> Option<Self::Item> {
    self.attrs.nth(n).map(|attr| Attribute {
      doc: self.doc,
      data: attr,
    })
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "Iterator fixes the index and size types in this method signature"
    )
  )]
  fn size_hint(&self) -> (usize, Option<usize>) {
    self.attrs.size_hint()
  }
}

impl<'a, 'input> DoubleEndedIterator for Attributes<'a, 'input> {
  #[inline]
  fn next_back(&mut self) -> Option<Self::Item> {
    self.attrs.next_back().map(|attr| Attribute {
      doc: self.doc,
      data: attr,
    })
  }
}

impl ExactSizeIterator for Attributes<'_, '_> {}

impl fmt::Debug for Attributes<'_, '_> {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.debug_struct("Attributes")
      .field("attrs", &self.attrs)
      .finish()
  }
}

#[derive(Clone)]
pub struct AxisIter<'a, 'input: 'a> {
  node: Option<Node<'a, 'input>>,
  next: fn(&Node<'a, 'input>) -> Option<Node<'a, 'input>>,
}

impl<'a, 'input: 'a> Iterator for AxisIter<'a, 'input> {
  type Item = Node<'a, 'input>;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    let node = self.node.take();
    self.node = node.as_ref().and_then(self.next);
    node
  }
}

impl fmt::Debug for AxisIter<'_, '_> {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.debug_struct("AxisIter")
      .field("node", &self.node)
      .field("next", &"fn()")
      .finish()
  }
}

#[derive(Clone, Debug)]
pub struct Children<'a, 'input: 'a> {
  front: Option<Node<'a, 'input>>,
  back: Option<Node<'a, 'input>>,
}

impl<'a, 'input: 'a> Iterator for Children<'a, 'input> {
  type Item = Node<'a, 'input>;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    match self.front.cmp(&self.back) {
      Ordering::Equal => {
        let node = self.front.take();
        self.back = None;
        node
      }
      Ordering::Less | Ordering::Greater => {
        let node = self.front.take();
        self.front = node.as_ref().and_then(|argument_0| {
          let Ok(sibling) = Node::next_sibling(argument_0);

          sibling
        });
        node
      }
    }
  }
}

impl<'a, 'input: 'a> DoubleEndedIterator for Children<'a, 'input> {
  #[inline]
  fn next_back(&mut self) -> Option<Self::Item> {
    match self.back.cmp(&self.front) {
      Ordering::Equal => {
        let node = self.back.take();
        self.front = None;
        node
      }
      Ordering::Less | Ordering::Greater => {
        let node = self.back.take();
        self.back = node.as_ref().and_then(|argument_0| {
          let Ok(prev_sibling) = Node::prev_sibling(argument_0);

          prev_sibling
        });
        node
      }
    }
  }
}

#[derive(Clone)]
pub struct Descendants<'a, 'input> {
  doc: &'a Document<'input>,
  nodes: Range<u64>,
}

impl<'a, 'input> Descendants<'a, 'input> {
  fn new(start: Node<'a, 'input>) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(from) = start.id.position();
    let until = match start.d.next_subtree {
      Some(id) => {
        let Ok(id_position) = id.position();

        id_position
      }
      None => {
        let Ok(start_length) = start.doc.nodes.len();

        start_length
      }
    };

    Ok(Self {
      doc: start.doc,
      nodes: from..until,
    })
  }
}

impl<'a, 'input> Iterator for Descendants<'a, 'input> {
  type Item = Node<'a, 'input>;

  fn next(&mut self) -> Option<Self::Item> {
    self.nodes.next().and_then(|position| {
      let Ok(found) = self.doc.node_at(position);

      found
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "Iterator fixes the index type in this method signature"
    )
  )]
  fn nth(&mut self, n: usize) -> Option<Self::Item> {
    self.nodes.nth(n).and_then(|position| {
      let Ok(found) = self.doc.node_at(position);

      found
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "Iterator fixes the size types in this method signature"
    )
  )]
  fn size_hint(&self) -> (usize, Option<usize>) {
    self.nodes.size_hint()
  }
}

impl<'a, 'input> DoubleEndedIterator for Descendants<'a, 'input> {
  fn next_back(&mut self) -> Option<Self::Item> {
    self.nodes.next_back().and_then(|position| {
      let Ok(found) = self.doc.node_at(position);

      found
    })
  }
}

impl ExactSizeIterator for Descendants<'_, '_> {}

impl fmt::Debug for Descendants<'_, '_> {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.debug_struct("Descendants")
      .field("nodes", &self.nodes)
      .finish()
  }
}

#[derive(Clone)]
pub struct NamespaceIter<'a, 'input> {
  doc: &'a Document<'input>,
  namespaces: core::slice::Iter<'a, NamespaceIdx>,
}

impl<'a, 'input> Iterator for NamespaceIter<'a, 'input> {
  type Item = &'a Namespace<'input>;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    self.namespaces.next().and_then(|idx| {
      let Ok(namespace) = self.doc.namespaces.get(*idx);

      namespace
    })
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "Iterator fixes the index and size types in this method signature"
    )
  )]
  fn nth(&mut self, n: usize) -> Option<Self::Item> {
    self.namespaces.nth(n).and_then(|idx| {
      let Ok(namespace) = self.doc.namespaces.get(*idx);

      namespace
    })
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "Iterator fixes the index and size types in this method signature"
    )
  )]
  fn size_hint(&self) -> (usize, Option<usize>) {
    self.namespaces.size_hint()
  }
}

impl<'a, 'input> DoubleEndedIterator for NamespaceIter<'a, 'input> {
  #[inline]
  fn next_back(&mut self) -> Option<Self::Item> {
    self.namespaces.next().and_then(|idx| {
      let Ok(namespace) = self.doc.namespaces.get(*idx);

      namespace
    })
  }
}

impl ExactSizeIterator for NamespaceIter<'_, '_> {}

impl fmt::Debug for NamespaceIter<'_, '_> {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.debug_struct("NamespaceIter")
      .field("namespaces", &self.namespaces)
      .finish()
  }
}

mod answers;
pub use answers::*;

mod node_list;
use node_list::NodeList;

#[cfg(test)]
mod tree_tests;

#[cfg(test)]
mod test_support;
